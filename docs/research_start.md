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
bash scripts/fetch_kaikki.sh
bash scripts/import_wiktextract.sh
bash scripts/prepare_filtered_wiktextract_data.sh
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

## 5. Reusable Models

Build full-corpus runtime models after the data is available:

```bash
bash scripts/build_guarded_ngram_models.sh
target/release/hyphlab predict --list-saved-models
```

Use reusable models for demos and application integration. Use held-out folds
for claims about generalization.

## 6. Add A Method

Rust-native adapter:

```bash
bash scripts/new_native_method.sh my_algo --supports en
cargo fmt --all
cargo check -p hyph-cli
bash scripts/run_method_smoke.sh my_algo
```

Non-Rust prototype:

1. Implement a persistent JSONL stdin/stdout process.
2. Add an `external-jsonl` manifest entry with `external_command`.
3. Run `hyphlab matrix` or one of the matrix scripts.

## 7. Report Hygiene

Before using a result:

- Check whether the report is full-gold, split, or 5-fold.
- Check the `Evaluation Data` block.
- Keep trainable/tuned methods off their training labels.
- Exclude `identity-oracle` and dictionary-oracle rows from claim tables.
- Use release builds and enough iterations for speed comparisons.
