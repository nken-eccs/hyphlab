# Wiktextract Typesetting Curation

This curation derives reader-facing line-break labels from normalized
Wiktextract / Kaikki records. It keeps the original lexical hyphenation where
that boundary is suitable for visible typography, then removes boundaries that
would create poor line fragments.

## Policy

- Keep the same record identifiers, words, locales, and variant structure when
  possible.
- Remove breaks that leave fewer than two alphabetic graphemes on the left or
  fewer than three alphabetic graphemes on the right.
- Remove breaks outside alphabetic spans, such as breaks next to apostrophes,
  punctuation, or spaces.
- Remove language-specific fragments listed under
  `data/curation/typeset_fragments/`.
- Collapse identical variants after curation. If a word no longer has multiple
  distinct break sets, it is no longer marked ambiguous.

Fragment files support three rule forms:

```text
fragment
prefix:fragment
suffix:fragment
```

Plain fragments block either visible side of a break. `prefix:` blocks only the
left fragment before the break, and `suffix:` blocks only the right fragment
after the break. This keeps broad strings from over-filtering unrelated words.

## Source Files

| language | source gold | typesetting gold | fragment file |
| --- | --- | --- | --- |
| Czech | `data/gold/wiktextract/cs.jsonl.zst` | `data/gold/wiktextract/cs_typeset.jsonl.zst` | `data/curation/typeset_fragments/wiktextract_cs.txt` |
| German | `data/gold/wiktextract/de.jsonl.zst` | `data/gold/wiktextract/de_typeset.jsonl.zst` | `data/curation/typeset_fragments/wiktextract_de.txt` |
| Spanish | `data/gold/wiktextract/es.jsonl.zst` | `data/gold/wiktextract/es_typeset.jsonl.zst` | `data/curation/typeset_fragments/wiktextract_es.txt` |
| Italian | `data/gold/wiktextract/it.jsonl.zst` | `data/gold/wiktextract/it_typeset.jsonl.zst` | `data/curation/typeset_fragments/wiktextract_it.txt` |
| Dutch | `data/gold/wiktextract/nl.jsonl.zst` | `data/gold/wiktextract/nl_typeset.jsonl.zst` | `data/curation/typeset_fragments/wiktextract_nl.txt` |
| Russian | `data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst` | `data/gold/wiktextract/ru_cyrl_trusted_dedup_typeset.jsonl.zst` | `data/curation/typeset_fragments/wiktextract_ru_cyrl_trusted_dedup.txt` |
| Turkish | `data/gold/wiktextract/tr.jsonl.zst` | `data/gold/wiktextract/tr_typeset.jsonl.zst` | `data/curation/typeset_fragments/wiktextract_tr.txt` |

## Counts

| dataset | records | breaks before | breaks after | no-break before | no-break after | ambiguous before | ambiguous after | changed records | removed breaks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `cs_typeset` | 62,238 | 129,350 | 94,523 | 4,185 | 12,246 | 2 | 2 | 34,601 | 34,827 |
| `de_typeset` | 968,552 | 2,731,041 | 2,558,340 | 28,160 | 47,020 | 0 | 0 | 171,036 | 172,701 |
| `es_typeset` | 810,956 | 2,651,634 | 2,222,200 | 1,326 | 18,123 | 0 | 0 | 382,333 | 429,434 |
| `it_typeset` | 4,558 | 11,283 | 7,489 | 142 | 733 | 0 | 0 | 3,494 | 3,794 |
| `nl_typeset` | 627,408 | 1,754,003 | 1,604,120 | 22,016 | 39,518 | 26 | 26 | 147,250 | 149,883 |
| `ru_cyrl_trusted_dedup_typeset` | 15,016 | 18,433 | 14,770 | 7,388 | 8,569 | 23 | 4 | 3,430 | 3,683 |
| `tr_typeset` | 18,435 | 43,193 | 31,256 | 711 | 3,024 | 4 | 4 | 9,995 | 11,937 |

## Evaluation

The held-out comparison is in
[`summary.md`](summary.md). The protocol uses deterministic grouped 5-fold
cross-validation. For each fold, Guarded N-gram is trained only on that fold's
train file and is evaluated on the same test file as Hypher and Liang.

Italian is intentionally reported with both the reusable Guarded N-gram
typesetting model and the Liang baseline. In this dataset, the Guarded N-gram
typesetting recipe is very conservative: it has high precision and low
serious-error rate, but low recall. Use the report table when choosing between
fast, conservative behavior and higher recall.

## Recreate

```bash
bash scripts/curate_wiktextract_typeset.sh

DATASETS="wiktextract_cs_typeset wiktextract_de_typeset wiktextract_es_typeset wiktextract_it_typeset wiktextract_nl_typeset wiktextract_ru_cyrl_trusted_dedup_typeset wiktextract_tr_typeset" \
REPORT_TITLE="Wiktextract Typesetting 5-Fold Evaluation" \
REPORT_ROOT=target/hyphlab-reports/wiktextract_typeset_5fold_v1 \
FOLD_ROOT=target/hyphlab-folds/wiktextract_typeset_5fold_v1 \
MODEL_ROOT=target/hyphlab-models/wiktextract_typeset_5fold_v1 \
MANIFEST_ROOT=target/hyphlab-manifests/wiktextract_typeset_5fold_v1 \
PUBLIC_REPORT_ROOT=docs/reports/wiktextract_typeset_5fold_v1 \
  bash scripts/run_multilingual_5fold_evaluation.sh
```
