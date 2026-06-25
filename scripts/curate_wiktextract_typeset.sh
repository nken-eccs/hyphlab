#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${BIN:-target/release/hyphlab}"
REPORT_ROOT="${REPORT_ROOT:-target/hyphlab-reports/curation/wiktextract_typeset}"
DATASETS="${DATASETS:-cs de es it nl ru_cyrl_trusted_dedup tr}"

if [ ! -x "$BIN" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

dataset_config() {
  local dataset="$1"
  INPUT=""
  OUTPUT=""
  GUARD_POLICY=""
  case "$dataset" in
    cs|de|es|it|nl|tr)
      INPUT="data/gold/wiktextract/$dataset.jsonl.zst"
      OUTPUT="data/gold/wiktextract/${dataset}_typeset.jsonl.zst"
      GUARD_POLICY="data/curation/guard_policies/wiktextract_${dataset}_typeset.toml"
      ;;
    ru_cyrl_trusted_dedup)
      INPUT="data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst"
      OUTPUT="data/gold/wiktextract/ru_cyrl_trusted_dedup_typeset.jsonl.zst"
      GUARD_POLICY="data/curation/guard_policies/wiktextract_ru_cyrl_trusted_dedup_typeset.toml"
      ;;
    *)
      printf 'unknown Wiktextract typeset dataset: %s\n' "$dataset" >&2
      exit 1
      ;;
  esac
}

mkdir -p "$REPORT_ROOT"

for dataset in $DATASETS; do
  dataset_config "$dataset"
  if [ ! -s "$INPUT" ]; then
    printf 'skip %s: missing or empty input %s\n' "$dataset" "$INPUT"
    continue
  fi
  "$BIN" data curate-typeset \
    --input "$INPUT" \
    --output "$OUTPUT" \
    --report "$REPORT_ROOT/${dataset}_typeset.tsv" \
    --guard-policy "$GUARD_POLICY" \
    --source-suffix typeset \
    --note-tag typeset_curated
  "$BIN" data stats --input "$OUTPUT"
done
