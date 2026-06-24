#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [ "$#" -lt 1 ]; then
  printf 'usage: %s <slug-or-method> [extra hyphlab matrix args]\n' "$0" >&2
  exit 2
fi

METHOD="$1"
shift

cargo run -p hyph-cli --features adapters-hyphenation-embedded -- matrix \
  --manifest "${METHODS_MANIFEST:-manifests/baselines.toml}" \
  --gold "${GOLD:-data/gold/toy_en.jsonl}" \
  --locale "${LOCALE:-en-US}" \
  --patterns "${PATTERNS:-tests/fixtures/toy_en.patterns}" \
  --output-dir "${OUTPUT_DIR:-target/hyphlab-reports/dev-smoke/$METHOD}" \
  --iterations "${ITERATIONS:-1}" \
  --init-iterations "${INIT_ITERATIONS:-1}" \
  --only "$METHOD" \
  "$@"
