# Method Roadmap

Goal: develop a method that beats `hypher` on precision, recall, F1, F0.5,
exact word accuracy, `serious_error`, false-positive rate, and steady-state
runtime.

The current target is not maximum recall alone. The first research frontier is:

```text
precision high
serious_error low
steady ns/word below hypher
recall and exact accuracy above hypher under those constraints
```

## Background

Liang's TeX hyphenation algorithm remains the main practical baseline for
pattern hyphenation. The TeX Users Group hosts Liang's thesis and notes that
`patgen` is the program that produces TeX patterns:
<https://tug.org/docs/liang/>.

Trogkanis and Elkan frame hyphenation as sequence prediction and explicitly
note that false positive hyphens are worse than false negatives. Their CRF paper
is the reference for learned sequence models in this lab:
<https://aclanthology.org/P10-1038/>.

## Implemented Candidate Family: `safe-ngram`

`safe-ngram` learns local boundary contexts from the train split:

1. For each candidate boundary, encode left/right lowercase ASCII context into a
   packed `u64`.
2. Count how often that context appears at a true break and at a non-break.
3. Keep contexts selected by a conservative rule:
   - `nN`: allow at most `N` observed negatives.
   - `pXX`: require empirical precision at least `XX%`.
   - `wXX`: require Wilson 95% lower-bound precision at least `XX%`.
4. At prediction time, emit a break if any selected context matches.

The method also supports an optional learned veto layer:

```text
safe-ngram-<add-rule>-veto-<veto-rule>
```

The current best production candidate uses local raw character n-grams for both
add and veto rules:

```text
safe-ngram-multi-s1-p65-veto-multi-s1-n0
```

Two generalized feature families are implemented for research variants:

- `mixcv`: adds consonant/vowel shape rules alongside raw character rules.
- `mixson`: adds coarse sonority class rules alongside raw character rules.

This is a high-precision finite-state-like rule filter. It is deliberately
simple and fast: no CRF inference, no pattern parser, and no heap allocation on
the hot path.

Run:

```bash
ITERATIONS=5 INIT_ITERATIONS=1 bash scripts/run_safe_ngram_matrix.sh
```

Compile the current best candidate and evaluate it without train-time rule
counting in method initialization:

```bash
bash scripts/run_safe_ngram_compiled.sh
```

Reports:

```text
target/hyphlab-reports/research/moby_en_us_safe_ngram_dev/compare.md
target/hyphlab-reports/research/moby_en_us_safe_ngram/compare.md
target/hyphlab-reports/research/moby_en_us_safe_ngram_compiled/compare.md
```

Current split-based Moby test highlights:

| method | precision | recall | f1 | f0.5 | exact | serious_error | steady ns/word |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher | 0.910055 | 0.736375 | 0.814055 | 0.869060 | 0.568720 | 0.123005 | 427.829 |
| safe-ngram-multi-s1-p65-veto-multi-s1-n0 | 0.969147 | 0.796901 | 0.874624 | 0.928988 | 0.675274 | 0.045705 | 343.404 |
| safe-ngram-multi-s1-p50-veto-multi-s1-n0 | 0.960168 | 0.805252 | 0.875913 | 0.924593 | 0.676363 | 0.059814 | 360.629 |
| safe-ngram-mixcv-multi-s1-p85-veto-multi-s1-n0 | 0.964955 | 0.808380 | 0.879755 | 0.928969 | 0.686550 | 0.052841 | 777.501 |
| safe-ngram-mixson-multi-s1-p90-veto-multi-s1-n0 | 0.934234 | 0.869565 | 0.900740 | 0.920542 | 0.734379 | 0.108732 | 755.811 |

Interpretation:

- `safe-ngram-multi-s1-p65-veto-multi-s1-n0` is the production-oriented winner:
  it beats `hypher` on every tracked accuracy metric, cuts `serious_error` by
  roughly 63%, and is faster in steady-state release benchmarks.
- `safe-ngram-multi-s1-p50-veto-multi-s1-n0` is the high-recall raw-ngram
  variant. It slightly improves F1 over `p65` but gives back precision and
  `serious_error`.
- `safe-ngram-mixcv-multi-s1-p85-veto-multi-s1-n0` is the best C/V-shape
  variant. It improves recall and F1, but its extra specs are slower.
