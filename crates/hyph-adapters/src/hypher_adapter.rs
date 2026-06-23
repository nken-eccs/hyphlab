use crate::MethodAdapter;
use anyhow::{Context, Result};
use hyph_core::{BoundaryMap, GraphemeIndex, HyphenationConfig, LanguageTag};
use hypher::Lang;
use smallvec::SmallVec;

#[derive(Debug, Clone)]
pub struct HypherAdapter {
    language: LanguageTag,
    lang: Lang,
    config: HyphenationConfig,
    id: String,
}

impl HypherAdapter {
    pub fn for_locale(locale: &str) -> Result<Self> {
        let language: LanguageTag = locale
            .parse()
            .map_err(|e: String| anyhow::anyhow!("invalid locale {locale:?}: {e}"))?;
        let lang = hypher_lang_for_tag(&language)
            .with_context(|| format!("hypher does not support locale {locale:?} in this build"))?;
        let (left_min, right_min) = lang.bounds();
        let config = HyphenationConfig {
            left_min,
            right_min,
            ..HyphenationConfig::default()
        };
        let id = format!("hypher-0.1.7-{}", language.language);

        Ok(Self {
            language,
            lang,
            config,
            id,
        })
    }

    pub fn native_segments<'a>(&self, word: &'a str) -> impl Iterator<Item = &'a str> {
        hypher::hyphenate_bounded(word, self.lang, self.config.left_min, self.config.right_min)
    }
}

impl MethodAdapter for HypherAdapter {
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
        let map = BoundaryMap::new(word);
        let lengths = self.native_segments(word).map(str::len);
        map.segment_lengths_to_breaks(lengths, out)
    }
}

fn hypher_lang_for_tag(tag: &LanguageTag) -> Option<Lang> {
    match tag.language.as_str() {
        "en" => Some(Lang::English),
        "de" => Some(Lang::German),
        "nl" => Some(Lang::Dutch),
        "cs" => Some(Lang::Czech),
        "es" => Some(Lang::Spanish),
        "it" => Some(Lang::Italian),
        "pt" => Some(Lang::Portuguese),
        "ru" => Some(Lang::Russian),
        "tr" => Some(Lang::Turkish),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphenates_english() {
        let adapter = HypherAdapter::for_locale("en-US").unwrap();
        let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
        adapter.hyphenate_into("extensive", &mut out).unwrap();
        assert!(!out.is_empty());
    }
}
