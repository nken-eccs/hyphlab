#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${BIN:-target/release/hyphlab}"
MODEL_ROOT="${MODEL_ROOT:-models/guarded_ngram/v1}"
MANIFEST_ROOT="${MANIFEST_ROOT:-manifests/guarded_ngram/v1}"
DATASETS="${DATASETS:-moby_en_us moby_en_us_typeset wiktextract_cs wiktextract_cs_typeset wiktextract_de wiktextract_de_typeset wiktextract_es wiktextract_es_typeset wiktextract_it wiktextract_it_typeset wiktextract_nl wiktextract_nl_typeset wiktextract_ru_cyrl_trusted_dedup wiktextract_ru_cyrl_trusted_dedup_typeset wiktextract_tr wiktextract_tr_typeset}"

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
  RUNTIME_METHOD=""
  RECIPE=""
  SLUG=""
  FRAGMENTS=""
  TRAINING_POLICY="full normalized corpus"

  case "$dataset" in
    moby_en_us)
      GOLD="data/gold/moby_en_us.jsonl.zst"
      LOCALE="en-US"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-en-us.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="safe-ngram-model"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85"
      SLUG="guarded_ngram"
      ;;
    moby_en_us_typeset)
      GOLD="data/gold/moby_en_us_typeset.jsonl.zst"
      LOCALE="en-US"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-en-us.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="typeset-safe-ngram-model"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85"
      SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/moby_en_us.txt"
      TRAINING_POLICY="full curated corpus plus runtime fragment guard"
      ;;
    wiktextract_cs)
      GOLD="data/gold/wiktextract/cs.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="safe-ngram-model"
      RECIPE="safe-ngram-unicode-2x2-s1-p50"
      SLUG="guarded_ngram"
      ;;
    wiktextract_cs_typeset)
      GOLD="data/gold/wiktextract/cs_typeset.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="typeset-safe-ngram-model"
      RECIPE="safe-ngram-unicode-2x2-s1-p50"
      SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_cs.txt"
      TRAINING_POLICY="full curated corpus plus runtime fragment guard"
      ;;
    wiktextract_de)
      GOLD="data/gold/wiktextract/de.jsonl.zst"
      LOCALE="de"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="safe-ngram-model"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      ;;
    wiktextract_de_typeset)
      GOLD="data/gold/wiktextract/de_typeset.jsonl.zst"
      LOCALE="de"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="typeset-safe-ngram-model"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_de.txt"
      TRAINING_POLICY="full curated corpus plus runtime fragment guard"
      ;;
    wiktextract_es)
      GOLD="data/gold/wiktextract/es.jsonl.zst"
      LOCALE="es"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-es.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="safe-ngram-model"
      RECIPE="safe-ngram-unicode-3x2-s1-p60"
      SLUG="guarded_ngram"
      ;;
    wiktextract_es_typeset)
      GOLD="data/gold/wiktextract/es_typeset.jsonl.zst"
      LOCALE="es"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-es.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="typeset-safe-ngram-model"
      RECIPE="safe-ngram-unicode-3x2-s1-p60"
      SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_es.txt"
      TRAINING_POLICY="full curated corpus plus runtime fragment guard"
      ;;
    wiktextract_it)
      GOLD="data/gold/wiktextract/it.jsonl.zst"
      LOCALE="it"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-it.tex"
      MODEL_KIND="italian-syllable-model"
      RUNTIME_METHOD="italian-syllable-model"
      RECIPE="italian-syllable"
      SLUG="italian_onset_syllable"
      ;;
    wiktextract_it_typeset)
      GOLD="data/gold/wiktextract/it_typeset.jsonl.zst"
      LOCALE="it"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-it.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="typeset-safe-ngram-model"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_it.txt"
      TRAINING_POLICY="full curated corpus plus runtime fragment guard"
      ;;
    wiktextract_nl)
      GOLD="data/gold/wiktextract/nl.jsonl.zst"
      LOCALE="nl"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-nl.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="safe-ngram-model"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      ;;
    wiktextract_nl_typeset)
      GOLD="data/gold/wiktextract/nl_typeset.jsonl.zst"
      LOCALE="nl"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-nl.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="typeset-safe-ngram-model"
      RECIPE="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_nl.txt"
      TRAINING_POLICY="full curated corpus plus runtime fragment guard"
      ;;
    wiktextract_ru_cyrl_trusted_dedup)
      GOLD="data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="safe-ngram-model"
      RECIPE="safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      ;;
    wiktextract_ru_cyrl_trusted_dedup_typeset)
      GOLD="data/gold/wiktextract/ru_cyrl_trusted_dedup_typeset.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="typeset-safe-ngram-model"
      RECIPE="safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80"
      SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_ru_cyrl_trusted_dedup.txt"
      TRAINING_POLICY="full curated corpus plus runtime fragment guard"
      ;;
    wiktextract_tr)
      GOLD="data/gold/wiktextract/tr.jsonl.zst"
      LOCALE="tr"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-tr.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="safe-ngram-model"
      RECIPE="safe-ngram-unicode-mixcv-2x2-s1-p70"
      SLUG="guarded_ngram"
      ;;
    wiktextract_tr_typeset)
      GOLD="data/gold/wiktextract/tr_typeset.jsonl.zst"
      LOCALE="tr"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-tr.tex"
      MODEL_KIND="safe-ngram"
      RUNTIME_METHOD="typeset-safe-ngram-model"
      RECIPE="safe-ngram-unicode-mixcv-2x2-s1-p70"
      SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_tr.txt"
      TRAINING_POLICY="full curated corpus plus runtime fragment guard"
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
  local manifest_fragments="$FRAGMENTS"
  case "$manifest_fragments" in
    data/*)
      manifest_fragments="../../../$manifest_fragments"
      ;;
  esac

  {
    printf '# Reusable runtime manifest.\n'
    printf '# Model trained from %s: %s\n' "$TRAINING_POLICY" "$GOLD"
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
        printf 'method = "%s"\n' "$RUNTIME_METHOD"
        printf 'dictionary = "%s"\n' "$model"
        ;;
      italian-syllable-model)
        printf 'method = "%s"\n' "$RUNTIME_METHOD"
        printf 'dictionary = "%s"\n' "$model"
        ;;
      *)
        printf 'unknown model kind for %s: %s\n' "$dataset" "$MODEL_KIND" >&2
        exit 1
        ;;
    esac
    if [ -n "$FRAGMENTS" ]; then
      printf 'patterns = "%s"\n' "$manifest_fragments"
      printf 'pass_patterns = true\n'
    fi
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

For English line breaking, start with the typesetting model:

```bash
target/release/hyphlab predict --saved-model en-US-typeset --word Japanese
target/release/hyphlab predict --saved-model en-US-typeset \
  --text "Japanese typography needs careful hyphenation."
```

Use `*-typeset` saved models when the result may become a visible line break.
The plain models follow source lexical hyphenation more closely; the typeset
models use curated labels and the same fragment guard at runtime.

List the reusable models and run other locales directly:

```bash
target/release/hyphlab predict --list-saved-models
target/release/hyphlab predict --saved-model en-US --word hyphenation --word typesetting
target/release/hyphlab predict --saved-model de-typeset --word Scheißhaus
target/release/hyphlab predict --saved-model de --text "Silbentrennung fuer lange Woerter"
target/release/hyphlab predict --saved-model en-US-typeset --with-hypher \
  --gold data/gold/moby_en_us_typeset.jsonl.zst \
  --word Japanese \
  --show-breaks
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
- English en-US typesetting: `moby_en_us_typeset.bin`, trained from the
  curated Moby typesetting corpus and guarded by the same fragment filter at
  runtime.
- Czech, German, Spanish, Dutch, Russian, and Turkish: the matching
  `wiktextract_*.bin` or `wiktextract_*_typeset.bin` model.
- Italian: `wiktextract_it.json` is an onset-style model trained from normalized
  Italian Wiktextract / Kaikki entries. `wiktextract_it_typeset.bin` is a
  guarded n-gram model trained from the curated Italian typesetting corpus.

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
      "$BIN" method train \
        --method "$RECIPE" \
        --gold "$GOLD" \
        --locale "$LOCALE" \
        --output "$model"
      ;;
    italian-syllable-model)
      "$BIN" method train \
        --method "$RECIPE" \
        --gold "$GOLD" \
        --locale "$LOCALE" \
        --output "$model"
      ;;
    *)
      printf 'unknown model kind: %s\n' "$MODEL_KIND" >&2
      exit 1
      ;;
  esac

  write_manifest "$dataset" "$manifest" "$manifest_model"

  {
    printf '| `%s` | `%s` | `%s` | %s | `%s` | `%s` | `%s` | `%s` |\n' \
      "$dataset" "$LOCALE" "$GOLD" "$TRAINING_POLICY" "$SLUG" "$RECIPE" "$model" "$manifest"
  } >> "$index"
done

printf 'wrote %s\n' "$index"
