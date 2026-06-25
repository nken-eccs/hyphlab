## Evaluation Data

Rows have mixed evaluation metadata.

- gold: `data/challenges/typeset_no_break/es.jsonl.zst`
- locale: `es`
- patterns: `data/patterns/tex-hyphen/tex/hyph-es.tex`, `none`
- ambiguous_policy: `exclude`

| method | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | steady words/sec | init ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher-0.1.7-es | 81 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.098765 | 0.901235 | 36391.437 | 258.920 | 3862200.501 | 0.000 |
| liang:hyph-es | 81 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.172840 | 0.827160 | 39043.825 | 704.177 | 1420097.429 | 0.731 |
| guarded:safe-ngram-unicode-3x2-s1-p60:es_typeset.jsonl:r41321:v0:n804472:model:wiktextract_es_typeset:fragments+case+proper | 81 | 1.000000 | 0.000000 | 0.000000 | 0.000000 | 1.000000 | 0.000000 | 0.000 | 148.364 | 6740170.585 | 0.245 |
