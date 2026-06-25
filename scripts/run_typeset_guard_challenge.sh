#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${BIN:-target/release/hyphlab}"
REPORT_ROOT="${REPORT_ROOT:-target/hyphlab-reports/typeset_guard_challenge_v1}"
PUBLIC_REPORT_ROOT="${PUBLIC_REPORT_ROOT:-docs/reports/typeset_guard_challenge_v1}"
ITERATIONS="${ITERATIONS:-50}"
INIT_ITERATIONS="${INIT_ITERATIONS:-10}"
INIT_WARMUP="${INIT_WARMUP:-2}"
DATASETS="${DATASETS:-moby_en_us_typeset wiktextract_cs_typeset wiktextract_de_typeset wiktextract_es_typeset wiktextract_it_typeset wiktextract_nl_typeset wiktextract_ru_cyrl_trusted_dedup_typeset wiktextract_tr_typeset}"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

if [ ! -s data/challenges/typeset_no_break/en_US.jsonl.zst ]; then
  FETCH_WIKIDATA="${FETCH_WIKIDATA:-0}" scripts/fetch_guard_challenge_data.sh
fi

mkdir -p "$REPORT_ROOT" "$PUBLIC_REPORT_ROOT"

dataset_config() {
  local dataset="$1"
  LOCALE=""
  GOLD=""
  PATTERNS=""
  MANIFEST="manifests/guarded_ngram/v1/${dataset}.toml"
  case "$dataset" in
    moby_en_us_typeset)
      LOCALE="en-US"
      GOLD="data/challenges/typeset_no_break/en_US.jsonl.zst"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-en-us.tex"
      ;;
    wiktextract_cs_typeset)
      LOCALE="cs"
      GOLD="data/challenges/typeset_no_break/cs.jsonl.zst"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      ;;
    wiktextract_de_typeset)
      LOCALE="de"
      GOLD="data/challenges/typeset_no_break/de.jsonl.zst"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      ;;
    wiktextract_es_typeset)
      LOCALE="es"
      GOLD="data/challenges/typeset_no_break/es.jsonl.zst"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-es.tex"
      ;;
    wiktextract_it_typeset)
      LOCALE="it"
      GOLD="data/challenges/typeset_no_break/it.jsonl.zst"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-it.tex"
      ;;
    wiktextract_nl_typeset)
      LOCALE="nl"
      GOLD="data/challenges/typeset_no_break/nl.jsonl.zst"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-nl.tex"
      ;;
    wiktextract_ru_cyrl_trusted_dedup_typeset)
      LOCALE="ru"
      GOLD="data/challenges/typeset_no_break/ru.jsonl.zst"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      ;;
    wiktextract_tr_typeset)
      LOCALE="tr"
      GOLD="data/challenges/typeset_no_break/tr.jsonl.zst"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-tr.tex"
      ;;
    *)
      printf 'unknown guard challenge dataset: %s\n' "$dataset" >&2
      exit 1
      ;;
  esac
}

for dataset in $DATASETS; do
  dataset_config "$dataset"
  if [ ! -s "$GOLD" ]; then
    printf 'skip %s: missing challenge gold %s\n' "$dataset" "$GOLD" >&2
    continue
  fi
  if [ ! -s "$MANIFEST" ]; then
    printf 'skip %s: missing manifest %s\n' "$dataset" "$MANIFEST" >&2
    continue
  fi
  matrix_args=(
    matrix
    --manifest "$MANIFEST"
    --gold "$GOLD"
    --locale "$LOCALE"
    --output-dir "$REPORT_ROOT/$dataset"
    --iterations "$ITERATIONS"
    --init-iterations "$INIT_ITERATIONS"
    --init-warmup "$INIT_WARMUP"
  )
  if [ -n "$PATTERNS" ]; then
    matrix_args+=(--patterns "$PATTERNS")
  fi
  "$BIN" "${matrix_args[@]}"
done

"$BIN" fold-summary \
  --input-dir "$REPORT_ROOT" \
  --output "$PUBLIC_REPORT_ROOT/summary.md" \
  --title "Typeset Guard Challenge Summary" \
  --unit-label "datasets" \
  --pair-label "dataset"

summary="$PUBLIC_REPORT_ROOT/summary.md"
tmp="$summary.tmp"
{
  sed -n '1p' "$summary"
  printf '\n'
  printf 'This challenge contains only no-break gold labels for proper names, MixedCase tokens, and ALLCAPS tokens. Use `exact`, `no_break_accuracy`, `serious_error`, and `fp/100k` as the meaningful metrics here; precision, recall, and F1 have no positive gold boundaries.\n'
  sed -n '2,$p' "$summary"
} > "$tmp"
mv "$tmp" "$summary"

for compare in "$REPORT_ROOT"/*/compare.md; do
  [ -e "$compare" ] || continue
  dataset="$(basename "$(dirname "$compare")")"
  cp "$compare" "$PUBLIC_REPORT_ROOT/${dataset}.md"
done
printf 'wrote %s/summary.md\n' "$PUBLIC_REPORT_ROOT"
