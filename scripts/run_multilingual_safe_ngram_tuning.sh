#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${BIN:-target/release/hyphlab}"
REPORT_ROOT="${REPORT_ROOT:-target/hyphlab-reports/multilingual/safe_ngram_tuned}"
SPLIT_ROOT="${SPLIT_ROOT:-target/hyphlab-splits}"
MODEL_ROOT="${MODEL_ROOT:-target/hyphlab-models/multilingual/safe_ngram_tuned}"
MANIFEST_ROOT="${MANIFEST_ROOT:-target/hyphlab-manifests/multilingual/safe_ngram_tuned}"
ITERATIONS="${ITERATIONS:-1}"
INIT_ITERATIONS="${INIT_ITERATIONS:-1}"
INIT_WARMUP="${INIT_WARMUP:-0}"
MIN_PRECISION="${MIN_PRECISION:-0.95}"
OBJECTIVE="${OBJECTIVE:-f1}"
SPEED_PARETO="${SPEED_PARETO:-1}"
SPEED_MAX_NS="${SPEED_MAX_NS:-}"
LEFT_MIN="${LEFT_MIN:-}"
RIGHT_MIN="${RIGHT_MIN:-}"
MIN_WORD_LEN="${MIN_WORD_LEN:-}"
DATASETS="${DATASETS:-moby_en_us wiktextract_cs wiktextract_de wiktextract_es wiktextract_it wiktextract_nl wiktextract_ru_cyrl_trusted_dedup wiktextract_tr}"
METHODS="${METHODS:-safe-ngram-unicode-2x2-s1-p50,safe-ngram-unicode-2x2-s1-p60,safe-ngram-unicode-2x2-s1-p70,safe-ngram-unicode-2x3-s1-p30,safe-ngram-unicode-2x3-s1-p40,safe-ngram-unicode-2x3-s1-p58,safe-ngram-unicode-3x2-s1-p60,safe-ngram-unicode-3x3-s1-p75,safe-ngram-unicode-3x3-s1-p85,safe-ngram-unicode-3x3-s1-p90,safe-ngram-unicode-3x4-s1-p30,safe-ngram-unicode-4x3-s1-p40,safe-ngram-unicode-3x4-s1-p60,safe-ngram-unicode-3x3-s1-p80,safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p80,safe-ngram-unicode-2x3-s1-p58-veto-unicode-3x4-s1-p85,safe-ngram-unicode-mixcv-2x2-s1-p70,safe-ngram-unicode-mixcv-2x3-s1-p75,safe-ngram-unicode-mixcv-3x3-s1-p80,safe-ngram-unicode-mixcv-3x3-s1-p85,safe-ngram-unicode-mixcv-2x3-s1-p58-veto-unicode-3x4-s1-p80,safe-ngram-unicode-mixcv-2x3-s1-p58-veto-unicode-mixcv-3x4-s1-p80}"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

mkdir -p "$REPORT_ROOT" "$SPLIT_ROOT" "$MODEL_ROOT" "$MANIFEST_ROOT"

dataset_config() {
  local dataset="$1"
  GOLD=""
  LOCALE=""
  PATTERNS=""

  case "$dataset" in
    moby_en_us)
      GOLD="data/gold/moby_en_us.jsonl.zst"
      LOCALE="en-US"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-en-us.tex"
      ;;
    wiktextract_cs)
      GOLD="data/gold/wiktextract/cs.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      ;;
    wiktextract_de)
      GOLD="data/gold/wiktextract/de.jsonl.zst"
      LOCALE="de"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      ;;
    wiktextract_es)
      GOLD="data/gold/wiktextract/es.jsonl.zst"
      LOCALE="es"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-es.tex"
      ;;
    wiktextract_it)
      GOLD="data/gold/wiktextract/it.jsonl.zst"
      LOCALE="it"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-it.tex"
      ;;
    wiktextract_nl)
      GOLD="data/gold/wiktextract/nl.jsonl.zst"
      LOCALE="nl"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-nl.tex"
      ;;
    wiktextract_ru)
      GOLD="data/gold/wiktextract/ru.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      ;;
    wiktextract_ru_dedup)
      GOLD="data/gold/wiktextract/ru_dedup.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      ;;
    wiktextract_ru_cyrl)
      GOLD="data/gold/wiktextract/ru_cyrl.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      ;;
    wiktextract_ru_cyrl_dedup)
      GOLD="data/gold/wiktextract/ru_cyrl_dedup.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      ;;
    wiktextract_ru_cyrl_trusted_dedup)
      GOLD="data/gold/wiktextract/ru_cyrl_trusted_dedup.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      ;;
    wiktextract_tr)
      GOLD="data/gold/wiktextract/tr.jsonl.zst"
      LOCALE="tr"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-tr.tex"
      ;;
    *)
      printf 'unknown dataset: %s\n' "$dataset" >&2
      exit 1
      ;;
  esac
}

