# hyph-bench 5-Fold Evaluation

Protocol:

- The selected method per dataset is fixed before this run.
- Each dataset is evaluated with deterministic grouped `5`-fold cross-validation.
- For each fold, trainable methods are trained only on that fold train file and evaluated on that fold test file.
- Hypher and Liang are evaluated on the same fold test files when supported for the dataset.
- LibreOffice hyphen dictionaries are included as an additional Liang/libhyphen pattern baseline when available.
- Ambiguous records use the default `exclude` policy.
- Runtime uses `target/release/hyphlab`, `50` steady-state iterations, `10` init iterations, and `2` init warmup.
- Runtime values are machine-local and should be used for within-run comparison unless hardware details are documented separately.

Selected methods:

| dataset | report slug | recipe |
| --- | --- | --- |
| `hyph_bench_cs_cstenten` | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` |
| `hyph_bench_cs_ujc` | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` |
| `hyph_bench_cssk_cshyphen` | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` |
| `hyph_bench_de_wortliste` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |

Mean and sample standard deviation across folds:

| dataset | method | folds | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | ns/word |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `hyph_bench_cs_cstenten` | `hypher` | 5 | 121299.800000 (sd 78.435961) | 0.998600 (sd 0.000043) | 0.999284 (sd 0.000057) | 0.998942 (sd 0.000046) | 0.998736 (sd 0.000043) | 0.994440 (sd 0.000227) | 0.003754 (sd 0.000109) | 64.640864 (sd 1.946427) | 456.491833 (sd 8.915291) |
| `hyph_bench_cs_cstenten` | `liang_tex` | 5 | 121299.800000 (sd 78.435961) | 0.970817 (sd 0.000170) | 0.889234 (sd 0.000566) | 0.928236 (sd 0.000281) | 0.953324 (sd 0.000136) | 0.757698 (sd 0.000663) | 0.055476 (sd 0.000356) | 1153.043194 (sd 6.949456) | 1098.155892 (sd 13.682371) |
| `hyph_bench_cs_cstenten` | `liang_libreoffice` | 5 | 121299.800000 (sd 78.435961) | 0.970819 (sd 0.000170) | 0.889236 (sd 0.000565) | 0.928238 (sd 0.000281) | 0.953326 (sd 0.000135) | 0.757700 (sd 0.000663) | 0.055476 (sd 0.000356) | 1152.977356 (sd 6.922422) | 1107.054518 (sd 26.541875) |
| `hyph_bench_cs_cstenten` | `guarded_ngram` | 5 | 121299.800000 (sd 78.435961) | 0.979336 (sd 0.000438) | 0.977392 (sd 0.000286) | 0.978363 (sd 0.000219) | 0.978946 (sd 0.000336) | 0.914547 (sd 0.000949) | 0.043431 (sd 0.000928) | 889.600851 (sd 19.371164) | 105.979845 (sd 0.853498) |
| `hyph_bench_cs_ujc` | `hypher` | 5 | 21028.800000 (sd 36.765473) | 0.967355 (sd 0.000463) | 0.991854 (sd 0.000401) | 0.979451 (sd 0.000199) | 0.972157 (sd 0.000334) | 0.904541 (sd 0.001241) | 0.080042 (sd 0.001231) | 1468.577905 (sd 21.687260) | 450.700181 (sd 12.716638) |
| `hyph_bench_cs_ujc` | `liang_tex` | 5 | 21028.800000 (sd 36.765473) | 0.955430 (sd 0.001464) | 0.897876 (sd 0.001930) | 0.925759 (sd 0.001538) | 0.943336 (sd 0.001437) | 0.771647 (sd 0.004102) | 0.079680 (sd 0.002328) | 1744.906192 (sd 55.890105) | 1047.057430 (sd 12.858227) |
| `hyph_bench_cs_ujc` | `liang_libreoffice` | 5 | 21028.800000 (sd 36.765473) | 0.955435 (sd 0.001461) | 0.897881 (sd 0.001926) | 0.925764 (sd 0.001533) | 0.943341 (sd 0.001433) | 0.771647 (sd 0.004110) | 0.079680 (sd 0.002328) | 1744.711255 (sd 55.786412) | 1053.818899 (sd 18.860352) |
| `hyph_bench_cs_ujc` | `guarded_ngram` | 5 | 21028.800000 (sd 36.765473) | 0.960459 (sd 0.000847) | 0.956348 (sd 0.001298) | 0.958399 (sd 0.000953) | 0.959634 (sd 0.000850) | 0.856531 (sd 0.003138) | 0.076125 (sd 0.001820) | 1640.286997 (sd 36.563905) | 118.618289 (sd 1.712561) |
| `hyph_bench_cssk_cshyphen` | `hypher` | 5 | 167096.400000 (sd 11.738825) | 0.933266 (sd 0.000093) | 0.998710 (sd 0.000035) | 0.964880 (sd 0.000046) | 0.945659 (sd 0.000074) | 0.817754 (sd 0.000257) | 0.180307 (sd 0.000240) | 2771.897641 (sd 5.060399) | 457.003144 (sd 5.527953) |
| `hyph_bench_cssk_cshyphen` | `liang_tex` | 5 | 167096.400000 (sd 11.738825) | 0.910423 (sd 0.000305) | 0.858038 (sd 0.000393) | 0.883455 (sd 0.000202) | 0.899441 (sd 0.000220) | 0.654467 (sd 0.000776) | 0.156871 (sd 0.000658) | 3022.684612 (sd 11.746045) | 1127.436011 (sd 25.540768) |
| `hyph_bench_cssk_cshyphen` | `liang_libreoffice` | 5 | 167096.400000 (sd 11.738825) | 0.910423 (sd 0.000305) | 0.858038 (sd 0.000393) | 0.883455 (sd 0.000202) | 0.899441 (sd 0.000220) | 0.654467 (sd 0.000776) | 0.156871 (sd 0.000658) | 3022.684612 (sd 11.746045) | 1116.581718 (sd 15.413966) |
| `hyph_bench_cssk_cshyphen` | `guarded_ngram` | 5 | 167096.400000 (sd 11.738825) | 0.973446 (sd 0.000377) | 0.966756 (sd 0.000414) | 0.970090 (sd 0.000228) | 0.972101 (sd 0.000283) | 0.894864 (sd 0.000728) | 0.050877 (sd 0.000639) | 944.195393 (sd 13.966889) | 123.222234 (sd 3.527201) |
| `hyph_bench_de_wortliste` | `hypher` | 5 | 120994.000000 (sd 0.000000) | 0.963553 (sd 0.000313) | 0.970578 (sd 0.000269) | 0.967053 (sd 0.000282) | 0.964950 (sd 0.000299) | 0.894808 (sd 0.000906) | 0.104832 (sd 0.000910) | 1121.403098 (sd 9.340213) | 753.118109 (sd 2.283804) |
| `hyph_bench_de_wortliste` | `liang_tex` | 5 | 120994.000000 (sd 0.000000) | 0.965362 (sd 0.000351) | 0.968825 (sd 0.000282) | 0.967091 (sd 0.000311) | 0.966053 (sd 0.000334) | 0.894892 (sd 0.000897) | 0.093905 (sd 0.000979) | 1115.215222 (sd 11.101236) | 1748.742028 (sd 30.218689) |
| `hyph_bench_de_wortliste` | `liang_libreoffice` | 5 | 120994.000000 (sd 0.000000) | 0.902556 (sd 0.000631) | 0.942426 (sd 0.000315) | 0.922060 (sd 0.000453) | 0.910258 (sd 0.000559) | 0.729059 (sd 0.001563) | 0.238843 (sd 0.001735) | 3264.217974 (sd 22.267085) | 1681.162100 (sd 22.049399) |
| `hyph_bench_de_wortliste` | `guarded_ngram` | 5 | 120994.000000 (sd 0.000000) | 0.986801 (sd 0.000411) | 0.950581 (sd 0.000310) | 0.968352 (sd 0.000152) | 0.979338 (sd 0.000285) | 0.856201 (sd 0.000813) | 0.033415 (sd 0.000971) | 407.898745 (sd 12.926378) | 104.559620 (sd 4.482269) |

Per-dataset fold summaries are written next to each dataset report.
