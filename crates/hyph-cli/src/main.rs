use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
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
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::File,
    hash::{BuildHasherDefault, Hasher},
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command as ProcessCommand, Stdio},
    sync::Mutex,
    time::Instant,
};
use unicode_segmentation::UnicodeSegmentation;

type U64HashMap<V> = HashMap<u64, V, BuildHasherDefault<IdentityHasher>>;
type U64HashSet = HashSet<u64, BuildHasherDefault<IdentityHasher>>;

#[derive(Debug, Default)]
struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut value = 0u64;
        for (shift, byte) in bytes.iter().take(8).enumerate() {
            value |= (*byte as u64) << (shift * 8);
        }
        self.0 = mix_u64(value);
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = mix_u64(value);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

fn mix_u64(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51afd7ed558ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ceb9fe1a85ec53);
    value ^ (value >> 33)
}

#[derive(Debug, Parser)]
#[command(name = "hyphlab")]
#[command(about = "Hyphenation research CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Data {
        #[command(subcommand)]
        command: DataCommand,
    },
    Crf {
        #[command(subcommand)]
        command: CrfCommand,
    },
    Dev {
        #[command(subcommand)]
        command: DevCommand,
    },
    Eval(EvalArgs),
    Compare(CompareArgs),
    #[command(name = "candidate-oracle")]
    CandidateOracle(CandidateOracleArgs),
    Speed(SpeedArgs),
    #[command(name = "init-bench")]
    InitBench(InitBenchArgs),
    #[command(name = "fold-summary")]
    FoldSummary(FoldSummaryArgs),
    #[command(name = "compile-safe-ngram")]
    CompileSafeNgram(CompileSafeNgramArgs),
    #[command(name = "compile-italian-syllable")]
    CompileItalianSyllable(CompileItalianSyllableArgs),
    Matrix(MatrixArgs),
    Predict(PredictArgs),
}

#[derive(Debug, Subcommand)]
enum DataCommand {
    ImportTsv(ImportTsvArgs),
    ImportMoby(ImportMobyArgs),
    ImportWlhamb(ImportWlhambArgs),
    ImportWiktextract(ImportWiktextractArgs),
    ExportPatgen(ExportPatgenArgs),
    #[command(name = "filter-script")]
    FilterScript(FilterScriptArgs),
    #[command(name = "filter-quality")]
    FilterQuality(FilterQualityArgs),
    #[command(name = "dedup-variants")]
    DedupVariants(DedupVariantsArgs),
    Split(SplitArgs),
    Kfold(KfoldArgs),
    Stats(StatsArgs),
}

#[derive(Debug, Subcommand)]
enum DevCommand {
    NewAdapter(NewAdapterArgs),
    Smoke(SmokeArgs),
}

#[derive(Debug, Subcommand)]
enum CrfCommand {
    Train(CrfTrainArgs),
    TuneThreshold(CrfTuneThresholdArgs),
    Convert(CrfConvertArgs),
}

#[derive(Debug, Parser)]
struct ImportTsvArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    locale: Option<String>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    license: Option<String>,
}

#[derive(Debug, Parser)]
struct ImportMobyArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long, default_value = "en-US")]
    locale: String,
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    license: Option<String>,
    #[arg(long, default_value = "0xA5")]
    separator: String,
}

#[derive(Debug, Parser)]
struct ImportWlhambArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    locale: Option<String>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    license: Option<String>,
    #[arg(long)]
    skip_invalid: bool,
}

#[derive(Debug, Parser)]
struct ImportWiktextractArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    locale: Option<String>,
    #[arg(long)]
    filter_lang_code: Option<String>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    license: Option<String>,
    #[arg(long)]
    skip_invalid: bool,
}

#[derive(Debug, Parser)]
struct StatsArgs {
    #[arg(short, long)]
    input: PathBuf,
}

#[derive(Debug, Parser)]
struct ExportPatgenArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long, default_value = "-")]
    separator: String,
    #[arg(long)]
    include_ambiguous: bool,
    #[arg(long)]
    ascii_alpha_only: bool,
    #[arg(long)]
    preserve_case: bool,
}

#[derive(Debug, Parser)]
struct FilterScriptArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long, value_enum)]
    script: ScriptFilterArg,
    #[arg(long)]
    include_ambiguous: bool,
}

#[derive(Debug, Parser)]
struct FilterQualityArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    drop_long_no_break: bool,
    #[arg(long, default_value_t = 5)]
    min_graphemes: usize,
    #[arg(long, default_value_t = 2)]
    min_vowels: usize,
}

#[derive(Debug, Parser)]
struct DedupVariantsArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ScriptFilterArg {
    Cyrillic,
    RussianCyrillic,
    Latin,
}

#[derive(Debug, Parser)]
struct SplitArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output_dir: PathBuf,
    #[arg(long, default_value_t = 0.8)]
    train_ratio: f64,
    #[arg(long, default_value_t = 0.1)]
    dev_ratio: f64,
    #[arg(long, default_value_t = 0.1)]
    test_ratio: f64,
    #[arg(long, default_value = "hyphlab-split-v1")]
    seed: String,
}

#[derive(Debug, Parser)]
struct KfoldArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output_dir: PathBuf,
    #[arg(long, default_value_t = 5)]
    folds: usize,
    #[arg(long, default_value = "hyphlab-kfold-v1")]
    seed: String,
    #[arg(long, default_value_t = 0.0)]
    dev_ratio: f64,
}

#[derive(Debug, Parser)]
struct NewAdapterArgs {
    slug: String,
    #[arg(long)]
    method: Option<String>,
    #[arg(long)]
    struct_name: Option<String>,
    #[arg(long, value_delimiter = ',')]
    supports: Vec<String>,
    #[arg(long)]
    requires_patterns: bool,
    #[arg(long)]
    pass_patterns: bool,
    #[arg(long)]
    requires_feature: Option<String>,
    #[arg(long, default_value = "methods.toml")]
    manifest: PathBuf,
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Parser)]
struct SmokeArgs {
    slug: String,
    #[arg(long, default_value = "methods.toml")]
    manifest: PathBuf,
    #[arg(long, default_value = "data/gold/toy_en.jsonl")]
    gold: PathBuf,
    #[arg(long, default_value = "en-US")]
    locale: String,
    #[arg(long, default_value = "tests/fixtures/toy_en.patterns")]
    patterns: PathBuf,
    #[arg(long, default_value = "target/hyphlab-reports/dev-smoke")]
    output_dir: PathBuf,
    #[arg(long, default_value_t = 1)]
    iterations: usize,
    #[arg(long, default_value_t = 1)]
    init_iterations: usize,
}

#[derive(Debug, Parser)]
struct CrfTrainArgs {
    #[arg(short, long)]
    gold: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(short, long, default_value = "en-US")]
    locale: String,
    #[arg(long, default_value = "trogkanis-elkan-crf")]
    id: String,
    #[arg(long, default_value_t = 5)]
    epochs: usize,
    #[arg(long, default_value_t = 0.05)]
    learning_rate: f32,
    #[arg(long, default_value_t = 1.0e-5)]
    l2: f32,
    #[arg(long, default_value_t = 0.9)]
    threshold: f32,
    #[arg(long, default_value_t = 2)]
    min_n: usize,
    #[arg(long, default_value_t = 5)]
    max_n: usize,
    #[arg(long)]
    left_min: Option<usize>,
    #[arg(long)]
    right_min: Option<usize>,
    #[arg(long)]
    min_word_len: Option<usize>,
    #[arg(long)]
    limit: Option<usize>,
    #[arg(long)]
    include_ambiguous: bool,
}

#[derive(Debug, Parser)]
struct CrfTuneThresholdArgs {
    #[arg(short, long)]
    model: PathBuf,
    #[arg(short, long)]
    gold: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long)]
    report: Option<PathBuf>,
    #[arg(long, default_value_t = 0.5)]
    min: f32,
    #[arg(long, default_value_t = 0.99)]
    max: f32,
    #[arg(long, default_value_t = 0.01)]
    step: f32,
    #[arg(long, value_enum, default_value = "f05")]
    objective: ThresholdObjectiveArg,
    #[arg(long, value_enum, default_value = "exclude")]
    ambiguous: AmbiguousPolicyArg,
    #[arg(long)]
    id: Option<String>,
}

#[derive(Debug, Parser)]
struct CrfConvertArgs {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    threshold: Option<f32>,
    #[arg(long)]
    id: Option<String>,
}

#[derive(Debug, Parser)]
struct CompileSafeNgramArgs {
    #[arg(short, long)]
    gold: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(short, long, default_value = "en-US")]
    locale: String,
    #[arg(long, default_value = "safe-ngram-multi-s1-n1")]
    method: String,
    #[arg(long)]
    left_min: Option<usize>,
    #[arg(long)]
    right_min: Option<usize>,
    #[arg(long)]
    min_word_len: Option<usize>,
    #[arg(long)]
    include_ambiguous: bool,
}

#[derive(Debug, Parser)]
struct CompileItalianSyllableArgs {
    #[arg(short, long)]
    gold: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(short, long, default_value = "it")]
    locale: String,
    #[arg(long, default_value = "italian-syllable")]
    method: String,
    #[arg(long)]
    left_min: Option<usize>,
    #[arg(long)]
    right_min: Option<usize>,
    #[arg(long)]
    min_word_len: Option<usize>,
    #[arg(long)]
    include_ambiguous: bool,
}

#[derive(Debug, Parser)]
struct EvalArgs {
    #[arg(short, long)]
    gold: PathBuf,
    #[arg(short, long, default_value = "hypher")]
    method: String,
    #[arg(short, long, default_value = "en-US")]
    locale: String,
    #[arg(long)]
    patterns: Option<PathBuf>,
    #[arg(long)]
    dictionary: Option<PathBuf>,
    #[arg(long)]
    external_command: Option<String>,
    #[arg(long)]
    left_min: Option<usize>,
    #[arg(long)]
    right_min: Option<usize>,
    #[arg(long)]
    min_word_len: Option<usize>,
    #[arg(long, value_enum, default_value = "exclude")]
    ambiguous: AmbiguousPolicyArg,
    #[arg(long)]
    json: bool,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long)]
    errors_output: Option<PathBuf>,
    #[arg(long)]
    skip_method_errors: bool,
    #[arg(long)]
    method_errors_output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct CompareArgs {
    #[arg(short, long, required = true)]
    input: Vec<PathBuf>,
    #[arg(long = "speed-input")]
    speed_input: Vec<PathBuf>,
    #[arg(long = "init-input")]
    init_input: Vec<PathBuf>,
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct FoldSummaryArgs {
    #[arg(short, long)]
    input_dir: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct SpeedArgs {
    #[arg(short, long)]
    gold: PathBuf,
    #[arg(short, long, default_value = "hypher")]
    method: String,
    #[arg(short, long, default_value = "en-US")]
    locale: String,
    #[arg(long)]
    patterns: Option<PathBuf>,
    #[arg(long)]
    dictionary: Option<PathBuf>,
    #[arg(long)]
    external_command: Option<String>,
    #[arg(long, default_value_t = 10)]
    iterations: usize,
    #[arg(long, default_value_t = 1)]
    warmup: usize,
    #[arg(long)]
    left_min: Option<usize>,
    #[arg(long)]
    right_min: Option<usize>,
    #[arg(long)]
    min_word_len: Option<usize>,
    #[arg(long, value_enum, default_value = "exclude")]
    ambiguous: AmbiguousPolicyArg,
    #[arg(long)]
    json: bool,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long)]
    skip_method_errors: bool,
    #[arg(long)]
    method_errors_output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct InitBenchArgs {
    #[arg(short, long, default_value = "hypher")]
    method: String,
    #[arg(short, long, default_value = "en-US")]
    locale: String,
    #[arg(long)]
    patterns: Option<PathBuf>,
    #[arg(long)]
    dictionary: Option<PathBuf>,
    #[arg(long)]
    gold: Option<PathBuf>,
    #[arg(long)]
    external_command: Option<String>,
    #[arg(long, default_value_t = 10)]
    iterations: usize,
    #[arg(long, default_value_t = 1)]
    warmup: usize,
    #[arg(long)]
    left_min: Option<usize>,
    #[arg(long)]
    right_min: Option<usize>,
    #[arg(long)]
    min_word_len: Option<usize>,
    #[arg(long)]
    json: bool,
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct MatrixArgs {
    #[arg(long, default_value = "methods.toml")]
    manifest: PathBuf,
    #[arg(short, long)]
    gold: PathBuf,
    #[arg(short, long, default_value = "en-US")]
    locale: String,
    #[arg(long)]
    patterns: Option<PathBuf>,
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long, default_value_t = 1)]
    iterations: usize,
    #[arg(long, default_value_t = 1)]
    init_iterations: usize,
    #[arg(long, default_value_t = 0)]
    init_warmup: usize,
    #[arg(long, value_enum, default_value = "exclude")]
    ambiguous: AmbiguousPolicyArg,
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
    #[arg(long)]
    abort_method_errors: bool,
}

#[derive(Debug, Deserialize)]
struct MethodsManifest {
    methods: Vec<ManifestMethod>,
}

#[derive(Debug, Deserialize)]
struct ManifestMethod {
    slug: String,
    method: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    supports: Vec<String>,
    #[serde(default)]
    requires_feature: Option<String>,
    #[serde(default)]
    requires_patterns: bool,
    #[serde(default)]
    pass_patterns: bool,
    #[serde(default)]
    patterns: Option<PathBuf>,
    #[serde(default)]
    dictionary: Option<PathBuf>,
    #[serde(default)]
    external_command: Option<String>,
    #[serde(default)]
    left_min: Option<usize>,
    #[serde(default)]
    right_min: Option<usize>,
    #[serde(default)]
    min_word_len: Option<usize>,
}

#[derive(Debug, Parser)]
struct PredictArgs {
    #[arg(short, long, default_value = "hypher")]
    method: String,
    #[arg(short, long, default_value = "en-US")]
    locale: String,
    #[arg(long, value_name = "KEY")]
    saved_model: Option<String>,
    #[arg(long)]
    list_saved_models: bool,
    #[arg(long)]
    patterns: Option<PathBuf>,
    #[arg(long)]
    dictionary: Option<PathBuf>,
    #[arg(long)]
    external_command: Option<String>,
    #[arg(long)]
    left_min: Option<usize>,
    #[arg(long)]
    right_min: Option<usize>,
    #[arg(long)]
    min_word_len: Option<usize>,
    #[arg(short, long)]
    input: Option<PathBuf>,
    #[arg(long, value_name = "WORD")]
    word: Vec<String>,
    #[arg(long, alias = "sentence", value_name = "TEXT")]
    text: Vec<String>,
    #[arg(long, default_value = "-")]
    separator: String,
    #[arg(long)]
    show_breaks: bool,
}

#[derive(Debug, Parser)]
struct CandidateOracleArgs {
    #[arg(long)]
    gold: PathBuf,
    #[arg(short, long, default_value = "en-US")]
    locale: String,
    #[arg(long)]
    patterns: Option<PathBuf>,
    #[arg(long)]
    dictionary: Option<PathBuf>,
    #[arg(long)]
    left_min: Option<usize>,
    #[arg(long)]
    right_min: Option<usize>,
    #[arg(long)]
    min_word_len: Option<usize>,
    #[arg(long, value_delimiter = ',')]
    methods: Vec<String>,
    #[arg(long, default_value_t = 950_000)]
    target_precision_ppm: u32,
    #[arg(long, default_value_t = 950_000)]
    target_recall_ppm: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AmbiguousPolicyArg {
    Exclude,
    First,
    Union,
    Intersection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ThresholdObjectiveArg {
    F1,
    F05,
    Precision,
    Recall,
    Exact,
}

impl From<AmbiguousPolicyArg> for AmbiguousPolicy {
    fn from(value: AmbiguousPolicyArg) -> Self {
        match value {
            AmbiguousPolicyArg::Exclude => Self::Exclude,
            AmbiguousPolicyArg::First => Self::First,
            AmbiguousPolicyArg::Union => Self::Union,
            AmbiguousPolicyArg::Intersection => Self::Intersection,
        }
    }
}

impl AmbiguousPolicyArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Exclude => "exclude",
            Self::First => "first",
            Self::Union => "union",
            Self::Intersection => "intersection",
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Data { command } => match command {
            DataCommand::ImportTsv(args) => cmd_import_tsv(args),
            DataCommand::ImportMoby(args) => cmd_import_moby(args),
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
        Command::CandidateOracle(args) => cmd_candidate_oracle(args),
        Command::Speed(args) => cmd_speed(args),
        Command::InitBench(args) => cmd_init_bench(args),
        Command::FoldSummary(args) => cmd_fold_summary(args),
        Command::CompileSafeNgram(args) => cmd_compile_safe_ngram(args),
        Command::CompileItalianSyllable(args) => cmd_compile_italian_syllable(args),
        Command::Matrix(args) => cmd_matrix(args),
        Command::Predict(args) => cmd_predict(args),
    }
}

fn cmd_import_tsv(args: ImportTsvArgs) -> Result<()> {
    let count = import_tsv(ImportTsvOptions {
        input: args.input,
        output: args.output.clone(),
        locale: args.locale,
        source: args.source,
        license: args.license,
    })?;
    println!("imported {count} records -> {}", args.output.display());
    Ok(())
}

fn cmd_import_moby(args: ImportMobyArgs) -> Result<()> {
    let separator = parse_byte(&args.separator)?;
    let count = import_moby(ImportMobyOptions {
        input: args.input,
        output: args.output.clone(),
        locale: Some(args.locale),
        source: args.source,
        license: args.license,
        separator,
    })?;
    println!("imported {count} records -> {}", args.output.display());
    Ok(())
}

fn cmd_import_wlhamb(args: ImportWlhambArgs) -> Result<()> {
    let report = import_wlhamb(ImportWlhambOptions {
        input: args.input,
        output: args.output.clone(),
        locale: args.locale,
        source: args.source,
        license: args.license,
        skip_invalid: args.skip_invalid,
    })?;
    println!(
        "imported {} records -> {}",
        report.records,
        args.output.display()
    );
    if report.skipped_invalid > 0 {
        println!("skipped_invalid: {}", report.skipped_invalid);
    }
    Ok(())
}

fn cmd_import_wiktextract(args: ImportWiktextractArgs) -> Result<()> {
    let report = import_wiktextract(ImportWiktextractOptions {
        input: args.input,
        output: args.output.clone(),
        locale: args.locale,
        filter_lang_code: args.filter_lang_code,
        source: args.source,
        license: args.license,
        skip_invalid: args.skip_invalid,
    })?;
    println!(
        "imported {} records -> {}",
        report.records,
        args.output.display()
    );
    println!("lines: {}", report.lines);
    if report.skipped_lang_code > 0 {
        println!("skipped_lang_code: {}", report.skipped_lang_code);
    }
    println!("skipped_no_hyphenation: {}", report.skipped_no_hyphenation);
    if report.skipped_invalid > 0 {
        println!("skipped_invalid: {}", report.skipped_invalid);
    }
    Ok(())
}

fn cmd_export_patgen(args: ExportPatgenArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    create_parent(&args.output)?;
    let file =
        File::create(&args.output).with_context(|| format!("create {}", args.output.display()))?;
    let mut writer = BufWriter::new(file);
    let mut emitted = BTreeSet::new();
    let mut skipped_ambiguous = 0usize;
    let mut skipped_non_alpha = 0usize;

    for record in records {
        if record.ambiguous && !args.include_ambiguous {
            skipped_ambiguous += 1;
            continue;
        }

        let word = if args.preserve_case {
            record.word.clone()
        } else {
            record.word.to_ascii_lowercase()
        };
        if args.ascii_alpha_only && !word.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            skipped_non_alpha += 1;
            continue;
        }
        emit_patgen_word(
            &mut writer,
            &mut emitted,
            &word,
            &record.breaks,
            &args.separator,
        )?;

        if args.include_ambiguous {
            for breaks in &record.variants {
                emit_patgen_word(&mut writer, &mut emitted, &word, breaks, &args.separator)?;
            }
        }
    }

    writer.flush()?;
    println!(
        "exported {} patgen entries -> {}",
        emitted.len(),
        args.output.display()
    );
    if skipped_ambiguous > 0 {
        println!("skipped_ambiguous: {skipped_ambiguous}");
    }
    if skipped_non_alpha > 0 {
        println!("skipped_non_alpha: {skipped_non_alpha}");
    }
    Ok(())
}

fn emit_patgen_word(
    writer: &mut impl Write,
    emitted: &mut BTreeSet<String>,
    word: &str,
    breaks: &[GraphemeIndex],
    separator: &str,
) -> Result<()> {
    let hyphenated = insert_separator(word, breaks, separator);
    if emitted.insert(hyphenated.clone()) {
        writeln!(writer, "{hyphenated}")?;
    }
    Ok(())
}

fn cmd_filter_script(args: FilterScriptArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());
    let input_count = records.len();
    let mut skipped_ambiguous = 0usize;
    let mut skipped_script = 0usize;
    let filtered = records
        .into_iter()
        .filter(|record| {
            if !args.include_ambiguous && record.ambiguous {
                skipped_ambiguous += 1;
                return false;
            }
            if !script_filter_matches(&record.word, args.script) {
                skipped_script += 1;
                return false;
            }
            true
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !filtered.is_empty(),
        "{} has no records matching {:?}",
        args.input.display(),
        args.script
    );
    let output_count = write_records(&args.output, filtered)?;
    println!("input_records: {input_count}");
    println!("output_records: {output_count}");
    println!("skipped_ambiguous: {skipped_ambiguous}");
    println!("skipped_script: {skipped_script}");
    println!("output: {}", args.output.display());
    Ok(())
}

fn script_filter_matches(word: &str, script: ScriptFilterArg) -> bool {
    let mut saw_alpha = false;
    for ch in word.chars() {
        if !ch.is_alphabetic() {
            continue;
        }
        saw_alpha = true;
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        let matches = match script {
            ScriptFilterArg::Cyrillic => safe_ngram_is_cyrillic_letter(lower),
            ScriptFilterArg::RussianCyrillic => safe_ngram_is_russian_cyrillic_letter(lower),
            ScriptFilterArg::Latin => safe_ngram_latin_base_letter(lower).is_some(),
        };
        if !matches {
            return false;
        }
    }
    saw_alpha
}

fn cmd_filter_quality(args: FilterQualityArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());
    let input_count = records.len();
    let mut dropped_long_no_break = 0usize;
    let filtered = records
        .into_iter()
        .filter(|record| {
            if args.drop_long_no_break
                && record.breaks.is_empty()
                && record.grapheme_len() >= args.min_graphemes
                && safe_ngram_word_vowel_count(&record.word) >= args.min_vowels
            {
                dropped_long_no_break += 1;
                return false;
            }
            true
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !filtered.is_empty(),
        "{} has no records after quality filtering",
        args.input.display()
    );
    let output_count = write_records(&args.output, filtered)?;
    println!("input_records: {input_count}");
    println!("output_records: {output_count}");
    println!("dropped_long_no_break: {dropped_long_no_break}");
    println!("output: {}", args.output.display());
    Ok(())
}

fn safe_ngram_word_vowel_count(word: &str) -> usize {
    word.chars()
        .filter(|ch| {
            let lower = ch.to_lowercase().next().unwrap_or(*ch);
            safe_ngram_unicode_is_vowel(lower)
        })
        .count()
}

fn cmd_dedup_variants(args: DedupVariantsArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());
    let input_count = records.len();
    let mut groups = BTreeMap::<String, (HyphenationRecord, BTreeSet<Vec<GraphemeIndex>>)>::new();
    for record in records {
        let key = split_group_key(&record);
        let entry = groups.entry(key).or_insert_with(|| {
            let mut template = record.clone();
            template.ambiguous = false;
            template.variants.clear();
            (template, BTreeSet::new())
        });
        entry.1.insert(record.breaks.clone().into_vec());
        for variant in record.variants {
            entry.1.insert(variant.into_vec());
        }
    }

    let mut ambiguous_words = 0usize;
    let mut deduped = Vec::with_capacity(groups.len());
    for (_key, (mut record, variants)) in groups {
        let mut variants = variants
            .into_iter()
            .map(SmallVec::from_vec)
            .collect::<Vec<_>>();
        variants.sort();
        let Some(first) = variants.first().cloned() else {
            continue;
        };
        record.breaks = first;
        if variants.len() > 1 {
            record.ambiguous = true;
            record.variants = variants;
            ambiguous_words += 1;
        } else {
            record.ambiguous = false;
            record.variants.clear();
        }
        deduped.push(record);
    }

    deduped.sort_by(|left, right| {
        split_group_key(left)
            .cmp(&split_group_key(right))
            .then_with(|| left.id.cmp(&right.id))
    });
    let output_count = write_records(&args.output, deduped)?;
    println!("input_records: {input_count}");
    println!("output_records: {output_count}");
    println!("ambiguous_words: {ambiguous_words}");
    println!(
        "collapsed_duplicates: {}",
        input_count.saturating_sub(output_count)
    );
    println!("output: {}", args.output.display());
    Ok(())
}

fn cmd_split(args: SplitArgs) -> Result<()> {
    anyhow::ensure!(
        args.train_ratio >= 0.0,
        "--train-ratio must be non-negative"
    );
    anyhow::ensure!(args.dev_ratio >= 0.0, "--dev-ratio must be non-negative");
    anyhow::ensure!(args.test_ratio >= 0.0, "--test-ratio must be non-negative");
    let ratio_sum = args.train_ratio + args.dev_ratio + args.test_ratio;
    anyhow::ensure!(ratio_sum > 0.0, "at least one split ratio must be positive");

    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());

    let mut grouped = BTreeMap::<String, Vec<HyphenationRecord>>::new();
    for record in records {
        grouped
            .entry(split_group_key(&record))
            .or_default()
            .push(record);
    }

    let mut groups = grouped
        .into_iter()
        .map(|(key, records)| (stable_hash64(&args.seed, &key), key, records))
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let total_groups = groups.len();
    let [train_group_count, dev_group_count, _test_group_count] = split_counts(
        total_groups,
        [args.train_ratio, args.dev_ratio, args.test_ratio],
        ratio_sum,
    );
    let train_group_cut = train_group_count;
    let dev_group_cut = train_group_count
        .saturating_add(dev_group_count)
        .min(total_groups);

    let mut train_records = Vec::new();
    let mut dev_records = Vec::new();
    let mut test_records = Vec::new();
    let mut train_groups = 0usize;
    let mut dev_groups = 0usize;
    let mut test_groups = 0usize;

    for (index, (_hash, _key, records)) in groups.into_iter().enumerate() {
        if index < train_group_cut {
            train_groups += 1;
            train_records.extend(records);
        } else if index < dev_group_cut {
            dev_groups += 1;
            dev_records.extend(records);
        } else {
            test_groups += 1;
            test_records.extend(records);
        }
    }

    let train_path = args.output_dir.join("train.jsonl.zst");
    let dev_path = args.output_dir.join("dev.jsonl.zst");
    let test_path = args.output_dir.join("test.jsonl.zst");
    let train_count = write_records(&train_path, train_records)?;
    let dev_count = write_records(&dev_path, dev_records)?;
    let test_count = write_records(&test_path, test_records)?;

    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("create {}", args.output_dir.display()))?;
    let summary_path = args.output_dir.join("split.json");
    let summary = serde_json::json!({
        "input": args.input.display().to_string(),
        "seed": args.seed,
        "group_key": "lang + lowercase(word)",
        "ratios": {
            "train": args.train_ratio,
            "dev": args.dev_ratio,
            "test": args.test_ratio,
        },
        "groups": {
            "train": train_groups,
            "dev": dev_groups,
            "test": test_groups,
        },
        "records": {
            "train": train_count,
            "dev": dev_count,
            "test": test_count,
        },
        "outputs": {
            "train": train_path.display().to_string(),
            "dev": dev_path.display().to_string(),
            "test": test_path.display().to_string(),
        },
    });
    std::fs::write(&summary_path, serde_json::to_vec_pretty(&summary)?)
        .with_context(|| format!("write {}", summary_path.display()))?;

    println!(
        "train: {train_count} records, {train_groups} groups -> {}",
        train_path.display()
    );
    println!(
        "dev: {dev_count} records, {dev_groups} groups -> {}",
        dev_path.display()
    );
    println!(
        "test: {test_count} records, {test_groups} groups -> {}",
        test_path.display()
    );
    println!("summary: {}", summary_path.display());
    Ok(())
}

fn cmd_kfold(args: KfoldArgs) -> Result<()> {
    anyhow::ensure!(args.folds >= 2, "--folds must be at least 2");
    anyhow::ensure!(
        (0.0..1.0).contains(&args.dev_ratio),
        "--dev-ratio must be in [0, 1)"
    );

    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());

    let mut grouped = BTreeMap::<String, Vec<HyphenationRecord>>::new();
    for record in records {
        grouped
            .entry(split_group_key(&record))
            .or_default()
            .push(record);
    }

    let mut groups = grouped
        .into_iter()
        .map(|(key, records)| (stable_hash64(&args.seed, &key), key, records))
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    anyhow::ensure!(
        groups.len() >= args.folds,
        "not enough word groups ({}) for {} folds",
        groups.len(),
        args.folds
    );

    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("create {}", args.output_dir.display()))?;
    let mut fold_summaries = Vec::new();
    for fold in 0..args.folds {
        let fold_dir = args.output_dir.join(format!("fold-{fold}"));
        let mut train_records = Vec::new();
        let mut dev_records = Vec::new();
        let mut test_records = Vec::new();
        let mut train_groups = 0usize;
        let mut dev_groups = 0usize;
        let mut test_groups = 0usize;

        for (index, (_hash, key, records)) in groups.iter().enumerate() {
            if index % args.folds == fold {
                test_groups += 1;
                test_records.extend(records.iter().cloned());
            } else if args.dev_ratio > 0.0
                && stable_unit_interval(&format!("{}:dev:{fold}", args.seed), key) < args.dev_ratio
            {
                dev_groups += 1;
                dev_records.extend(records.iter().cloned());
            } else {
                train_groups += 1;
                train_records.extend(records.iter().cloned());
            }
        }

        if args.dev_ratio > 0.0 && dev_records.is_empty() && train_records.len() > 1 {
            let moved = train_records
                .pop()
                .expect("train_records checked non-empty before pop");
            dev_records.push(moved);
            train_groups = train_groups.saturating_sub(1);
            dev_groups += 1;
        }

        let train_path = fold_dir.join("train.jsonl.zst");
        let dev_path = fold_dir.join("dev.jsonl.zst");
        let test_path = fold_dir.join("test.jsonl.zst");
        let train_count = write_records(&train_path, train_records)?;
        let dev_count = write_records(&dev_path, dev_records)?;
        let test_count = write_records(&test_path, test_records)?;

        let fold_summary = serde_json::json!({
            "fold": fold,
            "input": args.input.display().to_string(),
            "seed": args.seed,
            "group_key": "lang + lowercase(word)",
            "folds": args.folds,
            "dev_ratio": args.dev_ratio,
            "groups": {
                "train": train_groups,
                "dev": dev_groups,
                "test": test_groups,
            },
            "records": {
                "train": train_count,
                "dev": dev_count,
                "test": test_count,
            },
            "outputs": {
                "train": train_path.display().to_string(),
                "dev": dev_path.display().to_string(),
                "test": test_path.display().to_string(),
            },
        });
        std::fs::write(
            fold_dir.join("fold.json"),
            serde_json::to_vec_pretty(&fold_summary)?,
        )
        .with_context(|| format!("write {}", fold_dir.join("fold.json").display()))?;
        fold_summaries.push(fold_summary);
        println!(
            "fold-{fold}: train={train_count} dev={dev_count} test={test_count} -> {}",
            fold_dir.display()
        );
    }

    let summary_path = args.output_dir.join("kfold.json");
    let summary = serde_json::json!({
        "input": args.input.display().to_string(),
        "seed": args.seed,
        "group_key": "lang + lowercase(word)",
        "folds": args.folds,
        "dev_ratio": args.dev_ratio,
        "folds_detail": fold_summaries,
    });
    std::fs::write(&summary_path, serde_json::to_vec_pretty(&summary)?)
        .with_context(|| format!("write {}", summary_path.display()))?;
    println!("summary: {}", summary_path.display());
    Ok(())
}

fn split_group_key(record: &HyphenationRecord) -> String {
    format!("{}\u{1f}{}", record.lang, record.word.to_lowercase())
}

fn split_counts(total: usize, ratios: [f64; 3], ratio_sum: f64) -> [usize; 3] {
    if total == 0 {
        return [0; 3];
    }

    let positive = ratios.iter().filter(|ratio| **ratio > 0.0).count();
    if positive == 0 {
        return [total, 0, 0];
    }

    if positive > total {
        let mut order = [0usize, 1, 2];
        order.sort_by(|left, right| {
            ratios[*right]
                .partial_cmp(&ratios[*left])
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.cmp(right))
        });
        let mut counts = [0usize; 3];
        for index in order.into_iter().take(total) {
            counts[index] = 1;
        }
        return counts;
    }

    let mut counts = [0usize; 3];
    for (index, ratio) in ratios.iter().enumerate() {
        if *ratio > 0.0 {
            counts[index] = 1;
        }
    }

    let remaining = total - positive;
    let mut remainders = [(0usize, 0.0f64); 3];
    for (index, ratio) in ratios.iter().enumerate() {
        if *ratio <= 0.0 {
            remainders[index] = (index, -1.0);
            continue;
        }
        let exact = remaining as f64 * *ratio / ratio_sum;
        let floor = exact.floor() as usize;
        counts[index] += floor;
        remainders[index] = (index, exact - floor as f64);
    }

    let assigned = counts.iter().sum::<usize>();
    let mut leftover = total.saturating_sub(assigned);
    remainders.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    for (index, _remainder) in remainders {
        if leftover == 0 {
            break;
        }
        if ratios[index] > 0.0 {
            counts[index] += 1;
            leftover -= 1;
        }
    }

    counts
}

fn stable_hash64(seed: &str, value: &str) -> u64 {
    const OFFSET: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;

    let mut hash = OFFSET;
    for byte in seed.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash ^= 0xff;
    hash = hash.wrapping_mul(PRIME);
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn stable_unit_interval(seed: &str, value: &str) -> f64 {
    let hash = stable_hash64(seed, value);
    (hash as f64) / (u64::MAX as f64)
}

fn cmd_stats(args: StatsArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    let words = records.len();
    let breaks: usize = records.iter().map(|r| r.breaks.len()).sum();
    let no_break = records.iter().filter(|r| r.breaks.is_empty()).count();
    let ambiguous = records.iter().filter(|r| r.ambiguous).count();
    let mut locales: Vec<_> = records
        .iter()
        .filter_map(|r| r.locale.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    locales.truncate(12);

    println!("records: {words}");
    println!("breaks: {breaks}");
    println!("no_break_words: {no_break}");
    println!("ambiguous_words: {ambiguous}");
    println!("locales: {}", locales.join(", "));
    Ok(())
}

fn cmd_dev_new_adapter(args: NewAdapterArgs) -> Result<()> {
    let module = to_snake_identifier(&args.slug);
    let method = args
        .method
        .clone()
        .unwrap_or_else(|| module.replace('_', "-"));
    let struct_name = args
        .struct_name
        .clone()
        .unwrap_or_else(|| to_pascal_identifier(&args.slug));
    let root = args.root.clone();
    let adapter_path = root
        .join("crates")
        .join("hyph-adapters")
        .join("src")
        .join(format!("{module}.rs"));
    let adapters_lib_path = root
        .join("crates")
        .join("hyph-adapters")
        .join("src")
        .join("lib.rs");
    let manifest_path = if args.manifest.is_absolute() {
        args.manifest.clone()
    } else {
        root.join(&args.manifest)
    };

    let adapter_source = render_adapter_template(&module, &method, &struct_name);
    let manifest_entry = render_manifest_entry(
        &args.slug,
        &method,
        &args.supports,
        args.requires_patterns,
        args.pass_patterns,
        args.requires_feature.as_deref(),
    );

    if args.dry_run {
        println!("adapter: {}", adapter_path.display());
        println!("{adapter_source}");
        println!("manifest: {}", manifest_path.display());
        println!("{manifest_entry}");
        return Ok(());
    }

    if adapter_path.exists() && !args.force {
        anyhow::bail!(
            "{} already exists; pass --force to replace it",
            adapter_path.display()
        );
    }
    create_parent(&adapter_path)?;
    std::fs::write(&adapter_path, adapter_source)
        .with_context(|| format!("write {}", adapter_path.display()))?;

    update_adapter_registry(&adapters_lib_path, &module, &method, &struct_name)?;
    append_manifest_entry(&manifest_path, &args.slug, &method, &manifest_entry)?;

    println!("created {}", adapter_path.display());
    println!("updated {}", adapters_lib_path.display());
    println!("updated {}", manifest_path.display());
    println!();
    println!("next:");
    println!("  cargo fmt --all");
    println!("  cargo check -p hyph-cli");
    println!("  cargo run -p hyph-cli -- dev smoke {}", args.slug);
    Ok(())
}

fn cmd_dev_smoke(args: SmokeArgs) -> Result<()> {
    cmd_matrix(MatrixArgs {
        manifest: args.manifest,
        gold: args.gold,
        locale: args.locale,
        patterns: Some(args.patterns),
        output_dir: args.output_dir.join(&args.slug),
        iterations: args.iterations,
        init_iterations: args.init_iterations,
        init_warmup: 0,
        ambiguous: AmbiguousPolicyArg::Exclude,
        only: vec![args.slug],
        abort_method_errors: false,
    })
}

fn cmd_crf_train(args: CrfTrainArgs) -> Result<()> {
    let mut records = read_records(&args.gold)?;
    if !args.include_ambiguous {
        records.retain(|record| !record.ambiguous || record.variants.is_empty());
    }
    if let Some(limit) = args.limit {
        records.truncate(limit);
    }
    anyhow::ensure!(
        !records.is_empty(),
        "{} has no training records",
        args.gold.display()
    );

    let mut config = HyphenationConfig::default();
    if let Some(left_min) = args.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = args.right_min {
        config.right_min = right_min;
    }
    if let Some(min_word_len) = args.min_word_len {
        config.min_word_len = min_word_len;
    }
    let language = args
        .locale
        .parse::<LanguageTag>()
        .map_err(|err| anyhow::anyhow!("parse locale {:?}: {err}", args.locale))?;

    let model = train_crf(
        &records,
        CrfTrainOptions {
            id: args.id,
            language,
            config,
            threshold: args.threshold,
            min_n: args.min_n,
            max_n: args.max_n,
            epochs: args.epochs,
            learning_rate: args.learning_rate,
            l2: args.l2,
        },
    )?;
    model.save(&args.output)?;
    println!("records: {}", records.len());
    println!("features: {}", model.feature_count());
    println!("threshold: {:.3}", model.threshold());
    println!("output: {}", args.output.display());
    Ok(())
}

fn cmd_crf_tune_threshold(args: CrfTuneThresholdArgs) -> Result<()> {
    anyhow::ensure!((0.0..=1.0).contains(&args.min), "--min must be in [0, 1]");
    anyhow::ensure!((0.0..=1.0).contains(&args.max), "--max must be in [0, 1]");
    anyhow::ensure!(args.min <= args.max, "--min must be <= --max");
    anyhow::ensure!(args.step > 0.0, "--step must be greater than zero");

    let records = read_records(&args.gold)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.gold.display());
    let mut model = CrfHyphenator::load(&args.model)?;
    let ambiguous = AmbiguousPolicy::from(args.ambiguous);
    let mut rows = Vec::new();
    let mut best_threshold = args.min;
    let mut best_metrics = Metrics::default();
    let mut best_score = f64::NEG_INFINITY;

    let mut current_threshold = args.min;
    while current_threshold <= args.max + args.step * 0.5 {
        let threshold = current_threshold.min(args.max);
        model.set_threshold(threshold)?;
        let metrics = evaluate_crf_metrics(&model, &records, ambiguous)?;
        let score = threshold_objective_score(&metrics, args.objective);
        rows.push(serde_json::json!({
            "threshold": threshold,
            "score": score,
            "precision": metrics.precision(),
            "recall": metrics.recall(),
            "f1": metrics.f1(),
            "f0.5": metrics.f05(),
            "exact": metrics.exact_accuracy(),
            "serious_error": metrics.serious_word_error_rate(),
            "fp_per_100k": metrics.fp_per_100k_boundaries(),
            "tp": metrics.tp,
            "fp": metrics.fp,
            "fn": metrics.fn_,
            "tn": metrics.tn,
        }));
        if score > best_score
            || (score == best_score
                && (metrics.fp, std::cmp::Reverse(metrics.tp))
                    < (best_metrics.fp, std::cmp::Reverse(best_metrics.tp)))
        {
            best_score = score;
            best_threshold = threshold;
            best_metrics = metrics;
        }
        current_threshold += args.step;
    }

    model.set_threshold(best_threshold)?;
    if let Some(id) = &args.id {
        model.set_id(id.clone());
    }
    if let Some(path) = &args.output {
        model.save(path)?;
    }
    if let Some(path) = &args.report {
        create_parent(path)?;
        let payload = serde_json::json!({
            "model": args.model.display().to_string(),
            "gold": args.gold.display().to_string(),
            "objective": threshold_objective_name(args.objective),
            "best_threshold": best_threshold,
            "best_score": best_score,
            "best_metrics": {
                "precision": best_metrics.precision(),
                "recall": best_metrics.recall(),
                "f1": best_metrics.f1(),
                "f0.5": best_metrics.f05(),
                "exact": best_metrics.exact_accuracy(),
                "serious_error": best_metrics.serious_word_error_rate(),
                "fp_per_100k": best_metrics.fp_per_100k_boundaries(),
                "tp": best_metrics.tp,
                "fp": best_metrics.fp,
                "fn": best_metrics.fn_,
                "tn": best_metrics.tn,
            },
            "rows": rows,
        });
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &payload)?;
    }

    println!("objective: {}", threshold_objective_name(args.objective));
    println!("best_threshold: {:.4}", best_threshold);
    println!("best_score: {:.6}", best_score);
    print_metrics(model.id(), &best_metrics);
    if let Some(path) = &args.output {
        println!("output: {}", path.display());
    }
    if let Some(path) = &args.report {
        println!("report: {}", path.display());
    }
    Ok(())
}

fn cmd_crf_convert(args: CrfConvertArgs) -> Result<()> {
    let mut model = CrfHyphenator::load(&args.input)?;
    if let Some(threshold) = args.threshold {
        model.set_threshold(threshold)?;
    }
    if let Some(id) = args.id {
        model.set_id(id);
    }
    model.save(&args.output)?;
    let input_size = std::fs::metadata(&args.input)
        .with_context(|| format!("stat {}", args.input.display()))?
        .len();
    let output_size = std::fs::metadata(&args.output)
        .with_context(|| format!("stat {}", args.output.display()))?
        .len();
    println!("features: {}", model.feature_count());
    println!("threshold: {:.4}", model.threshold());
    println!("input: {} bytes ({})", input_size, args.input.display());
    println!("output: {} bytes ({})", output_size, args.output.display());
    if input_size > 0 {
        println!("size_ratio: {:.4}", output_size as f64 / input_size as f64);
    }
    Ok(())
}

fn cmd_compile_safe_ngram(args: CompileSafeNgramArgs) -> Result<()> {
    let mut records = read_records(&args.gold)?;
    if !args.include_ambiguous {
        records.retain(|record| !record.ambiguous || record.variants.is_empty());
    }
    anyhow::ensure!(
        !records.is_empty(),
        "{} has no training records",
        args.gold.display()
    );

    let mut config = HyphenationConfig::default();
    if let Some(left_min) = args.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = args.right_min {
        config.right_min = right_min;
    }
    if let Some(min_word_len) = args.min_word_len {
        config.min_word_len = min_word_len;
    }

    let (options, veto_options) = parse_safe_ngram_veto_options(&args.method)?;
    let (rules, trained_records) = learn_safe_ngram_rules(&records, &config, &options);
    let veto_rules = if let Some(veto_options) = &veto_options {
        learn_safe_ngram_veto_rules(&records, &config, &options, &rules, veto_options)
    } else {
        U64HashSet::default()
    };
    anyhow::ensure!(
        !rules.is_empty(),
        "safe-ngram learned no rules from {} with method {:?}",
        args.gold.display(),
        args.method
    );

    let model = SafeNgramModelFile::from_parts(
        args.method,
        args.locale,
        file_stem(&args.gold),
        config,
        options,
        rules,
        veto_options,
        veto_rules,
        trained_records,
    );
    model.save(&args.output)?;
    let output_size = std::fs::metadata(&args.output)
        .with_context(|| format!("stat {}", args.output.display()))?
        .len();
    println!("records: {}", records.len());
    println!("trained_records: {}", model.trained_records);
    println!("rules: {}", model.rules.len());
    println!("veto_rules: {}", model.veto_rules.len());
    println!("id: {}", model.id);
    println!("output: {} bytes ({})", output_size, args.output.display());
    Ok(())
}

fn cmd_compile_italian_syllable(args: CompileItalianSyllableArgs) -> Result<()> {
    anyhow::ensure!(
        normalize_locale_match_key(&args.locale).starts_with("it"),
        "compile-italian-syllable requires an Italian locale, got {}",
        args.locale
    );
    let mut records = read_records(&args.gold)?;
    if !args.include_ambiguous {
        records.retain(|record| !record.ambiguous || record.variants.is_empty());
    }
    anyhow::ensure!(
        !records.is_empty(),
        "{} has no training records",
        args.gold.display()
    );

    let mut config = italian_syllable_default_config();
    if let Some(left_min) = args.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = args.right_min {
        config.right_min = right_min;
    }
    if let Some(min_word_len) = args.min_word_len {
        config.min_word_len = min_word_len;
    }

    let learned_splits = learn_italian_syllable_splits(&records, &config);
    let trained_records = count_italian_syllable_training_records(&records, &config);
    let model = ItalianSyllableModelFile::from_parts(
        args.method,
        args.locale,
        file_stem(&args.gold),
        config,
        learned_splits,
        trained_records,
    );
    model.save(&args.output)?;
    let output_size = std::fs::metadata(&args.output)
        .with_context(|| format!("stat {}", args.output.display()))?
        .len();
    println!("records: {}", records.len());
    println!("trained_records: {}", model.trained_records);
    println!("clusters: {}", model.learned_splits.len());
    println!("id: {}", model.id);
    println!("output: {} bytes ({})", output_size, args.output.display());
    Ok(())
}

fn evaluate_crf_metrics(
    model: &CrfHyphenator,
    records: &[HyphenationRecord],
    ambiguous: AmbiguousPolicy,
) -> Result<Metrics> {
    evaluate_predictions(
        records.iter().cloned(),
        model.config(),
        ambiguous,
        |record, out| model.hyphenate_into(&record.word, out),
    )
}

fn threshold_objective_score(metrics: &Metrics, objective: ThresholdObjectiveArg) -> f64 {
    match objective {
        ThresholdObjectiveArg::F1 => metrics.f1(),
        ThresholdObjectiveArg::F05 => metrics.f05(),
        ThresholdObjectiveArg::Precision => metrics.precision(),
        ThresholdObjectiveArg::Recall => metrics.recall(),
        ThresholdObjectiveArg::Exact => metrics.exact_accuracy(),
    }
}

fn threshold_objective_name(objective: ThresholdObjectiveArg) -> &'static str {
    match objective {
        ThresholdObjectiveArg::F1 => "f1",
        ThresholdObjectiveArg::F05 => "f0.5",
        ThresholdObjectiveArg::Precision => "precision",
        ThresholdObjectiveArg::Recall => "recall",
        ThresholdObjectiveArg::Exact => "exact",
    }
}

fn render_adapter_template(module: &str, method: &str, struct_name: &str) -> String {
    format!(
        r#"use crate::MethodAdapter;
use anyhow::Result;
use hyph_core::{{GraphemeIndex, HyphenationConfig, LanguageTag}};
use smallvec::SmallVec;

#[derive(Debug, Clone)]
pub struct {struct_name} {{
    language: LanguageTag,
    config: HyphenationConfig,
    id: String,
}}

impl {struct_name} {{
    pub fn new(language: LanguageTag) -> Self {{
        let id = format!("{method}-{{}}", language.language);
        Self {{
            language,
            config: HyphenationConfig::default(),
            id,
        }}
    }}
}}

impl MethodAdapter for {struct_name} {{
    fn id(&self) -> &str {{
        &self.id
    }}

    fn language(&self) -> &LanguageTag {{
        &self.language
    }}

    fn config(&self) -> &HyphenationConfig {{
        &self.config
    }}

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {{
        out.clear();
        let _ = word;
        Ok(())
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn {module}_returns_sorted_breaks() {{
        let adapter = {struct_name}::new("en-US".parse().unwrap());
        let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
        adapter.hyphenate_into("hyphenation", &mut out).unwrap();
        assert!(out.windows(2).all(|pair| pair[0] < pair[1]));
    }}
}}
"#
    )
}

fn render_manifest_entry(
    slug: &str,
    method: &str,
    supports: &[String],
    requires_patterns: bool,
    pass_patterns: bool,
    requires_feature: Option<&str>,
) -> String {
    let mut out = format!("\n[[methods]]\nslug = {slug:?}\nmethod = {method:?}\n");
    if !supports.is_empty() {
        out.push_str("supports = [");
        for (index, locale) in supports.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("{locale:?}"));
        }
        out.push_str("]\n");
    }
    if let Some(feature) = requires_feature {
        out.push_str(&format!("requires_feature = {feature:?}\n"));
    }
    if requires_patterns {
        out.push_str("requires_patterns = true\n");
    }
    if pass_patterns {
        out.push_str("pass_patterns = true\n");
    }
    out
}

fn update_adapter_registry(
    lib_path: &Path,
    module: &str,
    method: &str,
    struct_name: &str,
) -> Result<()> {
    let factory = format!("{module}_factory");
    let mut text = std::fs::read_to_string(lib_path)
        .with_context(|| format!("read {}", lib_path.display()))?;
    text = insert_after_marker_once(
        text,
        "// hyphlab:adapter-modules",
        &format!("mod {module};"),
        &format!("module {module}"),
    )?;
    text = insert_after_marker_once(
        text,
        "// hyphlab:adapter-exports",
        &format!("pub use {module}::{struct_name};"),
        &format!("export {struct_name}"),
    )?;
    text = insert_after_marker_once(
        text,
        "// hyphlab:adapter-registrations",
        &format!(
            "        AdapterRegistration {{\n            names: &[{method:?}],\n            factory: {factory},\n        }},"
        ),
        &format!("registration {method}"),
    )?;
    text = insert_after_marker_once(
        text,
        "// hyphlab:adapter-factories",
        &format!(
            "fn {factory}(locale: &str) -> Result<Box<dyn MethodAdapter>> {{\n    Ok(Box::new({struct_name}::new(locale.parse().unwrap_or_default())))\n}}\n"
        ),
        &format!("factory {factory}"),
    )?;
    std::fs::write(lib_path, text).with_context(|| format!("write {}", lib_path.display()))?;
    Ok(())
}

fn insert_after_marker_once(
    text: String,
    marker: &str,
    insertion: &str,
    label: &str,
) -> Result<String> {
    if text.contains(insertion.trim()) {
        return Ok(text);
    }
    let marker_index = text
        .find(marker)
        .with_context(|| format!("missing scaffold marker {marker:?} for {label}"))?;
    let line_end = text[marker_index..]
        .find('\n')
        .map(|offset| marker_index + offset + 1)
        .unwrap_or(text.len());
    let mut updated = String::with_capacity(text.len() + insertion.len() + 1);
    updated.push_str(&text[..line_end]);
    updated.push_str(insertion);
    updated.push('\n');
    updated.push_str(&text[line_end..]);
    Ok(updated)
}

fn append_manifest_entry(path: &Path, slug: &str, method: &str, entry: &str) -> Result<()> {
    let mut text =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if manifest_contains_key(&text, "slug", slug) || manifest_contains_key(&text, "method", method)
    {
        println!(
            "manifest already contains slug {slug:?} or method {method:?}; leaving it unchanged"
        );
        return Ok(());
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(entry);
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn manifest_contains_key(text: &str, key: &str, value: &str) -> bool {
    let expected = format!("{key} = {value:?}");
    text.lines().any(|line| line.trim() == expected)
}

fn to_snake_identifier(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_underscore = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_underscore = false;
        } else if !last_was_underscore && !out.is_empty() {
            out.push('_');
            last_was_underscore = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("method");
    }
    if out
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        out.insert_str(0, "method_");
    }
    out
}

fn to_pascal_identifier(value: &str) -> String {
    let snake = to_snake_identifier(value);
    let mut out = String::new();
    for part in snake.split('_').filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    if out
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        out.insert_str(0, "Method");
    }
    if out.is_empty() {
        out.push_str("Method");
    }
    out
}

fn cmd_eval(args: EvalArgs) -> Result<()> {
    let records = read_records(&args.gold)?;
    let evaluation = evaluation_metadata(
        &args.gold,
        &args.locale,
        args.patterns.as_ref(),
        args.ambiguous,
        args.left_min,
        args.right_min,
        args.min_word_len,
    );
    let dictionary_is_gold_oracle = is_dictionary_method(&args.method) && args.dictionary.is_none();
    let method = prepare_method(MethodOptions {
        method: args.method.clone(),
        locale: args.locale.clone(),
        patterns: args.patterns.clone(),
        dictionary: args.dictionary.clone().or_else(|| {
            if is_dictionary_method(&args.method) {
                Some(args.gold.clone())
            } else {
                None
            }
        }),
        dictionary_is_gold_oracle,
        external_command: args.external_command.clone(),
        left_min: args.left_min,
        right_min: args.right_min,
        min_word_len: args.min_word_len,
    })?;
    let config = method.config().clone();

    let prediction_error_policy = if args.skip_method_errors {
        PredictionErrorPolicy::Skip
    } else {
        PredictionErrorPolicy::Abort
    };
    let report = evaluate_predictions_report_with_policy(
        records,
        &config,
        args.ambiguous.into(),
        prediction_error_policy,
        |record, out| {
            method
                .hyphenate_record_into(record, out)
                .with_context(|| format!("hyphenate {:?}", record.word))
        },
    )?;

    if let Some(path) = &args.output {
        write_report(path, method.id(), &evaluation, &report)?;
    }
    if let Some(path) = &args.errors_output {
        write_errors(path, &report.errors)?;
    }
    if let Some(path) = &args.method_errors_output {
        write_method_errors(path, &report.method_errors)?;
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report.metrics)?);
    } else {
        print_metrics(method.id(), &report.metrics);
        if let Some(path) = &args.output {
            println!("metrics_output: {}", path.display());
        }
        if let Some(path) = &args.errors_output {
            println!("errors_output: {}", path.display());
        }
        if let Some(path) = &args.method_errors_output {
            println!("method_errors_output: {}", path.display());
        }
    }
    Ok(())
}

fn cmd_compare(args: CompareArgs) -> Result<()> {
    let mut speeds = HashMap::new();
    for input in &args.speed_input {
        let speed = read_speed_summary(input)?;
        speeds.insert(speed.method.clone(), speed);
    }
    let mut inits = HashMap::new();
    for input in &args.init_input {
        let init = read_init_summary(input)?;
        inits.insert(init.method.clone(), init);
    }

    let mut rows = Vec::new();
    for input in &args.input {
        let file = File::open(input).with_context(|| format!("open {}", input.display()))?;
        let value: serde_json::Value =
            serde_json::from_reader(file).with_context(|| format!("parse {}", input.display()))?;
        let method = value
            .get("method")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| file_stem(input));
        let metrics_value = value
            .get("metrics")
            .cloned()
            .unwrap_or_else(|| value.clone());
        let metrics = serde_json::from_value::<Metrics>(metrics_value)
            .with_context(|| format!("read metrics from {}", input.display()))?;
        let evaluation = value
            .get("evaluation")
            .cloned()
            .map(serde_json::from_value::<EvaluationMetadata>)
            .transpose()
            .with_context(|| format!("read evaluation metadata from {}", input.display()))?;
        let speed = speeds.get(&method).cloned();
        let init = inits.get(&method).cloned();
        rows.push(CompareRow {
            method,
            metrics,
            evaluation,
            speed,
            init,
        });
    }

    let table = render_compare_table(&rows);
    if let Some(path) = &args.output {
        create_parent(path)?;
        std::fs::write(path, &table).with_context(|| format!("write {}", path.display()))?;
    } else {
        print!("{table}");
    }
    Ok(())
}

fn cmd_fold_summary(args: FoldSummaryArgs) -> Result<()> {
    let mut fold_dirs = std::fs::read_dir(&args.input_dir)
        .with_context(|| format!("read {}", args.input_dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("read entries from {}", args.input_dir.display()))?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    fold_dirs.sort();
    anyhow::ensure!(
        !fold_dirs.is_empty(),
        "{} has no fold directories",
        args.input_dir.display()
    );

    let mut points_by_method = BTreeMap::<String, Vec<FoldPoint>>::new();
    let mut points_by_fold = BTreeMap::<String, BTreeMap<String, FoldPoint>>::new();

    for fold_dir in fold_dirs {
        let fold_name = fold_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("fold")
            .to_string();
        let speed = read_fold_speed_or_init(&fold_dir.join("speed"), true)?;
        let init = read_fold_speed_or_init(&fold_dir.join("init"), false)?;
        let mut metric_paths = std::fs::read_dir(&fold_dir)
            .with_context(|| format!("read {}", fold_dir.display()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("read entries from {}", fold_dir.display()))?
            .into_iter()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        metric_paths.sort();

        for metric_path in metric_paths {
            let file = File::open(&metric_path)
                .with_context(|| format!("open {}", metric_path.display()))?;
            let value: serde_json::Value = serde_json::from_reader(file)
                .with_context(|| format!("parse {}", metric_path.display()))?;
            let Some(metrics_value) = value.get("metrics").cloned() else {
                continue;
            };
            let metrics = serde_json::from_value::<Metrics>(metrics_value)
                .with_context(|| format!("parse metrics from {}", metric_path.display()))?;
            let slug = file_stem(&metric_path);
            let point = FoldPoint {
                words: metrics.words as f64,
                precision: metrics.precision(),
                recall: metrics.recall(),
                f1: metrics.f1(),
                f05: metrics.f05(),
                exact: metrics.exact_accuracy(),
                serious_error: metrics.serious_word_error_rate(),
                fp_per_100k: metrics.fp_per_100k_boundaries(),
                ns_per_word: speed.get(&slug).copied(),
                init_ms: init.get(&slug).map(|ns| ns / 1_000_000.0),
            };
            points_by_method
                .entry(slug.clone())
                .or_default()
                .push(point.clone());
            points_by_fold
                .entry(fold_name.clone())
                .or_default()
                .insert(slug, point);
        }
    }

    let summary = render_fold_summary(&points_by_method, &points_by_fold);
    if let Some(path) = &args.output {
        create_parent(path)?;
        std::fs::write(path, &summary).with_context(|| format!("write {}", path.display()))?;
        println!("wrote {}", path.display());
    } else {
        print!("{summary}");
    }
    Ok(())
}

fn read_fold_speed_or_init(path: &Path, speed: bool) -> Result<HashMap<String, f64>> {
    let mut out = HashMap::new();
    if !path.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let path = entry
            .with_context(|| format!("read entry from {}", path.display()))?
            .path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let key = file_stem(&path);
        let value = if speed {
            read_speed_summary(&path)?.ns_per_word
        } else {
            read_init_summary(&path)?.ns_per_init
        };
        out.insert(key, value);
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct FoldPoint {
    words: f64,
    precision: f64,
    recall: f64,
    f1: f64,
    f05: f64,
    exact: f64,
    serious_error: f64,
    fp_per_100k: f64,
    ns_per_word: Option<f64>,
    init_ms: Option<f64>,
}

fn render_fold_summary(
    points_by_method: &BTreeMap<String, Vec<FoldPoint>>,
    points_by_fold: &BTreeMap<String, BTreeMap<String, FoldPoint>>,
) -> String {
    let mut out = String::new();
    out.push_str("## 5-Fold Summary\n\n");
    out.push_str("| method | folds | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | init ms | delta f0.5 | delta recall | delta serious | delta ns/word |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");

    for (method, points) in points_by_method {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            method,
            points.len(),
            fmt_mean_sd(points.iter().map(|point| point.words)),
            fmt_mean_sd(points.iter().map(|point| point.precision)),
            fmt_mean_sd(points.iter().map(|point| point.recall)),
            fmt_mean_sd(points.iter().map(|point| point.f1)),
            fmt_mean_sd(points.iter().map(|point| point.f05)),
            fmt_mean_sd(points.iter().map(|point| point.exact)),
            fmt_mean_sd(points.iter().map(|point| point.serious_error)),
            fmt_mean_sd(points.iter().map(|point| point.fp_per_100k)),
            fmt_optional_mean_sd(points.iter().filter_map(|point| point.ns_per_word)),
            fmt_optional_mean_sd(points.iter().filter_map(|point| point.init_ms)),
            fmt_delta(points_by_fold, method, |point| point.f05),
            fmt_delta(points_by_fold, method, |point| point.recall),
            fmt_delta(points_by_fold, method, |point| point.serious_error),
            fmt_optional_delta(points_by_fold, method, |point| point.ns_per_word),
        ));
    }

    out.push('\n');
    out.push_str("Deltas are paired against the `hypher` row in the same fold. Higher is better except `serious_error`, `fp/100k`, `steady ns/word`, and `init ms`.\n");
    out
}

fn fmt_mean_sd(values: impl Iterator<Item = f64>) -> String {
    let values = values.collect::<Vec<_>>();
    let (mean, sd) = mean_sd(&values);
    format!("{mean:.6} (sd {sd:.6})")
}

fn fmt_optional_mean_sd(values: impl Iterator<Item = f64>) -> String {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        String::new()
    } else {
        let (mean, sd) = mean_sd(&values);
        format!("{mean:.3} (sd {sd:.3})")
    }
}

fn fmt_delta(
    points_by_fold: &BTreeMap<String, BTreeMap<String, FoldPoint>>,
    method: &str,
    value: impl Fn(&FoldPoint) -> f64,
) -> String {
    let values = points_by_fold
        .values()
        .filter_map(|fold| Some(value(fold.get(method)?) - value(fold.get("hypher")?)))
        .collect::<Vec<_>>();
    if values.is_empty() {
        String::new()
    } else {
        let (mean, sd) = mean_sd(&values);
        format!("{mean:.6} (sd {sd:.6})")
    }
}

fn fmt_optional_delta(
    points_by_fold: &BTreeMap<String, BTreeMap<String, FoldPoint>>,
    method: &str,
    value: impl Fn(&FoldPoint) -> Option<f64>,
) -> String {
    let values = points_by_fold
        .values()
        .filter_map(|fold| Some(value(fold.get(method)?)? - value(fold.get("hypher")?)?))
        .collect::<Vec<_>>();
    if values.is_empty() {
        String::new()
    } else {
        let (mean, sd) = mean_sd(&values);
        format!("{mean:.3} (sd {sd:.3})")
    }
}

fn mean_sd(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if values.len() == 1 {
        return (mean, 0.0);
    }
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    (mean, variance.sqrt())
}

fn cmd_init_bench(args: InitBenchArgs) -> Result<()> {
    anyhow::ensure!(
        args.iterations > 0,
        "--iterations must be greater than zero"
    );

    for _ in 0..args.warmup {
        let method = prepare_method(init_method_options(&args)?)?;
        std::hint::black_box(method.id());
    }

    let started = Instant::now();
    let mut method_id = String::new();
    for _ in 0..args.iterations {
        let method = prepare_method(init_method_options(&args)?)?;
        method_id.clear();
        method_id.push_str(method.id());
        std::hint::black_box(method.id());
    }
    let elapsed = started.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let ns_per_init = elapsed_secs * 1_000_000_000.0 / args.iterations as f64;
    let inits_per_sec = args.iterations as f64 / elapsed_secs;
    let payload = serde_json::json!({
        "measurement": "method_prepare",
        "method": method_id,
        "iterations": args.iterations,
        "warmup": args.warmup,
        "elapsed_ms": elapsed_secs * 1000.0,
        "ns_per_init": ns_per_init,
        "inits_per_sec": inits_per_sec,
    });

    if let Some(path) = &args.output {
        create_parent(path)?;
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &payload)?;
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("method: {method_id}");
        println!("measurement: method_prepare");
        println!("warmup: {}", args.warmup);
        println!("iterations: {}", args.iterations);
        println!("elapsed_ms: {:.3}", elapsed_secs * 1000.0);
        println!("ns_per_init: {:.3}", ns_per_init);
        println!("inits_per_sec: {:.3}", inits_per_sec);
        if let Some(path) = &args.output {
            println!("output: {}", path.display());
        }
    }

    Ok(())
}

fn cmd_matrix(args: MatrixArgs) -> Result<()> {
    anyhow::ensure!(
        args.iterations > 0,
        "--iterations must be greater than zero"
    );
    anyhow::ensure!(
        args.init_iterations > 0,
        "--init-iterations must be greater than zero"
    );

    let manifest = read_methods_manifest(&args.manifest)?;
    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("create {}", args.output_dir.display()))?;
    let speed_dir = args.output_dir.join("speed");
    let init_dir = args.output_dir.join("init");
    std::fs::create_dir_all(&speed_dir)
        .with_context(|| format!("create {}", speed_dir.display()))?;
    std::fs::create_dir_all(&init_dir).with_context(|| format!("create {}", init_dir.display()))?;

    let manifest_dir = args.manifest.parent().unwrap_or_else(|| Path::new("."));
    let mut metric_inputs = Vec::new();
    let mut speed_inputs = Vec::new();
    let mut init_inputs = Vec::new();
    let skip_method_errors = !args.abort_method_errors;
    let only = args
        .only
        .iter()
        .map(|value| normalize_manifest_selector(value))
        .collect::<Vec<_>>();

    for method in manifest.methods {
        validate_manifest_slug(&method.slug)?;
        if !only.is_empty() && !manifest_method_selected(&method, &only) {
            continue;
        }
        if !method.enabled {
            println!("skip {}: disabled", method.slug);
            continue;
        }
        if let Some(feature) = &method.requires_feature {
            if !manifest_feature_available(feature) {
                println!("skip {}: feature {feature} is not enabled", method.slug);
                continue;
            }
        }
        if !manifest_method_supports_locale(&method, &args.locale) {
            println!("skip {}: unsupported locale {}", method.slug, args.locale);
            continue;
        }
        let patterns = match manifest_method_patterns(&method, manifest_dir, args.patterns.as_ref())
        {
            PatternDecision::Use(path) => Some(path),
            PatternDecision::Skip(reason) => {
                println!("skip {}: {reason}", method.slug);
                continue;
            }
            PatternDecision::None => None,
        };

        println!("\n-- {} ({}) --", method.slug, method.method);
        let dictionary = method
            .dictionary
            .as_ref()
            .map(|path| resolve_manifest_path(manifest_dir, path));
        let metric_path = args.output_dir.join(format!("{}.json", method.slug));
        let speed_path = speed_dir.join(format!("{}.json", method.slug));
        let init_path = init_dir.join(format!("{}.json", method.slug));

        cmd_eval(EvalArgs {
            gold: args.gold.clone(),
            method: method.method.clone(),
            locale: args.locale.clone(),
            patterns: patterns.clone(),
            dictionary: dictionary.clone(),
            external_command: method.external_command.clone(),
            left_min: method.left_min,
            right_min: method.right_min,
            min_word_len: method.min_word_len,
            ambiguous: args.ambiguous,
            json: false,
            output: Some(metric_path.clone()),
            errors_output: None,
            skip_method_errors,
            method_errors_output: Some(
                args.output_dir
                    .join(format!("{}_method_errors.jsonl", method.slug)),
            ),
        })?;

        cmd_speed(SpeedArgs {
            gold: args.gold.clone(),
            method: method.method.clone(),
            locale: args.locale.clone(),
            patterns: patterns.clone(),
            dictionary: dictionary.clone(),
            external_command: method.external_command.clone(),
            iterations: args.iterations,
            warmup: 1,
            left_min: method.left_min,
            right_min: method.right_min,
            min_word_len: method.min_word_len,
            ambiguous: args.ambiguous,
            json: false,
            output: Some(speed_path.clone()),
            skip_method_errors,
            method_errors_output: Some(
                speed_dir.join(format!("{}_method_errors.jsonl", method.slug)),
            ),
        })?;

        cmd_init_bench(InitBenchArgs {
            method: method.method,
            locale: args.locale.clone(),
            patterns,
            dictionary,
            gold: Some(args.gold.clone()),
            external_command: method.external_command,
            iterations: args.init_iterations,
            warmup: args.init_warmup,
            left_min: method.left_min,
            right_min: method.right_min,
            min_word_len: method.min_word_len,
            json: false,
            output: Some(init_path.clone()),
        })?;

        metric_inputs.push(metric_path);
        speed_inputs.push(speed_path);
        init_inputs.push(init_path);
    }

    anyhow::ensure!(
        !metric_inputs.is_empty(),
        "no manifest methods were runnable for locale {}",
        args.locale
    );
    cmd_compare(CompareArgs {
        input: metric_inputs,
        speed_input: speed_inputs,
        init_input: init_inputs,
        output: Some(args.output_dir.join("compare.md")),
    })?;
    println!("wrote {}", args.output_dir.join("compare.md").display());

    Ok(())
}

enum PatternDecision {
    Use(PathBuf),
    Skip(String),
    None,
}

fn read_methods_manifest(path: &Path) -> Result<MethodsManifest> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn validate_manifest_slug(slug: &str) -> Result<()> {
    anyhow::ensure!(!slug.trim().is_empty(), "manifest method slug is empty");
    anyhow::ensure!(
        !slug.contains('/') && !slug.contains('\\') && slug != "." && slug != "..",
        "manifest method slug must be a file-safe name: {slug:?}"
    );
    Ok(())
}

fn manifest_method_selected(method: &ManifestMethod, selectors: &[String]) -> bool {
    let slug = normalize_manifest_selector(&method.slug);
    let method_name = normalize_manifest_selector(&method.method);
    selectors
        .iter()
        .any(|selector| selector == &slug || selector == &method_name)
}

fn normalize_manifest_selector(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}

fn manifest_method_patterns(
    method: &ManifestMethod,
    manifest_dir: &Path,
    dataset_patterns: Option<&PathBuf>,
) -> PatternDecision {
    if !(method.requires_patterns || method.pass_patterns) {
        return PatternDecision::None;
    }
    let method_patterns = method.patterns.as_ref();
    let Some(path) = method_patterns.or(dataset_patterns) else {
        return if method.requires_patterns {
            PatternDecision::Skip("requires patterns but this dataset has none".to_string())
        } else {
            PatternDecision::None
        };
    };
    let path = if let Some(path) = method_patterns {
        resolve_manifest_path(manifest_dir, path)
    } else {
        path.clone()
    };
    if method.requires_patterns && !path.is_file() {
        return PatternDecision::Skip(format!("patterns file is missing: {}", path.display()));
    }
    PatternDecision::Use(path)
}

fn manifest_method_supports_locale(method: &ManifestMethod, locale: &str) -> bool {
    if method.supports.is_empty() {
        return true;
    }
    let normalized_locale = normalize_locale_match_key(locale);
    let language = normalized_locale
        .split('-')
        .next()
        .unwrap_or(normalized_locale.as_str());
    method.supports.iter().any(|supported| {
        let supported = normalize_locale_match_key(supported);
        supported == "*" || supported == normalized_locale || supported == language
    })
}

fn manifest_feature_available(feature: &str) -> bool {
    match feature {
        "adapters-hyphenation" => cfg!(feature = "adapters-hyphenation"),
        "adapters-hyphenation-embedded" => cfg!(feature = "adapters-hyphenation-embedded"),
        _ => false,
    }
}

fn normalize_locale_match_key(locale: &str) -> String {
    locale.trim().replace('_', "-").to_ascii_lowercase()
}

fn resolve_manifest_path(manifest_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_dir.join(path)
    }
}

fn cmd_speed(args: SpeedArgs) -> Result<()> {
    anyhow::ensure!(
        args.iterations > 0,
        "--iterations must be greater than zero"
    );
    let mut records = read_records(&args.gold)?;
    let evaluation = evaluation_metadata(
        &args.gold,
        &args.locale,
        args.patterns.as_ref(),
        args.ambiguous,
        args.left_min,
        args.right_min,
        args.min_word_len,
    );
    let skipped_ambiguous = if args.ambiguous == AmbiguousPolicyArg::Exclude {
        let before = records.len();
        records.retain(|record| !record.ambiguous || record.variants.is_empty());
        before - records.len()
    } else {
        0
    };
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.gold.display());
    let dictionary_is_gold_oracle = is_dictionary_method(&args.method) && args.dictionary.is_none();
    let method = prepare_method(MethodOptions {
        method: args.method.clone(),
        locale: args.locale.clone(),
        patterns: args.patterns.clone(),
        dictionary: args.dictionary.clone().or_else(|| {
            if is_dictionary_method(&args.method) {
                Some(args.gold.clone())
            } else {
                None
            }
        }),
        dictionary_is_gold_oracle,
        external_command: args.external_command.clone(),
        left_min: args.left_min,
        right_min: args.right_min,
        min_word_len: args.min_word_len,
    })?;

    let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
    let mut method_errors = Vec::new();
    if args.skip_method_errors {
        let mut filtered = Vec::with_capacity(records.len());
        for record in records {
            out.clear();
            match method.hyphenate_record_into(&record, &mut out) {
                Ok(()) => filtered.push(record),
                Err(error) => method_errors.push(MethodError {
                    id: record.id,
                    word: record.word,
                    error: error.to_string(),
                }),
            }
        }
        records = filtered;
    }
    anyhow::ensure!(
        !records.is_empty(),
        "{} has no records after method-error filtering",
        args.gold.display()
    );

    for _ in 0..args.warmup {
        for record in &records {
            method.hyphenate_record_into(std::hint::black_box(record), &mut out)?;
            std::hint::black_box(&out);
        }
    }

    let started = Instant::now();
    for _ in 0..args.iterations {
        for record in &records {
            method.hyphenate_record_into(std::hint::black_box(record), &mut out)?;
            std::hint::black_box(&out);
        }
    }
    let elapsed = started.elapsed();
    let total_predictions = records.len() * args.iterations;
    let elapsed_secs = elapsed.as_secs_f64();
    let ns_per_word = elapsed_secs * 1_000_000_000.0 / total_predictions as f64;
    let words_per_sec = total_predictions as f64 / elapsed_secs;
    let payload = serde_json::json!({
        "measurement": "steady_state_prediction",
        "method": method.id(),
        "evaluation": &evaluation,
        "words": records.len(),
        "iterations": args.iterations,
        "warmup": args.warmup,
        "skipped_ambiguous": skipped_ambiguous,
        "skipped_method_errors": method_errors.len(),
        "total_predictions": total_predictions,
        "elapsed_ms": elapsed_secs * 1000.0,
        "ns_per_word": ns_per_word,
        "words_per_sec": words_per_sec,
    });

    if let Some(path) = &args.output {
        create_parent(path)?;
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &payload)?;
    }
    if let Some(path) = &args.method_errors_output {
        write_method_errors(path, &method_errors)?;
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("method: {}", method.id());
        println!("words: {}", records.len());
        println!("skipped_ambiguous: {skipped_ambiguous}");
        println!("skipped_method_errors: {}", method_errors.len());
        println!("iterations: {}", args.iterations);
        println!("total_predictions: {total_predictions}");
        println!("elapsed_ms: {:.3}", elapsed_secs * 1000.0);
        println!("ns_per_word: {:.3}", ns_per_word);
        println!("words_per_sec: {:.3}", words_per_sec);
        if let Some(path) = &args.output {
            println!("output: {}", path.display());
        }
        if let Some(path) = &args.method_errors_output {
            println!("method_errors_output: {}", path.display());
        }
    }

    Ok(())
}

fn init_method_options(args: &InitBenchArgs) -> Result<MethodOptions> {
    let dictionary_is_gold_oracle =
        is_dictionary_method(&args.method) && args.dictionary.is_none() && args.gold.is_some();
    let dictionary = args.dictionary.clone().or_else(|| {
        if is_dictionary_method(&args.method) {
            args.gold.clone()
        } else {
            None
        }
    });
    if is_dictionary_method(&args.method) && dictionary.is_none() {
        anyhow::bail!(
            "--dictionary or --gold is required for --method {}",
            args.method
        );
    }

    Ok(MethodOptions {
        method: args.method.clone(),
        locale: args.locale.clone(),
        patterns: args.patterns.clone(),
        dictionary,
        dictionary_is_gold_oracle,
        external_command: args.external_command.clone(),
        left_min: args.left_min,
        right_min: args.right_min,
        min_word_len: args.min_word_len,
    })
}

#[derive(Debug, Clone, Copy)]
struct SavedModelSpec {
    key: &'static str,
    aliases: &'static [&'static str],
    locale: &'static str,
    method: &'static str,
    dictionary: &'static str,
}

const SAVED_MODEL_SPECS: &[SavedModelSpec] = &[
    SavedModelSpec {
        key: "en-US",
        aliases: &["en", "english", "moby", "moby-en-us", "moby_en_us"],
        locale: "en-US",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/moby_en_us.bin",
    },
    SavedModelSpec {
        key: "cs",
        aliases: &["czech", "wiktextract-cs", "wiktextract_cs"],
        locale: "cs",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_cs.bin",
    },
    SavedModelSpec {
        key: "de",
        aliases: &["german", "wiktextract-de", "wiktextract_de"],
        locale: "de",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_de.bin",
    },
    SavedModelSpec {
        key: "es",
        aliases: &["spanish", "wiktextract-es", "wiktextract_es"],
        locale: "es",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_es.bin",
    },
    SavedModelSpec {
        key: "it",
        aliases: &["italian", "wiktextract-it", "wiktextract_it"],
        locale: "it",
        method: "italian-syllable-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_it.json",
    },
    SavedModelSpec {
        key: "nl",
        aliases: &["dutch", "wiktextract-nl", "wiktextract_nl"],
        locale: "nl",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_nl.bin",
    },
    SavedModelSpec {
        key: "ru",
        aliases: &[
            "russian",
            "wiktextract-ru",
            "wiktextract_ru",
            "ru-cyrl-trusted-dedup",
            "wiktextract-ru-cyrl-trusted-dedup",
            "wiktextract_ru_cyrl_trusted_dedup",
        ],
        locale: "ru",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup.bin",
    },
    SavedModelSpec {
        key: "tr",
        aliases: &["turkish", "wiktextract-tr", "wiktextract_tr"],
        locale: "tr",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_tr.bin",
    },
];

fn print_saved_models() {
    println!("key\tlocale\tmethod\tdictionary");
    for spec in SAVED_MODEL_SPECS {
        println!(
            "{}\t{}\t{}\t{}",
            spec.key, spec.locale, spec.method, spec.dictionary
        );
    }
}

fn resolve_saved_model(key: &str) -> Result<&'static SavedModelSpec> {
    let normalized = normalize_saved_model_key(key);
    SAVED_MODEL_SPECS
        .iter()
        .find(|spec| {
            normalize_saved_model_key(spec.key) == normalized
                || spec
                    .aliases
                    .iter()
                    .any(|alias| normalize_saved_model_key(alias) == normalized)
        })
        .with_context(|| {
            format!("unknown --saved-model {key:?}; run `hyphlab predict --list-saved-models`")
        })
}

fn normalize_saved_model_key(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .replace('/', "-")
}

fn cmd_predict(args: PredictArgs) -> Result<()> {
    if args.list_saved_models {
        print_saved_models();
        return Ok(());
    }

    let PredictArgs {
        mut method,
        mut locale,
        saved_model,
        list_saved_models: _,
        patterns,
        mut dictionary,
        external_command,
        left_min,
        right_min,
        min_word_len,
        input,
        word,
        text,
        separator,
        show_breaks,
    } = args;

    if let Some(saved_model) = saved_model {
        anyhow::ensure!(
            patterns.is_none(),
            "--saved-model cannot be combined with --patterns"
        );
        anyhow::ensure!(
            dictionary.is_none(),
            "--saved-model cannot be combined with --dictionary"
        );
        let spec = resolve_saved_model(&saved_model)?;
        method = spec.method.to_string();
        locale = spec.locale.to_string();
        dictionary = Some(PathBuf::from(spec.dictionary));
    }

    let method = prepare_method(MethodOptions {
        method,
        locale,
        patterns,
        dictionary,
        dictionary_is_gold_oracle: false,
        external_command,
        left_min,
        right_min,
        min_word_len,
    })?;
    let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
    let has_direct_input = !word.is_empty() || !text.is_empty();

    for word in &word {
        predict_word_display(&method, word, &separator, show_breaks, &mut out)?;
    }
    for text in &text {
        predict_text_display(&method, text, &separator, &mut out)?;
    }

    if let Some(input) = input {
        let file =
            std::fs::File::open(&input).with_context(|| format!("open {}", input.display()))?;
        let reader = io::BufReader::new(file);
        for line in reader.lines() {
            predict_one(&method, &line?, &separator, &mut out)?;
        }
    } else if !has_direct_input {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            predict_one(&method, &line?, &separator, &mut out)?;
        }
    }

    Ok(())
}

fn cmd_candidate_oracle(args: CandidateOracleArgs) -> Result<()> {
    anyhow::ensure!(
        (1..=999_999).contains(&args.target_precision_ppm),
        "--target-precision-ppm must be in 1..=999999"
    );
    anyhow::ensure!(
        (1..=1_000_000).contains(&args.target_recall_ppm),
        "--target-recall-ppm must be in 1..=1000000"
    );
    let methods = if args.methods.is_empty() {
        vec![
            "hypher".to_string(),
            "liang".to_string(),
            "safe-ngram-multi-s1-p65-veto-multi-s1-n0".to_string(),
            "safe-ngram-mixson-multi-s1-p50-veto-multi-s1-n0".to_string(),
            "safe-ngram-multi-s1-p40-veto-mixcv-multi-s1-n0".to_string(),
        ]
    } else {
        args.methods.clone()
    };
    let prepared = methods
        .iter()
        .map(|method| {
            prepare_method(MethodOptions {
                method: method.clone(),
                locale: args.locale.clone(),
                patterns: args.patterns.clone(),
                dictionary: args.dictionary.clone(),
                dictionary_is_gold_oracle: false,
                external_command: None,
                left_min: args.left_min,
                right_min: args.right_min,
                min_word_len: args.min_word_len,
            })
            .with_context(|| format!("prepare candidate method {method:?}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut config = HyphenationConfig::default();
    if let Some(left_min) = args.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = args.right_min {
        config.right_min = right_min;
    }
    if let Some(min_word_len) = args.min_word_len {
        config.min_word_len = min_word_len;
    }

    let records = read_records(&args.gold)?;
    let mut total_words = 0usize;
    let mut skipped_ambiguous = 0usize;
    let mut total_gold = 0usize;
    let mut candidate_tp = 0usize;
    let mut candidate_fp = 0usize;
    let mut candidate_fn = 0usize;
    let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); prepared.len()];
    let mut groups = CandidateOracleGroups::default();

    for record in records {
        if record.ambiguous {
            skipped_ambiguous += 1;
            continue;
        }
        total_words += 1;
        let gold = filtered_break_set(&record.word, &record.breaks, &config);
        total_gold += gold.len();
        for (method, out) in prepared.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(&record, out)?;
        }
        let mut candidate = BTreeSet::<GraphemeIndex>::new();
        for pred in &predictions {
            candidate.extend(filtered_break_set(&record.word, pred, &config));
        }
        for boundary in &candidate {
            if gold.contains(boundary) {
                candidate_tp += 1;
            } else {
                candidate_fp += 1;
            }
        }
        candidate_fn += gold.difference(&candidate).count();

        if record.word.is_ascii() {
            let bytes = record.word.as_bytes();
            for boundary in candidate {
                let positive = gold.contains(&boundary);
                let mask = candidate_vote_mask(&predictions, boundary, &record.word, &config);
                groups.add(bytes, boundary as usize, mask, positive);
            }
        }
    }

    let candidate_precision = ratio(candidate_tp, candidate_tp + candidate_fp);
    let candidate_recall = ratio(candidate_tp, total_gold);
    let candidate_f1 = f1(candidate_precision, candidate_recall);
    let oracle_precision = if candidate_tp == 0 { 0.0 } else { 1.0 };
    let oracle_recall = ratio(candidate_tp, total_gold);
    let oracle_f1 = f1(oracle_precision, oracle_recall);
    let needed_tp = ceil_div(total_gold * args.target_recall_ppm as usize, 1_000_000);
    let max_fp_at_needed_tp = if args.target_precision_ppm >= 1_000_000 {
        0
    } else {
        (needed_tp * (1_000_000 - args.target_precision_ppm as usize))
            / args.target_precision_ppm as usize
    };

    println!("gold: {}", args.gold.display());
    println!("words: {total_words}");
    println!("skipped_ambiguous: {skipped_ambiguous}");
    println!("methods: {}", methods.join(", "));
    println!("total_gold_boundaries: {total_gold}");
    println!("candidate_tp: {candidate_tp}");
    println!("candidate_fp: {candidate_fp}");
    println!("candidate_fn: {candidate_fn}");
    println!("candidate_precision: {candidate_precision:.6}");
    println!("candidate_recall: {candidate_recall:.6}");
    println!("candidate_f1: {candidate_f1:.6}");
    println!("perfect_selector_precision: {oracle_precision:.6}");
    println!("perfect_selector_recall: {oracle_recall:.6}");
    println!("perfect_selector_f1: {oracle_f1:.6}");
    println!("target_precision_ppm: {}", args.target_precision_ppm);
    println!("target_recall_ppm: {}", args.target_recall_ppm);
    println!("needed_tp_for_target_recall: {needed_tp}");
    println!("max_fp_at_needed_tp_for_target_precision: {max_fp_at_needed_tp}");
    if candidate_tp < needed_tp {
        println!("target_possible_from_candidate_union: no");
    } else {
        let required_fp_removal = candidate_fp.saturating_sub(max_fp_at_needed_tp);
        println!("target_possible_from_candidate_union: yes");
        println!("minimum_fp_to_remove_at_target_recall: {required_fp_removal}");
    }
    for (label, counts) in groups.named_counts() {
        let point = grouped_oracle_point(counts, total_gold, args.target_precision_ppm);
        println!(
            "group_oracle[{label}]: recall={:.6} precision={:.6} f1={:.6} tp={} fp={} groups={}",
            point.recall,
            point.precision,
            point.f1,
            point.tp,
            point.fp,
            counts.len()
        );
    }
    Ok(())
}

#[derive(Default)]
struct CandidateOracleGroups {
    vote: U64HashMap<SafeNgramCounts>,
    vote_bucket: U64HashMap<SafeNgramCounts>,
    vote_raw3: U64HashMap<SafeNgramCounts>,
    vote_raw4: U64HashMap<SafeNgramCounts>,
    vote_raw5: U64HashMap<SafeNgramCounts>,
    vote_cv4: U64HashMap<SafeNgramCounts>,
    vote_son4: U64HashMap<SafeNgramCounts>,
}

impl CandidateOracleGroups {
    fn add(&mut self, bytes: &[u8], boundary: usize, vote_mask: u64, positive: bool) {
        let bucket = safe_ngram_boundary_bucket(bytes.len(), boundary);
        let raw3 = safe_ngram_key(
            bytes,
            boundary,
            0,
            SafeNgramSpec {
                left: 3,
                right: 3,
                bucketed: false,
                family: 0,
            },
        );
        let raw4 = safe_ngram_key(
            bytes,
            boundary,
            0,
            SafeNgramSpec {
                left: 4,
                right: 4,
                bucketed: false,
                family: 0,
            },
        );
        let raw5 = safe_ngram_key(
            bytes,
            boundary,
            0,
            SafeNgramSpec {
                left: 5,
                right: 5,
                bucketed: false,
                family: 0,
            },
        );
        let cv4 = safe_ngram_key(
            bytes,
            boundary,
            0,
            SafeNgramSpec {
                left: 4,
                right: 4,
                bucketed: false,
                family: 1,
            },
        );
        let son4 = safe_ngram_key(
            bytes,
            boundary,
            0,
            SafeNgramSpec {
                left: 4,
                right: 4,
                bucketed: false,
                family: 2,
            },
        );
        add_candidate_group_count(&mut self.vote, vote_mask, positive);
        add_candidate_group_count(
            &mut self.vote_bucket,
            mix_u64((vote_mask << 8) ^ bucket),
            positive,
        );
        add_candidate_group_count(
            &mut self.vote_raw3,
            mix_u64((vote_mask << 56) ^ raw3),
            positive,
        );
        add_candidate_group_count(
            &mut self.vote_raw4,
            mix_u64((vote_mask << 56) ^ raw4),
            positive,
        );
        add_candidate_group_count(
            &mut self.vote_raw5,
            mix_u64((vote_mask << 56) ^ raw5),
            positive,
        );
        add_candidate_group_count(
            &mut self.vote_cv4,
            mix_u64((vote_mask << 56) ^ cv4),
            positive,
        );
        add_candidate_group_count(
            &mut self.vote_son4,
            mix_u64((vote_mask << 56) ^ son4),
            positive,
        );
    }

    fn named_counts(&self) -> [(&'static str, &U64HashMap<SafeNgramCounts>); 7] {
        [
            ("vote", &self.vote),
            ("vote_bucket", &self.vote_bucket),
            ("vote_raw3", &self.vote_raw3),
            ("vote_raw4", &self.vote_raw4),
            ("vote_raw5", &self.vote_raw5),
            ("vote_cv4", &self.vote_cv4),
            ("vote_son4", &self.vote_son4),
        ]
    }
}

#[derive(Default)]
struct GroupedOraclePoint {
    tp: usize,
    fp: usize,
    precision: f64,
    recall: f64,
    f1: f64,
}

fn grouped_oracle_point(
    counts: &U64HashMap<SafeNgramCounts>,
    total_gold: usize,
    target_precision_ppm: u32,
) -> GroupedOraclePoint {
    let mut groups = counts.values().copied().collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        group_precision(*right)
            .partial_cmp(&group_precision(*left))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.positive.cmp(&left.positive))
    });
    let mut point = GroupedOraclePoint::default();
    for group in groups {
        let next_tp = point.tp + group.positive as usize;
        let next_fp = point.fp + group.negative as usize;
        let next_precision_ppm = if next_tp + next_fp == 0 {
            0.0
        } else {
            next_tp as f64 * 1_000_000.0 / (next_tp + next_fp) as f64
        };
        if next_precision_ppm + f64::EPSILON >= target_precision_ppm as f64 {
            point.tp = next_tp;
            point.fp = next_fp;
        }
    }
    point.precision = ratio(point.tp, point.tp + point.fp);
    point.recall = ratio(point.tp, total_gold);
    point.f1 = f1(point.precision, point.recall);
    point
}

fn group_precision(counts: SafeNgramCounts) -> f64 {
    ratio(
        counts.positive as usize,
        counts.positive.saturating_add(counts.negative) as usize,
    )
}

fn add_candidate_group_count(groups: &mut U64HashMap<SafeNgramCounts>, key: u64, positive: bool) {
    let slot = groups.entry(key).or_default();
    if positive {
        slot.positive = slot.positive.saturating_add(1);
    } else {
        slot.negative = slot.negative.saturating_add(1);
    }
}

fn candidate_vote_mask(
    predictions: &[SmallVec<[GraphemeIndex; 8]>],
    boundary: GraphemeIndex,
    word: &str,
    config: &HyphenationConfig,
) -> u64 {
    let mut mask = 0u64;
    for (idx, pred) in predictions.iter().enumerate() {
        if filtered_break_set(word, pred, config).contains(&boundary) {
            mask |= 1 << idx;
        }
    }
    mask
}

fn filtered_break_set(
    word: &str,
    breaks: &[GraphemeIndex],
    config: &HyphenationConfig,
) -> BTreeSet<GraphemeIndex> {
    let len = word.graphemes(true).count();
    breaks
        .iter()
        .copied()
        .filter(|idx| {
            let idx = *idx as usize;
            idx >= config.left_min && len.saturating_sub(idx) >= config.right_min
        })
        .collect()
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn f1(precision: f64, recall: f64) -> f64 {
    if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    }
}

fn ceil_div(numerator: usize, denominator: usize) -> usize {
    numerator.div_ceil(denominator)
}

fn predict_one(
    method: &PreparedMethod,
    word: &str,
    separator: &str,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) -> Result<()> {
    let word = word.trim();
    if word.is_empty() {
        return Ok(());
    }
    out.clear();
    method.hyphenate_into(word, out)?;
    println!(
        "{}\t{}\t{:?}",
        word,
        insert_separator(word, out, separator),
        out
    );
    Ok(())
}

fn predict_word_display(
    method: &PreparedMethod,
    word: &str,
    separator: &str,
    show_breaks: bool,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) -> Result<()> {
    let word = word.trim();
    if word.is_empty() {
        return Ok(());
    }
    out.clear();
    method.hyphenate_into(word, out)?;
    let rendered = insert_separator(word, out, separator);
    if show_breaks {
        println!("{word} -> {rendered}\t{:?}", out);
    } else {
        println!("{word} -> {rendered}");
    }
    Ok(())
}

fn predict_text_display(
    method: &PreparedMethod,
    text: &str,
    separator: &str,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) -> Result<()> {
    let rendered = hyphenate_text(method, text, separator, out)?;
    println!("{text} -> {rendered}");
    Ok(())
}

fn hyphenate_text(
    method: &PreparedMethod,
    text: &str,
    separator: &str,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) -> Result<String> {
    let mut rendered = String::with_capacity(text.len());
    let mut word = String::new();
    for ch in text.chars() {
        if is_text_word_char(ch) {
            word.push(ch);
        } else {
            flush_text_word(method, &mut word, &mut rendered, separator, out)?;
            rendered.push(ch);
        }
    }
    flush_text_word(method, &mut word, &mut rendered, separator, out)?;
    Ok(rendered)
}

fn flush_text_word(
    method: &PreparedMethod,
    word: &mut String,
    rendered: &mut String,
    separator: &str,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) -> Result<()> {
    if word.is_empty() {
        return Ok(());
    }
    out.clear();
    method.hyphenate_into(word, out)?;
    rendered.push_str(&insert_separator(word, out, separator));
    word.clear();
    Ok(())
}

fn is_text_word_char(ch: char) -> bool {
    ch.is_alphabetic() || ch == '\''
}

struct MethodOptions {
    method: String,
    locale: String,
    patterns: Option<PathBuf>,
    dictionary: Option<PathBuf>,
    dictionary_is_gold_oracle: bool,
    external_command: Option<String>,
    left_min: Option<usize>,
    right_min: Option<usize>,
    min_word_len: Option<usize>,
}

enum PreparedMethod {
    Adapter {
        inner: Box<dyn MethodAdapter>,
        config: HyphenationConfig,
    },
    Liang(LiangHyphenator),
    Dictionary {
        id: String,
        config: HyphenationConfig,
        entries: HashMap<String, SmallVec<[GraphemeIndex; 8]>>,
    },
    DictionaryFallback {
        id: String,
        config: HyphenationConfig,
        entries: HashMap<String, SmallVec<[GraphemeIndex; 8]>>,
        fallback: Box<PreparedMethod>,
    },
    SafeNgram(SafeNgramMethod),
    BoundaryBayes(BoundaryBayesMethod),
    StackedBayes(StackedBayesMethod),
    CandidateBayes(CandidateBayesMethod),
    CandidateGate(CandidateGateMethod),
    PruneBayes(PruneBayesMethod),
    MaskRerank(MaskRerankMethod),
    MaskOracle(MaskOracleMethod),
    MaskCost(MaskCostMethod),
    RankedUnion(RankedUnionMethod),
    ItalianSyllable(ItalianSyllableMethod),
    HypherSafeAdd(HypherSafeAddMethod),
    BaseSafeAdd(BaseSafeAddMethod),
    SafeLadder(SafeLadderMethod),
    BaseVeto(BaseVetoMethod),
    PronCountCap(PronCountCapMethod),
    AffixSafeAdd(AffixSafeAddMethod),
    AffixVeto(AffixVetoMethod),
    AnalogSafeAdd(AnalogSafeAddMethod),
    IdentityOracle {
        config: HyphenationConfig,
    },
    Crf(CrfHyphenator),
    Intersection {
        id: String,
        config: HyphenationConfig,
        first: Box<PreparedMethod>,
        second: Box<PreparedMethod>,
    },
    ExternalJsonl(ExternalJsonlMethod),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct SafeNgramSpec {
    left: usize,
    right: usize,
    #[serde(default)]
    bucketed: bool,
    #[serde(default)]
    family: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SafeNgramOptions {
    specs: Vec<SafeNgramSpec>,
    min_support: u32,
    max_negative: u32,
    #[serde(default)]
    min_precision_ppm: Option<u32>,
    #[serde(default)]
    min_wilson_ppm: Option<u32>,
    #[serde(default)]
    cap_vowel_nuclei: bool,
    #[serde(default)]
    orthographic_veto: bool,
    #[serde(default)]
    unicode_aware: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct SafeNgramCounts {
    positive: u32,
    negative: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct FloatFeatureStats {
    count: u32,
    total: f32,
}

struct SafeNgramMethod {
    id: String,
    config: HyphenationConfig,
    options: SafeNgramOptions,
    uses_unicode_features: bool,
    family_mask: u8,
    rules: U64HashSet,
    rules_dense: Option<SafeNgramDenseSet>,
    rules_dual_dense: Option<SafeNgramDualDenseSet>,
    rules_radix: Option<SafeNgramRadixSet>,
    veto_options: Option<SafeNgramOptions>,
    veto_rules: U64HashSet,
    veto_rules_dense: Option<SafeNgramDenseSet>,
    veto_rules_dual_dense: Option<SafeNgramDualDenseSet>,
    veto_rules_radix: Option<SafeNgramRadixSet>,
}

struct SafeNgramDenseSet {
    bit_count: usize,
    bits: Vec<u64>,
}

struct SafeNgramDualDenseSet {
    first: SafeNgramDenseSet,
    second: SafeNgramDenseSet,
}

struct SafeNgramRadixSet {
    low_bits: u8,
    low_mask: u64,
    offsets: Vec<u32>,
    lows: Vec<u16>,
}

#[derive(Clone, Copy)]
enum SafeNgramRuleLookup<'a> {
    Hash(&'a U64HashSet),
    Dense(&'a SafeNgramDenseSet),
    Radix(&'a SafeNgramRadixSet),
}

#[derive(Clone, Copy)]
enum SafeNgramDualRuleLookup<'a> {
    Hash(&'a U64HashSet),
    Dense(&'a SafeNgramDualDenseSet),
}

#[derive(Debug, Clone, Copy)]
struct BoundaryBayesOptions {
    min_support: u32,
    alpha: f32,
    threshold: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StackedBayesOptions {
    min_support: u32,
    alpha: f32,
    target_precision_ppm: u32,
    calibration_percent: u32,
    epochs: u32,
    learning_rate: f32,
    cap_vowel_nuclei: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct BoundaryBayesCounts {
    positive: u32,
    negative: u32,
}

struct BoundaryBayesMethod {
    id: String,
    config: HyphenationConfig,
    options: BoundaryBayesOptions,
    weights: U64HashMap<f32>,
    bias: f32,
}

struct StackedBayesMethod {
    id: String,
    config: HyphenationConfig,
    options: StackedBayesOptions,
    threshold: f32,
    weights: U64HashMap<f32>,
    bias: f32,
    hypher: Box<dyn MethodAdapter>,
    liang: Option<Box<PreparedMethod>>,
    safe_p65: SafeNgramMethod,
    safe_mixson_p50: SafeNgramMethod,
    safe_p40_mixcv: SafeNgramMethod,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CandidateBayesOptions {
    min_support: u32,
    alpha: f32,
    target_precision_ppm: u32,
    calibration_percent: u32,
    epochs: u32,
    learning_rate: f32,
}

struct CandidateBayesMethod {
    id: String,
    config: HyphenationConfig,
    threshold: f32,
    weights: U64HashMap<f32>,
    bias: f32,
    methods: Vec<PreparedMethod>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct PruneBayesOptions {
    base_precision: u32,
    min_support: u32,
    alpha: f32,
    target_precision_ppm: u32,
    calibration_percent: u32,
    wide_sources: bool,
}

struct PruneBayesMethod {
    id: String,
    config: HyphenationConfig,
    threshold: f32,
    weights: U64HashMap<f32>,
    bias: f32,
    base: Box<PreparedMethod>,
    methods: Vec<PreparedMethod>,
}

struct RankedUnionMethod {
    id: String,
    config: HyphenationConfig,
    candidate: Box<PreparedMethod>,
    anchors: Vec<(PreparedMethod, i32)>,
    min_score: i32,
    cap_vowel_nuclei: bool,
}

#[derive(Default)]
struct StackedVotes {
    hypher: SmallVec<[GraphemeIndex; 8]>,
    liang: SmallVec<[GraphemeIndex; 8]>,
    safe_p65: SmallVec<[GraphemeIndex; 8]>,
    safe_mixson_p50: SmallVec<[GraphemeIndex; 8]>,
    safe_p40_mixcv: SmallVec<[GraphemeIndex; 8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum CandidateGateKind {
    Vote,
    VoteBucket,
    VoteRaw3,
    VoteRaw4,
    VoteRaw5,
    VoteCv4,
    VoteSon4,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CandidateGateOptions {
    kind: CandidateGateKind,
    target_precision_ppm: u32,
    min_support: u32,
    calibration_percent: u32,
}

struct CandidateGateMethod {
    id: String,
    config: HyphenationConfig,
    options: CandidateGateOptions,
    methods: Vec<PreparedMethod>,
    selected: U64HashSet,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MaskRerankOptions {
    epochs: u32,
    learning_rate: f32,
    max_candidate_boundaries: usize,
    max_masks: usize,
    fp_weight: f32,
    fn_weight: f32,
    cap_vowel_nuclei: bool,
    wide_sources: bool,
}

struct MaskRerankMethod {
    id: String,
    config: HyphenationConfig,
    options: MaskRerankOptions,
    weights: U64HashMap<f32>,
    methods: Vec<PreparedMethod>,
}

struct MaskOracleMethod {
    id: String,
    config: HyphenationConfig,
    options: MaskRerankOptions,
    methods: Vec<PreparedMethod>,
}

struct MaskCostMethod {
    id: String,
    config: HyphenationConfig,
    options: MaskRerankOptions,
    group_costs: U64HashMap<FloatFeatureStats>,
    global_cost: f32,
    methods: Vec<PreparedMethod>,
}

struct ItalianSyllableMethod {
    id: String,
    config: HyphenationConfig,
    learned_splits: U64HashMap<u8>,
}

#[derive(Clone, Copy, Default)]
struct ItalianSplitCounts {
    counts: [u32; 5],
}

#[derive(Debug, Serialize, Deserialize)]
struct ItalianSyllableModelFile {
    schema_version: u32,
    id: String,
    method: String,
    locale: String,
    source: String,
    config: HyphenationConfig,
    learned_splits: Vec<ItalianSyllableSplit>,
    trained_records: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ItalianSyllableSplit {
    key: String,
    split: u8,
}

#[derive(Debug, Clone)]
struct MaskCandidate {
    breaks: SmallVec<[GraphemeIndex; 8]>,
    features: Vec<u64>,
    groups: SmallVec<[u64; 8]>,
}

#[derive(Debug, Clone)]
struct MaskTrainingExample {
    target_idx: usize,
    costs: Vec<f32>,
    candidates: Vec<MaskCandidate>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SafeNgramModelFile {
    schema_version: u32,
    id: String,
    method: String,
    locale: String,
    source: String,
    config: HyphenationConfig,
    options: SafeNgramOptions,
    rules: Vec<u64>,
    #[serde(default)]
    veto_options: Option<SafeNgramOptions>,
    #[serde(default)]
    veto_rules: Vec<u64>,
    trained_records: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct SafeNgramModelMeta {
    schema_version: u32,
    id: String,
    method: String,
    locale: String,
    source: String,
    config: HyphenationConfig,
    options: SafeNgramOptions,
    trained_records: usize,
    rule_count: usize,
    #[serde(default)]
    veto_options: Option<SafeNgramOptions>,
    #[serde(default)]
    veto_rule_count: usize,
}

impl SafeNgramModelFile {
    fn from_parts(
        method: String,
        locale: String,
        source: String,
        config: HyphenationConfig,
        options: SafeNgramOptions,
        rules: U64HashSet,
        veto_options: Option<SafeNgramOptions>,
        veto_rules: U64HashSet,
        trained_records: usize,
    ) -> Self {
        let mut rules = rules.into_iter().collect::<Vec<_>>();
        rules.sort_unstable();
        let mut veto_rules = veto_rules.into_iter().collect::<Vec<_>>();
        veto_rules.sort_unstable();
        Self {
            schema_version: 1,
            id: format!(
                "{method}:{source}:r{}:v{}:n{}",
                rules.len(),
                veto_rules.len(),
                trained_records
            ),
            method,
            locale,
            source,
            config,
            options,
            rules,
            veto_options,
            veto_rules,
            trained_records,
        }
    }

    fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        if path_extension_eq(path, "bin") {
            Self::load_binary(file, path)
        } else if path_extension_eq(path, "zst") {
            let decoder = zstd::stream::read::Decoder::new(file)
                .with_context(|| format!("open zstd decoder for {}", path.display()))?;
            serde_json::from_reader(BufReader::new(decoder))
                .with_context(|| format!("parse {}", path.display()))
        } else {
            serde_json::from_reader(BufReader::new(file))
                .with_context(|| format!("parse {}", path.display()))
        }
    }

    fn save(&self, path: &Path) -> Result<()> {
        create_parent(path)?;
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        if path_extension_eq(path, "bin") {
            self.save_binary(file, path)?;
        } else if path_extension_eq(path, "zst") {
            let writer = BufWriter::new(file);
            let mut encoder = zstd::stream::write::Encoder::new(writer, 3)
                .with_context(|| format!("open zstd encoder for {}", path.display()))?;
            serde_json::to_writer(&mut encoder, self)
                .with_context(|| format!("write {}", path.display()))?;
            encoder.finish()?;
        } else {
            serde_json::to_writer_pretty(BufWriter::new(file), self)
                .with_context(|| format!("write {}", path.display()))?;
        }
        Ok(())
    }

    fn metadata(&self) -> SafeNgramModelMeta {
        SafeNgramModelMeta {
            schema_version: self.schema_version,
            id: self.id.clone(),
            method: self.method.clone(),
            locale: self.locale.clone(),
            source: self.source.clone(),
            config: self.config.clone(),
            options: self.options.clone(),
            trained_records: self.trained_records,
            rule_count: self.rules.len(),
            veto_options: self.veto_options.clone(),
            veto_rule_count: self.veto_rules.len(),
        }
    }

    fn load_binary(file: File, path: &Path) -> Result<Self> {
        let mut reader = BufReader::new(file);
        let mut magic = [0u8; 8];
        reader
            .read_exact(&mut magic)
            .with_context(|| format!("read magic from {}", path.display()))?;
        anyhow::ensure!(
            &magic == b"HYSG1\0\0\0",
            "invalid safe-ngram binary model magic in {}",
            path.display()
        );

        let mut len_bytes = [0u8; 4];
        reader
            .read_exact(&mut len_bytes)
            .with_context(|| format!("read metadata length from {}", path.display()))?;
        let metadata_len = u32::from_le_bytes(len_bytes) as usize;
        let mut metadata_bytes = vec![0u8; metadata_len];
        reader
            .read_exact(&mut metadata_bytes)
            .with_context(|| format!("read metadata from {}", path.display()))?;
        let metadata: SafeNgramModelMeta =
            serde_json::from_slice(&metadata_bytes).context("parse safe-ngram binary metadata")?;

        let mut rule_bytes = vec![
            0u8;
            (metadata.rule_count + metadata.veto_rule_count)
                * std::mem::size_of::<u64>()
        ];
        reader
            .read_exact(&mut rule_bytes)
            .with_context(|| format!("read rules from {}", path.display()))?;
        let all_rules = rule_bytes
            .chunks_exact(8)
            .map(|chunk| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(chunk);
                u64::from_le_bytes(bytes)
            })
            .collect::<Vec<_>>();
        let (rules, veto_rules) = all_rules.split_at(metadata.rule_count);

        Ok(Self {
            schema_version: metadata.schema_version,
            id: metadata.id,
            method: metadata.method,
            locale: metadata.locale,
            source: metadata.source,
            config: metadata.config,
            options: metadata.options,
            rules: rules.to_vec(),
            veto_options: metadata.veto_options,
            veto_rules: veto_rules.to_vec(),
            trained_records: metadata.trained_records,
        })
    }

    fn save_binary(&self, file: File, path: &Path) -> Result<()> {
        let metadata = serde_json::to_vec(&self.metadata())?;
        anyhow::ensure!(
            metadata.len() <= u32::MAX as usize,
            "safe-ngram metadata is too large for binary model"
        );
        let mut writer = BufWriter::new(file);
        writer
            .write_all(b"HYSG1\0\0\0")
            .with_context(|| format!("write magic to {}", path.display()))?;
        writer
            .write_all(&(metadata.len() as u32).to_le_bytes())
            .with_context(|| format!("write metadata length to {}", path.display()))?;
        writer
            .write_all(&metadata)
            .with_context(|| format!("write metadata to {}", path.display()))?;
        for rule in &self.rules {
            writer
                .write_all(&rule.to_le_bytes())
                .with_context(|| format!("write rules to {}", path.display()))?;
        }
        for rule in &self.veto_rules {
            writer
                .write_all(&rule.to_le_bytes())
                .with_context(|| format!("write veto rules to {}", path.display()))?;
        }
        writer.flush()?;
        Ok(())
    }
}

impl ItalianSyllableModelFile {
    fn from_parts(
        method: String,
        locale: String,
        source: String,
        config: HyphenationConfig,
        learned_splits: U64HashMap<u8>,
        trained_records: usize,
    ) -> Self {
        let mut learned_splits = learned_splits.into_iter().collect::<Vec<_>>();
        learned_splits.sort_unstable_by_key(|(key, _)| *key);
        let learned_splits = learned_splits
            .into_iter()
            .map(|(key, split)| ItalianSyllableSplit {
                key: format!("0x{key:016x}"),
                split,
            })
            .collect::<Vec<_>>();
        Self {
            schema_version: 1,
            id: format!(
                "{method}:{source}:clusters{}:n{}",
                learned_splits.len(),
                trained_records
            ),
            method,
            locale,
            source,
            config,
            learned_splits,
            trained_records,
        }
    }

    fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        if path_extension_eq(path, "zst") {
            let decoder = zstd::stream::read::Decoder::new(file)
                .with_context(|| format!("open zstd decoder for {}", path.display()))?;
            serde_json::from_reader(BufReader::new(decoder))
                .with_context(|| format!("parse {}", path.display()))
        } else {
            serde_json::from_reader(BufReader::new(file))
                .with_context(|| format!("parse {}", path.display()))
        }
    }

    fn save(&self, path: &Path) -> Result<()> {
        create_parent(path)?;
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        if path_extension_eq(path, "zst") {
            let writer = BufWriter::new(file);
            let mut encoder = zstd::stream::write::Encoder::new(writer, 3)
                .with_context(|| format!("open zstd encoder for {}", path.display()))?;
            serde_json::to_writer(&mut encoder, self)
                .with_context(|| format!("write {}", path.display()))?;
            encoder.finish()?;
        } else {
            serde_json::to_writer_pretty(BufWriter::new(file), self)
                .with_context(|| format!("write {}", path.display()))?;
        }
        Ok(())
    }

    fn into_learned_splits(self, path: &Path) -> Result<U64HashMap<u8>> {
        let mut learned = U64HashMap::<u8>::default();
        for entry in self.learned_splits {
            let key = parse_u64_key(&entry.key)
                .with_context(|| format!("parse split key in {}", path.display()))?;
            anyhow::ensure!(
                entry.split <= 4,
                "invalid Italian syllable split {} in {}",
                entry.split,
                path.display()
            );
            learned.insert(key, entry.split);
        }
        Ok(learned)
    }
}

fn path_extension_eq(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

const SAFE_NGRAM_SPEC1_PREFIX: u64 = 1u64 << 56;

impl SafeNgramDenseSet {
    fn from_options(rules: &U64HashSet, options: &SafeNgramOptions) -> Option<Self> {
        let spec = options.specs.first().copied()?;
        if options.specs.len() != 1 || spec.bucketed || rules.is_empty() {
            return None;
        }
        let total_bits = (spec.left + spec.right).checked_mul(5)?;
        if total_bits != 25 {
            return None;
        }
        Self::from_unprefixed_keys(total_bits, rules.iter().copied())
    }

    fn from_unprefixed_keys<I>(total_bits: usize, keys: I) -> Option<Self>
    where
        I: IntoIterator<Item = u64>,
    {
        let bit_count = 1usize.checked_shl(total_bits as u32)?;
        let mut bits = vec![0u64; bit_count.div_ceil(64)];
        for key in keys {
            let key = usize::try_from(key).ok()?;
            if key >= bit_count {
                return None;
            }
            bits[key >> 6] |= 1u64 << (key & 63);
        }
        Some(Self { bit_count, bits })
    }

    #[inline]
    fn contains(&self, key: u64) -> bool {
        let Ok(key) = usize::try_from(key) else {
            return false;
        };
        if key >= self.bit_count {
            return false;
        }
        (self.bits[key >> 6] & (1u64 << (key & 63))) != 0
    }
}

impl SafeNgramDualDenseSet {
    fn from_options(rules: &U64HashSet, options: &SafeNgramOptions) -> Option<Self> {
        if options.specs.len() != 2
            || options.specs.iter().any(|spec| spec.bucketed)
            || rules.is_empty()
        {
            return None;
        }
        let first_bits = (options.specs[0].left + options.specs[0].right).checked_mul(5)?;
        let second_bits = (options.specs[1].left + options.specs[1].right).checked_mul(5)?;
        if first_bits != 25 || second_bits != 25 {
            return None;
        }

        let mut first = Vec::new();
        let mut second = Vec::new();
        for key in rules {
            if key & SAFE_NGRAM_SPEC1_PREFIX != 0 {
                second.push(key & !SAFE_NGRAM_SPEC1_PREFIX);
            } else {
                first.push(*key);
            }
        }
        Some(Self {
            first: SafeNgramDenseSet::from_unprefixed_keys(first_bits, first)?,
            second: SafeNgramDenseSet::from_unprefixed_keys(second_bits, second)?,
        })
    }

    #[inline]
    fn contains(&self, key0: u64, key1: u64) -> bool {
        self.first.contains(key0) || self.second.contains(key1)
    }
}

impl SafeNgramRadixSet {
    fn from_options(rules: &U64HashSet, options: &SafeNgramOptions) -> Option<Self> {
        // The prefix-bucketed layout is kept as an experimental fallback, but
        // current speed tests favor the identity-hashed table.
        const ENABLE_SAFE_NGRAM_RADIX_SET: bool = false;
        if !ENABLE_SAFE_NGRAM_RADIX_SET {
            return None;
        }
        let spec = options.specs.first().copied()?;
        if options.specs.len() != 1 || spec.bucketed || rules.is_empty() {
            return None;
        }
        Self::from_rules(rules, spec)
    }

    fn from_rules(rules: &U64HashSet, spec: SafeNgramSpec) -> Option<Self> {
        let total_bits = (spec.left + spec.right).checked_mul(5)?;
        if total_bits == 0 || total_bits > 35 {
            return None;
        }
        let low_bits = total_bits.min(16) as u8;
        let high_bits = total_bits - usize::from(low_bits);
        if high_bits > 20 {
            return None;
        }
        let bucket_count = 1usize.checked_shl(high_bits as u32)?;
        let low_mask = if low_bits == 64 {
            u64::MAX
        } else {
            (1u64 << low_bits) - 1
        };

        let mut keys = rules.iter().copied().collect::<Vec<_>>();
        keys.sort_unstable();

        let mut offsets = vec![0u32; bucket_count + 1];
        for key in &keys {
            let bucket = (*key >> low_bits) as usize;
            if bucket >= bucket_count {
                return None;
            }
            offsets[bucket + 1] = offsets[bucket + 1].saturating_add(1);
        }
        for index in 1..offsets.len() {
            offsets[index] = offsets[index].saturating_add(offsets[index - 1]);
        }

        let lows = keys
            .into_iter()
            .map(|key| (key & low_mask) as u16)
            .collect::<Vec<_>>();
        Some(Self {
            low_bits,
            low_mask,
            offsets,
            lows,
        })
    }

    fn contains(&self, key: u64) -> bool {
        let bucket = (key >> self.low_bits) as usize;
        let Some((&start, &end)) = self.offsets.get(bucket).zip(self.offsets.get(bucket + 1))
        else {
            return false;
        };
        let range = &self.lows[start as usize..end as usize];
        let low = (key & self.low_mask) as u16;
        if range.len() <= 8 {
            range.iter().any(|value| *value == low)
        } else {
            range.binary_search(&low).is_ok()
        }
    }
}

impl<'a> SafeNgramRuleLookup<'a> {
    fn contains(self, key: u64) -> bool {
        match self {
            Self::Hash(rules) => rules.contains(&key),
            Self::Dense(rules) => rules.contains(key),
            Self::Radix(rules) => rules.contains(key),
        }
    }
}

impl<'a> SafeNgramDualRuleLookup<'a> {
    #[inline]
    fn contains(self, key0: u64, key1: u64) -> bool {
        match self {
            Self::Hash(rules) => {
                rules.contains(&key0) || rules.contains(&(SAFE_NGRAM_SPEC1_PREFIX | key1))
            }
            Self::Dense(rules) => rules.contains(key0, key1),
        }
    }
}

impl SafeNgramMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let (options, veto_options) = parse_safe_ngram_veto_options(method)?;
        let _language = locale
            .parse::<LanguageTag>()
            .map_err(|err| anyhow::anyhow!("parse locale {locale:?}: {err}"))?;
        let (rules, trained_records) = learn_safe_ngram_rules(records, &config, &options);
        let veto_rules = if let Some(veto_options) = &veto_options {
            learn_safe_ngram_veto_rules(records, &config, &options, &rules, veto_options)
        } else {
            U64HashSet::default()
        };

        anyhow::ensure!(
            !rules.is_empty(),
            "safe-ngram learned no rules from {} with method {method:?}",
            path.display()
        );

        let id = format!(
            "{method}:{}:r{}:v{}:n{}",
            file_stem(path),
            rules.len(),
            veto_rules.len(),
            trained_records
        );
        let rules_dense = SafeNgramDenseSet::from_options(&rules, &options);
        let rules_dual_dense = SafeNgramDualDenseSet::from_options(&rules, &options);
        let rules_radix = SafeNgramRadixSet::from_options(&rules, &options);
        let veto_rules_dense = veto_options
            .as_ref()
            .and_then(|options| SafeNgramDenseSet::from_options(&veto_rules, options));
        let veto_rules_dual_dense = veto_options
            .as_ref()
            .and_then(|options| SafeNgramDualDenseSet::from_options(&veto_rules, options));
        let veto_rules_radix = veto_options
            .as_ref()
            .and_then(|options| SafeNgramRadixSet::from_options(&veto_rules, options));
        let uses_unicode_features =
            safe_ngram_uses_unicode_features(&options, veto_options.as_ref());
        let family_mask = safe_ngram_family_mask(&options, veto_options.as_ref());
        Ok(Self {
            id,
            config,
            options,
            uses_unicode_features,
            family_mask,
            rules,
            rules_dense,
            rules_dual_dense,
            rules_radix,
            veto_options,
            veto_rules,
            veto_rules_dense,
            veto_rules_dual_dense,
            veto_rules_radix,
        })
    }

    fn from_model(path: &Path, locale: &str, model: SafeNgramModelFile) -> Result<Self> {
        anyhow::ensure!(
            model.schema_version == 1,
            "unsupported safe-ngram model schema version {} in {}",
            model.schema_version,
            path.display()
        );
        anyhow::ensure!(
            normalize_locale_match_key(locale) == normalize_locale_match_key(&model.locale),
            "safe-ngram model locale {} does not match requested locale {}",
            model.locale,
            locale
        );
        anyhow::ensure!(
            !model.rules.is_empty(),
            "safe-ngram model {} has no rules",
            path.display()
        );
        let options = model.options;
        let veto_options = model.veto_options;
        let rules = model.rules.into_iter().collect::<U64HashSet>();
        let veto_rules = model.veto_rules.into_iter().collect::<U64HashSet>();
        let rules_dense = SafeNgramDenseSet::from_options(&rules, &options);
        let rules_dual_dense = SafeNgramDualDenseSet::from_options(&rules, &options);
        let rules_radix = SafeNgramRadixSet::from_options(&rules, &options);
        let veto_rules_dense = veto_options
            .as_ref()
            .and_then(|options| SafeNgramDenseSet::from_options(&veto_rules, options));
        let veto_rules_dual_dense = veto_options
            .as_ref()
            .and_then(|options| SafeNgramDualDenseSet::from_options(&veto_rules, options));
        let veto_rules_radix = veto_options
            .as_ref()
            .and_then(|options| SafeNgramRadixSet::from_options(&veto_rules, options));
        let uses_unicode_features =
            safe_ngram_uses_unicode_features(&options, veto_options.as_ref());
        let family_mask = safe_ngram_family_mask(&options, veto_options.as_ref());
        Ok(Self {
            id: format!("{}:model:{}", model.id, file_stem(path)),
            config: model.config,
            options,
            uses_unicode_features,
            family_mask,
            rules,
            rules_dense,
            rules_dual_dense,
            rules_radix,
            veto_options,
            veto_rules,
            veto_rules_dense,
            veto_rules_dual_dense,
            veto_rules_radix,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn rules_lookup(&self) -> SafeNgramRuleLookup<'_> {
        if let Some(rules) = &self.rules_dense {
            SafeNgramRuleLookup::Dense(rules)
        } else if let Some(rules) = &self.rules_radix {
            SafeNgramRuleLookup::Radix(rules)
        } else {
            SafeNgramRuleLookup::Hash(&self.rules)
        }
    }

    fn rules_dual_lookup(&self) -> SafeNgramDualRuleLookup<'_> {
        if let Some(rules) = &self.rules_dual_dense {
            SafeNgramDualRuleLookup::Dense(rules)
        } else {
            SafeNgramDualRuleLookup::Hash(&self.rules)
        }
    }

    fn veto_rules_lookup(&self) -> SafeNgramRuleLookup<'_> {
        if let Some(rules) = &self.veto_rules_dense {
            SafeNgramRuleLookup::Dense(rules)
        } else if let Some(rules) = &self.veto_rules_radix {
            SafeNgramRuleLookup::Radix(rules)
        } else {
            SafeNgramRuleLookup::Hash(&self.veto_rules)
        }
    }

    fn veto_rules_dual_lookup(&self) -> SafeNgramDualRuleLookup<'_> {
        if let Some(rules) = &self.veto_rules_dual_dense {
            SafeNgramDualRuleLookup::Dense(rules)
        } else {
            SafeNgramDualRuleLookup::Hash(&self.veto_rules)
        }
    }

    fn uses_unicode_features(&self) -> bool {
        self.uses_unicode_features
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        if self.uses_unicode_features() && !word.is_ascii() {
            self.hyphenate_unicode_into(word, out);
            return Ok(());
        }
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        if self.options.specs.len() == 1 && !self.options.cap_vowel_nuclei {
            let spec = self.options.specs[0];
            if let Some(veto_options) = &self.veto_options {
                if veto_options.specs.len() == 1
                    && !spec.bucketed
                    && !veto_options.specs[0].bucketed
                {
                    let veto_spec = veto_options.specs[0];
                    match (spec.family, veto_spec.family) {
                        (1, 1) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_cv_code_at,
                            safe_ngram_cv_code_at,
                        ),
                        (1, 2) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_cv_code_at,
                            safe_ngram_sonority_code_at,
                        ),
                        (1, _) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_cv_code_at,
                            safe_ngram_raw_code_at,
                        ),
                        (2, 1) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_sonority_code_at,
                            safe_ngram_cv_code_at,
                        ),
                        (2, 2) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_sonority_code_at,
                            safe_ngram_sonority_code_at,
                        ),
                        (2, _) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_sonority_code_at,
                            safe_ngram_raw_code_at,
                        ),
                        (_, 1) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_raw_code_at,
                            safe_ngram_cv_code_at,
                        ),
                        (_, 2) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_raw_code_at,
                            safe_ngram_sonority_code_at,
                        ),
                        (_, _) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_raw_code_at,
                            safe_ngram_raw_code_at,
                        ),
                    }
                    if self.options.orthographic_veto {
                        safe_ngram_apply_orthographic_veto(bytes, out);
                    }
                    return Ok(());
                }
            } else if !spec.bucketed {
                let use_rule_lookup = self.rules_dense.is_some() || self.rules_radix.is_some();
                match spec.family {
                    1 if use_rule_lookup => safe_ngram_hyphenate_single_spec_lookup(
                        bytes,
                        &self.config,
                        spec,
                        self.rules_lookup(),
                        out,
                        safe_ngram_cv_code_at,
                    ),
                    1 => safe_ngram_hyphenate_single_spec(
                        bytes,
                        &self.config,
                        spec,
                        &self.rules,
                        out,
                        safe_ngram_cv_code_at,
                    ),
                    2 if use_rule_lookup => safe_ngram_hyphenate_single_spec_lookup(
                        bytes,
                        &self.config,
                        spec,
                        self.rules_lookup(),
                        out,
                        safe_ngram_sonority_code_at,
                    ),
                    2 => safe_ngram_hyphenate_single_spec(
                        bytes,
                        &self.config,
                        spec,
                        &self.rules,
                        out,
                        safe_ngram_sonority_code_at,
                    ),
                    _ if use_rule_lookup => safe_ngram_hyphenate_single_spec_lookup(
                        bytes,
                        &self.config,
                        spec,
                        self.rules_lookup(),
                        out,
                        safe_ngram_raw_code_at,
                    ),
                    _ => safe_ngram_hyphenate_single_spec(
                        bytes,
                        &self.config,
                        spec,
                        &self.rules,
                        out,
                        safe_ngram_raw_code_at,
                    ),
                }
                if self.options.orthographic_veto {
                    safe_ngram_apply_orthographic_veto(bytes, out);
                }
            } else {
                let rules = self.rules_lookup();
                for boundary in
                    self.config.left_min..=bytes.len().saturating_sub(self.config.right_min)
                {
                    if rules.contains(safe_ngram_key(bytes, boundary, 0, spec)) {
                        out.push(boundary as GraphemeIndex);
                    }
                }
                if self.options.orthographic_veto {
                    safe_ngram_apply_orthographic_veto(bytes, out);
                }
            }
            return Ok(());
        }
        let rules = self.rules_lookup();
        let veto_rules = self.veto_rules_lookup();
        if !self.options.cap_vowel_nuclei
            && self.options.specs.len() == 2
            && self.options.specs.iter().all(|spec| !spec.bucketed)
        {
            let add_spec0 = self.options.specs[0];
            let add_spec1 = self.options.specs[1];
            if let Some(veto_options) = &self.veto_options {
                if veto_options.specs.len() == 2
                    && veto_options.specs.iter().all(|spec| !spec.bucketed)
                {
                    let veto_spec0 = veto_options.specs[0];
                    let veto_spec1 = veto_options.specs[1];
                    safe_ngram_hyphenate_dual_add_veto_lookup(
                        bytes,
                        &self.config,
                        add_spec0,
                        add_spec1,
                        self.rules_dual_lookup(),
                        veto_spec0,
                        veto_spec1,
                        self.veto_rules_dual_lookup(),
                        out,
                    );
                    if self.options.orthographic_veto {
                        safe_ngram_apply_orthographic_veto(bytes, out);
                    }
                    return Ok(());
                }
                if veto_options.specs.len() == 1 && !veto_options.specs[0].bucketed {
                    let veto_spec = veto_options.specs[0];
                    safe_ngram_hyphenate_dual_add_single_veto_lookup(
                        bytes,
                        &self.config,
                        add_spec0,
                        add_spec1,
                        self.rules_dual_lookup(),
                        veto_spec,
                        self.veto_rules_lookup(),
                        out,
                    );
                    if self.options.orthographic_veto {
                        safe_ngram_apply_orthographic_veto(bytes, out);
                    }
                    return Ok(());
                }
            } else {
                safe_ngram_hyphenate_dual_spec_lookup(
                    bytes,
                    &self.config,
                    add_spec0,
                    add_spec1,
                    self.rules_dual_lookup(),
                    out,
                );
                if self.options.orthographic_veto {
                    safe_ngram_apply_orthographic_veto(bytes, out);
                }
                return Ok(());
            }
        }
        if !self.options.cap_vowel_nuclei {
            for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min)
            {
                let add_hit = self
                    .options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        rules.contains(safe_ngram_key(bytes, boundary, spec_idx, *spec))
                    });
                if !add_hit {
                    continue;
                }
                let veto_hit = self.veto_options.as_ref().is_some_and(|veto_options| {
                    veto_options
                        .specs
                        .iter()
                        .enumerate()
                        .any(|(spec_idx, spec)| {
                            veto_rules.contains(safe_ngram_key(bytes, boundary, spec_idx, *spec))
                        })
                });
                if !veto_hit {
                    out.push(boundary as GraphemeIndex);
                }
            }
            if self.options.orthographic_veto {
                safe_ngram_apply_orthographic_veto(bytes, out);
            }
            return Ok(());
        }
        let mut scored = SmallVec::<[(u32, GraphemeIndex); 8]>::new();
        for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min) {
            let add_score = self
                .options
                .specs
                .iter()
                .enumerate()
                .filter(|(spec_idx, spec)| {
                    rules.contains(safe_ngram_key(bytes, boundary, *spec_idx, **spec))
                })
                .count() as u32;
            if add_score == 0 {
                continue;
            }
            let veto_hit = self.veto_options.as_ref().is_some_and(|veto_options| {
                veto_options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        veto_rules.contains(safe_ngram_key(bytes, boundary, spec_idx, *spec))
                    })
            });
            if !veto_hit {
                let boundary = boundary as GraphemeIndex;
                out.push(boundary);
                if self.options.cap_vowel_nuclei {
                    scored.push((add_score, boundary));
                }
            }
        }
        if self.options.orthographic_veto {
            safe_ngram_apply_orthographic_veto(bytes, out);
            if self.options.cap_vowel_nuclei {
                scored.retain(|(_, boundary)| out.contains(boundary));
            }
        }
        if self.options.cap_vowel_nuclei {
            let cap = stacked_vowel_break_cap(bytes);
            if out.len() > cap {
                scored.sort_by(|left, right| {
                    right
                        .0
                        .cmp(&left.0)
                        .then_with(|| {
                            left.1
                                .abs_diff(bytes.len() as GraphemeIndex / 2)
                                .cmp(&right.1.abs_diff(bytes.len() as GraphemeIndex / 2))
                        })
                        .then_with(|| left.1.cmp(&right.1))
                });
                scored.truncate(cap);
                out.clear();
                out.extend(scored.into_iter().map(|(_, boundary)| boundary));
                out.sort_unstable();
            }
        }
        Ok(())
    }

    fn hyphenate_unicode_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) {
        let char_tables = safe_ngram_char_tables_if_simple(word, self.family_mask);
        let grapheme_tables;
        let tables = if let Some(tables) = char_tables.as_ref() {
            tables
        } else {
            grapheme_tables = safe_ngram_grapheme_tables(word, self.family_mask);
            &grapheme_tables
        };
        let grapheme_len = tables.len;
        if grapheme_len < self.config.min_word_len {
            return;
        }
        let start = self.config.left_min;
        let end = grapheme_len.saturating_sub(self.config.right_min);
        if start > end {
            return;
        }

        if self.options.specs.len() == 1 {
            let spec = self.options.specs[0];
            if !spec.bucketed {
                if let Some(veto_options) = &self.veto_options {
                    if veto_options.specs.len() == 1 && !veto_options.specs[0].bucketed {
                        let veto_spec = veto_options.specs[0];
                        safe_ngram_hyphenate_grapheme_single_add_veto_lookup(
                            tables.codes(spec.family),
                            tables.codes(veto_spec.family),
                            grapheme_len,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                        );
                        return;
                    }
                } else if self.rules_dense.is_some() || self.rules_radix.is_some() {
                    safe_ngram_hyphenate_grapheme_single_spec_lookup(
                        tables.codes(spec.family),
                        grapheme_len,
                        &self.config,
                        spec,
                        self.rules_lookup(),
                        out,
                    );
                    return;
                } else {
                    safe_ngram_hyphenate_grapheme_single_spec(
                        tables.codes(spec.family),
                        grapheme_len,
                        &self.config,
                        spec,
                        &self.rules,
                        out,
                    );
                    return;
                }
            }
        }

        let rules = self.rules_lookup();
        let veto_rules = self.veto_rules_lookup();
        if !self.options.cap_vowel_nuclei
            && self.options.specs.len() == 2
            && self.options.specs.iter().all(|spec| !spec.bucketed)
        {
            let add_spec0 = self.options.specs[0];
            let add_spec1 = self.options.specs[1];
            if let Some(veto_options) = &self.veto_options {
                if veto_options.specs.len() == 2
                    && veto_options.specs.iter().all(|spec| !spec.bucketed)
                {
                    let veto_spec0 = veto_options.specs[0];
                    let veto_spec1 = veto_options.specs[1];
                    safe_ngram_hyphenate_grapheme_dual_add_veto_lookup(
                        tables.codes(add_spec0.family),
                        tables.codes(add_spec1.family),
                        tables.codes(veto_spec0.family),
                        tables.codes(veto_spec1.family),
                        grapheme_len,
                        &self.config,
                        add_spec0,
                        add_spec1,
                        self.rules_dual_lookup(),
                        veto_spec0,
                        veto_spec1,
                        self.veto_rules_dual_lookup(),
                        out,
                    );
                    return;
                }
                if veto_options.specs.len() == 1 && !veto_options.specs[0].bucketed {
                    let veto_spec = veto_options.specs[0];
                    safe_ngram_hyphenate_grapheme_dual_add_single_veto_lookup(
                        tables.codes(add_spec0.family),
                        tables.codes(add_spec1.family),
                        tables.codes(veto_spec.family),
                        grapheme_len,
                        &self.config,
                        add_spec0,
                        add_spec1,
                        self.rules_dual_lookup(),
                        veto_spec,
                        self.veto_rules_lookup(),
                        out,
                    );
                    return;
                }
            } else {
                safe_ngram_hyphenate_grapheme_dual_spec_lookup(
                    tables.codes(add_spec0.family),
                    tables.codes(add_spec1.family),
                    grapheme_len,
                    &self.config,
                    add_spec0,
                    add_spec1,
                    self.rules_dual_lookup(),
                    out,
                );
                return;
            }
        }
        for boundary in start..=end {
            let add_hit = self
                .options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    let key =
                        safe_ngram_grapheme_key(&tables, grapheme_len, boundary, spec_idx, *spec);
                    rules.contains(key)
                });
            if !add_hit {
                continue;
            }
            let veto_hit = self.veto_options.as_ref().is_some_and(|veto_options| {
                veto_options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        let key = safe_ngram_grapheme_key(
                            &tables,
                            grapheme_len,
                            boundary,
                            spec_idx,
                            *spec,
                        );
                        veto_rules.contains(key)
                    })
            });
            if !veto_hit {
                out.push(boundary as GraphemeIndex);
            }
        }
    }
}

impl ItalianSyllableMethod {
    fn new(method: &str, options: &MethodOptions) -> Result<Self> {
        anyhow::ensure!(
            normalize_locale_match_key(&options.locale).starts_with("it"),
            "italian-syllable requires an Italian locale, got {}",
            options.locale
        );
        let mut config = italian_syllable_default_config();
        apply_config_overrides(&mut config, options);
        let learned_splits = if let Some(path) = options.dictionary.as_ref() {
            let records = read_records(path)?;
            learn_italian_syllable_splits(&records, &config)
        } else {
            U64HashMap::default()
        };
        Ok(Self {
            id: format!("{method}:clusters{}", learned_splits.len()),
            config,
            learned_splits,
        })
    }

    fn from_model(path: &Path, locale: &str, model: ItalianSyllableModelFile) -> Result<Self> {
        anyhow::ensure!(
            model.schema_version == 1,
            "unsupported Italian syllable model schema version {} in {}",
            model.schema_version,
            path.display()
        );
        anyhow::ensure!(
            normalize_locale_match_key(locale).starts_with("it"),
            "italian-syllable-model requires an Italian locale, got {}",
            locale
        );
        anyhow::ensure!(
            normalize_locale_match_key(&model.locale).starts_with("it"),
            "Italian syllable model locale {} is not Italian in {}",
            model.locale,
            path.display()
        );
        let id = model.id.clone();
        let config = model.config.clone();
        let learned_splits = model.into_learned_splits(path)?;
        Ok(Self {
            id: format!("{id}:model:{}", file_stem(path)),
            config,
            learned_splits,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        let chars = word
            .chars()
            .map(italian_lower_char)
            .collect::<SmallVec<[char; 32]>>();
        let len = chars.len();
        if len < self.config.min_word_len {
            return Ok(());
        }

        let mut vowels = SmallVec::<[usize; 8]>::new();
        for idx in 0..len {
            if italian_is_vowel_nucleus(&chars, idx) {
                vowels.push(idx);
            }
        }
        if vowels.len() < 2 {
            return Ok(());
        }

        for pair in vowels.windows(2) {
            let left_vowel = pair[0];
            let right_vowel = pair[1];
            if right_vowel <= left_vowel + 1 {
                let key = italian_adjacent_vowels_key(&chars, left_vowel, right_vowel);
                let learned = self.learned_splits.get(&key).copied();
                let should_break = learned.map(|split| split != 0).unwrap_or_else(|| {
                    italian_adjacent_vowels_break(&chars, left_vowel, right_vowel)
                });
                if should_break {
                    self.push_italian_boundary(left_vowel + 1, len, out);
                }
                continue;
            }

            let cluster_start = left_vowel + 1;
            let cluster_end = right_vowel;
            let cluster_len = cluster_end - cluster_start;
            if cluster_len == 0 {
                continue;
            }
            if italian_cluster_is_all_non_letters(&chars[cluster_start..cluster_end]) {
                continue;
            }

            let key = italian_cluster_key(&chars[cluster_start..cluster_end]);
            let learned = self.learned_splits.get(&key).copied();
            if learned == Some(0) {
                continue;
            }
            let onset_len = learned
                .map(usize::from)
                .unwrap_or_else(|| italian_best_onset_len(&chars, cluster_start, cluster_end));
            let boundary = cluster_end.saturating_sub(onset_len.max(1));
            self.push_italian_boundary(boundary, len, out);
        }

        out.sort_unstable();
        out.dedup();
        Ok(())
    }

    fn push_italian_boundary(
        &self,
        boundary: usize,
        len: usize,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) {
        if boundary >= self.config.left_min && len.saturating_sub(boundary) >= self.config.right_min
        {
            out.push(boundary as GraphemeIndex);
        }
    }
}

fn italian_syllable_default_config() -> HyphenationConfig {
    HyphenationConfig {
        right_min: 2,
        min_word_len: 4,
        ..HyphenationConfig::default()
    }
}

fn italian_lower_char(ch: char) -> char {
    if ch.is_ascii() {
        ch.to_ascii_lowercase()
    } else {
        ch.to_lowercase().next().unwrap_or(ch)
    }
}

fn learn_italian_syllable_splits(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
) -> U64HashMap<u8> {
    let mut counts = U64HashMap::<ItalianSplitCounts>::default();
    for record in records {
        if record.ambiguous {
            continue;
        }
        let chars = record
            .word
            .chars()
            .map(italian_lower_char)
            .collect::<SmallVec<[char; 32]>>();
        let len = chars.len();
        if len < config.min_word_len {
            continue;
        }
        let mut vowels = SmallVec::<[usize; 8]>::new();
        for idx in 0..len {
            if italian_is_vowel_nucleus(&chars, idx) {
                vowels.push(idx);
            }
        }
        for pair in vowels.windows(2) {
            let left_vowel = pair[0];
            let right_vowel = pair[1];
            let cluster_start = left_vowel + 1;
            let cluster_end = right_vowel;
            let Some(gold_boundary) =
                italian_gold_boundary_in_interval(&record.breaks, cluster_start, cluster_end)
            else {
                continue;
            };
            if right_vowel <= left_vowel + 1 {
                let key = italian_adjacent_vowels_key(&chars, left_vowel, right_vowel);
                let split = if gold_boundary == 0 { 0 } else { 1 };
                let slot = counts.entry(key).or_default();
                slot.counts[split] = slot.counts[split].saturating_add(1);
                continue;
            }
            if italian_cluster_is_all_non_letters(&chars[cluster_start..cluster_end]) {
                continue;
            }
            let key = italian_cluster_key(&chars[cluster_start..cluster_end]);
            let split = if gold_boundary == 0 {
                0
            } else {
                (cluster_end.saturating_sub(gold_boundary as usize)).min(4)
            };
            let slot = counts.entry(key).or_default();
            slot.counts[split] = slot.counts[split].saturating_add(1);
        }
    }

    let mut learned = U64HashMap::<u8>::default();
    for (key, split_counts) in counts {
        let total = split_counts.counts.iter().copied().sum::<u32>();
        if total < 2 {
            continue;
        }
        let (best_split, best_count) = split_counts
            .counts
            .iter()
            .copied()
            .enumerate()
            .max_by_key(|(_, count)| *count)
            .unwrap_or((0, 0));
        if best_count.saturating_mul(4) >= total.saturating_mul(3) {
            learned.insert(key, best_split as u8);
        }
    }
    learned
}

fn count_italian_syllable_training_records(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
) -> usize {
    records
        .iter()
        .filter(|record| !record.ambiguous && record.word.chars().count() >= config.min_word_len)
        .count()
}

fn italian_gold_boundary_in_interval(
    breaks: &[GraphemeIndex],
    cluster_start: usize,
    cluster_end: usize,
) -> Option<GraphemeIndex> {
    let mut found = None;
    for boundary in breaks.iter().copied() {
        let boundary_usize = boundary as usize;
        if (cluster_start..=cluster_end).contains(&boundary_usize) {
            if found.is_some() {
                return None;
            }
            found = Some(boundary);
        }
    }
    Some(found.unwrap_or(0))
}

fn italian_adjacent_vowels_key(chars: &[char], left: usize, right: usize) -> u64 {
    0xA0u64 << 56
        | (u64::from(italian_char_code(chars[left])) << 8)
        | u64::from(italian_char_code(chars[right]))
}

fn italian_cluster_key(cluster: &[char]) -> u64 {
    let mut key = (cluster.len().min(7) as u64) << 56;
    for (idx, ch) in cluster.iter().take(7).enumerate() {
        key |= u64::from(italian_char_code(*ch)) << (idx * 8);
    }
    key
}

fn italian_char_code(ch: char) -> u8 {
    match ch {
        'a' | 'à' | 'á' | 'â' | 'ä' => b'a',
        'e' | 'è' | 'é' | 'ê' | 'ë' => b'e',
        'i' | 'ì' | 'í' | 'î' | 'ï' => b'i',
        'o' | 'ò' | 'ó' | 'ô' | 'ö' => b'o',
        'u' | 'ù' | 'ú' | 'û' | 'ü' => b'u',
        ch if ch.is_ascii() => ch as u8,
        _ => 0x7f,
    }
}

fn italian_is_vowel_char(ch: char) -> bool {
    matches!(
        ch,
        'a' | 'à'
            | 'á'
            | 'â'
            | 'ä'
            | 'e'
            | 'è'
            | 'é'
            | 'ê'
            | 'ë'
            | 'i'
            | 'ì'
            | 'í'
            | 'î'
            | 'ï'
            | 'o'
            | 'ò'
            | 'ó'
            | 'ô'
            | 'ö'
            | 'u'
            | 'ù'
            | 'ú'
            | 'û'
            | 'ü'
    )
}

fn italian_is_vowel_nucleus(chars: &[char], idx: usize) -> bool {
    let ch = chars[idx];
    if !italian_is_vowel_char(ch) {
        return false;
    }
    if ch == 'u'
        && idx > 0
        && matches!(chars[idx - 1], 'q' | 'g')
        && idx + 1 < chars.len()
        && italian_is_vowel_char(chars[idx + 1])
    {
        return false;
    }
    true
}

fn italian_is_consonant_char(ch: char) -> bool {
    (ch.is_alphabetic() || ch == '\'') && !italian_is_vowel_char(ch)
}

fn italian_cluster_is_all_non_letters(cluster: &[char]) -> bool {
    cluster.iter().all(|ch| !ch.is_alphabetic())
}

fn italian_adjacent_vowels_break(chars: &[char], left: usize, right: usize) -> bool {
    let l = chars[left];
    let r = chars[right];
    if matches!((l, r), ('i', _) | ('u', _) | (_, 'i') | (_, 'u')) {
        return false;
    }
    l != r
}

fn italian_best_onset_len(chars: &[char], start: usize, end: usize) -> usize {
    let cluster_len = end - start;
    let max_onset = cluster_len.min(3);
    for onset_len in (1..=max_onset).rev() {
        let onset_start = end - onset_len;
        if italian_legal_onset(&chars[onset_start..end]) {
            return onset_len;
        }
    }
    1
}

fn italian_legal_onset(onset: &[char]) -> bool {
    match onset {
        [a] => italian_is_consonant_char(*a) && *a != 'h',
        ['q', 'u'] | ['g', 'u'] | ['c', 'h'] | ['g', 'h'] | ['g', 'n'] => true,
        ['g', 'l'] => true,
        ['s', b] => italian_is_consonant_char(*b),
        [a, b] if matches!(*b, 'l' | 'r') => {
            matches!(*a, 'b' | 'c' | 'd' | 'f' | 'g' | 'p' | 't' | 'v')
        }
        ['s', a, b] => italian_legal_onset(&[*a, *b]),
        _ => false,
    }
}

impl BoundaryBayesMethod {
    fn train(
        method: &str,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_boundary_bayes_options(method)?;
        let mut counts = U64HashMap::<BoundaryBayesCounts>::default();
        let mut total_positive = 0u32;
        let mut total_negative = 0u32;
        let mut trained_records = 0usize;
        let mut features = SmallVec::<[u64; 32]>::new();

        for record in records {
            if record.ambiguous || !record.word.is_ascii() {
                continue;
            }
            let bytes = record.word.as_bytes();
            if bytes.len() < config.min_word_len {
                continue;
            }
            trained_records += 1;
            for boundary in config.left_min..=bytes.len().saturating_sub(config.right_min) {
                let positive = record.breaks.contains(&(boundary as GraphemeIndex));
                if positive {
                    total_positive = total_positive.saturating_add(1);
                } else {
                    total_negative = total_negative.saturating_add(1);
                }
                boundary_bayes_features(bytes, boundary, &mut features);
                for key in features.iter().copied() {
                    let slot = counts.entry(key).or_default();
                    if positive {
                        slot.positive = slot.positive.saturating_add(1);
                    } else {
                        slot.negative = slot.negative.saturating_add(1);
                    }
                }
            }
        }

        anyhow::ensure!(
            total_positive > 0 && total_negative > 0,
            "boundary-bayes needs positive and negative training boundaries in {}",
            path.display()
        );

        let alpha = options.alpha;
        let positive_den = total_positive as f32 + 2.0 * alpha;
        let negative_den = total_negative as f32 + 2.0 * alpha;
        let mut weights = U64HashMap::<f32>::default();
        for (key, feature_counts) in counts {
            if feature_counts
                .positive
                .saturating_add(feature_counts.negative)
                < options.min_support
            {
                continue;
            }
            let p_feature_given_positive = (feature_counts.positive as f32 + alpha) / positive_den;
            let p_feature_given_negative = (feature_counts.negative as f32 + alpha) / negative_den;
            let weight = (p_feature_given_positive / p_feature_given_negative).ln();
            if weight.abs() >= 0.01 {
                weights.insert(key, weight);
            }
        }

        let bias = ((total_positive as f32 + alpha) / (total_negative as f32 + alpha)).ln();
        anyhow::ensure!(
            !weights.is_empty(),
            "boundary-bayes learned no features from {} with method {method:?}",
            path.display()
        );

        Ok(Self {
            id: format!(
                "{method}:{}:f{}:n{}",
                file_stem(path),
                weights.len(),
                trained_records
            ),
            config,
            options,
            weights,
            bias,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        let mut features = SmallVec::<[u64; 32]>::new();
        for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min) {
            boundary_bayes_features(bytes, boundary, &mut features);
            let mut score = self.bias;
            for key in features.iter().copied() {
                if let Some(weight) = self.weights.get(&key) {
                    score += *weight;
                }
            }
            if score >= self.options.threshold {
                out.push(boundary as GraphemeIndex);
            }
        }
        Ok(())
    }
}

impl StackedBayesMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_stacked_bayes_options(method)?;
        let hypher = adapter_for_method("hypher", locale)?;
        let liang = if let Some(patterns) = patterns {
            Some(Box::new(prepare_liang(MethodOptions {
                method: "liang".to_string(),
                locale: locale.to_string(),
                patterns: Some(patterns.clone()),
                dictionary: None,
                dictionary_is_gold_oracle: false,
                external_command: None,
                left_min: Some(config.left_min),
                right_min: Some(config.right_min),
                min_word_len: Some(config.min_word_len),
            })?))
        } else {
            None
        };
        let safe_p65 = Self::train_safe_base(
            "safe-ngram-multi-s1-p65-veto-multi-s1-n0",
            locale,
            path,
            config.clone(),
            records,
        )?;
        let safe_mixson_p50 = Self::train_safe_base(
            "safe-ngram-mixson-multi-s1-p50-veto-multi-s1-n0",
            locale,
            path,
            config.clone(),
            records,
        )?;
        let safe_p40_mixcv = Self::train_safe_base(
            "safe-ngram-multi-s1-p40-veto-mixcv-multi-s1-n0",
            locale,
            path,
            config.clone(),
            records,
        )?;

        let mut model = Self {
            id: String::new(),
            config,
            options,
            threshold: 0.0,
            weights: U64HashMap::default(),
            bias: 0.0,
            hypher,
            liang,
            safe_p65,
            safe_mixson_p50,
            safe_p40_mixcv,
        };
        let fit_records = records
            .iter()
            .filter(|record| model.stacked_bayes_fit_record(record))
            .cloned()
            .collect::<Vec<_>>();
        let calibration_records = records
            .iter()
            .filter(|record| model.stacked_bayes_calibration_record(record))
            .cloned()
            .collect::<Vec<_>>();
        let fit_slice = if fit_records.is_empty() {
            records
        } else {
            fit_records.as_slice()
        };
        model.fit_weights(fit_slice)?;
        model.threshold = if calibration_records.is_empty() {
            model.calibrate_threshold(fit_slice)?
        } else {
            model.calibrate_threshold(&calibration_records)?
        };
        model.id = format!(
            "{method}:{}:f{}:thr{:.3}:cal{}",
            file_stem(path),
            model.weights.len(),
            model.threshold,
            model.options.calibration_percent
        );
        Ok(model)
    }

    fn train_logit(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_stacked_bayes_options(method)?;
        let hypher = adapter_for_method("hypher", locale)?;
        let liang = if let Some(patterns) = patterns {
            Some(Box::new(prepare_liang(MethodOptions {
                method: "liang".to_string(),
                locale: locale.to_string(),
                patterns: Some(patterns.clone()),
                dictionary: None,
                dictionary_is_gold_oracle: false,
                external_command: None,
                left_min: Some(config.left_min),
                right_min: Some(config.right_min),
                min_word_len: Some(config.min_word_len),
            })?))
        } else {
            None
        };
        let safe_p65 = Self::train_safe_base(
            "safe-ngram-multi-s1-p65-veto-multi-s1-n0",
            locale,
            path,
            config.clone(),
            records,
        )?;
        let safe_mixson_p50 = Self::train_safe_base(
            "safe-ngram-mixson-multi-s1-p50-veto-multi-s1-n0",
            locale,
            path,
            config.clone(),
            records,
        )?;
        let safe_p40_mixcv = Self::train_safe_base(
            "safe-ngram-multi-s1-p40-veto-mixcv-multi-s1-n0",
            locale,
            path,
            config.clone(),
            records,
        )?;

        let mut model = Self {
            id: String::new(),
            config,
            options,
            threshold: 0.0,
            weights: U64HashMap::default(),
            bias: 0.0,
            hypher,
            liang,
            safe_p65,
            safe_mixson_p50,
            safe_p40_mixcv,
        };
        let fit_records = records
            .iter()
            .filter(|record| model.stacked_bayes_fit_record(record))
            .cloned()
            .collect::<Vec<_>>();
        let calibration_records = records
            .iter()
            .filter(|record| model.stacked_bayes_calibration_record(record))
            .cloned()
            .collect::<Vec<_>>();
        let fit_slice = if fit_records.is_empty() {
            records
        } else {
            fit_records.as_slice()
        };
        model.fit_logistic(fit_slice)?;
        model.threshold = if calibration_records.is_empty() {
            model.calibrate_threshold(fit_slice)?
        } else {
            model.calibrate_threshold(&calibration_records)?
        };
        model.id = format!(
            "{method}:{}:f{}:thr{:.3}:cal{}:e{}",
            file_stem(path),
            model.weights.len(),
            model.threshold,
            model.options.calibration_percent,
            model.options.epochs
        );
        Ok(model)
    }

    fn train_safe_base(
        method: &str,
        locale: &str,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<SafeNgramMethod> {
        SafeNgramMethod::train(method, locale, path, config, records)
    }

    fn stacked_bayes_calibration_record(&self, record: &HyphenationRecord) -> bool {
        self.options.calibration_percent > 0
            && stable_unit_interval("stacked-bayes-calibration", &split_group_key(record))
                < self.options.calibration_percent as f64 / 100.0
    }

    fn stacked_bayes_fit_record(&self, record: &HyphenationRecord) -> bool {
        !self.stacked_bayes_calibration_record(record)
    }

    fn fit_weights(&mut self, records: &[HyphenationRecord]) -> Result<()> {
        let mut counts = U64HashMap::<BoundaryBayesCounts>::default();
        let mut total_positive = 0u32;
        let mut total_negative = 0u32;
        let mut features = SmallVec::<[u64; 64]>::new();
        let mut votes = StackedVotes::default();

        for record in records {
            if record.ambiguous
                || !record.word.is_ascii()
                || record.word.len() < self.config.min_word_len
            {
                continue;
            }
            let bytes = record.word.as_bytes();
            self.collect_votes(&record.word, &mut votes)?;
            for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min)
            {
                let positive = record.breaks.contains(&(boundary as GraphemeIndex));
                if positive {
                    total_positive = total_positive.saturating_add(1);
                } else {
                    total_negative = total_negative.saturating_add(1);
                }
                stacked_bayes_features(bytes, boundary, &votes, &mut features);
                for key in features.iter().copied() {
                    let slot = counts.entry(key).or_default();
                    if positive {
                        slot.positive = slot.positive.saturating_add(1);
                    } else {
                        slot.negative = slot.negative.saturating_add(1);
                    }
                }
            }
        }

        anyhow::ensure!(
            total_positive > 0 && total_negative > 0,
            "stacked-bayes needs positive and negative training boundaries"
        );
        let alpha = self.options.alpha;
        let positive_den = total_positive as f32 + 2.0 * alpha;
        let negative_den = total_negative as f32 + 2.0 * alpha;
        let mut weights = U64HashMap::<f32>::default();
        for (key, feature_counts) in counts {
            if feature_counts
                .positive
                .saturating_add(feature_counts.negative)
                < self.options.min_support
            {
                continue;
            }
            let p_feature_given_positive = (feature_counts.positive as f32 + alpha) / positive_den;
            let p_feature_given_negative = (feature_counts.negative as f32 + alpha) / negative_den;
            let weight = (p_feature_given_positive / p_feature_given_negative).ln();
            if weight.abs() >= 0.01 {
                weights.insert(key, weight);
            }
        }
        anyhow::ensure!(!weights.is_empty(), "stacked-bayes learned no features");
        self.weights = weights;
        self.bias = ((total_positive as f32 + alpha) / (total_negative as f32 + alpha)).ln();
        Ok(())
    }

    fn fit_logistic(&mut self, records: &[HyphenationRecord]) -> Result<()> {
        let mut total_positive = 0u32;
        let mut total_negative = 0u32;
        for record in records {
            if record.ambiguous
                || !record.word.is_ascii()
                || record.word.len() < self.config.min_word_len
            {
                continue;
            }
            let bytes = record.word.as_bytes();
            for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min)
            {
                if record.breaks.contains(&(boundary as GraphemeIndex)) {
                    total_positive = total_positive.saturating_add(1);
                } else {
                    total_negative = total_negative.saturating_add(1);
                }
            }
        }
        anyhow::ensure!(
            total_positive > 0 && total_negative > 0,
            "stacked-logit needs positive and negative training boundaries"
        );
        let alpha = self.options.alpha;
        self.bias = ((total_positive as f32 + alpha) / (total_negative as f32 + alpha)).ln();

        let mut votes = StackedVotes::default();
        let mut features = SmallVec::<[u64; 64]>::new();
        let mut updates = 0u64;
        for epoch in 0..self.options.epochs {
            let rate = self.options.learning_rate / (1.0 + epoch as f32 * 0.35);
            for record in records {
                if record.ambiguous
                    || !record.word.is_ascii()
                    || record.word.len() < self.config.min_word_len
                {
                    continue;
                }
                let bytes = record.word.as_bytes();
                self.collect_votes(&record.word, &mut votes)?;
                for boundary in
                    self.config.left_min..=bytes.len().saturating_sub(self.config.right_min)
                {
                    let positive = record.breaks.contains(&(boundary as GraphemeIndex));
                    stacked_bayes_features(bytes, boundary, &votes, &mut features);
                    let score = self.score_features(&features);
                    let probability = sigmoid(score);
                    let target = if positive { 1.0 } else { 0.0 };
                    let gradient = target - probability;
                    self.bias += rate * gradient * 0.1;
                    for key in features.iter().copied() {
                        let slot = self.weights.entry(key).or_insert(0.0);
                        *slot += rate * gradient;
                    }
                    updates += 1;
                }
            }
        }
        anyhow::ensure!(updates > 0, "stacked-logit produced no training updates");
        self.weights.retain(|_, weight| weight.abs() >= 0.001);
        Ok(())
    }

    fn calibrate_threshold(&self, records: &[HyphenationRecord]) -> Result<f32> {
        let mut scored = Vec::<(f32, bool)>::new();
        let mut total_positive = 0usize;
        let mut votes = StackedVotes::default();
        let mut features = SmallVec::<[u64; 64]>::new();

        for record in records {
            if record.ambiguous
                || !record.word.is_ascii()
                || record.word.len() < self.config.min_word_len
            {
                continue;
            }
            let bytes = record.word.as_bytes();
            self.collect_votes(&record.word, &mut votes)?;
            for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min)
            {
                let positive = record.breaks.contains(&(boundary as GraphemeIndex));
                if positive {
                    total_positive += 1;
                }
                stacked_bayes_features(bytes, boundary, &votes, &mut features);
                scored.push((self.score_features(&features), positive));
            }
        }
        anyhow::ensure!(
            total_positive > 0 && !scored.is_empty(),
            "stacked-bayes calibration split has no usable boundaries"
        );
        scored.sort_by(|left, right| {
            right
                .0
                .partial_cmp(&left.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut tp = 0usize;
        let mut fp = 0usize;
        let mut best_recall = -1.0f64;
        let mut best_threshold = scored.first().map(|(score, _)| *score).unwrap_or(0.0);
        for (score, positive) in scored {
            if positive {
                tp += 1;
            } else {
                fp += 1;
            }
            let predicted = tp + fp;
            let precision_ppm = tp as f64 * 1_000_000.0 / predicted as f64;
            if precision_ppm + f64::EPSILON >= self.options.target_precision_ppm as f64 {
                let recall = tp as f64 / total_positive as f64;
                if recall >= best_recall {
                    best_recall = recall;
                    best_threshold = score;
                }
            }
        }
        Ok(best_threshold)
    }

    fn collect_votes(&self, word: &str, votes: &mut StackedVotes) -> Result<()> {
        self.hypher.hyphenate_into(word, &mut votes.hypher)?;
        if let Some(liang) = &self.liang {
            liang.hyphenate_into(word, &mut votes.liang)?;
        } else {
            votes.liang.clear();
        }
        self.safe_p65.hyphenate_into(word, &mut votes.safe_p65)?;
        self.safe_mixson_p50
            .hyphenate_into(word, &mut votes.safe_mixson_p50)?;
        self.safe_p40_mixcv
            .hyphenate_into(word, &mut votes.safe_p40_mixcv)?;
        Ok(())
    }

    fn score_features(&self, features: &[u64]) -> f32 {
        let mut score = self.bias;
        for key in features {
            if let Some(weight) = self.weights.get(key) {
                score += *weight;
            }
        }
        score
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        let mut votes = StackedVotes::default();
        let mut features = SmallVec::<[u64; 64]>::new();
        self.collect_votes(word, &mut votes)?;
        let mut scored = Vec::<(f32, GraphemeIndex)>::new();
        for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min) {
            stacked_bayes_features(bytes, boundary, &votes, &mut features);
            let score = self.score_features(&features);
            if score >= self.threshold {
                scored.push((score, boundary as GraphemeIndex));
            }
        }
        if self.options.cap_vowel_nuclei {
            let cap = stacked_vowel_break_cap(bytes);
            if scored.len() > cap {
                scored.sort_by(|left, right| {
                    right
                        .0
                        .partial_cmp(&left.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| left.1.cmp(&right.1))
                });
                scored.truncate(cap);
            }
        }
        out.extend(scored.into_iter().map(|(_, boundary)| boundary));
        out.sort_unstable();
        Ok(())
    }
}

impl CandidateBayesMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_candidate_bayes_options(method)?;
        let methods =
            prepare_candidate_base_methods(locale, path, patterns, config.clone(), records)?;

        let mut fit_records = Vec::new();
        let mut calibration_records = Vec::new();
        if options.calibration_percent > 0 {
            for record in records {
                if stable_unit_interval("candidate-bayes-calibration", &split_group_key(record))
                    < options.calibration_percent as f64 / 100.0
                {
                    calibration_records.push(record.clone());
                } else {
                    fit_records.push(record.clone());
                }
            }
        }
        let use_calibration = !fit_records.is_empty() && !calibration_records.is_empty();
        let (fit_slice, calibration_slice, fit_methods) = if use_calibration {
            let fit_methods = prepare_candidate_base_methods(
                locale,
                path,
                patterns,
                config.clone(),
                &fit_records,
            )?;
            (
                fit_records.as_slice(),
                calibration_records.as_slice(),
                Some(fit_methods),
            )
        } else {
            (records, records, None)
        };
        let training_methods = fit_methods.as_deref().unwrap_or(methods.as_slice());
        let (weights, bias) =
            fit_candidate_bayes_weights(&config, &options, fit_slice, training_methods)?;
        let threshold = calibrate_candidate_bayes_threshold(
            &config,
            &options,
            &weights,
            bias,
            calibration_slice,
            training_methods,
        )?;

        Ok(Self {
            id: format!(
                "{method}:{}:f{}:thr{:.3}:cal{}",
                file_stem(path),
                weights.len(),
                threshold,
                if use_calibration {
                    options.calibration_percent
                } else {
                    0
                }
            ),
            config,
            threshold,
            weights,
            bias,
            methods,
        })
    }

    fn train_logit(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_candidate_bayes_options(method)?;
        let methods =
            prepare_candidate_base_methods(locale, path, patterns, config.clone(), records)?;

        let mut fit_records = Vec::new();
        let mut calibration_records = Vec::new();
        if options.calibration_percent > 0 {
            for record in records {
                if stable_unit_interval("candidate-logit-calibration", &split_group_key(record))
                    < options.calibration_percent as f64 / 100.0
                {
                    calibration_records.push(record.clone());
                } else {
                    fit_records.push(record.clone());
                }
            }
        }
        let use_calibration = !fit_records.is_empty() && !calibration_records.is_empty();
        let (fit_slice, calibration_slice, fit_methods) = if use_calibration {
            let fit_methods = prepare_candidate_base_methods(
                locale,
                path,
                patterns,
                config.clone(),
                &fit_records,
            )?;
            (
                fit_records.as_slice(),
                calibration_records.as_slice(),
                Some(fit_methods),
            )
        } else {
            (records, records, None)
        };
        let training_methods = fit_methods.as_deref().unwrap_or(methods.as_slice());
        let (weights, bias) =
            fit_candidate_logit_weights(&config, &options, fit_slice, training_methods)?;
        let threshold = calibrate_candidate_bayes_threshold(
            &config,
            &options,
            &weights,
            bias,
            calibration_slice,
            training_methods,
        )?;

        Ok(Self {
            id: format!(
                "{method}:{}:f{}:thr{:.3}:cal{}:e{}",
                file_stem(path),
                weights.len(),
                threshold,
                if use_calibration {
                    options.calibration_percent
                } else {
                    0
                },
                options.epochs
            ),
            config,
            threshold,
            weights,
            bias,
            methods,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.methods.len()];
        for (method, pred) in self.methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_into(word, pred)?;
        }
        let mut candidate = BTreeSet::<GraphemeIndex>::new();
        for pred in &predictions {
            candidate.extend(filtered_break_set(word, pred, &self.config));
        }
        let bytes = word.as_bytes();
        let mut features = SmallVec::<[u64; 96]>::new();
        for boundary in candidate {
            let mask = candidate_vote_mask(&predictions, boundary, word, &self.config);
            candidate_bayes_features(bytes, boundary as usize, mask, &mut features);
            if score_sparse_features(self.bias, &self.weights, &features) >= self.threshold {
                out.push(boundary);
            }
        }
        out.sort_unstable();
        Ok(())
    }
}

impl PruneBayesMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_prune_bayes_options(method)?;
        let base_method = format!(
            "liang-safe-add-multi-s1-p{}-veto-mixcv-multi-s1-n0",
            options.base_precision
        );
        let base = prepare_candidate_base_method(
            &base_method,
            locale,
            path,
            patterns,
            config.clone(),
            records,
        )?;
        let methods = if options.wide_sources {
            prepare_mask_rerank_base_methods(locale, path, patterns, config.clone(), records, true)?
        } else {
            prepare_candidate_base_methods(locale, path, patterns, config.clone(), records)?
        };

        let mut fit_records = Vec::new();
        let mut calibration_records = Vec::new();
        if options.calibration_percent > 0 {
            for record in records {
                if stable_unit_interval("prune-bayes-calibration", &split_group_key(record))
                    < options.calibration_percent as f64 / 100.0
                {
                    calibration_records.push(record.clone());
                } else {
                    fit_records.push(record.clone());
                }
            }
        }
        let use_calibration = !fit_records.is_empty() && !calibration_records.is_empty();
        let (fit_slice, calibration_slice, fit_base, fit_methods) = if use_calibration {
            let fit_base = prepare_candidate_base_method(
                &base_method,
                locale,
                path,
                patterns,
                config.clone(),
                &fit_records,
            )?;
            let fit_methods = if options.wide_sources {
                prepare_mask_rerank_base_methods(
                    locale,
                    path,
                    patterns,
                    config.clone(),
                    &fit_records,
                    true,
                )?
            } else {
                prepare_candidate_base_methods(
                    locale,
                    path,
                    patterns,
                    config.clone(),
                    &fit_records,
                )?
            };
            (
                fit_records.as_slice(),
                calibration_records.as_slice(),
                Some(fit_base),
                Some(fit_methods),
            )
        } else {
            (records, records, None, None)
        };
        let training_base = fit_base.as_ref().unwrap_or(&base);
        let training_methods = fit_methods.as_deref().unwrap_or(methods.as_slice());
        let (weights, bias) = fit_prune_bayes_weights(
            &config,
            &options,
            fit_slice,
            training_base,
            training_methods,
        )?;
        let threshold = calibrate_prune_bayes_threshold(
            &config,
            &options,
            &weights,
            bias,
            calibration_slice,
            training_base,
            training_methods,
        )?;

        Ok(Self {
            id: format!(
                "{method}:{}:f{}:thr{:.3}:cal{}:src{}",
                file_stem(path),
                weights.len(),
                threshold,
                if use_calibration {
                    options.calibration_percent
                } else {
                    0
                },
                methods.len()
            ),
            config,
            threshold,
            weights,
            bias,
            base: Box::new(base),
            methods,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        self.base.hyphenate_into(word, out)?;
        *out = filtered_break_vec(word, out, &self.config);
        if !word.is_ascii() || word.len() < self.config.min_word_len || out.is_empty() {
            return Ok(());
        }
        let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.methods.len()];
        for (method, pred) in self.methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_into(word, pred)?;
        }
        let bytes = word.as_bytes();
        let mut features = SmallVec::<[u64; 96]>::new();
        out.retain(|boundary| {
            let mask = candidate_vote_mask(&predictions, *boundary, word, &self.config);
            candidate_bayes_features(bytes, *boundary as usize, mask, &mut features);
            score_sparse_features(self.bias, &self.weights, &features) >= self.threshold
        });
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

impl CandidateGateMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
        calibration_override: Option<&[HyphenationRecord]>,
    ) -> Result<Self> {
        let options = parse_candidate_gate_options(method)?;
        let methods =
            prepare_candidate_base_methods(locale, path, patterns, config.clone(), records)?;
        let mut fit_records = Vec::new();
        let mut calibration_records = Vec::new();
        if options.calibration_percent > 0 {
            for record in records {
                if stable_unit_interval("candidate-gate-calibration", &split_group_key(record))
                    < options.calibration_percent as f64 / 100.0
                {
                    calibration_records.push(record.clone());
                } else {
                    fit_records.push(record.clone());
                }
            }
        }
        let use_split_calibration = !fit_records.is_empty() && !calibration_records.is_empty();
        let selected = if let Some(calibration_records) = calibration_override {
            learn_candidate_gate(calibration_records, &config, &options, &methods)?
        } else if use_split_calibration {
            let fit_methods = prepare_candidate_base_methods(
                locale,
                path,
                patterns,
                config.clone(),
                &fit_records,
            )?;
            learn_candidate_gate(&calibration_records, &config, &options, &fit_methods)?
        } else {
            learn_candidate_gate(records, &config, &options, &methods)?
        };
        anyhow::ensure!(
            !selected.is_empty(),
            "candidate-gate selected no groups from {}",
            path.display()
        );
        Ok(Self {
            id: format!(
                "{method}:{}:{:?}:g{}:cal{}",
                file_stem(path),
                options.kind,
                selected.len(),
                if calibration_override.is_some() {
                    100
                } else if use_split_calibration {
                    options.calibration_percent
                } else {
                    0
                }
            ),
            config,
            options,
            methods,
            selected,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.methods.len()];
        for (method, pred) in self.methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_into(word, pred)?;
        }
        let mut candidate = BTreeSet::<GraphemeIndex>::new();
        for pred in &predictions {
            candidate.extend(filtered_break_set(word, pred, &self.config));
        }
        let bytes = word.as_bytes();
        for boundary in candidate {
            let mask = candidate_vote_mask(&predictions, boundary, word, &self.config);
            let key = candidate_gate_key(self.options.kind, bytes, boundary as usize, mask);
            if self.selected.contains(&key) {
                out.push(boundary);
            }
        }
        out.sort_unstable();
        Ok(())
    }
}

impl MaskRerankMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_mask_rerank_options(method)?;
        let methods = prepare_mask_rerank_base_methods(
            locale,
            path,
            patterns,
            config.clone(),
            records,
            options.wide_sources,
        )?;
        let mut model = Self {
            id: String::new(),
            config,
            options,
            weights: U64HashMap::default(),
            methods,
        };
        model.fit(records)?;
        model.id = format!(
            "{method}:{}:f{}:src{}:e{}",
            file_stem(path),
            model.weights.len(),
            model.methods.len(),
            model.options.epochs
        );
        Ok(model)
    }

    fn fit(&mut self, records: &[HyphenationRecord]) -> Result<()> {
        let mut examples = Vec::<MaskTrainingExample>::new();
        for record in records {
            if record.ambiguous
                || !record.word.is_ascii()
                || record.word.len() < self.config.min_word_len
            {
                continue;
            }
            let candidates = self.generate_record_candidates(record)?;
            if candidates.is_empty() {
                continue;
            }
            let gold = filtered_break_vec(&record.word, &record.breaks, &self.config);
            if let Some(target_idx) = best_oracle_mask_candidate(
                &candidates,
                &gold,
                self.options.fp_weight,
                self.options.fn_weight,
            ) {
                let costs = candidates
                    .iter()
                    .map(|candidate| {
                        mask_candidate_cost(
                            &candidate.breaks,
                            &gold,
                            self.options.fp_weight,
                            self.options.fn_weight,
                        )
                    })
                    .collect();
                examples.push(MaskTrainingExample {
                    target_idx,
                    costs,
                    candidates,
                });
            }
        }
        anyhow::ensure!(
            !examples.is_empty(),
            "mask-rerank produced no usable training examples"
        );

        self.weights = fit_mask_feature_reward_weights(&examples);
        let mut updates = 0u64;
        for epoch in 0..self.options.epochs {
            let rate = self.options.learning_rate / (1.0 + epoch as f32 * 0.25);
            for example in &examples {
                let pred_idx = best_cost_augmented_mask_candidate(
                    &example.candidates,
                    &example.costs,
                    &self.weights,
                );
                if pred_idx != example.target_idx {
                    add_sparse_features(
                        &mut self.weights,
                        &example.candidates[example.target_idx].features,
                        rate,
                    );
                    add_sparse_features(
                        &mut self.weights,
                        &example.candidates[pred_idx].features,
                        -rate,
                    );
                    updates += 1;
                }
            }
        }
        anyhow::ensure!(updates > 0, "mask-rerank produced no training updates");
        self.weights.retain(|_, weight| weight.abs() >= 0.001);
        Ok(())
    }

    fn generate_record_candidates(&self, record: &HyphenationRecord) -> Result<Vec<MaskCandidate>> {
        let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.methods.len()];
        for (method, pred) in self.methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(record, pred)?;
        }
        Ok(generate_mask_candidates(
            &record.word,
            &predictions,
            &self.config,
            &self.options,
        ))
    }

    fn generate_word_candidates(&self, word: &str) -> Result<Vec<MaskCandidate>> {
        let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.methods.len()];
        for (method, pred) in self.methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_into(word, pred)?;
        }
        Ok(generate_mask_candidates(
            word,
            &predictions,
            &self.config,
            &self.options,
        ))
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let candidates = self.generate_word_candidates(word)?;
        if candidates.is_empty() {
            return Ok(());
        }
        let pred_idx = best_scored_mask_candidate(&candidates, &self.weights);
        out.extend(candidates[pred_idx].breaks.iter().copied());
        Ok(())
    }
}

impl MaskOracleMethod {
    fn new(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_mask_rerank_options(method)?;
        let methods = prepare_mask_rerank_base_methods(
            locale,
            path,
            patterns,
            config.clone(),
            records,
            options.wide_sources,
        )?;
        Ok(Self {
            id: format!("{method}:{}:src{}:oracle", file_stem(path), methods.len()),
            config,
            options,
            methods,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_record_into(
        &self,
        record: &HyphenationRecord,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) -> Result<()> {
        out.clear();
        if !record.word.is_ascii() || record.word.len() < self.config.min_word_len {
            return Ok(());
        }
        let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.methods.len()];
        for (method, pred) in self.methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(record, pred)?;
        }
        let candidates =
            generate_mask_candidates(&record.word, &predictions, &self.config, &self.options);
        if candidates.is_empty() {
            return Ok(());
        }
        let gold = filtered_break_vec(&record.word, &record.breaks, &self.config);
        if let Some(best_idx) = best_oracle_mask_candidate(
            &candidates,
            &gold,
            self.options.fp_weight,
            self.options.fn_weight,
        ) {
            out.extend(candidates[best_idx].breaks.iter().copied());
        }
        Ok(())
    }
}

impl MaskCostMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_mask_rerank_options(method)?;
        let methods = prepare_mask_rerank_base_methods(
            locale,
            path,
            patterns,
            config.clone(),
            records,
            options.wide_sources,
        )?;
        let mut group_costs = U64HashMap::<FloatFeatureStats>::default();
        let mut global_total = 0.0f32;
        let mut global_count = 0u32;
        let mut usable_records = 0usize;

        let scratch = Self {
            id: String::new(),
            config: config.clone(),
            options: options.clone(),
            group_costs: U64HashMap::default(),
            global_cost: 0.0,
            methods,
        };
        for record in records {
            if record.ambiguous
                || !record.word.is_ascii()
                || record.word.len() < config.min_word_len
            {
                continue;
            }
            let candidates = scratch.generate_record_candidates(record)?;
            if candidates.is_empty() {
                continue;
            }
            usable_records += 1;
            let gold = filtered_break_vec(&record.word, &record.breaks, &config);
            for candidate in candidates {
                let cost = mask_candidate_cost(
                    &candidate.breaks,
                    &gold,
                    options.fp_weight,
                    options.fn_weight,
                );
                global_total += cost;
                global_count = global_count.saturating_add(1);
                for group in candidate.groups {
                    let slot = group_costs.entry(group).or_default();
                    slot.count = slot.count.saturating_add(1);
                    slot.total += cost;
                }
            }
        }
        anyhow::ensure!(
            global_count > 0,
            "mask-cost produced no usable training candidates"
        );
        let global_cost = global_total / global_count as f32;
        Ok(Self {
            id: format!(
                "{method}:{}:g{}:src{}:n{}",
                file_stem(path),
                group_costs.len(),
                scratch.methods.len(),
                usable_records
            ),
            config,
            options,
            group_costs,
            global_cost,
            methods: scratch.methods,
        })
    }

    fn generate_record_candidates(&self, record: &HyphenationRecord) -> Result<Vec<MaskCandidate>> {
        let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.methods.len()];
        for (method, pred) in self.methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(record, pred)?;
        }
        Ok(generate_mask_candidates(
            &record.word,
            &predictions,
            &self.config,
            &self.options,
        ))
    }

    fn generate_word_candidates(&self, word: &str) -> Result<Vec<MaskCandidate>> {
        let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.methods.len()];
        for (method, pred) in self.methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_into(word, pred)?;
        }
        Ok(generate_mask_candidates(
            word,
            &predictions,
            &self.config,
            &self.options,
        ))
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let candidates = self.generate_word_candidates(word)?;
        if candidates.is_empty() {
            return Ok(());
        }
        let best_idx =
            best_cost_table_mask_candidate(&candidates, &self.group_costs, self.global_cost);
        out.extend(candidates[best_idx].breaks.iter().copied());
        Ok(())
    }
}

impl RankedUnionMethod {
    fn new(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let lower = method.to_ascii_lowercase();
        let mut min_score = 1;
        let mut cap_vowel_nuclei = lower.contains("cap");
        for part in lower.split('-') {
            if matches!(part, "nocap" | "uncapped") {
                cap_vowel_nuclei = false;
            }
            if let Some(value) = part.strip_prefix('t') {
                if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                    min_score = value.parse::<i32>().with_context(|| {
                        format!("parse ranked-union score threshold from {part:?}")
                    })?;
                }
            }
        }
        anyhow::ensure!(
            min_score > 0,
            "ranked-union score threshold must be positive"
        );

        let candidate_method = if lower.contains("p65") {
            "liang-safe-add-multi-s1-p65-veto-mixcv-multi-s1-n0"
        } else {
            "liang-safe-add-multi-s1-p80-veto-mixcv-multi-s1-n0"
        };
        let candidate = prepare_candidate_base_method(
            candidate_method,
            locale,
            path,
            patterns,
            config.clone(),
            records,
        )?;
        let anchors = vec![
            (
                prepare_candidate_base_method(
                    "hypher",
                    locale,
                    path,
                    patterns,
                    config.clone(),
                    records,
                )?,
                4,
            ),
            (
                prepare_candidate_base_method(
                    "safe-ngram-multi-s1-p65-veto-multi-s1-n0",
                    locale,
                    path,
                    patterns,
                    config.clone(),
                    records,
                )?,
                5,
            ),
            (
                prepare_candidate_base_method(
                    "safe-ngram-mixson-multi-s1-p90-veto-multi-s1-n0",
                    locale,
                    path,
                    patterns,
                    config.clone(),
                    records,
                )?,
                3,
            ),
            (
                prepare_candidate_base_method(
                    "safe-ngram-multi-s1-p50-veto-multi-s1-n0",
                    locale,
                    path,
                    patterns,
                    config.clone(),
                    records,
                )?,
                2,
            ),
        ];
        Ok(Self {
            id: format!(
                "{method}:{}:{}:anchors{}:t{}:{}",
                file_stem(path),
                candidate.id(),
                anchors.len(),
                min_score,
                if cap_vowel_nuclei { "cap" } else { "nocap" }
            ),
            config,
            candidate: Box::new(candidate),
            anchors,
            min_score,
            cap_vowel_nuclei,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let mut candidate = SmallVec::<[GraphemeIndex; 8]>::new();
        self.candidate.hyphenate_into(word, &mut candidate)?;
        let candidate_set = filtered_break_set(word, &candidate, &self.config);
        if candidate_set.is_empty() {
            return Ok(());
        }

        let mut anchor_predictions =
            vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.anchors.len()];
        for ((method, _), pred) in self.anchors.iter().zip(anchor_predictions.iter_mut()) {
            method.hyphenate_into(word, pred)?;
        }

        let mut scored = Vec::<(i32, GraphemeIndex)>::new();
        for boundary in candidate_set {
            let mut score = 1;
            for ((_, weight), pred) in self.anchors.iter().zip(anchor_predictions.iter()) {
                if filtered_break_set(word, pred, &self.config).contains(&boundary) {
                    score += *weight;
                }
            }
            if score >= self.min_score {
                scored.push((score, boundary));
            }
        }
        if self.cap_vowel_nuclei {
            let cap = stacked_vowel_break_cap(word.as_bytes());
            if scored.len() > cap {
                scored
                    .sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
                scored.truncate(cap);
            }
        }
        out.extend(scored.into_iter().map(|(_, boundary)| boundary));
        out.sort_unstable();
        Ok(())
    }
}

struct HypherSafeAddMethod {
    id: String,
    base: Box<dyn MethodAdapter>,
    config: HyphenationConfig,
    add_options: SafeNgramOptions,
    add_rules: U64HashSet,
    veto_options: Option<SafeNgramOptions>,
    veto_rules: U64HashSet,
}

struct BaseSafeAddMethod {
    id: String,
    base: Box<PreparedMethod>,
    config: HyphenationConfig,
    add_options: SafeNgramOptions,
    add_rules: U64HashSet,
    veto_options: Option<SafeNgramOptions>,
    veto_rules: U64HashSet,
}

struct SafeLadderMethod {
    id: String,
    base: SafeNgramMethod,
    config: HyphenationConfig,
    add_options: SafeNgramOptions,
    add_rules: U64HashSet,
    veto_options: Option<SafeNgramOptions>,
    veto_rules: U64HashSet,
}

struct BaseVetoMethod {
    id: String,
    base: Box<PreparedMethod>,
    config: HyphenationConfig,
    options: SafeNgramOptions,
    veto_rules: U64HashSet,
}

#[derive(Debug, Clone, Copy)]
struct PronCountCapOptions {
    base_precision: u32,
    veto_precision: Option<u32>,
    fill: bool,
    wide_sources: bool,
    orthographic_fallback: bool,
    fallback_slack: usize,
}

struct PronCountCapMethod {
    id: String,
    base: Box<PreparedMethod>,
    config: HyphenationConfig,
    options: PronCountCapOptions,
    syllable_counts: HashMap<String, u8>,
    scorer_methods: Vec<PreparedMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AffixSafeAddOptions {
    min_support: u32,
    min_precision_ppm: u32,
    max_negative: u32,
    min_suffix_len: usize,
    max_suffix_len: usize,
    min_prefix_len: usize,
    max_prefix_len: usize,
}

struct AffixSafeAddMethod {
    id: String,
    base: Box<PreparedMethod>,
    config: HyphenationConfig,
    options: AffixSafeAddOptions,
    suffix_rules: U64HashSet,
    prefix_rules: U64HashSet,
}

struct AffixVetoMethod {
    id: String,
    base: Box<PreparedMethod>,
    config: HyphenationConfig,
    options: AffixSafeAddOptions,
    right_rules: U64HashSet,
    left_rules: U64HashSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AnalogSafeAddOptions {
    min_key_len: usize,
    max_key_len: usize,
    max_candidates: usize,
    min_vote: i32,
    min_sources: u32,
    whole_mask: bool,
}

#[derive(Debug, Clone)]
struct AnalogEntry {
    word: String,
    breaks: SmallVec<[GraphemeIndex; 8]>,
}

struct AnalogSafeAddMethod {
    id: String,
    base: Box<PreparedMethod>,
    config: HyphenationConfig,
    options: AnalogSafeAddOptions,
    entries: Vec<AnalogEntry>,
    prefix_index: HashMap<String, Vec<usize>>,
    suffix_index: HashMap<String, Vec<usize>>,
}

impl HypherSafeAddMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let base = adapter_for_method("hypher", locale)?;
        let (add_options, veto_options) = parse_hypher_safe_options(method)?;
        let (add_rules, trained_records) = learn_safe_ngram_rules(records, &config, &add_options);
        anyhow::ensure!(
            !add_rules.is_empty(),
            "hypher-safe-add learned no add rules from {} with method {method:?}",
            path.display()
        );
        let veto_rules = if let Some(options) = &veto_options {
            learn_hypher_veto_rules(records, &config, options, base.as_ref())?
        } else {
            U64HashSet::default()
        };
        let id = format!(
            "{method}:{}:add{}:veto{}:n{}",
            file_stem(path),
            add_rules.len(),
            veto_rules.len(),
            trained_records
        );
        Ok(Self {
            id,
            base,
            config,
            add_options,
            add_rules,
            veto_options,
            veto_rules,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        self.base.hyphenate_into(word, out)?;
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        if let Some(veto_options) = &self.veto_options {
            out.retain(|boundary| {
                !veto_options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        self.veto_rules.contains(&safe_ngram_key(
                            bytes,
                            *boundary as usize,
                            spec_idx,
                            *spec,
                        ))
                    })
            });
        }
        for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min) {
            let boundary = boundary as GraphemeIndex;
            if out.contains(&boundary) {
                continue;
            }
            if self
                .add_options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    self.add_rules.contains(&safe_ngram_key(
                        bytes,
                        boundary as usize,
                        spec_idx,
                        *spec,
                    ))
                })
            {
                out.push(boundary);
            }
        }
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

impl BaseSafeAddMethod {
    fn train(
        method: &str,
        base_label: &str,
        base: PreparedMethod,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let (add_options, veto_options) = parse_safe_ngram_veto_options(method)?;
        let (add_rules, trained_records) = learn_safe_ngram_rules(records, &config, &add_options);
        anyhow::ensure!(
            !add_rules.is_empty(),
            "{method} learned no add rules from {}",
            path.display()
        );
        let veto_rules = if let Some(options) = &veto_options {
            learn_base_safe_add_veto_rules(
                records,
                &config,
                &add_options,
                &add_rules,
                options,
                &base,
            )?
        } else {
            U64HashSet::default()
        };
        let id = format!(
            "{method}:{}:{}:add{}:veto{}:n{}",
            base_label,
            file_stem(path),
            add_rules.len(),
            veto_rules.len(),
            trained_records
        );
        Ok(Self {
            id,
            base: Box::new(base),
            config,
            add_options,
            add_rules,
            veto_options,
            veto_rules,
        })
    }

    fn train_residual(
        method: &str,
        base_label: &str,
        base: PreparedMethod,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let (add_options, veto_options) = parse_safe_ngram_veto_options(method)?;
        let (add_rules, trained_records) =
            learn_residual_safe_ngram_rules(records, &config, &add_options, &base)?;
        anyhow::ensure!(
            !add_rules.is_empty(),
            "{method} learned no residual add rules from {}",
            path.display()
        );
        let veto_rules = if let Some(options) = &veto_options {
            learn_base_safe_add_veto_rules(
                records,
                &config,
                &add_options,
                &add_rules,
                options,
                &base,
            )?
        } else {
            U64HashSet::default()
        };
        let id = format!(
            "{method}:{}:{}:resadd{}:veto{}:n{}",
            base_label,
            file_stem(path),
            add_rules.len(),
            veto_rules.len(),
            trained_records
        );
        Ok(Self {
            id,
            base: Box::new(base),
            config,
            add_options,
            add_rules,
            veto_options,
            veto_rules,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        self.base.hyphenate_into(word, out)?;
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min) {
            let boundary = boundary as GraphemeIndex;
            if out.contains(&boundary) {
                continue;
            }
            if self
                .add_options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    self.add_rules.contains(&safe_ngram_key(
                        bytes,
                        boundary as usize,
                        spec_idx,
                        *spec,
                    ))
                })
            {
                out.push(boundary);
            }
        }
        if let Some(veto_options) = &self.veto_options {
            out.retain(|boundary| {
                !veto_options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        self.veto_rules.contains(&safe_ngram_key(
                            bytes,
                            *boundary as usize,
                            spec_idx,
                            *spec,
                        ))
                    })
            });
        }
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

impl SafeLadderMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let (base_method, add_options, veto_options) = parse_safe_ladder_options(method)?;
        let base = SafeNgramMethod::train(&base_method, locale, path, config.clone(), records)?;
        let (add_rules, trained_records) =
            learn_safe_ladder_residual_rules(records, &config, &base, &add_options)?;
        anyhow::ensure!(
            !add_rules.is_empty(),
            "{method} learned no residual add rules from {}",
            path.display()
        );
        let veto_rules = if let Some(options) = &veto_options {
            learn_safe_ladder_veto_rules(
                records,
                &config,
                &base,
                &add_options,
                &add_rules,
                options,
            )?
        } else {
            U64HashSet::default()
        };
        Ok(Self {
            id: format!(
                "{method}:{}:{}:resadd{}:veto{}:n{}",
                base.id(),
                file_stem(path),
                add_rules.len(),
                veto_rules.len(),
                trained_records
            ),
            base,
            config,
            add_options,
            add_rules,
            veto_options,
            veto_rules,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        self.base.hyphenate_into(word, out)?;
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min) {
            let boundary_idx = boundary as GraphemeIndex;
            if out.contains(&boundary_idx) {
                continue;
            }
            let add_hit = self
                .add_options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    self.add_rules
                        .contains(&safe_ngram_key(bytes, boundary, spec_idx, *spec))
                });
            if !add_hit {
                continue;
            }
            let veto_hit = self.veto_options.as_ref().is_some_and(|veto_options| {
                veto_options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        self.veto_rules
                            .contains(&safe_ngram_key(bytes, boundary, spec_idx, *spec))
                    })
            });
            if !veto_hit {
                out.push(boundary_idx);
            }
        }
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

impl BaseVetoMethod {
    fn train(
        method: &str,
        base_label: &str,
        base: PreparedMethod,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_safe_ngram_options(method)?;
        let mut counts = U64HashMap::<SafeNgramCounts>::default();
        let mut pred = SmallVec::<[GraphemeIndex; 8]>::new();
        let mut trained_records = 0usize;

        for record in records {
            if record.ambiguous
                || !record.word.is_ascii()
                || record.word.len() < config.min_word_len
            {
                continue;
            }
            let bytes = record.word.as_bytes();
            base.hyphenate_record_into(record, &mut pred)?;
            pred.retain(|boundary| {
                let boundary = *boundary as usize;
                boundary >= config.left_min
                    && bytes.len().saturating_sub(boundary) >= config.right_min
            });
            pred.sort_unstable();
            pred.dedup();
            if pred.is_empty() {
                continue;
            }
            trained_records += 1;

            for boundary in pred.iter().copied() {
                let boundary_usize = boundary as usize;
                let positive = record.breaks.contains(&boundary);
                for (spec_idx, spec) in options.specs.iter().enumerate() {
                    let key = safe_ngram_key(bytes, boundary_usize, spec_idx, *spec);
                    let slot = counts.entry(key).or_default();
                    if positive {
                        slot.positive = slot.positive.saturating_add(1);
                    } else {
                        slot.negative = slot.negative.saturating_add(1);
                    }
                }
            }
        }

        let veto_rules = counts
            .into_iter()
            .filter_map(|(key, counts)| {
                safe_ngram_veto_counts_selected(counts, &options).then_some(key)
            })
            .collect::<U64HashSet>();
        anyhow::ensure!(
            !veto_rules.is_empty(),
            "{method} learned no veto rules from {}",
            path.display()
        );
        Ok(Self {
            id: format!(
                "{method}:{}:{}:veto{}:n{}",
                base_label,
                file_stem(path),
                veto_rules.len(),
                trained_records
            ),
            base: Box::new(base),
            config,
            options,
            veto_rules,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        self.base.hyphenate_into(word, out)?;
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        out.retain(|boundary| {
            let boundary = *boundary as usize;
            !self
                .options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    self.veto_rules
                        .contains(&safe_ngram_key(bytes, boundary, spec_idx, *spec))
                })
        });
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

impl PronCountCapMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        patterns: Option<&PathBuf>,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_pron_count_cap_options(method)?;
        let veto_part = options
            .veto_precision
            .map_or_else(|| "n0".to_string(), |precision| format!("p{precision}"));
        let base_method = format!(
            "liang-safe-add-multi-s1-p{}-veto-mixcv-multi-s1-{veto_part}",
            options.base_precision
        );
        let base = prepare_candidate_base_method(
            &base_method,
            locale,
            path,
            patterns,
            config.clone(),
            records,
        )?;
        let scorer_methods = if options.wide_sources {
            prepare_mask_rerank_base_methods(locale, path, patterns, config.clone(), records, true)?
        } else {
            prepare_candidate_base_methods(locale, path, patterns, config.clone(), records)?
        };
        let cmu_path = Path::new("data/raw/cmudict/cmudict.dict");
        let syllable_counts = load_cmudict_syllable_counts(cmu_path)?;
        anyhow::ensure!(
            !syllable_counts.is_empty(),
            "pron-count-cap loaded no syllable counts from {}",
            cmu_path.display()
        );
        Ok(Self {
            id: format!(
                "{method}:{}:{}:cmu{}:src{}",
                file_stem(
                    patterns.context("--patterns is required for --method pron-count-cap-*")?
                ),
                file_stem(path),
                syllable_counts.len(),
                scorer_methods.len()
            ),
            base: Box::new(base),
            config,
            options,
            syllable_counts,
            scorer_methods,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        self.base.hyphenate_into(word, out)?;
        out.sort_unstable();
        out.dedup();
        if !word.is_ascii()
            || !word.chars().all(|ch| ch.is_ascii_alphabetic())
            || word.len() < self.config.min_word_len
        {
            return Ok(());
        }
        let lower = word.to_ascii_lowercase();
        let cmu_target = self
            .syllable_counts
            .get(&lower)
            .copied()
            .map(|syllables| syllables.saturating_sub(1) as usize);
        let target = if let Some(target) = cmu_target {
            target
        } else if self.options.orthographic_fallback {
            orthographic_break_estimate(word.as_bytes()).saturating_add(self.options.fallback_slack)
        } else {
            return Ok(());
        };
        if out.len() == target {
            return Ok(());
        }

        let mut predictions =
            vec![SmallVec::<[GraphemeIndex; 8]>::new(); self.scorer_methods.len()];
        for (method, pred) in self.scorer_methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_into(word, pred)?;
            *pred = filtered_break_vec(word, pred, &self.config);
        }
        let mut scored = out
            .iter()
            .copied()
            .map(|boundary| (pron_count_boundary_score(&predictions, boundary), boundary))
            .collect::<Vec<_>>();
        if scored.len() > target {
            scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
            scored.truncate(target);
            out.clear();
            out.extend(scored.into_iter().map(|(_, boundary)| boundary));
            out.sort_unstable();
        }
        if self.options.fill && cmu_target.is_some() && out.len() < target {
            let mut candidate = BTreeSet::<GraphemeIndex>::new();
            for pred in &predictions {
                candidate.extend(pred.iter().copied());
            }
            let mut additions = candidate
                .into_iter()
                .filter(|boundary| !out.contains(boundary))
                .map(|boundary| (pron_count_boundary_score(&predictions, boundary), boundary))
                .collect::<Vec<_>>();
            additions
                .sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
            for (_, boundary) in additions {
                if out.len() >= target {
                    break;
                }
                out.push(boundary);
            }
            out.sort_unstable();
            out.dedup();
        }
        Ok(())
    }
}

impl AffixSafeAddMethod {
    fn train(
        method: &str,
        base_label: &str,
        base: PreparedMethod,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_affix_safe_add_options(method)?;
        let mut suffix_counts = U64HashMap::<SafeNgramCounts>::default();
        let mut prefix_counts = U64HashMap::<SafeNgramCounts>::default();
        let mut pred = SmallVec::<[GraphemeIndex; 8]>::new();
        let mut trained_records = 0usize;

        for record in records {
            if record.ambiguous || !record.word.is_ascii() {
                continue;
            }
            let bytes = record.word.as_bytes();
            if bytes.len() < config.min_word_len {
                continue;
            }
            trained_records += 1;
            base.hyphenate_record_into(record, &mut pred)?;
            pred.retain(|boundary| {
                let boundary = *boundary as usize;
                boundary >= config.left_min
                    && bytes.len().saturating_sub(boundary) >= config.right_min
            });
            pred.sort_unstable();
            pred.dedup();

            for suffix_len in options.min_suffix_len..=options.max_suffix_len.min(bytes.len()) {
                let boundary = bytes.len() - suffix_len;
                if boundary < config.left_min
                    || bytes.len().saturating_sub(boundary) < config.right_min
                {
                    continue;
                }
                let boundary = boundary as GraphemeIndex;
                if pred.contains(&boundary) {
                    continue;
                }
                let positive = record.breaks.contains(&boundary);
                let key = affix_suffix_key(bytes, boundary as usize, suffix_len);
                let slot = suffix_counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }

            for prefix_len in options.min_prefix_len..=options.max_prefix_len.min(bytes.len()) {
                let boundary = prefix_len;
                if boundary < config.left_min
                    || bytes.len().saturating_sub(boundary) < config.right_min
                {
                    continue;
                }
                let boundary = boundary as GraphemeIndex;
                if pred.contains(&boundary) {
                    continue;
                }
                let positive = record.breaks.contains(&boundary);
                let key = affix_prefix_key(bytes, prefix_len);
                let slot = prefix_counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }

        let suffix_rules = select_affix_rules(suffix_counts, &options);
        let prefix_rules = select_affix_rules(prefix_counts, &options);
        anyhow::ensure!(
            !suffix_rules.is_empty() || !prefix_rules.is_empty(),
            "{method} learned no affix add rules from {}",
            path.display()
        );
        Ok(Self {
            id: format!(
                "{method}:{}:{}:suffix{}:prefix{}:n{}",
                base_label,
                file_stem(path),
                suffix_rules.len(),
                prefix_rules.len(),
                trained_records
            ),
            base: Box::new(base),
            config,
            options,
            suffix_rules,
            prefix_rules,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        self.base.hyphenate_into(word, out)?;
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        for suffix_len in self.options.min_suffix_len..=self.options.max_suffix_len.min(bytes.len())
        {
            let boundary = bytes.len() - suffix_len;
            if boundary < self.config.left_min
                || bytes.len().saturating_sub(boundary) < self.config.right_min
            {
                continue;
            }
            let boundary_idx = boundary as GraphemeIndex;
            if out.contains(&boundary_idx) {
                continue;
            }
            if self
                .suffix_rules
                .contains(&affix_suffix_key(bytes, boundary, suffix_len))
            {
                out.push(boundary_idx);
            }
        }
        for prefix_len in self.options.min_prefix_len..=self.options.max_prefix_len.min(bytes.len())
        {
            let boundary = prefix_len;
            if boundary < self.config.left_min
                || bytes.len().saturating_sub(boundary) < self.config.right_min
            {
                continue;
            }
            let boundary_idx = boundary as GraphemeIndex;
            if out.contains(&boundary_idx) {
                continue;
            }
            if self
                .prefix_rules
                .contains(&affix_prefix_key(bytes, prefix_len))
            {
                out.push(boundary_idx);
            }
        }
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

impl AffixVetoMethod {
    fn train(
        method: &str,
        base_label: &str,
        base: PreparedMethod,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_affix_safe_add_options(method)?;
        let mut right_counts = U64HashMap::<SafeNgramCounts>::default();
        let mut left_counts = U64HashMap::<SafeNgramCounts>::default();
        let mut pred = SmallVec::<[GraphemeIndex; 8]>::new();
        let mut trained_records = 0usize;

        for record in records {
            if record.ambiguous || !record.word.is_ascii() {
                continue;
            }
            let bytes = record.word.as_bytes();
            if bytes.len() < config.min_word_len {
                continue;
            }
            trained_records += 1;
            base.hyphenate_record_into(record, &mut pred)?;
            pred.retain(|boundary| {
                let boundary = *boundary as usize;
                boundary >= config.left_min
                    && bytes.len().saturating_sub(boundary) >= config.right_min
            });
            pred.sort_unstable();
            pred.dedup();

            for boundary in pred.iter().copied() {
                let boundary = boundary as usize;
                let should_veto = !record.breaks.contains(&(boundary as GraphemeIndex));
                for right_len in options.min_suffix_len
                    ..=options
                        .max_suffix_len
                        .min(bytes.len().saturating_sub(boundary))
                {
                    let key = affix_right_context_key(bytes, boundary, right_len);
                    let slot = right_counts.entry(key).or_default();
                    if should_veto {
                        slot.positive = slot.positive.saturating_add(1);
                    } else {
                        slot.negative = slot.negative.saturating_add(1);
                    }
                }
                for left_len in options.min_prefix_len..=options.max_prefix_len.min(boundary) {
                    let key = affix_left_context_key(bytes, boundary, left_len);
                    let slot = left_counts.entry(key).or_default();
                    if should_veto {
                        slot.positive = slot.positive.saturating_add(1);
                    } else {
                        slot.negative = slot.negative.saturating_add(1);
                    }
                }
            }
        }

        let right_rules = select_affix_rules(right_counts, &options);
        let left_rules = select_affix_rules(left_counts, &options);
        anyhow::ensure!(
            !right_rules.is_empty() || !left_rules.is_empty(),
            "{method} learned no affix veto rules from {}",
            path.display()
        );
        Ok(Self {
            id: format!(
                "{method}:{}:{}:right{}:left{}:n{}",
                base_label,
                file_stem(path),
                right_rules.len(),
                left_rules.len(),
                trained_records
            ),
            base: Box::new(base),
            config,
            options,
            right_rules,
            left_rules,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        self.base.hyphenate_into(word, out)?;
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        out.retain(|boundary| {
            let boundary = *boundary as usize;
            let right_hit = (self.options.min_suffix_len
                ..=self
                    .options
                    .max_suffix_len
                    .min(bytes.len().saturating_sub(boundary)))
                .any(|right_len| {
                    self.right_rules
                        .contains(&affix_right_context_key(bytes, boundary, right_len))
                });
            let left_hit = (self.options.min_prefix_len
                ..=self.options.max_prefix_len.min(boundary))
                .any(|left_len| {
                    self.left_rules
                        .contains(&affix_left_context_key(bytes, boundary, left_len))
                });
            !(right_hit || left_hit)
        });
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

impl AnalogSafeAddMethod {
    fn train(
        method: &str,
        base_label: &str,
        base: PreparedMethod,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let options = parse_analog_safe_add_options(method)?;
        let mut entries = Vec::<AnalogEntry>::new();
        let mut prefix_index = HashMap::<String, Vec<usize>>::new();
        let mut suffix_index = HashMap::<String, Vec<usize>>::new();
        for record in records {
            if record.ambiguous
                || !record.word.is_ascii()
                || !record.word.chars().all(|ch| ch.is_ascii_alphabetic())
            {
                continue;
            }
            let word = record.word.to_ascii_lowercase();
            if word.len() < config.min_word_len {
                continue;
            }
            let idx = entries.len();
            for key_len in options.min_key_len..=options.max_key_len.min(word.len()) {
                prefix_index
                    .entry(word[..key_len].to_string())
                    .or_default()
                    .push(idx);
                suffix_index
                    .entry(word[word.len() - key_len..].to_string())
                    .or_default()
                    .push(idx);
            }
            entries.push(AnalogEntry {
                word,
                breaks: record.breaks.clone(),
            });
        }
        anyhow::ensure!(
            !entries.is_empty(),
            "{method} indexed no analog entries from {}",
            path.display()
        );
        Ok(Self {
            id: format!(
                "{method}:{}:{}:entries{}",
                base_label,
                file_stem(path),
                entries.len()
            ),
            base: Box::new(base),
            config,
            options,
            entries,
            prefix_index,
            suffix_index,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        self.base.hyphenate_into(word, out)?;
        if !word.is_ascii()
            || !word.chars().all(|ch| ch.is_ascii_alphabetic())
            || word.len() < self.config.min_word_len
        {
            return Ok(());
        }
        let lower = word.to_ascii_lowercase();
        let mut candidate_scores = HashMap::<usize, i32>::new();
        for key_len in (self.options.min_key_len..=self.options.max_key_len.min(lower.len())).rev()
        {
            if let Some(indices) = self.prefix_index.get(&lower[..key_len]) {
                for idx in indices.iter().copied().take(self.options.max_candidates) {
                    *candidate_scores.entry(idx).or_insert(0) += key_len as i32;
                }
            }
            if let Some(indices) = self.suffix_index.get(&lower[lower.len() - key_len..]) {
                for idx in indices.iter().copied().take(self.options.max_candidates) {
                    *candidate_scores.entry(idx).or_insert(0) += key_len as i32;
                }
            }
        }
        if candidate_scores.is_empty() {
            return Ok(());
        }
        let mut ranked = candidate_scores.into_iter().collect::<Vec<_>>();
        ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        ranked.truncate(self.options.max_candidates);

        if self.options.whole_mask {
            let mut mask_votes = BTreeMap::<Vec<GraphemeIndex>, (i32, u32)>::new();
            for (idx, seed_score) in ranked {
                let entry = &self.entries[idx];
                if entry.word == lower {
                    continue;
                }
                let pre = ascii_lcp_len(&lower, &entry.word);
                let suf = ascii_lcsuf_len(&lower, &entry.word);
                let len_penalty = lower.len().abs_diff(entry.word.len()) as i32;
                let transfer_weight = (seed_score + pre as i32 + suf as i32 - len_penalty).max(1);
                let mut transferred =
                    transfer_breaks(&lower, &entry.word, &entry.breaks, &self.config)
                        .into_iter()
                        .filter(|boundary| !out.contains(boundary))
                        .collect::<Vec<_>>();
                transferred.sort_unstable();
                transferred.dedup();
                if transferred.is_empty() {
                    continue;
                }
                let slot = mask_votes.entry(transferred).or_insert((0, 0));
                slot.0 += transfer_weight;
                slot.1 = slot.1.saturating_add(1);
            }
            if let Some((mask, (vote, sources))) = mask_votes.into_iter().max_by(
                |(left_mask, (left_vote, left_sources)),
                 (right_mask, (right_vote, right_sources))| {
                    left_vote
                        .cmp(right_vote)
                        .then_with(|| left_sources.cmp(right_sources))
                        .then_with(|| right_mask.len().cmp(&left_mask.len()))
                },
            ) {
                if vote >= self.options.min_vote && sources >= self.options.min_sources {
                    out.extend(mask);
                }
            }
            out.sort_unstable();
            out.dedup();
            return Ok(());
        }

        let mut votes = BTreeMap::<GraphemeIndex, (i32, u32)>::new();
        for (idx, seed_score) in ranked {
            let entry = &self.entries[idx];
            if entry.word == lower {
                continue;
            }
            let pre = ascii_lcp_len(&lower, &entry.word);
            let suf = ascii_lcsuf_len(&lower, &entry.word);
            let len_penalty = lower.len().abs_diff(entry.word.len()) as i32;
            let transfer_weight = (seed_score + pre as i32 + suf as i32 - len_penalty).max(1);
            for boundary in transfer_breaks(&lower, &entry.word, &entry.breaks, &self.config) {
                if out.contains(&boundary) {
                    continue;
                }
                let slot = votes.entry(boundary).or_insert((0, 0));
                slot.0 += transfer_weight;
                slot.1 = slot.1.saturating_add(1);
            }
        }
        for (boundary, (vote, sources)) in votes {
            if vote >= self.options.min_vote && sources >= self.options.min_sources {
                out.push(boundary);
            }
        }
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

fn parse_hypher_safe_options(method: &str) -> Result<(SafeNgramOptions, Option<SafeNgramOptions>)> {
    parse_safe_ngram_veto_options(method)
}

fn parse_affix_safe_add_options(method: &str) -> Result<AffixSafeAddOptions> {
    let lower = method.to_ascii_lowercase();
    let mut options = AffixSafeAddOptions {
        min_support: 3,
        min_precision_ppm: 980_000,
        max_negative: u32::MAX,
        min_suffix_len: 3,
        max_suffix_len: 12,
        min_prefix_len: 2,
        max_prefix_len: 8,
    };
    for part in lower.split('-') {
        if let Some(value) = part.strip_prefix('s') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.min_support = value
                    .parse::<u32>()
                    .with_context(|| format!("parse affix-safe-add support from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('p') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse affix-safe-add precision from {part:?}"))?;
                options.min_precision_ppm = if parsed <= 100 {
                    parsed.saturating_mul(10_000)
                } else if parsed <= 1000 {
                    parsed.saturating_mul(1000)
                } else {
                    parsed
                };
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('n') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.max_negative = value
                    .parse::<u32>()
                    .with_context(|| format!("parse affix-safe-add max negatives from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix("suf") {
            if let Some((min, max)) = value.split_once('x') {
                options.min_suffix_len = min
                    .parse::<usize>()
                    .with_context(|| format!("parse affix-safe-add suffix min from {part:?}"))?;
                options.max_suffix_len = max
                    .parse::<usize>()
                    .with_context(|| format!("parse affix-safe-add suffix max from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix("pre") {
            if let Some((min, max)) = value.split_once('x') {
                options.min_prefix_len = min
                    .parse::<usize>()
                    .with_context(|| format!("parse affix-safe-add prefix min from {part:?}"))?;
                options.max_prefix_len = max
                    .parse::<usize>()
                    .with_context(|| format!("parse affix-safe-add prefix max from {part:?}"))?;
            }
        }
    }
    anyhow::ensure!(options.min_support > 0, "affix support must be positive");
    anyhow::ensure!(
        (1..=999_999).contains(&options.min_precision_ppm),
        "affix precision threshold must be in 1..=999999 ppm"
    );
    anyhow::ensure!(
        options.min_suffix_len <= options.max_suffix_len,
        "suffix length range must be non-empty"
    );
    anyhow::ensure!(
        options.min_prefix_len <= options.max_prefix_len,
        "prefix length range must be non-empty"
    );
    Ok(options)
}

fn parse_analog_safe_add_options(method: &str) -> Result<AnalogSafeAddOptions> {
    let lower = method.to_ascii_lowercase();
    let mut options = AnalogSafeAddOptions {
        min_key_len: 4,
        max_key_len: 9,
        max_candidates: 96,
        min_vote: 42,
        min_sources: 2,
        whole_mask: false,
    };
    for part in lower.split('-') {
        if matches!(part, "mask" | "whole" | "family") {
            options.whole_mask = true;
            continue;
        }
        if let Some(value) = part.strip_prefix('v') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.min_vote = value
                    .parse::<i32>()
                    .with_context(|| format!("parse analog-safe-add vote from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('c') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.max_candidates = value
                    .parse::<usize>()
                    .with_context(|| format!("parse analog-safe-add candidates from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('r') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.min_sources = value
                    .parse::<u32>()
                    .with_context(|| format!("parse analog-safe-add sources from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('k') {
            if let Some((min, max)) = value.split_once('x') {
                options.min_key_len = min
                    .parse::<usize>()
                    .with_context(|| format!("parse analog-safe-add key min from {part:?}"))?;
                options.max_key_len = max
                    .parse::<usize>()
                    .with_context(|| format!("parse analog-safe-add key max from {part:?}"))?;
            }
        }
    }
    anyhow::ensure!(
        options.min_key_len <= options.max_key_len,
        "analog key length range must be non-empty"
    );
    anyhow::ensure!(
        options.max_candidates > 0,
        "analog max candidates must be positive"
    );
    anyhow::ensure!(options.min_vote > 0, "analog vote must be positive");
    anyhow::ensure!(
        options.min_sources > 0,
        "analog source count must be positive"
    );
    Ok(options)
}

fn parse_pron_count_cap_options(method: &str) -> Result<PronCountCapOptions> {
    let lower = method.to_ascii_lowercase();
    let mut options = PronCountCapOptions {
        base_precision: 80,
        veto_precision: None,
        fill: false,
        wide_sources: false,
        orthographic_fallback: false,
        fallback_slack: 1,
    };
    for part in lower.split('-') {
        match part {
            "fill" | "add" | "decode" => {
                options.fill = true;
                continue;
            }
            "wide" | "full" => {
                options.wide_sources = true;
                continue;
            }
            "orth" | "ortho" | "fallback" => {
                options.orthographic_fallback = true;
                continue;
            }
            _ => {}
        }
        if let Some(value) = part.strip_prefix("slack") {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.fallback_slack = value.parse::<usize>().with_context(|| {
                    format!("parse pron-count-cap fallback slack from {part:?}")
                })?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('v') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.veto_precision = Some(value.parse::<u32>().with_context(|| {
                    format!("parse pron-count-cap veto precision from {part:?}")
                })?);
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('p') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.base_precision = value.parse::<u32>().with_context(|| {
                    format!("parse pron-count-cap base precision from {part:?}")
                })?;
            }
        }
    }
    anyhow::ensure!(
        (1..=999_999).contains(&options.base_precision),
        "pron-count-cap base precision must be in 1..=999999"
    );
    Ok(options)
}

fn parse_boundary_bayes_options(method: &str) -> Result<BoundaryBayesOptions> {
    let lower = method.to_ascii_lowercase();
    let mut options = BoundaryBayesOptions {
        min_support: 2,
        alpha: 0.5,
        threshold: 0.0,
    };
    for part in lower.split('-') {
        if let Some(value) = part.strip_prefix('s') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.min_support = value
                    .parse::<u32>()
                    .with_context(|| format!("parse boundary-bayes support from {part:?}"))?;
            }
        }
        if let Some(value) = part.strip_prefix('a') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse boundary-bayes alpha from {part:?}"))?;
                options.alpha = parsed as f32 / 100.0;
            }
        }
        if let Some(value) = part.strip_prefix('t') {
            let (negative, value) = value
                .strip_prefix('m')
                .map_or((false, value), |value| (true, value));
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse boundary-bayes threshold from {part:?}"))?
                    as f32
                    / 100.0;
                options.threshold = if negative { -parsed } else { parsed };
            }
        }
    }
    anyhow::ensure!(
        options.min_support > 0,
        "boundary-bayes support must be positive"
    );
    anyhow::ensure!(options.alpha > 0.0, "boundary-bayes alpha must be positive");
    Ok(options)
}

fn parse_stacked_bayes_options(method: &str) -> Result<StackedBayesOptions> {
    let lower = method.to_ascii_lowercase();
    let mut options = StackedBayesOptions {
        min_support: 2,
        alpha: 0.5,
        target_precision_ppm: 950_000,
        calibration_percent: 15,
        epochs: 3,
        learning_rate: 0.03,
        cap_vowel_nuclei: false,
    };
    for part in lower.split('-') {
        if matches!(part, "cap" | "vowelcap" | "nucleuscap") {
            options.cap_vowel_nuclei = true;
            continue;
        }
        if let Some(value) = part.strip_prefix('s') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.min_support = value
                    .parse::<u32>()
                    .with_context(|| format!("parse stacked-bayes support from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('a') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse stacked-bayes alpha from {part:?}"))?;
                options.alpha = parsed as f32 / 100.0;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('p') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value.parse::<u32>().with_context(|| {
                    format!("parse stacked-bayes target precision from {part:?}")
                })?;
                options.target_precision_ppm = if parsed <= 100 {
                    parsed.saturating_mul(10_000)
                } else if parsed <= 1000 {
                    parsed.saturating_mul(1000)
                } else {
                    parsed
                };
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('c') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.calibration_percent = value.parse::<u32>().with_context(|| {
                    format!("parse stacked-bayes calibration percent from {part:?}")
                })?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('e') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.epochs = value
                    .parse::<u32>()
                    .with_context(|| format!("parse stacked-bayes epochs from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('l') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse stacked-bayes learning rate from {part:?}"))?;
                options.learning_rate = parsed as f32 / 1000.0;
            }
        }
    }
    anyhow::ensure!(
        options.min_support > 0,
        "stacked-bayes support must be positive"
    );
    anyhow::ensure!(options.alpha > 0.0, "stacked-bayes alpha must be positive");
    anyhow::ensure!(
        (1..=999_999).contains(&options.target_precision_ppm),
        "stacked-bayes target precision must be in 1..=999999 ppm"
    );
    anyhow::ensure!(
        options.calibration_percent <= 50,
        "stacked-bayes calibration percent must be <= 50"
    );
    anyhow::ensure!(options.epochs > 0, "stacked-bayes epochs must be positive");
    anyhow::ensure!(
        options.learning_rate > 0.0,
        "stacked-bayes learning rate must be positive"
    );
    Ok(options)
}

fn parse_candidate_bayes_options(method: &str) -> Result<CandidateBayesOptions> {
    let lower = method.to_ascii_lowercase();
    let mut options = CandidateBayesOptions {
        min_support: 2,
        alpha: 1.0,
        target_precision_ppm: 950_000,
        calibration_percent: 20,
        epochs: 3,
        learning_rate: 0.03,
    };
    for part in lower.split('-') {
        if let Some(value) = part.strip_prefix('s') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.min_support = value
                    .parse::<u32>()
                    .with_context(|| format!("parse candidate-bayes support from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('a') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse candidate-bayes alpha from {part:?}"))?;
                options.alpha = parsed as f32 / 100.0;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('p') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value.parse::<u32>().with_context(|| {
                    format!("parse candidate-bayes target precision from {part:?}")
                })?;
                options.target_precision_ppm = if parsed <= 100 {
                    parsed.saturating_mul(10_000)
                } else if parsed <= 1000 {
                    parsed.saturating_mul(1000)
                } else {
                    parsed
                };
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('c') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.calibration_percent = value.parse::<u32>().with_context(|| {
                    format!("parse candidate-bayes calibration percent from {part:?}")
                })?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('e') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.epochs = value
                    .parse::<u32>()
                    .with_context(|| format!("parse candidate-bayes epochs from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('l') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value.parse::<u32>().with_context(|| {
                    format!("parse candidate-bayes learning rate from {part:?}")
                })?;
                options.learning_rate = parsed as f32 / 1000.0;
            }
        }
    }
    anyhow::ensure!(
        options.min_support > 0,
        "candidate-bayes support must be positive"
    );
    anyhow::ensure!(
        options.alpha > 0.0,
        "candidate-bayes alpha must be positive"
    );
    anyhow::ensure!(
        (1..=999_999).contains(&options.target_precision_ppm),
        "candidate-bayes target precision must be in 1..=999999 ppm"
    );
    anyhow::ensure!(
        options.calibration_percent <= 50,
        "candidate-bayes calibration percent must be <= 50"
    );
    anyhow::ensure!(
        options.epochs > 0,
        "candidate-bayes epochs must be positive"
    );
    anyhow::ensure!(
        options.learning_rate > 0.0,
        "candidate-bayes learning rate must be positive"
    );
    Ok(options)
}

fn parse_prune_bayes_options(method: &str) -> Result<PruneBayesOptions> {
    let lower = method.to_ascii_lowercase();
    let mut options = PruneBayesOptions {
        base_precision: 80,
        min_support: 2,
        alpha: 1.0,
        target_precision_ppm: 950_000,
        calibration_percent: 20,
        wide_sources: false,
    };
    for part in lower.split('-') {
        match part {
            "wide" | "full" => {
                options.wide_sources = true;
                continue;
            }
            _ => {}
        }
        if let Some(value) = part.strip_prefix('b') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.base_precision = value
                    .parse::<u32>()
                    .with_context(|| format!("parse prune-bayes base precision from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('s') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.min_support = value
                    .parse::<u32>()
                    .with_context(|| format!("parse prune-bayes support from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('a') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse prune-bayes alpha from {part:?}"))?;
                options.alpha = parsed as f32 / 100.0;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('p') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse prune-bayes target precision from {part:?}"))?;
                options.target_precision_ppm = if parsed <= 100 {
                    parsed.saturating_mul(10_000)
                } else if parsed <= 1000 {
                    parsed.saturating_mul(1000)
                } else {
                    parsed
                };
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('c') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.calibration_percent = value
                    .parse::<u32>()
                    .with_context(|| format!("parse prune-bayes calibration from {part:?}"))?;
            }
        }
    }
    anyhow::ensure!(
        options.base_precision > 0,
        "prune-bayes base precision must be positive"
    );
    anyhow::ensure!(
        options.min_support > 0,
        "prune-bayes support must be positive"
    );
    anyhow::ensure!(options.alpha > 0.0, "prune-bayes alpha must be positive");
    anyhow::ensure!(
        (1..=999_999).contains(&options.target_precision_ppm),
        "prune-bayes target precision must be in 1..=999999 ppm"
    );
    anyhow::ensure!(
        options.calibration_percent <= 50,
        "prune-bayes calibration percent must be <= 50"
    );
    Ok(options)
}

fn parse_candidate_gate_options(method: &str) -> Result<CandidateGateOptions> {
    let lower = method.to_ascii_lowercase();
    let mut options = CandidateGateOptions {
        kind: CandidateGateKind::VoteRaw3,
        target_precision_ppm: 950_000,
        min_support: 1,
        calibration_percent: 0,
    };
    for part in lower.split('-') {
        options.kind = match part {
            "vote" => CandidateGateKind::Vote,
            "votebucket" | "vote_bucket" | "bucket" => CandidateGateKind::VoteBucket,
            "raw3" | "vote_raw3" => CandidateGateKind::VoteRaw3,
            "raw4" | "vote_raw4" => CandidateGateKind::VoteRaw4,
            "raw5" | "vote_raw5" => CandidateGateKind::VoteRaw5,
            "cv4" | "vote_cv4" => CandidateGateKind::VoteCv4,
            "son4" | "vote_son4" | "sonority4" => CandidateGateKind::VoteSon4,
            _ => options.kind,
        };
        if let Some(value) = part.strip_prefix('p') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value.parse::<u32>().with_context(|| {
                    format!("parse candidate-gate target precision from {part:?}")
                })?;
                options.target_precision_ppm = if parsed <= 100 {
                    parsed.saturating_mul(10_000)
                } else if parsed <= 1000 {
                    parsed.saturating_mul(1000)
                } else {
                    parsed
                };
            }
        }
        if let Some(value) = part.strip_prefix('s') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.min_support = value
                    .parse::<u32>()
                    .with_context(|| format!("parse candidate-gate support from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('c') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.calibration_percent = value.parse::<u32>().with_context(|| {
                    format!("parse candidate-gate calibration percent from {part:?}")
                })?;
            }
        }
    }
    anyhow::ensure!(
        (1..=999_999).contains(&options.target_precision_ppm),
        "candidate-gate target precision must be in 1..=999999 ppm"
    );
    anyhow::ensure!(
        options.min_support > 0,
        "candidate-gate support must be positive"
    );
    anyhow::ensure!(
        options.calibration_percent <= 50,
        "candidate-gate calibration percent must be <= 50"
    );
    Ok(options)
}

fn parse_mask_rerank_options(method: &str) -> Result<MaskRerankOptions> {
    let lower = method.to_ascii_lowercase();
    let mut options = MaskRerankOptions {
        epochs: 3,
        learning_rate: 0.05,
        max_candidate_boundaries: 7,
        max_masks: 128,
        fp_weight: 1.10,
        fn_weight: 1.00,
        cap_vowel_nuclei: false,
        wide_sources: false,
    };
    for part in lower.split('-') {
        match part {
            "cap" | "vowelcap" | "nucleuscap" => {
                options.cap_vowel_nuclei = true;
                continue;
            }
            "nocap" | "uncapped" => {
                options.cap_vowel_nuclei = false;
                continue;
            }
            "wide" | "full" => {
                options.wide_sources = true;
                continue;
            }
            "lite" | "fast" => {
                options.wide_sources = false;
                continue;
            }
            _ => {}
        }
        if let Some(value) = part.strip_prefix('e') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.epochs = value
                    .parse::<u32>()
                    .with_context(|| format!("parse mask-rerank epochs from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('l') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse mask-rerank learning rate from {part:?}"))?;
                options.learning_rate = parsed as f32 / 1000.0;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('b') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.max_candidate_boundaries = value.parse::<usize>().with_context(|| {
                    format!("parse mask-rerank max candidate boundaries from {part:?}")
                })?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('m') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                options.max_masks = value
                    .parse::<usize>()
                    .with_context(|| format!("parse mask-rerank max masks from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix("fp") {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse mask-rerank fp weight from {part:?}"))?;
                options.fp_weight = parsed as f32 / 100.0;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix("fn") {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let parsed = value
                    .parse::<u32>()
                    .with_context(|| format!("parse mask-rerank fn weight from {part:?}"))?;
                options.fn_weight = parsed as f32 / 100.0;
            }
        }
    }
    anyhow::ensure!(options.epochs > 0, "mask-rerank epochs must be positive");
    anyhow::ensure!(
        options.learning_rate > 0.0,
        "mask-rerank learning rate must be positive"
    );
    anyhow::ensure!(
        (1..=16).contains(&options.max_candidate_boundaries),
        "mask-rerank candidate boundary cap must be in 1..=16"
    );
    anyhow::ensure!(
        options.max_masks > 0,
        "mask-rerank max masks must be positive"
    );
    anyhow::ensure!(
        options.fp_weight > 0.0 && options.fn_weight > 0.0,
        "mask-rerank fp/fn weights must be positive"
    );
    Ok(options)
}

fn candidate_gate_base_methods() -> [&'static str; 6] {
    [
        "hypher",
        "liang",
        "safe-ngram-multi-s1-p65-veto-multi-s1-n0",
        "safe-ngram-mixson-multi-s1-p50-veto-multi-s1-n0",
        "safe-ngram-multi-s1-p40-veto-mixcv-multi-s1-n0",
        "liang-safe-add-multi-s1-p40-veto-mixcv-multi-s1-n0",
    ]
}

fn mask_rerank_base_methods(wide_sources: bool) -> &'static [&'static str] {
    const LITE: [&str; 6] = [
        "hypher",
        "liang",
        "safe-ngram-multi-s1-p65-veto-multi-s1-n0",
        "safe-ngram-mixson-multi-s1-p50-veto-multi-s1-n0",
        "safe-ngram-multi-s1-p40-veto-mixcv-multi-s1-n0",
        "liang-safe-add-multi-s1-p65-veto-mixcv-multi-s1-n0",
    ];
    const WIDE: [&str; 9] = [
        "hypher",
        "liang",
        "safe-ngram-multi-s1-p65-veto-multi-s1-n0",
        "safe-ngram-mixson-multi-s1-p50-veto-multi-s1-n0",
        "safe-ngram-multi-s1-p40-veto-mixcv-multi-s1-n0",
        "safe-ngram-mixson-multi-s1-p90-veto-multi-s1-n0",
        "liang-safe-add-multi-s1-p65-veto-mixcv-multi-s1-n0",
        "liang-safe-add-multi-s1-p80-veto-mixcv-multi-s1-n0",
        "liang-safe-add-multi-s1-p90-veto-mixcv-multi-s1-n0",
    ];
    if wide_sources {
        &WIDE
    } else {
        &LITE
    }
}

fn prepare_mask_rerank_base_methods(
    locale: &str,
    path: &Path,
    patterns: Option<&PathBuf>,
    config: HyphenationConfig,
    records: &[HyphenationRecord],
    wide_sources: bool,
) -> Result<Vec<PreparedMethod>> {
    mask_rerank_base_methods(wide_sources)
        .iter()
        .map(|method| {
            prepare_candidate_base_method(method, locale, path, patterns, config.clone(), records)
                .with_context(|| format!("prepare mask-rerank base method {method:?}"))
        })
        .collect()
}

fn prepare_candidate_base_methods(
    locale: &str,
    path: &Path,
    patterns: Option<&PathBuf>,
    config: HyphenationConfig,
    records: &[HyphenationRecord],
) -> Result<Vec<PreparedMethod>> {
    candidate_gate_base_methods()
        .iter()
        .map(|method| {
            prepare_candidate_base_method(method, locale, path, patterns, config.clone(), records)
                .with_context(|| format!("prepare candidate base method {method:?}"))
        })
        .collect()
}

fn prepare_candidate_base_method(
    method: &str,
    locale: &str,
    path: &Path,
    patterns: Option<&PathBuf>,
    config: HyphenationConfig,
    records: &[HyphenationRecord],
) -> Result<PreparedMethod> {
    match method {
        "hypher" => {
            let adapter = adapter_for_method("hypher", locale)?;
            let mut adapter_config = adapter.config().clone();
            adapter_config.left_min = config.left_min;
            adapter_config.right_min = config.right_min;
            adapter_config.min_word_len = config.min_word_len;
            Ok(PreparedMethod::Adapter {
                inner: adapter,
                config: adapter_config,
            })
        }
        "liang" => prepare_liang(MethodOptions {
            method: "liang".to_string(),
            locale: locale.to_string(),
            patterns: patterns.cloned(),
            dictionary: None,
            dictionary_is_gold_oracle: false,
            external_command: None,
            left_min: Some(config.left_min),
            right_min: Some(config.right_min),
            min_word_len: Some(config.min_word_len),
        }),
        method if method.starts_with("safe-ngram") => Ok(PreparedMethod::SafeNgram(
            SafeNgramMethod::train(method, locale, path, config, records)?,
        )),
        method if method.starts_with("liang-safe-add") => {
            let base = prepare_liang(MethodOptions {
                method: "liang".to_string(),
                locale: locale.to_string(),
                patterns: patterns.cloned(),
                dictionary: None,
                dictionary_is_gold_oracle: false,
                external_command: None,
                left_min: Some(config.left_min),
                right_min: Some(config.right_min),
                min_word_len: Some(config.min_word_len),
            })?;
            let base_label = file_stem(
                patterns.context("--patterns is required for candidate liang-safe-add base")?,
            );
            Ok(PreparedMethod::BaseSafeAdd(BaseSafeAddMethod::train(
                method,
                &base_label,
                base,
                path,
                config,
                records,
            )?))
        }
        _ => anyhow::bail!("unsupported candidate base method {method:?}"),
    }
}

fn fit_candidate_bayes_weights(
    config: &HyphenationConfig,
    options: &CandidateBayesOptions,
    records: &[HyphenationRecord],
    methods: &[PreparedMethod],
) -> Result<(U64HashMap<f32>, f32)> {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut total_positive = 0u32;
    let mut total_negative = 0u32;
    let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); methods.len()];
    let mut features = SmallVec::<[u64; 96]>::new();

    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let gold = filtered_break_set(&record.word, &record.breaks, config);
        for (method, pred) in methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(record, pred)?;
        }
        let mut candidate = BTreeSet::<GraphemeIndex>::new();
        for pred in &predictions {
            candidate.extend(filtered_break_set(&record.word, pred, config));
        }
        let bytes = record.word.as_bytes();
        for boundary in candidate {
            let positive = gold.contains(&boundary);
            if positive {
                total_positive = total_positive.saturating_add(1);
            } else {
                total_negative = total_negative.saturating_add(1);
            }
            let mask = candidate_vote_mask(&predictions, boundary, &record.word, config);
            candidate_bayes_features(bytes, boundary as usize, mask, &mut features);
            for key in features.iter().copied() {
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    anyhow::ensure!(
        total_positive > 0 && total_negative > 0,
        "candidate-bayes needs positive and negative candidate boundaries"
    );
    let alpha = options.alpha;
    let positive_den = total_positive as f32 + 2.0 * alpha;
    let negative_den = total_negative as f32 + 2.0 * alpha;
    let mut weights = U64HashMap::<f32>::default();
    for (key, feature_counts) in counts {
        if feature_counts
            .positive
            .saturating_add(feature_counts.negative)
            < options.min_support
        {
            continue;
        }
        let p_feature_given_positive = (feature_counts.positive as f32 + alpha) / positive_den;
        let p_feature_given_negative = (feature_counts.negative as f32 + alpha) / negative_den;
        let weight = (p_feature_given_positive / p_feature_given_negative).ln();
        if weight.abs() >= 0.01 {
            weights.insert(key, weight);
        }
    }
    anyhow::ensure!(!weights.is_empty(), "candidate-bayes learned no features");
    let bias = ((total_positive as f32 + alpha) / (total_negative as f32 + alpha)).ln();
    Ok((weights, bias))
}

fn fit_candidate_logit_weights(
    config: &HyphenationConfig,
    options: &CandidateBayesOptions,
    records: &[HyphenationRecord],
    methods: &[PreparedMethod],
) -> Result<(U64HashMap<f32>, f32)> {
    let mut total_positive = 0u32;
    let mut total_negative = 0u32;
    let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); methods.len()];

    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let gold = filtered_break_set(&record.word, &record.breaks, config);
        for (method, pred) in methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(record, pred)?;
        }
        let mut candidate = BTreeSet::<GraphemeIndex>::new();
        for pred in &predictions {
            candidate.extend(filtered_break_set(&record.word, pred, config));
        }
        for boundary in candidate {
            if gold.contains(&boundary) {
                total_positive = total_positive.saturating_add(1);
            } else {
                total_negative = total_negative.saturating_add(1);
            }
        }
    }

    anyhow::ensure!(
        total_positive > 0 && total_negative > 0,
        "candidate-logit needs positive and negative candidate boundaries"
    );

    let alpha = options.alpha;
    let mut bias = ((total_positive as f32 + alpha) / (total_negative as f32 + alpha)).ln();
    let mut weights = U64HashMap::<f32>::default();
    let mut features = SmallVec::<[u64; 96]>::new();
    let mut updates = 0u64;

    for epoch in 0..options.epochs {
        let rate = options.learning_rate / (1.0 + epoch as f32 * 0.35);
        for record in records {
            if record.ambiguous
                || !record.word.is_ascii()
                || record.word.len() < config.min_word_len
            {
                continue;
            }
            let gold = filtered_break_set(&record.word, &record.breaks, config);
            for (method, pred) in methods.iter().zip(predictions.iter_mut()) {
                method.hyphenate_record_into(record, pred)?;
            }
            let mut candidate = BTreeSet::<GraphemeIndex>::new();
            for pred in &predictions {
                candidate.extend(filtered_break_set(&record.word, pred, config));
            }
            let bytes = record.word.as_bytes();
            for boundary in candidate {
                let target = if gold.contains(&boundary) { 1.0 } else { 0.0 };
                let mask = candidate_vote_mask(&predictions, boundary, &record.word, config);
                candidate_bayes_features(bytes, boundary as usize, mask, &mut features);
                let score = score_sparse_features(bias, &weights, &features);
                let gradient = target - sigmoid(score);
                bias += rate * gradient * 0.1;
                for key in features.iter().copied() {
                    let slot = weights.entry(key).or_insert(0.0);
                    *slot += rate * gradient;
                }
                updates += 1;
            }
        }
    }

    anyhow::ensure!(updates > 0, "candidate-logit produced no training updates");
    weights.retain(|_, weight| weight.abs() >= 0.001);
    Ok((weights, bias))
}

fn calibrate_candidate_bayes_threshold(
    config: &HyphenationConfig,
    options: &CandidateBayesOptions,
    weights: &U64HashMap<f32>,
    bias: f32,
    records: &[HyphenationRecord],
    methods: &[PreparedMethod],
) -> Result<f32> {
    let mut scored = Vec::<(f32, bool)>::new();
    let mut total_positive = 0usize;
    let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); methods.len()];
    let mut features = SmallVec::<[u64; 96]>::new();

    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let gold = filtered_break_set(&record.word, &record.breaks, config);
        total_positive += gold.len();
        for (method, pred) in methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(record, pred)?;
        }
        let mut candidate = BTreeSet::<GraphemeIndex>::new();
        for pred in &predictions {
            candidate.extend(filtered_break_set(&record.word, pred, config));
        }
        let bytes = record.word.as_bytes();
        for boundary in candidate {
            let positive = gold.contains(&boundary);
            let mask = candidate_vote_mask(&predictions, boundary, &record.word, config);
            candidate_bayes_features(bytes, boundary as usize, mask, &mut features);
            scored.push((score_sparse_features(bias, weights, &features), positive));
        }
    }

    anyhow::ensure!(
        total_positive > 0 && !scored.is_empty(),
        "candidate-bayes calibration split has no usable boundaries"
    );
    scored.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut best_recall = -1.0f64;
    let mut best_threshold = scored.first().map(|(score, _)| *score).unwrap_or(0.0);
    for (score, positive) in scored {
        if positive {
            tp += 1;
        } else {
            fp += 1;
        }
        let predicted = tp + fp;
        let precision_ppm = tp as f64 * 1_000_000.0 / predicted as f64;
        if precision_ppm + f64::EPSILON >= options.target_precision_ppm as f64 {
            let recall = tp as f64 / total_positive as f64;
            if recall >= best_recall {
                best_recall = recall;
                best_threshold = score;
            }
        }
    }
    Ok(best_threshold)
}

fn fit_prune_bayes_weights(
    config: &HyphenationConfig,
    options: &PruneBayesOptions,
    records: &[HyphenationRecord],
    base: &PreparedMethod,
    methods: &[PreparedMethod],
) -> Result<(U64HashMap<f32>, f32)> {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut total_positive = 0u32;
    let mut total_negative = 0u32;
    let mut base_pred = SmallVec::<[GraphemeIndex; 8]>::new();
    let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); methods.len()];
    let mut features = SmallVec::<[u64; 96]>::new();

    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let gold = filtered_break_set(&record.word, &record.breaks, config);
        base.hyphenate_record_into(record, &mut base_pred)?;
        base_pred = filtered_break_vec(&record.word, &base_pred, config);
        for (method, pred) in methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(record, pred)?;
        }
        let bytes = record.word.as_bytes();
        for boundary in base_pred.iter().copied() {
            let positive = gold.contains(&boundary);
            if positive {
                total_positive = total_positive.saturating_add(1);
            } else {
                total_negative = total_negative.saturating_add(1);
            }
            let mask = candidate_vote_mask(&predictions, boundary, &record.word, config);
            candidate_bayes_features(bytes, boundary as usize, mask, &mut features);
            for key in features.iter().copied() {
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    anyhow::ensure!(
        total_positive > 0 && total_negative > 0,
        "prune-bayes needs positive and negative base boundaries"
    );
    let alpha = options.alpha;
    let positive_den = total_positive as f32 + 2.0 * alpha;
    let negative_den = total_negative as f32 + 2.0 * alpha;
    let mut weights = U64HashMap::<f32>::default();
    for (key, feature_counts) in counts {
        if feature_counts
            .positive
            .saturating_add(feature_counts.negative)
            < options.min_support
        {
            continue;
        }
        let p_feature_given_positive = (feature_counts.positive as f32 + alpha) / positive_den;
        let p_feature_given_negative = (feature_counts.negative as f32 + alpha) / negative_den;
        let weight = (p_feature_given_positive / p_feature_given_negative).ln();
        if weight.abs() >= 0.01 {
            weights.insert(key, weight);
        }
    }
    anyhow::ensure!(!weights.is_empty(), "prune-bayes learned no features");
    let bias = ((total_positive as f32 + alpha) / (total_negative as f32 + alpha)).ln();
    Ok((weights, bias))
}

fn calibrate_prune_bayes_threshold(
    config: &HyphenationConfig,
    options: &PruneBayesOptions,
    weights: &U64HashMap<f32>,
    bias: f32,
    records: &[HyphenationRecord],
    base: &PreparedMethod,
    methods: &[PreparedMethod],
) -> Result<f32> {
    let mut scored = Vec::<(f32, bool)>::new();
    let mut total_positive = 0usize;
    let mut base_pred = SmallVec::<[GraphemeIndex; 8]>::new();
    let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); methods.len()];
    let mut features = SmallVec::<[u64; 96]>::new();

    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let gold = filtered_break_set(&record.word, &record.breaks, config);
        total_positive += gold.len();
        base.hyphenate_record_into(record, &mut base_pred)?;
        base_pred = filtered_break_vec(&record.word, &base_pred, config);
        for (method, pred) in methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(record, pred)?;
        }
        let bytes = record.word.as_bytes();
        for boundary in base_pred.iter().copied() {
            let positive = gold.contains(&boundary);
            let mask = candidate_vote_mask(&predictions, boundary, &record.word, config);
            candidate_bayes_features(bytes, boundary as usize, mask, &mut features);
            scored.push((score_sparse_features(bias, weights, &features), positive));
        }
    }

    anyhow::ensure!(
        total_positive > 0 && !scored.is_empty(),
        "prune-bayes calibration split has no usable boundaries"
    );
    scored.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut best_recall = -1.0f64;
    let mut best_threshold = scored.first().map(|(score, _)| *score).unwrap_or(0.0);
    for (score, positive) in scored {
        if positive {
            tp += 1;
        } else {
            fp += 1;
        }
        let predicted = tp + fp;
        let precision_ppm = tp as f64 * 1_000_000.0 / predicted as f64;
        if precision_ppm + f64::EPSILON >= options.target_precision_ppm as f64 {
            let recall = tp as f64 / total_positive as f64;
            if recall >= best_recall {
                best_recall = recall;
                best_threshold = score;
            }
        }
    }
    Ok(best_threshold)
}

fn score_sparse_features(bias: f32, weights: &U64HashMap<f32>, features: &[u64]) -> f32 {
    let mut score = bias;
    for key in features {
        if let Some(weight) = weights.get(key) {
            score += *weight;
        }
    }
    score
}

fn candidate_bayes_features(
    bytes: &[u8],
    boundary: usize,
    vote_mask: u64,
    out: &mut SmallVec<[u64; 96]>,
) {
    out.clear();
    let mut base = SmallVec::<[u64; 32]>::new();
    boundary_bayes_features(bytes, boundary, &mut base);
    out.extend(base);

    let vote_count = vote_mask.count_ones() as u64;
    let bucket = safe_ngram_boundary_bucket(bytes.len(), boundary);
    out.push(boundary_bayes_pack(160, vote_mask));
    out.push(boundary_bayes_pack(161, vote_count));
    out.push(boundary_bayes_pack(162, (vote_mask << 8) | bucket));
    out.push(boundary_bayes_pack(
        163,
        (vote_count << 8) | boundary_bayes_len_bucket(bytes.len()),
    ));
    for idx in 0..8 {
        if vote_mask & (1 << idx) != 0 {
            out.push(boundary_bayes_pack(168 + idx, 1));
            out.push(boundary_bayes_pack(176 + idx, bucket));
        }
    }
    candidate_suffix_features(bytes, boundary, out);
    out.sort_unstable();
    out.dedup();
}

fn candidate_suffix_features(bytes: &[u8], boundary: usize, out: &mut SmallVec<[u64; 96]>) {
    const SUFFIXES: [&[u8]; 24] = [
        b"tion", b"sion", b"able", b"ible", b"ing", b"ed", b"ly", b"ally", b"ness", b"ment",
        b"less", b"ful", b"ous", b"ious", b"ation", b"ization", b"ity", b"ive", b"ize", b"ise",
        b"al", b"ic", b"ical", b"ology",
    ];
    for (idx, suffix) in SUFFIXES.iter().enumerate() {
        if bytes.len() <= suffix.len() || !ascii_ends_with_ignore_case(bytes, suffix) {
            continue;
        }
        let suffix_start = bytes.len() - suffix.len();
        out.push(boundary_bayes_pack(192, idx as u64));
        if boundary == suffix_start {
            out.push(boundary_bayes_pack(193, idx as u64));
        }
        let distance = boundary.abs_diff(suffix_start);
        if distance <= 3 {
            out.push(boundary_bayes_pack(
                194,
                (idx as u64) << 3 | distance as u64,
            ));
        }
    }
}

fn generate_mask_candidates(
    word: &str,
    predictions: &[SmallVec<[GraphemeIndex; 8]>],
    config: &HyphenationConfig,
    options: &MaskRerankOptions,
) -> Vec<MaskCandidate> {
    let normalized_predictions = predictions
        .iter()
        .map(|pred| filtered_break_vec(word, pred, config))
        .collect::<Vec<_>>();
    let mut union = BTreeSet::<GraphemeIndex>::new();
    for pred in &normalized_predictions {
        union.extend(pred.iter().copied());
    }

    let mut seen = HashSet::<Vec<GraphemeIndex>>::new();
    let mut candidates = Vec::<MaskCandidate>::new();
    add_mask_candidate(
        word,
        &[],
        &union,
        &normalized_predictions,
        config,
        options,
        &mut seen,
        &mut candidates,
    );
    for pred in &normalized_predictions {
        add_mask_candidate(
            word,
            pred,
            &union,
            &normalized_predictions,
            config,
            options,
            &mut seen,
            &mut candidates,
        );
    }

    for threshold in 1..=normalized_predictions.len() {
        let mask = union
            .iter()
            .copied()
            .filter(|boundary| {
                boundary_vote_count(&normalized_predictions, *boundary) >= threshold as u32
            })
            .collect::<SmallVec<[GraphemeIndex; 8]>>();
        add_mask_candidate(
            word,
            &mask,
            &union,
            &normalized_predictions,
            config,
            options,
            &mut seen,
            &mut candidates,
        );
    }

    let mut ranked_boundaries = union.iter().copied().collect::<Vec<_>>();
    ranked_boundaries.sort_by(|left, right| {
        mask_boundary_score(&normalized_predictions, *right)
            .cmp(&mask_boundary_score(&normalized_predictions, *left))
            .then_with(|| left.cmp(right))
    });
    for keep in 1..=ranked_boundaries
        .len()
        .min(options.max_candidate_boundaries)
    {
        let mut mask = ranked_boundaries[..keep]
            .iter()
            .copied()
            .collect::<SmallVec<[GraphemeIndex; 8]>>();
        mask.sort_unstable();
        add_mask_candidate(
            word,
            &mask,
            &union,
            &normalized_predictions,
            config,
            options,
            &mut seen,
            &mut candidates,
        );
    }

    let top_boundaries = ranked_boundaries
        .iter()
        .take(options.max_candidate_boundaries)
        .copied()
        .collect::<Vec<_>>();
    for pred in &normalized_predictions {
        for boundary in &top_boundaries {
            if !pred.contains(boundary) {
                let mut mask = pred.clone();
                mask.push(*boundary);
                mask.sort_unstable();
                mask.dedup();
                add_mask_candidate(
                    word,
                    &mask,
                    &union,
                    &normalized_predictions,
                    config,
                    options,
                    &mut seen,
                    &mut candidates,
                );
            }
        }
        for boundary in pred {
            let mask = pred
                .iter()
                .copied()
                .filter(|idx| idx != boundary)
                .collect::<SmallVec<[GraphemeIndex; 8]>>();
            add_mask_candidate(
                word,
                &mask,
                &union,
                &normalized_predictions,
                config,
                options,
                &mut seen,
                &mut candidates,
            );
        }
    }

    let mut subset_boundaries = top_boundaries;
    subset_boundaries.sort_unstable();
    let subset_count = if subset_boundaries.len() >= usize::BITS as usize {
        usize::MAX
    } else {
        1usize << subset_boundaries.len()
    };
    if subset_count <= options.max_masks {
        for bits in 0..subset_count {
            let mask = subset_boundaries
                .iter()
                .enumerate()
                .filter_map(|(idx, boundary)| ((bits & (1usize << idx)) != 0).then_some(*boundary))
                .collect::<SmallVec<[GraphemeIndex; 8]>>();
            add_mask_candidate(
                word,
                &mask,
                &union,
                &normalized_predictions,
                config,
                options,
                &mut seen,
                &mut candidates,
            );
        }
    }

    candidates
}

#[allow(clippy::too_many_arguments)]
fn add_mask_candidate(
    word: &str,
    breaks: &[GraphemeIndex],
    union: &BTreeSet<GraphemeIndex>,
    predictions: &[SmallVec<[GraphemeIndex; 8]>],
    config: &HyphenationConfig,
    options: &MaskRerankOptions,
    seen: &mut HashSet<Vec<GraphemeIndex>>,
    candidates: &mut Vec<MaskCandidate>,
) {
    if candidates.len() >= options.max_masks {
        return;
    }
    let mut normalized = filtered_break_vec(word, breaks, config);
    if options.cap_vowel_nuclei {
        let cap = stacked_vowel_break_cap(word.as_bytes());
        if normalized.len() > cap {
            normalized.truncate(cap);
        }
    }
    let key = normalized.iter().copied().collect::<Vec<_>>();
    if !seen.insert(key) {
        return;
    }
    let features = mask_rerank_features(word, &normalized, union, predictions);
    let groups = mask_group_keys(word, &normalized, union, predictions);
    candidates.push(MaskCandidate {
        breaks: normalized,
        features,
        groups,
    });
}

fn filtered_break_vec(
    word: &str,
    breaks: &[GraphemeIndex],
    config: &HyphenationConfig,
) -> SmallVec<[GraphemeIndex; 8]> {
    let mut out = filtered_break_set(word, breaks, config)
        .into_iter()
        .collect::<SmallVec<[GraphemeIndex; 8]>>();
    out.sort_unstable();
    out.dedup();
    out
}

fn mask_rerank_features(
    word: &str,
    mask: &[GraphemeIndex],
    union: &BTreeSet<GraphemeIndex>,
    predictions: &[SmallVec<[GraphemeIndex; 8]>],
) -> Vec<u64> {
    let bytes = word.as_bytes();
    let mut out = Vec::<u64>::with_capacity(128);
    let len_bucket = boundary_bayes_len_bucket(bytes.len());
    let cap = stacked_vowel_break_cap(bytes);
    let mask_len = mask.len();
    out.push(boundary_bayes_pack(200, len_bucket));
    out.push(boundary_bayes_pack(201, mask_len.min(15) as u64));
    out.push(boundary_bayes_pack(
        202,
        mask_len.abs_diff(cap).min(15) as u64,
    ));
    out.push(boundary_bayes_pack(
        203,
        ((len_bucket & 0xf) << 4) | mask_len.min(15) as u64,
    ));

    let exact_sources = predictions
        .iter()
        .enumerate()
        .fold(0u64, |acc, (idx, pred)| {
            if mask_equal(mask, pred) {
                acc | (1 << idx)
            } else {
                acc
            }
        });
    out.push(boundary_bayes_pack(204, exact_sources));

    let (min_segment, max_segment, short_segments, no_vowel_segments) =
        mask_segment_stats(bytes, mask);
    out.push(boundary_bayes_pack(205, min_segment.min(15) as u64));
    out.push(boundary_bayes_pack(206, max_segment.min(31) as u64));
    out.push(boundary_bayes_pack(207, short_segments.min(15) as u64));
    out.push(boundary_bayes_pack(208, no_vowel_segments.min(15) as u64));

    for (idx, pred) in predictions.iter().enumerate() {
        let distance = mask_symmetric_distance(mask, pred).min(15) as u64;
        out.push(boundary_bayes_pack(210 + idx as u8, distance));
    }

    let mut vote_hist = [0u8; 10];
    for boundary in mask {
        let vote_count = boundary_vote_count(predictions, *boundary).min(9) as usize;
        vote_hist[vote_count] = vote_hist[vote_count].saturating_add(1);
        let vote_mask = candidate_vote_mask_from_normalized(predictions, *boundary);
        let mut local = SmallVec::<[u64; 96]>::new();
        candidate_bayes_features(bytes, *boundary as usize, vote_mask, &mut local);
        out.extend(
            local
                .into_iter()
                .map(|feature| mix_u64(feature ^ 0xa511_0000_0000_0000)),
        );
    }
    for (vote_count, count) in vote_hist.iter().enumerate() {
        if *count > 0 {
            out.push(boundary_bayes_pack(
                224,
                ((vote_count as u64) << 8) | u64::from(*count),
            ));
        }
    }
    for boundary in union {
        if mask.contains(boundary) {
            continue;
        }
        let vote_count = boundary_vote_count(predictions, *boundary);
        if vote_count >= 2 {
            out.push(boundary_bayes_pack(
                225,
                (u64::from(vote_count.min(15)) << 8)
                    | safe_ngram_boundary_bucket(bytes.len(), *boundary as usize),
            ));
        }
    }
    mask_suffix_features(bytes, mask, &mut out);
    out.sort_unstable();
    out.dedup();
    out
}

fn mask_group_keys(
    word: &str,
    mask: &[GraphemeIndex],
    union: &BTreeSet<GraphemeIndex>,
    predictions: &[SmallVec<[GraphemeIndex; 8]>],
) -> SmallVec<[u64; 8]> {
    let bytes = word.as_bytes();
    let cap = stacked_vowel_break_cap(bytes);
    let exact_sources = predictions
        .iter()
        .enumerate()
        .fold(0u64, |acc, (idx, pred)| {
            if mask_equal(mask, pred) {
                acc | (1 << idx)
            } else {
                acc
            }
        });
    let mut vote_hist = [0u8; 4];
    for boundary in mask {
        let votes = boundary_vote_count(predictions, *boundary).min(3) as usize;
        vote_hist[votes] = vote_hist[votes].saturating_add(1).min(15);
    }
    let hist = vote_hist
        .iter()
        .enumerate()
        .fold(0u64, |acc, (idx, count)| {
            acc | (u64::from(*count) << (idx * 4))
        });
    let distance_sig = predictions
        .iter()
        .take(12)
        .enumerate()
        .fold(0u64, |acc, (idx, pred)| {
            acc | ((mask_symmetric_distance(mask, pred).min(3) as u64) << (idx * 2))
        });
    let (min_segment, max_segment, short_segments, no_vowel_segments) =
        mask_segment_stats(bytes, mask);
    let segment_sig = (min_segment.min(7) as u64)
        | ((max_segment.min(15) as u64) << 3)
        | ((short_segments.min(7) as u64) << 7)
        | ((no_vowel_segments.min(7) as u64) << 10);
    let len_sig = (boundary_bayes_len_bucket(bytes.len()) << 8)
        | ((mask.len().min(15) as u64) << 4)
        | mask.len().abs_diff(cap).min(15) as u64;
    let union_sig = (union.len().min(15) as u64) << 8 | mask.len().min(15) as u64;
    let mut out = SmallVec::<[u64; 8]>::new();
    out.push(boundary_bayes_pack(
        236,
        (exact_sources << 24) | (hist << 8) | len_sig,
    ));
    out.push(boundary_bayes_pack(237, (distance_sig << 16) | len_sig));
    out.push(boundary_bayes_pack(238, (hist << 16) | segment_sig));
    out.push(boundary_bayes_pack(239, (exact_sources << 16) | union_sig));
    out.push(boundary_bayes_pack(240, len_sig));
    out.push(boundary_bayes_pack(241, segment_sig));
    out
}

fn mask_suffix_features(bytes: &[u8], mask: &[GraphemeIndex], out: &mut Vec<u64>) {
    const SUFFIXES: [&[u8]; 18] = [
        b"tion", b"sion", b"able", b"ible", b"ing", b"ed", b"ly", b"ness", b"ment", b"ous",
        b"ation", b"ization", b"ity", b"ive", b"ize", b"ise", b"al", b"ic",
    ];
    for (idx, suffix) in SUFFIXES.iter().enumerate() {
        if bytes.len() <= suffix.len() || !ascii_ends_with_ignore_case(bytes, suffix) {
            continue;
        }
        let suffix_start = bytes.len() - suffix.len();
        let near = mask
            .iter()
            .map(|boundary| (*boundary as usize).abs_diff(suffix_start))
            .min()
            .unwrap_or(15)
            .min(15);
        out.push(boundary_bayes_pack(232, idx as u64));
        out.push(boundary_bayes_pack(233, ((idx as u64) << 8) | near as u64));
    }
}

fn mask_segment_stats(bytes: &[u8], mask: &[GraphemeIndex]) -> (usize, usize, usize, usize) {
    let mut prev = 0usize;
    let mut min_segment = usize::MAX;
    let mut max_segment = 0usize;
    let mut short_segments = 0usize;
    let mut no_vowel_segments = 0usize;
    for boundary in mask
        .iter()
        .copied()
        .chain(std::iter::once(bytes.len() as GraphemeIndex))
    {
        let boundary = boundary as usize;
        let segment = &bytes[prev.min(bytes.len())..boundary.min(bytes.len())];
        let len = segment.len();
        min_segment = min_segment.min(len);
        max_segment = max_segment.max(len);
        if len <= 2 {
            short_segments += 1;
        }
        if !segment.iter().any(|byte| {
            matches!(
                byte.to_ascii_lowercase(),
                b'a' | b'e' | b'i' | b'o' | b'u' | b'y'
            )
        }) {
            no_vowel_segments += 1;
        }
        prev = boundary;
    }
    if min_segment == usize::MAX {
        min_segment = bytes.len();
    }
    (min_segment, max_segment, short_segments, no_vowel_segments)
}

fn mask_equal(left: &[GraphemeIndex], right: &[GraphemeIndex]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(left, right)| left == right)
}

fn mask_symmetric_distance(left: &[GraphemeIndex], right: &[GraphemeIndex]) -> usize {
    left.iter().filter(|idx| !right.contains(idx)).count()
        + right.iter().filter(|idx| !left.contains(idx)).count()
}

fn boundary_vote_count(
    predictions: &[SmallVec<[GraphemeIndex; 8]>],
    boundary: GraphemeIndex,
) -> u32 {
    predictions
        .iter()
        .filter(|pred| pred.contains(&boundary))
        .count() as u32
}

fn candidate_vote_mask_from_normalized(
    predictions: &[SmallVec<[GraphemeIndex; 8]>],
    boundary: GraphemeIndex,
) -> u64 {
    predictions
        .iter()
        .enumerate()
        .fold(0u64, |mask, (idx, pred)| {
            if pred.contains(&boundary) {
                mask | (1 << idx)
            } else {
                mask
            }
        })
}

fn mask_boundary_score(
    predictions: &[SmallVec<[GraphemeIndex; 8]>],
    boundary: GraphemeIndex,
) -> i32 {
    let weights = [6, 2, 6, 3, 2, 4, 4, 3, 3, 2, 2, 2];
    predictions
        .iter()
        .enumerate()
        .filter_map(|(idx, pred)| pred.contains(&boundary).then_some(weights[idx.min(11)]))
        .sum()
}

fn pron_count_boundary_score(
    predictions: &[SmallVec<[GraphemeIndex; 8]>],
    boundary: GraphemeIndex,
) -> i32 {
    let vote_score = mask_boundary_score(predictions, boundary);
    let vote_count = boundary_vote_count(predictions, boundary) as i32;
    vote_score + vote_count * 8
}

fn best_oracle_mask_candidate(
    candidates: &[MaskCandidate],
    gold: &[GraphemeIndex],
    fp_weight: f32,
    fn_weight: f32,
) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| {
            mask_candidate_oracle_score(&left.breaks, gold, fp_weight, fn_weight)
                .partial_cmp(&mask_candidate_oracle_score(
                    &right.breaks,
                    gold,
                    fp_weight,
                    fn_weight,
                ))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    mask_candidate_f1(&left.breaks, gold)
                        .partial_cmp(&mask_candidate_f1(&right.breaks, gold))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| right.breaks.len().cmp(&left.breaks.len()))
        })
        .map(|(idx, _)| idx)
}

fn mask_candidate_oracle_score(
    mask: &[GraphemeIndex],
    gold: &[GraphemeIndex],
    fp_weight: f32,
    fn_weight: f32,
) -> f32 {
    let tp = mask.iter().filter(|idx| gold.contains(idx)).count() as f32;
    let fp = mask.iter().filter(|idx| !gold.contains(idx)).count() as f32;
    let fn_ = gold.iter().filter(|idx| !mask.contains(idx)).count() as f32;
    tp - fp_weight * fp - fn_weight * fn_
}

fn mask_candidate_cost(
    mask: &[GraphemeIndex],
    gold: &[GraphemeIndex],
    fp_weight: f32,
    fn_weight: f32,
) -> f32 {
    let fp = mask.iter().filter(|idx| !gold.contains(idx)).count() as f32;
    let fn_ = gold.iter().filter(|idx| !mask.contains(idx)).count() as f32;
    fp_weight * fp + fn_weight * fn_
}

fn fit_mask_feature_reward_weights(examples: &[MaskTrainingExample]) -> U64HashMap<f32> {
    let mut stats = U64HashMap::<FloatFeatureStats>::default();
    let mut global_total = 0.0f32;
    let mut global_count = 0u32;
    for example in examples {
        for (candidate, cost) in example.candidates.iter().zip(example.costs.iter().copied()) {
            let quality = -cost;
            global_total += quality;
            global_count = global_count.saturating_add(1);
            for feature in &candidate.features {
                let slot = stats.entry(*feature).or_default();
                slot.count = slot.count.saturating_add(1);
                slot.total += quality;
            }
        }
    }
    if global_count == 0 {
        return U64HashMap::default();
    }
    let global_mean = global_total / global_count as f32;
    let prior = 8.0f32;
    let scale = 0.35f32;
    stats
        .into_iter()
        .filter_map(|(feature, stat)| {
            if stat.count < 2 {
                return None;
            }
            let mean = (stat.total + global_mean * prior) / (stat.count as f32 + prior);
            let weight = (mean - global_mean) * scale;
            (weight.abs() >= 0.001).then_some((feature, weight))
        })
        .collect()
}

fn mask_candidate_f1(mask: &[GraphemeIndex], gold: &[GraphemeIndex]) -> f32 {
    let tp = mask.iter().filter(|idx| gold.contains(idx)).count() as f32;
    let fp = mask.iter().filter(|idx| !gold.contains(idx)).count() as f32;
    let fn_ = gold.iter().filter(|idx| !mask.contains(idx)).count() as f32;
    if tp == 0.0 {
        if fp == 0.0 && fn_ == 0.0 {
            1.0
        } else {
            0.0
        }
    } else {
        2.0 * tp / (2.0 * tp + fp + fn_)
    }
}

fn best_scored_mask_candidate(candidates: &[MaskCandidate], weights: &U64HashMap<f32>) -> usize {
    candidates
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| {
            score_feature_list(&left.features, weights)
                .partial_cmp(&score_feature_list(&right.features, weights))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.breaks.len().cmp(&left.breaks.len()))
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn best_cost_augmented_mask_candidate(
    candidates: &[MaskCandidate],
    costs: &[f32],
    weights: &U64HashMap<f32>,
) -> usize {
    candidates
        .iter()
        .enumerate()
        .max_by(|(left_idx, left), (right_idx, right)| {
            (score_feature_list(&left.features, weights)
                + costs.get(*left_idx).copied().unwrap_or(0.0))
            .partial_cmp(
                &(score_feature_list(&right.features, weights)
                    + costs.get(*right_idx).copied().unwrap_or(0.0)),
            )
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.breaks.len().cmp(&left.breaks.len()))
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn best_cost_table_mask_candidate(
    candidates: &[MaskCandidate],
    group_costs: &U64HashMap<FloatFeatureStats>,
    global_cost: f32,
) -> usize {
    candidates
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            mask_cost_table_score(left, group_costs, global_cost)
                .partial_cmp(&mask_cost_table_score(right, group_costs, global_cost))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.breaks.len().cmp(&right.breaks.len()))
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn mask_cost_table_score(
    candidate: &MaskCandidate,
    group_costs: &U64HashMap<FloatFeatureStats>,
    global_cost: f32,
) -> f32 {
    const WEIGHTS: [f32; 6] = [5.0, 4.0, 3.0, 2.0, 1.0, 1.0];
    let prior = 6.0f32;
    let mut total = 0.0f32;
    let mut weight_total = 0.0f32;
    for (idx, group) in candidate.groups.iter().enumerate() {
        let weight = WEIGHTS.get(idx).copied().unwrap_or(1.0);
        let expected = group_costs.get(group).map_or(global_cost, |stats| {
            (stats.total + global_cost * prior) / (stats.count as f32 + prior)
        });
        total += expected * weight;
        weight_total += weight;
    }
    if weight_total == 0.0 {
        global_cost
    } else {
        total / weight_total
    }
}

fn score_feature_list(features: &[u64], weights: &U64HashMap<f32>) -> f32 {
    features
        .iter()
        .filter_map(|feature| weights.get(feature))
        .copied()
        .sum()
}

fn add_sparse_features(weights: &mut U64HashMap<f32>, features: &[u64], delta: f32) {
    for feature in features {
        *weights.entry(*feature).or_insert(0.0) += delta;
    }
}

fn ascii_ends_with_ignore_case(bytes: &[u8], suffix: &[u8]) -> bool {
    bytes.len() >= suffix.len()
        && bytes[bytes.len() - suffix.len()..]
            .iter()
            .zip(suffix.iter())
            .all(|(left, right)| left.to_ascii_lowercase() == *right)
}

fn learn_candidate_gate(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    options: &CandidateGateOptions,
    methods: &[PreparedMethod],
) -> Result<U64HashSet> {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut predictions = vec![SmallVec::<[GraphemeIndex; 8]>::new(); methods.len()];
    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let gold = filtered_break_set(&record.word, &record.breaks, config);
        for (method, pred) in methods.iter().zip(predictions.iter_mut()) {
            method.hyphenate_record_into(record, pred)?;
        }
        let mut candidate = BTreeSet::<GraphemeIndex>::new();
        for pred in &predictions {
            candidate.extend(filtered_break_set(&record.word, pred, config));
        }
        let bytes = record.word.as_bytes();
        for boundary in candidate {
            let mask = candidate_vote_mask(&predictions, boundary, &record.word, config);
            let key = candidate_gate_key(options.kind, bytes, boundary as usize, mask);
            let positive = gold.contains(&boundary);
            add_candidate_group_count(&mut counts, key, positive);
        }
    }

    let mut groups = counts
        .into_iter()
        .filter(|(_, counts)| {
            counts.positive.saturating_add(counts.negative) >= options.min_support
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        group_precision(right.1)
            .partial_cmp(&group_precision(left.1))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.1.positive.cmp(&left.1.positive))
    });

    let mut selected = U64HashSet::default();
    let mut tp = 0usize;
    let mut fp = 0usize;
    for (key, group) in groups {
        let next_tp = tp + group.positive as usize;
        let next_fp = fp + group.negative as usize;
        let next_precision_ppm = next_tp as f64 * 1_000_000.0 / (next_tp + next_fp) as f64;
        if next_precision_ppm + f64::EPSILON >= options.target_precision_ppm as f64 {
            selected.insert(key);
            tp = next_tp;
            fp = next_fp;
        }
    }
    Ok(selected)
}

fn candidate_gate_key(
    kind: CandidateGateKind,
    bytes: &[u8],
    boundary: usize,
    vote_mask: u64,
) -> u64 {
    match kind {
        CandidateGateKind::Vote => vote_mask,
        CandidateGateKind::VoteBucket => {
            mix_u64((vote_mask << 8) ^ safe_ngram_boundary_bucket(bytes.len(), boundary))
        }
        CandidateGateKind::VoteRaw3 => mix_u64(
            (vote_mask << 56)
                ^ safe_ngram_key(
                    bytes,
                    boundary,
                    0,
                    SafeNgramSpec {
                        left: 3,
                        right: 3,
                        bucketed: false,
                        family: 0,
                    },
                ),
        ),
        CandidateGateKind::VoteRaw4 => mix_u64(
            (vote_mask << 56)
                ^ safe_ngram_key(
                    bytes,
                    boundary,
                    0,
                    SafeNgramSpec {
                        left: 4,
                        right: 4,
                        bucketed: false,
                        family: 0,
                    },
                ),
        ),
        CandidateGateKind::VoteRaw5 => mix_u64(
            (vote_mask << 56)
                ^ safe_ngram_key(
                    bytes,
                    boundary,
                    0,
                    SafeNgramSpec {
                        left: 5,
                        right: 5,
                        bucketed: false,
                        family: 0,
                    },
                ),
        ),
        CandidateGateKind::VoteCv4 => mix_u64(
            (vote_mask << 56)
                ^ safe_ngram_key(
                    bytes,
                    boundary,
                    0,
                    SafeNgramSpec {
                        left: 4,
                        right: 4,
                        bucketed: false,
                        family: 1,
                    },
                ),
        ),
        CandidateGateKind::VoteSon4 => mix_u64(
            (vote_mask << 56)
                ^ safe_ngram_key(
                    bytes,
                    boundary,
                    0,
                    SafeNgramSpec {
                        left: 4,
                        right: 4,
                        bucketed: false,
                        family: 2,
                    },
                ),
        ),
    }
}

fn stacked_bayes_features(
    bytes: &[u8],
    boundary: usize,
    votes: &StackedVotes,
    out: &mut SmallVec<[u64; 64]>,
) {
    out.clear();
    for size in 1..=5 {
        out.push(boundary_bayes_context_key(
            size as u8,
            bytes,
            boundary,
            size,
            size,
            safe_ngram_raw_code_at,
        ));
    }
    for size in 1..=4 {
        out.push(boundary_bayes_context_key(
            16 + size as u8,
            bytes,
            boundary,
            size,
            size,
            safe_ngram_cv_code_at,
        ));
        out.push(boundary_bayes_context_key(
            32 + size as u8,
            bytes,
            boundary,
            size,
            size,
            safe_ngram_sonority_code_at,
        ));
    }
    out.push(boundary_bayes_pack(
        48,
        safe_ngram_boundary_bucket(bytes.len(), boundary),
    ));
    out.push(boundary_bayes_pack(
        49,
        boundary_bayes_len_bucket(bytes.len()),
    ));
    out.push(boundary_bayes_pack(
        50,
        (safe_ngram_boundary_bucket(bytes.len(), boundary) << 4)
            | boundary_bayes_len_bucket(bytes.len()),
    ));
    for width in 2..=5 {
        if bytes.len() >= width {
            out.push(boundary_bayes_hash_slice(64 + width as u8, &bytes[..width]));
            out.push(boundary_bayes_hash_slice(
                72 + width as u8,
                &bytes[bytes.len() - width..],
            ));
        }
        if boundary >= width {
            out.push(boundary_bayes_hash_slice(
                80 + width as u8,
                &bytes[boundary - width..boundary],
            ));
        }
        if bytes.len().saturating_sub(boundary) >= width {
            out.push(boundary_bayes_hash_slice(
                88 + width as u8,
                &bytes[boundary..boundary + width],
            ));
        }
    }

    let mut vote_mask = 0u64;
    let mut vote_count = 0u64;
    for (idx, hit) in [
        stacked_vote_contains(&votes.hypher, boundary),
        stacked_vote_contains(&votes.liang, boundary),
        stacked_vote_contains(&votes.safe_p65, boundary),
        stacked_vote_contains(&votes.safe_mixson_p50, boundary),
        stacked_vote_contains(&votes.safe_p40_mixcv, boundary),
    ]
    .into_iter()
    .enumerate()
    {
        if hit {
            vote_mask |= 1 << idx;
            vote_count += 1;
            out.push(boundary_bayes_pack(128 + idx as u8, 1));
            out.push(boundary_bayes_pack(
                144 + idx as u8,
                safe_ngram_boundary_bucket(bytes.len(), boundary),
            ));
        }
    }
    out.push(boundary_bayes_pack(136, vote_mask));
    out.push(boundary_bayes_pack(137, vote_count));
    if vote_mask != 0 {
        out.push(boundary_bayes_pack(
            138,
            (vote_mask << 8) | safe_ngram_boundary_bucket(bytes.len(), boundary),
        ));
    }
    out.sort_unstable();
    out.dedup();
}

fn stacked_vote_contains(votes: &[GraphemeIndex], boundary: usize) -> bool {
    let boundary = boundary as GraphemeIndex;
    votes.contains(&boundary)
}

fn sigmoid(score: f32) -> f32 {
    if score >= 20.0 {
        1.0
    } else if score <= -20.0 {
        0.0
    } else {
        1.0 / (1.0 + (-score).exp())
    }
}

fn stacked_vowel_break_cap(bytes: &[u8]) -> usize {
    let mut nuclei = 0usize;
    let mut in_vowel = false;
    for byte in bytes.iter().copied() {
        let is_vowel = matches!(
            byte.to_ascii_lowercase(),
            b'a' | b'e' | b'i' | b'o' | b'u' | b'y'
        );
        if is_vowel && !in_vowel {
            nuclei += 1;
        }
        in_vowel = is_vowel;
    }
    nuclei.saturating_sub(1)
}

fn orthographic_break_estimate(bytes: &[u8]) -> usize {
    let mut nuclei = 0usize;
    let mut in_vowel = false;
    for byte in bytes.iter().copied() {
        let is_vowel = matches!(
            byte.to_ascii_lowercase(),
            b'a' | b'e' | b'i' | b'o' | b'u' | b'y'
        );
        if is_vowel && !in_vowel {
            nuclei += 1;
        }
        in_vowel = is_vowel;
    }
    if bytes.len() > 3
        && bytes
            .last()
            .is_some_and(|byte| byte.eq_ignore_ascii_case(&b'e'))
        && bytes
            .get(bytes.len().saturating_sub(2))
            .is_some_and(|byte| {
                !matches!(
                    byte.to_ascii_lowercase(),
                    b'a' | b'e' | b'i' | b'o' | b'u' | b'y'
                )
            })
        && nuclei > 1
    {
        nuclei -= 1;
    }
    if bytes.len() > 3
        && bytes[bytes.len() - 2].eq_ignore_ascii_case(&b'l')
        && bytes[bytes.len() - 1].eq_ignore_ascii_case(&b'e')
        && !matches!(
            bytes[bytes.len() - 3].to_ascii_lowercase(),
            b'a' | b'e' | b'i' | b'o' | b'u' | b'y'
        )
    {
        nuclei += 1;
    }
    nuclei.saturating_sub(1)
}

fn boundary_bayes_features(bytes: &[u8], boundary: usize, out: &mut SmallVec<[u64; 32]>) {
    out.clear();
    for size in 1..=5 {
        out.push(boundary_bayes_context_key(
            size as u8,
            bytes,
            boundary,
            size,
            size,
            safe_ngram_raw_code_at,
        ));
    }
    for size in 1..=4 {
        out.push(boundary_bayes_context_key(
            16 + size as u8,
            bytes,
            boundary,
            size,
            size,
            safe_ngram_cv_code_at,
        ));
        out.push(boundary_bayes_context_key(
            32 + size as u8,
            bytes,
            boundary,
            size,
            size,
            safe_ngram_sonority_code_at,
        ));
    }
    out.push(boundary_bayes_pack(
        48,
        safe_ngram_boundary_bucket(bytes.len(), boundary),
    ));
    out.push(boundary_bayes_pack(
        49,
        boundary_bayes_len_bucket(bytes.len()),
    ));
    out.push(boundary_bayes_pack(
        50,
        (safe_ngram_boundary_bucket(bytes.len(), boundary) << 4)
            | boundary_bayes_len_bucket(bytes.len()),
    ));
    for width in 2..=5 {
        if bytes.len() >= width {
            out.push(boundary_bayes_hash_slice(64 + width as u8, &bytes[..width]));
            out.push(boundary_bayes_hash_slice(
                72 + width as u8,
                &bytes[bytes.len() - width..],
            ));
        }
        if boundary >= width {
            out.push(boundary_bayes_hash_slice(
                80 + width as u8,
                &bytes[boundary - width..boundary],
            ));
        }
        if bytes.len().saturating_sub(boundary) >= width {
            out.push(boundary_bayes_hash_slice(
                88 + width as u8,
                &bytes[boundary..boundary + width],
            ));
        }
    }
    out.sort_unstable();
    out.dedup();
}

fn boundary_bayes_context_key(
    kind: u8,
    bytes: &[u8],
    boundary: usize,
    left: usize,
    right: usize,
    code_at: fn(&[u8], isize) -> u64,
) -> u64 {
    let padded_boundary = boundary as isize + 1;
    let mut payload = 0u64;
    let mut shift = 0u32;
    for offset in 0..left {
        let position = padded_boundary - left as isize + offset as isize;
        payload |= code_at(bytes, position) << shift;
        shift += 5;
    }
    for offset in 0..right {
        let position = padded_boundary + offset as isize;
        payload |= code_at(bytes, position) << shift;
        shift += 5;
    }
    boundary_bayes_pack(kind, payload)
}

fn boundary_bayes_hash_slice(kind: u8, bytes: &[u8]) -> u64 {
    let mut value = 0xcbf29ce484222325u64 ^ u64::from(kind);
    for byte in bytes {
        value ^= u64::from(byte.to_ascii_lowercase());
        value = value.wrapping_mul(0x100000001b3);
    }
    boundary_bayes_pack(kind, mix_u64(value))
}

fn boundary_bayes_pack(kind: u8, payload: u64) -> u64 {
    (u64::from(kind) << 56) | (payload & 0x00ff_ffff_ffff_ffff)
}

fn boundary_bayes_len_bucket(byte_len: usize) -> u64 {
    if byte_len <= 5 {
        0
    } else if byte_len <= 7 {
        1
    } else if byte_len <= 9 {
        2
    } else if byte_len <= 12 {
        3
    } else {
        4
    }
}

fn parse_safe_ngram_veto_options(
    method: &str,
) -> Result<(SafeNgramOptions, Option<SafeNgramOptions>)> {
    if let Some((add_part, veto_part)) = method.split_once("-veto-") {
        let add_options = parse_safe_ngram_options(add_part)?;
        let veto_options = parse_safe_ngram_options(&format!("safe-ngram-{veto_part}"))?;
        Ok((add_options, Some(veto_options)))
    } else {
        Ok((parse_safe_ngram_options(method)?, None))
    }
}

fn parse_safe_ladder_options(
    method: &str,
) -> Result<(String, SafeNgramOptions, Option<SafeNgramOptions>)> {
    let lower = method.to_ascii_lowercase();
    let mut base_precision = 65u32;
    let mut residual_precision = 75u32;
    let mut residual_family = "mixson";
    for part in lower.split('-') {
        if matches!(part, "raw" | "mixcv" | "mixson" | "mixbucket") {
            residual_family = part;
            continue;
        }
        if let Some(value) = part.strip_prefix('b') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                base_precision = value
                    .parse::<u32>()
                    .with_context(|| format!("parse safe-ladder base precision from {part:?}"))?;
            }
            continue;
        }
        if let Some(value) = part.strip_prefix('r') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                residual_precision = value.parse::<u32>().with_context(|| {
                    format!("parse safe-ladder residual precision from {part:?}")
                })?;
            }
        }
    }
    let base_method = format!("safe-ngram-multi-s1-p{base_precision}-veto-multi-s1-n0");
    let add_method = if residual_family == "raw" {
        format!("safe-ngram-multi-s1-p{residual_precision}")
    } else {
        format!("safe-ngram-{residual_family}-multi-s1-p{residual_precision}")
    };
    let add_options = parse_safe_ngram_options(&add_method)?;
    let veto_options = lower
        .split_once("-veto-")
        .map(|(_, veto_part)| parse_safe_ngram_options(&format!("safe-ngram-{veto_part}")))
        .transpose()?;
    Ok((base_method, add_options, veto_options))
}

fn learn_safe_ngram_rules(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    options: &SafeNgramOptions,
) -> (U64HashSet, usize) {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut trained_records = 0usize;

    for record in records {
        if record.ambiguous {
            continue;
        }
        if options.unicode_aware {
            let family_mask = safe_ngram_options_family_mask(options);
            let tables = safe_ngram_char_tables_if_simple(&record.word, family_mask)
                .unwrap_or_else(|| safe_ngram_grapheme_tables(&record.word, family_mask));
            let grapheme_len = tables.len;
            if grapheme_len < config.min_word_len {
                continue;
            }
            trained_records += 1;
            for boundary in config.left_min..=grapheme_len.saturating_sub(config.right_min) {
                let positive = record.breaks.contains(&(boundary as GraphemeIndex));
                for (spec_idx, spec) in options.specs.iter().enumerate() {
                    let key =
                        safe_ngram_grapheme_key(&tables, grapheme_len, boundary, spec_idx, *spec);
                    let slot = counts.entry(key).or_default();
                    if positive {
                        slot.positive = slot.positive.saturating_add(1);
                    } else {
                        slot.negative = slot.negative.saturating_add(1);
                    }
                }
            }
            continue;
        }

        if !record.word.is_ascii() {
            continue;
        }
        let bytes = record.word.as_bytes();
        if bytes.len() < config.min_word_len {
            continue;
        }
        trained_records += 1;
        for boundary in config.left_min..=bytes.len().saturating_sub(config.right_min) {
            let positive = record.breaks.contains(&(boundary as GraphemeIndex));
            for (spec_idx, spec) in options.specs.iter().enumerate() {
                let key = safe_ngram_key(bytes, boundary, spec_idx, *spec);
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    let rules = counts
        .into_iter()
        .filter_map(|(key, counts)| safe_ngram_counts_selected(counts, options).then_some(key))
        .collect::<U64HashSet>();
    (rules, trained_records)
}

fn learn_safe_ngram_veto_rules(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    add_options: &SafeNgramOptions,
    add_rules: &U64HashSet,
    veto_options: &SafeNgramOptions,
) -> U64HashSet {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();

    for record in records {
        if record.ambiguous {
            continue;
        }
        if add_options.unicode_aware || veto_options.unicode_aware {
            let family_mask = safe_ngram_family_mask(add_options, Some(veto_options));
            let tables = safe_ngram_char_tables_if_simple(&record.word, family_mask)
                .unwrap_or_else(|| safe_ngram_grapheme_tables(&record.word, family_mask));
            let grapheme_len = tables.len;
            if grapheme_len < config.min_word_len {
                continue;
            }
            for boundary in config.left_min..=grapheme_len.saturating_sub(config.right_min) {
                let add_hit = add_options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        let key = safe_ngram_grapheme_key(
                            &tables,
                            grapheme_len,
                            boundary,
                            spec_idx,
                            *spec,
                        );
                        add_rules.contains(&key)
                    });
                if !add_hit {
                    continue;
                }

                let positive = record.breaks.contains(&(boundary as GraphemeIndex));
                for (spec_idx, spec) in veto_options.specs.iter().enumerate() {
                    let key =
                        safe_ngram_grapheme_key(&tables, grapheme_len, boundary, spec_idx, *spec);
                    let slot = counts.entry(key).or_default();
                    if positive {
                        slot.positive = slot.positive.saturating_add(1);
                    } else {
                        slot.negative = slot.negative.saturating_add(1);
                    }
                }
            }
            continue;
        }

        if !record.word.is_ascii() {
            continue;
        }
        let bytes = record.word.as_bytes();
        if bytes.len() < config.min_word_len {
            continue;
        }
        for boundary in config.left_min..=bytes.len().saturating_sub(config.right_min) {
            let add_hit = add_options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    add_rules.contains(&safe_ngram_key(bytes, boundary, spec_idx, *spec))
                });
            if !add_hit {
                continue;
            }

            let positive = record.breaks.contains(&(boundary as GraphemeIndex));
            for (spec_idx, spec) in veto_options.specs.iter().enumerate() {
                let key = safe_ngram_key(bytes, boundary, spec_idx, *spec);
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    counts
        .into_iter()
        .filter_map(|(key, counts)| {
            safe_ngram_veto_counts_selected(counts, veto_options).then_some(key)
        })
        .collect::<U64HashSet>()
}

fn learn_safe_ladder_residual_rules(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    base: &SafeNgramMethod,
    options: &SafeNgramOptions,
) -> Result<(U64HashSet, usize)> {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut base_pred = SmallVec::<[GraphemeIndex; 8]>::new();
    let mut trained_records = 0usize;

    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let bytes = record.word.as_bytes();
        base.hyphenate_into(&record.word, &mut base_pred)?;
        base_pred.retain(|boundary| {
            let boundary = *boundary as usize;
            boundary >= config.left_min && bytes.len().saturating_sub(boundary) >= config.right_min
        });
        base_pred.sort_unstable();
        base_pred.dedup();
        trained_records += 1;

        for boundary in config.left_min..=bytes.len().saturating_sub(config.right_min) {
            if base_pred.contains(&(boundary as GraphemeIndex)) {
                continue;
            }
            let positive = record.breaks.contains(&(boundary as GraphemeIndex));
            for (spec_idx, spec) in options.specs.iter().enumerate() {
                let key = safe_ngram_key(bytes, boundary, spec_idx, *spec);
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    Ok((
        counts
            .into_iter()
            .filter_map(|(key, counts)| safe_ngram_counts_selected(counts, options).then_some(key))
            .collect(),
        trained_records,
    ))
}

fn learn_safe_ladder_veto_rules(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    base: &SafeNgramMethod,
    add_options: &SafeNgramOptions,
    add_rules: &U64HashSet,
    veto_options: &SafeNgramOptions,
) -> Result<U64HashSet> {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut base_pred = SmallVec::<[GraphemeIndex; 8]>::new();

    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let bytes = record.word.as_bytes();
        base.hyphenate_into(&record.word, &mut base_pred)?;
        base_pred.retain(|boundary| {
            let boundary = *boundary as usize;
            boundary >= config.left_min && bytes.len().saturating_sub(boundary) >= config.right_min
        });
        base_pred.sort_unstable();
        base_pred.dedup();

        for boundary in config.left_min..=bytes.len().saturating_sub(config.right_min) {
            if base_pred.contains(&(boundary as GraphemeIndex)) {
                continue;
            }
            let add_hit = add_options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    add_rules.contains(&safe_ngram_key(bytes, boundary, spec_idx, *spec))
                });
            if !add_hit {
                continue;
            }

            let positive = record.breaks.contains(&(boundary as GraphemeIndex));
            for (spec_idx, spec) in veto_options.specs.iter().enumerate() {
                let key = safe_ngram_key(bytes, boundary, spec_idx, *spec);
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    Ok(counts
        .into_iter()
        .filter_map(|(key, counts)| {
            safe_ngram_veto_counts_selected(counts, veto_options).then_some(key)
        })
        .collect())
}

fn learn_hypher_veto_rules(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    options: &SafeNgramOptions,
    base: &dyn MethodAdapter,
) -> Result<U64HashSet> {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut pred = SmallVec::<[GraphemeIndex; 8]>::new();

    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let bytes = record.word.as_bytes();
        base.hyphenate_into(&record.word, &mut pred)?;
        for boundary in pred.iter().copied() {
            let boundary_usize = boundary as usize;
            if boundary_usize < config.left_min
                || bytes.len().saturating_sub(boundary_usize) < config.right_min
            {
                continue;
            }
            let positive = record.breaks.contains(&boundary);
            for (spec_idx, spec) in options.specs.iter().enumerate() {
                let key = safe_ngram_key(bytes, boundary_usize, spec_idx, *spec);
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    Ok(counts
        .into_iter()
        .filter_map(|(key, counts)| safe_ngram_veto_counts_selected(counts, options).then_some(key))
        .collect::<U64HashSet>())
}

fn learn_base_safe_add_veto_rules(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    add_options: &SafeNgramOptions,
    add_rules: &U64HashSet,
    veto_options: &SafeNgramOptions,
    base: &PreparedMethod,
) -> Result<U64HashSet> {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut pred = SmallVec::<[GraphemeIndex; 8]>::new();

    for record in records {
        if record.ambiguous || !record.word.is_ascii() || record.word.len() < config.min_word_len {
            continue;
        }
        let bytes = record.word.as_bytes();
        base.hyphenate_into(&record.word, &mut pred)?;
        pred.retain(|boundary| {
            let boundary = *boundary as usize;
            boundary >= config.left_min && bytes.len().saturating_sub(boundary) >= config.right_min
        });

        for boundary in config.left_min..=bytes.len().saturating_sub(config.right_min) {
            let add_hit = add_options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    add_rules.contains(&safe_ngram_key(bytes, boundary, spec_idx, *spec))
                });
            if add_hit {
                pred.push(boundary as GraphemeIndex);
            }
        }

        pred.sort_unstable();
        pred.dedup();
        for boundary in pred.iter().copied() {
            let boundary_usize = boundary as usize;
            let positive = record.breaks.contains(&boundary);
            for (spec_idx, spec) in veto_options.specs.iter().enumerate() {
                let key = safe_ngram_key(bytes, boundary_usize, spec_idx, *spec);
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    Ok(counts
        .into_iter()
        .filter_map(|(key, counts)| {
            safe_ngram_veto_counts_selected(counts, veto_options).then_some(key)
        })
        .collect::<U64HashSet>())
}

fn learn_residual_safe_ngram_rules(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    options: &SafeNgramOptions,
    base: &PreparedMethod,
) -> Result<(U64HashSet, usize)> {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut trained_records = 0usize;
    let mut pred = SmallVec::<[GraphemeIndex; 8]>::new();

    for record in records {
        if record.ambiguous || !record.word.is_ascii() {
            continue;
        }
        let bytes = record.word.as_bytes();
        if bytes.len() < config.min_word_len {
            continue;
        }
        trained_records += 1;
        base.hyphenate_record_into(record, &mut pred)?;
        pred.retain(|boundary| {
            let boundary = *boundary as usize;
            boundary >= config.left_min && bytes.len().saturating_sub(boundary) >= config.right_min
        });
        pred.sort_unstable();
        pred.dedup();

        for boundary in config.left_min..=bytes.len().saturating_sub(config.right_min) {
            let boundary_idx = boundary as GraphemeIndex;
            if pred.contains(&boundary_idx) {
                continue;
            }
            let positive = record.breaks.contains(&boundary_idx);
            for (spec_idx, spec) in options.specs.iter().enumerate() {
                let key = safe_ngram_key(bytes, boundary, spec_idx, *spec);
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    let rules = counts
        .into_iter()
        .filter_map(|(key, counts)| safe_ngram_counts_selected(counts, options).then_some(key))
        .collect::<U64HashSet>();
    Ok((rules, trained_records))
}

fn select_affix_rules(
    counts: U64HashMap<SafeNgramCounts>,
    options: &AffixSafeAddOptions,
) -> U64HashSet {
    counts
        .into_iter()
        .filter_map(|(key, counts)| {
            if counts.positive < options.min_support || counts.negative > options.max_negative {
                return None;
            }
            let total = counts.positive.saturating_add(counts.negative);
            if total == 0 {
                return None;
            }
            (u64::from(counts.positive) * 1_000_000
                >= u64::from(total) * u64::from(options.min_precision_ppm))
            .then_some(key)
        })
        .collect()
}

fn affix_suffix_key(bytes: &[u8], boundary: usize, suffix_len: usize) -> u64 {
    let suffix = &bytes[boundary..boundary + suffix_len.min(bytes.len().saturating_sub(boundary))];
    let left = if boundary == 0 {
        1
    } else {
        safe_ngram_sonority_code_at(bytes, boundary as isize)
    };
    let len_bucket = boundary_bayes_len_bucket(bytes.len());
    let mut hash = 0xcbf29ce484222325u64 ^ 0x5355_4646;
    for byte in suffix {
        hash ^= u64::from(byte.to_ascii_lowercase());
        hash = hash.wrapping_mul(0x100000001b3);
    }
    boundary_bayes_pack(
        242,
        mix_u64(hash ^ (left << 48) ^ (len_bucket << 44) ^ suffix_len as u64),
    )
}

fn affix_prefix_key(bytes: &[u8], prefix_len: usize) -> u64 {
    let prefix = &bytes[..prefix_len.min(bytes.len())];
    let right = safe_ngram_sonority_code_at(bytes, prefix_len as isize + 1);
    let len_bucket = boundary_bayes_len_bucket(bytes.len());
    let mut hash = 0xcbf29ce484222325u64 ^ 0x5052_4546;
    for byte in prefix {
        hash ^= u64::from(byte.to_ascii_lowercase());
        hash = hash.wrapping_mul(0x100000001b3);
    }
    boundary_bayes_pack(
        243,
        mix_u64(hash ^ (right << 48) ^ (len_bucket << 44) ^ prefix_len as u64),
    )
}

fn affix_right_context_key(bytes: &[u8], boundary: usize, right_len: usize) -> u64 {
    let end = boundary.saturating_add(right_len).min(bytes.len());
    let right = &bytes[boundary..end];
    let left_code = if boundary == 0 {
        1
    } else {
        safe_ngram_sonority_code_at(bytes, boundary as isize)
    };
    let bucket = safe_ngram_boundary_bucket(bytes.len(), boundary);
    let mut hash = 0xcbf29ce484222325u64 ^ 0x5249_4748;
    for byte in right {
        hash ^= u64::from(byte.to_ascii_lowercase());
        hash = hash.wrapping_mul(0x100000001b3);
    }
    boundary_bayes_pack(
        244,
        mix_u64(hash ^ (left_code << 48) ^ (bucket << 40) ^ right_len as u64),
    )
}

fn affix_left_context_key(bytes: &[u8], boundary: usize, left_len: usize) -> u64 {
    let start = boundary.saturating_sub(left_len);
    let left = &bytes[start..boundary.min(bytes.len())];
    let right_code = safe_ngram_sonority_code_at(bytes, boundary as isize + 1);
    let bucket = safe_ngram_boundary_bucket(bytes.len(), boundary);
    let mut hash = 0xcbf29ce484222325u64 ^ 0x4c45_4654;
    for byte in left {
        hash ^= u64::from(byte.to_ascii_lowercase());
        hash = hash.wrapping_mul(0x100000001b3);
    }
    boundary_bayes_pack(
        245,
        mix_u64(hash ^ (right_code << 48) ^ (bucket << 40) ^ left_len as u64),
    )
}

fn ascii_lcp_len(left: &str, right: &str) -> usize {
    left.bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count()
}

fn ascii_lcsuf_len(left: &str, right: &str) -> usize {
    left.bytes()
        .rev()
        .zip(right.bytes().rev())
        .take_while(|(left, right)| left == right)
        .count()
}

fn transfer_breaks(
    target: &str,
    source: &str,
    source_breaks: &[GraphemeIndex],
    config: &HyphenationConfig,
) -> SmallVec<[GraphemeIndex; 8]> {
    let pre = ascii_lcp_len(target, source);
    let suf = ascii_lcsuf_len(target, source);
    let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
    for boundary in source_breaks {
        let boundary = *boundary as usize;
        if boundary < pre
            && boundary >= config.left_min
            && target.len().saturating_sub(boundary) >= config.right_min
        {
            out.push(boundary as GraphemeIndex);
        }
        let right_distance = source.len().saturating_sub(boundary);
        if right_distance < suf {
            let target_boundary = target.len().saturating_sub(right_distance);
            if target_boundary >= config.left_min
                && target.len().saturating_sub(target_boundary) >= config.right_min
            {
                out.push(target_boundary as GraphemeIndex);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn parse_safe_ngram_options(method: &str) -> Result<SafeNgramOptions> {
    let lower = method.to_ascii_lowercase();
    let mut specs = vec![SafeNgramSpec {
        left: 4,
        right: 4,
        bucketed: false,
        family: 0,
    }];
    let mut min_support = 2u32;
    let mut max_negative = 0u32;
    let mut max_negative_set = false;
    let mut min_precision_ppm = None;
    let mut min_wilson_ppm = None;
    let mut bucketed = false;
    let mut mix_bucketed = false;
    let mut family = 0u8;
    let mut mix_cv = false;
    let mut mix_sonority = false;
    let mut cap_vowel_nuclei = false;
    let mut orthographic_veto = false;
    let mut unicode_aware = false;

    for part in lower.split('-') {
        if matches!(
            part,
            "unicode" | "unicodeaware" | "unicode_aware" | "uni" | "uchar"
        ) {
            unicode_aware = true;
            continue;
        }
        if matches!(part, "ucv" | "unicodecv") {
            unicode_aware = true;
            family = 1;
            continue;
        }
        if matches!(
            part,
            "uson" | "usonority" | "unicodeson" | "unicodesonority"
        ) {
            unicode_aware = true;
            family = 2;
            continue;
        }
        if matches!(part, "cap" | "vowelcap" | "nucleuscap") {
            cap_vowel_nuclei = true;
            continue;
        }
        if matches!(
            part,
            "orthoveto" | "orthographicveto" | "structveto" | "shapeveto"
        ) {
            orthographic_veto = true;
            continue;
        }
        if matches!(part, "cv" | "shape" | "consonantvowel") {
            family = 1;
            continue;
        }
        if matches!(part, "mixcv" | "cvraw" | "rawcv" | "mixshape") {
            mix_cv = true;
            continue;
        }
        if matches!(part, "son" | "sonority") {
            family = 2;
            continue;
        }
        if matches!(part, "mixson" | "mixsonority" | "sonraw" | "rawson") {
            mix_sonority = true;
            continue;
        }
        if matches!(part, "bucket" | "bucketed" | "pos" | "position") {
            bucketed = true;
            continue;
        }
        if matches!(part, "mixbucket" | "bucketmix" | "mixedbucket") {
            mix_bucketed = true;
            continue;
        }
        if part == "multi" {
            specs = vec![
                SafeNgramSpec {
                    left: 5,
                    right: 5,
                    bucketed: false,
                    family,
                },
                SafeNgramSpec {
                    left: 4,
                    right: 4,
                    bucketed: false,
                    family,
                },
                SafeNgramSpec {
                    left: 3,
                    right: 3,
                    bucketed: false,
                    family,
                },
            ];
            continue;
        }
        if let Some((left, right)) = part.split_once('x') {
            let left = left
                .parse::<usize>()
                .with_context(|| format!("parse safe-ngram left context from {part:?}"))?;
            let right = right
                .parse::<usize>()
                .with_context(|| format!("parse safe-ngram right context from {part:?}"))?;
            anyhow::ensure!(
                left <= 5 && right <= 5 && left + right <= 10,
                "safe-ngram context must fit the packed key, got {left}x{right}"
            );
            specs = vec![SafeNgramSpec {
                left,
                right,
                bucketed: false,
                family,
            }];
            continue;
        }
        if let Some(value) = part.strip_prefix('s') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                min_support = value
                    .parse::<u32>()
                    .with_context(|| format!("parse safe-ngram support from {part:?}"))?;
            }
        }
        if let Some(value) = part.strip_prefix('n') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                max_negative = value
                    .parse::<u32>()
                    .with_context(|| format!("parse safe-ngram max negatives from {part:?}"))?;
                max_negative_set = true;
            }
        }
        if let Some(value) = part.strip_prefix('p') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let value = value
                    .parse::<u32>()
                    .with_context(|| format!("parse safe-ngram precision from {part:?}"))?;
                anyhow::ensure!(
                    (1..=999_999).contains(&value),
                    "safe-ngram precision threshold must be in 1..=999999"
                );
                min_precision_ppm = Some(if value <= 100 {
                    value * 10_000
                } else if value <= 1000 {
                    value * 1000
                } else {
                    value
                });
            }
        }
        if let Some(value) = part.strip_prefix('w') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let value = value
                    .parse::<u32>()
                    .with_context(|| format!("parse safe-ngram Wilson threshold from {part:?}"))?;
                anyhow::ensure!(
                    (1..=999_999).contains(&value),
                    "safe-ngram Wilson threshold must be in 1..=999999"
                );
                min_wilson_ppm = Some(if value <= 100 {
                    value * 10_000
                } else if value <= 1000 {
                    value * 1000
                } else {
                    value
                });
            }
        }
    }

    anyhow::ensure!(min_support > 0, "safe-ngram support must be positive");
    if (min_precision_ppm.is_some() || min_wilson_ppm.is_some()) && !max_negative_set {
        max_negative = u32::MAX;
    }
    for spec in &mut specs {
        spec.family = family;
    }
    if bucketed {
        for spec in &mut specs {
            spec.bucketed = true;
        }
    } else if mix_bucketed {
        let mut bucketed_specs = specs.clone();
        for spec in &mut bucketed_specs {
            spec.bucketed = true;
        }
        specs.extend(bucketed_specs);
    }
    if mix_cv {
        let mut cv_specs = specs.clone();
        for spec in &mut cv_specs {
            spec.family = 1;
        }
        specs.extend(cv_specs);
    }
    if mix_sonority {
        let mut sonority_specs = specs.clone();
        for spec in &mut sonority_specs {
            spec.family = 2;
        }
        specs.extend(sonority_specs);
    }
    Ok(SafeNgramOptions {
        specs,
        min_support,
        max_negative,
        min_precision_ppm,
        min_wilson_ppm,
        cap_vowel_nuclei,
        orthographic_veto,
        unicode_aware,
    })
}

fn safe_ngram_counts_selected(counts: SafeNgramCounts, options: &SafeNgramOptions) -> bool {
    if counts.positive < options.min_support || counts.negative > options.max_negative {
        return false;
    }
    if let Some(min_precision_ppm) = options.min_precision_ppm {
        let total = counts.positive.saturating_add(counts.negative);
        return u64::from(counts.positive) * 1_000_000
            >= u64::from(total) * u64::from(min_precision_ppm);
    }
    if let Some(min_wilson_ppm) = options.min_wilson_ppm {
        return safe_ngram_wilson_lower_ppm(counts.positive, counts.negative)
            >= f64::from(min_wilson_ppm);
    }
    true
}

fn safe_ngram_veto_counts_selected(counts: SafeNgramCounts, options: &SafeNgramOptions) -> bool {
    safe_ngram_counts_selected(
        SafeNgramCounts {
            positive: counts.negative,
            negative: counts.positive,
        },
        options,
    )
}

struct SafeNgramGraphemeTables {
    len: usize,
    raw: SmallVec<[u8; 32]>,
    cv: SmallVec<[u8; 32]>,
    sonority: SmallVec<[u8; 32]>,
}

impl SafeNgramGraphemeTables {
    fn codes(&self, family: u8) -> &[u8] {
        match family {
            1 => &self.cv,
            2 => &self.sonority,
            _ => &self.raw,
        }
    }
}

fn safe_ngram_uses_unicode_features(
    options: &SafeNgramOptions,
    veto_options: Option<&SafeNgramOptions>,
) -> bool {
    options.unicode_aware || veto_options.is_some_and(|options| options.unicode_aware)
}

fn safe_ngram_family_mask(
    options: &SafeNgramOptions,
    veto_options: Option<&SafeNgramOptions>,
) -> u8 {
    let mut mask = safe_ngram_options_family_mask(options);
    if let Some(veto_options) = veto_options {
        mask |= safe_ngram_options_family_mask(veto_options);
    }
    mask
}

fn safe_ngram_options_family_mask(options: &SafeNgramOptions) -> u8 {
    let mut mask = 0u8;
    for spec in &options.specs {
        mask |= match spec.family {
            1 => 1 << 1,
            2 => 1 << 2,
            _ => 1,
        };
    }
    mask
}

fn safe_ngram_grapheme_tables(word: &str, family_mask: u8) -> SafeNgramGraphemeTables {
    let mut len = 0usize;
    let mut raw = SmallVec::<[u8; 32]>::new();
    let mut cv = SmallVec::<[u8; 32]>::new();
    let mut sonority = SmallVec::<[u8; 32]>::new();
    for grapheme in UnicodeSegmentation::graphemes(word, true) {
        len += 1;
        let codes = safe_ngram_unicode_codes(grapheme);
        if family_mask & 1 != 0 {
            raw.push(codes.raw);
        }
        if family_mask & (1 << 1) != 0 {
            cv.push(codes.cv);
        }
        if family_mask & (1 << 2) != 0 {
            sonority.push(codes.sonority);
        }
    }
    SafeNgramGraphemeTables {
        len,
        raw,
        cv,
        sonority,
    }
}

fn safe_ngram_char_tables_if_simple(
    word: &str,
    family_mask: u8,
) -> Option<SafeNgramGraphemeTables> {
    let mut len = 0usize;
    let mut raw = SmallVec::<[u8; 32]>::new();
    let mut cv = SmallVec::<[u8; 32]>::new();
    let mut sonority = SmallVec::<[u8; 32]>::new();
    for ch in word.chars() {
        if !safe_ngram_char_is_single_grapheme(ch) {
            return None;
        }
        len += 1;
        let ch = safe_ngram_fast_lower_char(ch);
        let codes = safe_ngram_unicode_codes_lower_char(ch);
        if family_mask & 1 != 0 {
            raw.push(codes.raw);
        }
        if family_mask & (1 << 1) != 0 {
            cv.push(codes.cv);
        }
        if family_mask & (1 << 2) != 0 {
            sonority.push(codes.sonority);
        }
    }
    Some(SafeNgramGraphemeTables {
        len,
        raw,
        cv,
        sonority,
    })
}

fn safe_ngram_char_is_single_grapheme(ch: char) -> bool {
    !matches!(
        ch,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{fe20}'..='\u{fe2f}'
            | '\u{200d}'
    )
}

fn safe_ngram_grapheme_key(
    tables: &SafeNgramGraphemeTables,
    grapheme_len: usize,
    boundary: usize,
    spec_idx: usize,
    spec: SafeNgramSpec,
) -> u64 {
    debug_assert!(spec.left + spec.right <= 10);
    let codes = tables.codes(spec.family);
    let padded_boundary = boundary as isize + 1;
    let mut key = (spec_idx as u64) << 56;
    if spec.bucketed {
        key |= safe_ngram_boundary_bucket(grapheme_len, boundary) << 50;
    }
    let mut shift = 0u32;
    for offset in 0..spec.left {
        let position = padded_boundary - spec.left as isize + offset as isize;
        key |= safe_ngram_grapheme_code_at(codes, position) << shift;
        shift += 5;
    }
    for offset in 0..spec.right {
        let position = padded_boundary + offset as isize;
        key |= safe_ngram_grapheme_code_at(codes, position) << shift;
        shift += 5;
    }
    key
}

fn safe_ngram_grapheme_code_at(codes: &[u8], padded_position: isize) -> u64 {
    if padded_position <= 0 || padded_position > codes.len() as isize {
        return 1;
    }
    u64::from(codes[padded_position as usize - 1])
}

#[derive(Clone, Copy)]
struct SafeNgramUnicodeCodes {
    raw: u8,
    cv: u8,
    sonority: u8,
}

fn safe_ngram_unicode_codes(grapheme: &str) -> SafeNgramUnicodeCodes {
    let Some(ch) = safe_ngram_primary_lower_char(grapheme) else {
        return SafeNgramUnicodeCodes {
            raw: 31,
            cv: 8,
            sonority: 12,
        };
    };
    safe_ngram_unicode_codes_lower_char(ch)
}

fn safe_ngram_unicode_codes_lower_char(ch: char) -> SafeNgramUnicodeCodes {
    let base = safe_ngram_latin_base_letter(ch);
    let cyrillic_raw = safe_ngram_cyrillic_raw_code(ch);
    let known_alphabetic = base.is_some() || cyrillic_raw.is_some();
    let is_apostrophe = ch == '\'' || ch == '\u{2019}';
    let is_hyphen = ch == '-' || ch == '\u{2010}' || ch == '\u{2011}' || ch == '\u{2013}';
    let is_numeric = !known_alphabetic && !is_apostrophe && !is_hyphen && ch.is_numeric();
    let is_alphabetic =
        known_alphabetic || (!is_numeric && !is_apostrophe && !is_hyphen && ch.is_alphabetic());
    let is_vowel = base.is_some_and(|base| matches!(base, b'a' | b'e' | b'i' | b'o' | b'u'))
        || matches!(
            ch,
            'а' | 'е'
                | 'ё'
                | 'и'
                | 'о'
                | 'у'
                | 'ы'
                | 'э'
                | 'ю'
                | 'я'
                | 'і'
                | 'ї'
                | 'є'
                | 'ӧ'
                | 'ӱ'
        );

    let raw = if let Some(base) = base {
        base - b'a' + 2
    } else if let Some(code) = cyrillic_raw {
        code as u8
    } else if is_apostrophe {
        28
    } else if is_hyphen {
        29
    } else if is_numeric {
        30
    } else if is_alphabetic {
        (2 + (mix_u64(ch as u64) % 26)) as u8
    } else {
        31
    };

    let cv = if is_vowel {
        2
    } else if matches!(ch, 'y' | 'ý' | 'ÿ' | 'j' | 'w' | 'й') {
        3
    } else if is_alphabetic {
        4
    } else if is_apostrophe {
        5
    } else if is_hyphen {
        6
    } else if is_numeric {
        7
    } else {
        8
    };

    let sonority = if is_vowel {
        2
    } else if matches!(ch, 'y' | 'ý' | 'ÿ' | 'й') {
        3
    } else if base.is_some_and(|base| matches!(base, b'l' | b'r')) || matches!(ch, 'л' | 'р') {
        4
    } else if base.is_some_and(|base| matches!(base, b'm' | b'n')) || matches!(ch, 'м' | 'н') {
        5
    } else if base.is_some_and(|base| matches!(base, b'f' | b'v' | b's' | b'z' | b'h'))
        || matches!(ch, 'ф' | 'в' | 'с' | 'з' | 'х' | 'ш' | 'ж' | 'щ')
    {
        6
    } else if matches!(ch, 'w' | 'j') {
        7
    } else if is_alphabetic {
        8
    } else if is_apostrophe {
        9
    } else if is_hyphen {
        10
    } else if is_numeric {
        11
    } else {
        12
    };

    SafeNgramUnicodeCodes { raw, cv, sonority }
}

fn safe_ngram_fast_lower_char(ch: char) -> char {
    if ch.is_ascii() {
        return ch.to_ascii_lowercase();
    }
    match ch {
        'А'..='Я' => char::from_u32((ch as u32) + 32).unwrap_or(ch),
        'Ё' => 'ё',
        'І' => 'і',
        'Ї' => 'ї',
        'Є' => 'є',
        'Ў' => 'ў',
        'Ґ' => 'ґ',
        _ => ch,
    }
}

fn safe_ngram_cyrillic_raw_code(ch: char) -> Option<u64> {
    Some(match ch {
        'а' => 2,
        'е' | 'ё' => 3,
        'и' | 'й' | 'і' | 'ї' => 4,
        'о' => 5,
        'у' | 'ў' => 6,
        'ы' => 7,
        'э' | 'є' => 8,
        'ю' => 9,
        'я' => 10,
        'б' => 11,
        'в' => 12,
        'г' | 'ґ' => 13,
        'д' => 14,
        'ж' => 15,
        'з' => 16,
        'к' => 17,
        'л' => 18,
        'м' => 19,
        'н' => 20,
        'п' => 21,
        'р' => 22,
        'с' => 23,
        'т' => 24,
        'ф' => 25,
        'х' => 26,
        'ц' => 27,
        'ч' => 28,
        'ш' => 29,
        'щ' => 30,
        'ь' | 'ъ' => 31,
        _ => return None,
    })
}

fn safe_ngram_is_cyrillic_letter(ch: char) -> bool {
    matches!(
        ch,
        '\u{0400}'..='\u{052f}'
            | '\u{1c80}'..='\u{1c8f}'
            | '\u{2de0}'..='\u{2dff}'
            | '\u{a640}'..='\u{a69f}'
    )
}

fn safe_ngram_is_russian_cyrillic_letter(ch: char) -> bool {
    matches!(
        ch,
        'а' | 'б'
            | 'в'
            | 'г'
            | 'д'
            | 'е'
            | 'ё'
            | 'ж'
            | 'з'
            | 'и'
            | 'й'
            | 'к'
            | 'л'
            | 'м'
            | 'н'
            | 'о'
            | 'п'
            | 'р'
            | 'с'
            | 'т'
            | 'у'
            | 'ф'
            | 'х'
            | 'ц'
            | 'ч'
            | 'ш'
            | 'щ'
            | 'ъ'
            | 'ы'
            | 'ь'
            | 'э'
            | 'ю'
            | 'я'
    )
}

fn safe_ngram_primary_lower_char(grapheme: &str) -> Option<char> {
    let ch = grapheme
        .chars()
        .find(|ch| ch.is_alphanumeric())
        .or_else(|| grapheme.chars().next())?;
    ch.to_lowercase().next().or(Some(ch))
}

fn safe_ngram_latin_base_letter(ch: char) -> Option<u8> {
    if ch.is_ascii_alphabetic() {
        return Some(ch.to_ascii_lowercase() as u8);
    }
    match ch {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'ǎ' | 'ǟ' | 'ǡ' => {
            Some(b'a')
        }
        'æ' => Some(b'a'),
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => Some(b'c'),
        'ď' | 'đ' => Some(b'd'),
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => Some(b'e'),
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => Some(b'g'),
        'ĥ' | 'ħ' => Some(b'h'),
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => Some(b'i'),
        'ĵ' => Some(b'j'),
        'ķ' => Some(b'k'),
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => Some(b'l'),
        'ñ' | 'ń' | 'ņ' | 'ň' => Some(b'n'),
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'ơ' => Some(b'o'),
        'œ' => Some(b'o'),
        'ŕ' | 'ŗ' | 'ř' => Some(b'r'),
        'ś' | 'ŝ' | 'ş' | 'š' | 'ß' => Some(b's'),
        'ţ' | 'ť' | 'ŧ' => Some(b't'),
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'ư' => Some(b'u'),
        'ŵ' => Some(b'w'),
        'ý' | 'ÿ' | 'ŷ' => Some(b'y'),
        'ź' | 'ż' | 'ž' => Some(b'z'),
        _ => None,
    }
}

fn safe_ngram_unicode_is_vowel(ch: char) -> bool {
    if let Some(base) = safe_ngram_latin_base_letter(ch) {
        return matches!(base, b'a' | b'e' | b'i' | b'o' | b'u');
    }
    matches!(
        ch,
        'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я' | 'і' | 'ї' | 'є' | 'ӧ' | 'ӱ'
    )
}

fn load_cmudict_syllable_counts(path: &Path) -> Result<HashMap<String, u8>> {
    let file = File::open(path)
        .with_context(|| format!("open CMU pronunciation dictionary {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut counts = HashMap::<String, u8>::new();
    for line in reader.lines() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        let line = line.trim();
        if line.is_empty() || line.starts_with(";;;") {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(raw_word) = parts.next() else {
            continue;
        };
        let word = raw_word
            .split_once('(')
            .map_or(raw_word, |(base, _)| base)
            .to_ascii_lowercase();
        if !word.chars().all(|ch| ch.is_ascii_alphabetic()) {
            continue;
        }
        let syllables = parts
            .filter(|phone| phone.as_bytes().last().is_some_and(u8::is_ascii_digit))
            .count()
            .min(u8::MAX as usize) as u8;
        if syllables > 0 {
            counts.entry(word).or_insert(syllables);
        }
    }
    Ok(counts)
}

fn safe_ngram_wilson_lower_ppm(positive: u32, negative: u32) -> f64 {
    let total = positive.saturating_add(negative);
    if total == 0 {
        return 0.0;
    }
    let total = f64::from(total);
    let phat = f64::from(positive) / total;
    let z = 1.959963984540054;
    let z2 = z * z;
    let denominator = 1.0 + z2 / total;
    let center = phat + z2 / (2.0 * total);
    let margin = z * ((phat * (1.0 - phat) + z2 / (4.0 * total)) / total).sqrt();
    ((center - margin) / denominator).max(0.0) * 1_000_000.0
}

fn safe_ngram_key(bytes: &[u8], boundary: usize, spec_idx: usize, spec: SafeNgramSpec) -> u64 {
    match spec.family {
        1 => safe_ngram_key_with(bytes, boundary, spec_idx, spec, safe_ngram_cv_code_at),
        2 => safe_ngram_key_with(bytes, boundary, spec_idx, spec, safe_ngram_sonority_code_at),
        _ => safe_ngram_key_with(bytes, boundary, spec_idx, spec, safe_ngram_raw_code_at),
    }
}

fn safe_ngram_hyphenate_single_spec<F>(
    bytes: &[u8],
    config: &HyphenationConfig,
    spec: SafeNgramSpec,
    rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
    code_at: F,
) where
    F: Fn(&[u8], isize) -> u64,
{
    debug_assert!(!spec.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width = spec.left + spec.right;
    debug_assert!(width > 0 && width <= 10);
    let last_shift = ((width - 1) * 5) as u32;
    let mut key = safe_ngram_key_with(bytes, start, 0, spec, &code_at);
    for boundary in start..=end {
        if rules.contains(&key) {
            out.push(boundary as GraphemeIndex);
        }
        let next_code_position = boundary as isize + 1 + spec.right as isize;
        key = (key >> 5) | (code_at(bytes, next_code_position) << last_shift);
    }
}

fn safe_ngram_hyphenate_single_spec_lookup<F>(
    bytes: &[u8],
    config: &HyphenationConfig,
    spec: SafeNgramSpec,
    rules: SafeNgramRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
    code_at: F,
) where
    F: Fn(&[u8], isize) -> u64,
{
    if let SafeNgramRuleLookup::Hash(rules) = rules {
        safe_ngram_hyphenate_single_spec(bytes, config, spec, rules, out, code_at);
        return;
    }

    debug_assert!(!spec.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width = spec.left + spec.right;
    debug_assert!(width > 0 && width <= 10);
    let last_shift = ((width - 1) * 5) as u32;
    let mut key = safe_ngram_key_with(bytes, start, 0, spec, &code_at);
    for boundary in start..=end {
        if rules.contains(key) {
            out.push(boundary as GraphemeIndex);
        }
        let next_code_position = boundary as isize + 1 + spec.right as isize;
        key = (key >> 5) | (code_at(bytes, next_code_position) << last_shift);
    }
}

fn safe_ngram_hyphenate_single_add_veto<F, G>(
    bytes: &[u8],
    config: &HyphenationConfig,
    add_spec: SafeNgramSpec,
    add_rules: &U64HashSet,
    veto_spec: SafeNgramSpec,
    veto_rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
    add_code_at: F,
    veto_code_at: G,
) where
    F: Fn(&[u8], isize) -> u64,
    G: Fn(&[u8], isize) -> u64,
{
    debug_assert!(!add_spec.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width = add_spec.left + add_spec.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width > 0 && add_width <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift = ((add_width - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key = safe_ngram_key_with(bytes, start, 0, add_spec, &add_code_at);
    let mut veto_key = safe_ngram_key_with(bytes, start, 0, veto_spec, &veto_code_at);
    for boundary in start..=end {
        if add_rules.contains(&add_key) && !veto_rules.contains(&veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next_code_position = boundary as isize + 1 + add_spec.right as isize;
        let veto_next_code_position = boundary as isize + 1 + veto_spec.right as isize;
        add_key = (add_key >> 5) | (add_code_at(bytes, add_next_code_position) << add_last_shift);
        veto_key =
            (veto_key >> 5) | (veto_code_at(bytes, veto_next_code_position) << veto_last_shift);
    }
}

fn safe_ngram_hyphenate_single_add_veto_lookup<F, G>(
    bytes: &[u8],
    config: &HyphenationConfig,
    add_spec: SafeNgramSpec,
    add_rules: SafeNgramRuleLookup<'_>,
    veto_spec: SafeNgramSpec,
    veto_rules: SafeNgramRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
    add_code_at: F,
    veto_code_at: G,
) where
    F: Fn(&[u8], isize) -> u64,
    G: Fn(&[u8], isize) -> u64,
{
    if let (SafeNgramRuleLookup::Hash(add_rules), SafeNgramRuleLookup::Hash(veto_rules)) =
        (add_rules, veto_rules)
    {
        safe_ngram_hyphenate_single_add_veto(
            bytes,
            config,
            add_spec,
            add_rules,
            veto_spec,
            veto_rules,
            out,
            add_code_at,
            veto_code_at,
        );
        return;
    }

    debug_assert!(!add_spec.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width = add_spec.left + add_spec.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width > 0 && add_width <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift = ((add_width - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key = safe_ngram_key_with(bytes, start, 0, add_spec, &add_code_at);
    let mut veto_key = safe_ngram_key_with(bytes, start, 0, veto_spec, &veto_code_at);
    for boundary in start..=end {
        if add_rules.contains(add_key) && !veto_rules.contains(veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next_code_position = boundary as isize + 1 + add_spec.right as isize;
        let veto_next_code_position = boundary as isize + 1 + veto_spec.right as isize;
        add_key = (add_key >> 5) | (add_code_at(bytes, add_next_code_position) << add_last_shift);
        veto_key =
            (veto_key >> 5) | (veto_code_at(bytes, veto_next_code_position) << veto_last_shift);
    }
}

type SafeNgramByteCodeAt = fn(&[u8], isize) -> u64;

fn safe_ngram_byte_code_at_for_family(family: u8) -> SafeNgramByteCodeAt {
    match family {
        1 => safe_ngram_cv_code_at,
        2 => safe_ngram_sonority_code_at,
        _ => safe_ngram_raw_code_at,
    }
}

fn safe_ngram_hyphenate_dual_spec(
    bytes: &[u8],
    config: &HyphenationConfig,
    spec0: SafeNgramSpec,
    spec1: SafeNgramSpec,
    rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!spec0.bucketed);
    debug_assert!(!spec1.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let code0 = safe_ngram_byte_code_at_for_family(spec0.family);
    let code1 = safe_ngram_byte_code_at_for_family(spec1.family);
    let width0 = spec0.left + spec0.right;
    let width1 = spec1.left + spec1.right;
    debug_assert!(width0 > 0 && width0 <= 10);
    debug_assert!(width1 > 0 && width1 <= 10);
    let last_shift0 = ((width0 - 1) * 5) as u32;
    let last_shift1 = ((width1 - 1) * 5) as u32;
    let mut key0 = safe_ngram_key_with(bytes, start, 0, spec0, code0);
    let mut key1 = safe_ngram_key_with(bytes, start, 0, spec1, code1);
    const SPEC1_PREFIX: u64 = 1u64 << 56;
    for boundary in start..=end {
        if rules.contains(&key0) || rules.contains(&(SPEC1_PREFIX | key1)) {
            out.push(boundary as GraphemeIndex);
        }
        let next0 = boundary as isize + 1 + spec0.right as isize;
        let next1 = boundary as isize + 1 + spec1.right as isize;
        key0 = (key0 >> 5) | (code0(bytes, next0) << last_shift0);
        key1 = (key1 >> 5) | (code1(bytes, next1) << last_shift1);
    }
}

fn safe_ngram_hyphenate_dual_spec_lookup(
    bytes: &[u8],
    config: &HyphenationConfig,
    spec0: SafeNgramSpec,
    spec1: SafeNgramSpec,
    rules: SafeNgramDualRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let SafeNgramDualRuleLookup::Hash(rules) = rules {
        safe_ngram_hyphenate_dual_spec(bytes, config, spec0, spec1, rules, out);
        return;
    }

    debug_assert!(!spec0.bucketed);
    debug_assert!(!spec1.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let code0 = safe_ngram_byte_code_at_for_family(spec0.family);
    let code1 = safe_ngram_byte_code_at_for_family(spec1.family);
    let width0 = spec0.left + spec0.right;
    let width1 = spec1.left + spec1.right;
    debug_assert!(width0 > 0 && width0 <= 10);
    debug_assert!(width1 > 0 && width1 <= 10);
    let last_shift0 = ((width0 - 1) * 5) as u32;
    let last_shift1 = ((width1 - 1) * 5) as u32;
    let mut key0 = safe_ngram_key_with(bytes, start, 0, spec0, code0);
    let mut key1 = safe_ngram_key_with(bytes, start, 0, spec1, code1);
    for boundary in start..=end {
        if rules.contains(key0, key1) {
            out.push(boundary as GraphemeIndex);
        }
        let next0 = boundary as isize + 1 + spec0.right as isize;
        let next1 = boundary as isize + 1 + spec1.right as isize;
        key0 = (key0 >> 5) | (code0(bytes, next0) << last_shift0);
        key1 = (key1 >> 5) | (code1(bytes, next1) << last_shift1);
    }
}

fn safe_ngram_hyphenate_dual_add_veto(
    bytes: &[u8],
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: &U64HashSet,
    veto_spec0: SafeNgramSpec,
    veto_spec1: SafeNgramSpec,
    veto_rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec0.bucketed);
    debug_assert!(!veto_spec1.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_code0 = safe_ngram_byte_code_at_for_family(add_spec0.family);
    let add_code1 = safe_ngram_byte_code_at_for_family(add_spec1.family);
    let veto_code0 = safe_ngram_byte_code_at_for_family(veto_spec0.family);
    let veto_code1 = safe_ngram_byte_code_at_for_family(veto_spec1.family);
    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width0 = veto_spec0.left + veto_spec0.right;
    let veto_width1 = veto_spec1.left + veto_spec1.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width0 > 0 && veto_width0 <= 10);
    debug_assert!(veto_width1 > 0 && veto_width1 <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift0 = ((veto_width0 - 1) * 5) as u32;
    let veto_last_shift1 = ((veto_width1 - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_key_with(bytes, start, 0, add_spec0, add_code0);
    let mut add_key1 = safe_ngram_key_with(bytes, start, 0, add_spec1, add_code1);
    let mut veto_key0 = safe_ngram_key_with(bytes, start, 0, veto_spec0, veto_code0);
    let mut veto_key1 = safe_ngram_key_with(bytes, start, 0, veto_spec1, veto_code1);
    const SPEC1_PREFIX: u64 = 1u64 << 56;
    for boundary in start..=end {
        let add_hit =
            add_rules.contains(&add_key0) || add_rules.contains(&(SPEC1_PREFIX | add_key1));
        if add_hit {
            let veto_hit =
                veto_rules.contains(&veto_key0) || veto_rules.contains(&(SPEC1_PREFIX | veto_key1));
            if !veto_hit {
                out.push(boundary as GraphemeIndex);
            }
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next0 = boundary as isize + 1 + veto_spec0.right as isize;
        let veto_next1 = boundary as isize + 1 + veto_spec1.right as isize;
        add_key0 = (add_key0 >> 5) | (add_code0(bytes, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5) | (add_code1(bytes, add_next1) << add_last_shift1);
        veto_key0 = (veto_key0 >> 5) | (veto_code0(bytes, veto_next0) << veto_last_shift0);
        veto_key1 = (veto_key1 >> 5) | (veto_code1(bytes, veto_next1) << veto_last_shift1);
    }
}

fn safe_ngram_hyphenate_dual_add_veto_lookup(
    bytes: &[u8],
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: SafeNgramDualRuleLookup<'_>,
    veto_spec0: SafeNgramSpec,
    veto_spec1: SafeNgramSpec,
    veto_rules: SafeNgramDualRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let (SafeNgramDualRuleLookup::Hash(add_rules), SafeNgramDualRuleLookup::Hash(veto_rules)) =
        (add_rules, veto_rules)
    {
        safe_ngram_hyphenate_dual_add_veto(
            bytes, config, add_spec0, add_spec1, add_rules, veto_spec0, veto_spec1, veto_rules, out,
        );
        return;
    }

    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec0.bucketed);
    debug_assert!(!veto_spec1.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_code0 = safe_ngram_byte_code_at_for_family(add_spec0.family);
    let add_code1 = safe_ngram_byte_code_at_for_family(add_spec1.family);
    let veto_code0 = safe_ngram_byte_code_at_for_family(veto_spec0.family);
    let veto_code1 = safe_ngram_byte_code_at_for_family(veto_spec1.family);
    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width0 = veto_spec0.left + veto_spec0.right;
    let veto_width1 = veto_spec1.left + veto_spec1.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width0 > 0 && veto_width0 <= 10);
    debug_assert!(veto_width1 > 0 && veto_width1 <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift0 = ((veto_width0 - 1) * 5) as u32;
    let veto_last_shift1 = ((veto_width1 - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_key_with(bytes, start, 0, add_spec0, add_code0);
    let mut add_key1 = safe_ngram_key_with(bytes, start, 0, add_spec1, add_code1);
    let mut veto_key0 = safe_ngram_key_with(bytes, start, 0, veto_spec0, veto_code0);
    let mut veto_key1 = safe_ngram_key_with(bytes, start, 0, veto_spec1, veto_code1);
    for boundary in start..=end {
        if add_rules.contains(add_key0, add_key1) && !veto_rules.contains(veto_key0, veto_key1) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next0 = boundary as isize + 1 + veto_spec0.right as isize;
        let veto_next1 = boundary as isize + 1 + veto_spec1.right as isize;
        add_key0 = (add_key0 >> 5) | (add_code0(bytes, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5) | (add_code1(bytes, add_next1) << add_last_shift1);
        veto_key0 = (veto_key0 >> 5) | (veto_code0(bytes, veto_next0) << veto_last_shift0);
        veto_key1 = (veto_key1 >> 5) | (veto_code1(bytes, veto_next1) << veto_last_shift1);
    }
}

fn safe_ngram_hyphenate_dual_add_single_veto(
    bytes: &[u8],
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: &U64HashSet,
    veto_spec: SafeNgramSpec,
    veto_rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_code0 = safe_ngram_byte_code_at_for_family(add_spec0.family);
    let add_code1 = safe_ngram_byte_code_at_for_family(add_spec1.family);
    let veto_code = safe_ngram_byte_code_at_for_family(veto_spec.family);
    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_key_with(bytes, start, 0, add_spec0, add_code0);
    let mut add_key1 = safe_ngram_key_with(bytes, start, 0, add_spec1, add_code1);
    let mut veto_key = safe_ngram_key_with(bytes, start, 0, veto_spec, veto_code);
    const SPEC1_PREFIX: u64 = 1u64 << 56;
    for boundary in start..=end {
        let add_hit =
            add_rules.contains(&add_key0) || add_rules.contains(&(SPEC1_PREFIX | add_key1));
        if add_hit && !veto_rules.contains(&veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next = boundary as isize + 1 + veto_spec.right as isize;
        add_key0 = (add_key0 >> 5) | (add_code0(bytes, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5) | (add_code1(bytes, add_next1) << add_last_shift1);
        veto_key = (veto_key >> 5) | (veto_code(bytes, veto_next) << veto_last_shift);
    }
}

fn safe_ngram_hyphenate_dual_add_single_veto_lookup(
    bytes: &[u8],
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: SafeNgramDualRuleLookup<'_>,
    veto_spec: SafeNgramSpec,
    veto_rules: SafeNgramRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let (SafeNgramDualRuleLookup::Hash(add_rules), SafeNgramRuleLookup::Hash(veto_rules)) =
        (add_rules, veto_rules)
    {
        safe_ngram_hyphenate_dual_add_single_veto(
            bytes, config, add_spec0, add_spec1, add_rules, veto_spec, veto_rules, out,
        );
        return;
    }

    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = bytes.len().saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_code0 = safe_ngram_byte_code_at_for_family(add_spec0.family);
    let add_code1 = safe_ngram_byte_code_at_for_family(add_spec1.family);
    let veto_code = safe_ngram_byte_code_at_for_family(veto_spec.family);
    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_key_with(bytes, start, 0, add_spec0, add_code0);
    let mut add_key1 = safe_ngram_key_with(bytes, start, 0, add_spec1, add_code1);
    let mut veto_key = safe_ngram_key_with(bytes, start, 0, veto_spec, veto_code);
    for boundary in start..=end {
        if add_rules.contains(add_key0, add_key1) && !veto_rules.contains(veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next = boundary as isize + 1 + veto_spec.right as isize;
        add_key0 = (add_key0 >> 5) | (add_code0(bytes, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5) | (add_code1(bytes, add_next1) << add_last_shift1);
        veto_key = (veto_key >> 5) | (veto_code(bytes, veto_next) << veto_last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_single_spec(
    codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    spec: SafeNgramSpec,
    rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width = spec.left + spec.right;
    debug_assert!(width > 0 && width <= 10);
    let last_shift = ((width - 1) * 5) as u32;
    let mut key = safe_ngram_grapheme_key_from_codes(codes, start, spec);
    for boundary in start..=end {
        if rules.contains(&key) {
            out.push(boundary as GraphemeIndex);
        }
        let next_code_position = boundary as isize + 1 + spec.right as isize;
        key = (key >> 5) | (safe_ngram_grapheme_code_at(codes, next_code_position) << last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_single_spec_lookup(
    codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    spec: SafeNgramSpec,
    rules: SafeNgramRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let SafeNgramRuleLookup::Hash(rules) = rules {
        safe_ngram_hyphenate_grapheme_single_spec(codes, grapheme_len, config, spec, rules, out);
        return;
    }

    debug_assert!(!spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width = spec.left + spec.right;
    debug_assert!(width > 0 && width <= 10);
    let last_shift = ((width - 1) * 5) as u32;
    let mut key = safe_ngram_grapheme_key_from_codes(codes, start, spec);
    for boundary in start..=end {
        if rules.contains(key) {
            out.push(boundary as GraphemeIndex);
        }
        let next_code_position = boundary as isize + 1 + spec.right as isize;
        key = (key >> 5) | (safe_ngram_grapheme_code_at(codes, next_code_position) << last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_single_add_veto(
    add_codes: &[u8],
    veto_codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec: SafeNgramSpec,
    add_rules: &U64HashSet,
    veto_spec: SafeNgramSpec,
    veto_rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!add_spec.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width = add_spec.left + add_spec.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width > 0 && add_width <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift = ((add_width - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key = safe_ngram_grapheme_key_from_codes(add_codes, start, add_spec);
    let mut veto_key = safe_ngram_grapheme_key_from_codes(veto_codes, start, veto_spec);
    for boundary in start..=end {
        if add_rules.contains(&add_key) && !veto_rules.contains(&veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next_code_position = boundary as isize + 1 + add_spec.right as isize;
        let veto_next_code_position = boundary as isize + 1 + veto_spec.right as isize;
        add_key = (add_key >> 5)
            | (safe_ngram_grapheme_code_at(add_codes, add_next_code_position) << add_last_shift);
        veto_key = (veto_key >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes, veto_next_code_position) << veto_last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_single_add_veto_lookup(
    add_codes: &[u8],
    veto_codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec: SafeNgramSpec,
    add_rules: SafeNgramRuleLookup<'_>,
    veto_spec: SafeNgramSpec,
    veto_rules: SafeNgramRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let (SafeNgramRuleLookup::Hash(add_rules), SafeNgramRuleLookup::Hash(veto_rules)) =
        (add_rules, veto_rules)
    {
        safe_ngram_hyphenate_grapheme_single_add_veto(
            add_codes,
            veto_codes,
            grapheme_len,
            config,
            add_spec,
            add_rules,
            veto_spec,
            veto_rules,
            out,
        );
        return;
    }

    debug_assert!(!add_spec.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width = add_spec.left + add_spec.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width > 0 && add_width <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift = ((add_width - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key = safe_ngram_grapheme_key_from_codes(add_codes, start, add_spec);
    let mut veto_key = safe_ngram_grapheme_key_from_codes(veto_codes, start, veto_spec);
    for boundary in start..=end {
        if add_rules.contains(add_key) && !veto_rules.contains(veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next_code_position = boundary as isize + 1 + add_spec.right as isize;
        let veto_next_code_position = boundary as isize + 1 + veto_spec.right as isize;
        add_key = (add_key >> 5)
            | (safe_ngram_grapheme_code_at(add_codes, add_next_code_position) << add_last_shift);
        veto_key = (veto_key >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes, veto_next_code_position) << veto_last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_spec(
    codes0: &[u8],
    codes1: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    spec0: SafeNgramSpec,
    spec1: SafeNgramSpec,
    rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!spec0.bucketed);
    debug_assert!(!spec1.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width0 = spec0.left + spec0.right;
    let width1 = spec1.left + spec1.right;
    debug_assert!(width0 > 0 && width0 <= 10);
    debug_assert!(width1 > 0 && width1 <= 10);
    let last_shift0 = ((width0 - 1) * 5) as u32;
    let last_shift1 = ((width1 - 1) * 5) as u32;
    let mut key0 = safe_ngram_grapheme_key_from_codes(codes0, start, spec0);
    let mut key1 = safe_ngram_grapheme_key_from_codes(codes1, start, spec1);
    const SPEC1_PREFIX: u64 = 1u64 << 56;
    for boundary in start..=end {
        if rules.contains(&key0) || rules.contains(&(SPEC1_PREFIX | key1)) {
            out.push(boundary as GraphemeIndex);
        }
        let next0 = boundary as isize + 1 + spec0.right as isize;
        let next1 = boundary as isize + 1 + spec1.right as isize;
        key0 = (key0 >> 5) | (safe_ngram_grapheme_code_at(codes0, next0) << last_shift0);
        key1 = (key1 >> 5) | (safe_ngram_grapheme_code_at(codes1, next1) << last_shift1);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_spec_lookup(
    codes0: &[u8],
    codes1: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    spec0: SafeNgramSpec,
    spec1: SafeNgramSpec,
    rules: SafeNgramDualRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let SafeNgramDualRuleLookup::Hash(rules) = rules {
        safe_ngram_hyphenate_grapheme_dual_spec(
            codes0,
            codes1,
            grapheme_len,
            config,
            spec0,
            spec1,
            rules,
            out,
        );
        return;
    }

    debug_assert!(!spec0.bucketed);
    debug_assert!(!spec1.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width0 = spec0.left + spec0.right;
    let width1 = spec1.left + spec1.right;
    debug_assert!(width0 > 0 && width0 <= 10);
    debug_assert!(width1 > 0 && width1 <= 10);
    let last_shift0 = ((width0 - 1) * 5) as u32;
    let last_shift1 = ((width1 - 1) * 5) as u32;
    let mut key0 = safe_ngram_grapheme_key_from_codes(codes0, start, spec0);
    let mut key1 = safe_ngram_grapheme_key_from_codes(codes1, start, spec1);
    for boundary in start..=end {
        if rules.contains(key0, key1) {
            out.push(boundary as GraphemeIndex);
        }
        let next0 = boundary as isize + 1 + spec0.right as isize;
        let next1 = boundary as isize + 1 + spec1.right as isize;
        key0 = (key0 >> 5) | (safe_ngram_grapheme_code_at(codes0, next0) << last_shift0);
        key1 = (key1 >> 5) | (safe_ngram_grapheme_code_at(codes1, next1) << last_shift1);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_add_veto(
    add_codes0: &[u8],
    add_codes1: &[u8],
    veto_codes0: &[u8],
    veto_codes1: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: &U64HashSet,
    veto_spec0: SafeNgramSpec,
    veto_spec1: SafeNgramSpec,
    veto_rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec0.bucketed);
    debug_assert!(!veto_spec1.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width0 = veto_spec0.left + veto_spec0.right;
    let veto_width1 = veto_spec1.left + veto_spec1.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width0 > 0 && veto_width0 <= 10);
    debug_assert!(veto_width1 > 0 && veto_width1 <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift0 = ((veto_width0 - 1) * 5) as u32;
    let veto_last_shift1 = ((veto_width1 - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_grapheme_key_from_codes(add_codes0, start, add_spec0);
    let mut add_key1 = safe_ngram_grapheme_key_from_codes(add_codes1, start, add_spec1);
    let mut veto_key0 = safe_ngram_grapheme_key_from_codes(veto_codes0, start, veto_spec0);
    let mut veto_key1 = safe_ngram_grapheme_key_from_codes(veto_codes1, start, veto_spec1);
    const SPEC1_PREFIX: u64 = 1u64 << 56;
    for boundary in start..=end {
        let add_hit =
            add_rules.contains(&add_key0) || add_rules.contains(&(SPEC1_PREFIX | add_key1));
        if add_hit {
            let veto_hit =
                veto_rules.contains(&veto_key0) || veto_rules.contains(&(SPEC1_PREFIX | veto_key1));
            if !veto_hit {
                out.push(boundary as GraphemeIndex);
            }
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next0 = boundary as isize + 1 + veto_spec0.right as isize;
        let veto_next1 = boundary as isize + 1 + veto_spec1.right as isize;
        add_key0 = (add_key0 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes0, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes1, add_next1) << add_last_shift1);
        veto_key0 = (veto_key0 >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes0, veto_next0) << veto_last_shift0);
        veto_key1 = (veto_key1 >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes1, veto_next1) << veto_last_shift1);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_add_veto_lookup(
    add_codes0: &[u8],
    add_codes1: &[u8],
    veto_codes0: &[u8],
    veto_codes1: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: SafeNgramDualRuleLookup<'_>,
    veto_spec0: SafeNgramSpec,
    veto_spec1: SafeNgramSpec,
    veto_rules: SafeNgramDualRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let (SafeNgramDualRuleLookup::Hash(add_rules), SafeNgramDualRuleLookup::Hash(veto_rules)) =
        (add_rules, veto_rules)
    {
        safe_ngram_hyphenate_grapheme_dual_add_veto(
            add_codes0,
            add_codes1,
            veto_codes0,
            veto_codes1,
            grapheme_len,
            config,
            add_spec0,
            add_spec1,
            add_rules,
            veto_spec0,
            veto_spec1,
            veto_rules,
            out,
        );
        return;
    }

    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec0.bucketed);
    debug_assert!(!veto_spec1.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width0 = veto_spec0.left + veto_spec0.right;
    let veto_width1 = veto_spec1.left + veto_spec1.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width0 > 0 && veto_width0 <= 10);
    debug_assert!(veto_width1 > 0 && veto_width1 <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift0 = ((veto_width0 - 1) * 5) as u32;
    let veto_last_shift1 = ((veto_width1 - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_grapheme_key_from_codes(add_codes0, start, add_spec0);
    let mut add_key1 = safe_ngram_grapheme_key_from_codes(add_codes1, start, add_spec1);
    let mut veto_key0 = safe_ngram_grapheme_key_from_codes(veto_codes0, start, veto_spec0);
    let mut veto_key1 = safe_ngram_grapheme_key_from_codes(veto_codes1, start, veto_spec1);
    for boundary in start..=end {
        if add_rules.contains(add_key0, add_key1) && !veto_rules.contains(veto_key0, veto_key1) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next0 = boundary as isize + 1 + veto_spec0.right as isize;
        let veto_next1 = boundary as isize + 1 + veto_spec1.right as isize;
        add_key0 = (add_key0 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes0, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes1, add_next1) << add_last_shift1);
        veto_key0 = (veto_key0 >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes0, veto_next0) << veto_last_shift0);
        veto_key1 = (veto_key1 >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes1, veto_next1) << veto_last_shift1);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_add_single_veto(
    add_codes0: &[u8],
    add_codes1: &[u8],
    veto_codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: &U64HashSet,
    veto_spec: SafeNgramSpec,
    veto_rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_grapheme_key_from_codes(add_codes0, start, add_spec0);
    let mut add_key1 = safe_ngram_grapheme_key_from_codes(add_codes1, start, add_spec1);
    let mut veto_key = safe_ngram_grapheme_key_from_codes(veto_codes, start, veto_spec);
    const SPEC1_PREFIX: u64 = 1u64 << 56;
    for boundary in start..=end {
        let add_hit =
            add_rules.contains(&add_key0) || add_rules.contains(&(SPEC1_PREFIX | add_key1));
        if add_hit && !veto_rules.contains(&veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next = boundary as isize + 1 + veto_spec.right as isize;
        add_key0 = (add_key0 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes0, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes1, add_next1) << add_last_shift1);
        veto_key = (veto_key >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes, veto_next) << veto_last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_add_single_veto_lookup(
    add_codes0: &[u8],
    add_codes1: &[u8],
    veto_codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: SafeNgramDualRuleLookup<'_>,
    veto_spec: SafeNgramSpec,
    veto_rules: SafeNgramRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let (SafeNgramDualRuleLookup::Hash(add_rules), SafeNgramRuleLookup::Hash(veto_rules)) =
        (add_rules, veto_rules)
    {
        safe_ngram_hyphenate_grapheme_dual_add_single_veto(
            add_codes0,
            add_codes1,
            veto_codes,
            grapheme_len,
            config,
            add_spec0,
            add_spec1,
            add_rules,
            veto_spec,
            veto_rules,
            out,
        );
        return;
    }

    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_grapheme_key_from_codes(add_codes0, start, add_spec0);
    let mut add_key1 = safe_ngram_grapheme_key_from_codes(add_codes1, start, add_spec1);
    let mut veto_key = safe_ngram_grapheme_key_from_codes(veto_codes, start, veto_spec);
    for boundary in start..=end {
        if add_rules.contains(add_key0, add_key1) && !veto_rules.contains(veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next = boundary as isize + 1 + veto_spec.right as isize;
        add_key0 = (add_key0 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes0, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes1, add_next1) << add_last_shift1);
        veto_key = (veto_key >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes, veto_next) << veto_last_shift);
    }
}

fn safe_ngram_apply_orthographic_veto(bytes: &[u8], out: &mut SmallVec<[GraphemeIndex; 8]>) {
    out.retain(|boundary| safe_ngram_orthographic_break_allowed(bytes, *boundary as usize));
}

fn safe_ngram_orthographic_break_allowed(bytes: &[u8], boundary: usize) -> bool {
    if boundary == 0 || boundary >= bytes.len() {
        return false;
    }
    let left = bytes[boundary - 1].to_ascii_lowercase();
    let right = bytes[boundary].to_ascii_lowercase();

    if matches!(
        (left, right),
        (b'c', b'h')
            | (b'c', b'k')
            | (b'p', b'h')
            | (b'q', b'u')
            | (b's', b'h')
            | (b't', b'h')
            | (b'w', b'h')
    ) {
        return false;
    }

    if is_safe_ngram_vowelish(left)
        && is_safe_ngram_vowelish(right)
        && matches!(
            (left, right),
            (b'a', b'i')
                | (b'a', b'u')
                | (b'a', b'w')
                | (b'a', b'y')
                | (b'e', b'a')
                | (b'e', b'e')
                | (b'e', b'i')
                | (b'e', b'w')
                | (b'e', b'y')
                | (b'i', b'e')
                | (b'o', b'a')
                | (b'o', b'e')
                | (b'o', b'i')
                | (b'o', b'o')
                | (b'o', b'u')
                | (b'o', b'w')
                | (b'o', b'y')
                | (b'u', b'e')
                | (b'u', b'i')
                | (b'u', b'y')
        )
    {
        return false;
    }

    true
}

fn is_safe_ngram_vowelish(byte: u8) -> bool {
    matches!(byte, b'a' | b'e' | b'i' | b'o' | b'u' | b'y')
}

fn safe_ngram_key_with<F>(
    bytes: &[u8],
    boundary: usize,
    spec_idx: usize,
    spec: SafeNgramSpec,
    code_at: F,
) -> u64
where
    F: Fn(&[u8], isize) -> u64,
{
    debug_assert!(spec.left + spec.right <= 10);
    let padded_boundary = boundary as isize + 1;
    let mut key = (spec_idx as u64) << 56;
    if spec.bucketed {
        key |= safe_ngram_boundary_bucket(bytes.len(), boundary) << 50;
    }
    let mut shift = 0u32;
    for offset in 0..spec.left {
        let position = padded_boundary - spec.left as isize + offset as isize;
        key |= code_at(bytes, position) << shift;
        shift += 5;
    }
    for offset in 0..spec.right {
        let position = padded_boundary + offset as isize;
        key |= code_at(bytes, position) << shift;
        shift += 5;
    }
    key
}

fn safe_ngram_grapheme_key_from_codes(codes: &[u8], boundary: usize, spec: SafeNgramSpec) -> u64 {
    debug_assert!(spec.left + spec.right <= 10);
    let padded_boundary = boundary as isize + 1;
    let mut key = 0u64;
    let mut shift = 0u32;
    for offset in 0..spec.left {
        let position = padded_boundary - spec.left as isize + offset as isize;
        key |= safe_ngram_grapheme_code_at(codes, position) << shift;
        shift += 5;
    }
    for offset in 0..spec.right {
        let position = padded_boundary + offset as isize;
        key |= safe_ngram_grapheme_code_at(codes, position) << shift;
        shift += 5;
    }
    key
}

fn safe_ngram_boundary_bucket(byte_len: usize, boundary: usize) -> u64 {
    let right = byte_len.saturating_sub(boundary);
    let edge_bucket = if boundary <= 2 {
        0
    } else if right <= 3 {
        1
    } else if boundary <= 3 {
        2
    } else if right <= 4 {
        3
    } else {
        4
    };
    let len_bucket = if byte_len <= 6 {
        0
    } else if byte_len <= 8 {
        1
    } else if byte_len <= 11 {
        2
    } else {
        3
    };
    ((len_bucket << 3) | edge_bucket) as u64
}

fn safe_ngram_cv_code_at(bytes: &[u8], padded_position: isize) -> u64 {
    if padded_position <= 0 || padded_position > bytes.len() as isize {
        return 1;
    }
    let byte = bytes[padded_position as usize - 1].to_ascii_lowercase();
    match byte {
        b'a' | b'e' | b'i' | b'o' | b'u' => 2,
        b'y' => 3,
        b'a'..=b'z' => 4,
        b'\'' => 5,
        b'-' => 6,
        b'0'..=b'9' => 7,
        _ => 8,
    }
}

fn safe_ngram_sonority_code_at(bytes: &[u8], padded_position: isize) -> u64 {
    if padded_position <= 0 || padded_position > bytes.len() as isize {
        return 1;
    }
    let byte = bytes[padded_position as usize - 1].to_ascii_lowercase();
    match byte {
        b'a' | b'e' | b'i' | b'o' | b'u' => 2,
        b'y' => 3,
        b'l' | b'r' => 4,
        b'm' | b'n' => 5,
        b'f' | b'v' | b's' | b'z' | b'h' => 6,
        b'w' | b'j' => 7,
        b'b' | b'c' | b'd' | b'g' | b'k' | b'p' | b'q' | b't' | b'x' => 8,
        b'\'' => 9,
        b'-' => 10,
        b'0'..=b'9' => 11,
        _ => 12,
    }
}

const SAFE_NGRAM_RAW_CODES: [u64; 256] = build_safe_ngram_raw_codes();

const fn build_safe_ngram_raw_codes() -> [u64; 256] {
    let mut codes = [31u64; 256];
    let mut idx = 0usize;
    while idx < 26 {
        codes[b'a' as usize + idx] = idx as u64 + 2;
        codes[b'A' as usize + idx] = idx as u64 + 2;
        idx += 1;
    }
    codes[b'\'' as usize] = 28;
    codes[b'-' as usize] = 29;
    idx = 0;
    while idx < 10 {
        codes[b'0' as usize + idx] = 30;
        idx += 1;
    }
    codes
}

fn safe_ngram_raw_code_at(bytes: &[u8], padded_position: isize) -> u64 {
    if padded_position <= 0 || padded_position > bytes.len() as isize {
        return 1;
    }
    SAFE_NGRAM_RAW_CODES[bytes[padded_position as usize - 1] as usize]
}

impl PreparedMethod {
    fn id(&self) -> &str {
        match self {
            Self::Adapter { inner, .. } => inner.id(),
            Self::Liang(inner) => inner.id(),
            Self::Dictionary { id, .. } => id,
            Self::DictionaryFallback { id, .. } => id,
            Self::SafeNgram(inner) => inner.id(),
            Self::BoundaryBayes(inner) => inner.id(),
            Self::StackedBayes(inner) => inner.id(),
            Self::CandidateBayes(inner) => inner.id(),
            Self::CandidateGate(inner) => inner.id(),
            Self::PruneBayes(inner) => inner.id(),
            Self::MaskRerank(inner) => inner.id(),
            Self::MaskOracle(inner) => inner.id(),
            Self::MaskCost(inner) => inner.id(),
            Self::RankedUnion(inner) => inner.id(),
            Self::ItalianSyllable(inner) => inner.id(),
            Self::HypherSafeAdd(inner) => inner.id(),
            Self::BaseSafeAdd(inner) => inner.id(),
            Self::SafeLadder(inner) => inner.id(),
            Self::BaseVeto(inner) => inner.id(),
            Self::PronCountCap(inner) => inner.id(),
            Self::AffixSafeAdd(inner) => inner.id(),
            Self::AffixVeto(inner) => inner.id(),
            Self::AnalogSafeAdd(inner) => inner.id(),
            Self::IdentityOracle { .. } => "identity-oracle",
            Self::Crf(inner) => inner.id(),
            Self::Intersection { id, .. } => id,
            Self::ExternalJsonl(inner) => inner.id(),
        }
    }

    fn config(&self) -> &HyphenationConfig {
        match self {
            Self::Adapter { config, .. } => config,
            Self::Liang(inner) => inner.config(),
            Self::Dictionary { config, .. } => config,
            Self::DictionaryFallback { config, .. } => config,
            Self::SafeNgram(inner) => inner.config(),
            Self::BoundaryBayes(inner) => inner.config(),
            Self::StackedBayes(inner) => inner.config(),
            Self::CandidateBayes(inner) => inner.config(),
            Self::CandidateGate(inner) => inner.config(),
            Self::PruneBayes(inner) => inner.config(),
            Self::MaskRerank(inner) => inner.config(),
            Self::MaskOracle(inner) => inner.config(),
            Self::MaskCost(inner) => inner.config(),
            Self::RankedUnion(inner) => inner.config(),
            Self::ItalianSyllable(inner) => inner.config(),
            Self::HypherSafeAdd(inner) => inner.config(),
            Self::BaseSafeAdd(inner) => inner.config(),
            Self::SafeLadder(inner) => inner.config(),
            Self::BaseVeto(inner) => inner.config(),
            Self::PronCountCap(inner) => inner.config(),
            Self::AffixSafeAdd(inner) => inner.config(),
            Self::AffixVeto(inner) => inner.config(),
            Self::AnalogSafeAdd(inner) => inner.config(),
            Self::IdentityOracle { config } => config,
            Self::Crf(inner) => inner.config(),
            Self::Intersection { config, .. } => config,
            Self::ExternalJsonl(inner) => inner.config(),
        }
    }

    fn hyphenate_record_into(
        &self,
        record: &HyphenationRecord,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) -> Result<()> {
        match self {
            Self::IdentityOracle { .. } => {
                out.clear();
                out.extend(record.breaks.iter().copied());
                Ok(())
            }
            Self::MaskOracle(inner) => inner.hyphenate_record_into(record, out),
            _ => self.hyphenate_into(&record.word, out),
        }
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        match self {
            Self::Adapter { inner, .. } => inner.hyphenate_into(word, out),
            Self::Liang(inner) => inner.hyphenate_into(word, out),
            Self::Crf(inner) => inner.hyphenate_into(word, out),
            Self::SafeNgram(inner) => inner.hyphenate_into(word, out),
            Self::BoundaryBayes(inner) => inner.hyphenate_into(word, out),
            Self::StackedBayes(inner) => inner.hyphenate_into(word, out),
            Self::CandidateBayes(inner) => inner.hyphenate_into(word, out),
            Self::CandidateGate(inner) => inner.hyphenate_into(word, out),
            Self::PruneBayes(inner) => inner.hyphenate_into(word, out),
            Self::MaskRerank(inner) => inner.hyphenate_into(word, out),
            Self::MaskOracle(_) => {
                anyhow::bail!(
                    "mask-oracle requires an evaluation record and cannot predict plain words"
                )
            }
            Self::MaskCost(inner) => inner.hyphenate_into(word, out),
            Self::RankedUnion(inner) => inner.hyphenate_into(word, out),
            Self::ItalianSyllable(inner) => inner.hyphenate_into(word, out),
            Self::HypherSafeAdd(inner) => inner.hyphenate_into(word, out),
            Self::BaseSafeAdd(inner) => inner.hyphenate_into(word, out),
            Self::SafeLadder(inner) => inner.hyphenate_into(word, out),
            Self::BaseVeto(inner) => inner.hyphenate_into(word, out),
            Self::PronCountCap(inner) => inner.hyphenate_into(word, out),
            Self::AffixSafeAdd(inner) => inner.hyphenate_into(word, out),
            Self::AffixVeto(inner) => inner.hyphenate_into(word, out),
            Self::AnalogSafeAdd(inner) => inner.hyphenate_into(word, out),
            Self::Dictionary { entries, .. } => {
                out.clear();
                if let Some(breaks) = entries.get(word) {
                    out.extend(breaks.iter().copied());
                } else {
                    let lower = word.to_lowercase();
                    if let Some(breaks) = entries.get(&lower) {
                        out.extend(breaks.iter().copied());
                    }
                }
                Ok(())
            }
            Self::DictionaryFallback {
                entries, fallback, ..
            } => {
                out.clear();
                if let Some(breaks) = entries.get(word) {
                    out.extend(breaks.iter().copied());
                    return Ok(());
                }
                let lower = word.to_lowercase();
                if let Some(breaks) = entries.get(&lower) {
                    out.extend(breaks.iter().copied());
                    return Ok(());
                }
                fallback.hyphenate_into(word, out)
            }
            Self::IdentityOracle { .. } => {
                anyhow::bail!(
                    "identity-oracle requires an evaluation record and cannot predict plain words"
                )
            }
            Self::Intersection { first, second, .. } => {
                let mut left = SmallVec::<[GraphemeIndex; 8]>::new();
                let mut right = SmallVec::<[GraphemeIndex; 8]>::new();
                first.hyphenate_into(word, &mut left)?;
                second.hyphenate_into(word, &mut right)?;
                right.sort_unstable();
                out.clear();
                out.extend(
                    left.into_iter()
                        .filter(|idx| right.binary_search(idx).is_ok()),
                );
                out.sort_unstable();
                out.dedup();
                Ok(())
            }
            Self::ExternalJsonl(inner) => inner.hyphenate_into(word, out),
        }
    }
}

struct ExternalJsonlMethod {
    id: String,
    language: LanguageTag,
    config: HyphenationConfig,
    process: Mutex<ExternalJsonlProcess>,
}

struct ExternalJsonlProcess {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    sequence: u64,
}

impl Drop for ExternalJsonlProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl ExternalJsonlMethod {
    fn new(command: &str, locale: &str, config: HyphenationConfig) -> Result<Self> {
        let language = locale
            .parse::<LanguageTag>()
            .map_err(|err| anyhow::anyhow!("parse locale {locale:?}: {err}"))?;
        let mut child = ProcessCommand::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn external-jsonl command {command:?}"))?;
        let stdin = child
            .stdin
            .take()
            .context("external-jsonl command did not expose stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("external-jsonl command did not expose stdout")?;
        Ok(Self {
            id: format!("external-jsonl:{command}"),
            language,
            config,
            process: Mutex::new(ExternalJsonlProcess {
                child,
                stdin: BufWriter::new(stdin),
                stdout: BufReader::new(stdout),
                sequence: 0,
            }),
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        let mut process = self
            .process
            .lock()
            .map_err(|_| anyhow::anyhow!("external-jsonl process lock poisoned"))?;
        process.sequence += 1;
        let id = process.sequence.to_string();
        let input = serde_json::json!({
            "id": id,
            "word": word,
            "lang": self.language.to_string(),
        });
        serde_json::to_writer(&mut process.stdin, &input)?;
        process.stdin.write_all(b"\n")?;
        process.stdin.flush()?;

        let mut line = String::new();
        let bytes = process
            .stdout
            .read_line(&mut line)
            .with_context(|| format!("read external-jsonl response for {word:?}"))?;
        anyhow::ensure!(
            bytes > 0,
            "external-jsonl command closed stdout while processing {word:?}"
        );
        let value = serde_json::from_str::<serde_json::Value>(&line)
            .with_context(|| format!("parse external-jsonl response for {word:?}: {line:?}"))?;
        if let Some(error) = value.get("error").and_then(|value| value.as_str()) {
            anyhow::bail!("external-jsonl command returned error for {word:?}: {error}");
        }
        if let Some(response_id) = value.get("id").and_then(|value| value.as_str()) {
            anyhow::ensure!(
                response_id == id,
                "external-jsonl response id mismatch for {word:?}: expected {id}, got {response_id}"
            );
        }

        out.clear();
        if let Some(breaks) = value.get("breaks").and_then(|value| value.as_array()) {
            for item in breaks {
                let idx = item
                    .as_u64()
                    .with_context(|| format!("external-jsonl non-integer break for {word:?}"))?;
                out.push(idx.try_into().with_context(|| {
                    format!("external-jsonl break out of range for {word:?}: {idx}")
                })?);
            }
        } else if let Some(hyphenated) = value.get("hyphenated").and_then(|value| value.as_str()) {
            out.extend(hyphenated_to_breaks(word, hyphenated)?);
        } else {
            anyhow::bail!(
                "external-jsonl response for {word:?} must contain `breaks` or `hyphenated`"
            );
        }
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

fn prepare_method(options: MethodOptions) -> Result<PreparedMethod> {
    let method = options.method.to_ascii_lowercase();
    match method.as_str() {
        "identity-oracle" | "record-oracle" => prepare_identity_oracle(options),
        "liang" | "patterns" | "tex" => prepare_liang(options),
        "dict" | "dictionary" | "lookup" => prepare_dictionary(options),
        "dict-fallback-safe-ngram-model" | "dictionary-fallback-safe-ngram-model" => {
            prepare_dictionary_fallback_safe_ngram_model(options)
        }
        method if method.starts_with("hypher-safe-add") => prepare_hypher_safe_add(options),
        method
            if method.starts_with("liang-safe-add")
                || method.starts_with("patterns-safe-add")
                || method.starts_with("tex-safe-add") =>
        {
            prepare_liang_safe_add(options)
        }
        method if method.starts_with("safe-ladder") || method.starts_with("safe-residual") => {
            prepare_safe_ladder(options)
        }
        method
            if method.starts_with("liang-veto")
                || method.starts_with("patterns-veto")
                || method.starts_with("tex-veto") =>
        {
            prepare_liang_veto(options)
        }
        method
            if method.starts_with("residual-safe-add") || method.starts_with("hybrid-safe-add") =>
        {
            prepare_residual_safe_add(options)
        }
        method if method.starts_with("affix-safe-add") || method.starts_with("morph-safe-add") => {
            prepare_affix_safe_add(options)
        }
        method if method.starts_with("affix-veto") || method.starts_with("morph-veto") => {
            prepare_affix_veto(options)
        }
        method
            if method.starts_with("analog-safe-add") || method.starts_with("analogy-safe-add") =>
        {
            prepare_analog_safe_add(options)
        }
        method
            if method.starts_with("pron-count-cap")
                || method.starts_with("pron-count-fill")
                || method.starts_with("syllable-count-cap") =>
        {
            prepare_pron_count_cap(options)
        }
        method if method.starts_with("boundary-bayes") => prepare_boundary_bayes(options),
        method if method.starts_with("stacked-bayes") => prepare_stacked_bayes(options),
        method if method.starts_with("stacked-logit") || method.starts_with("stacked-logistic") => {
            prepare_stacked_logit(options)
        }
        method if method.starts_with("candidate-bayes") => prepare_candidate_bayes(options),
        method
            if method.starts_with("candidate-logit")
                || method.starts_with("candidate-logistic") =>
        {
            prepare_candidate_logit(options)
        }
        method if method.starts_with("candidate-gate") => prepare_candidate_gate(options),
        method if method.starts_with("prune-bayes") || method.starts_with("base-prune-bayes") => {
            prepare_prune_bayes(options)
        }
        method
            if method.starts_with("mask-rerank")
                || method.starts_with("word-rerank")
                || method.starts_with("listwise-rerank") =>
        {
            prepare_mask_rerank(options)
        }
        method if method.starts_with("mask-oracle") || method.starts_with("mask-upper-bound") => {
            prepare_mask_oracle(options)
        }
        method if method.starts_with("mask-cost") || method.starts_with("word-cost") => {
            prepare_mask_cost(options)
        }
        method if method.starts_with("ranked-union") || method.starts_with("ranked-hybrid") => {
            prepare_ranked_union(options)
        }
        "italian-syllable-model"
        | "it-syllable-model"
        | "italian-onset-model"
        | "it-onset-model" => prepare_italian_syllable_model(options),
        method
            if method.starts_with("italian-syllable")
                || method.starts_with("it-syllable")
                || method.starts_with("italian-onset")
                || method.starts_with("it-onset") =>
        {
            prepare_italian_syllable(options)
        }
        "safe-ngram-model" => prepare_safe_ngram_model(options),
        method if method.starts_with("safe-ngram") => prepare_safe_ngram(options),
        "trogkanis-elkan-crf" => prepare_crf(options),
        "hyphenation-runtime" | "hyphenation-standard-runtime" | "hyphenation-file" => {
            prepare_hyphenation_runtime(options)
        }
        "hyphenation-extended-runtime" | "hyphenation-extended" => {
            prepare_hyphenation_extended_runtime(options)
        }
        "hypher-liang-consensus" | "consensus" => prepare_hypher_liang_consensus(options),
        "external-jsonl" | "external" | "subprocess" => prepare_external_jsonl(options),
        _ => {
            let adapter = adapter_for_method(&options.method, &options.locale)?;
            let mut config = adapter.config().clone();
            apply_config_overrides(&mut config, &options);
            Ok(PreparedMethod::Adapter {
                inner: adapter,
                config,
            })
        }
    }
}

fn prepare_identity_oracle(options: MethodOptions) -> Result<PreparedMethod> {
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::IdentityOracle { config })
}

fn prepare_crf(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options.dictionary.as_ref().context(
        "--dictionary is required as the CRF model path for --method trogkanis-elkan-crf",
    )?;
    let model = CrfHyphenator::load(path)?;
    let mut config = model.config().clone();
    apply_config_overrides(&mut config, &options);
    if config != *model.config() {
        anyhow::bail!(
            "CRF config overrides are not supported at load time; train a model with the desired config"
        );
    }
    Ok(PreparedMethod::Crf(model))
}

#[cfg(feature = "adapters-hyphenation")]
fn prepare_hyphenation_runtime(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required for --method hyphenation-runtime")?;
    let adapter = HyphenationCrateAdapter::from_path(&options.locale, path)?;
    let mut config = adapter.config().clone();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::Adapter {
        inner: Box::new(adapter),
        config,
    })
}

#[cfg(feature = "adapters-hyphenation")]
fn prepare_hyphenation_extended_runtime(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required for --method hyphenation-extended-runtime")?;
    let adapter = HyphenationCrateAdapter::from_extended_path(&options.locale, path)?;
    let mut config = adapter.config().clone();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::Adapter {
        inner: Box::new(adapter),
        config,
    })
}

#[cfg(not(feature = "adapters-hyphenation"))]
fn prepare_hyphenation_runtime(_options: MethodOptions) -> Result<PreparedMethod> {
    anyhow::bail!(
        "method `hyphenation-runtime` requires feature `adapters-hyphenation` or `adapters-hyphenation-embedded`"
    )
}

#[cfg(not(feature = "adapters-hyphenation"))]
fn prepare_hyphenation_extended_runtime(_options: MethodOptions) -> Result<PreparedMethod> {
    anyhow::bail!("method `hyphenation-extended-runtime` requires feature `adapters-hyphenation`")
}

fn prepare_hypher_liang_consensus(options: MethodOptions) -> Result<PreparedMethod> {
    let adapter = adapter_for_method("hypher", &options.locale)?;
    let mut config = adapter.config().clone();
    apply_config_overrides(&mut config, &options);
    let first = PreparedMethod::Adapter {
        inner: adapter,
        config: config.clone(),
    };
    let second = prepare_liang(MethodOptions {
        method: "liang".to_string(),
        locale: options.locale.clone(),
        patterns: options.patterns.clone(),
        dictionary: None,
        dictionary_is_gold_oracle: false,
        external_command: None,
        left_min: options.left_min,
        right_min: options.right_min,
        min_word_len: options.min_word_len,
    })?;
    Ok(PreparedMethod::Intersection {
        id: format!("hypher-liang-consensus:{}", second.id()),
        config,
        first: Box::new(first),
        second: Box::new(second),
    })
}

fn prepare_external_jsonl(options: MethodOptions) -> Result<PreparedMethod> {
    let command = options
        .external_command
        .as_ref()
        .context("--external-command is required for --method external-jsonl")?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::ExternalJsonl(ExternalJsonlMethod::new(
        command,
        &options.locale,
        config,
    )?))
}

fn prepare_liang(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .patterns
        .as_ref()
        .context("--patterns is required for --method liang")?;
    let mut set = parse_pattern_file(path)?;
    let mut config = HyphenationConfig::default();
    if let Some(left_min) = set.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = set.right_min {
        config.right_min = right_min;
    }
    set.left_min = None;
    set.right_min = None;
    apply_config_overrides(&mut config, &options);

    let language = options
        .locale
        .parse::<LanguageTag>()
        .map_err(|err| anyhow::anyhow!("parse locale {:?}: {err}", options.locale))?;
    let id = format!("liang:{}", file_stem(path));
    Ok(PreparedMethod::Liang(LiangHyphenator::new(
        id, language, config, set,
    )))
}

fn prepare_safe_ngram(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method safe-ngram")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::SafeNgram(SafeNgramMethod::train(
        &options.method,
        &options.locale,
        path,
        config,
        &records,
    )?))
}

fn prepare_boundary_bayes(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method boundary-bayes")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::BoundaryBayes(BoundaryBayesMethod::train(
        &options.method,
        path,
        config,
        &records,
    )?))
}

fn prepare_stacked_bayes(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method stacked-bayes")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::StackedBayes(StackedBayesMethod::train(
        &options.method,
        &options.locale,
        path,
        options.patterns.as_ref(),
        config,
        &records,
    )?))
}

fn prepare_stacked_logit(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method stacked-logit")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::StackedBayes(
        StackedBayesMethod::train_logit(
            &options.method,
            &options.locale,
            path,
            options.patterns.as_ref(),
            config,
            &records,
        )?,
    ))
}

fn prepare_candidate_bayes(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method candidate-bayes")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::CandidateBayes(CandidateBayesMethod::train(
        &options.method,
        &options.locale,
        path,
        options.patterns.as_ref(),
        config,
        &records,
    )?))
}

fn prepare_candidate_logit(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method candidate-logit")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::CandidateBayes(
        CandidateBayesMethod::train_logit(
            &options.method,
            &options.locale,
            path,
            options.patterns.as_ref(),
            config,
            &records,
        )?,
    ))
}

fn prepare_prune_bayes(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method prune-bayes")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::PruneBayes(PruneBayesMethod::train(
        &options.method,
        &options.locale,
        path,
        options.patterns.as_ref(),
        config,
        &records,
    )?))
}

fn prepare_ranked_union(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method ranked-union")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::RankedUnion(RankedUnionMethod::new(
        &options.method,
        &options.locale,
        path,
        options.patterns.as_ref(),
        config,
        &records,
    )?))
}

fn prepare_italian_syllable(options: MethodOptions) -> Result<PreparedMethod> {
    Ok(PreparedMethod::ItalianSyllable(ItalianSyllableMethod::new(
        &options.method,
        &options,
    )?))
}

fn prepare_italian_syllable_model(options: MethodOptions) -> Result<PreparedMethod> {
    anyhow::ensure!(
        options.left_min.is_none() && options.right_min.is_none() && options.min_word_len.is_none(),
        "italian-syllable-model uses the saved model config; CLI config overrides are not supported"
    );
    let path = options.dictionary.as_ref().context(
        "--dictionary is required as the model path for --method italian-syllable-model",
    )?;
    let model = ItalianSyllableModelFile::load(path)?;
    Ok(PreparedMethod::ItalianSyllable(
        ItalianSyllableMethod::from_model(path, &options.locale, model)?,
    ))
}

fn prepare_candidate_gate(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method candidate-gate")?;
    let records = read_records(path)?;
    let calibration_records = options
        .external_command
        .as_ref()
        .map(PathBuf::from)
        .map(|path| {
            read_records(&path)
                .with_context(|| format!("read selector calibration {}", path.display()))
        })
        .transpose()?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::CandidateGate(CandidateGateMethod::train(
        &options.method,
        &options.locale,
        path,
        options.patterns.as_ref(),
        config,
        &records,
        calibration_records.as_deref(),
    )?))
}

fn prepare_mask_rerank(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method mask-rerank")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::MaskRerank(MaskRerankMethod::train(
        &options.method,
        &options.locale,
        path,
        options.patterns.as_ref(),
        config,
        &records,
    )?))
}

fn prepare_mask_oracle(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method mask-oracle")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::MaskOracle(MaskOracleMethod::new(
        &options.method,
        &options.locale,
        path,
        options.patterns.as_ref(),
        config,
        &records,
    )?))
}

fn prepare_mask_cost(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method mask-cost")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::MaskCost(MaskCostMethod::train(
        &options.method,
        &options.locale,
        path,
        options.patterns.as_ref(),
        config,
        &records,
    )?))
}

fn prepare_safe_ngram_model(options: MethodOptions) -> Result<PreparedMethod> {
    anyhow::ensure!(
        options.left_min.is_none() && options.right_min.is_none() && options.min_word_len.is_none(),
        "safe-ngram-model uses the saved model config; CLI config overrides are not supported"
    );
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the model path for --method safe-ngram-model")?;
    let model = SafeNgramModelFile::load(path)?;
    Ok(PreparedMethod::SafeNgram(SafeNgramMethod::from_model(
        path,
        &options.locale,
        model,
    )?))
}

fn prepare_hypher_safe_add(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method hypher-safe-add-*")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::HypherSafeAdd(HypherSafeAddMethod::train(
        &options.method,
        &options.locale,
        path,
        config,
        &records,
    )?))
}

fn prepare_liang_safe_add(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method liang-safe-add-*")?;
    let records = read_records(path)?;
    let base = prepare_liang(MethodOptions {
        method: "liang".to_string(),
        locale: options.locale.clone(),
        patterns: options.patterns.clone(),
        dictionary: None,
        dictionary_is_gold_oracle: false,
        external_command: None,
        left_min: options.left_min,
        right_min: options.right_min,
        min_word_len: options.min_word_len,
    })?;
    let config = base.config().clone();
    let base_label = file_stem(
        options
            .patterns
            .as_ref()
            .context("--patterns is required for --method liang-safe-add-*")?,
    );
    Ok(PreparedMethod::BaseSafeAdd(BaseSafeAddMethod::train(
        &options.method,
        &base_label,
        base,
        path,
        config,
        &records,
    )?))
}

fn prepare_safe_ladder(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method safe-ladder-*")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::SafeLadder(SafeLadderMethod::train(
        &options.method,
        &options.locale,
        path,
        config,
        &records,
    )?))
}

fn prepare_liang_veto(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method liang-veto-*")?;
    let records = read_records(path)?;
    let base = prepare_liang(MethodOptions {
        method: "liang".to_string(),
        locale: options.locale.clone(),
        patterns: options.patterns.clone(),
        dictionary: None,
        dictionary_is_gold_oracle: false,
        external_command: None,
        left_min: options.left_min,
        right_min: options.right_min,
        min_word_len: options.min_word_len,
    })?;
    let config = base.config().clone();
    let base_label = file_stem(
        options
            .patterns
            .as_ref()
            .context("--patterns is required for --method liang-veto-*")?,
    );
    Ok(PreparedMethod::BaseVeto(BaseVetoMethod::train(
        &options.method,
        &base_label,
        base,
        path,
        config,
        &records,
    )?))
}

fn prepare_residual_safe_add(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method residual-safe-add-*")?;
    let records = read_records(path)?;
    let base_method = "liang-safe-add-multi-s1-p65-veto-mixcv-multi-s1-n0";
    let base = prepare_liang_safe_add(MethodOptions {
        method: base_method.to_string(),
        locale: options.locale.clone(),
        patterns: options.patterns.clone(),
        dictionary: options.dictionary.clone(),
        dictionary_is_gold_oracle: false,
        external_command: None,
        left_min: options.left_min,
        right_min: options.right_min,
        min_word_len: options.min_word_len,
    })?;
    let config = base.config().clone();
    Ok(PreparedMethod::BaseSafeAdd(
        BaseSafeAddMethod::train_residual(
            &options.method,
            "liang_safe_add_p65_mixcv",
            base,
            path,
            config,
            &records,
        )?,
    ))
}

fn prepare_affix_safe_add(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method affix-safe-add-*")?;
    let records = read_records(path)?;
    let base_method = "liang-safe-add-multi-s1-p65-veto-mixcv-multi-s1-n0";
    let base = prepare_liang_safe_add(MethodOptions {
        method: base_method.to_string(),
        locale: options.locale.clone(),
        patterns: options.patterns.clone(),
        dictionary: options.dictionary.clone(),
        dictionary_is_gold_oracle: false,
        external_command: None,
        left_min: options.left_min,
        right_min: options.right_min,
        min_word_len: options.min_word_len,
    })?;
    let config = base.config().clone();
    Ok(PreparedMethod::AffixSafeAdd(AffixSafeAddMethod::train(
        &options.method,
        "liang_safe_add_p65_mixcv",
        base,
        path,
        config,
        &records,
    )?))
}

fn prepare_affix_veto(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method affix-veto-*")?;
    let records = read_records(path)?;
    let base_method = "liang-safe-add-multi-s1-p65-veto-mixcv-multi-s1-n0";
    let base = prepare_liang_safe_add(MethodOptions {
        method: base_method.to_string(),
        locale: options.locale.clone(),
        patterns: options.patterns.clone(),
        dictionary: options.dictionary.clone(),
        dictionary_is_gold_oracle: false,
        external_command: None,
        left_min: options.left_min,
        right_min: options.right_min,
        min_word_len: options.min_word_len,
    })?;
    let config = base.config().clone();
    Ok(PreparedMethod::AffixVeto(AffixVetoMethod::train(
        &options.method,
        "liang_safe_add_p65_mixcv",
        base,
        path,
        config,
        &records,
    )?))
}

fn prepare_analog_safe_add(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method analog-safe-add-*")?;
    let records = read_records(path)?;
    let base_method = "liang-safe-add-multi-s1-p65-veto-mixcv-multi-s1-n0";
    let base = prepare_liang_safe_add(MethodOptions {
        method: base_method.to_string(),
        locale: options.locale.clone(),
        patterns: options.patterns.clone(),
        dictionary: options.dictionary.clone(),
        dictionary_is_gold_oracle: false,
        external_command: None,
        left_min: options.left_min,
        right_min: options.right_min,
        min_word_len: options.min_word_len,
    })?;
    let config = base.config().clone();
    Ok(PreparedMethod::AnalogSafeAdd(AnalogSafeAddMethod::train(
        &options.method,
        "liang_safe_add_p65_mixcv",
        base,
        path,
        config,
        &records,
    )?))
}

fn prepare_pron_count_cap(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method pron-count-cap-*")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::PronCountCap(PronCountCapMethod::train(
        &options.method,
        &options.locale,
        path,
        options.patterns.as_ref(),
        config,
        &records,
    )?))
}

fn prepare_dictionary(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required for --method dict")?;
    let records = read_records(path)?;
    let mut entries = HashMap::new();
    for record in records {
        entries.insert(record.word, record.breaks);
    }

    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::Dictionary {
        id: if options.dictionary_is_gold_oracle {
            format!("dict-oracle:{}", file_stem(path))
        } else {
            format!("dict:{}", file_stem(path))
        },
        config,
        entries,
    })
}

fn is_dictionary_method(method: &str) -> bool {
    matches!(
        method.to_ascii_lowercase().as_str(),
        "dict" | "dictionary" | "lookup"
    )
}

fn prepare_dictionary_fallback_safe_ngram_model(options: MethodOptions) -> Result<PreparedMethod> {
    let dictionary_path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the primary dictionary for --method dict-fallback-safe-ngram-model")?;
    let fallback_path = options
        .patterns
        .as_ref()
        .context("--patterns is required as the fallback safe-ngram model path for --method dict-fallback-safe-ngram-model")?;
    let records = read_records(dictionary_path)?;
    let mut entries = HashMap::new();
    for record in records {
        entries.insert(record.word, record.breaks);
    }
    let fallback_model = SafeNgramModelFile::load(fallback_path)?;
    let fallback = SafeNgramMethod::from_model(fallback_path, &options.locale, fallback_model)
        .with_context(|| format!("load fallback safe-ngram model {}", fallback_path.display()))?;
    let mut config = fallback.config().clone();
    apply_config_overrides(&mut config, &options);
    anyhow::ensure!(
        config == *fallback.config(),
        "dict-fallback-safe-ngram-model does not support config overrides that differ from the saved fallback model"
    );
    let fallback = PreparedMethod::SafeNgram(fallback);
    Ok(PreparedMethod::DictionaryFallback {
        id: format!(
            "dict-fallback:{}->{}",
            file_stem(dictionary_path),
            fallback.id()
        ),
        config,
        entries,
        fallback: Box::new(fallback),
    })
}

fn apply_config_overrides(config: &mut HyphenationConfig, options: &MethodOptions) {
    if let Some(left_min) = options.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = options.right_min {
        config.right_min = right_min;
    }
    if let Some(min_word_len) = options.min_word_len {
        config.min_word_len = min_word_len;
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn parse_byte(value: &str) -> Result<u8> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u8::from_str_radix(hex, 16).with_context(|| format!("parse byte {value:?}"))
    } else {
        trimmed
            .parse::<u8>()
            .with_context(|| format!("parse byte {value:?}"))
    }
}

fn parse_u64_key(value: &str) -> Result<u64> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).with_context(|| format!("parse u64 key {value:?}"))
    } else {
        trimmed
            .parse::<u64>()
            .with_context(|| format!("parse u64 key {value:?}"))
    }
}

fn evaluation_metadata(
    gold: &Path,
    locale: &str,
    patterns: Option<&PathBuf>,
    ambiguous_policy: AmbiguousPolicyArg,
    left_min: Option<usize>,
    right_min: Option<usize>,
    min_word_len: Option<usize>,
) -> EvaluationMetadata {
    EvaluationMetadata {
        gold: gold.display().to_string(),
        locale: locale.to_string(),
        patterns: patterns.map(|path| path.display().to_string()),
        ambiguous_policy: ambiguous_policy.as_str().to_string(),
        left_min,
        right_min,
        min_word_len,
    }
}

fn write_report(
    path: &Path,
    method: &str,
    evaluation: &EvaluationMetadata,
    report: &EvaluationReport,
) -> Result<()> {
    create_parent(path)?;
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let writer = BufWriter::new(file);
    let payload = serde_json::json!({
        "method": method,
        "evaluation": evaluation,
        "metrics": &report.metrics,
        "error_words": report.errors.len(),
        "method_error_words": report.method_errors.len(),
    });
    serde_json::to_writer_pretty(writer, &payload)?;
    Ok(())
}

fn write_errors(path: &Path, errors: &[WordError]) -> Result<()> {
    create_parent(path)?;
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for error in errors {
        serde_json::to_writer(&mut writer, error)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn write_method_errors(path: &Path, errors: &[MethodError]) -> Result<()> {
    create_parent(path)?;
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for error in errors {
        serde_json::to_writer(&mut writer, error)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory {}", parent.display()))?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct CompareRow {
    method: String,
    metrics: Metrics,
    evaluation: Option<EvaluationMetadata>,
    speed: Option<SpeedSummary>,
    init: Option<InitSummary>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct EvaluationMetadata {
    gold: String,
    locale: String,
    patterns: Option<String>,
    ambiguous_policy: String,
    left_min: Option<usize>,
    right_min: Option<usize>,
    min_word_len: Option<usize>,
}

#[derive(Debug, Clone)]
struct SpeedSummary {
    method: String,
    ns_per_word: f64,
    words_per_sec: f64,
}

#[derive(Debug, Clone)]
struct InitSummary {
    method: String,
    ns_per_init: f64,
}

fn read_speed_summary(path: &Path) -> Result<SpeedSummary> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_reader(file).with_context(|| format!("parse {}", path.display()))?;
    let method = value
        .get("method")
        .and_then(|value| value.as_str())
        .with_context(|| format!("read method from {}", path.display()))?
        .to_string();
    let ns_per_word = value
        .get("ns_per_word")
        .and_then(|value| value.as_f64())
        .with_context(|| format!("read ns_per_word from {}", path.display()))?;
    let words_per_sec = value
        .get("words_per_sec")
        .and_then(|value| value.as_f64())
        .with_context(|| format!("read words_per_sec from {}", path.display()))?;
    Ok(SpeedSummary {
        method,
        ns_per_word,
        words_per_sec,
    })
}

fn read_init_summary(path: &Path) -> Result<InitSummary> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_reader(file).with_context(|| format!("parse {}", path.display()))?;
    let method = value
        .get("method")
        .and_then(|value| value.as_str())
        .with_context(|| format!("read method from {}", path.display()))?
        .to_string();
    let ns_per_init = value
        .get("ns_per_init")
        .and_then(|value| value.as_f64())
        .with_context(|| format!("read ns_per_init from {}", path.display()))?;
    Ok(InitSummary {
        method,
        ns_per_init,
    })
}

fn render_compare_table(rows: &[CompareRow]) -> String {
    let include_speed = rows.iter().any(|row| row.speed.is_some());
    let include_init = rows.iter().any(|row| row.init.is_some());
    let include_method_errors = rows.iter().any(|row| row.metrics.skipped_method_errors > 0);

    let mut headers = vec![
        "method",
        "words",
        "precision",
        "recall",
        "f1",
        "f0.5",
        "exact",
        "serious_error",
        "fp/100k",
    ];
    if include_method_errors {
        headers.insert(2, "method_errors");
    }
    if include_speed {
        headers.push("steady ns/word");
        headers.push("steady words/sec");
    }
    if include_init {
        headers.push("init ms");
    }

    let mut out = render_evaluation_metadata(rows);
    out.push_str("| ");
    out.push_str(&headers.join(" | "));
    out.push_str(" |\n");
    out.push_str("| --- |");
    for _ in 1..headers.len() {
        out.push_str(" ---: |");
    }
    out.push('\n');

    for row in rows {
        let metrics = &row.metrics;
        out.push_str(&format!("| {} | {} |", row.method, metrics.words));
        if include_method_errors {
            out.push_str(&format!(" {} |", metrics.skipped_method_errors));
        }
        out.push_str(&format!(
            " {:.6} | {:.6} | {:.6} | {:.6} | {:.6} | {:.6} | {:.3} |",
            metrics.precision(),
            metrics.recall(),
            metrics.f1(),
            metrics.f05(),
            metrics.exact_accuracy(),
            metrics.serious_word_error_rate(),
            metrics.fp_per_100k_boundaries()
        ));
        if include_speed {
            if let Some(speed) = &row.speed {
                out.push_str(&format!(
                    " {:.3} | {:.3} |",
                    speed.ns_per_word, speed.words_per_sec
                ));
            } else {
                out.push_str("  |  |");
            }
        }
        if include_init {
            if let Some(init) = &row.init {
                out.push_str(&format!(" {:.3} |", init.ns_per_init / 1_000_000.0));
            } else {
                out.push_str("  |");
            }
        }
        out.push('\n');
    }
    out
}

fn render_evaluation_metadata(rows: &[CompareRow]) -> String {
    let evaluations = rows
        .iter()
        .filter_map(|row| row.evaluation.as_ref())
        .collect::<Vec<_>>();
    if evaluations.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("## Evaluation Data\n\n");
    if evaluations
        .iter()
        .all(|evaluation| *evaluation == evaluations[0])
    {
        let evaluation = evaluations[0];
        out.push_str(&format!("- gold: `{}`\n", evaluation.gold));
        out.push_str(&format!("- locale: `{}`\n", evaluation.locale));
        out.push_str(&format!(
            "- patterns: `{}`\n",
            evaluation.patterns.as_deref().unwrap_or("none")
        ));
        out.push_str(&format!(
            "- ambiguous_policy: `{}`\n",
            evaluation.ambiguous_policy
        ));
        out.push_str(&format!(
            "- boundary_config: left_min=`{}`, right_min=`{}`, min_word_len=`{}`\n",
            optional_usize(evaluation.left_min),
            optional_usize(evaluation.right_min),
            optional_usize(evaluation.min_word_len)
        ));
    } else {
        out.push_str("Rows have mixed evaluation metadata.\n\n");
        render_metadata_set(
            &mut out,
            "gold",
            evaluations.iter().map(|item| item.gold.as_str()),
        );
        render_metadata_set(
            &mut out,
            "locale",
            evaluations.iter().map(|item| item.locale.as_str()),
        );
        render_metadata_set(
            &mut out,
            "patterns",
            evaluations
                .iter()
                .map(|item| item.patterns.as_deref().unwrap_or("none")),
        );
        render_metadata_set(
            &mut out,
            "ambiguous_policy",
            evaluations
                .iter()
                .map(|item| item.ambiguous_policy.as_str()),
        );
    }
    out.push('\n');
    out
}

fn render_metadata_set<'a>(out: &mut String, label: &str, values: impl Iterator<Item = &'a str>) {
    let values = values.collect::<BTreeSet<_>>();
    let rendered = values
        .into_iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!("- {label}: {rendered}\n"));
}

fn optional_usize(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "default".to_string())
}

fn print_metrics(method: &str, metrics: &Metrics) {
    println!("method: {method}");
    println!("words: {}", metrics.words);
    println!("skipped_ambiguous: {}", metrics.skipped_ambiguous);
    println!("skipped_method_errors: {}", metrics.skipped_method_errors);
    println!("precision: {:.6}", metrics.precision());
    println!("recall: {:.6}", metrics.recall());
    println!("f1: {:.6}", metrics.f1());
    println!("f0.5: {:.6}", metrics.f05());
    println!("exact_accuracy: {:.6}", metrics.exact_accuracy());
    println!(
        "serious_word_error_rate: {:.6}",
        metrics.serious_word_error_rate()
    );
    println!("no_break_accuracy: {:.6}", metrics.no_break_accuracy());
    println!(
        "fp_per_100k_boundaries: {:.3}",
        metrics.fp_per_100k_boundaries()
    );
    println!(
        "confusion: tp={} fp={} fn={} tn={}",
        metrics.tp, metrics.fp, metrics.fn_, metrics.tn
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_counts_keeps_positive_splits_nonempty_when_possible() {
        assert_eq!(split_counts(5, [0.8, 0.1, 0.1], 1.0), [3, 1, 1]);
    }

    #[test]
    fn split_counts_allocates_tiny_inputs_by_largest_ratios() {
        assert_eq!(split_counts(2, [0.8, 0.1, 0.1], 1.0), [1, 1, 0]);
    }

    #[test]
    fn safe_ngram_binary_roundtrip_preserves_model() {
        let options = SafeNgramOptions {
            specs: vec![
                SafeNgramSpec {
                    left: 5,
                    right: 5,
                    bucketed: false,
                    family: 0,
                },
                SafeNgramSpec {
                    left: 4,
                    right: 4,
                    bucketed: false,
                    family: 0,
                },
                SafeNgramSpec {
                    left: 3,
                    right: 3,
                    bucketed: false,
                    family: 0,
                },
            ],
            min_support: 1,
            max_negative: 1,
            min_precision_ppm: None,
            min_wilson_ppm: None,
            cap_vowel_nuclei: false,
            orthographic_veto: false,
            unicode_aware: false,
        };
        let mut rules = U64HashSet::default();
        rules.insert(42);
        rules.insert(7);
        rules.insert(99);
        let model = SafeNgramModelFile::from_parts(
            "safe-ngram-multi-s1-n1".to_string(),
            "en-US".to_string(),
            "fixture".to_string(),
            HyphenationConfig::default(),
            options.clone(),
            rules,
            None,
            U64HashSet::default(),
            3,
        );
        let path = std::env::temp_dir().join(format!(
            "hyphlab-safe-ngram-{}-roundtrip.bin",
            std::process::id()
        ));
        model.save(&path).unwrap();
        let loaded = SafeNgramModelFile::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.method, "safe-ngram-multi-s1-n1");
        assert_eq!(loaded.locale, "en-US");
        assert_eq!(loaded.source, "fixture");
        assert_eq!(loaded.config, HyphenationConfig::default());
        assert_eq!(loaded.options, options);
        assert_eq!(loaded.trained_records, 3);
        assert_eq!(loaded.rules, vec![7, 42, 99]);
    }

    #[test]
    fn unicode_safe_ngram_learns_grapheme_boundaries() {
        let options = parse_safe_ngram_options("safe-ngram-unicode-1x1-s1-p80").unwrap();
        assert!(options.unicode_aware);
        let config = HyphenationConfig {
            left_min: 1,
            right_min: 1,
            min_word_len: 2,
            ..HyphenationConfig::default()
        };
        let record =
            HyphenationRecord::new("tr:1", "tr", "çağ", SmallVec::from_vec(vec![1]), "test");
        let (rules, trained_records) = learn_safe_ngram_rules(&[record], &config, &options);
        assert_eq!(trained_records, 1);
        assert!(!rules.is_empty());
    }

    #[test]
    fn script_filter_distinguishes_russian_cyrillic_from_mixed_words() {
        assert!(script_filter_matches(
            "перенос",
            ScriptFilterArg::RussianCyrillic
        ));
        assert!(script_filter_matches("нақтылық", ScriptFilterArg::Cyrillic));
        assert!(!script_filter_matches(
            "нақтылық",
            ScriptFilterArg::RussianCyrillic
        ));
        assert!(!script_filter_matches(
            "Gaslieferant",
            ScriptFilterArg::RussianCyrillic
        ));
        assert!(script_filter_matches(
            "Gaslieferant",
            ScriptFilterArg::Latin
        ));
    }
}
