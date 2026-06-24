# Data

The data directory contains small fixtures, source manifests, and curated
typesetting corpora. Large raw downloads, copied pattern corpora, and the
remaining normalized gold files can be recreated from the scripts below.

## Recreate Downloads

Core research inputs:

```bash
bash scripts/fetch_core_data.sh
```

This downloads Moby Hyphenator II, TeX hyphen patterns, hyph-bench,
LibreOffice dictionaries, and Hunspell hyphen sources. It also copies the
representative TeX/LibreOffice pattern files into `data/patterns` and imports
Moby into `data/gold/moby_en_us.jsonl.zst`.

Current evaluation usage:

- `data/gold/moby_en_us.jsonl.zst` and selected
  `data/gold/wiktextract/*.jsonl.zst` files are the gold data for the
  multilingual 5-fold report. `data/gold/moby_en_us_typeset.jsonl.zst` is a
  curated derivative for English typesetting experiments.
- `data/gold/wiktextract/*_typeset.jsonl.zst` files are curated derivatives
  for multilingual typesetting experiments.
- `data/patterns/tex-hyphen/...` drives Liang / TeX pattern baselines.
- `data/gold/hyph_bench/*.jsonl.zst` is used by the optional full-gold
  baseline matrix and the additional `hyph-bench` 5-fold report.
- Copied LibreOffice dictionaries and extracted Hunspell hyphen sources are
  kept as reference pattern resources. LibreOffice hyphen dictionaries can be
  enabled as an additional Liang/libhyphen pattern baseline. They are not used
  as gold data in the multilingual 5-fold report.

## Choosing Data

Use `data/gold/moby_en_us.jsonl.zst` for en-US English experiments that need
the original Moby syllable labels and the `moby_en_us.bin` reusable model.

Use `data/gold/moby_en_us_typeset.jsonl.zst` for en-US typesetting experiments
where unsafe line fragments are filtered out by the current fragment policy.
Regenerate it with:

```bash
bash scripts/curate_moby_typeset.sh
```

Use `data/gold/wiktextract/*.jsonl.zst` for multilingual learned-model
experiments and the `wiktextract_*` reusable models. Russian experiments should
usually use
`data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst`.

Use the matching `data/gold/wiktextract/*_typeset.jsonl.zst` files when the
target is reader-facing line breaking. Regenerate them after importing
Wiktextract / Kaikki with:

```bash
bash scripts/curate_wiktextract_typeset.sh
```

Use `data/gold/hyph_bench/*.jsonl.zst` for additional external-corpus checks.
The hyph-bench report covers Czech/German datasets that can be compared with
Hypher and the selected Guarded N-gram recipes.

Use `data/patterns/*` and `external/*` when you need pattern resources for
Liang/libhyphen-style baselines or source comparison. These files are not the
training source for the Guarded N-gram runtime models and are not gold labels
for the main multilingual 5-fold report.

The models under `models/guarded_ngram/v1/` are full-corpus runtime models. For
unbiased trainable-method evaluation, create train/test splits or use
`scripts/run_multilingual_5fold_evaluation.sh`.

The Moby importer collapses duplicate words into one record. Conflicting
hyphenations for the same word are preserved as `ambiguous=true` records with
all distinct break sets in `variants`.

The Moby typesetting curation scans every record and removes candidate
boundaries that violate the configured line-fragment policy. It also drops
records containing the Unicode replacement character. The curation report is
written under `target/hyphlab-reports/curation/`.

The Wiktextract typesetting curation uses the same boundary policy with
language-specific fragment files under `data/curation/typeset_fragments/`.
Summary notes are in
`docs/reports/wiktextract_typeset_5fold_v1/curation.md`.

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
- `data/patterns`: copied TeX and LibreOffice pattern files. Current Liang
  runs use the TeX pattern files.
- `data/gold`: normalized JSONL and JSONL.zst gold corpora.
- `data/manifests`: source inventory and license notes.

## Restricted Data

CELEX/WebCelex is license-restricted and is not downloaded by these scripts.
Place local exports under `data/raw/celex` before adding an importer.
