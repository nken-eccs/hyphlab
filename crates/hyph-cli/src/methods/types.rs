// Method runtime types and serializable model headers.

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
    ItalianSyllable(ItalianSyllableMethod),
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

struct SafeNgramMethod {
    id: String,
    config: HyphenationConfig,
    options: SafeNgramOptions,
    uses_unicode_features: bool,
    family_mask: u8,
    rules: U64HashSet,
    rules_dense: Option<SafeNgramDenseSet>,
    rules_dual_dense: Option<SafeNgramDualDenseSet>,
    veto_options: Option<SafeNgramOptions>,
    veto_rules: U64HashSet,
    veto_rules_dense: Option<SafeNgramDenseSet>,
    veto_rules_dual_dense: Option<SafeNgramDualDenseSet>,
}

struct SafeNgramDenseSet {
    bit_count: usize,
    bits: Vec<u64>,
}

struct SafeNgramDualDenseSet {
    first: SafeNgramDenseSet,
    second: SafeNgramDenseSet,
}

#[derive(Clone, Copy)]
enum SafeNgramRuleLookup<'a> {
    Hash(&'a U64HashSet),
    Dense(&'a SafeNgramDenseSet),
}

#[derive(Clone, Copy)]
enum SafeNgramDualRuleLookup<'a> {
    Hash(&'a U64HashSet),
    Dense(&'a SafeNgramDualDenseSet),
}

#[derive(Debug)]
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

