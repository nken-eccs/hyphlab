use crate::write_records;
use anyhow::{Context, Result};
use hyph_core::{hyphenated_to_breaks, strip_separators, HyphenationRecord};
use serde_json::Value;
use smallvec::SmallVec;
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct ImportWiktextractOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub locale: Option<String>,
    pub filter_lang_code: Option<String>,
    pub source: Option<String>,
    pub license: Option<String>,
    pub skip_invalid: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportWiktextractReport {
    pub records: usize,
    pub lines: usize,
    pub skipped_lang_code: usize,
    pub skipped_no_hyphenation: usize,
    pub skipped_invalid: usize,
}

pub fn import_wiktextract(options: ImportWiktextractOptions) -> Result<ImportWiktextractReport> {
    let mut records = Vec::new();
    let mut report = ImportWiktextractReport::default();
    let source = options
        .source
        .clone()
        .unwrap_or_else(|| "wiktextract".to_string());
    let reader = open_reader(&options.input)?;

    for (line_no, line) in reader.lines().enumerate() {
        report.lines += 1;
        let line = line.with_context(|| format!("read line {}", line_no + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str::<Value>(&line)
            .with_context(|| format!("parse JSONL line {}", line_no + 1))?;

        let json_lang_code = value.get("lang_code").and_then(Value::as_str);
        if let Some(filter_lang_code) = &options.filter_lang_code {
            if json_lang_code != Some(filter_lang_code.as_str()) {
                report.skipped_lang_code += 1;
                continue;
            }
        }

        let mut candidates = Vec::new();
        collect_hyphenations(&value, &mut candidates);
        candidates.sort();
        candidates.dedup();
        if candidates.is_empty() {
            report.skipped_no_hyphenation += 1;
            continue;
        }

        let json_word = value.get("word").and_then(Value::as_str).unwrap_or("");
        let locale = options
            .locale
            .clone()
            .or_else(|| json_lang_code.map(str::to_string))
            .unwrap_or_else(|| "und".to_string());

        let mut variants = BTreeSet::new();
        let mut chosen_word = None;
        for candidate in candidates {
            let candidate = candidate.trim();
            if candidate.is_empty() {
                continue;
            }
            let stripped = strip_separators(candidate);
            if stripped.is_empty() {
                continue;
            }

            let word = if !json_word.is_empty() && stripped == json_word {
                json_word
            } else if json_word.is_empty() {
                stripped.as_str()
            } else {
                report.skipped_invalid += 1;
                if options.skip_invalid {
                    continue;
                }
                anyhow::bail!(
                    "line {} hyphenation {:?} strips to {:?}, not JSON word {:?}",
                    line_no + 1,
                    candidate,
                    stripped,
                    json_word
                );
            };

            match hyphenated_to_breaks(word, candidate) {
                Ok(breaks) => {
                    chosen_word.get_or_insert_with(|| word.to_string());
                    variants.insert(breaks.into_vec());
                }
                Err(err) if options.skip_invalid => {
                    report.skipped_invalid += 1;
                    eprintln!(
                        "skipping invalid Wiktextract hyphenation at line {}: {err}",
                        line_no + 1
                    );
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("parse Wiktextract hyphenation at line {}", line_no + 1)
                    });
                }
            }
        }

        let Some(word) = chosen_word else {
            report.skipped_no_hyphenation += 1;
            continue;
        };
        let mut variants = variants
            .into_iter()
            .map(SmallVec::from_vec)
            .collect::<Vec<_>>();
        if variants.is_empty() {
            report.skipped_no_hyphenation += 1;
            continue;
        }
        variants.sort();
        let breaks = variants[0].clone();
        let ambiguous = variants.len() > 1;
        let mut record = HyphenationRecord::new(
            format!("wiktextract:{}:{}", locale, line_no + 1),
            locale,
            word,
            breaks,
            source.clone(),
        );
        record.license = options.license.clone();
        record.ambiguous = ambiguous;
        if ambiguous {
            record.variants = variants;
        }
        records.push(record);
    }

    report.records = write_records(&options.output, records)?;
    Ok(report)
}

fn collect_hyphenations(value: &Value, out: &mut Vec<String>) {
    collect_field(value, "hyphenation", out);
    collect_field(value, "hyphenations", out);

    if let Some(sounds) = value.get("sounds").and_then(Value::as_array) {
        for sound in sounds {
            collect_field(sound, "hyphenation", out);
            collect_field(sound, "hyphenations", out);
        }
    }
}

fn collect_field(value: &Value, field: &str, out: &mut Vec<String>) {
    let Some(value) = value.get(field) else {
        return;
    };
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                match item {
                    Value::String(text) => out.push(text.clone()),
                    Value::Object(_) => {
                        collect_parts(item, out);
                        collect_field(item, "hyphenation", out);
                        collect_field(item, "hyphenations", out);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn collect_parts(value: &Value, out: &mut Vec<String>) {
    let Some(parts) = value.get("parts").and_then(Value::as_array) else {
        return;
    };
    let mut text = String::new();
    for (idx, part) in parts.iter().filter_map(Value::as_str).enumerate() {
        if idx > 0 {
            text.push('-');
        }
        text.push_str(part);
    }
    if !text.is_empty() {
        out.push(text);
    }
}

fn open_reader(path: &Path) -> Result<Box<dyn BufRead>> {
    if path == Path::new("-") {
        return Ok(Box::new(BufReader::new(io::stdin())));
    }
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    Ok(Box::new(BufReader::new(file)))
}
