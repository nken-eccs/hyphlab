## Evaluation Data

Rows have mixed evaluation metadata.

- gold: `data/challenges/typeset_no_break/it.jsonl.zst`
- locale: `it`
- patterns: `data/patterns/tex-hyphen/tex/hyph-it.tex`, `none`
- ambiguous_policy: `exclude`

| method | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | steady words/sec | init ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher-0.1.7-it | 79 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.202532 | 0.797468 | 37500.000 | 198.766 | 5031046.012 | 0.000 |
| liang:hyph-it | 79 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.341772 | 0.658228 | 35436.893 | 604.262 | 1654911.904 | 0.050 |
| guarded:safe-ngram-unicode-2x2-s1-p50:it_typeset.jsonl:r2736:v0:n4242:model:wiktextract_it_typeset:fragments+case+proper | 79 | 1.000000 | 0.000000 | 0.000000 | 0.000000 | 1.000000 | 0.000000 | 0.000 | 39.209 | 25504439.064 | 0.056 |
