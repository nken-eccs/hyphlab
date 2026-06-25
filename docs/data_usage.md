# Data And Model Usage

This page maps each data and model file to its role. The main rule is: runtime
models are trained on full normalized corpora, while accuracy claims for
trainable methods use held-out folds.

## Runtime Assets

| asset | location | use when |
| --- | --- | --- |
| Reusable Guarded N-gram models | `models/guarded_ngram/v1/` | You want a model you can run without a training step. |
| Reusable manifests | `manifests/guarded_ngram/v1/` | You want to run the reusable models in a matrix. |

The reusable model inventory lives in
[`models/guarded_ngram/v1/README.md`](../models/guarded_ngram/v1/README.md).
Use it for exact model files, recipes, source corpora, and manifests.

## Gold And Source Data

| asset | location | use when |
| --- | --- | --- |
| Normalized gold corpora | `data/gold/` | You want to train a new model, make splits, or run fixed baselines. |
| Curated Moby typesetting corpus | `data/gold/moby_en_us_typeset.jsonl.zst` | You want English typesetting-oriented training or evaluation. |
| Curated Wiktextract typesetting corpora | `data/gold/wiktextract/*_typeset.jsonl.zst` | You want multilingual typesetting-oriented training or evaluation. |
| TeX, LibreOffice, Hunspell resources | `data/patterns/`, `external/` | You want Liang/libhyphen-style baselines or source comparison material. |

## Reports

| report | location | use when |
| --- | --- | --- |
| Main 5-fold report | `docs/reports/multilingual_5fold_v1/` | You want a fair comparison between Hypher, Liang, and the selected learned method. |
| Typesetting policy report | `docs/reports/typeset_policy_5fold_v1/` | You want the current reader-facing comparison. |
| Guard challenge report | `docs/reports/typeset_guard_challenge_v1/` | You want to check runtime guard behavior. |
| Moby typesetting curation | `docs/reports/moby_typeset_5fold_v1/` | You want en-US-only curation details. |
| Wiktextract typesetting curation | `docs/reports/wiktextract_typeset_5fold_v1/` | You want multilingual curation details. |
| Additional hyph-bench report | `docs/reports/hyph_bench_5fold_v1/` | You want an external-corpus stress check. |
| Generated local reports | `target/hyphlab-reports/` | You are running new experiments locally. |

## Evaluation Data

Use `scripts/run_multilingual_5fold_evaluation.sh` for the main comparison.
For each fold, the script:

1. Splits one normalized gold corpus into train and test folds.
2. Trains the selected learned model only on the train fold.
3. Evaluates Hypher, Liang, and the selected method on the same held-out test
   fold.
4. Writes aggregate metrics and fold-level rows under `docs/reports/`.

The `docs/reports/multilingual_5fold_v1/` report uses Moby and filtered
Wiktextract / Kaikki records as gold data. TeX pattern files are used only by
the Liang baseline in that report.

The `docs/reports/typeset_policy_5fold_v1/` report uses curated Moby and
Wiktextract / Kaikki derivatives with the current guard policies. The reusable
`*-typeset` models use the matching policies at runtime.

The `docs/reports/typeset_guard_challenge_v1/` report uses
`data/challenges/typeset_no_break/*.jsonl.zst`. Those files contain only
no-break labels for proper names and case-sensitive tokens. In that report,
`exact`, `no_break_accuracy`, `serious_error`, and `fp/100k` are the useful
metrics; precision/recall/F1 have no positive gold boundaries.

The `docs/reports/moby_typeset_5fold_v1/` and
`docs/reports/wiktextract_typeset_5fold_v1/` reports contain curation details
for the English-only and Wiktextract-only parts of the policy.

The `docs/reports/hyph_bench_5fold_v1/` report uses selected hyph-bench
Czech/German WLHAMB-derived gold files. It can also include LibreOffice pattern
files as an additional Liang/libhyphen-style baseline.

## Choosing A Data Source

Use Moby for en-US dictionary-style English experiments that should stay close
to the source syllable labels. Use `moby_en_us_typeset` when the target is
reader-facing line-break behavior: it preserves useful break opportunities
while also filtering breaks that create unsuitable semantic fragments or poor
typesetting outcomes.

Use Wiktextract / Kaikki when you want multilingual lexical entries in the
currently supported language set. Russian experiments should usually use
`ru_cyrl_trusted_dedup.jsonl.zst`, because it removes Cyrillic duplicates and
filters likely missing hyphenation annotations.

Use Wiktextract typesetting derivatives when the predicted break may become a
visible line break. They keep the source lexical labels where suitable, then
remove boundary positions that produce unsuitable short, nonalphabetic, or
language-specific fragments, or split configured protected tokens.

Use hyph-bench when you want an external benchmark-style check for languages
that are already imported and supported by the current adapters.

Use TeX, LibreOffice, and Hunspell hyphen resources when you want pattern
baselines or reference material. They are not used as gold labels for the main
multilingual 5-fold report.
