#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

download() {
  local url="$1"
  local output="$2"
  mkdir -p "$(dirname "$output")"
  curl --fail --location --retry 3 --retry-delay 2 --continue-at - \
    --output "$output" "$url"
}

extract_archive() {
  local archive="$1"
  local output_dir="$2"
  mkdir -p "$output_dir"
  tar -xzf "$archive" -C "$output_dir" --strip-components=1
}

copy_file() {
  local input="$1"
  local output="$2"
  mkdir -p "$(dirname "$output")"
  cp "$input" "$output"
}

download \
  "https://www.gutenberg.org/files/3204/files/mhyph.txt" \
  "data/raw/moby/mhyph.txt"

download \
  "https://github.com/hyphenation/tex-hyphen/archive/refs/heads/master.tar.gz" \
  "data/raw/archives/tex-hyphen-master.tar.gz"
download \
  "https://github.com/Kozea/hyph-bench/archive/refs/heads/main.tar.gz" \
  "data/raw/archives/hyph-bench-main.tar.gz"
download \
  "https://github.com/LibreOffice/dictionaries/archive/refs/heads/master.tar.gz" \
  "data/raw/archives/libreoffice-dictionaries-master.tar.gz"
download \
  "https://github.com/hunspell/hyphen/archive/refs/heads/master.tar.gz" \
  "data/raw/archives/hunspell-hyphen-master.tar.gz"

extract_archive "data/raw/archives/tex-hyphen-master.tar.gz" "external/tex-hyphen"
extract_archive "data/raw/archives/hyph-bench-main.tar.gz" "external/hyph-bench"
extract_archive "data/raw/archives/libreoffice-dictionaries-master.tar.gz" "external/libreoffice-dictionaries"
extract_archive "data/raw/archives/hunspell-hyphen-master.tar.gz" "external/hunspell-hyphen"

TEX_PATTERNS="external/tex-hyphen/hyph-utf8/tex/generic/hyph-utf8/patterns/tex"
for file in \
  hyph-en-us.tex \
  hyph-en-gb.tex \
  hyph-de-1996.tex \
  hyph-cs.tex \
  hyph-es.tex \
  hyph-it.tex \
  hyph-nl.tex \
  hyph-pt.tex \
  hyph-ru.tex \
  hyph-th.tex \
  hyph-tr.tex
do
  copy_file "$TEX_PATTERNS/$file" "data/patterns/tex-hyphen/tex/$file"
done

copy_file "external/libreoffice-dictionaries/en/hyph_en_US.dic" "data/patterns/libreoffice/en/hyph_en_US.dic"
copy_file "external/libreoffice-dictionaries/en/hyph_en_GB.dic" "data/patterns/libreoffice/en/hyph_en_GB.dic"
copy_file "external/libreoffice-dictionaries/en/README_hyph_en_US.txt" "data/patterns/libreoffice/en/README_hyph_en_US.txt"
copy_file "external/libreoffice-dictionaries/en/README_hyph_en_GB.txt" "data/patterns/libreoffice/en/README_hyph_en_GB.txt"
copy_file "external/libreoffice-dictionaries/de/hyph_de_DE.dic" "data/patterns/libreoffice/de/hyph_de_DE.dic"
copy_file "external/libreoffice-dictionaries/de/hyph_de_AT.dic" "data/patterns/libreoffice/de/hyph_de_AT.dic"
copy_file "external/libreoffice-dictionaries/de/hyph_de_CH.dic" "data/patterns/libreoffice/de/hyph_de_CH.dic"
copy_file "external/libreoffice-dictionaries/cs_CZ/hyph_cs_CZ.dic" "data/patterns/libreoffice/cs/hyph_cs_CZ.dic"
copy_file "external/libreoffice-dictionaries/es/hyph_es.dic" "data/patterns/libreoffice/es/hyph_es.dic"
copy_file "external/libreoffice-dictionaries/it_IT/hyph_it_IT.dic" "data/patterns/libreoffice/it/hyph_it_IT.dic"
copy_file "external/libreoffice-dictionaries/nl_NL/hyph_nl_NL.dic" "data/patterns/libreoffice/nl/hyph_nl_NL.dic"
copy_file "external/libreoffice-dictionaries/pt_BR/hyph_pt_BR.dic" "data/patterns/libreoffice/pt/hyph_pt_BR.dic"
copy_file "external/libreoffice-dictionaries/pt_PT/hyph_pt_PT.dic" "data/patterns/libreoffice/pt/hyph_pt_PT.dic"
copy_file "external/libreoffice-dictionaries/ru_RU/hyph_ru_RU.dic" "data/patterns/libreoffice/ru/hyph_ru_RU.dic"
copy_file "external/libreoffice-dictionaries/th_TH/hyph_th_TH.dic" "data/patterns/libreoffice/th/hyph_th_TH.dic"

MOBY_FILE="$ROOT/data/raw/moby/mhyph.txt" \
OUTPUT="$ROOT/data/gold/moby_en_us.jsonl.zst" \
SOURCE="moby_hyphenator_ii" \
LICENSE="public-domain-in-usa" \
bash "$ROOT/scripts/import_moby.sh"

