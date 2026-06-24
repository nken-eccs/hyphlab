# Add a Method

Use this page when adding a new hyphenation algorithm and comparing it with the
existing methods.

## Choose the Path

| method shape | use this path | main files |
| --- | --- | --- |
| Fixed Rust-native method | Implement a `MethodAdapter` | `crates/hyph-adapters/src/`, `manifests/baselines.toml` |
| Trainable method | Add a trainer, then materialize a runtime manifest | `crates/hyph-cli/src/methods/`, `crates/hyph-cli/src/commands/method.rs` |
| Saved-model method | Add a runtime loader and manifest row | `crates/hyph-cli/src/methods/registry.rs`, `manifests/` |
| Non-Rust prototype | Use a persistent JSONL subprocess | experiment manifest with `method = "external-jsonl"` |

Fixed methods can be evaluated on full gold corpora. Trainable, tuned, or
distilled methods must be evaluated with held-out splits or 5-fold runs.

## Fixed Rust Method

Create the adapter and append a manifest row:

```bash
bash scripts/new_native_method.sh my_algo --supports en,de
```

Then implement `hyphenate_into` in the generated file under
`crates/hyph-adapters/src/`. The scaffold also registers the adapter in
`crates/hyph-adapters/src/lib.rs` and appends a row to
`manifests/baselines.toml`.

Check the method:

```bash
cargo fmt --all
cargo check -p hyph-cli --features adapters-hyphenation-embedded
bash scripts/run_method_smoke.sh my_algo
```

Compare it with fixed baselines:

```bash
DATASETS=moby_en_us ITERATIONS=5 INIT_ITERATIONS=5 \
  bash scripts/run_baseline_matrix.sh
```

The baseline manifest intentionally contains only methods that do not train on
the evaluated labels.

## Trainable Method

Use this path when the method learns from labels, compiles a model, tunes a
threshold, or distills another method. The standard workflow is:

```text
train or fold gold -> hyphlab method materialize -> runtime manifest -> hyphlab matrix
```

Create a training manifest:

```toml
[[methods]]
slug = "hypher"
method = "hypher"
supports = ["en"]

[[methods]]
slug = "my_algo"
method = "safe-ngram-3x3-s1-p80"
supports = ["en"]

[methods.train]
runtime_method = "safe-ngram-model"
output = "{model_dir}/{slug}.bin"
```

Materialize it with train data:

```bash
target/release/hyphlab data split \
  --input data/gold/moby_en_us.jsonl.zst \
  --output-dir data/splits/moby_en_us \
  --seed my_algo_moby_v1

target/release/hyphlab method materialize \
  --manifest experiments/manifests/my_algo.toml \
  --gold data/splits/moby_en_us/train.jsonl.zst \
  --locale en-US \
  --model-dir target/hyphlab-models/my_algo \
  --output target/hyphlab-manifests/my_algo/runtime.toml

target/release/hyphlab matrix \
  --manifest target/hyphlab-manifests/my_algo/runtime.toml \
  --gold data/splits/moby_en_us/test.jsonl.zst \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --output-dir target/hyphlab-reports/my_algo/moby_en_us_test \
  --iterations 20 \
  --init-iterations 5
```

`hyphlab matrix` rejects manifests that still contain `[methods.train]`, so a
trainable method cannot accidentally be evaluated before its fold-local model
has been built.

For a new trainable family, add its runtime loader under
`crates/hyph-cli/src/methods/` and one training branch in
`crates/hyph-cli/src/commands/method.rs`. Existing supported families are
Guarded N-gram, Italian syllable, and CRF.

You can also train one model directly:

```bash
target/release/hyphlab method train \
  --method safe-ngram-3x3-s1-p80 \
  --gold data/splits/moby_en_us/train.jsonl.zst \
  --locale en-US \
  --output target/hyphlab-models/my_algo/model.bin
```

## Saved Model

Use this path when a model already exists and no training should happen during
the comparison. Add a runtime manifest row:

```toml
[[methods]]
slug = "my_algo"
method = "safe-ngram-model"
dictionary = "../models/my_algo.bin"
supports = ["en"]
```

Then run `hyphlab matrix`. Full-corpus reusable models are useful for demos and
application integration; use split or 5-fold materialization for accuracy
claims.

## External Prototype

For quick experiments in another language, run a persistent subprocess:

```toml
[[methods]]
slug = "my_external"
method = "external-jsonl"
external_command = "python3 experiments/prototypes/my_algo.py"
supports = ["en"]
```

The process reads one JSON object per line from stdin and writes one JSON object
per line to stdout:

```json
{"id":"1","word":"hyphenation","lang":"en-US"}
{"id":"1","breaks":[2,6]}
```

It can also return:

```json
{"id":"1","hyphenated":"hy-phen-ation"}
```

Run it with `hyphlab matrix` like any other runtime manifest entry.

## Before Promoting a Result

- Keep fixed-method, split, and 5-fold results separate.
- Do not include `identity-oracle`, dictionary-oracle, or full-corpus reusable
  models in claim tables for trainable methods.
- Report precision, recall, F1, F0.5, exact word accuracy, `serious_error`,
  false positives per 100k boundaries, initialization time, and steady ns/word.
- Put durable reports under `docs/reports/`; put scratch folds, models, and
  probes under `target/`.
