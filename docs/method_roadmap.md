# Method Development

Goal: develop fast hyphenation methods that improve on Hypher under held-out
evaluation. The main metrics are precision, recall, F1, F0.5, exact word
accuracy, `serious_error`, false positives per 100k words, initialization time,
and steady-state ns/word.

## Current Baseline To Beat

Use the multilingual 5-fold report as the main comparison:

```text
docs/reports/multilingual_5fold_v1/summary.md
```

Use the hyph-bench report as an additional external-corpus check:

```text
docs/reports/hyph_bench_5fold_v1/summary.md
```

Use the curated Moby typesetting report when the target is en-US line-break
behavior rather than preserving every Moby syllable boundary:

```text
docs/reports/moby_typeset_5fold_v1/summary.md
```

Use the curated Wiktextract typesetting report for multilingual line-break
behavior:

```text
docs/reports/wiktextract_typeset_5fold_v1/summary.md
```

The selected learned methods are:

| language/data | selected method family |
| --- | --- |
| en-US Moby | Guarded N-gram |
| en-US Moby typesetting | Guarded N-gram |
| Czech, German, Spanish, Dutch, Russian, Turkish Wiktextract | Guarded N-gram |
| Italian Wiktextract | Italian onset-syllable model |
| Wiktextract typesetting derivatives | Guarded N-gram |
| Czech/German hyph-bench | Guarded N-gram |

Detailed recipes live in [`guarded_ngram.md`](guarded_ngram.md), and full-corpus
runtime model inventory lives in
[`../models/guarded_ngram/v1/README.md`](../models/guarded_ngram/v1/README.md).

## Guarded N-gram Summary

Guarded N-gram learns local boundary contexts from labeled training records.
Each candidate boundary gets one or more compact feature keys. A key is kept as
an add rule when it has enough positive evidence and passes the configured
precision or error threshold. Optional veto rules are learned only over
boundaries that an add rule would propose.

Runtime logic:

```text
predict boundary when add_rule_hits && !veto_rule_hits
```

When add and veto use the same feature definition, they can be collapsed into a
single effective rule set by subtracting veto keys from add keys. Current
selected recipes usually use different context sizes for add and veto, so the
two-layer form is kept.

## Development Loop

1. Choose the target dataset and evaluation track from
   [`evaluation.md`](evaluation.md).
2. Train or tune only on train/dev folds.
3. Freeze the method string or model format.
4. Run deterministic 5-fold evaluation.
5. Compare against Hypher and Liang on the same fold test files.
6. Inspect both accuracy and speed before promoting a recipe.

Useful commands:

```bash
bash scripts/run_multilingual_safe_ngram_tuning.sh
bash scripts/run_multilingual_5fold_evaluation.sh
bash scripts/run_baseline_matrix.sh
```

## Candidate Directions

### Pattern-Guided Guarded N-gram

Use Liang or LibreOffice patterns to propose high-recall candidate boundaries,
then learn a compact Guarded N-gram veto layer to reduce false positives and
`serious_error`.

### Pruned Pattern Tables

Start from TeX or LibreOffice patterns and remove or down-rank patterns that
cause train-fold false positives. Optimize a cost-sensitive loss:

```text
loss = FN + lambda_fp * FP + lambda_serious * word_has_any_FP
```

Keep the runtime representation close to a compact pattern trie.

### Distilled Boundary Tables

Train a richer offline scorer with n-gram, shape, Liang, suffix, and position
features. Distill only high-confidence decisions into compact add/veto rule
tables for production speed.

### Language-Specific Structure

Use simple linguistic constraints where they reduce error without adding much
runtime cost:

- vowel nucleus limits
- onset/coda shape filters
- suffix and prefix guards
- script-specific normalization
- syllable onset models for languages where this is a better fit than generic
  boundary n-grams

### Model Format And Runtime

Promote candidates only when they can be loaded from a compact runtime model:

- binary rule table for Guarded N-gram
- JSON or compressed JSON only when the model is small enough
- dense bitsets for small key spaces
- ASCII or simple-char fast paths before grapheme fallback

## Reporting Rules

- Use full-gold reports only for fixed methods that do not train or tune on the
  evaluated labels.
- Use split or 5-fold reports for learned, tuned, pruned, or distilled methods.
- Keep `identity-oracle` and dictionary-oracle rows out of claim tables.
- Report precision, recall, F0.5, exact accuracy, `serious_error`, FP/100k,
  steady ns/word, and init ms together.
- Link to maintained report summaries instead of copying large metric tables
  into planning documents.
