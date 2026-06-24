#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${BIN:-target/release/hyphlab}"
REPORT_ROOT="${REPORT_ROOT:-target/hyphlab-reports/multilingual_5fold_v1}"
FOLD_ROOT="${FOLD_ROOT:-target/hyphlab-folds/multilingual_5fold_v1}"
MODEL_ROOT="${MODEL_ROOT:-target/hyphlab-models/multilingual_5fold_v1}"
MANIFEST_ROOT="${MANIFEST_ROOT:-target/hyphlab-manifests/multilingual_5fold_v1}"
PUBLIC_REPORT_ROOT="${PUBLIC_REPORT_ROOT:-docs/reports/multilingual_5fold_v1}"
FOLDS="${FOLDS:-5}"
DEV_RATIO="${DEV_RATIO:-0}"
ITERATIONS="${ITERATIONS:-50}"
INIT_ITERATIONS="${INIT_ITERATIONS:-10}"
INIT_WARMUP="${INIT_WARMUP:-2}"
DATASETS="${DATASETS:-moby_en_us wiktextract_cs wiktextract_de wiktextract_es wiktextract_it wiktextract_nl wiktextract_ru_cyrl_trusted_dedup wiktextract_tr}"
ENABLE_LIBREOFFICE_BASELINE="${ENABLE_LIBREOFFICE_BASELINE:-0}"
REPORT_TITLE="${REPORT_TITLE:-Multilingual 5-Fold Evaluation}"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

mkdir -p "$REPORT_ROOT" "$FOLD_ROOT" "$MODEL_ROOT" "$MANIFEST_ROOT"

