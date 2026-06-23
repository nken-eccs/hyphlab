use crate::MethodAdapter;
use anyhow::{Context, Result};
use hyph_core::{BoundaryMap, GraphemeIndex, HyphenationConfig, LanguageTag};
use hyphenation::extended::Extended;
use hyphenation::{Hyphenator as _, Language, Load, Standard};
use smallvec::SmallVec;
use std::path::Path;

pub struct HyphenationCrateAdapter {
    language: LanguageTag,
    dictionary: HyphenationDictionary,
    config: HyphenationConfig,
    id: String,
}

enum HyphenationDictionary {
    Standard(Standard),
    Extended(Extended),
}

impl HyphenationCrateAdapter {
    pub fn from_path(locale: &str, path: impl AsRef<Path>) -> Result<Self> {
        let language: LanguageTag = locale
            .parse()
            .map_err(|e: String| anyhow::anyhow!("invalid locale {locale:?}: {e}"))?;
        let hyphenation_lang = hyphenation_language_for_tag(&language)
            .with_context(|| format!("hyphenation crate does not support locale {locale:?}"))?;
        let dictionary = Standard::from_path(hyphenation_lang, path)
            .context("load hyphenation crate dictionary")?;
        Ok(Self::new(
            language,
            HyphenationDictionary::Standard(dictionary),
            "hyphenation-0.8.4-standard-runtime",
        ))
    }

    pub fn from_extended_path(locale: &str, path: impl AsRef<Path>) -> Result<Self> {
        let language: LanguageTag = locale
            .parse()
            .map_err(|e: String| anyhow::anyhow!("invalid locale {locale:?}: {e}"))?;
        let hyphenation_lang = hyphenation_language_for_tag(&language)
            .with_context(|| format!("hyphenation crate does not support locale {locale:?}"))?;
        let dictionary = Extended::from_path(hyphenation_lang, path)
            .context("load extended hyphenation crate dictionary")?;
        Ok(Self::new(
            language,
            HyphenationDictionary::Extended(dictionary),
            "hyphenation-0.8.4-extended-runtime",
        ))
    }

    #[cfg(feature = "rust-hyphenation-embedded")]
    pub fn embedded_en_us() -> Result<Self> {
        let language: LanguageTag = "en-US".parse().unwrap();
        let dictionary = Standard::from_embedded(Language::EnglishUS)
            .context("load embedded hyphenation en-US dictionary")?;
        Ok(Self::new(
            language,
            HyphenationDictionary::Standard(dictionary),
            "hyphenation-0.8.4-embedded-en-us",
        ))
    }

    fn new(language: LanguageTag, dictionary: HyphenationDictionary, id: &str) -> Self {
        let (left_min, right_min) = match &dictionary {
            HyphenationDictionary::Standard(dictionary) => dictionary.unbreakable_chars(),
            HyphenationDictionary::Extended(dictionary) => dictionary.unbreakable_chars(),
        };
        let config = HyphenationConfig {
            left_min,
            right_min,
            ..HyphenationConfig::default()
        };
        Self {
            language,
            dictionary,
            config,
            id: id.to_string(),
        }
    }

    pub fn native_breaks(&self, word: &str) -> Vec<usize> {
        match &self.dictionary {
            HyphenationDictionary::Standard(dictionary) => dictionary.hyphenate(word).breaks,
            HyphenationDictionary::Extended(dictionary) => dictionary
                .hyphenate(word)
                .breaks
                .into_iter()
                .map(|(byte, _subregion)| byte)
                .collect(),
        }
    }
}

impl MethodAdapter for HyphenationCrateAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn language(&self) -> &LanguageTag {
        &self.language
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        let map = BoundaryMap::new(word);
        for byte in self.native_breaks(word) {
            if let Some(idx) = map.byte_to_grapheme_break(byte) {
                out.push(idx);
            }
        }
        Ok(())
    }
}

fn hyphenation_language_for_tag(tag: &LanguageTag) -> Option<Language> {
    match (tag.language.as_str(), tag.region.as_deref()) {
        ("en", Some("GB")) => Some(Language::EnglishGB),
        ("en", _) => Some(Language::EnglishUS),
        ("de", _) => Some(Language::German1996),
        ("nl", _) => Some(Language::Dutch),
        ("cs", _) => Some(Language::Czech),
        ("es", _) => Some(Language::Spanish),
        ("it", _) => Some(Language::Italian),
        ("pt", _) => Some(Language::Portuguese),
        ("ru", _) => Some(Language::Russian),
        ("tr", _) => Some(Language::Turkish),
        _ => None,
    }
}
