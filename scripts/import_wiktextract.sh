#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

LANGS="${WIKTEXTRACT_LANGS:-cs de es it nl pt ru th tr}"
SOURCE="${SOURCE:-wiktextract}"
LICENSE="${LICENSE:-CC BY-SA 4.0 / GFDL via Wiktionary}"
mkdir -p data/gold/wiktextract

for lang in $LANGS
do
  input="${WIKTEXTRACT_INPUT:-data/raw/kaikki/$lang/$lang-extract.jsonl.gz}"
  output="${WIKTEXTRACT_OUTPUT:-data/gold/wiktextract/$lang.jsonl.zst}"
  if [ ! -f "$input" ]; then
    printf 'missing %s; run scripts/fetch_kaikki.sh first\n' "$input" >&2
    exit 1
  fi

  case "$input" in
    *.gz)
      gzip -dc "$input" | cargo run -p hyph-cli -- data import-wiktextract \
        --input - \
        --output "$output" \
        --locale "$lang" \
        --filter-lang-code "$lang" \
        --source "$SOURCE:$lang" \
        --license "$LICENSE" \
        --skip-invalid
      ;;
    *)
      cargo run -p hyph-cli -- data import-wiktextract \
        --input "$input" \
        --output "$output" \
        --locale "$lang" \
        --filter-lang-code "$lang" \
        --source "$SOURCE:$lang" \
        --license "$LICENSE" \
        --skip-invalid
      ;;
  esac
done