dataset_config() {
  local dataset="$1"
  GOLD=""
  LOCALE=""
  PATTERNS=""
  LIBREOFFICE_PATTERNS=""
  SELECTED_KIND=""
  SELECTED_METHOD=""
  SELECTED_RUNTIME_METHOD=""
  SELECTED_SLUG=""
  FRAGMENTS=""
  INCLUDE_HYPHER="1"
  INCLUDE_LIANG="1"

  case "$dataset" in
    moby_en_us)
      GOLD="data/gold/moby_en_us.jsonl.zst"
      LOCALE="en-US"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-en-us.tex"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    moby_en_us_typeset)
      GOLD="data/gold/moby_en_us_typeset.jsonl.zst"
      LOCALE="en-US"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-en-us.tex"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85"
      SELECTED_RUNTIME_METHOD="typeset-safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/moby_en_us.txt"
      ;;
    wiktextract_cs)
      GOLD="data/gold/wiktextract/cs.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/cs/hyph_cs_CZ.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x2-s1-p50"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    wiktextract_cs_typeset)
      GOLD="data/gold/wiktextract/cs_typeset.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/cs/hyph_cs_CZ.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x2-s1-p50"
      SELECTED_RUNTIME_METHOD="typeset-safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_cs.txt"
      ;;
    wiktextract_de)
      GOLD="data/gold/wiktextract/de.jsonl.zst"
      LOCALE="de"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/de/hyph_de_DE.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    wiktextract_de_typeset)
      GOLD="data/gold/wiktextract/de_typeset.jsonl.zst"
      LOCALE="de"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/de/hyph_de_DE.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="typeset-safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_de.txt"
      ;;
    wiktextract_es)
      GOLD="data/gold/wiktextract/es.jsonl.zst"
      LOCALE="es"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-es.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/es/hyph_es.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-3x2-s1-p60"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    wiktextract_es_typeset)
      GOLD="data/gold/wiktextract/es_typeset.jsonl.zst"
      LOCALE="es"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-es.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/es/hyph_es.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-3x2-s1-p60"
      SELECTED_RUNTIME_METHOD="typeset-safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_es.txt"
      ;;
    wiktextract_it)
      GOLD="data/gold/wiktextract/it.jsonl.zst"
      LOCALE="it"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-it.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/it/hyph_it_IT.dic"
      SELECTED_KIND="italian-syllable"
      SELECTED_METHOD="italian-syllable"
      SELECTED_RUNTIME_METHOD="italian-syllable-model"
      SELECTED_SLUG="italian_onset_syllable"
      ;;
    wiktextract_it_typeset)
      GOLD="data/gold/wiktextract/it_typeset.jsonl.zst"
      LOCALE="it"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-it.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/it/hyph_it_IT.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="typeset-safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_it.txt"
      ;;
    wiktextract_nl)
      GOLD="data/gold/wiktextract/nl.jsonl.zst"
      LOCALE="nl"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-nl.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/nl/hyph_nl_NL.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    wiktextract_nl_typeset)
      GOLD="data/gold/wiktextract/nl_typeset.jsonl.zst"
      LOCALE="nl"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-nl.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/nl/hyph_nl_NL.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="typeset-safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_nl.txt"
      ;;
    wiktextract_ru_cyrl_trusted_dedup)
      GOLD="data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/ru/hyph_ru_RU.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    wiktextract_ru_cyrl_trusted_dedup_typeset)
      GOLD="data/gold/wiktextract/ru_cyrl_trusted_dedup_typeset.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/ru/hyph_ru_RU.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="typeset-safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_ru_cyrl_trusted_dedup.txt"
      ;;
    wiktextract_tr)
      GOLD="data/gold/wiktextract/tr.jsonl.zst"
      LOCALE="tr"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-tr.tex"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-mixcv-2x2-s1-p70"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    wiktextract_tr_typeset)
      GOLD="data/gold/wiktextract/tr_typeset.jsonl.zst"
      LOCALE="tr"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-tr.tex"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-mixcv-2x2-s1-p70"
      SELECTED_RUNTIME_METHOD="typeset-safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      FRAGMENTS="data/curation/typeset_fragments/wiktextract_tr.txt"
      ;;
    hyph_bench_cs_cstenten)
      GOLD="data/gold/hyph_bench/cs_cstenten.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/cs/hyph_cs_CZ.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x2-s1-p50"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    hyph_bench_cs_ujc)
      GOLD="data/gold/hyph_bench/cs_ujc.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/cs/hyph_cs_CZ.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x2-s1-p50"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    hyph_bench_cssk_cshyphen)
      GOLD="data/gold/hyph_bench/cssk_cshyphen.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/cs/hyph_cs_CZ.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x2-s1-p50"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    hyph_bench_de_wortliste)
      GOLD="data/gold/hyph_bench/de_wortliste.jsonl.zst"
      LOCALE="de-DE"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      LIBREOFFICE_PATTERNS="data/patterns/libreoffice/de/hyph_de_DE.dic"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      ;;
    hyph_bench_is_hyphis)
      GOLD="data/gold/hyph_bench/is_hyphis.jsonl.zst"
      LOCALE="is-IS"
      PATTERNS=""
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      INCLUDE_HYPHER="0"
      INCLUDE_LIANG="0"
      ;;
    hyph_bench_th_orchid)
      GOLD="data/gold/hyph_bench/th_orchid.jsonl.zst"
      LOCALE="th-TH"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-th.tex"
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-2x2-s1-p50"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      INCLUDE_HYPHER="0"
      ;;
    hyph_bench_uk_wiktionary)
      GOLD="data/gold/hyph_bench/uk_wiktionary.jsonl.zst"
      LOCALE="uk-UA"
      PATTERNS=""
      SELECTED_KIND="safe-ngram"
      SELECTED_METHOD="safe-ngram-unicode-mixcv-2x3-s1-p65-veto-unicode-3x4-s1-p80"
      SELECTED_RUNTIME_METHOD="safe-ngram-model"
      SELECTED_SLUG="guarded_ngram"
      INCLUDE_HYPHER="0"
      INCLUDE_LIANG="0"
      ;;
    *)
      printf 'unknown dataset: %s\n' "$dataset" >&2
      exit 1
      ;;
  esac
}

