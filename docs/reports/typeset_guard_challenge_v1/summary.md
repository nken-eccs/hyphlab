## Typeset Guard Challenge Summary

This challenge contains only no-break gold labels for proper names, MixedCase tokens, and ALLCAPS tokens. Use `exact`, `no_break_accuracy`, `serious_error`, and `fp/100k` as the meaningful metrics here; precision, recall, and F1 have no positive gold boundaries.

| method | datasets | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | init ms | delta f0.5 | delta recall | delta serious | delta ns/word |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| guarded_ngram | 8 | 75.500000 (sd 11.722993) | 1.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 1.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 162.052 (sd 168.810) | 0.220 (sd 0.159) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | -0.857017 (sd 0.114763) | -99.602 (sd 152.537) |
| hypher | 8 | 75.500000 (sd 11.722993) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.142983 (sd 0.114763) | 0.857017 (sd 0.114763) | 37784.562693 (sd 6993.523943) | 261.655 (sd 31.645) | 0.000 (sd 0.000) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.000 (sd 0.000) |
| liang_tex | 8 | 75.500000 (sd 11.722993) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | 0.212673 (sd 0.108948) | 0.787327 (sd 0.108948) | 37692.033520 (sd 6729.083302) | 741.985 (sd 69.972) | 1.513 (sd 2.248) | 0.000000 (sd 0.000000) | 0.000000 (sd 0.000000) | -0.069690 (sd 0.045805) | 480.330 (sd 46.988) |

Deltas are paired against the `hypher` row in the same dataset. Higher is better except `serious_error`, `fp/100k`, `steady ns/word`, and `init ms`.
