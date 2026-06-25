use anyhow::{Context, Result};
use clap::Parser;
use hyph_core::{insert_separator, GraphemeIndex, HyphenationRecord};
use hyph_data::read_records;
use smallvec::SmallVec;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    io::{self, BufRead},
    path::PathBuf,
};

use crate::{prepare_method, MethodOptions, PreparedMethod};

#[derive(Debug, Parser)]
pub(crate) struct PredictArgs {
    #[arg(short, long, default_value = "hypher")]
    method: String,
    #[arg(short, long, default_value = "en-US")]
    locale: String,
    #[arg(long, value_name = "KEY")]
    saved_model: Option<String>,
    #[arg(long = "with-saved-model", value_name = "KEY")]
    with_saved_model: Vec<String>,
    #[arg(long = "with-method", value_name = "METHOD")]
    with_method: Vec<String>,
    #[arg(long)]
    with_hypher: bool,
    #[arg(long)]
    list_saved_models: bool,
    #[arg(long)]
    gold: Option<PathBuf>,
    #[arg(long)]
    patterns: Option<PathBuf>,
    #[arg(long)]
    guard_policy: Option<PathBuf>,
    #[arg(long)]
    guard_fragments: Option<PathBuf>,
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

#[derive(Debug, Clone, Copy)]
struct SavedModelSpec {
    key: &'static str,
    aliases: &'static [&'static str],
    locale: &'static str,
    method: &'static str,
    dictionary: &'static str,
    patterns: Option<&'static str>,
    guard_policy: Option<&'static str>,
    guard_fragments: Option<&'static str>,
}

const SAVED_MODEL_SPECS: &[SavedModelSpec] = &[
    SavedModelSpec {
        key: "en-US",
        aliases: &["en", "english", "moby", "moby-en-us", "moby_en_us"],
        locale: "en-US",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/moby_en_us.bin",
        patterns: None,
        guard_policy: None,
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "en-US-typeset",
        aliases: &[
            "en-typeset",
            "english-typeset",
            "moby-typeset",
            "moby_en_us_typeset",
            "moby-en-us-typeset",
        ],
        locale: "en-US",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/moby_en_us_typeset.bin",
        patterns: None,
        guard_policy: Some("data/curation/guard_policies/moby_en_us_typeset.toml"),
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "cs",
        aliases: &["czech", "wiktextract-cs", "wiktextract_cs"],
        locale: "cs",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_cs.bin",
        patterns: None,
        guard_policy: None,
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "cs-typeset",
        aliases: &[
            "czech-typeset",
            "wiktextract-cs-typeset",
            "wiktextract_cs_typeset",
        ],
        locale: "cs",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_cs_typeset.bin",
        patterns: None,
        guard_policy: Some("data/curation/guard_policies/wiktextract_cs_typeset.toml"),
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "de",
        aliases: &["german", "wiktextract-de", "wiktextract_de"],
        locale: "de",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_de.bin",
        patterns: None,
        guard_policy: None,
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "de-typeset",
        aliases: &[
            "german-typeset",
            "wiktextract-de-typeset",
            "wiktextract_de_typeset",
        ],
        locale: "de",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_de_typeset.bin",
        patterns: None,
        guard_policy: Some("data/curation/guard_policies/wiktextract_de_typeset.toml"),
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "es",
        aliases: &["spanish", "wiktextract-es", "wiktextract_es"],
        locale: "es",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_es.bin",
        patterns: None,
        guard_policy: None,
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "es-typeset",
        aliases: &[
            "spanish-typeset",
            "wiktextract-es-typeset",
            "wiktextract_es_typeset",
        ],
        locale: "es",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_es_typeset.bin",
        patterns: None,
        guard_policy: Some("data/curation/guard_policies/wiktextract_es_typeset.toml"),
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "it",
        aliases: &["italian", "wiktextract-it", "wiktextract_it"],
        locale: "it",
        method: "italian-syllable-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_it.json",
        patterns: None,
        guard_policy: None,
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "it-typeset",
        aliases: &[
            "italian-typeset",
            "wiktextract-it-typeset",
            "wiktextract_it_typeset",
        ],
        locale: "it",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_it_typeset.bin",
        patterns: None,
        guard_policy: Some("data/curation/guard_policies/wiktextract_it_typeset.toml"),
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "nl",
        aliases: &["dutch", "wiktextract-nl", "wiktextract_nl"],
        locale: "nl",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_nl.bin",
        patterns: None,
        guard_policy: None,
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "nl-typeset",
        aliases: &[
            "dutch-typeset",
            "wiktextract-nl-typeset",
            "wiktextract_nl_typeset",
        ],
        locale: "nl",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_nl_typeset.bin",
        patterns: None,
        guard_policy: Some("data/curation/guard_policies/wiktextract_nl_typeset.toml"),
        guard_fragments: None,
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
        patterns: None,
        guard_policy: None,
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "ru-typeset",
        aliases: &[
            "russian-typeset",
            "wiktextract-ru-typeset",
            "wiktextract_ru_typeset",
            "ru-cyrl-trusted-dedup-typeset",
            "wiktextract-ru-cyrl-trusted-dedup-typeset",
            "wiktextract_ru_cyrl_trusted_dedup_typeset",
        ],
        locale: "ru",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_ru_cyrl_trusted_dedup_typeset.bin",
        patterns: None,
        guard_policy: Some(
            "data/curation/guard_policies/wiktextract_ru_cyrl_trusted_dedup_typeset.toml",
        ),
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "tr",
        aliases: &["turkish", "wiktextract-tr", "wiktextract_tr"],
        locale: "tr",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_tr.bin",
        patterns: None,
        guard_policy: None,
        guard_fragments: None,
    },
    SavedModelSpec {
        key: "tr-typeset",
        aliases: &[
            "turkish-typeset",
            "wiktextract-tr-typeset",
            "wiktextract_tr_typeset",
        ],
        locale: "tr",
        method: "safe-ngram-model",
        dictionary: "models/guarded_ngram/v1/wiktextract_tr_typeset.bin",
        patterns: None,
        guard_policy: Some("data/curation/guard_policies/wiktextract_tr_typeset.toml"),
        guard_fragments: None,
    },
];