write_manifest() {
  local train="$1"
  local manifest="$2"
  local model_dir="$3"
  local training_manifest="$manifest.train.toml"
  mkdir -p "$model_dir" "$(dirname "$manifest")"
  local method_fragments="$FRAGMENTS"
  local model_extension
  case "$SELECTED_KIND" in
    safe-ngram) model_extension="bin" ;;
    italian-syllable) model_extension="json" ;;
    *)
      printf 'unknown selected method kind: %s\n' "$SELECTED_KIND" >&2
      exit 1
      ;;
  esac

  {
    if [ "$INCLUDE_HYPHER" = "1" ]; then
      printf '[[methods]]\n'
      printf 'slug = "hypher"\n'
      printf 'method = "hypher"\n\n'
    fi
    if [ "$INCLUDE_LIANG" = "1" ]; then
      printf '[[methods]]\n'
      printf 'slug = "liang_tex"\n'
      printf 'method = "liang"\n'
      printf 'requires_patterns = true\n\n'
    fi
    if [ "$ENABLE_LIBREOFFICE_BASELINE" = "1" ] && [ -n "$LIBREOFFICE_PATTERNS" ]; then
      printf '[[methods]]\n'
      printf 'slug = "liang_libreoffice"\n'
      printf 'method = "liang"\n'
      printf 'patterns = "%s"\n' "$(pwd)/$LIBREOFFICE_PATTERNS"
      printf 'requires_patterns = true\n\n'
    fi
    printf '[[methods]]\n'
    printf 'slug = "%s"\n' "$SELECTED_SLUG"
    printf 'method = "%s"\n' "$SELECTED_METHOD"
    if [ -n "$method_fragments" ]; then
      printf 'patterns = "%s"\n' "$(pwd)/$method_fragments"
      printf 'pass_patterns = true\n'
    fi
    printf '\n'
    printf '[methods.train]\n'
    printf 'runtime_method = "%s"\n' "$SELECTED_RUNTIME_METHOD"
    printf 'output = "{model_dir}/{slug}.%s"\n' "$model_extension"
    printf '\n'
  } > "$training_manifest"

  "$BIN" method materialize \
    --manifest "$training_manifest" \
    --gold "$train" \
    --locale "$LOCALE" \
    --model-dir "$model_dir" \
    --output "$manifest"
}

fold_row() {
  local dataset="$1"
  local report_dir="$2"
  local slug="$3"
  local label="$4"

  for fold_dir in "$report_dir"/fold-*; do
    [ -d "$fold_dir" ] || continue
    local metric_json="$fold_dir/$slug.json"
    local speed_json="$fold_dir/speed/$slug.json"
    [ -f "$metric_json" ] || continue
    [ -f "$speed_json" ] || continue
    jq -r --arg dataset "$dataset" --arg label "$label" --arg fold "$(basename "$fold_dir")" --slurpfile speed "$speed_json" '
      def div0($a;$b): if $b == 0 then 0 else ($a / $b) end;
      .metrics as $m |
      ($m.tp|tonumber) as $tp |
      ($m.fp|tonumber) as $fp |
      ($m.fn_|tonumber) as $fn |
      div0($tp; ($tp+$fp)) as $p |
      div0($tp; ($tp+$fn)) as $r |
      div0(2*$p*$r; ($p+$r)) as $f1 |
      div0(1.25*$p*$r; (0.25*$p+$r)) as $f05 |
      div0($m.exact_words; $m.words) as $exact |
      div0($m.serious_word_errors; $m.words) as $serious |
      div0($fp * 100000.0; ($tp+$fp+$fn+$m.tn)) as $fp100k |
      [$dataset, $label, $fold, $m.words, $p, $r, $f1, $f05, $exact, $serious, $fp100k, $speed[0].ns_per_word] | @tsv
    ' "$metric_json"
  done
}

