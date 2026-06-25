#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${BIN:-target/release/hyphlab}"
OUT_DIR="${OUT_DIR:-data/challenges/typeset_no_break}"
SEED="${SEED:-data/challenges/typeset_no_break_seed.tsv}"
RAW_DIR="${RAW_DIR:-data/challenges/raw/wikidata}"
FETCH_WIKIDATA="${FETCH_WIKIDATA:-1}"
LIMIT="${LIMIT:-200}"

if [ ! -x "$BIN" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

mkdir -p "$OUT_DIR" "$RAW_DIR" data/curation/proper_names

combined="$OUT_DIR/all.tsv"
cp "$SEED" "$combined"

fetch_wikidata_locale() {
  local locale="$1"
  local lang="$2"
  local raw="$RAW_DIR/${locale}.tsv"
  local query
  query="
SELECT ?label WHERE {
  VALUES ?class { wd:Q5 wd:Q43229 wd:Q4830453 wd:Q515 wd:Q6256 }
  ?item wdt:P31/wdt:P279* ?class ;
        rdfs:label ?label .
  FILTER(LANG(?label) = \"$lang\")
  FILTER(STRLEN(STR(?label)) >= 4 && STRLEN(STR(?label)) <= 32)
  FILTER(!CONTAINS(STR(?label), \" \"))
}
LIMIT $LIMIT
"
  if curl -fsSL -A "hyphlab-guard-challenge/1.0" \
    -H "Accept: text/tab-separated-values, application/sparql-results+xml;q=0.8" \
    --get \
    --data-urlencode "format=tsv" \
    --data-urlencode "query=$query" \
    "https://query.wikidata.org/sparql" \
    -o "$raw"; then
    if head -n 1 "$raw" | grep -q '^<'; then
      sed -n 's/.*<literal[^>]*>\([^<]*\)<\/literal>.*/\1/p' "$raw"
    else
      awk 'BEGIN { FS="\t" } NR > 1 { print $1 }' "$raw"
    fi | awk -v locale="$locale" 'BEGIN { OFS="\t" }
      {
        label=$0
        gsub(/^"|"$/, "", label)
        gsub(/\\"/, "\"", label)
        if (label != "" && label !~ /[-_]/ && label !~ /[<>]/) {
          print label, label, locale, "wikidata:proper_name", "CC0"
        }
      }' >> "$combined"
  else
    printf 'warning: failed to fetch Wikidata labels for %s\n' "$locale" >&2
  fi
}

if [ "$FETCH_WIKIDATA" = "1" ]; then
  fetch_wikidata_locale "en-US" "en"
  fetch_wikidata_locale "cs" "cs"
  fetch_wikidata_locale "de" "de"
  fetch_wikidata_locale "es" "es"
  fetch_wikidata_locale "it" "it"
  fetch_wikidata_locale "nl" "nl"
  fetch_wikidata_locale "ru" "ru"
  fetch_wikidata_locale "tr" "tr"
fi

awk 'BEGIN { FS="\t"; OFS="\t" }
  NR == 1 { print; next }
  !seen[$1 "\t" $3]++ { print }
' "$combined" > "$OUT_DIR/all.dedup.tsv"
mv "$OUT_DIR/all.dedup.tsv" "$combined"

for locale in en-US cs de es it nl ru tr; do
  slug="${locale//-/_}"
  tsv="$OUT_DIR/${slug}.tsv"
  jsonl="$OUT_DIR/${slug}.jsonl.zst"
  awk -v locale="$locale" 'BEGIN { FS="\t"; OFS="\t" }
    NR == 1 { print; next }
    $3 == locale { print }
  ' "$combined" > "$tsv"
  "$BIN" data import-tsv \
    --input "$tsv" \
    --output "$jsonl" \
    --source "typeset-no-break-challenge" \
    --license "manual-curation and Wikidata CC0"
done

for locale in en-US cs de es it nl ru tr; do
  slug="${locale//-/_}"
  names="data/curation/proper_names/${slug}.txt"
  case "$locale" in
    en-US) names="data/curation/proper_names/en_us.txt" ;;
  esac
  awk -v locale="$locale" 'BEGIN { FS="\t" }
    NR > 1 && $3 == locale && $4 ~ /proper/ { print $1 }
  ' "$combined" | sort -u > "$names"
done

printf 'wrote challenge data under %s\n' "$OUT_DIR"
