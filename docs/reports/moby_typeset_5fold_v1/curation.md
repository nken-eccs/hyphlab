# Moby Typesetting Curation

The curated corpus is generated from `data/gold/moby_en_us.jsonl.zst`:

```bash
bash scripts/curate_moby_typeset.sh
```

Output:

```text
data/gold/moby_en_us_typeset.jsonl.zst
```

## Policy

The source Moby labels are syllable-oriented. For typesetting experiments, the
curation step removes boundaries that would create unsafe visible line
fragments.

Rules:

- Drop records containing the Unicode replacement character.
- Remove boundaries outside alphabetic token spans.
- Remove boundaries that leave fewer than 2 alphabetic graphemes on the left of
  the token.
- Remove boundaries that leave fewer than 3 alphabetic graphemes on the right
  of the token.
- Remove boundaries that expose a listed fragment as an exact standalone prefix
  or suffix. The current fragment list is
  `data/curation/typeset_fragments/moby_en_us.txt`.

The curation does not copy Hypher, Liang, or Guarded N-gram output into the gold
data. It keeps surviving Moby boundaries after applying the safety filters.

## Fragment Scan

Two read-only scans checked the full Moby corpus for exact line fragments under
the same token, length, and replacement-character rules. The integrated
fragment list covers the high-confidence automatic candidates:

```text
jap gyp nig paki spic fag gook coon dago
anal arse anus ass bastard bitch clit idiot klan moron nazi
piss scum shit fuck fucker twat slut crap sodo pedo paedo peder
rape rectum vagina boob whore
```

The following candidates are real visible fragments, but remain manual
style-policy questions because they are common names, scientific prefixes,
ethnonyms, medical terms, or ordinary words in many contexts:

```text
sex cum cock homo jew hell negro islam hindu arab chin
pee poo butt slave vom mac aa mc
```

The scans did not find surviving exact eligible hits for several severe
substring-only or absent candidates, including `cunt`, `dildo`, `porn`,
`penis`, `semen`, `kike`, `chink`, `wetback`, `redskin`, and `wop`.

## Current Run

```text
input_records: 185149
output_records: 183865
changed_records: 63096
dropped_records: 1284
dropped_replacement_char_records: 1284
removed_breaks: 72276
```

Corpus stats:

| measure | value |
| --- | ---: |
| records | 183,865 |
| breaks | 350,842 |
| no-break words | 22,816 |
| ambiguous words | 1,496 |
| sensitive-fragment removals | 299 |

Examples:

| word | original | curated | reason |
| --- | --- | --- | --- |
| `Japanese` | `Jap-a-nese` | `Japa-nese` | remove `Jap-` sensitive prefix |
| `gypsum` | `gyp-sum` | `gypsum` | remove `gyp-` fragment |
| `Pakistan` | `Pa-ki-stan` | `Pa-kistan` | remove `paki-` fragment |
| `bitchery` | `bitch-ery` | `bitchery` | remove `bitch-` fragment |
| `whoredom` | `whore-dom` | `whoredom` | remove `whore-` fragment |
| `scumble` | `scum-ble` | `scumble` | remove `scum-` fragment |

The full change log is written to:

```text
target/hyphlab-reports/curation/moby_en_us_typeset.tsv
```

## Runtime Guard

The reusable `en-US-typeset` model uses the same exact fragment filter after
Guarded N-gram prediction. This prevents a learned rule from reintroducing a
line fragment that the curated gold corpus removed.

Example:

```bash
target/release/hyphlab predict --saved-model en-US-typeset \
  --gold data/gold/moby_en_us_typeset.jsonl.zst \
  --word Japanese --word bitchery --word whoredom --word scumble \
  --show-breaks
```

## Held-Out Evaluation

The 5-fold comparison is in:

```text
docs/reports/moby_typeset_5fold_v1/summary.md
```

Mean held-out results:

| method | precision | recall | f1 | serious_error | ns/word |
| --- | ---: | ---: | ---: | ---: | ---: |
| Hypher | 0.898820 | 0.742253 | 0.813066 | 0.132994 | 420.643676 |
| Liang TeX | 0.898836 | 0.742258 | 0.813076 | 0.132966 | 1080.091440 |
| Guarded N-gram with fragment guard | 0.954636 | 0.835253 | 0.890962 | 0.071821 | 84.402693 |
