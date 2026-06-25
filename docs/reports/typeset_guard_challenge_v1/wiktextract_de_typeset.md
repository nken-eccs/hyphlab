## Evaluation Data

Rows have mixed evaluation metadata.

- gold: `data/challenges/typeset_no_break/de.jsonl.zst`
- locale: `de`
- patterns: `data/patterns/tex-hyphen/tex/hyph-de-1996.tex`, `none`
- ambiguous_policy: `exclude`

| method | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | steady words/sec | init ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher-0.1.7-de | 82 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.097561 | 0.902439 | 39193.084 | 268.069 | 3730379.116 | 0.000 |
| liang:hyph-de-1996 | 82 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.134146 | 0.865854 | 39852.399 | 814.878 | 1227177.492 | 6.840 |
| guarded:safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80:de_typeset.jsonl:r68404:v7689:n961318:model:wiktextract_de_typeset:fragments+case+proper | 82 | 1.000000 | 0.000000 | 0.000000 | 0.000000 | 1.000000 | 0.000000 | 0.000 | 91.910 | 10880151.154 | 0.453 |
