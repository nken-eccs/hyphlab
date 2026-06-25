## Evaluation Data

Rows have mixed evaluation metadata.

- gold: `data/challenges/typeset_no_break/ru.jsonl.zst`
- locale: `ru`
- patterns: `data/patterns/tex-hyphen/tex/hyph-ru.tex`, `none`
- ambiguous_policy: `exclude`

| method | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | steady words/sec | init ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher-0.1.7-ru | 52 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.076923 | 0.923077 | 38709.677 | 304.824 | 3280583.237 | 0.000 |
| liang:hyph-ru | 52 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.173077 | 0.826923 | 40119.760 | 786.458 | 1271522.972 | 1.091 |
| guarded:safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80:ru_cyrl_trusted_dedup_typeset.jsonl:r7213:v602:n8135:model:wiktextract_ru_cyrl_trusted_dedup_typeset:fragments+case+proper | 52 | 1.000000 | 0.000000 | 0.000000 | 0.000000 | 1.000000 | 0.000000 | 0.000 | 566.667 | 1764705.084 | 0.125 |
