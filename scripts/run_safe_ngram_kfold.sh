#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

GOLD_ALL="${GOLD_ALL:-data/gold/moby_en_us.jsonl.zst}"
FOLD_DATA_DIR="${FOLD_DATA_DIR:-target/hyphlab-folds/moby_en_us_5fold}"
REPORT_DIR="${REPORT_DIR:-target/hyphlab-reports/research/moby_en_us_current_candidates_5fold}"
MODEL_DIR="${MODEL_DIR:-target/hyphlab-models/kfold/moby_en_us_current_candidates}"
MANIFEST_DIR="${MANIFEST_DIR:-target/hyphlab-manifests/kfold/moby_en_us_current_candidates}"
LOCALE="${LOCALE:-en-US}"
FOLDS="${FOLDS:-5}"
DEV_RATIO="${DEV_RATIO:-0}"
ITERATIONS="${ITERATIONS:-3}"
INIT_ITERATIONS="${INIT_ITERATIONS:-3}"
INIT_WARMUP="${INIT_WARMUP:-1}"
BIN="${BIN:-target/release/hyphlab}"
METHODS="${METHODS:-safe-ngram-multi-s1-p65-veto-multi-s1-n0,safe-ngram-multi-s1-p50-veto-multi-s1-n0,safe-ngram-mixcv-multi-s1-p85-veto-multi-s1-n0,safe-ngram-mixson-multi-s1-p90-veto-multi-s1-n0,safe-ngram-multi-s1-w40-veto-multi-s1-n0,safe-ngram-3x3-s1-n2-veto-4x4-s1-n0}"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

if [ "${SKIP_SPLIT:-0}" != "1" ]; then
  "$BIN" data kfold \
    --input "$GOLD_ALL" \
    --output-dir "$FOLD_DATA_DIR" \
    --folds "$FOLDS" \
    --dev-ratio "$DEV_RATIO" \
    --seed moby_en_us_5fold_v1
fi

IFS=',' read -r -a METHOD_LIST <<< "$METHODS"
mkdir -p "$REPORT_DIR" "$MODEL_DIR" "$MANIFEST_DIR"

for ((fold = 0; fold < FOLDS; fold++)); do
  fold_data="$FOLD_DATA_DIR/fold-$fold"
  train="$fold_data/train.jsonl.zst"
  test="$fold_data/test.jsonl.zst"
  manifest="$MANIFEST_DIR/fold-$fold.toml"
  fold_model_dir="$MODEL_DIR/fold-$fold"
  fold_report="$REPORT_DIR/fold-$fold"
  mkdir -p "$fold_model_dir" "$fold_report"

  {
    printf '[[methods]]\n'
    printf 'slug = "hypher"\n'
    printf 'method = "hypher"\n'
    printf 'supports = ["en"]\n\n'
  } > "$manifest"

  for method in "${METHOD_LIST[@]}"; do
    slug="$(printf '%s' "$method" | tr '-' '_' | tr -c '[:alnum:]_' '_')"
    model="$(pwd)/$fold_model_dir/$slug.bin"
    "$BIN" method train \
      --method "$method" \
      --gold "$train" \
      --locale "$LOCALE" \
      --output "$model"
    {
      printf '[[methods]]\n'
      printf 'slug = "%s"\n' "$slug"
      printf 'method = "safe-ngram-model"\n'
      printf 'dictionary = "%s"\n' "$model"
      printf 'supports = ["en"]\n\n'
    } >> "$manifest"
  done

  "$BIN" matrix \
    --manifest "$manifest" \
    --gold "$test" \
    --locale "$LOCALE" \
    --output-dir "$fold_report" \
    --iterations "$ITERATIONS" \
    --init-iterations "$INIT_ITERATIONS" \
    --init-warmup "$INIT_WARMUP"
done

"$BIN" fold-summary \
  --input-dir "$REPORT_DIR" \
  --output "$REPORT_DIR/summary.md"

printf 'wrote %s/summary.md\n' "$REPORT_DIR"
