#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

GOLD="${GOLD:-data/gold/moby_en_us.jsonl.zst}"
REPORT_DIR="${REPORT_DIR:-target/hyphlab-reports/moby}"
ITERATIONS="${ITERATIONS:-3}"

mkdir -p "$REPORT_DIR/speed"

cargo run -p hyph-cli --release -- eval \
  --gold "$GOLD" \
  --method no-hyphen \
  --locale en-US \
  --output "$REPORT_DIR/no_hyphen.json" \
  --errors-output "$REPORT_DIR/no_hyphen_errors.jsonl"

cargo run -p hyph-cli --release -- eval \
  --gold "$GOLD" \
  --method hypher \
  --locale en-US \
  --output "$REPORT_DIR/hypher.json" \
  --errors-output "$REPORT_DIR/hypher_errors.jsonl"

cargo run -p hyph-cli --release --features adapters-hyphenation-embedded -- eval \
  --gold "$GOLD" \
  --method hyphenation-embedded \
  --locale en-US \
  --output "$REPORT_DIR/hyphenation_embedded.json" \
  --errors-output "$REPORT_DIR/hyphenation_embedded_errors.jsonl"

cargo run -p hyph-cli --release -- eval \
  --gold "$GOLD" \
  --method dict \
  --locale en-US \
  --output "$REPORT_DIR/dict.json" \
  --errors-output "$REPORT_DIR/dict_errors.jsonl"

cargo run -p hyph-cli --release -- eval \
  --gold "$GOLD" \
  --method identity-oracle \
  --locale en-US \
  --output "$REPORT_DIR/identity_oracle.json" \
  --errors-output "$REPORT_DIR/identity_oracle_errors.jsonl"

cargo run -p hyph-cli --release -- eval \
  --gold "$GOLD" \
  --method liang \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --output "$REPORT_DIR/liang_tex.json" \
  --errors-output "$REPORT_DIR/liang_tex_errors.jsonl"

cargo run -p hyph-cli --release -- eval \
  --gold "$GOLD" \
  --method hypher-liang-consensus \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --output "$REPORT_DIR/hypher_liang_consensus.json" \
  --errors-output "$REPORT_DIR/hypher_liang_consensus_errors.jsonl"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD" \
  --method no-hyphen \
  --locale en-US \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/speed/no_hyphen.json"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD" \
  --method hypher \
  --locale en-US \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/speed/hypher.json"

cargo run -p hyph-cli --release --features adapters-hyphenation-embedded -- speed \
  --gold "$GOLD" \
  --method hyphenation-embedded \
  --locale en-US \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/speed/hyphenation_embedded.json"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD" \
  --method dict \
  --locale en-US \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/speed/dict.json"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD" \
  --method identity-oracle \
  --locale en-US \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/speed/identity_oracle.json"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD" \
  --method liang \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/speed/liang_tex.json"

cargo run -p hyph-cli --release -- speed \
  --gold "$GOLD" \
  --method hypher-liang-consensus \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --iterations "$ITERATIONS" \
  --output "$REPORT_DIR/speed/hypher_liang_consensus.json"

cargo run -p hyph-cli --release -- compare \
  --input "$REPORT_DIR/no_hyphen.json" \
  --input "$REPORT_DIR/hypher.json" \
  --input "$REPORT_DIR/hyphenation_embedded.json" \
  --input "$REPORT_DIR/dict.json" \
  --input "$REPORT_DIR/identity_oracle.json" \
  --input "$REPORT_DIR/liang_tex.json" \
  --input "$REPORT_DIR/hypher_liang_consensus.json" \
  --speed-input "$REPORT_DIR/speed/no_hyphen.json" \
  --speed-input "$REPORT_DIR/speed/hypher.json" \
  --speed-input "$REPORT_DIR/speed/hyphenation_embedded.json" \
  --speed-input "$REPORT_DIR/speed/dict.json" \
  --speed-input "$REPORT_DIR/speed/identity_oracle.json" \
  --speed-input "$REPORT_DIR/speed/liang_tex.json" \
  --speed-input "$REPORT_DIR/speed/hypher_liang_consensus.json" \
  --output "$REPORT_DIR/compare.md"
