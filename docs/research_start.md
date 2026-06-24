# Research Workflow

Use this when setting up or resuming local experiments.

## 1. Smoke Test

```bash
bash scripts/run_toy_experiment.sh
cat target/hyphlab-reports/compare.md
```

This checks import, fixed baselines, metrics JSON, word-error JSONL, speed JSON,
and Markdown comparison output.

## 2. Data

```bash
bash scripts/fetch_core_data.sh
bash scripts/import_hyph_bench.sh
bash scripts/curate_moby_typeset.sh
bash scripts/fetch_kaikki.sh
bash scripts/import_wiktextract.sh
bash scripts/prepare_filtered_wiktextract_data.sh
bash scripts/curate_wiktextract_typeset.sh
```

See [`data_usage.md`](data_usage.md) and [`../data/README.md`](../data/README.md)
before choosing a corpus.

## 3. Fixed Baselines

Use full gold corpora only for methods that do not learn from the evaluated
file:

```bash
DATASETS=moby_en_us ITERATIONS=5 INIT_ITERATIONS=5 \
  bash scripts/run_baseline_matrix.sh
```

Reports are written under:

```text
target/hyphlab-reports/baselines/
```

## 4. Trainable Methods

Use grouped splits or deterministic 5-fold evaluation. For a manual Moby split:

```bash
cargo run -p hyph-cli --release -- data split \
  --input data/gold/moby_en_us.jsonl.zst \
  --output-dir data/splits/moby_en_us \
  --seed moby_en_us_v1
```

Use `train.jsonl.zst` for fitting, `dev.jsonl.zst` for choices, and
`test.jsonl.zst` for final comparison.

For the maintained multilingual comparison:

```bash
bash scripts/run_multilingual_5fold_evaluation.sh
cat docs/reports/multilingual_5fold_v1/summary.md
```

For the curated English typesetting comparison:

```bash
DATASETS=moby_en_us_typeset \
REPORT_TITLE="Moby Typesetting 5-Fold Evaluation" \
REPORT_ROOT=target/hyphlab-reports/moby_typeset_5fold_v1 \
FOLD_ROOT=target/hyphlab-folds/moby_typeset_5fold_v1 \
MODEL_ROOT=target/hyphlab-models/moby_typeset_5fold_v1 \
MANIFEST_ROOT=target/hyphlab-manifests/moby_typeset_5fold_v1 \
PUBLIC_REPORT_ROOT=docs/reports/moby_typeset_5fold_v1 \
  bash scripts/run_multilingual_5fold_evaluation.sh

cat docs/reports/moby_typeset_5fold_v1/summary.md
```

For the curated multilingual Wiktextract typesetting comparison:

```bash
DATASETS="wiktextract_cs_typeset wiktextract_de_typeset wiktextract_es_typeset wiktextract_it_typeset wiktextract_nl_typeset wiktextract_ru_cyrl_trusted_dedup_typeset wiktextract_tr_typeset" \
REPORT_TITLE="Wiktextract Typesetting 5-Fold Evaluation" \
REPORT_ROOT=target/hyphlab-reports/wiktextract_typeset_5fold_v1 \
FOLD_ROOT=target/hyphlab-folds/wiktextract_typeset_5fold_v1 \
MODEL_ROOT=target/hyphlab-models/wiktextract_typeset_5fold_v1 \
MANIFEST_ROOT=target/hyphlab-manifests/wiktextract_typeset_5fold_v1 \
PUBLIC_REPORT_ROOT=docs/reports/wiktextract_typeset_5fold_v1 \
  bash scripts/run_multilingual_5fold_evaluation.sh

cat docs/reports/wiktextract_typeset_5fold_v1/summary.md
```

## 5. Reusable Models

Build full-corpus runtime models after the data is available:

```bash
bash scripts/build_guarded_ngram_models.sh
target/release/hyphlab predict --list-saved-models
```

Use reusable models for demos and application integration. Use held-out folds
for claims about generalization.

## 6. Add a Method

Rust-native adapter:

```bash
bash scripts/new_native_method.sh my_algo --supports en
cargo fmt --all
cargo check -p hyph-cli --features adapters-hyphenation-embedded
bash scripts/run_method_smoke.sh my_algo
```

Non-Rust prototype:

1. Implement a persistent JSONL stdin/stdout process.
2. Add an `external-jsonl` manifest entry with `external_command`.
3. Run `hyphlab matrix` or one of the matrix scripts.

Trainable method:

```bash
target/release/hyphlab method materialize \
  --manifest experiments/manifests/method_workflow_example.toml \
  --gold data/splits/moby_en_us/train.jsonl.zst \
  --locale en-US \
  --model-dir target/hyphlab-models/my_algo \
  --output target/hyphlab-manifests/my_algo/runtime.toml

target/release/hyphlab matrix \
  --manifest target/hyphlab-manifests/my_algo/runtime.toml \
  --gold data/splits/moby_en_us/test.jsonl.zst \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --output-dir target/hyphlab-reports/my_algo
```

See [`add_method.md`](add_method.md) for the full integration paths.

## 7. Report Hygiene

Before using a result:

- Check whether the report is full-gold, split, or 5-fold.
- Check the `Evaluation Data` block.
- Keep trainable/tuned methods off their training labels.
- Exclude `identity-oracle` and dictionary-oracle rows from claim tables.
- Use release builds and enough iterations for speed comparisons.
