# hyphlab

Research environment for multilingual hyphenation methods.

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

Use a reusable model:

```bash
target/release/hyphlab predict --list-saved-models
target/release/hyphlab predict --saved-model en-US --word hyphenation --word typesetting
target/release/hyphlab predict --saved-model de --text "Silbentrennung fuer lange Woerter"
target/release/hyphlab predict --saved-model it --word informazione --word straordinario
```

Compare with Hypher and known gold labels when available:

```bash
target/release/hyphlab predict --saved-model en-US --with-hypher \
  --gold data/gold/toy_en.jsonl \
  --word hyphenation \
  --show-breaks
```

## Main Workflows

Prepare data:

```bash
bash scripts/fetch_core_data.sh
bash scripts/import_hyph_bench.sh
bash scripts/fetch_kaikki.sh
bash scripts/import_wiktextract.sh
bash scripts/prepare_filtered_wiktextract_data.sh
```

Run fixed baselines on full gold data:

```bash
DATASETS=moby_en_us ITERATIONS=5 INIT_ITERATIONS=5 \
  bash scripts/run_baseline_matrix.sh
```

Run the multilingual 5-fold comparison:

```bash
bash scripts/run_multilingual_5fold_evaluation.sh
cat docs/reports/multilingual_5fold_v1/summary.md
```

Run the additional hyph-bench 5-fold comparison:

```bash
DATASETS="hyph_bench_cs_cstenten hyph_bench_cs_ujc hyph_bench_cssk_cshyphen hyph_bench_de_wortliste" \
ENABLE_LIBREOFFICE_BASELINE=1 \
REPORT_TITLE="hyph-bench 5-Fold Evaluation" \
REPORT_ROOT=target/hyphlab-reports/hyph_bench_5fold_v1 \
FOLD_ROOT=target/hyphlab-folds/hyph_bench_5fold_v1 \
MODEL_ROOT=target/hyphlab-models/hyph_bench_5fold_v1 \
MANIFEST_ROOT=target/hyphlab-manifests/hyph_bench_5fold_v1 \
PUBLIC_REPORT_ROOT=docs/reports/hyph_bench_5fold_v1 \
  bash scripts/run_multilingual_5fold_evaluation.sh
```

Regenerate reusable runtime models:

```bash
bash scripts/build_guarded_ngram_models.sh
cat models/guarded_ngram/v1/README.md
```

## What Goes Where

| location | purpose |
| --- | --- |
| `docs/README.md` | Documentation map and reading order. |
| `docs/evaluation.md` | Evaluation policy, leakage rules, and speed columns. |
| `docs/data_usage.md` | Data, model, and report roles. |
| `docs/guarded_ngram.md` | Current reusable method family and model recipes. |
| `docs/method_roadmap.md` | Method development goals and next experiments. |
| `docs/reports/` | Reproducible summary reports for selected comparisons. |
| `models/guarded_ngram/v1/` | Ready-to-run full-corpus runtime models. |
| `manifests/guarded_ngram/v1/` | Matrix manifests for reusable models. |
| `data/gold/` | Normalized training and evaluation labels. |
| `data/patterns/` | TeX and LibreOffice pattern files for Liang-style baselines. |
| `target/` | Local reports, temporary models, folds, and scratch outputs. |

The reusable models under `models/guarded_ngram/v1/` are trained from full
normalized corpora. They are convenient for demos and application integration.
For accuracy claims about trainable methods, use held-out split or 5-fold
reports.

## Common Commands

Evaluate one method:

```bash
target/release/hyphlab eval \
  --gold data/gold/moby_en_us.jsonl.zst \
  --method hypher \
  --locale en-US \
  --output target/hyphlab-reports/manual/hypher.json \
  --errors-output target/hyphlab-reports/manual/hypher_errors.jsonl
```

Evaluate Liang patterns:

```bash
target/release/hyphlab eval \
  --gold data/gold/moby_en_us.jsonl.zst \
  --method liang \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --output target/hyphlab-reports/manual/liang_tex.json
```

Run a manifest matrix:

```bash
target/release/hyphlab matrix \
  --manifest methods.toml \
  --gold data/gold/moby_en_us.jsonl.zst \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --output-dir target/hyphlab-reports/manual/moby_en_us \
  --iterations 5 \
  --init-iterations 5
```

Compile a Guarded N-gram model:

```bash
target/release/hyphlab compile-safe-ngram \
  --gold data/splits/moby_en_us/train.jsonl.zst \
  --locale en-US \
  --method safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85 \
  --output models/guarded_ngram/custom/en_us.bin
```

Compile an Italian onset-syllable model:

```bash
target/release/hyphlab compile-italian-syllable \
  --gold data/gold/wiktextract/it.jsonl.zst \
  --locale it \
  --output models/guarded_ngram/custom/it.json
```

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
`crates/hyph-cli/src/methods/registry.rs` and add a manifest entry.

For a non-Rust prototype, use an `external-jsonl` manifest entry with an
`external_command`.

## Source Layout

- `crates/hyph-cli/src/main.rs`: imports and command dispatch.
- `crates/hyph-cli/src/cli.rs`: CLI subcommands and argument structs.
- `crates/hyph-cli/src/commands/`: data preparation, evaluation, benchmark,
  matrix, CRF, compile, and scaffold commands.
- `crates/hyph-cli/src/methods/`: method registry, baselines, Guarded N-gram,
  Italian onset models, and saved model IO.
- `crates/hyph-cli/src/predict.rs`: interactive prediction and comparison
  output.
- `crates/hyph-cli/src/reports.rs`: report parsing and Markdown rendering.

## More Detail

- [Documentation map](docs/README.md)
- [Evaluation policy](docs/evaluation.md)
- [Data and model usage](docs/data_usage.md)
- [Guarded N-gram](docs/guarded_ngram.md)
- [Method development](docs/method_roadmap.md)
- [Data setup](data/README.md)
- [Experiment manifests](experiments/README.md)
