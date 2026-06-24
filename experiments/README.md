# Experiments

This directory stores reusable experiment definitions. New reports, temporary
models, folds, and local probes should be written under `target/`.

## Manifest Groups

| manifest | role |
| --- | --- |
| `manifests/method_workflow_example.toml` | Minimal training manifest for `hyphlab method materialize`. |
| `manifests/moby_en_us_with_crf.toml` | Moby split comparison with fixed baselines, train-split dictionary, and CRF. |
| `manifests/moby_en_us_crf_tuned.toml` | Moby CRF threshold variants. |
| `manifests/moby_en_us_crf_sgd_tuned.toml` | Moby CRF SGD and regularization variants. |
| `manifests/moby_en_us_safe_ngram.toml` | Moby Guarded N-gram candidate sweep. |
| `manifests/moby_en_us_safe_ngram_compiled.toml` | Moby compiled Guarded N-gram model check. |
| `manifests/moby_en_us_precision_recall90_simple.toml` | Exploratory Moby precision/recall candidates that depend on local probe artifacts. |

The maintained comparison reports are:

```text
docs/reports/multilingual_5fold_v1/summary.md
docs/reports/hyph_bench_5fold_v1/summary.md
```

Use [`../docs/evaluation.md`](../docs/evaluation.md) before promoting a local
experiment result to a report.

## Common Runs

CRF comparison:

```bash
bash scripts/run_crf_unified_matrix.sh
```

Guarded N-gram Moby sweep:

```bash
bash scripts/run_safe_ngram_matrix.sh
```

Multilingual Guarded N-gram tuning:

```bash
bash scripts/prepare_filtered_wiktextract_data.sh
bash scripts/run_multilingual_safe_ngram_tuning.sh
cat target/hyphlab-reports/multilingual/safe_ngram_tuned/summary.md
```

Maintained multilingual 5-fold report:

```bash
bash scripts/run_multilingual_5fold_evaluation.sh
cat docs/reports/multilingual_5fold_v1/summary.md
```

Manual manifest run:

```bash
target/release/hyphlab matrix \
  --manifest experiments/manifests/moby_en_us_crf_sgd_tuned.toml \
  --gold data/splits/moby_en_us/test.jsonl.zst \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --output-dir target/hyphlab-reports/unified/moby_en_us_test_crf_sgd_tuned \
  --iterations 5 \
  --init-iterations 5
```

Training manifest run:

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
  --output-dir target/hyphlab-reports/my_algo \
  --iterations 5 \
  --init-iterations 5
```

## Notes

- Keep exploratory manifests here if they are still useful as templates.
- Keep durable conclusions in `docs/`.
- Keep generated metrics and models under `target/`.
- Recreate missing CRF or Guarded N-gram models with the scripts documented in
  [`../docs/research_start.md`](../docs/research_start.md).
