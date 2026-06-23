# hyphlab

Rust research environment for multilingual hyphenation experiments.

`hyphlab` is organized around one repeatable loop:

1. Normalize gold corpora into JSONL / JSONL.zst records.
2. Register hyphenation methods in a manifest.
3. Run accuracy, steady-state speed, and initialization benchmarks.
4. Compare all methods in Markdown reports with explicit evaluation metadata.

## Start Here

Run the smoke test first:

```bash
cd hyphenation/hyphlab
bash scripts/run_toy_experiment.sh
cat target/hyphlab-reports/compare.md
```

This checks import, evaluation, error reporting, comparison, `hypher`,
`hyphenation`, dictionary lookup, Liang patterns, and consensus baselines.

Then download and normalize the main research data:

```bash
bash scripts/fetch_core_data.sh
bash scripts/import_hyph_bench.sh
bash scripts/fetch_kaikki.sh
bash scripts/import_wiktextract.sh
bash scripts/prepare_filtered_wiktextract_data.sh
```

The filtered Wiktextract step keeps the original normalized corpora and adds
script-specific / duplicate-normalized corpora used by multilingual experiments,
for example `data/gold/wiktextract/ru_cyrl_dedup.jsonl.zst`. For Russian it
also writes `ru_cyrl_trusted_dedup.jsonl.zst`, which excludes long vowel-bearing
no-break entries that are likely missing hyphenation annotations rather than
trusted negative examples.

Run the full-gold baseline matrix for non-trainable methods:

```bash
bash scripts/run_baseline_matrix.sh
```

The main index is written to:

```text
target/hyphlab-reports/baselines/index.md
```

For trainable methods, use split-based or 5-fold tracks so the evaluated labels
are not visible during training or tuning. The fixed multilingual comparison is
published under `docs/reports/multilingual_5fold_v1/` and can be regenerated
with `scripts/run_multilingual_5fold_evaluation.sh`.

## Evaluation Tracks

Use two separate tracks.

### Full-Gold Baselines

Use this for methods that do not train or tune on the evaluated corpus:

```bash
DATASETS=moby_en_us ITERATIONS=5 INIT_ITERATIONS=5 \
  bash scripts/run_baseline_matrix.sh
```

This evaluates the full normalized gold file, for example
`data/gold/moby_en_us.jsonl.zst`.

### Split-Based Trainable Runs

Use this for CRF, learned models, tuned thresholds, dictionary baselines trained
from the same source corpus, or any method that can leak evaluation answers:

```bash
cargo run -p hyph-cli --release -- data split \
  --input data/gold/moby_en_us.jsonl.zst \
  --output-dir data/splits/moby_en_us \
  --seed moby_en_us_v1

bash scripts/run_crf_unified_matrix.sh
```

This evaluates on `data/splits/moby_en_us/test.jsonl.zst`.

## Multilingual 5-Fold Evaluation

Use this for the fixed multilingual comparison. The script uses one selected
method per dataset, runs deterministic 5-fold cross-validation, trains only on
each fold's train split, and evaluates Hypher, Liang, and the selected method on
the same fold test split.

```bash
bash scripts/run_multilingual_5fold_evaluation.sh
cat docs/reports/multilingual_5fold_v1/summary.md
```

The selected methods and reusable recipes are documented in
[`docs/guarded_ngram.md`](docs/guarded_ngram.md).

## Reusable Model Artifacts

Prebuilt reusable models and manifests are kept in the repository:

```text
models/guarded_ngram/v1/
manifests/guarded_ngram/v1/
```

For example, run the English model from a fresh checkout:

```bash
cargo build -p hyph-cli --release --features adapters-hyphenation-embedded

target/release/hyphlab eval \
  --gold data/gold/toy_en.jsonl \
  --locale en-US \
  --method safe-ngram-model \
  --dictionary models/guarded_ngram/v1/moby_en_us.bin \
  --output target/hyphlab-reports/manual/guarded_ngram_toy_en.json
```

Regenerate the reusable models after fetching and importing the full data:

```bash
bash scripts/build_guarded_ngram_models.sh
cat models/guarded_ngram/v1/README.md
```

Guarded N-gram languages generate compact `.bin` files under `models/`. Italian
uses a compact onset-syllable JSON model under `models/`. For unbiased
evaluation, train on a split and evaluate on held-out data.

Try the reusable Italian model without downloading any evaluation corpus:

```bash
printf "informazione\nstraordinario\nuniversita\n" |
  target/release/hyphlab predict \
    --locale it \
    --method italian-syllable-model \
    --dictionary models/guarded_ngram/v1/wiktextract_it.json
```

## Published Artifacts And Local Reports

