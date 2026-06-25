# Guarded N-gram Models

These are reusable runtime models built from the full normalized corpora listed
below. Use them for demos, application integration, and quick experiments. For
unbiased accuracy claims, use the 5-fold scripts so every fold trains on its
train split and evaluates on held-out data.

## Use

Build the CLI once:

```bash
cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
```

For visible English line breaks, use the typesetting model:

```bash
target/release/hyphlab predict --saved-model en-US-typeset --word Japanese
target/release/hyphlab predict --saved-model en-US-typeset \
  --text "Japanese typography needs careful hyphenation."
```

Use `*-typeset` saved models when the result may become a visible line break.
The plain models follow source lexical hyphenation more closely; the typeset
models use curated labels and the same guard policy at runtime: unsafe
fragments, MixedCase / ALLCAPS tokens, and configured proper names are kept
intact.

List the reusable models and run other locales directly:

```bash
target/release/hyphlab predict --list-saved-models
target/release/hyphlab predict --saved-model en-US --word hyphenation --word typesetting
target/release/hyphlab predict --saved-model de-typeset --word Scheißhaus
target/release/hyphlab predict --saved-model de --text "Silbentrennung fuer lange Woerter"
target/release/hyphlab predict --saved-model en-US-typeset --with-hypher \
  --gold data/gold/moby_en_us_typeset.jsonl.zst \
  --word Japanese \
  --show-breaks
```

Try the Italian onset-syllable model:

```bash
target/release/hyphlab predict --saved-model it --word informazione --word straordinario
```

The manifests in `manifests/guarded_ngram/v1/` can be passed to
`target/release/hyphlab matrix`; their model paths are relative to the manifest
file location.

## Customize Proper Names

For application-specific names, change the guard policy rather than the model
binary:

1. Copy the closest policy, for example
   `data/curation/guard_policies/moby_en_us_typeset.toml`.
2. Create a house-style name list, for example
   `data/curation/proper_names/my_en_us.txt`, with one protected name per line.
3. Use `[proper_names].paths` to layer the base list and your house list.
4. Use the policy with `--guard-policy`:

```bash
target/release/hyphlab predict --saved-model en-US-typeset \
  --guard-policy data/curation/guard_policies/my_en_us_typeset.toml \
  --word McDonald \
  --show-breaks
```

Paths inside a guard policy are resolved relative to the policy file. Use
`matching = "case-insensitive"` for ordinary name protection and
`matching = "case-sensitive"` only when case must distinguish entries.
See `docs/guarded_ngram.md` for runtime order, guard-policy format, and
tokenization limits.

If the change should become part of a reusable corpus and model, rerun the
curation, model build, and evaluation scripts with the updated policy.

## Which Model Should I Use?

Use the model whose locale and source corpus match your target:

- English en-US: `moby_en_us.bin`, trained from Moby Hyphenator II.
- English en-US typesetting: `moby_en_us_typeset.bin`, trained from the
  curated Moby typesetting corpus and guarded by the same policy at runtime.
- Czech, German, Spanish, Dutch, Russian, and Turkish: the matching
  `wiktextract_*.bin` or `wiktextract_*_typeset.bin` model.
- Italian: `wiktextract_it.json` is an onset-style model trained from normalized
  Italian Wiktextract / Kaikki entries. `wiktextract_it_typeset.bin` is a
  guarded n-gram model trained from the curated Italian typesetting corpus.

These files are full-corpus runtime models. Do not evaluate them on the same
full corpus as an independent test. For reproducible comparisons against Hypher
or Liang baselines, use `docs/reports/multilingual_5fold_v1/` or rerun
`scripts/run_multilingual_5fold_evaluation.sh`.

## Inventory

