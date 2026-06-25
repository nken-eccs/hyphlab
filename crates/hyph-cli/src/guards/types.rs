use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use super::{CaseGuard, CaseGuardConfig, ProperNameGuard, ProperNameGuardConfig};
use crate::{load_sensitive_fragments, SensitiveFragmentRules, TypesetFragmentFilter};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct FragmentGuardConfig {
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct GuardPolicyFile {
    pub(crate) fragments: Option<FragmentGuardConfig>,
    pub(crate) case: Option<CaseGuardConfig>,
    pub(crate) proper_names: Option<ProperNameGuardConfig>,
}

pub(crate) struct GuardPolicySet {
    pub(crate) fragment_rules: Option<SensitiveFragmentRules>,
    pub(crate) fragment_filter: Option<TypesetFragmentFilter>,
    pub(crate) case: Option<CaseGuard>,
    pub(crate) proper_names: Option<ProperNameGuard>,
}

impl GuardPolicySet {
    pub(crate) fn empty() -> Self {
        Self {
            fragment_rules: None,
            fragment_filter: None,
            case: None,
            proper_names: None,
        }
    }

    pub(crate) fn from_options(
        locale: &str,
        policy_path: Option<&PathBuf>,
        fragment_path: Option<&PathBuf>,
    ) -> Result<Self> {
        let mut out = if let Some(path) = policy_path {
            Self::load_policy(path, locale)?
        } else {
            Self::empty()
        };
        if let Some(path) = fragment_path {
            out.set_fragments(path)?;
        }
        Ok(out)
    }

    pub(crate) fn load_policy(path: &Path, locale: &str) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut file: GuardPolicyFile =
            toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        if let Some(fragments) = &mut file.fragments {
            fragments.path = resolve_policy_path(base, &fragments.path);
        }
        if let Some(proper_names) = &mut file.proper_names {
            if let Some(path) = &mut proper_names.path {
                *path = resolve_policy_path(base, path);
            }
            for path in &mut proper_names.paths {
                *path = resolve_policy_path(base, path);
            }
        }
        Self::from_policy_file(file, locale)
    }

    fn from_policy_file(file: GuardPolicyFile, locale: &str) -> Result<Self> {
        let mut out = Self::empty();
        if let Some(fragments) = file.fragments {
            out.set_fragments(&fragments.path)?;
        }
        if let Some(case) = file.case {
            out.case = Some(CaseGuard::new(case, locale));
        }
        if let Some(proper_names) = file.proper_names {
            out.proper_names = Some(ProperNameGuard::load(proper_names)?);
        }
        Ok(out)
    }

    pub(crate) fn set_fragments(&mut self, path: &Path) -> Result<()> {
        let rules = load_sensitive_fragments(path)?;
        self.fragment_filter = Some(TypesetFragmentFilter::new(rules.clone()));
        self.fragment_rules = Some(rules);
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.fragment_filter.is_none() && self.case.is_none() && self.proper_names.is_none()
    }

    pub(crate) fn id_suffix(&self) -> String {
        let mut parts = Vec::new();
        if self.fragment_filter.is_some() {
            parts.push("fragments");
        }
        if self.case.is_some() {
            parts.push("case");
        }
        if self.proper_names.is_some() {
            parts.push("proper");
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join("+")
        }
    }
}

fn resolve_policy_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}
