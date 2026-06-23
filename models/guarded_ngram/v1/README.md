# Guarded N-gram Models

These models are generated from the configured normalized corpora. Use them for reuse, demos, and downstream integration. For unbiased evaluation, train on a split and evaluate on held-out data instead of evaluating a full-corpus model on its own training corpus.

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

## Inventory

| dataset | locale | slug | recipe | model | manifest |
| --- | --- | --- | --- | --- | --- |
| `moby_en_us` | `en-US` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85` | `models/guarded_ngram/v1/moby_en_us.bin` | `manifests/guarded_ngram/v1/moby_en_us.toml` |
| `wiktextract_cs` | `cs` | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` | `models/guarded_ngram/v1/wiktextract_cs.bin` | `manifests/guarded_ngram/v1/wiktextract_cs.toml` |
| `wiktextract_de` | `de` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_de.bin` | `manifests/guarded_ngram/v1/wiktextract_de.toml` |
| `wiktextract_es` | `es` | `guarded_ngram` | `safe-ngram-unicode-3x2-s1-p60` | `models/guarded_ngram/v1/wiktextract_es.bin` | `manifests/guarded_ngram/v1/wiktextract_es.toml` |
| `wiktextract_it` | `it` | `italian_onset_syllable` | `italian-syllable` | `models/guarded_ngram/v1/wiktextract_it.json` | `manifests/guarded_ngram/v1/wiktextract_it.toml` |
| `wiktextract_nl` | `nl` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_nl.bin` | `manifests/guarded_ngram/v1/wiktextract_nl.toml` |
| `wiktextract_ru_cyrl_trusted_dedup` | `ru` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80` | `models/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup.bin` | `manifests/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup.toml` |
| `wiktextract_tr` | `tr` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x2-s1-p70` | `models/guarded_ngram/v1/wiktextract_tr.bin` | `manifests/guarded_ngram/v1/wiktextract_tr.toml` |
