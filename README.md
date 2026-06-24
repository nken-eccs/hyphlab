# hyphlab

Toolkit for multilingual hyphenation research and runtime models.

`hyphlab` has three main jobs:

1. Normalize hyphenation corpora into a shared JSONL format.
2. Train or load hyphenation methods behind one CLI.
3. Compare accuracy, serious errors, startup cost, and steady-state speed with
   repeatable reports.

## Quick Start

Run the smoke test:

```bash
cd hyphenation/hyphlab
bash scripts/run_toy_experiment.sh
cat target/hyphlab-reports/compare.md
```

Build the CLI:

```bash
cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
```

Use the English typesetting model:

```bash
target/release/hyphlab predict --saved-model en-US-typeset --word Japanese
target/release/hyphlab predict --saved-model en-US-typeset \
  --text "Japanese typography needs careful hyphenation."
```

`en-US-typeset` is the recommended English model for line-breaking use. It is
trained from the curated Moby typesetting corpus and applies the same fragment
guard at runtime, so sensitive fragments filtered during curation are also
blocked during prediction.

Use `en-US` when you want labels close to the source syllable dictionary. Use
`en-US-typeset` when the break may be shown to readers: it keeps useful
hyphenation points while also considering semantic fragments and typographic
appropriateness.

For Czech, German, Spanish, Italian, Dutch, Russian, and Turkish, use the
matching `*-typeset` model for reader-facing line breaks:

```bash
target/release/hyphlab predict --saved-model de-typeset --word Scheißhaus
target/release/hyphlab predict --saved-model es-typeset --word extraordinario
target/release/hyphlab predict --saved-model tr-typeset --word cumhuriyet
```

List every reusable model, or use a non-typesetting model:

```bash
target/release/hyphlab predict --list-saved-models
target/release/hyphlab predict --saved-model en-US --word hyphenation --word typesetting
target/release/hyphlab predict --saved-model de --text "Silbentrennung fuer lange Woerter"
target/release/hyphlab predict --saved-model it --word informazione --word straordinario
```

Compare with Hypher and known gold labels when available:

```bash
target/release/hyphlab predict --saved-model en-US-typeset --with-hypher \
  --gold data/gold/moby_en_us_typeset.jsonl.zst \
  --word Japanese \
  --show-breaks
```

## Main Workflows

Prepare the core English data:

```bash
bash scripts/fetch_core_data.sh
bash scripts/curate_moby_typeset.sh
```

Additional corpora such as Wiktextract / Kaikki and hyph-bench are described in
[`data/README.md`](data/README.md).
After importing Wiktextract / Kaikki, build the multilingual typesetting
derivatives with:

```bash
bash scripts/curate_wiktextract_typeset.sh
```

Run the maintained multilingual 5-fold comparison after preparing the relevant
corpora:

```bash
bash scripts/run_multilingual_5fold_evaluation.sh
cat docs/reports/multilingual_5fold_v1/summary.md
```

The English typesetting comparison is summarized in
[`docs/reports/moby_typeset_5fold_v1/summary.md`](docs/reports/moby_typeset_5fold_v1/summary.md),
with the curation policy in
[`docs/reports/moby_typeset_5fold_v1/curation.md`](docs/reports/moby_typeset_5fold_v1/curation.md).
The multilingual Wiktextract typesetting comparison is summarized in
[`docs/reports/wiktextract_typeset_5fold_v1/summary.md`](docs/reports/wiktextract_typeset_5fold_v1/summary.md),
with the curation policy in
[`docs/reports/wiktextract_typeset_5fold_v1/curation.md`](docs/reports/wiktextract_typeset_5fold_v1/curation.md).

Build the reusable runtime models:

```bash
bash scripts/build_guarded_ngram_models.sh
cat models/guarded_ngram/v1/README.md
```

For fixed baselines, custom matrices, or one-off method evaluation, start from
[`docs/research_start.md`](docs/research_start.md).

## What Goes Where

| location | purpose |
| --- | --- |
| `docs/` | Evaluation policy, data usage, method notes, and reading order. |
| `docs/reports/` | Reproducible summary reports for selected comparisons. |
| `models/guarded_ngram/v1/` | Ready-to-run full-corpus runtime models. |
| `manifests/guarded_ngram/v1/` | Matrix manifests for reusable models. |
| `data/gold/` | Normalized training and evaluation labels. |
| `data/curation/` | Curation policy files for derived datasets. |
| `data/patterns/` | TeX and LibreOffice pattern files for Liang-style baselines. |
| `target/` | Local reports, temporary models, folds, and scratch outputs. |

The reusable models under `models/guarded_ngram/v1/` are trained from full
normalized corpora. They are convenient for demos and application integration.
For accuracy claims about trainable methods, use held-out split or 5-fold
reports.

## Add A Method

Create a simple Rust-native adapter:

```bash
bash scripts/new_native_method.sh my_algo --supports en,de
cargo fmt --all
cargo check -p hyph-cli
bash scripts/run_method_smoke.sh my_algo
DATASETS=moby_en_us bash scripts/run_baseline_matrix.sh
```

For a method needing custom setup, add a preparation branch in
`crates/hyph-cli/src/methods/registry.rs` and add a manifest entry. For a
non-Rust prototype, use an `external-jsonl` manifest entry with an
`external_command`.

## More Detail

- [Documentation map](docs/README.md)
- [Evaluation policy](docs/evaluation.md)
- [Data and model usage](docs/data_usage.md)
- [Guarded N-gram](docs/guarded_ngram.md)
- [Method development](docs/method_roadmap.md)
- [Data setup](data/README.md)
- [Experiment manifests](experiments/README.md)
