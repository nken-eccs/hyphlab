#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

import_wlhamb() {
  local input="$1"
  local output="$2"
  local locale="$3"
  local source="$4"
  local extra_args="${5:-}"
  cargo run -p hyph-cli -- data import-wlhamb \
    --input "$input" \
    --output "$output" \
    --locale "$locale" \
    --source "$source" \
    --license "upstream-hyph-bench" \
    $extra_args
  cargo run -p hyph-cli -- data stats --input "$output"
}

import_wlhamb \
  "external/hyph-bench/data/cs/cshyphen_cstenten/cs_cstenten.wlhamb" \
  "data/gold/hyph_bench/cs_cstenten.jsonl.zst" \
  "cs" \
  "hyph-bench:cs_cstenten"

import_wlhamb \
  "external/hyph-bench/data/cs/cshyphen_ujc/cs_ujc.wlhamb" \
  "data/gold/hyph_bench/cs_ujc.jsonl.zst" \
  "cs" \
  "hyph-bench:cs_ujc"

import_wlhamb \
  "external/hyph-bench/data/cssk/cshyphen/cssk_cshyphen.wlhamb" \
  "data/gold/hyph_bench/cssk_cshyphen.jsonl.zst" \
  "cs" \
  "hyph-bench:cssk_cshyphen"

import_wlhamb \
  "external/hyph-bench/data/de/wortliste/de_wortliste.wlhamb" \
  "data/gold/hyph_bench/de_wortliste.jsonl.zst" \
  "de-DE" \
  "hyph-bench:de_wortliste"

import_wlhamb \
  "external/hyph-bench/data/is/hyphenation-is/is_hyphis.wlhamb" \
  "data/gold/hyph_bench/is_hyphis.jsonl.zst" \
  "is-IS" \
  "hyph-bench:is_hyphis"

import_wlhamb \
  "external/hyph-bench/data/th/orchid/th_orchid.wlhamb" \
  "data/gold/hyph_bench/th_orchid.jsonl.zst" \
  "th-TH" \
  "hyph-bench:th_orchid" \
  "--skip-invalid"

import_wlhamb \
  "external/hyph-bench/data/uk/cshyphen/uk_wiktionary.wlhamb" \
  "data/gold/hyph_bench/uk_wiktionary.jsonl.zst" \
  "uk-UA" \
  "hyph-bench:uk_wiktionary"
