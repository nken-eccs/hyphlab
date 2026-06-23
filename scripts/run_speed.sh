#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

GOLD_FILE="${GOLD_FILE:-data/gold/toy_en.jsonl}"
REPORT_DIR="${REPORT_DIR:-target/hyphlab-reports/speed}"
LOCALE="${LOCALE:-en-US}"
ITERATIONS="${ITERATIONS:-1000}"
PATTERNS_FILE="${PATTERNS_FILE:-tests/fixtures/toy_en.patterns}"

mkdir -p "$REPORT_DIR"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD_FILE" \
  --method hypher \
  --locale "$LOCALE" \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/hypher_speed.json"

cargo run -p hyph-cli --release --features adapters-hyphenation-embedded -- speed \
  --gold "$GOLD_FILE" \
  --method hyphenation-embedded \
  --locale "$LOCALE" \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/hyphenation_embedded_speed.json"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD_FILE" \
  --method dict \
  --locale "$LOCALE" \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/dict_speed.json"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD_FILE" \
  --method liang \
  --locale "$LOCALE" \
  --patterns "$PATTERNS_FILE" \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/liang_speed.json"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD_FILE" \
  --method hypher-liang-consensus \
  --locale "$LOCALE" \
  --patterns "$PATTERNS_FILE" \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/consensus_speed.json"
