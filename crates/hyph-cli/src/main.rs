use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
// Compact integer-key maps used by learned rule tables and binary models.
mod hashing;
// User-facing prediction command, saved-model shortcuts, and gold rendering.
mod predict;
#[cfg(test)]
mod tests;

#[cfg(feature = "adapters-hyphenation")]
use hyph_adapters::HyphenationCrateAdapter;
use hyph_adapters::{adapter_for_method, MethodAdapter};
use hyph_core::{
    hyphenated_to_breaks, insert_separator, GraphemeIndex, HyphenationConfig, HyphenationRecord,
    LanguageTag,
};
use hyph_crf::{train_crf, CrfHyphenator, CrfTrainOptions};
use hyph_data::{
    import_moby, import_tsv, import_wiktextract, import_wlhamb, read_records, write_records,
    ImportMobyOptions, ImportTsvOptions, ImportWiktextractOptions, ImportWlhambOptions,
};
use hyph_eval::{
    evaluate_predictions, evaluate_predictions_report_with_policy, AmbiguousPolicy,
    EvaluationReport, MethodError, Metrics, PredictionErrorPolicy, WordError,
};
use hyph_patterns::{parse_pattern_file, LiangHyphenator};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::File,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command as ProcessCommand, Stdio},
    sync::Mutex,
    time::Instant,
};
use unicode_segmentation::UnicodeSegmentation;

use hashing::{mix_u64, U64HashMap, U64HashSet};
use predict::{cmd_predict, PredictArgs};

include!("cli.rs");
include!("typeset_fragments.rs");

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Data { command } => match command {
            DataCommand::ImportTsv(args) => cmd_import_tsv(args),
            DataCommand::ImportMoby(args) => cmd_import_moby(args),
            DataCommand::CurateTypeset(args) => cmd_curate_typeset(args),
            DataCommand::ImportWlhamb(args) => cmd_import_wlhamb(args),
            DataCommand::ImportWiktextract(args) => cmd_import_wiktextract(args),
            DataCommand::ExportPatgen(args) => cmd_export_patgen(args),
            DataCommand::FilterScript(args) => cmd_filter_script(args),
            DataCommand::FilterQuality(args) => cmd_filter_quality(args),
            DataCommand::DedupVariants(args) => cmd_dedup_variants(args),
            DataCommand::Split(args) => cmd_split(args),
            DataCommand::Kfold(args) => cmd_kfold(args),
            DataCommand::Stats(args) => cmd_stats(args),
        },
        Command::Crf { command } => match command {
            CrfCommand::Train(args) => cmd_crf_train(args),
            CrfCommand::TuneThreshold(args) => cmd_crf_tune_threshold(args),
            CrfCommand::Convert(args) => cmd_crf_convert(args),
        },
        Command::Dev { command } => match command {
            DevCommand::NewAdapter(args) => cmd_dev_new_adapter(args),
            DevCommand::Smoke(args) => cmd_dev_smoke(args),
        },
        Command::Eval(args) => cmd_eval(args),
        Command::Compare(args) => cmd_compare(args),
        Command::Speed(args) => cmd_speed(args),
        Command::InitBench(args) => cmd_init_bench(args),
        Command::FoldSummary(args) => cmd_fold_summary(args),
        Command::CompileSafeNgram(args) => cmd_compile_safe_ngram(args),
        Command::CompileItalianSyllable(args) => cmd_compile_italian_syllable(args),
        Command::Matrix(args) => cmd_matrix(args),
        Command::Predict(args) => cmd_predict(args),
    }
}

include!("commands/data.rs");

include!("commands/dev.rs");

include!("commands/crf.rs");

include!("commands/compile.rs");

include!("commands/eval.rs");

include!("commands/compare_fold.rs");

include!("commands/bench_matrix_speed.rs");

include!("methods/types.rs");

include!("methods/model_io.rs");

include!("methods/safe_ngram_lookup.rs");

include!("methods/safe_ngram_method.rs");

include!("methods/italian.rs");

include!("methods/safe_ngram_train.rs");

include!("methods/safe_ngram_unicode.rs");

include!("methods/safe_ngram_ascii.rs");

include!("methods/safe_ngram_grapheme_kernels.rs");

include!("methods/safe_ngram_orthography.rs");

include!("methods/registry.rs");

include!("reports.rs");
