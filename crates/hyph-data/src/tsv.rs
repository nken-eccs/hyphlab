use crate::write_records;
use anyhow::{Context, Result};
use hyph_core::{hyphenated_to_breaks, insert_separator, HyphenationRecord, Normalization};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ImportTsvOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub locale: Option<String>,
    pub source: Option<String>,
    pub license: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TsvRow {
    word: String,
    hyphenated: String,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    locale: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

pub fn import_tsv(options: ImportTsvOptions) -> Result<usize> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(&options.input)
        .with_context(|| format!("open TSV {}", options.input.display()))?;

    let mut records = Vec::new();

    for (idx, row) in reader.deserialize::<TsvRow>().enumerate() {
        let row = row.with_context(|| format!("parse TSV row {}", idx + 2))?;
        let locale = row
            .locale
            .clone()
            .or_else(|| options.locale.clone())
            .or_else(|| row.lang.clone())
            .unwrap_or_else(|| "und".to_string());
        let lang = row
            .lang
            .clone()
            .or_else(|| locale.split(['-', '_']).next().map(str::to_string))
            .unwrap_or_else(|| "und".to_string());
        let source = row
            .source
            .clone()
            .or_else(|| options.source.clone())
            .unwrap_or_else(|| "tsv".to_string());
        let license = row.license.clone().or_else(|| options.license.clone());
        let id = row
            .id
            .clone()
            .unwrap_or_else(|| format!("{}:{:06}", source, idx + 1));
        let breaks = hyphenated_to_breaks(&row.word, &row.hyphenated)
            .with_context(|| format!("convert hyphenation for word {:?}", row.word))?;

        let mut record = HyphenationRecord {
            id,
            lang: lang.to_ascii_lowercase(),
            locale: Some(locale),
            word: row.word,
            hyphenated: row.hyphenated,
            breaks,
            source,
            license,
            normalization: Normalization::Nfc,
            ambiguous: false,
            variants: Vec::new(),
            notes: Vec::new(),
        };

        record.hyphenated = insert_separator(&record.word, &record.breaks, "-");
        records.push(record);
    }

    write_records(&options.output, records)
}
