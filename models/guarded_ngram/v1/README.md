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

Run a reusable model directly:

```bash
target/release/hyphlab eval \
  --gold data/gold/toy_en.jsonl \
  --locale en-US \
  --method safe-ngram-model \
  --dictionary models/guarded_ngram/v1/moby_en_us.bin \
  --output target/hyphlab-reports/manual/guarded_ngram_toy_en.json
```

Try the Italian onset-syllable model without an evaluation corpus:

```bash
printf "informazione\nstraordinario\nuniversita\n" |
  target/release/hyphlab predict \
    --locale it \
    --method italian-syllable-model \
    --dictionary models/guarded_ngram/v1/wiktextract_it.json
```

The manifests in `manifests/guarded_ngram/v1/` can be passed to
`target/release/hyphlab matrix`; their model paths are relative to the manifest
file location.

## Which Model Should I Use?

Use the model whose locale and source corpus match your target:

- English en-US: `moby_en_us.bin`, trained from Moby Hyphenator II.
- Czech, German, Spanish, Dutch, Russian, and Turkish: the matching
  `wiktextract_*.bin` model, trained from normalized Wiktextract / Kaikki
  entries for that language.
- Italian: `wiktextract_it.json`, an onset-syllable model trained from
  normalized Italian Wiktextract / Kaikki entries.

These files are full-corpus runtime models. Do not evaluate them on the same
full corpus as an independent test. For reproducible comparisons against Hypher
or Liang baselines, use `docs/reports/multilingual_5fold_v1/` or rerun
`scripts/run_multilingual_5fold_evaluation.sh`.

## Inventory

| dataset | locale | trained from | training policy | slug | recipe | model | manifest |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `moby_en_us` | `en-US` | `data/gold/moby_en_us.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85` | `models/guarded_ngram/v1/moby_en_us.bin` | `manifests/guarded_ngram/v1/moby_en_us.toml` |
| `wiktextract_cs` | `cs` | `data/gold/wiktextract/cs.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` | `models/guarded_ngram/v1/wiktextract_cs.bin` | `manifests/guarded_ngram/v1/wiktextract_cs.toml` |
| `wiktextract_de` | `de` | `data/gold/wiktextract/de.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_de.bin` | `manifests/guarded_ngram/v1/wiktextract_de.toml` |
| `wiktextract_es` | `es` | `data/gold/wiktextract/es.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-3x2-s1-p60` | `models/guarded_ngram/v1/wiktextract_es.bin` | `manifests/guarded_ngram/v1/wiktextract_es.toml` |
| `wiktextract_it` | `it` | `data/gold/wiktextract/it.jsonl.zst` | full normalized corpus | `italian_onset_syllable` | `italian-syllable` | `models/guarded_ngram/v1/wiktextract_it.json` | `manifests/guarded_ngram/v1/wiktextract_it.toml` |
| `wiktextract_nl` | `nl` | `data/gold/wiktextract/nl.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_nl.bin` | `manifests/guarded_ngram/v1/wiktextract_nl.toml` |
| `wiktextract_ru_cyrl_trusted_dedup` | `ru` | `data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup.bin` | `manifests/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup.toml` |
| `wiktextract_tr` | `tr` | `data/gold/wiktextract/tr.jsonl.zst` | full normalized corpus | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x2-s1-p70` | `models/guarded_ngram/v1/wiktextract_tr.bin` | `manifests/guarded_ngram/v1/wiktextract_tr.toml` |
