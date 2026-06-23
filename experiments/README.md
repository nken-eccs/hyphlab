# Experiments

Reusable experiment definitions live here. Generated metrics, models, and
reports still go under `target/`.

## Manifests

- `manifests/moby_en_us_with_crf.toml`: split-based Moby comparison with
  fixed baselines, a train-split dictionary baseline, and the default CRF model.
- `manifests/moby_en_us_crf_tuned.toml`: split-based Moby comparison with
  tuned CRF threshold variants.
- `manifests/moby_en_us_crf_sgd_tuned.toml`: split-based Moby comparison with
  SGD / regularization CRF variants.
- `manifests/moby_en_us_safe_ngram.toml`: split-based Moby comparison for the
  high-precision `safe-ngram` finite-context rule candidates.
- `manifests/moby_en_us_precision_recall90_simple.toml`: simple, interpretable
  candidates for maximizing speed and precision while keeping recall near or
  above 0.90 on the Moby test split. These methods use Liang or hypher plus
  finite-context safe add/veto rules, without pronunciation dictionaries or
  learned black-box rerankers.

Run the current CRF comparison:

```bash
bash scripts/run_crf_unified_matrix.sh
```

Run the current high-precision fast-rule comparison:

```bash
bash scripts/run_safe_ngram_matrix.sh
```

The simple precision/recall>=0.90 manifest is an exploratory definition that
expects generated probe files under `target/probes/` and split data under
`data/splits/`. After recreating those local artifacts, run:

```bash
target/release/hyphlab matrix \
  --manifest experiments/manifests/moby_en_us_precision_recall90_simple.toml \
  --gold data/splits/moby_en_us/test.jsonl.zst \
  --locale en-US \
  --patterns target/probes/patgen/moby_patgen_l1_4.pat \
  --output-dir target/hyphlab-reports/unified/moby_en_us_precision_recall90_simple \
  --iterations 20 \
  --init-iterations 5
```

The current simple candidates are:

- `hypher_safe_fast_r90`: fastest candidate with 5-fold evidence that recall
  stays above 0.90.
- `liang_safe_fast_r90`: higher precision than `hypher_safe_fast_r90`, with a
  slower Liang-pattern base.
- `liang_safe_precision_r90`: best precision among the simple Liang/safe-rule
  candidates, with slower prediction and initialization.

The main held-out report is
`target/hyphlab-reports/unified/moby_en_us_precision_recall90_simple/compare.md`.
The 5-fold summary for `hypher_safe_fast_r90` is
`target/hyphlab-reports/research/precision_recall90/kfold_hypher_safe/summary.md`.

## Multilingual Safe-Ngram Tuning

Use the multilingual tuning script to train and compare Unicode-aware
`safe-ngram` candidates per language:

```bash
bash scripts/prepare_filtered_wiktextract_data.sh
bash scripts/run_multilingual_safe_ngram_tuning.sh
cat target/hyphlab-reports/multilingual/safe_ngram_tuned/summary.md
```

The script currently covers:

- `moby_en_us`
- `wiktextract_cs`
- `wiktextract_de`
- `wiktextract_es`
- `wiktextract_it`
- `wiktextract_nl`
- `wiktextract_ru_cyrl_trusted_dedup`
- `wiktextract_tr`

`wiktextract_ru_cyrl_trusted_dedup` is a Russian-only Cyrillic subset with
duplicate words merged into `ambiguous=true + variants`. It also drops long
vowel-bearing no-break entries, because those are usually missing annotations
rather than reliable no-break labels. The broader `wiktextract_ru_cyrl_dedup`
and full `wiktextract_ru` corpora can still be selected explicitly with
`DATASETS=wiktextract_ru_cyrl_dedup` or `DATASETS=wiktextract_ru`.

For each dataset it:

