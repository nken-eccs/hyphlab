// CLI command tree and argument structs.

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

