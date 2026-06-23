#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

: "${GOLD_FILE:?set GOLD_FILE to a hyphlab JSONL or JSONL.zst gold file}"
: "${PATTERNS_FILE:?set PATTERNS_FILE to a TeX or Hunspell hyphenation pattern file}"

REPORT_DIR="${REPORT_DIR:-target/hyphlab-reports}"
LOCALE="${LOCALE:-en-US}"
mkdir -p "$REPORT_DIR"

cargo run -p hyph-cli -- eval \
  --gold "$GOLD_FILE" \
  --method liang \
  --locale "$LOCALE" \
  --patterns "$PATTERNS_FILE" \
  --output "$REPORT_DIR/liang_patterns.json" \
  --errors-output "$REPORT_DIR/liang_patterns_errors.jsonl"
