#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

INPUT="${INPUT:-data/gold/moby_en_us.jsonl.zst}"
OUTPUT="${OUTPUT:-data/gold/moby_en_us_typeset.jsonl.zst}"
REPORT="${REPORT:-target/hyphlab-reports/curation/moby_en_us_typeset.tsv}"
GUARD_POLICY="${GUARD_POLICY:-data/curation/guard_policies/moby_en_us_typeset.toml}"

cargo run -p hyph-cli -- data curate-typeset \
  --input "$INPUT" \
  --output "$OUTPUT" \
  --report "$REPORT" \
  --guard-policy "$GUARD_POLICY"

cargo run -p hyph-cli -- data stats --input "$OUTPUT"
