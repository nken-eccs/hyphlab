# Documentation

This directory keeps the durable project notes and maintained report summaries.

## Start Here

| read this | when |
| --- | --- |
| [`../README.md`](../README.md) | You want to run the CLI or main scripts. |
| [`data_usage.md`](data_usage.md) | You are choosing data, models, or reports. |
| [`evaluation.md`](evaluation.md) | You need to know which comparisons are fair. |
| [`guarded_ngram.md`](guarded_ngram.md) | You need method mechanics, recipes, or guard-policy customization. |
| [`add_method.md`](add_method.md) | You are adding a Rust method, trainable model, or external prototype. |
| [`research_start.md`](research_start.md) | You are resuming hands-on experiments. |

## Maintained Reports

| report | purpose |
| --- | --- |
| [`reports/multilingual_5fold_v1/summary.md`](reports/multilingual_5fold_v1/summary.md) | Main held-out multilingual comparison. |
| [`reports/typeset_policy_5fold_v1/summary.md`](reports/typeset_policy_5fold_v1/summary.md) | Held-out comparison for the current typesetting guard policy. |
| [`reports/typeset_guard_challenge_v1/summary.md`](reports/typeset_guard_challenge_v1/summary.md) | No-break challenge for names and case-sensitive tokens. |
| [`reports/moby_typeset_5fold_v1/summary.md`](reports/moby_typeset_5fold_v1/summary.md) | English typesetting curation details. |
| [`reports/wiktextract_typeset_5fold_v1/summary.md`](reports/wiktextract_typeset_5fold_v1/summary.md) | Wiktextract typesetting curation details. |
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