1. Creates deterministic train/dev/test splits under `target/hyphlab-splits/`.
2. Compiles each candidate into `target/hyphlab-models/multilingual/`.
3. Evaluates candidates and Hypher on dev.
4. Selects the best `safe-ngram` candidate by `OBJECTIVE` while penalizing
   candidates below `MIN_PRECISION`.
5. Reports the selected candidate and Hypher on the held-out test split.

The default policy is precision-first:

```bash
MIN_PRECISION=0.95 OBJECTIVE=f1 \
  bash scripts/run_multilingual_safe_ngram_tuning.sh
```

Run a looser p90 track when recall / F1 should be allowed to trade against
precision:

```bash
REPORT_ROOT=target/hyphlab-reports/multilingual/safe_ngram_tuned_p90 \
MODEL_ROOT=target/hyphlab-models/multilingual/safe_ngram_tuned_p90 \
MANIFEST_ROOT=target/hyphlab-manifests/multilingual/safe_ngram_tuned_p90 \
MIN_PRECISION=0.90 \
  bash scripts/run_multilingual_safe_ngram_tuning.sh
```

Override `DATASETS` or `METHODS` to add a language or candidate without editing
the script:

```bash
DATASETS="wiktextract_de wiktextract_tr" \
METHODS="safe-ngram-unicode-3x3-s1-p80,safe-ngram-unicode-mixcv-2x3-s1-p58-veto-unicode-mixcv-3x4-s1-p80" \
  bash scripts/run_multilingual_safe_ngram_tuning.sh
```

For a Russian-only exploratory sweep:

```bash
DATASETS=wiktextract_ru_cyrl_trusted_dedup \
MIN_PRECISION=0.30 \
OBJECTIVE=f0.5 \
METHODS="safe-ngram-unicode-2x3-s1-p40,safe-ngram-unicode-2x3-s1-p55,safe-ngram-unicode-3x3-s1-p60,safe-ngram-unicode-3x4-s1-p60,safe-ngram-unicode-4x3-s1-p60" \
  bash scripts/run_multilingual_safe_ngram_tuning.sh
```

Use `OBJECTIVE=serious_error MIN_PRECISION=0.95` for a stricter safety track
that minimizes serious word errors among high-precision candidates.

Boundary constraints can be tuned per run. For Italian, `RIGHT_MIN=2` is
important because the gold data contains many valid breaks that leave a
two-grapheme final segment:

```bash
DATASETS=wiktextract_it RIGHT_MIN=2 SPEED_MAX_NS=100 \
MIN_PRECISION=0.95 OBJECTIVE=recall \
  bash scripts/run_multilingual_safe_ngram_tuning.sh
```

Italian also has a Rust-native onset syllabifier. For reuse, load the committed
model:

```bash
printf "informazione\nstraordinario\nuniversita\n" |
  target/release/hyphlab predict \
    --locale it \
    --method italian-syllable-model \
    --dictionary models/guarded_ngram/v1/wiktextract_it.json
```

For split-based experiments, use `italian-syllable` with `--dictionary`
pointing at the train split. That learns a small cluster split table from the
train split and evaluates on held-out data:

```bash
target/release/hyphlab eval \
  --gold target/hyphlab-splits/wiktextract_it/test.jsonl.zst \
  --locale it \
  --method italian-syllable \
  --dictionary target/hyphlab-splits/wiktextract_it/train.jsonl.zst
```

Run a manifest manually:

```bash
target/release/hyphlab matrix \
  --manifest experiments/manifests/moby_en_us_crf_sgd_tuned.toml \
  --gold data/splits/moby_en_us/test.jsonl.zst \
  --locale en-US \
  --patterns data/patterns/tex-hyphen/tex/hyph-en-us.tex \
  --output-dir target/hyphlab-reports/unified/moby_en_us_test_crf_sgd_tuned \
  --iterations 5 \
  --init-iterations 5
```

Model paths in these manifests point to `target/hyphlab-models`. Re-run the CRF
training and tuning commands from `docs/research_start.md` if those files are
missing.