summarize_overall() {
  local fold_tsv="$1"
  local summary_md="$2"
  local summary_tsv="$3"

  {
    printf 'dataset\tmethod\tfolds\twords\tprecision\trecall\tf1\tf0.5\texact\tserious_error\tfp_per_100k\tns_per_word\n'
    tail -n +2 "$fold_tsv" |
      awk -F '\t' '
        function key(dataset, method) { return dataset SUBSEP method }
        function mean(sum, n) { return n ? sum / n : 0 }
        function sd(sum, sumsq, n,     m, v) {
          if (n < 2) return 0
          m = sum / n
          v = (sumsq - n * m * m) / (n - 1)
          return v > 0 ? sqrt(v) : 0
        }
        {
          k = key($1, $2)
          if (!(k in seen)) {
            seen[k] = 1
            order[++order_n] = k
            dataset[k] = $1
            method[k] = $2
          }
          n[k]++
          for (i = 4; i <= 12; i++) {
            value = $i + 0
            sum[k,i] += value
            sumsq[k,i] += value * value
          }
        }
        END {
          for (idx = 1; idx <= order_n; idx++) {
            k = order[idx]
            printf "%s\t%s\t%d", dataset[k], method[k], n[k]
            for (i = 4; i <= 12; i++) {
              printf "\t%.6f (sd %.6f)", mean(sum[k,i], n[k]), sd(sum[k,i], sumsq[k,i], n[k])
            }
            printf "\n"
          }
        }'
  } > "$summary_tsv"

  {
    printf '# %s\n\n' "$REPORT_TITLE"
    printf 'Protocol:\n\n'
    printf '%s\n' '- The selected method per dataset is fixed before this run.'
    printf -- '- Each dataset is evaluated with deterministic grouped `%s`-fold cross-validation.\n' "$FOLDS"
    printf '%s\n' '- For each fold, trainable methods are trained only on that fold train file and evaluated on that fold test file.'
    printf '%s\n' '- Hypher and Liang are evaluated on the same fold test files when supported for the dataset.'
    if [ "$ENABLE_LIBREOFFICE_BASELINE" = "1" ]; then
      printf '%s\n' '- LibreOffice hyphen dictionaries are included as an additional Liang/libhyphen pattern baseline when available.'
    fi
    printf '%s\n' '- Ambiguous records use the default `exclude` policy.'
    printf -- '- Runtime uses `target/release/hyphlab`, `%s` steady-state iterations, `%s` init iterations, and `%s` init warmup.\n' "$ITERATIONS" "$INIT_ITERATIONS" "$INIT_WARMUP"
    printf -- '- Runtime values are machine-local and should be used for within-run comparison unless hardware details are documented separately.\n\n'
    printf 'Selected methods:\n\n'
    printf '| dataset | report slug | recipe |\n'
    printf '| --- | --- | --- |\n'
    for dataset in $DATASETS; do
      dataset_config "$dataset"
      printf '| `%s` | `%s` | `%s` |\n' "$dataset" "$SELECTED_SLUG" "$SELECTED_METHOD"
    done
    printf '\n'
    printf 'Gold data:\n\n'
    for dataset in $DATASETS; do
      dataset_config "$dataset"
      printf -- '- `%s`: `%s`\n' "$dataset" "$GOLD"
    done
    printf '\n'
    printf 'Mean and sample standard deviation across folds:\n\n'
    printf '| dataset | method | folds | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | ns/word |\n'
    printf '| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n'
    tail -n +2 "$summary_tsv" |
      while IFS="$(printf '\t')" read -r dataset method folds words p r f1 f05 exact serious fp ns; do
        printf '| `%s` | `%s` | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s |\n' \
          "$dataset" "$method" "$folds" "$words" "$p" "$r" "$f1" "$f05" "$exact" "$serious" "$fp" "$ns"
      done
    printf '\n'
    printf 'Per-dataset fold summaries are written next to each dataset report.\n'
  } > "$summary_md"
}

publish_summary() {
  if [ "${PUBLISH_SUMMARY:-1}" != "1" ]; then
    return
  fi
  mkdir -p "$PUBLIC_REPORT_ROOT"
  cp "$REPORT_ROOT/summary.md" "$PUBLIC_REPORT_ROOT/summary.md"
  cp "$REPORT_ROOT/summary.tsv" "$PUBLIC_REPORT_ROOT/summary.tsv"
  cp "$REPORT_ROOT/folds.tsv" "$PUBLIC_REPORT_ROOT/folds.tsv"
}

fold_tsv="$REPORT_ROOT/folds.tsv"
{
  printf 'dataset\tmethod\tfold\twords\tprecision\trecall\tf1\tf0.5\texact\tserious_error\tfp_per_100k\tns_per_word\n'
} > "$fold_tsv"

