## Evaluation Data

Rows have mixed evaluation metadata.

- gold: `data/challenges/typeset_no_break/tr.jsonl.zst`
- locale: `tr`
- patterns: `data/patterns/tex-hyphen/tex/hyph-tr.tex`, `none`
- ambiguous_policy: `exclude`

| method | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | steady words/sec | init ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher-0.1.7-tr | 81 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.049383 | 0.950617 | 41715.976 | 246.451 | 4057608.015 | 0.001 |
| liang:hyph-tr | 81 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.074074 | 0.925926 | 42528.736 | 731.985 | 1366147.407 | 0.076 |
| guarded:safe-ngram-unicode-mixcv-2x2-s1-p70:tr_typeset.jsonl:r3850:v0:n16760:model:wiktextract_tr_typeset:fragments+case+proper | 81 | 1.000000 | 0.000000 | 0.000000 | 0.000000 | 1.000000 | 0.000000 | 0.000 | 162.695 | 6146458.729 | 0.071 |
