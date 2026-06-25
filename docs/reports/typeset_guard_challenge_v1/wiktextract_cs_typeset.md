## Evaluation Data

Rows have mixed evaluation metadata.

- gold: `data/challenges/typeset_no_break/cs.jsonl.zst`
- locale: `cs`
- patterns: `data/patterns/tex-hyphen/tex/hyph-cs.tex`, `none`
- ambiguous_policy: `exclude`

| method | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | steady words/sec | init ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher-0.1.7-cs | 67 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.149254 | 0.850746 | 45126.354 | 255.274 | 3917363.509 | 0.001 |
| liang:hyph-cs | 67 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.238806 | 0.761194 | 41552.511 | 713.259 | 1402015.638 | 0.472 |
| guarded:safe-ngram-unicode-2x2-s1-p50:cs_typeset.jsonl:r10611:v0:n56103:model:wiktextract_cs_typeset:fragments+case+proper | 67 | 1.000000 | 0.000000 | 0.000000 | 0.000000 | 1.000000 | 0.000000 | 0.000 | 135.485 | 7380886.808 | 0.092 |
