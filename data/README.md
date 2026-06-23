# Data

The checked-in repository keeps only small fixtures and manifests. Large raw
downloads, copied pattern corpora, and generated compressed gold files are
ignored by Git and can be recreated.

## Recreate Downloads

Core research inputs:

```bash
bash scripts/fetch_core_data.sh
```

This downloads Moby Hyphenator II, TeX hyphen patterns, hyph-bench,
LibreOffice dictionaries, and Hunspell hyphen sources. It also copies the
representative TeX/LibreOffice pattern files into `data/patterns` and imports
Moby into `data/gold/moby_en_us.jsonl.zst`.

The Moby importer collapses duplicate words into one record. Conflicting
hyphenations for the same word are preserved as `ambiguous=true` records with
all distinct break sets in `variants`.

Normalize hyph-bench WLHAMB files:

```bash
bash scripts/import_hyph_bench.sh
```

This writes `data/gold/hyph_bench/*.jsonl.zst`. The Thai Orchid source contains
some separators inside Unicode grapheme clusters; those lines are skipped and
reported by the importer.

Wiktionary extracts from Kaikki:

```bash
bash scripts/fetch_kaikki.sh
bash scripts/import_wiktextract.sh
```

The default languages are `cs de es it nl pt ru th tr`. Override with
`KAIKKI_LANGS="de ru"` when downloading a subset, and
`WIKTEXTRACT_LANGS="de ru"` when importing a subset.

The full raw English Wiktextract file is much larger than the language extracts:

```bash
INCLUDE_RAW_EN=1 KAIKKI_LANGS="" bash scripts/fetch_kaikki.sh
gzip -dc data/raw/kaikki/enwiktionary/raw-wiktextract-data.jsonl.gz | \
  cargo run -p hyph-cli -- data import-wiktextract \
    --input - \
    --output data/gold/wiktextract/en.jsonl.zst \
    --locale en \
    --source wiktextract:en \
    --skip-invalid
```

## Local Paths

- `data/raw`: downloaded source archives and raw corpora.
- `external`: expanded upstream repositories.
- `data/patterns`: copied TeX and LibreOffice pattern files used by Liang runs.
- `data/gold`: normalized JSONL and JSONL.zst gold corpora.
- `data/manifests`: source inventory and license notes.

## Restricted Data

CELEX/WebCelex is license-restricted and is not downloaded by these scripts.
Place local exports under `data/raw/celex` before adding an importer.