method_slug() {
  printf '%s' "$1" | tr '-' '_' | tr -c '[:alnum:]_' '_'
}

metric_tsv() {
  local json="$1"
  local speed_json="$2"
  jq -r --slurpfile speed "$speed_json" '
    def div0($a;$b): if $b == 0 then 0 else ($a / $b) end;
    .metrics as $m |
    ($m.tp|tonumber) as $tp |
    ($m.fp|tonumber) as $fp |
    ($m.fn_|tonumber) as $fn |
    ($m.tn|tonumber) as $tn |
    div0($tp; ($tp+$fp)) as $p |
    div0($tp; ($tp+$fn)) as $r |
    div0(2*$p*$r; ($p+$r)) as $f1 |
    div0(1.25*$p*$r; (0.25*$p+$r)) as $f05 |
    div0($m.exact_words; $m.words) as $exact |
    div0($m.serious_word_errors; $m.words) as $serious |
    [ .method, $m.words, $p, $r, $f1, $f05, $exact, $serious, $speed[0].ns_per_word, $speed[0].words_per_sec ] | @tsv
  ' "$json"
}

best_safe_slug() {
  local dir="$1"
  local tab
  tab="$(printf '\t')"
  local rows
  rows="$(
    for json in "$dir"/safe_ngram*.json; do
      [ -f "$json" ] || continue
      local slug
      local speed_json
      slug="$(basename "$json" .json)"
      speed_json="$dir/speed/$slug.json"
      [ -f "$speed_json" ] || continue
      jq -r --arg slug "$slug" --arg objective "$OBJECTIVE" --argjson min_precision "$MIN_PRECISION" --slurpfile speed "$speed_json" '
        def div0($a;$b): if $b == 0 then 0 else ($a / $b) end;
        .metrics as $m |
        ($m.tp|tonumber) as $tp |
        ($m.fp|tonumber) as $fp |
        ($m.fn_|tonumber) as $fn |
        div0($m.serious_word_errors; $m.words) as $serious |
        div0($tp; ($tp+$fp)) as $p |
        div0($tp; ($tp+$fn)) as $r |
        div0(2*$p*$r; ($p+$r)) as $f1 |
        div0(1.25*$p*$r; (0.25*$p+$r)) as $f05 |
        div0($m.exact_words; $m.words) as $exact |
        ($speed[0].ns_per_word // 1000000000000) as $ns |
        (if $objective == "serious" or $objective == "serious_error" then -$serious
         elif $objective == "precision" then $p
         elif $objective == "recall" then $r
         elif $objective == "f0.5" or $objective == "f05" then $f05
         else $f1 end) as $base_score |
        (if $p >= $min_precision then $base_score else -1.0 + $base_score end) as $score |
        [$slug, $score, $p, $r, $f1, $f05, $exact, $serious, $ns] | @tsv
      ' "$json"
    done
  )"
  if [ -z "$rows" ]; then
    return 1
  fi

  if [ -n "$SPEED_MAX_NS" ]; then
    local capped
    capped="$(
      printf '%s\n' "$rows" |
        awk -F "$tab" -v max_ns="$SPEED_MAX_NS" -v min_p="$MIN_PRECISION" \
          '$9 <= max_ns && $3 >= min_p { print }' |
        sort -t "$tab" -k2,2gr -k9,9g |
        head -n 1
    )"
    if [ -n "$capped" ]; then
      printf '%s\n' "$capped" | cut -f1
      return 0
    fi
  fi

  local incumbent
  incumbent="$(printf '%s\n' "$rows" | sort -t "$tab" -k2,2gr -k9,9g | head -n 1)"
  if [ "$SPEED_PARETO" = "1" ]; then
    local incumbent_slug incumbent_score incumbent_p incumbent_r incumbent_f1 incumbent_f05 incumbent_exact incumbent_serious incumbent_ns
    IFS="$tab" read -r incumbent_slug incumbent_score incumbent_p incumbent_r incumbent_f1 incumbent_f05 incumbent_exact incumbent_serious incumbent_ns <<< "$incumbent"
    local fastest
    fastest="$(
      printf '%s\n' "$rows" |
        awk -F "$tab" \
          -v p="$incumbent_p" \
          -v r="$incumbent_r" \
          -v f1="$incumbent_f1" \
          -v f05="$incumbent_f05" \
          -v exact="$incumbent_exact" \
          -v serious="$incumbent_serious" \
          'BEGIN { eps = 1e-12 }
           $3 + eps >= p && $4 + eps >= r && $5 + eps >= f1 && $6 + eps >= f05 && $7 + eps >= exact && $8 <= serious + eps { print }' |
        sort -t "$tab" -k9,9g |
        head -n 1
    )"
    if [ -n "$fastest" ]; then
      printf '%s\n' "$fastest" | cut -f1
      return 0
    fi
  fi
  printf '%s\n' "$incumbent" | cut -f1
}

