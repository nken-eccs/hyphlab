#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

TRAIN="${TRAIN:-data/splits/moby_en_us/train.jsonl.zst}"
GOLD="${GOLD:-data/splits/moby_en_us/test.jsonl.zst}"
LOCALE="${LOCALE:-en-US}"
PATTERNS="${PATTERNS:-data/patterns/tex-hyphen/tex/hyph-en-us.tex}"
METHOD="${METHOD:-safe-ngram-multi-s1-p65-veto-multi-s1-n0}"
MODEL="${MODEL:-target/hyphlab-models/safe-ngram-multi-s1-p65-veto-multi-s1-n0.bin}"
MANIFEST="${MANIFEST:-experiments/manifests/moby_en_us_safe_ngram_compiled.toml}"
REPORT_DIR="${REPORT_DIR:-target/hyphlab-reports/research/moby_en_us_safe_ngram_compiled}"
ITERATIONS="${ITERATIONS:-5}"
INIT_ITERATIONS="${INIT_ITERATIONS:-5}"
INIT_WARMUP="${INIT_WARMUP:-1}"
BIN="${BIN:-target/release/hyphlab}"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

"$BIN" compile-safe-ngram \
  --gold "$TRAIN" \
  --locale "$LOCALE" \
  --method "$METHOD" \
  --output "$MODEL"

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
