#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${BIN:-target/release/hyphlab}"
MODEL_ROOT="${MODEL_ROOT:-models/guarded_ngram/v1}"
MANIFEST_ROOT="${MANIFEST_ROOT:-manifests/guarded_ngram/v1}"
DATASETS="${DATASETS:-moby_en_us wiktextract_cs wiktextract_de wiktextract_es wiktextract_it wiktextract_nl wiktextract_ru_cyrl_trusted_dedup wiktextract_tr}"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

mkdir -p "$MODEL_ROOT" "$MANIFEST_ROOT"

dataset_config() {
  local dataset="$1"
  GOLD=""
  LOCALE=""
  PATTERNS=""
  MODEL_KIND=""
  RECIPE=""
  SLUG=""

  case "$dataset" in
    moby_en_us)
      GOLD="data/gold/moby_en_us.jsonl.zst"
      LOCALE="en-US"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-en-us.tex"
      MODEL_KIND="safe-ngram"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85"
      SLUG="guarded_ngram"
      ;;
    wiktextract_cs)
      GOLD="data/gold/wiktextract/cs.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      MODEL_KIND="safe-ngram"
      RECIPE="safe-ngram-unicode-2x2-s1-p50"
      SLUG="guarded_ngram"
      ;;
    wiktextract_de)
      GOLD="data/gold/wiktextract/de.jsonl.zst"
      LOCALE="de"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      MODEL_KIND="safe-ngram"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      ;;
    wiktextract_es)
      GOLD="data/gold/wiktextract/es.jsonl.zst"
      LOCALE="es"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-es.tex"
      MODEL_KIND="safe-ngram"
      RECIPE="safe-ngram-unicode-3x2-s1-p60"
      SLUG="guarded_ngram"
      ;;
    wiktextract_it)
      GOLD="data/gold/wiktextract/it.jsonl.zst"
      LOCALE="it"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-it.tex"
      MODEL_KIND="italian-syllable-model"
      RECIPE="italian-syllable"
      SLUG="italian_onset_syllable"
      ;;
    wiktextract_nl)
      GOLD="data/gold/wiktextract/nl.jsonl.zst"
      LOCALE="nl"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-nl.tex"
      MODEL_KIND="safe-ngram"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      ;;
    wiktextract_ru_cyrl_trusted_dedup)
      GOLD="data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      MODEL_KIND="safe-ngram"
      RECIPE="safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      ;;
    wiktextract_tr)
      GOLD="data/gold/wiktextract/tr.jsonl.zst"
      LOCALE="tr"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-tr.tex"
      MODEL_KIND="safe-ngram"
      RECIPE="safe-ngram-unicode-mixcv-2x2-s1-p70"
      SLUG="guarded_ngram"
      ;;
    *)
      printf 'unknown dataset: %s\n' "$dataset" >&2
      exit 1
      ;;
  esac
}

write_manifest() {
  local dataset="$1"
  local manifest="$2"
  local model="$3"
  mkdir -p "$(dirname "$manifest")"

  {
    printf '# Reusable runtime manifest.\n'
    printf '# Model trained from full normalized corpus: %s\n' "$GOLD"
    printf '# Use split-based or 5-fold runs for unbiased accuracy evaluation.\n\n'
    printf '[[methods]]\n'
    printf 'slug = "hypher"\n'
    printf 'method = "hypher"\n\n'
    printf '[[methods]]\n'
    printf 'slug = "liang_tex"\n'
    printf 'method = "liang"\n'
    printf 'requires_patterns = true\n\n'
    printf '[[methods]]\n'
    printf 'slug = "%s"\n' "$SLUG"
    case "$MODEL_KIND" in
      safe-ngram)
        printf 'method = "safe-ngram-model"\n'
        printf 'dictionary = "%s"\n' "$model"
        ;;
      italian-syllable-model)
        printf 'method = "italian-syllable-model"\n'
        printf 'dictionary = "%s"\n' "$model"
        ;;
      *)
        printf 'unknown model kind for %s: %s\n' "$dataset" "$MODEL_KIND" >&2
        exit 1
        ;;
    esac
    printf '\n'
  } > "$manifest"
}

index="$MODEL_ROOT/README.md"
cat > "$index" <<'EOF'
# Guarded N-gram Models

These are reusable runtime models built from the full normalized corpora listed
below. Use them for demos, application integration, and quick experiments. For
unbiased accuracy claims, use the 5-fold scripts so every fold trains on its
train split and evaluates on held-out data.

## Use

Build the CLI once:

```bash
cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
```

List the reusable models and run one directly:

```bash
target/release/hyphlab predict --list-saved-models
target/release/hyphlab predict --saved-model en-US --word hyphenation --word typesetting
target/release/hyphlab predict --saved-model de --text "Silbentrennung fuer lange Woerter"
```

Try the Italian onset-syllable model:

```bash
target/release/hyphlab predict --saved-model it --word informazione --word straordinario
```

The manifests in `manifests/guarded_ngram/v1/` can be passed to
`target/release/hyphlab matrix`; their model paths are relative to the manifest
file location.

## Which Model Should I Use?

Use the model whose locale and source corpus match your target:

- English en-US: `moby_en_us.bin`, trained from Moby Hyphenator II.
- Czech, German, Spanish, Dutch, Russian, and Turkish: the matching
  `wiktextract_*.bin` model, trained from normalized Wiktextract / Kaikki
  entries for that language.
- Italian: `wiktextract_it.json`, an onset-syllable model trained from
  normalized Italian Wiktextract / Kaikki entries.

These files are full-corpus runtime models. Do not evaluate them on the same
full corpus as an independent test. For reproducible comparisons against Hypher
or Liang baselines, use `docs/reports/multilingual_5fold_v1/` or rerun
`scripts/run_multilingual_5fold_evaluation.sh`.

## Inventory

| dataset | locale | trained from | training policy | slug | recipe | model | manifest |
| --- | --- | --- | --- | --- | --- | --- | --- |
EOF

for dataset in $DATASETS; do
  dataset_config "$dataset"
  if [ ! -s "$GOLD" ]; then
    printf 'skip %s: missing or empty gold %s\n' "$dataset" "$GOLD"
    continue
  fi

  case "$MODEL_KIND" in
    italian-syllable-model)
      model="$MODEL_ROOT/$dataset.json"
      ;;
    *)
      model="$MODEL_ROOT/$dataset.bin"
      ;;
  esac
  manifest="$MANIFEST_ROOT/$dataset.toml"
  manifest_model="$model"
  case "$manifest_model" in
    models/*)
      manifest_model="../../../$manifest_model"
      ;;
  esac

  case "$MODEL_KIND" in
    safe-ngram)
      "$BIN" compile-safe-ngram \
        --gold "$GOLD" \
        --locale "$LOCALE" \
        --method "$RECIPE" \
        --output "$model"
      ;;
    italian-syllable-model)
      "$BIN" compile-italian-syllable \
        --gold "$GOLD" \
        --locale "$LOCALE" \
        --method "$RECIPE" \
        --output "$model"
      ;;
    *)
      printf 'unknown model kind: %s\n' "$MODEL_KIND" >&2
      exit 1
      ;;
  esac

  write_manifest "$dataset" "$manifest" "$manifest_model"

  {
    printf '| `%s` | `%s` | `%s` | full normalized corpus | `%s` | `%s` | `%s` | `%s` |\n' \
      "$dataset" "$LOCALE" "$GOLD" "$SLUG" "$RECIPE" "$model" "$manifest"
  } >> "$index"
done

printf 'wrote %s\n' "$index"
