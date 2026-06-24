# Data And Model Usage

This page maps each data and model file to its role. The main rule is: runtime
models are trained on full normalized corpora, while accuracy claims for
trainable methods use held-out folds.

## Quick Map

| asset | location | role | use when |
| --- | --- | --- | --- |
| Reusable Guarded N-gram models | `models/guarded_ngram/v1/` | Full-corpus runtime models | You want a model you can run without a training step. |
| Reusable manifests | `manifests/guarded_ngram/v1/` | Matrix entries pointing at reusable models | You want to run the reusable models in a matrix. |
| Main 5-fold report | `docs/reports/multilingual_5fold_v1/` | Held-out multilingual comparison | You want a fair comparison between Hypher, Liang, and the selected learned method. |
| Moby typesetting report | `docs/reports/moby_typeset_5fold_v1/` | Held-out comparison on curated Moby typesetting labels | You want en-US typesetting-oriented results. |
| Wiktextract typesetting report | `docs/reports/wiktextract_typeset_5fold_v1/` | Held-out comparison on curated multilingual labels | You want multilingual typesetting-oriented results. |
| Additional hyph-bench report | `docs/reports/hyph_bench_5fold_v1/` | Held-out comparison on hyph-bench Czech/German data | You want an external-corpus stress check. |
| Generated reports | `target/hyphlab-reports/` | Experiment output | You are running new experiments locally. |
| Normalized gold corpora | `data/gold/` | Training and evaluation labels | You want to train a new model, make splits, or run fixed baselines. |
| Curated Moby typesetting corpus | `data/gold/moby_en_us_typeset.jsonl.zst` | Derived en-US labels with unsafe line fragments removed | You want English typesetting-oriented training or evaluation. |
| Curated Wiktextract typesetting corpora | `data/gold/wiktextract/*_typeset.jsonl.zst` | Derived multilingual labels with typesetting guards | You want multilingual typesetting-oriented training or evaluation. |
| TeX, LibreOffice, Hunspell resources | `data/patterns/`, `external/` | Pattern/reference resources | You want Liang/libhyphen-style baselines or source comparison material. |

## Model Training Sources

These models are trained from the full normalized corpus named below. They are
convenient runtime models, not independent test results.

| model | locale | trained from | model type |
| --- | --- | --- | --- |
| `models/guarded_ngram/v1/moby_en_us.bin` | `en-US` | `data/gold/moby_en_us.jsonl.zst` | Guarded N-gram |
| `models/guarded_ngram/v1/moby_en_us_typeset.bin` | `en-US` | `data/gold/moby_en_us_typeset.jsonl.zst` | Guarded N-gram with fragment guard |
| `models/guarded_ngram/v1/wiktextract_cs.bin` | `cs` | `data/gold/wiktextract/cs.jsonl.zst` | Guarded N-gram |
| `models/guarded_ngram/v1/wiktextract_cs_typeset.bin` | `cs` | `data/gold/wiktextract/cs_typeset.jsonl.zst` | Guarded N-gram with fragment guard |
| `models/guarded_ngram/v1/wiktextract_de.bin` | `de` | `data/gold/wiktextract/de.jsonl.zst` | Guarded N-gram |
| `models/guarded_ngram/v1/wiktextract_de_typeset.bin` | `de` | `data/gold/wiktextract/de_typeset.jsonl.zst` | Guarded N-gram with fragment guard |
| `models/guarded_ngram/v1/wiktextract_es.bin` | `es` | `data/gold/wiktextract/es.jsonl.zst` | Guarded N-gram |
| `models/guarded_ngram/v1/wiktextract_es_typeset.bin` | `es` | `data/gold/wiktextract/es_typeset.jsonl.zst` | Guarded N-gram with fragment guard |
| `models/guarded_ngram/v1/wiktextract_it.json` | `it` | `data/gold/wiktextract/it.jsonl.zst` | Italian onset-syllable |
| `models/guarded_ngram/v1/wiktextract_it_typeset.bin` | `it` | `data/gold/wiktextract/it_typeset.jsonl.zst` | Guarded N-gram with fragment guard |
| `models/guarded_ngram/v1/wiktextract_nl.bin` | `nl` | `data/gold/wiktextract/nl.jsonl.zst` | Guarded N-gram |
| `models/guarded_ngram/v1/wiktextract_nl_typeset.bin` | `nl` | `data/gold/wiktextract/nl_typeset.jsonl.zst` | Guarded N-gram with fragment guard |
| `models/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup.bin` | `ru` | `data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst` | Guarded N-gram |
| `models/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup_typeset.bin` | `ru` | `data/gold/wiktextract/ru_cyrl_trusted_dedup_typeset.jsonl.zst` | Guarded N-gram with fragment guard |
| `models/guarded_ngram/v1/wiktextract_tr.bin` | `tr` | `data/gold/wiktextract/tr.jsonl.zst` | Guarded N-gram |
| `models/guarded_ngram/v1/wiktextract_tr_typeset.bin` | `tr` | `data/gold/wiktextract/tr_typeset.jsonl.zst` | Guarded N-gram with fragment guard |

Use `models/guarded_ngram/v1/README.md` for the exact recipes and manifests.

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

The `docs/reports/moby_typeset_5fold_v1/` report uses
`data/gold/moby_en_us_typeset.jsonl.zst`, a curated derivative of Moby for
typesetting-oriented English experiments. The reusable `en-US-typeset` model
uses the same fragment guard at runtime.

The `docs/reports/wiktextract_typeset_5fold_v1/` report uses curated
Wiktextract / Kaikki derivatives for Czech, German, Spanish, Italian, Dutch,
Russian, and Turkish. The reusable `*-typeset` models use the matching fragment
guards at runtime.

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
language-specific fragments.

Use hyph-bench when you want an external benchmark-style check for languages
that are already imported and supported by the current adapters.

Use TeX, LibreOffice, and Hunspell hyphen resources when you want pattern
baselines or reference material. They are not used as gold labels for the main
multilingual 5-fold report.