Committed artifacts:

- Multilingual 5-fold evaluation:
  `docs/reports/multilingual_5fold_v1/summary.md`
- Reusable Guarded N-gram models:
  `models/guarded_ngram/v1/README.md`
- Reusable Guarded N-gram manifests:
  `manifests/guarded_ngram/v1/`

Generated local reports and scratch models are written under `target/`, which
is ignored by Git. Common outputs include:

- Full-gold baseline matrix:
  `target/hyphlab-reports/baselines/index.md`
- Moby train/dev/test CRF comparison:
  `target/hyphlab-reports/unified/moby_en_us_test_crf_sgd_tuned/compare.md`
- Moby 5-fold current-candidate summary:
  `target/hyphlab-reports/research/moby_en_us_current_candidates_5fold/summary.md`
- Multilingual Unicode `safe-ngram` p95 tuning summary:
  `target/hyphlab-reports/multilingual/safe_ngram_tuned/summary.md`
- Multilingual Unicode `safe-ngram` p90 tuning summary:
  `target/hyphlab-reports/multilingual/safe_ngram_tuned_p90/summary.md`
- Binary-size report:
  `target/hyphlab-reports/binary-size.md`
- CRF models:
  `target/hyphlab-models/`

Every new `compare.md` begins with an `Evaluation Data` block showing the gold
path, locale, pattern file, ambiguity policy, and boundary configuration.

## Directory Map

```text
crates/
  hyph-cli/       CLI, matrix runner, report generation, CRF commands
  hyph-core/      record types, language tags, grapheme boundary utilities
  hyph-data/      importers and JSONL / JSONL.zst IO
  hyph-eval/      metrics and error records
  hyph-patterns/  pure Rust Liang pattern parser and evaluator
  hyph-adapters/  native crate adapters and adapter benchmarks
  hyph-crf/       trainable CRF reproduction and model formats

data/
  gold/           normalized corpora
  splits/         train/dev/test splits
  patterns/       copied TeX and LibreOffice pattern files
  raw/            downloaded upstream archives and raw corpora
  manifests/      source and license inventory

models/
  guarded_ngram/  reusable compiled Guarded N-gram models

manifests/
  guarded_ngram/  reusable manifests for compiled models

experiments/
  manifests/      reusable matrix manifests
  toy.md          smoke experiment note

scripts/
  fetch_*.sh      download upstream data
  import_*.sh     normalize upstream data
  run_*.sh        repeatable experiment entry points

target/
  hyphlab-reports/ generated detailed reports and scratch outputs
  hyphlab-models/  generated CRF and temporary safe-ngram models
```

## Common Commands

Evaluate one method:

```bash
cargo run -p hyph-cli --release -- eval \
  --gold data/gold/moby_en_us.jsonl.zst \
  --method hypher \
  --locale en-US \
  --output target/hyphlab-reports/manual/hypher.json \
  --errors-output target/hyphlab-reports/manual/hypher_errors.jsonl
```

Evaluate Liang patterns:

```bash
cargo run -p hyph-cli --release -- eval \
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

Run the reusable Italian onset-syllable model:

```bash
printf "informazione\nstraordinario\nuniversita\n" |
  target/release/hyphlab predict \
    --locale it \
    --method italian-syllable-model \
    --dictionary models/guarded_ngram/v1/wiktextract_it.json
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

Run the 5-fold check for the current fast-rule candidate set:

```bash
bash scripts/run_safe_ngram_kfold.sh
cat target/hyphlab-reports/research/moby_en_us_current_candidates_5fold/summary.md
```

Run adapter Criterion benchmarks:

```bash
cargo bench -p hyph-adapters --bench crate_baselines
cargo bench -p hyph-adapters --features rust-hyphenation-embedded --bench crate_baselines
bash scripts/report_binary_size.sh
```

## Add A Method

For a simple Rust-native adapter:

```bash
bash scripts/new_native_method.sh my_algo --supports en,de
cargo fmt --all
cargo check -p hyph-cli
bash scripts/run_method_smoke.sh my_algo
DATASETS=moby_en_us bash scripts/run_baseline_matrix.sh
```

For a method needing custom setup, add a `prepare_<method>` path in
`crates/hyph-cli/src/main.rs` and add a manifest entry.

For a non-Rust prototype, use `external-jsonl` in a manifest with
`external_command = "..."`.

## More Detail

- [Research workflow](docs/research_start.md)
- [Evaluation policy](docs/evaluation.md)
- [Method roadmap](docs/method_roadmap.md)
- [Data setup](data/README.md)
- [Experiment manifests](experiments/README.md)
- Source and license inventory: `data/manifests/sources.yaml`
