# Wiktextract Typesetting 5-Fold Evaluation

Current reader-facing policy numbers are in
[`../typeset_policy_5fold_v1/summary.md`](../typeset_policy_5fold_v1/summary.md).
Use this report for Wiktextract-specific curation context and for comparing the
older Wiktextract-only run.

Protocol:

- The selected method per dataset is fixed before this run.
- Each dataset is evaluated with deterministic grouped `5`-fold cross-validation.
- For each fold, trainable methods are trained only on that fold train file and evaluated on that fold test file.
- Hypher and Liang are evaluated on the same fold test files when supported for the dataset.
- Ambiguous records use the default `exclude` policy.
- Runtime uses `target/release/hyphlab`, `20` steady-state iterations, `5` init iterations, and `2` init warmup.
- Runtime values are machine-local and should be used for within-run comparison unless hardware details are documented separately.

Selected methods:

| dataset | report slug | recipe |
| --- | --- | --- |
| `wiktextract_cs_typeset` | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` |
| `wiktextract_de_typeset` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |
| `wiktextract_es_typeset` | `guarded_ngram` | `safe-ngram-unicode-3x2-s1-p60` |
| `wiktextract_it_typeset` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |
| `wiktextract_nl_typeset` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |
| `wiktextract_ru_cyrl_trusted_dedup_typeset` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80` |
| `wiktextract_tr_typeset` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x2-s1-p70` |

Gold data:

- `wiktextract_cs_typeset`: `data/gold/wiktextract/cs_typeset.jsonl.zst`
- `wiktextract_de_typeset`: `data/gold/wiktextract/de_typeset.jsonl.zst`
- `wiktextract_es_typeset`: `data/gold/wiktextract/es_typeset.jsonl.zst`
- `wiktextract_it_typeset`: `data/gold/wiktextract/it_typeset.jsonl.zst`
- `wiktextract_nl_typeset`: `data/gold/wiktextract/nl_typeset.jsonl.zst`
- `wiktextract_ru_cyrl_trusted_dedup_typeset`: `data/gold/wiktextract/ru_cyrl_trusted_dedup_typeset.jsonl.zst`
- `wiktextract_tr_typeset`: `data/gold/wiktextract/tr_typeset.jsonl.zst`

Mean and sample standard deviation across folds:

| dataset | method | folds | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | ns/word |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `wiktextract_cs_typeset` | `hypher` | 5 | 12447.200000 (sd 54.029622) | 0.704381 (sd 0.003269) | 0.980584 (sd 0.001337) | 0.819841 (sd 0.002346) | 0.746429 (sd 0.002970) | 0.434861 (sd 0.004758) | 0.553762 (sd 0.004710) | 13576.036746 (sd 180.987837) | 405.159759 (sd 6.835590) |
| `wiktextract_cs_typeset` | `liang_tex` | 5 | 12447.200000 (sd 54.029622) | 0.945701 (sd 0.002419) | 0.889442 (sd 0.001955) | 0.916708 (sd 0.001880) | 0.933887 (sd 0.002133) | 0.807832 (sd 0.002360) | 0.065745 (sd 0.002267) | 2094.797606 (sd 93.919196) | 945.976584 (sd 14.992496) |
| `wiktextract_cs_typeset` | `guarded_ngram` | 5 | 12447.200000 (sd 54.029622) | 0.956743 (sd 0.002066) | 0.931966 (sd 0.002708) | 0.944190 (sd 0.001998) | 0.951681 (sd 0.001920) | 0.856709 (sd 0.003530) | 0.061487 (sd 0.002579) | 1728.477031 (sd 84.081055) | 377.778865 (sd 66.356735) |
| `wiktextract_de_typeset` | `hypher` | 5 | 193710.400000 (sd 53.561180) | 0.891825 (sd 0.000296) | 0.958114 (sd 0.000319) | 0.923782 (sd 0.000040) | 0.904338 (sd 0.000189) | 0.681102 (sd 0.000636) | 0.254220 (sd 0.000884) | 3366.189093 (sd 9.084400) | 731.343605 (sd 7.547008) |
| `wiktextract_de_typeset` | `liang_tex` | 5 | 193710.400000 (sd 53.561180) | 0.945550 (sd 0.000213) | 0.958114 (sd 0.000319) | 0.951790 (sd 0.000129) | 0.948036 (sd 0.000144) | 0.817295 (sd 0.000559) | 0.104326 (sd 0.000468) | 1793.281745 (sd 7.405725) | 1780.314328 (sd 137.254390) |
| `wiktextract_de_typeset` | `guarded_ngram` | 5 | 193710.400000 (sd 53.561180) | 0.996254 (sd 0.000088) | 0.975406 (sd 0.000349) | 0.985720 (sd 0.000184) | 0.992013 (sd 0.000101) | 0.932755 (sd 0.000787) | 0.009465 (sd 0.000204) | 119.207006 (sd 2.779674) | 286.905305 (sd 34.354314) |
| `wiktextract_es_typeset` | `hypher` | 5 | 162191.200000 (sd 97.433054) | 0.904009 (sd 0.000619) | 0.933348 (sd 0.000203) | 0.918444 (sd 0.000363) | 0.909728 (sd 0.000515) | 0.598663 (sd 0.001464) | 0.270748 (sd 0.001511) | 3830.967713 (sd 24.914689) | 444.600752 (sd 10.967644) |
| `wiktextract_es_typeset` | `liang_tex` | 5 | 162191.200000 (sd 97.433054) | 0.998774 (sd 0.000036) | 0.933348 (sd 0.000203) | 0.964953 (sd 0.000108) | 0.984965 (sd 0.000052) | 0.829171 (sd 0.000711) | 0.003122 (sd 0.000090) | 51.493508 (sd 1.536564) | 1095.728544 (sd 48.478184) |
| `wiktextract_es_typeset` | `guarded_ngram` | 5 | 162191.200000 (sd 97.433054) | 0.999167 (sd 0.000017) | 0.993017 (sd 0.000147) | 0.996083 (sd 0.000073) | 0.997931 (sd 0.000031) | 0.981099 (sd 0.000403) | 0.002262 (sd 0.000044) | 37.214797 (sd 0.755500) | 301.202686 (sd 10.793323) |
| `wiktextract_it_typeset` | `hypher` | 5 | 911.600000 (sd 12.157302) | 0.684240 (sd 0.004485) | 0.964529 (sd 0.003170) | 0.800545 (sd 0.002081) | 0.726454 (sd 0.003710) | 0.277009 (sd 0.018983) | 0.713761 (sd 0.016728) | 14000.418536 (sd 309.492242) | 273.878661 (sd 12.584172) |
| `wiktextract_it_typeset` | `liang_tex` | 5 | 911.600000 (sd 12.157302) | 0.977601 (sd 0.003731) | 0.964529 (sd 0.003170) | 0.971020 (sd 0.003304) | 0.974958 (sd 0.003529) | 0.927580 (sd 0.007469) | 0.029648 (sd 0.005781) | 845.790812 (sd 140.705254) | 842.094469 (sd 55.689900) |
| `wiktextract_it_typeset` | `guarded_ngram` | 5 | 911.600000 (sd 12.157302) | 0.989746 (sd 0.003213) | 0.529955 (sd 0.015977) | 0.690200 (sd 0.014222) | 0.843288 (sd 0.009722) | 0.439733 (sd 0.023877) | 0.008987 (sd 0.002714) | 209.185239 (sd 61.211125) | 174.917438 (sd 8.559146) |
| `wiktextract_nl_typeset` | `hypher` | 5 | 125476.400000 (sd 36.671515) | 0.877475 (sd 0.000406) | 0.976187 (sd 0.000306) | 0.924203 (sd 0.000359) | 0.895588 (sd 0.000388) | 0.687552 (sd 0.001709) | 0.292801 (sd 0.001457) | 3778.786757 (sd 13.774117) | 625.120226 (sd 11.379926) |
| `wiktextract_nl_typeset` | `liang_tex` | 5 | 125476.400000 (sd 36.671515) | 0.936466 (sd 0.000301) | 0.976187 (sd 0.000306) | 0.955914 (sd 0.000238) | 0.944150 (sd 0.000263) | 0.850850 (sd 0.000985) | 0.125434 (sd 0.000769) | 2055.885239 (sd 10.482513) | 1857.806694 (sd 481.435767) |
| `wiktextract_nl_typeset` | `guarded_ngram` | 5 | 125476.400000 (sd 36.671515) | 0.994807 (sd 0.000160) | 0.969881 (sd 0.000259) | 0.982186 (sd 0.000120) | 0.989720 (sd 0.000113) | 0.921552 (sd 0.000465) | 0.012680 (sd 0.000411) | 157.162176 (sd 4.863583) | 145.520227 (sd 92.083138) |
| `wiktextract_ru_cyrl_trusted_dedup_typeset` | `hypher` | 5 | 3002.400000 (sd 0.547723) | 0.840803 (sd 0.006917) | 0.926240 (sd 0.004776) | 0.881450 (sd 0.005570) | 0.856603 (sd 0.006360) | 0.809685 (sd 0.009372) | 0.163470 (sd 0.008772) | 5894.907976 (sd 272.307281) | 1255.616025 (sd 993.936304) |
| `wiktextract_ru_cyrl_trusted_dedup_typeset` | `liang_tex` | 5 | 3002.400000 (sd 0.547723) | 0.952891 (sd 0.007470) | 0.926240 (sd 0.004776) | 0.939370 (sd 0.005591) | 0.947435 (sd 0.006623) | 0.921529 (sd 0.005819) | 0.043033 (sd 0.007337) | 1889.105708 (sd 301.726044) | 3253.469970 (sd 1931.579587) |
| `wiktextract_ru_cyrl_trusted_dedup_typeset` | `guarded_ngram` | 5 | 3002.400000 (sd 0.547723) | 0.952386 (sd 0.003686) | 0.938416 (sd 0.002879) | 0.945347 (sd 0.002771) | 0.949557 (sd 0.003226) | 0.929123 (sd 0.003116) | 0.042766 (sd 0.003554) | 1934.806956 (sd 142.848198) | 917.952725 (sd 548.445346) |
| `wiktextract_tr_typeset` | `hypher` | 5 | 3686.200000 (sd 25.606640) | 0.650211 (sd 0.009283) | 0.982782 (sd 0.001086) | 0.782597 (sd 0.006523) | 0.697396 (sd 0.008482) | 0.510466 (sd 0.009588) | 0.484053 (sd 0.009273) | 16622.255796 (sd 511.032250) | 334.792457 (sd 4.711374) |
| `wiktextract_tr_typeset` | `liang_tex` | 5 | 3686.200000 (sd 25.606640) | 0.767425 (sd 0.009787) | 0.971518 (sd 0.001782) | 0.857463 (sd 0.005804) | 0.801066 (sd 0.008418) | 0.808624 (sd 0.008304) | 0.182042 (sd 0.008361) | 11133.828494 (sd 494.203045) | 973.129269 (sd 57.195816) |
| `wiktextract_tr_typeset` | `guarded_ngram` | 5 | 3686.200000 (sd 25.606640) | 0.986210 (sd 0.001276) | 0.995617 (sd 0.000979) | 0.990891 (sd 0.000996) | 0.988077 (sd 0.001141) | 0.976394 (sd 0.002521) | 0.022900 (sd 0.002225) | 526.866932 (sd 54.725716) | 366.704252 (sd 16.614125) |

Per-dataset fold summaries are written next to each dataset report.
