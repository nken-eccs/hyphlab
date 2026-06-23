#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

download() {
  local url="$1"
  local output="$2"
  mkdir -p "$(dirname "$output")"
  curl --fail --location --retry 3 --retry-delay 2 --continue-at - \
    --output "$output" "$url"
}

LANGS="${KAIKKI_LANGS:-cs de es it nl pt ru th tr}"
BASE_URL="${KAIKKI_BASE_URL:-https://kaikki.org/dictionary/rawdata}"

for lang in $LANGS
do
  download \
    "$BASE_URL/$lang-extract.jsonl.gz" \
    "data/raw/kaikki/$lang/$lang-extract.jsonl.gz"
done

if [ "${INCLUDE_RAW_EN:-0}" = "1" ]; then
  download \
    "https://kaikki.org/dictionary/raw-wiktextract-data.jsonl.gz" \
    "data/raw/kaikki/enwiktionary/raw-wiktextract-data.jsonl.gz"
fi

