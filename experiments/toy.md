# Toy Experiment

```bash
bash scripts/run_toy_experiment.sh
cat target/hyphlab-reports/compare.md
```

This smoke experiment exercises:

- TSV import into normalized JSONL.
- `no-hyphen`, Typst `hypher`, `hyphenation 0.8.4`, dictionary lookup, and pure Liang baselines.
- Metrics JSON, word-error JSONL, and Markdown comparison-table generation.
