use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct CaseGuardConfig {
    pub(crate) protect_titlecase: bool,
    pub(crate) protect_mixed_case: bool,
    pub(crate) protect_all_caps: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct CaseGuard {
    config: CaseGuardConfig,
    locale: String,
}

impl CaseGuard {
    pub(crate) fn new(config: CaseGuardConfig, locale: &str) -> Self {
        Self {
            config,
            locale: locale.to_ascii_lowercase(),
        }
    }

    pub(crate) fn protected_reason(&self, token: &str) -> Option<&'static str> {
        match classify_case(token, &self.locale) {
            CaseShape::Titlecase if self.config.protect_titlecase => Some("case_titlecase"),
            CaseShape::Mixed if self.config.protect_mixed_case => Some("case_mixed"),
            CaseShape::AllCaps if self.config.protect_all_caps => Some("case_all_caps"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaseShape {
    Lower,
    Titlecase,
    Mixed,
    AllCaps,
    Uncased,
}

fn classify_case(token: &str, locale: &str) -> CaseShape {
    let mut cased = Vec::new();
    for ch in token.chars().filter(|ch| ch.is_alphabetic()) {
        let is_upper = ch.is_uppercase();
        let is_lower = ch.is_lowercase();
        if is_upper || is_lower {
            cased.push((ch, is_upper, is_lower));
        }
    }

    if cased.is_empty() {
        return CaseShape::Uncased;
    }

    let has_upper = cased.iter().any(|(_, upper, _)| *upper);
    let has_lower = cased.iter().any(|(_, _, lower)| *lower);
    if has_upper && !has_lower {
        return CaseShape::AllCaps;
    }
    if has_lower && !has_upper {
        return CaseShape::Lower;
    }

    if is_locale_titlecase(&cased, locale) {
        CaseShape::Titlecase
    } else {
        CaseShape::Mixed
    }
}

fn is_locale_titlecase(cased: &[(char, bool, bool)], locale: &str) -> bool {
    if locale.starts_with("nl") && is_dutch_ij_titlecase(cased) {
        return true;
    }
    cased.first().is_some_and(|(_, upper, _)| *upper)
        && cased.iter().skip(1).all(|(_, _, lower)| *lower)
}

fn is_dutch_ij_titlecase(cased: &[(char, bool, bool)]) -> bool {
    cased.len() >= 3
        && matches!(cased[0], ('I' | 'Ĳ', true, _))
        && matches!(cased[1], ('J', true, _))
        && cased.iter().skip(2).all(|(_, _, lower)| *lower)
}