write_manifest() {
  local dataset="$1"
  local train="$2"
  local manifest="$3"
  local model_dir="$4"

  {
    printf '[[methods]]\n'
    printf 'slug = "hypher"\n'
    printf 'method = "hypher"\n\n'
    printf '[[methods]]\n'
    printf 'slug = "liang_tex"\n'
    printf 'method = "liang"\n'
    printf 'requires_patterns = true\n\n'
  } > "$manifest"

  IFS=',' read -r -a method_list <<< "$METHODS"
  for method in "${method_list[@]}"; do
    local slug
    local model
    slug="$(method_slug "$method")"
    model="$(pwd)/$model_dir/$slug.bin"
    local compile_args=(method train --method "$method" --gold "$train" --locale "$LOCALE" --output "$model")
    if [ -n "$LEFT_MIN" ]; then
      compile_args+=(--left-min "$LEFT_MIN")
    fi
    if [ -n "$RIGHT_MIN" ]; then
      compile_args+=(--right-min "$RIGHT_MIN")
    fi
    if [ -n "$MIN_WORD_LEN" ]; then
      compile_args+=(--min-word-len "$MIN_WORD_LEN")
    fi
    "$BIN" "${compile_args[@]}"
    {
      printf '[[methods]]\n'
      printf 'slug = "%s"\n' "$slug"
      printf 'method = "safe-ngram-model"\n'
      printf 'dictionary = "%s"\n' "$model"
      printf '\n'
    } >> "$manifest"
  done

  printf 'wrote manifest for %s: %s\n' "$dataset" "$manifest"
}

summary_tsv="$REPORT_ROOT/summary.tsv"
{
  printf 'dataset\tselected_slug\tmethod\twords\tprecision\trecall\tf1\tf0.5\texact\tserious_error\tns_per_word\twords_per_sec\tbaseline_method\tbaseline_precision\tbaseline_recall\tbaseline_f1\tbaseline_serious_error\tbaseline_ns_per_word\n'
} > "$summary_tsv"

