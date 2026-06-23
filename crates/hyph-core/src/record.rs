use crate::{insert_separator, GraphemeIndex};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Normalization {
    Raw,
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
}

impl Default for Normalization {
    fn default() -> Self {
        Self::Nfc
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HyphenationRecord {
    pub id: String,
    pub lang: String,
    pub locale: Option<String>,
    pub word: String,
    pub breaks: SmallVec<[GraphemeIndex; 8]>,
    pub hyphenated: String,
    pub source: String,
    pub license: Option<String>,
    pub normalization: Normalization,
    pub ambiguous: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<SmallVec<[GraphemeIndex; 8]>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl HyphenationRecord {
    pub fn new(
        id: impl Into<String>,
        locale: impl Into<String>,
        word: impl Into<String>,
        breaks: SmallVec<[GraphemeIndex; 8]>,
        source: impl Into<String>,
    ) -> Self {
        let locale = locale.into();
        let lang = locale
            .split(['-', '_'])
            .next()
            .unwrap_or(locale.as_str())
            .to_ascii_lowercase();
        let word = word.into();
        let hyphenated = insert_separator(&word, &breaks, "-");

        Self {
            id: id.into(),
            lang,
            locale: Some(locale),
            word,
            breaks,
            hyphenated,
            source: source.into(),
            license: None,
            normalization: Normalization::Nfc,
            ambiguous: false,
            variants: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn grapheme_len(&self) -> usize {
        unicode_segmentation::UnicodeSegmentation::graphemes(self.word.as_str(), true).count()
    }
}