pub(crate) fn cmd_predict(args: PredictArgs) -> Result<()> {
    if args.list_saved_models {
        print_saved_models();
        return Ok(());
    }

    let PredictArgs {
        mut method,
        mut locale,
        saved_model,
        with_saved_model,
        mut with_method,
        with_hypher,
        list_saved_models: _,
        gold,
        mut patterns,
        guard_policy,
        guard_fragments,
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

    let shared_patterns = patterns.clone();
    let shared_guard_policy = guard_policy.clone();
    let shared_guard_fragments = guard_fragments.clone();
    let shared_dictionary = dictionary.clone();
    let shared_external_command = external_command.clone();
    let gold_lookup = gold.map(GoldLookup::load).transpose()?;

    let saved_model_spec = if let Some(saved_model) = saved_model.as_deref() {
        anyhow::ensure!(
            dictionary.is_none(),
            "--saved-model cannot be combined with --dictionary"
        );
        let spec = resolve_saved_model(saved_model)?;
        method = spec.method.to_string();
        locale = spec.locale.to_string();
        dictionary = Some(PathBuf::from(spec.dictionary));
        if patterns.is_none() {
            patterns = spec.patterns.map(PathBuf::from);
        }
        Some(spec)
    } else {
        None
    };

    if with_hypher
        && !with_method
            .iter()
            .any(|method| method.eq_ignore_ascii_case("hypher"))
    {
        with_method.push("hypher".to_string());
    }

    let mut methods = Vec::<PredictMethod>::new();
    let primary_label = if let Some(saved_model) = &saved_model {
        format!("saved:{saved_model}")
    } else {
        method.clone()
    };
    methods.push(PredictMethod {
        label: primary_label,
        method: prepare_method(MethodOptions {
            method,
            locale: locale.clone(),
            patterns: patterns.clone(),
            guard_policy: guard_policy
                .clone()
                .or_else(|| saved_model_spec.and_then(|spec| spec.guard_policy.map(PathBuf::from))),
            guard_fragments: guard_fragments.clone().or_else(|| {
                saved_model_spec.and_then(|spec| spec.guard_fragments.map(PathBuf::from))
            }),
            dictionary,
            dictionary_is_gold_oracle: false,
            external_command,
            left_min,
            right_min,
            min_word_len,
        })?,
    });

    for saved_model in with_saved_model {
        let spec = resolve_saved_model(&saved_model)?;
        methods.push(PredictMethod {
            label: format!("saved:{saved_model}"),
            method: prepare_method(MethodOptions {
                method: spec.method.to_string(),
                locale: spec.locale.to_string(),
                patterns: spec.patterns.map(PathBuf::from),
                guard_policy: spec.guard_policy.map(PathBuf::from),
                guard_fragments: spec.guard_fragments.map(PathBuf::from),
                dictionary: Some(PathBuf::from(spec.dictionary)),
                dictionary_is_gold_oracle: false,
                external_command: None,
                left_min,
                right_min,
                min_word_len,
            })?,
        });
    }

    for extra_method in with_method {
        methods.push(PredictMethod {
            label: extra_method.clone(),
            method: prepare_method(MethodOptions {
                method: extra_method,
                locale: locale.clone(),
                patterns: shared_patterns.clone(),
                guard_policy: shared_guard_policy.clone(),
                guard_fragments: shared_guard_fragments.clone(),
                dictionary: shared_dictionary.clone(),
                dictionary_is_gold_oracle: false,
                external_command: shared_external_command.clone(),
                left_min,
                right_min,
                min_word_len,
            })?,
        });
    }
    dedup_predict_methods(&mut methods);

    let detailed = methods.len() > 1 || gold_lookup.is_some();
    let method = methods
        .first()
        .context("predict requires at least one prepared method")?;
    let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
    let has_direct_input = !word.is_empty() || !text.is_empty();

    for word in &word {
        if detailed {
            predict_word_compare(
                &methods,
                gold_lookup.as_ref(),
                word,
                &separator,
                show_breaks,
            )?;
        } else {
            predict_word_display(&method.method, word, &separator, show_breaks, &mut out)?;
        }
    }
    for text in &text {
        if detailed {
            predict_text_compare(&methods, gold_lookup.as_ref(), text, &separator)?;
        } else {
            predict_text_display(&method.method, text, &separator, &mut out)?;
        }
    }

    if let Some(input) = input {
        let file =
            std::fs::File::open(&input).with_context(|| format!("open {}", input.display()))?;
        let reader = io::BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            if detailed {
                predict_word_compare(
                    &methods,
                    gold_lookup.as_ref(),
                    &line,
                    &separator,
                    show_breaks,
                )?;
            } else {
                predict_one(&method.method, &line, &separator, &mut out)?;
            }
        }
    } else if !has_direct_input {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = line?;
            if detailed {
                predict_word_compare(
                    &methods,
                    gold_lookup.as_ref(),
                    &line,
                    &separator,
                    show_breaks,
                )?;
            } else {
                predict_one(&method.method, &line, &separator, &mut out)?;
            }
        }
    }

    Ok(())
}