for dataset in $DATASETS; do
  dataset_config "$dataset"
  if [ ! -s "$GOLD" ]; then
    printf 'skip %s: missing or empty gold %s\n' "$dataset" "$GOLD"
    continue
  fi

  split_dir="$SPLIT_ROOT/$dataset"
  train="$split_dir/train.jsonl.zst"
  dev="$split_dir/dev.jsonl.zst"
  test="$split_dir/test.jsonl.zst"
  if [ "${SKIP_SPLIT:-0}" != "1" ] || [ ! -f "$test" ]; then
    "$BIN" data split \
      --input "$GOLD" \
      --output-dir "$split_dir" \
      --train-ratio 0.8 \
      --dev-ratio 0.1 \
      --test-ratio 0.1 \
      --seed "${dataset}_multilingual_tuning_v1"
  fi

  dataset_report="$REPORT_ROOT/$dataset"
  dataset_model="$MODEL_ROOT/$dataset"
  dataset_manifest="$MANIFEST_ROOT/$dataset.toml"
  mkdir -p "$dataset_report" "$dataset_model" "$(dirname "$dataset_manifest")"

  write_manifest "$dataset" "$train" "$dataset_manifest" "$dataset_model"

  "$BIN" matrix \
    --manifest "$dataset_manifest" \
    --gold "$dev" \
    --locale "$LOCALE" \
    --patterns "$PATTERNS" \
    --output-dir "$dataset_report/dev" \
    --iterations "$ITERATIONS" \
    --init-iterations "$INIT_ITERATIONS" \
    --init-warmup "$INIT_WARMUP"

  selected_slug="$(best_safe_slug "$dataset_report/dev")"
  printf '%s\n' "$selected_slug" > "$dataset_report/selected_slug.txt"

  "$BIN" matrix \
    --manifest "$dataset_manifest" \
    --gold "$test" \
    --locale "$LOCALE" \
    --patterns "$PATTERNS" \
    --output-dir "$dataset_report/test" \
    --iterations "$ITERATIONS" \
    --init-iterations "$INIT_ITERATIONS" \
    --init-warmup "$INIT_WARMUP"

  selected_metrics="$(metric_tsv "$dataset_report/test/$selected_slug.json" "$dataset_report/test/speed/$selected_slug.json")"
  hypher_metrics="$(metric_tsv "$dataset_report/test/hypher.json" "$dataset_report/test/speed/hypher.json")"
  printf '%s\t%s\t%s\t%s\n' "$dataset" "$selected_slug" "$selected_metrics" "$(printf '%s' "$hypher_metrics" | cut -f1,3,4,5,8,9)" >> "$summary_tsv"
done

summary_md="$REPORT_ROOT/summary.md"
{
  printf '# Multilingual safe-ngram tuning\n\n'
  printf 'Selection policy: choose the best safe-ngram candidate on each dev split using `%s`, with precision below `%s` penalized.' "$OBJECTIVE" "$MIN_PRECISION"
  if [ -n "$SPEED_MAX_NS" ]; then
    printf ' If any safe-ngram candidate is at or below `%s` ns/word on dev and meets the precision floor, choose the best such candidate by `%s`.' "$SPEED_MAX_NS" "$OBJECTIVE"
  fi
  if [ "$SPEED_PARETO" = "1" ]; then
    printf ' Then choose the fastest dev candidate that does not lower precision, recall, f1, f0.5, or exact words, and does not raise serious_error relative to that candidate.'
  fi
  printf ' Report the selected method on the held-out test split.\n\n'
  printf '| dataset | selected | precision | recall | f1 | f0.5 | exact | serious_error | ns/word | Hypher precision | Hypher recall | Hypher f1 | Hypher serious_error | Hypher ns/word |\n'
  printf '| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n'
  tail -n +2 "$summary_tsv" | while IFS="$(printf '\t')" read -r dataset selected method words p r f1 f05 exact serious ns wps base_method bp br bf1 bserious bns; do
    printf '| %s | `%s` | %.6f | %.6f | %.6f | %.6f | %.6f | %.6f | %.3f | %.6f | %.6f | %.6f | %.6f | %.3f |\n' \
      "$dataset" "$selected" "$p" "$r" "$f1" "$f05" "$exact" "$serious" "$ns" "$bp" "$br" "$bf1" "$bserious" "$bns"
  done
} > "$summary_md"

printf 'wrote %s\n' "$summary_md"