- `safe-ngram-mixson-multi-s1-p90-veto-multi-s1-n0` is the best high-recall
  sonority variant. It is useful for research, but not the current production
  choice because `serious_error` and runtime are higher than the raw candidate.
- `hypher-safe-add-*-veto-*` variants are useful diagnostic hybrids: they show
  that adding high-confidence train contexts can recover recall, but they are
  not the fastest production path because they still call `hypher`.

The training-time version reports high init ms because it learns from
`train.jsonl.zst` during method preparation. Use `compile-safe-ngram` and
`safe-ngram-model` to measure the deployable model-load path.

Current 5-fold Moby highlights:

| method | precision | recall | f1 | f0.5 | exact | serious_error | steady ns/word |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher | 0.912083 | 0.738206 | 0.815984 | 0.871049 | 0.562147 | 0.122637 | 444.758 |
| safe-ngram-multi-s1-p65-veto-multi-s1-n0 | 0.969670 | 0.802939 | 0.878463 | 0.931005 | 0.676490 | 0.045928 | 374.285 |
| safe-ngram-multi-s1-p50-veto-multi-s1-n0 | 0.960177 | 0.810677 | 0.879116 | 0.926022 | 0.676064 | 0.060988 | 395.537 |
| safe-ngram-mixcv-multi-s1-p85-veto-multi-s1-n0 | 0.965888 | 0.813335 | 0.883071 | 0.930964 | 0.686144 | 0.052847 | 808.107 |
| safe-ngram-mixson-multi-s1-p90-veto-multi-s1-n0 | 0.936450 | 0.872977 | 0.903600 | 0.923027 | 0.733602 | 0.107507 | 796.120 |

This keeps `test.jsonl.zst` as the final holdout, while the 5-fold report is a
stability check across the full Moby gold file. For a paper-like final claim,
select a method family from the 5-fold summary, freeze the method string, retrain
on `train.jsonl.zst`, and report the held-out test split once.

Use the dev report for variant selection and the test report only for final
claims.

## Next Candidates

### 1. Liang-Margin Filter

Extend the Liang evaluator to compute confidence features during the same trie
walk:

- odd score at boundary
- margin to nearest even veto
- longest matching pattern
- number of supporting odd patterns
- boundary position and word length bucket

Tune a threshold on dev. This should preserve more recall than `safe-ngram`
while greatly reducing false positives.

### 2. Pattern-Pruned Liang

Start from TeX/LibreOffice patterns and remove or down-rank rules that cause
train false positives. Optimize a cost-sensitive loss:

```text
loss = FN + lambda_fp * FP + lambda_serious * word_has_any_FP
```

Use dev to select the pruning strength.

### 3. Veto Layer

Add a compact forbidden-boundary layer on top of any base method:

```text
predict = base(word) - forbidden_contexts(word)
```

The layer may store:

- exact `(word_hash, boundary)` forbids from train
- generalized local context forbids
- suffix/prefix class forbids

This targets `serious_error` directly.

### 4. Cost-Sensitive Pattern Miner

Mine positive and negative Liang-like patterns from train substrings. Select
patterns by beam search against the same cost-sensitive loss. The production
artifact should be a compact trie, not a large model.

### 5. Calibrated Boundary Scorer

Use CRF/logistic scoring with Liang and n-gram features as an offline scorer.
Then distill accepted high-confidence boundaries into a compact `safe-ngram` or
pattern artifact. This keeps runtime fast while allowing richer training.

### 6. Distilled Production Automaton

After a richer method finds good rules, compile the accepted rules into a
binary automaton:

- packed transitions
- no runtime pattern parsing
- ASCII fast path
- optional compressed exception/veto tables

This is the most likely path to beating `hypher` on speed in a production
setting.

## Reporting Rules

- Any method trained, pruned, tuned, calibrated, or distilled from Moby must use
  `data/splits/moby_en_us/train.jsonl.zst` and `dev.jsonl.zst` before final
  `test.jsonl.zst` evaluation.
- Full-gold reports are only for fixed external baselines.
- Report precision, recall, F0.5, serious_error, FP/100k, steady ns/word, and
  init ms together.
