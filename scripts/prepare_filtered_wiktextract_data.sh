#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${BIN:-target/release/hyphlab}"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

"$BIN" data filter-script \
  --input data/gold/wiktextract/ru.jsonl.zst \
  --output data/gold/wiktextract/ru_cyrl.jsonl.zst \
  --script russian-cyrillic

"$BIN" data dedup-variants \
  --input data/gold/wiktextract/ru.jsonl.zst \
  --output data/gold/wiktextract/ru_dedup.jsonl.zst

"$BIN" data dedup-variants \
  --input data/gold/wiktextract/ru_cyrl.jsonl.zst \
  --output data/gold/wiktextract/ru_cyrl_dedup.jsonl.zst

"$BIN" data filter-quality \
  --input data/gold/wiktextract/ru_cyrl_dedup.jsonl.zst \
  --output data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst \
  --drop-long-no-break \
  --min-graphemes 5 \
  --min-vowels 2
