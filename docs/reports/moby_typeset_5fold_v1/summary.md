# Moby Typesetting 5-Fold Evaluation

Protocol:

- The selected method per dataset is fixed before this run.
- Each dataset is evaluated with deterministic grouped `5`-fold cross-validation.
- For each fold, trainable methods are trained only on that fold train file and evaluated on that fold test file.
- Hypher and Liang are evaluated on the same fold test files when supported for the dataset.
- Ambiguous records use the default `exclude` policy.
- Runtime uses `target/release/hyphlab`, `50` steady-state iterations, `10` init iterations, and `2` init warmup.
- Runtime values are machine-local and should be used for within-run comparison unless hardware details are documented separately.

Selected methods:

| dataset | report slug | recipe |
| --- | --- | --- |
| `moby_en_us_typeset` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85` |

Gold data:

- `moby_en_us_typeset`: `data/gold/moby_en_us_typeset.jsonl.zst`

Mean and sample standard deviation across folds:

| dataset | method | folds | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | ns/word |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `moby_en_us_typeset` | `hypher` | 5 | 36473.800000 (sd 28.234730) | 0.898820 (sd 0.001231) | 0.742253 (sd 0.001168) | 0.813066 (sd 0.000538) | 0.862435 (sd 0.000743) | 0.561521 (sd 0.002009) | 0.132994 (sd 0.001541) | 2806.769726 (sd 37.782544) | 420.643676 (sd 8.360976) |
| `moby_en_us_typeset` | `liang_tex` | 5 | 36473.800000 (sd 28.234730) | 0.898836 (sd 0.001220) | 0.742258 (sd 0.001173) | 0.813076 (sd 0.000536) | 0.862449 (sd 0.000733) | 0.561532 (sd 0.002016) | 0.132966 (sd 0.001536) | 2806.286907 (sd 37.504259) | 1080.091440 (sd 93.656229) |
| `moby_en_us_typeset` | `guarded_ngram` | 5 | 36473.800000 (sd 28.234730) | 0.954636 (sd 0.000854) | 0.835253 (sd 0.001521) | 0.890962 (sd 0.001025) | 0.928104 (sd 0.000825) | 0.706355 (sd 0.002649) | 0.071821 (sd 0.001231) | 1333.329680 (sd 28.811270) | 84.402693 (sd 2.053617) |

Per-dataset fold summaries are written next to each dataset report.
