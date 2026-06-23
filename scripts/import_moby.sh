#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

: "${MOBY_FILE:?set MOBY_FILE to the local Moby hyphenation file}"

OUTPUT="${OUTPUT:-data/gold/moby_en_us.jsonl.zst}"
LOCALE="${LOCALE:-en-US}"
SOURCE="${SOURCE:-moby}"
LICENSE="${LICENSE:-public-domain}"
SEPARATOR="${SEPARATOR:-0xA5}"

cargo run -p hyph-cli -- data import-moby \
  --input "$MOBY_FILE" \
  --output "$OUTPUT" \
  --locale "$LOCALE" \
  --source "$SOURCE" \
  --license "$LICENSE" \
  --separator "$SEPARATOR"

cargo run -p hyph-cli -- data stats --input "$OUTPUT"
