#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

REPORT="${REPORT:-target/hyphlab-reports/binary-size.md}"
mkdir -p "$(dirname "$REPORT")"

cargo build -p hyph-cli --release
default_size="$(wc -c < target/release/hyphlab | tr -d ' ')"

cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
embedded_size="$(wc -c < target/release/hyphlab | tr -d ' ')"

{
  printf '| build | bytes | mib |\n'
  printf '| --- | ---: | ---: |\n'
  awk -v size="$default_size" 'BEGIN { printf "| default | %d | %.3f |\n", size, size / 1048576 }'
  awk -v size="$embedded_size" 'BEGIN { printf "| adapters-hyphenation-embedded | %d | %.3f |\n", size, size / 1048576 }'
} > "$REPORT"

printf 'wrote %s\n' "$REPORT"

