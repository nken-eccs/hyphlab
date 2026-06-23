use crate::write_records;
use anyhow::{Context, Result};
use hyph_core::{
    hyphenated_to_breaks, insert_separator, strip_separators, HyphenationRecord, Normalization,
};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct ImportWlhambOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub locale: Option<String>,
    pub source: Option<String>,
    pub license: Option<String>,
    pub skip_invalid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportWlhambReport {
    pub records: usize,
    pub skipped_invalid: usize,
}

pub fn import_wlhamb(options: ImportWlhambOptions) -> Result<ImportWlhambReport> {
    let file = File::open(&options.input)
        .with_context(|| format!("open WLHAMB {}", options.input.display()))?;
    let reader = BufReader::new(file);
    let locale = options.locale.unwrap_or_else(|| "und".to_string());
    let lang = locale
        .split(['-', '_'])
        .next()
        .unwrap_or("und")
        .to_ascii_lowercase();
    let source = options
        .source
        .unwrap_or_else(|| source_name(&options.input));

    let mut records = Vec::new();
    let mut skipped_invalid = 0usize;
    for (idx, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read WLHAMB line {}", idx + 1))?;
        let hyphenated = line.trim();
        if hyphenated.is_empty() || hyphenated.starts_with('#') {
            continue;
        }

        let word = strip_separators(hyphenated);
        let breaks = match hyphenated_to_breaks(&word, hyphenated) {
            Ok(breaks) => breaks,
            Err(error) if options.skip_invalid => {
                skipped_invalid += 1;
                let _ = error;
                continue;
            }
            Err(error) => {
                return Err(error).with_context(|| format!("convert WLHAMB line {}", idx + 1))
            }
        };
        let id = format!("{}:{:06}", source, records.len() + 1);

        records.push(HyphenationRecord {
            id,
            lang: lang.clone(),
            locale: Some(locale.clone()),
            word: word.clone(),
            hyphenated: insert_separator(&word, &breaks, "-"),
            breaks,
            source: source.clone(),
            license: options.license.clone(),
            normalization: Normalization::Nfc,
            ambiguous: false,
            variants: Vec::new(),
            notes: Vec::new(),
        });
    }

    let records = write_records(options.output, records)?;
    Ok(ImportWlhambReport {
        records,
        skipped_invalid,
    })
}

fn source_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("wlhamb")
        .to_string()
}
