#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

REPORT_DIR="${REPORT_DIR:-target/hyphlab-reports}"
SPEED_ITERATIONS="${SPEED_ITERATIONS:-1000}"
mkdir -p "$REPORT_DIR/speed" data/gold

cargo run -p hyph-cli -- data import-tsv \
  --input tests/fixtures/toy_en.tsv \
  --output data/gold/toy_en.jsonl \
  --locale en-US \
  --source toy

cargo run -p hyph-cli -- eval \
  --gold data/gold/toy_en.jsonl \
  --method no-hyphen \
  --locale en-US \
  --output "$REPORT_DIR/no_hyphen.json" \
  --errors-output "$REPORT_DIR/no_hyphen_errors.jsonl"

cargo run -p hyph-cli -- eval \
  --gold data/gold/toy_en.jsonl \
  --method hypher \
  --locale en-US \
  --output "$REPORT_DIR/hypher.json" \
  --errors-output "$REPORT_DIR/hypher_errors.jsonl"

cargo run -p hyph-cli --features adapters-hyphenation-embedded -- eval \
  --gold data/gold/toy_en.jsonl \
  --method hyphenation-embedded \
  --locale en-US \
  --output "$REPORT_DIR/hyphenation_embedded.json" \
  --errors-output "$REPORT_DIR/hyphenation_embedded_errors.jsonl"

cargo run -p hyph-cli -- eval \
  --gold data/gold/toy_en.jsonl \
  --method dict \
  --locale en-US \
  --output "$REPORT_DIR/dict.json" \
  --errors-output "$REPORT_DIR/dict_errors.jsonl"

cargo run -p hyph-cli -- eval \
  --gold data/gold/toy_en.jsonl \
  --method identity-oracle \
  --locale en-US \
  --output "$REPORT_DIR/identity_oracle.json" \
  --errors-output "$REPORT_DIR/identity_oracle_errors.jsonl"

cargo run -p hyph-cli -- eval \
  --gold data/gold/toy_en.jsonl \
  --method liang \
  --locale en-US \
  --patterns tests/fixtures/toy_en.patterns \
  --output "$REPORT_DIR/liang.json" \
  --errors-output "$REPORT_DIR/liang_errors.jsonl"

cargo run -p hyph-cli -- eval \
  --gold data/gold/toy_en.jsonl \
  --method hypher-liang-consensus \
  --locale en-US \
  --patterns tests/fixtures/toy_en.patterns \
  --output "$REPORT_DIR/consensus.json" \
  --errors-output "$REPORT_DIR/consensus_errors.jsonl"

cargo run -p hyph-cli --release -- speed \
  --gold data/gold/toy_en.jsonl \
  --method no-hyphen \
  --locale en-US \
  --iterations "$SPEED_ITERATIONS" \
  --output "$REPORT_DIR/speed/no_hyphen.json"

cargo run -p hyph-cli --release -- speed \
  --gold data/gold/toy_en.jsonl \
  --method hypher \
  --locale en-US \
  --iterations "$SPEED_ITERATIONS" \
  --output "$REPORT_DIR/speed/hypher.json"

cargo run -p hyph-cli --release --features adapters-hyphenation-embedded -- speed \
  --gold data/gold/toy_en.jsonl \
  --method hyphenation-embedded \
  --locale en-US \
  --iterations "$SPEED_ITERATIONS" \
  --output "$REPORT_DIR/speed/hyphenation_embedded.json"

cargo run -p hyph-cli --release -- speed \
  --gold data/gold/toy_en.jsonl \
  --method dict \
  --locale en-US \
  --iterations "$SPEED_ITERATIONS" \
  --output "$REPORT_DIR/speed/dict.json"

cargo run -p hyph-cli --release -- speed \
  --gold data/gold/toy_en.jsonl \
  --method identity-oracle \
  --locale en-US \
  --iterations "$SPEED_ITERATIONS" \
  --output "$REPORT_DIR/speed/identity_oracle.json"

cargo run -p hyph-cli --release -- speed \
  --gold data/gold/toy_en.jsonl \
  --method liang \
  --locale en-US \
  --patterns tests/fixtures/toy_en.patterns \
  --iterations "$SPEED_ITERATIONS" \
  --output "$REPORT_DIR/speed/liang.json"

cargo run -p hyph-cli --release -- speed \
  --gold data/gold/toy_en.jsonl \
  --method hypher-liang-consensus \
  --locale en-US \
  --patterns tests/fixtures/toy_en.patterns \
  --iterations "$SPEED_ITERATIONS" \
  --output "$REPORT_DIR/speed/consensus.json"

cargo run -p hyph-cli -- compare \
  --input "$REPORT_DIR/no_hyphen.json" \
  --input "$REPORT_DIR/hypher.json" \
  --input "$REPORT_DIR/hyphenation_embedded.json" \
  --input "$REPORT_DIR/dict.json" \
  --input "$REPORT_DIR/identity_oracle.json" \
  --input "$REPORT_DIR/liang.json" \
  --input "$REPORT_DIR/consensus.json" \
  --speed-input "$REPORT_DIR/speed/no_hyphen.json" \
  --speed-input "$REPORT_DIR/speed/hypher.json" \
  --speed-input "$REPORT_DIR/speed/hyphenation_embedded.json" \
  --speed-input "$REPORT_DIR/speed/dict.json" \
  --speed-input "$REPORT_DIR/speed/identity_oracle.json" \
  --speed-input "$REPORT_DIR/speed/liang.json" \
  --speed-input "$REPORT_DIR/speed/consensus.json" \
  --output "$REPORT_DIR/compare.md"

printf 'wrote %s/compare.md\n' "$REPORT_DIR"
