# Guarded N-gram Boundary Rules

Guarded N-gram is the fast finite-context rule family used by the current
multilingual experiments. A model is compiled from labeled hyphenation records
and then loaded as a compact binary rule table. The reusable bundle also
includes a small Italian onset-syllable model, because that selected Italian
method is a syllabification rule model rather than an n-gram boundary table.

The method learns local boundary contexts that are safe enough to add a break.
Some recipes also learn broader veto contexts that suppress likely false
positives. At runtime, prediction is a small number of packed n-gram lookups per
candidate boundary, so the steady-state path stays simple and fast.

## Train A Model

Use a labeled train corpus in hyphlab JSONL or JSONL.zst format:

```bash
cargo build -p hyph-cli --release --features adapters-hyphenation-embedded

target/release/hyphlab compile-safe-ngram \
  --gold data/splits/moby_en_us/train.jsonl.zst \
  --locale en-US \
  --method safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85 \
  --output models/guarded_ngram/custom/en_us.bin
```

## Evaluate A Model

```bash
target/release/hyphlab eval \
  --gold data/splits/moby_en_us/test.jsonl.zst \
  --locale en-US \
  --method safe-ngram-model \
  --dictionary models/guarded_ngram/custom/en_us.bin \
  --output target/hyphlab-reports/manual/guarded_ngram_en_us.json
```

For fair accuracy claims, train on `train` and evaluate on held-out `test`.
Models compiled from a full corpus are useful for reuse and integration, but
must not be evaluated on the same full corpus as if they were independent.

## Manifest Entry

```toml
[[methods]]
slug = "guarded_ngram"
method = "safe-ngram-model"
dictionary = "models/guarded_ngram/custom/en_us.bin"
```

Paths in a manifest are resolved relative to the manifest file location. A
manifest at the project root can use `models/...`; generated manifests under
`manifests/guarded_ngram/v1/` use paths such as
`../../../models/guarded_ngram/v1/moby_en_us.bin`.

Use the manifest with the matrix runner:

```bash
target/release/hyphlab matrix \
  --manifest methods.toml \
  --gold data/splits/moby_en_us/test.jsonl.zst \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --output-dir target/hyphlab-reports/manual/guarded_ngram_en_us \
  --iterations 20 \
  --init-iterations 5
```

## Current Recipes

These recipes are the selected multilingual settings used by
`scripts/run_multilingual_5fold_evaluation.sh`.

| dataset | locale | report slug | recipe |
| --- | --- | --- | --- |
| `moby_en_us` | `en-US` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85` |
| `wiktextract_cs` | `cs` | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` |
| `wiktextract_de` | `de` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |
| `wiktextract_es` | `es` | `guarded_ngram` | `safe-ngram-unicode-3x2-s1-p60` |
| `wiktextract_it` | `it` | `italian_onset_syllable` | `italian-syllable` |
| `wiktextract_nl` | `nl` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |
| `wiktextract_ru_cyrl_trusted_dedup` | `ru` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80` |
| `wiktextract_tr` | `tr` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x2-s1-p70` |

Build reusable full-corpus models for the current recipes:

```bash
bash scripts/build_guarded_ngram_models.sh
cat models/guarded_ngram/v1/README.md
```

The generated manifests point at the reusable runtime models. Italian uses
`models/guarded_ngram/v1/wiktextract_it.json` there; the 5-fold evaluation
script still trains its cluster table from each fold's train split for the
measured comparison.

The runtime models are trained from the full normalized corpora listed in
`models/guarded_ngram/v1/README.md`. Use split-based or 5-fold runs for claims
about generalization.
