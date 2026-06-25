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

Use `*-typeset` models for text shown to readers. These models are trained from
curated corpora and apply guard policies at runtime, so unsafe fragments,
MixedCase / ALLCAPS tokens, and configured proper names stay intact.

Use plain models such as `en-US` when you want labels closer to the source
syllable or lexical dictionary. For runtime details and guard-policy
customization, see [`docs/guarded_ngram.md`](docs/guarded_ngram.md).

Other reader-facing models use the same suffix:

```bash
target/release/hyphlab predict --saved-model de-typeset --word Scheißhaus
target/release/hyphlab predict --saved-model tr-typeset --word cumhuriyet
```

List every reusable model:

```bash
target/release/hyphlab predict --list-saved-models
```

Check the install with a tiny experiment:

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

Prepare corpora:

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

Build reusable runtime models:

```bash
bash scripts/build_guarded_ngram_models.sh
cat models/guarded_ngram/v1/README.md
```

Run the maintained multilingual 5-fold comparison:

```bash
bash scripts/run_multilingual_5fold_evaluation.sh
cat docs/reports/multilingual_5fold_v1/summary.md
```

Maintained reports are indexed in
[`docs/README.md#maintained-reports`](docs/README.md#maintained-reports).

For baselines, custom matrices, or one-off evaluations, start from
[`docs/research_start.md`](docs/research_start.md).

## Project Map

| location | purpose |
| --- | --- |
| `docs/` | Evaluation policy, data usage, method notes, and reports. |
| `docs/reports/` | Reproducible summary reports for selected comparisons. |
| `models/guarded_ngram/v1/` | Ready-to-run full-corpus runtime models. |
| `manifests/guarded_ngram/v1/` | Matrix manifests for reusable models. |
| `data/gold/` | Normalized training and evaluation labels. |
| `data/curation/` | Guard policies, fragment lists, and proper-name lists for derived datasets. |
| `data/challenges/` | No-break challenge data for names and case-sensitive tokens. |
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
