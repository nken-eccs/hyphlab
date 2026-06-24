#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

INPUT="${INPUT:-data/gold/moby_en_us.jsonl.zst}"
OUTPUT="${OUTPUT:-data/gold/moby_en_us_typeset.jsonl.zst}"
REPORT="${REPORT:-target/hyphlab-reports/curation/moby_en_us_typeset.tsv}"
FRAGMENTS="${FRAGMENTS:-data/curation/typeset_fragments/moby_en_us.txt}"

cargo run -p hyph-cli -- data curate-typeset \
  --input "$INPUT" \
  --output "$OUTPUT" \
  --report "$REPORT" \
  --sensitive-fragments "$FRAGMENTS"

cargo run -p hyph-cli -- data stats --input "$OUTPUT"
