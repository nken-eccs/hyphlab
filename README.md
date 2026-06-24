# hyphlab

Toolkit for multilingual hyphenation data, evaluation, and runtime models.

`hyphlab` has three main jobs:

1. Normalize hyphenation corpora into a shared JSONL format.
2. Train or load hyphenation methods behind one CLI.
3. Compare accuracy, serious errors, startup cost, and steady-state speed with
   repeatable reports.

## Quick Start

Build the CLI:

```bash
cd hyphenation/hyphlab
cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
```

Use the English typesetting model:

```bash
target/release/hyphlab predict --saved-model en-US-typeset --word Japanese
target/release/hyphlab predict --saved-model en-US-typeset \
  --text "Japanese typography needs careful hyphenation."
```

Use `en-US-typeset` for reader-facing line breaks. It is trained from the
curated Moby typesetting corpus and applies the same fragment guard at runtime.
Use `en-US` when you want labels closer to the source syllable dictionary.

Reader-facing multilingual models use the `*-typeset` suffix:

```bash
target/release/hyphlab predict --saved-model de-typeset --word Scheißhaus
target/release/hyphlab predict --saved-model tr-typeset --word cumhuriyet
```

List every reusable model:

```bash
target/release/hyphlab predict --list-saved-models
```

Run the smoke test:

```bash
bash scripts/run_toy_experiment.sh
cat target/hyphlab-reports/compare.md
```

Compare with Hypher and known gold labels when available:

```bash
target/release/hyphlab predict --saved-model en-US-typeset --with-hypher \
  --gold data/gold/moby_en_us_typeset.jsonl.zst \
  --word Japanese \
  --show-breaks
```

## Main Workflows

Prepare English data:

```bash
bash scripts/fetch_core_data.sh
bash scripts/curate_moby_typeset.sh
```

Additional corpora such as Wiktextract / Kaikki and hyph-bench are covered in
[`data/README.md`](data/README.md). After importing Wiktextract / Kaikki, build
the multilingual typesetting derivatives with:

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

For fixed baselines, custom matrices, or one-off evaluations, start from
[`docs/research_start.md`](docs/research_start.md).

## What Goes Where

| location | purpose |
| --- | --- |
| `docs/` | Evaluation policy, data usage, method notes, and reports. |
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

## Add a Method

Create a simple Rust-native adapter:

```bash
bash scripts/new_native_method.sh my_algo --supports en,de
cargo fmt --all
cargo check -p hyph-cli --features adapters-hyphenation-embedded
bash scripts/run_method_smoke.sh my_algo
DATASETS=moby_en_us bash scripts/run_baseline_matrix.sh
```

For trainable methods, saved-model methods, or non-Rust prototypes, use
[`docs/add_method.md`](docs/add_method.md).

## More Detail

- [Documentation map](docs/README.md)
- [Evaluation policy](docs/evaluation.md)
- [Add a method](docs/add_method.md)
- [Data and model usage](docs/data_usage.md)
- [Guarded N-gram](docs/guarded_ngram.md)
- [Data setup](data/README.md)
- [Experiment manifests](experiments/README.md)
