# Documentation

This directory keeps the durable project notes. Local scratch reports, temporary
models, and exploratory outputs should stay under `target/`.

## Reading Order

| read this | when |
| --- | --- |
| [`../README.md`](../README.md) | You want to run the CLI or the main scripts. |
| [`evaluation.md`](evaluation.md) | You need to know which comparisons are fair. |
| [`add_method.md`](add_method.md) | You are adding a Rust method, trainable model, or external prototype. |
| [`data_usage.md`](data_usage.md) | You need to know which data or model file to use. |
| [`guarded_ngram.md`](guarded_ngram.md) | You need the current method recipes or model format. |
| [`research_start.md`](research_start.md) | You are resuming hands-on experiments. |

## Maintained Reports

| report | purpose |
| --- | --- |
| [`reports/multilingual_5fold_v1/summary.md`](reports/multilingual_5fold_v1/summary.md) | Main held-out multilingual comparison. |
| [`reports/moby_typeset_5fold_v1/summary.md`](reports/moby_typeset_5fold_v1/summary.md) | Held-out comparison on the curated Moby typesetting corpus. |
| [`reports/wiktextract_typeset_5fold_v1/summary.md`](reports/wiktextract_typeset_5fold_v1/summary.md) | Held-out comparison on curated Wiktextract typesetting corpora. |
| [`reports/hyph_bench_5fold_v1/summary.md`](reports/hyph_bench_5fold_v1/summary.md) | Additional held-out comparison on selected hyph-bench data. |

Each report documents its protocol, datasets, selected methods, and runtime
settings. Treat those report files as the source for published numbers. Other
documents should describe how to reproduce results, not copy large metric
tables.

## Documentation Rules

- Put data and model roles in `data_usage.md`.
- Put evaluation policy in `evaluation.md`.
- Put method integration steps in `add_method.md`.
- Put method mechanics and recipes in `guarded_ngram.md`.
- Keep exploratory commands in `experiments/README.md` or scripts, not in the
  main README.
- Prefer linking to report summaries instead of duplicating metric tables.