fn print_saved_models() {
    println!("key\tlocale\tmethod\tdictionary\tpatterns\tguard_policy");
    for spec in SAVED_MODEL_SPECS {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            spec.key,
            spec.locale,
            spec.method,
            spec.dictionary,
            spec.patterns.unwrap_or(""),
            spec.guard_policy.unwrap_or("")
        );
    }
}

fn resolve_saved_model(key: &str) -> Result<&'static SavedModelSpec> {
    resolve_saved_model_optional(key).with_context(|| {
        format!("unknown --saved-model {key:?}; run `hyphlab predict --list-saved-models`")
    })
}

fn resolve_saved_model_optional(key: &str) -> Option<&'static SavedModelSpec> {
    let normalized = normalize_saved_model_key(key);
    SAVED_MODEL_SPECS.iter().find(|spec| {
        normalize_saved_model_key(spec.key) == normalized
            || spec
                .aliases
                .iter()
                .any(|alias| normalize_saved_model_key(alias) == normalized)
    })
}

fn normalize_saved_model_key(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .replace('/', "-")
}

struct PredictMethod {
    label: String,
    method: PreparedMethod,
}

struct GoldLookup {
    path: PathBuf,
    records: HashMap<String, Vec<HyphenationRecord>>,
}

impl GoldLookup {
    fn load(path: PathBuf) -> Result<Self> {
        let mut records = HashMap::<String, Vec<HyphenationRecord>>::new();
        for record in read_records(&path)? {
            records.entry(record.word.clone()).or_default().push(record);
        }
        Ok(Self { path, records })
    }

