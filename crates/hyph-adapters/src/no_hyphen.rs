use crate::MethodAdapter;
use anyhow::Result;
use hyph_core::{GraphemeIndex, HyphenationConfig, LanguageTag};
use smallvec::SmallVec;

#[derive(Debug, Clone)]
pub struct NoHyphen {
    language: LanguageTag,
    config: HyphenationConfig,
}

impl NoHyphen {
    pub fn new(language: LanguageTag) -> Self {
        Self {
            language,
            config: HyphenationConfig::default(),
        }
    }
}

impl MethodAdapter for NoHyphen {
    fn id(&self) -> &str {
        "no-hyphen"
    }

    fn language(&self) -> &LanguageTag {
        &self.language
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, _word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        Ok(())
    }
}