if [ "${SUMMARIZE_ONLY:-0}" = "1" ]; then
  for dataset in $DATASETS; do
    dataset_config "$dataset"
    dataset_report="$REPORT_ROOT/$dataset"
    if [ "$INCLUDE_HYPHER" = "1" ]; then
      fold_row "$dataset" "$dataset_report" "hypher" "hypher" >> "$fold_tsv"
    fi
    if [ "$INCLUDE_LIANG" = "1" ]; then
      fold_row "$dataset" "$dataset_report" "liang_tex" "liang_tex" >> "$fold_tsv"
    fi
    if [ "$ENABLE_LIBREOFFICE_BASELINE" = "1" ] && [ -n "$LIBREOFFICE_PATTERNS" ]; then
      fold_row "$dataset" "$dataset_report" "liang_libreoffice" "liang_libreoffice" >> "$fold_tsv"
    fi
    fold_row "$dataset" "$dataset_report" "$SELECTED_SLUG" "$SELECTED_SLUG" >> "$fold_tsv"
  done

  summarize_overall "$fold_tsv" "$REPORT_ROOT/summary.md" "$REPORT_ROOT/summary.tsv"
  publish_summary
  printf 'wrote %s\n' "$REPORT_ROOT/summary.md"
  printf 'wrote %s\n' "$PUBLIC_REPORT_ROOT/summary.md"
  exit 0
fi

for dataset in $DATASETS; do
  dataset_config "$dataset"
  if [ ! -s "$GOLD" ]; then
    printf 'skip %s: missing or empty gold %s\n' "$dataset" "$GOLD"
    continue
  fi

  fold_data_dir="$FOLD_ROOT/$dataset"
  dataset_report="$REPORT_ROOT/$dataset"
  dataset_model_root="$MODEL_ROOT/$dataset"
  dataset_manifest_root="$MANIFEST_ROOT/$dataset"
  mkdir -p "$fold_data_dir" "$dataset_report" "$dataset_model_root" "$dataset_manifest_root"

  if [ "${SKIP_SPLIT:-0}" != "1" ]; then
    "$BIN" data kfold \
      --input "$GOLD" \
      --output-dir "$fold_data_dir" \
      --folds "$FOLDS" \
      --dev-ratio "$DEV_RATIO" \
      --seed "multilingual_5fold_${dataset}_v1"
  fi

  for ((fold = 0; fold < FOLDS; fold++)); do
    fold_name="fold-$fold"
    train="$fold_data_dir/$fold_name/train.jsonl.zst"
    test="$fold_data_dir/$fold_name/test.jsonl.zst"
    manifest="$dataset_manifest_root/$fold_name.toml"
    fold_model_dir="$dataset_model_root/$fold_name"
    fold_report="$dataset_report/$fold_name"
    mkdir -p "$fold_report"

    write_manifest "$train" "$manifest" "$fold_model_dir"

    matrix_args=(
      --manifest "$manifest" \
      --gold "$test" \
      --locale "$LOCALE" \
      --output-dir "$fold_report" \
      --iterations "$ITERATIONS" \
      --init-iterations "$INIT_ITERATIONS" \
      --init-warmup "$INIT_WARMUP"
    )
    if [ -n "$PATTERNS" ]; then
      matrix_args+=(--patterns "$PATTERNS")
    fi

    "$BIN" matrix "${matrix_args[@]}"
  done

  "$BIN" fold-summary \
    --input-dir "$dataset_report" \
    --output "$dataset_report/summary.md"

  if [ "$INCLUDE_HYPHER" = "1" ]; then
    fold_row "$dataset" "$dataset_report" "hypher" "hypher" >> "$fold_tsv"
  fi
  if [ "$INCLUDE_LIANG" = "1" ]; then
    fold_row "$dataset" "$dataset_report" "liang_tex" "liang_tex" >> "$fold_tsv"
  fi
  if [ "$ENABLE_LIBREOFFICE_BASELINE" = "1" ] && [ -n "$LIBREOFFICE_PATTERNS" ]; then
    fold_row "$dataset" "$dataset_report" "liang_libreoffice" "liang_libreoffice" >> "$fold_tsv"
  fi
  fold_row "$dataset" "$dataset_report" "$SELECTED_SLUG" "$SELECTED_SLUG" >> "$fold_tsv"
done

summarize_overall "$fold_tsv" "$REPORT_ROOT/summary.md" "$REPORT_ROOT/summary.tsv"
publish_summary

printf 'wrote %s\n' "$REPORT_ROOT/summary.md"
printf 'wrote %s\n' "$PUBLIC_REPORT_ROOT/summary.md"
