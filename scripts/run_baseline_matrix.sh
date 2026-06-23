#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

REPORT_ROOT="${REPORT_ROOT:-target/hyphlab-reports/baselines}"
ITERATIONS="${ITERATIONS:-1}"
INIT_ITERATIONS="${INIT_ITERATIONS:-1}"
INIT_WARMUP="${INIT_WARMUP:-0}"
BIN="${BIN:-target/release/hyphlab}"
METHODS_MANIFEST="${METHODS_MANIFEST:-methods.toml}"
DATASETS="${DATASETS:-moby_en_us hyph_bench_cs_cstenten hyph_bench_cs_ujc hyph_bench_cssk_cshyphen hyph_bench_de_wortliste hyph_bench_is_hyphis hyph_bench_th_orchid hyph_bench_uk_wiktionary wiktextract_cs wiktextract_de wiktextract_es wiktextract_it wiktextract_nl wiktextract_ru wiktextract_th wiktextract_tr}"

mkdir -p "$REPORT_ROOT"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  cargo build -p hyph-cli --release --features adapters-hyphenation-embedded
fi

dataset_config() {
  local dataset="$1"
  GOLD=""
  LOCALE=""
  PATTERNS=""
  LABEL="$dataset"

  case "$dataset" in
    moby_en_us)
      GOLD="data/gold/moby_en_us.jsonl.zst"
      LOCALE="en-US"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-en-us.tex"
      LABEL="Moby English US"
      ;;
    hyph_bench_cs_cstenten)
      GOLD="data/gold/hyph_bench/cs_cstenten.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      LABEL="hyph-bench cs_cstenten"
      ;;
    hyph_bench_cs_ujc)
      GOLD="data/gold/hyph_bench/cs_ujc.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      LABEL="hyph-bench cs_ujc"
      ;;
    hyph_bench_cssk_cshyphen)
      GOLD="data/gold/hyph_bench/cssk_cshyphen.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      LABEL="hyph-bench cssk_cshyphen"
      ;;
    hyph_bench_de_wortliste)
      GOLD="data/gold/hyph_bench/de_wortliste.jsonl.zst"
      LOCALE="de-DE"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      LABEL="hyph-bench de_wortliste"
      ;;
    hyph_bench_is_hyphis)
      GOLD="data/gold/hyph_bench/is_hyphis.jsonl.zst"
      LOCALE="is-IS"
      PATTERNS=""
      LABEL="hyph-bench is_hyphis"
      ;;
    hyph_bench_th_orchid)
      GOLD="data/gold/hyph_bench/th_orchid.jsonl.zst"
      LOCALE="th-TH"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-th.tex"
      LABEL="hyph-bench th_orchid"
      ;;
    hyph_bench_uk_wiktionary)
      GOLD="data/gold/hyph_bench/uk_wiktionary.jsonl.zst"
      LOCALE="uk-UA"
      PATTERNS=""
      LABEL="hyph-bench uk_wiktionary"
      ;;
    wiktextract_cs)
      GOLD="data/gold/wiktextract/cs.jsonl.zst"
      LOCALE="cs"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-cs.tex"
      LABEL="Wiktextract cs"
      ;;
    wiktextract_de)
      GOLD="data/gold/wiktextract/de.jsonl.zst"
      LOCALE="de"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-de-1996.tex"
      LABEL="Wiktextract de"
      ;;
    wiktextract_es)
      GOLD="data/gold/wiktextract/es.jsonl.zst"
      LOCALE="es"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-es.tex"
      LABEL="Wiktextract es"
      ;;
    wiktextract_it)
      GOLD="data/gold/wiktextract/it.jsonl.zst"
      LOCALE="it"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-it.tex"
      LABEL="Wiktextract it"
      ;;
    wiktextract_nl)
      GOLD="data/gold/wiktextract/nl.jsonl.zst"
      LOCALE="nl"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-nl.tex"
      LABEL="Wiktextract nl"
      ;;
    wiktextract_ru)
      GOLD="data/gold/wiktextract/ru.jsonl.zst"
      LOCALE="ru"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-ru.tex"
      LABEL="Wiktextract ru"
      ;;
    wiktextract_th)
      GOLD="data/gold/wiktextract/th.jsonl.zst"
      LOCALE="th"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-th.tex"
      LABEL="Wiktextract th"
      ;;
    wiktextract_tr)
      GOLD="data/gold/wiktextract/tr.jsonl.zst"
      LOCALE="tr"
      PATTERNS="data/patterns/tex-hyphen/tex/hyph-tr.tex"
      LABEL="Wiktextract tr"
      ;;
    *)
      printf 'unknown dataset: %s\n' "$dataset" >&2
      exit 1
      ;;
  esac
}

run_dataset() {
  local dataset="$1"
  dataset_config "$dataset"
  if [ ! -s "$GOLD" ]; then
    printf 'skip %s: missing or empty gold %s\n' "$dataset" "$GOLD"
    return
  fi

  local dataset_dir="$REPORT_ROOT/$dataset"
  local matrix_args=(
    --manifest "$METHODS_MANIFEST"
    --gold "$GOLD"
    --locale "$LOCALE"
    --output-dir "$dataset_dir"
    --iterations "$ITERATIONS"
    --init-iterations "$INIT_ITERATIONS"
    --init-warmup "$INIT_WARMUP"
  )
  if [ -n "$PATTERNS" ]; then
    matrix_args+=(--patterns "$PATTERNS")
  fi

  printf '\n== %s ==\n' "$LABEL"
  printf 'gold=%s locale=%s patterns=%s\n' "$GOLD" "$LOCALE" "${PATTERNS:-none}"
  "$BIN" matrix "${matrix_args[@]}"

  printf 'wrote %s/compare.md\n' "$dataset_dir"
}

for dataset in $DATASETS
do
  run_dataset "$dataset"
done

{
  printf '# Baseline Matrix\n\n'
  printf 'Evaluation policy: full normalized gold corpus for each listed dataset; no train/dev/test split is used by this baseline matrix.\n\n'
  printf 'Release binary: `%s`\n\n' "$BIN"
  printf 'Method manifest: `%s`\n\n' "$METHODS_MANIFEST"
  printf 'Iterations per steady-state speed run: `%s`\n\n' "$ITERATIONS"
  printf 'Iterations per init/load run: `%s`\n\n' "$INIT_ITERATIONS"
  for dataset in $DATASETS
  do
    if [ -f "$REPORT_ROOT/$dataset/compare.md" ]; then
      printf '%s\n' "- [$dataset]($dataset/compare.md)"
    fi
  done
} > "$REPORT_ROOT/index.md"

printf '\nwrote %s/index.md\n' "$REPORT_ROOT"
