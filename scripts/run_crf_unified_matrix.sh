#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

MANIFEST="${MANIFEST:-experiments/manifests/moby_en_us_crf_sgd_tuned.toml}"
GOLD="${GOLD:-data/splits/moby_en_us/test.jsonl.zst}"
LOCALE="${LOCALE:-en-US}"
PATTERNS="${PATTERNS:-data/patterns/tex-hyphen/tex/hyph-en-us.tex}"
REPORT_DIR="${REPORT_DIR:-target/hyphlab-reports/unified/moby_en_us_test_crf_sgd_tuned}"
ITERATIONS="${ITERATIONS:-1}"
INIT_ITERATIONS="${INIT_ITERATIONS:-1}"
INIT_WARMUP="${INIT_WARMUP:-0}"
BIN="${BIN:-target/release/hyphlab}"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

printf 'manifest=%s\n' "$MANIFEST"
printf 'gold=%s\n' "$GOLD"
printf 'locale=%s\n' "$LOCALE"
printf 'patterns=%s\n' "$PATTERNS"
printf 'report_dir=%s\n' "$REPORT_DIR"

"$BIN" matrix \
  --manifest "$MANIFEST" \
  --gold "$GOLD" \
  --locale "$LOCALE" \
  --patterns "$PATTERNS" \
  --output-dir "$REPORT_DIR" \
  --iterations "$ITERATIONS" \
  --init-iterations "$INIT_ITERATIONS" \
  --init-warmup "$INIT_WARMUP"

printf 'wrote %s/compare.md\n' "$REPORT_DIR"
