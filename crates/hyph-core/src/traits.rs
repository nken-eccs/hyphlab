use crate::{LanguageTag, Normalization};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

pub type GraphemeIndex = u16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hyphenation {
    pub word: String,
    pub breaks: SmallVec<[GraphemeIndex; 8]>,
    pub confidence: Option<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HyphenationConfig {
    pub left_min: usize,
    pub right_min: usize,
    pub min_word_len: usize,
    pub normalization: Normalization,
    pub allow_nonstandard: bool,
}

impl Default for HyphenationConfig {
    fn default() -> Self {
        Self {
            left_min: 2,
            right_min: 3,
            min_word_len: 5,
            normalization: Normalization::Nfc,
            allow_nonstandard: false,
        }
    }
}

pub trait Hyphenator: Send + Sync {
    fn id(&self) -> &str;
    fn language(&self) -> &LanguageTag;
    fn config(&self) -> &HyphenationConfig;
    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()>;

    fn hyphenate(&self, word: &str) -> Result<Hyphenation> {
        let mut breaks = SmallVec::new();
        self.hyphenate_into(word, &mut breaks)?;
        Ok(Hyphenation {
            word: word.to_string(),
            breaks,
            confidence: None,
        })
    }
}
