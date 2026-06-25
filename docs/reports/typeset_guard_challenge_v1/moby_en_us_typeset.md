## Evaluation Data

Rows have mixed evaluation metadata.

- gold: `data/challenges/typeset_no_break/en_US.jsonl.zst`
- locale: `en-US`
- patterns: `data/patterns/tex-hyphen/tex/hyph-en-us.tex`, `none`
- ambiguous_policy: `exclude`

| method | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | steady words/sec | init ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher-0.1.7-en | 90 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.400000 | 0.600000 | 21895.425 | 271.444 | 3683995.088 | 0.001 |
| liang:hyph-en-us | 90 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.400000 | 0.600000 | 21895.425 | 812.398 | 1230923.422 | 0.745 |
| guarded:safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85:moby_en_us_typeset.jsonl:r49478:v6947:n174891:model:moby_en_us_typeset:fragments+case+proper | 90 | 1.000000 | 0.000000 | 0.000000 | 0.000000 | 1.000000 | 0.000000 | 0.000 | 70.259 | 14232984.467 | 0.295 |