    fn find(&self, word: &str) -> Option<&[HyphenationRecord]> {
        self.records
            .get(word)
            .or_else(|| {
                let lower = word.to_lowercase();
                if lower == word {
                    None
                } else {
                    self.records.get(&lower)
                }
            })
            .map(Vec::as_slice)
    }

    fn label(&self) -> String {
        format!("gold:{}", self.path.display())
    }
}

fn dedup_predict_methods(methods: &mut Vec<PredictMethod>) {
    let mut seen = HashSet::<String>::new();
    methods.retain(|method| seen.insert(method.label.clone()));
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

fn predict_word_compare(
    methods: &[PredictMethod],
    gold: Option<&GoldLookup>,
    word: &str,
    separator: &str,
    show_breaks: bool,
) -> Result<()> {
    let word = word.trim();
    if word.is_empty() {
        return Ok(());
    }
    println!("{word}");
    if let Some(gold) = gold {
        if let Some(records) = gold.find(word) {
            println!(
                "  {}: {}",
                gold.label(),
                format_gold_records(records, separator)
            );
        } else {
            println!("  {}: <not found>", gold.label());
        }
    }
    let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
    for method in methods {
        out.clear();
        method
            .method
            .hyphenate_into(word, &mut out)
            .with_context(|| format!("predict {word:?} with {}", method.label))?;
        let rendered = insert_separator(word, &out, separator);
        if show_breaks {
            println!("  {}: {}\t{:?}", method.label, rendered, out);
        } else {
            println!("  {}: {}", method.label, rendered);
        }
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

fn predict_text_compare(
    methods: &[PredictMethod],
    gold: Option<&GoldLookup>,
    text: &str,
    separator: &str,
) -> Result<()> {
    println!("{text}");
    let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
    if let Some(gold) = gold {
        let (rendered, hits, total) = hyphenate_text_with_gold(gold, text, separator);
        if hits == 0 {
            println!("  {}: <no tokens found> ({total} tokens)", gold.label());
        } else {
            println!("  {}: {} ({hits}/{total} tokens)", gold.label(), rendered);
        }
    }
    for method in methods {
        let rendered = hyphenate_text(&method.method, text, separator, &mut out)
            .with_context(|| format!("predict text with {}", method.label))?;
        println!("  {}: {}", method.label, rendered);
    }
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

fn hyphenate_text_with_gold(
    gold: &GoldLookup,
    text: &str,
    separator: &str,
) -> (String, usize, usize) {
    let mut rendered = String::with_capacity(text.len());
    let mut word = String::new();
    let mut hits = 0usize;
    let mut total = 0usize;
    for ch in text.chars() {
        if is_text_word_char(ch) {
            word.push(ch);
        } else {
            flush_gold_text_word(
                gold,
                &mut word,
                &mut rendered,
                separator,
                &mut hits,
                &mut total,
            );
            rendered.push(ch);
        }
    }
    flush_gold_text_word(
        gold,
        &mut word,
        &mut rendered,
        separator,
        &mut hits,
        &mut total,
    );
    (rendered, hits, total)
}

fn flush_gold_text_word(
    gold: &GoldLookup,
    word: &mut String,
    rendered: &mut String,
    separator: &str,
    hits: &mut usize,
    total: &mut usize,
) {
    if word.is_empty() {
        return;
    }
    *total += 1;
    if let Some(records) = gold.find(word) {
        *hits += 1;
        rendered.push_str(
            &gold_record_primary_form(records, separator).unwrap_or_else(|| word.clone()),
        );
    } else {
        rendered.push_str(word);
    }
    word.clear();
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

fn format_gold_records(records: &[HyphenationRecord], separator: &str) -> String {
    let mut forms = BTreeSet::<String>::new();
    let mut ambiguous = false;
    for record in records {
        ambiguous |= record.ambiguous || !record.variants.is_empty() || records.len() > 1;
        forms.insert(insert_separator(&record.word, &record.breaks, separator));
        for variant in &record.variants {
            forms.insert(insert_separator(&record.word, variant, separator));
        }
    }
    let mut rendered = forms.into_iter().collect::<Vec<_>>().join(" | ");
    if ambiguous {
        rendered.push_str(" (ambiguous)");
    }
    rendered
}

fn gold_record_primary_form(records: &[HyphenationRecord], separator: &str) -> Option<String> {
    records
        .first()
        .map(|record| insert_separator(&record.word, &record.breaks, separator))
}
