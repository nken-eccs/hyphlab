use crate::write_records;
use anyhow::{Context, Result};
use hyph_core::{insert_separator, BoundaryMap, GraphemeIndex, HyphenationRecord, Normalization};
use smallvec::SmallVec;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct ImportMobyOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub locale: Option<String>,
    pub source: Option<String>,
    pub license: Option<String>,
    pub separator: u8,
}

impl Default for ImportMobyOptions {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: PathBuf::new(),
            locale: None,
            source: None,
            license: None,
            separator: 0xA5,
        }
    }
}

pub fn import_moby(options: ImportMobyOptions) -> Result<usize> {
    let bytes =
        fs::read(&options.input).with_context(|| format!("read {}", options.input.display()))?;
    let locale = options.locale.unwrap_or_else(|| "en-US".to_string());
    let lang = locale
        .split(['-', '_'])
        .next()
        .unwrap_or("und")
        .to_ascii_lowercase();
    let source = options
        .source
        .unwrap_or_else(|| source_name(&options.input));

    let mut records = Vec::<HyphenationRecord>::new();
    let mut record_index_by_word = HashMap::<String, usize>::new();
    for (idx, raw_line) in bytes.split(|byte| *byte == b'\n').enumerate() {
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.iter().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }

        let pieces = line
            .split(|byte| *byte == options.separator)
            .filter(|part| !part.is_empty())
            .map(decode_moby_bytes)
            .collect::<Vec<_>>();
        if pieces.is_empty() {
            continue;
        }

        let word = pieces.join("");
        let breaks = breaks_from_pieces(&word, &pieces)
            .with_context(|| format!("convert Moby line {}", idx + 1))?;
        if let Some(record_idx) = record_index_by_word.get(&word).copied() {
            merge_moby_variant(&mut records[record_idx], breaks);
        } else {
            let id = format!("{}:{:06}", source, records.len() + 1);
            record_index_by_word.insert(word.clone(), records.len());
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
    }

    write_records(options.output, records)
}

fn merge_moby_variant(record: &mut HyphenationRecord, breaks: SmallVec<[GraphemeIndex; 8]>) {
    if record.breaks == breaks || record.variants.iter().any(|variant| variant == &breaks) {
        return;
    }
    if record.variants.is_empty() {
        record.variants.push(record.breaks.clone());
    }
    record.variants.push(breaks);
    record.ambiguous = true;
    record.notes.push("duplicate_word_variant".to_string());
}

fn source_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("moby")
        .to_string()
}

fn decode_moby_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn breaks_from_pieces(word: &str, pieces: &[String]) -> Result<SmallVec<[GraphemeIndex; 8]>> {
    let map = BoundaryMap::new(word);
    let mut breaks = SmallVec::<[GraphemeIndex; 8]>::new();
    let mut byte = 0usize;

    for piece in pieces.iter().take(pieces.len().saturating_sub(1)) {
        byte += piece.len();
        let Some(idx) = map.byte_to_grapheme_break(byte) else {
            anyhow::bail!("separator falls inside grapheme in {word:?}");
        };
        breaks.push(idx);
    }

    Ok(breaks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_records;
    use std::fs;

    #[test]
    fn imports_moby_separator_fixture() {
        let dir = std::env::temp_dir().join(format!("hyphlab-moby-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let input = dir.join("moby.txt");
        let output = dir.join("moby.jsonl");
        fs::write(&input, b"hy\xa5phen\xa5ation\nabout\n").unwrap();

        let count = import_moby(ImportMobyOptions {
            input,
            output: output.clone(),
            locale: Some("en-US".to_string()),
            source: Some("moby-test".to_string()),
            license: None,
            separator: 0xA5,
        })
        .unwrap();

        assert_eq!(count, 2);
        let records = read_records(output).unwrap();
        assert_eq!(records[0].word, "hyphenation");
        assert_eq!(records[0].breaks.as_slice(), &[2, 6]);
        assert!(records[1].breaks.is_empty());
    }

    #[test]
    fn merges_duplicate_moby_words_into_variants() {
        let dir =
            std::env::temp_dir().join(format!("hyphlab-moby-variant-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let input = dir.join("moby.txt");
        let output = dir.join("moby.jsonl");
        fs::write(&input, b"Aar\xa5on\nAa\xa5ron\nabout\n").unwrap();

        let count = import_moby(ImportMobyOptions {
            input,
            output: output.clone(),
            locale: Some("en-US".to_string()),
            source: Some("moby-test".to_string()),
            license: None,
            separator: 0xA5,
        })
        .unwrap();

        assert_eq!(count, 2);
        let records = read_records(output).unwrap();
        assert_eq!(records[0].word, "Aaron");
        assert_eq!(records[0].breaks.as_slice(), &[3]);
        assert!(records[0].ambiguous);
        assert_eq!(records[0].variants.len(), 2);
        assert_eq!(records[0].variants[0].as_slice(), &[3]);
        assert_eq!(records[0].variants[1].as_slice(), &[2]);
    }
}
