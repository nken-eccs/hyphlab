# Evaluation Policy

This project keeps two evaluation tracks separate.

## Full-Gold Baselines

Use full normalized gold corpora for methods that do not learn from the
evaluation file:

- `no-hyphen`
- `hypher`
- `hyphenation-embedded`
- `hyphenation-runtime`
- `liang`
- `hypher-liang-consensus`
- fixed external engines that were not fit on the evaluated corpus

Entry point:

```bash
DATASETS=moby_en_us ITERATIONS=5 INIT_ITERATIONS=5 \
  bash scripts/run_baseline_matrix.sh
```

This writes reports under:

```text
target/hyphlab-reports/baselines/
```

Use this track when the question is "how do fixed engines compare on all
available labeled words?"

## Split-Based Trainable Runs

Use train/dev/test splits for any method that can see labels from the same
source corpus:

- CRF or other learned models
- `safe-ngram` and other rules learned from train data
- tuned thresholds
- learned pattern sets
- dictionary baselines populated from the same source corpus
- any prototype whose parameters were chosen from corpus results

Create the split:

```bash
cargo run -p hyph-cli --release -- data split \
  --input data/gold/moby_en_us.jsonl.zst \
  --output-dir data/splits/moby_en_us \
  --seed moby_en_us_v1
```

The splitter groups by `lang + lowercase(word)`, so duplicate words and
ambiguous variants cannot cross split boundaries.

Use:

- `train.jsonl.zst` for fitting.
- `dev.jsonl.zst` for threshold or hyperparameter choices.
- `test.jsonl.zst` for final comparison.

Entry point for the current CRF comparison:

```bash
bash scripts/run_crf_unified_matrix.sh
```

This writes:

```text
target/hyphlab-reports/unified/moby_en_us_test_crf_sgd_tuned/compare.md
```

## Oracles And Leakage

`identity-oracle` and `dict` without `--dictionary` are sanity checks only.
They use the evaluated record or evaluated gold file and must not be included in
claim tables.

A fair dictionary baseline for a split experiment uses:

```bash
--dictionary data/splits/moby_en_us/train.jsonl.zst
--gold data/splits/moby_en_us/test.jsonl.zst
```

## Report Metadata

Metrics JSON and speed JSON include an `evaluation` object:

```json
{
  "gold": "data/gold/moby_en_us.jsonl.zst",
  "locale": "en-US",
  "patterns": "data/patterns/tex-hyphen/tex/hyph-en-us.tex",
  "ambiguous_policy": "exclude",
  "left_min": null,
  "right_min": null,
  "min_word_len": null
}
```

`compare.md` renders this as an `Evaluation Data` block at the top. If rows use
different pattern files or setup options, the block says that the metadata is
mixed and lists the values.

## Speed Columns

- `steady ns/word`: prediction throughput after the method is prepared.
- `steady words/sec`: inverse throughput for scan-friendly comparisons.
- `init ms`: method preparation time, including dictionary loading, pattern
  parsing, automaton construction, model loading, or subprocess startup.

Keep these separate. Embedded methods may have very low `init ms`; runtime
pattern parsers and model loaders can have much higher preparation cost even if
their steady-state speed is acceptable.
