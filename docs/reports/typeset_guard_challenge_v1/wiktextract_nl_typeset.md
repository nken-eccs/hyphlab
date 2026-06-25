## Evaluation Data

Rows have mixed evaluation metadata.

- gold: `data/challenges/typeset_no_break/nl.jsonl.zst`
- locale: `nl`
- patterns: `data/patterns/tex-hyphen/tex/hyph-nl.tex`, `none`
- ambiguous_policy: `exclude`

| method | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | steady words/sec | init ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hypher-0.1.7-nl | 72 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.069444 | 0.930556 | 41744.548 | 289.491 | 3454344.126 | 0.000 |
| liang:hyph-nl | 72 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.166667 | 0.833333 | 41106.719 | 768.461 | 1301302.495 | 2.099 |
| guarded:safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80:nl_typeset.jsonl:r73220:v6443:n618808:model:wiktextract_nl_typeset:fragments+case+proper | 72 | 1.000000 | 0.000000 | 0.000000 | 0.000000 | 1.000000 | 0.000000 | 0.000 | 81.829 | 12220664.465 | 0.423 |
