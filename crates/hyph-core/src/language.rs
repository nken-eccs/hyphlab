use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LanguageTag {
    pub language: String,
    pub region: Option<String>,
}

impl LanguageTag {
    pub fn new(language: impl Into<String>, region: Option<impl Into<String>>) -> Self {
        Self {
            language: language.into().to_ascii_lowercase(),
            region: region.map(|r| r.into().to_ascii_uppercase()),
        }
    }

    pub fn language_only(language: impl Into<String>) -> Self {
        Self::new(language, Option::<String>::None)
    }

    pub fn canonical(&self) -> String {
        match &self.region {
            Some(region) => format!("{}-{}", self.language, region),
            None => self.language.clone(),
        }
    }
}

impl Default for LanguageTag {
    fn default() -> Self {
        Self::new("en", Some("US"))
    }
}

impl fmt::Display for LanguageTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical())
    }
}

impl FromStr for LanguageTag {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        if input.is_empty() {
            return Err("empty language tag".to_string());
        }

        let mut parts = input.split(['-', '_']);
        let language = parts
            .next()
            .filter(|part| !part.is_empty())
            .ok_or_else(|| "missing language subtag".to_string())?;
        let region = parts.next().filter(|part| !part.is_empty());

        Ok(Self::new(language, region))
    }
}
