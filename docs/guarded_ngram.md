# Guarded N-gram Boundary Rules

Guarded N-gram is the fast finite-context rule family used by the current
multilingual experiments. A model is compiled from labeled hyphenation records
and then loaded as a compact binary rule table. The reusable bundle keeps the
plain Italian onset-syllable model as a small comparison method; the Italian
typesetting model uses Guarded N-gram like the other `*-typeset` models.

The method learns local boundary contexts that are safe enough to add a break.
Some recipes also learn broader veto contexts that suppress likely false
positives. At runtime, prediction is a small number of packed n-gram lookups per
candidate boundary, so the steady-state path stays simple and fast.

## Runtime Processing

Plain Guarded N-gram models run in this order:

1. Load the compiled model for the selected locale.
2. For each word, examine boundaries allowed by the saved model config.
3. Add breaks whose learned n-gram contexts match.
4. Remove breaks matched by learned veto rules or orthographic veto rules when
   configured.
5. Return the remaining breaks.

Typesetting models run the same prediction first, then apply a guard policy:

1. Remove breaks that expose configured visible fragments.
2. Remove breaks inside configured proper names.
3. Remove breaks inside MixedCase or ALLCAPS tokens.

The guard policy only removes breaks; it never adds new ones. Ordinary
titlecase words are allowed by default.

## Train A Model

Use a labeled train corpus in hyphlab JSONL or JSONL.zst format:

```bash
cargo build -p hyph-cli --release --features adapters-hyphenation-embedded

target/release/hyphlab method train \
  --method safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85 \
  --gold data/splits/moby_en_us/train.jsonl.zst \
  --locale en-US \
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

For reader-facing line breaks, add a guard policy. The policy is applied after
the learned model predicts breaks and only removes breaks:

```toml
[[methods]]
slug = "guarded_ngram"
method = "safe-ngram-model"
dictionary = "models/guarded_ngram/v1/moby_en_us_typeset.bin"
guard_policy = "data/curation/guard_policies/moby_en_us_typeset.toml"
```

## Customize Guard Policies

Use runtime customization when an application needs to protect additional names
without changing the trained model:

1. Copy a policy from `data/curation/guard_policies/`.
2. Create a house-style name list under `data/curation/proper_names/`.
3. Add one name per line.
4. Use `[proper_names].paths` to layer the base list and your house list.
5. Run prediction with the copied policy.

Example:

```toml
[fragments]
path = "../typeset_fragments/moby_en_us.txt"

[case]
protect_mixed_case = true
protect_all_caps = true
protect_titlecase = false

[proper_names]
paths = [
  "../proper_names/en_us.txt",
  "../proper_names/my_en_us.txt",
]
matching = "case-insensitive"
```

```bash
target/release/hyphlab predict --saved-model en-US-typeset \
  --guard-policy data/curation/guard_policies/my_en_us_typeset.toml \
  --word McDonald \
  --word MyProductName \
  --show-breaks
```

Paths in a guard policy are resolved relative to the policy file. The CLI
`--guard-policy` option overrides the policy bundled with a saved model.

`[proper_names].path = "..."` is also supported for a single list. Use
`paths = [...]` for publisher house-style overlays, so curated base-list updates
do not have to be copied into local files.

Proper-name matching currently protects alphabetic token spans. Names that rely
on punctuation or spacing, such as `O'Connor`, `Coca-Cola`, or `São Paulo`, may
need explicit component entries such as `Connor`, `Coca`, `Cola`, `São`, and
`Paulo`, depending on the style rule you want. The shipped lists are curation
seeds, not complete editorial authority files.

## Language Guard Policy

The current guard policy is intentionally small and predictable:

| rule | default behavior |
| --- | --- |
| Visible fragments | Remove breaks that expose configured prefix/suffix fragments. |
| MixedCase | Protected for all shipped typeset policies. |
| ALLCAPS | Protected for all shipped typeset policies. |
| Titlecase | Not protected by default; ordinary sentence-initial words can still break. |
| Dutch `IJ` titlecase | Treated as titlecase for case classification. |
| Proper names | Exact token-span match against configured lists; shipped policies use case-insensitive matching. |

Use dataset-level customization when the policy should become part of the
curated data and reports:

```bash
GUARD_POLICY=data/curation/guard_policies/my_en_us_typeset.toml \
  bash scripts/curate_moby_typeset.sh

bash scripts/build_guarded_ngram_models.sh
bash scripts/run_typeset_guard_challenge.sh
bash scripts/run_multilingual_5fold_evaluation.sh
```

For Wiktextract languages, update the language-specific policy under
`data/curation/guard_policies/` and rerun
`scripts/curate_wiktextract_typeset.sh`.

Save the entry to a manual manifest, then run it with the matrix runner:

```bash
target/release/hyphlab matrix \
  --manifest target/hyphlab-manifests/manual/guarded_ngram_en_us.toml \
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
| `moby_en_us_typeset` | `en-US` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85` |
| `wiktextract_cs` | `cs` | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` |
| `wiktextract_cs_typeset` | `cs` | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` |
| `wiktextract_de` | `de` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |
| `wiktextract_de_typeset` | `de` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |
| `wiktextract_es` | `es` | `guarded_ngram` | `safe-ngram-unicode-3x2-s1-p60` |
| `wiktextract_es_typeset` | `es` | `guarded_ngram` | `safe-ngram-unicode-3x2-s1-p60` |
| `wiktextract_it` | `it` | `italian_onset_syllable` | `italian-syllable` |
| `wiktextract_it_typeset` | `it` | `guarded_ngram` | `safe-ngram-unicode-2x2-s1-p50` |
| `wiktextract_nl` | `nl` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |
| `wiktextract_nl_typeset` | `nl` | `guarded_ngram` | `safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80` |
| `wiktextract_ru_cyrl_trusted_dedup` | `ru` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80` |
| `wiktextract_ru_cyrl_trusted_dedup_typeset` | `ru` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80` |
| `wiktextract_tr` | `tr` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x2-s1-p70` |
| `wiktextract_tr_typeset` | `tr` | `guarded_ngram` | `safe-ngram-unicode-mixcv-2x2-s1-p70` |

Build reusable full-corpus models for the current recipes:

```bash
bash scripts/build_guarded_ngram_models.sh
cat models/guarded_ngram/v1/README.md
```

The generated manifests point at the reusable runtime models. Plain Italian
uses `models/guarded_ngram/v1/wiktextract_it.json`; Italian typesetting uses
`models/guarded_ngram/v1/wiktextract_it_typeset.bin`.

The runtime models are trained from the full normalized corpora listed in
`models/guarded_ngram/v1/README.md`. Use split-based or 5-fold runs for claims
about generalization.