| dataset | locale | trained from | training policy | slug | recipe | model | manifest |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `moby_en_us` | `en-US` | `data/gold/moby_en_us.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85` | `models/guarded_ngram/v1/moby_en_us.bin` | `manifests/guarded_ngram/v1/moby_en_us.toml` |
| `moby_en_us_typeset` | `en-US` | `data/gold/moby_en_us_typeset.jsonl.zst` | full curated corpus plus runtime guard policy | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85` | `models/guarded_ngram/v1/moby_en_us_typeset.bin` | `manifests/guarded_ngram/v1/moby_en_us_typeset.toml` |
| `wiktextract_cs` | `cs` | `data/gold/wiktextract/cs.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` | `models/guarded_ngram/v1/wiktextract_cs.bin` | `manifests/guarded_ngram/v1/wiktextract_cs.toml` |
| `wiktextract_cs_typeset` | `cs` | `data/gold/wiktextract/cs_typeset.jsonl.zst` | full curated corpus plus runtime guard policy | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` | `models/guarded_ngram/v1/wiktextract_cs_typeset.bin` | `manifests/guarded_ngram/v1/wiktextract_cs_typeset.toml` |
| `wiktextract_de` | `de` | `data/gold/wiktextract/de.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_de.bin` | `manifests/guarded_ngram/v1/wiktextract_de.toml` |
| `wiktextract_de_typeset` | `de` | `data/gold/wiktextract/de_typeset.jsonl.zst` | full curated corpus plus runtime guard policy | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_de_typeset.bin` | `manifests/guarded_ngram/v1/wiktextract_de_typeset.toml` |
| `wiktextract_es` | `es` | `data/gold/wiktextract/es.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-3x2-s1-p60` | `models/guarded_ngram/v1/wiktextract_es.bin` | `manifests/guarded_ngram/v1/wiktextract_es.toml` |
| `wiktextract_es_typeset` | `es` | `data/gold/wiktextract/es_typeset.jsonl.zst` | full curated corpus plus runtime guard policy | `guarded_ngram` | `safe-ngram-unicode-3x2-s1-p60` | `models/guarded_ngram/v1/wiktextract_es_typeset.bin` | `manifests/guarded_ngram/v1/wiktextract_es_typeset.toml` |
| `wiktextract_it` | `it` | `data/gold/wiktextract/it.jsonl.zst` | full normalized corpus | `italian_onset_syllable` | `italian-syllable` | `models/guarded_ngram/v1/wiktextract_it.json` | `manifests/guarded_ngram/v1/wiktextract_it.toml` |
| `wiktextract_it_typeset` | `it` | `data/gold/wiktextract/it_typeset.jsonl.zst` | full curated corpus plus runtime guard policy | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` | `models/guarded_ngram/v1/wiktextract_it_typeset.bin` | `manifests/guarded_ngram/v1/wiktextract_it_typeset.toml` |
| `wiktextract_nl` | `nl` | `data/gold/wiktextract/nl.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_nl.bin` | `manifests/guarded_ngram/v1/wiktextract_nl.toml` |
| `wiktextract_nl_typeset` | `nl` | `data/gold/wiktextract/nl_typeset.jsonl.zst` | full curated corpus plus runtime guard policy | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_nl_typeset.bin` | `manifests/guarded_ngram/v1/wiktextract_nl_typeset.toml` |
| `wiktextract_ru_cyrl_trusted_dedup` | `ru` | `data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup.bin` | `manifests/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup.toml` |
| `wiktextract_ru_cyrl_trusted_dedup_typeset` | `ru` | `data/gold/wiktextract/ru_cyrl_trusted_dedup_typeset.jsonl.zst` | full curated corpus plus runtime guard policy | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup_typeset.bin` | `manifests/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup_typeset.toml` |
| `wiktextract_tr` | `tr` | `data/gold/wiktextract/tr.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x2-s1-p70` | `models/guarded_ngram/v1/wiktextract_tr.bin` | `manifests/guarded_ngram/v1/wiktextract_tr.toml` |
| `wiktextract_tr_typeset` | `tr` | `data/gold/wiktextract/tr_typeset.jsonl.zst` | full curated corpus plus runtime guard policy | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x2-s1-p70` | `models/guarded_ngram/v1/wiktextract_tr_typeset.bin` | `manifests/guarded_ngram/v1/wiktextract_tr_typeset.toml` |
