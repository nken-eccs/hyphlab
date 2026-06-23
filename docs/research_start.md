# Research Start Checklist

Use this checklist when setting up or resuming a local research run.

## 1. Smoke Test

```bash
bash scripts/run_toy_experiment.sh
cat target/hyphlab-reports/compare.md
```

Expected coverage:

- TSV import into normalized JSONL.
- `no-hyphen`, `hypher`, `hyphenation-embedded`, dictionary lookup, Liang, and
  consensus baselines.
- Metrics JSON, word-error JSONL, speed JSON, and Markdown comparison output.

## 2. Data

Core data:

```bash
bash scripts/fetch_core_data.sh
bash scripts/import_hyph_bench.sh
```

Optional Wiktionary / Kaikki data:

```bash
bash scripts/fetch_kaikki.sh
bash scripts/import_wiktextract.sh
```

Important outputs:

```text
data/gold/moby_en_us.jsonl.zst
data/gold/hyph_bench/*.jsonl.zst
data/gold/wiktextract/*.jsonl.zst
data/patterns/
```

See `data/README.md` for source inventory and restricted-data notes.

## 3. Baseline Matrix

For fixed, non-trainable methods, evaluate full gold corpora:

```bash
DATASETS=moby_en_us ITERATIONS=5 INIT_ITERATIONS=5 \
  bash scripts/run_baseline_matrix.sh
```

Open:

```text
target/hyphlab-reports/baselines/index.md
target/hyphlab-reports/baselines/moby_en_us/compare.md
```

Each `compare.md` starts with `Evaluation Data`, so the gold file and pattern
file are visible in the report itself.

## 4. Split-Based Trainable Runs

Build the split once:

```bash
cargo run -p hyph-cli --release -- data split \
  --input data/gold/moby_en_us.jsonl.zst \
  --output-dir data/splits/moby_en_us \
  --seed moby_en_us_v1
```

Train a CRF model:

```bash
cargo run -p hyph-cli --release -- crf train \
  --gold data/splits/moby_en_us/train.jsonl.zst \
  --locale en-US \
  --id trogkanis-elkan-crf \
  --epochs 5 \
  --threshold 0.9 \
  --output target/hyphlab-models/trogkanis_elkan_crf_moby_en_us.json
```

Tune the threshold on dev and write a compact model:

```bash
cargo run -p hyph-cli --release -- crf tune-threshold \
  --model target/hyphlab-models/trogkanis_elkan_crf_moby_en_us.json \
  --gold data/splits/moby_en_us/dev.jsonl.zst \
  --objective f05 \
  --output target/hyphlab-models/trogkanis_elkan_crf_moby_en_us_tuned.bin.zst \
  --report target/hyphlab-reports/trogkanis_elkan_crf_thresholds.json
```

Run the current CRF comparison on test:

```bash
ITERATIONS=5 INIT_ITERATIONS=5 bash scripts/run_crf_unified_matrix.sh
```

Open:

```text
target/hyphlab-reports/unified/moby_en_us_test_crf_sgd_tuned/compare.md
```

## 5. Add A New Method

The current high-precision fast-rule candidate is `safe-ngram`:

```bash
ITERATIONS=5 INIT_ITERATIONS=1 bash scripts/run_safe_ngram_matrix.sh
```

See `docs/method_roadmap.md` for the development plan.

Rust-native adapter path:

```bash
bash scripts/new_native_method.sh my_algo --supports en
cargo fmt --all
cargo check -p hyph-cli
bash scripts/run_method_smoke.sh my_algo
DATASETS=moby_en_us bash scripts/run_baseline_matrix.sh
```

External prototype path:

1. Implement a persistent JSONL stdin/stdout process.
2. Add an `external-jsonl` manifest entry with `external_command = "..."`.
3. Run `hyphlab matrix` or `scripts/run_baseline_matrix.sh`.

## 6. Report Hygiene

Before using a result in notes or a table, check:

- Is the report full-gold or split-based?
- Does the `Evaluation Data` block show the expected gold file?
- Are trainable/tuned methods evaluated only on `test.jsonl.zst`?
- Are `identity-oracle` and `dict-oracle` excluded from claim tables?
- Are speed values from release builds with enough iterations?
