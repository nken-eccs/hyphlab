use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ProperNameGuardConfig {
    pub(crate) path: Option<PathBuf>,
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) matching: ProperNameMatching,
}

impl Default for ProperNameGuardConfig {
    fn default() -> Self {
        Self {
            path: None,
            paths: Vec::new(),
            matching: ProperNameMatching::CaseInsensitive,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProperNameMatching {
    CaseInsensitive,
    CaseSensitive,
}

impl Default for ProperNameMatching {
    fn default() -> Self {
        Self::CaseInsensitive
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProperNameGuard {
    matching: ProperNameMatching,
    names: HashSet<String>,
}

impl ProperNameGuard {
    pub(crate) fn load(config: ProperNameGuardConfig) -> Result<Self> {
        let mut names = HashSet::new();

        let paths = config.configured_paths();
        ensure!(
            !paths.is_empty(),
            "[proper_names] requires `path` or `paths`"
        );
        for path in paths {
            load_names(path, config.matching, &mut names)?;
        }

        Ok(Self {
            matching: config.matching,
            names,
        })
    }

    pub(crate) fn needs_folded_token(&self) -> bool {
        matches!(self.matching, ProperNameMatching::CaseInsensitive)
    }

    pub(crate) fn protected_reason(
        &self,
        token: &str,
        folded_token: Option<&str>,
    ) -> Option<&'static str> {
        let protected = match self.matching {
            ProperNameMatching::CaseSensitive => self.names.contains(token),
            ProperNameMatching::CaseInsensitive => folded_token
                .map(|folded| self.names.contains(folded))
                .unwrap_or_else(|| self.names.contains(&token.to_lowercase())),
        };
        protected.then_some("proper_name")
    }

    pub(crate) fn protects_lowercase_token(&self, token: &str) -> bool {
        match self.matching {
            ProperNameMatching::CaseSensitive => self.names.contains(token),
            ProperNameMatching::CaseInsensitive => self.names.contains(token),
        }
    }
}

impl ProperNameGuardConfig {
    fn configured_paths(&self) -> Vec<&PathBuf> {
        let mut paths = Vec::new();
        if let Some(path) = &self.path {
            paths.push(path);
        }
        paths.extend(self.paths.iter());
        paths
    }
}

fn load_names(
    path: &Path,
    matching: ProperNameMatching,
    names: &mut HashSet<String>,
) -> Result<()> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        let value = line.split('#').next().unwrap_or("").trim();
        if value.is_empty() {
            continue;
        }
        let value = match matching {
            ProperNameMatching::CaseSensitive => value.to_string(),
            ProperNameMatching::CaseInsensitive => value.to_lowercase(),
        };
        names.insert(value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_layered_name_lists() {
        let dir = std::env::temp_dir();
        let stem = format!("hyphlab-names-{}", std::process::id());
        let base = dir.join(format!("{stem}-base.txt"));
        let house = dir.join(format!("{stem}-house.txt"));
        std::fs::write(&base, "McDonald\n").unwrap();
        std::fs::write(&house, "MyProductName\n").unwrap();

        let guard = ProperNameGuard::load(ProperNameGuardConfig {
            path: Some(base.clone()),
            paths: vec![house.clone()],
            matching: ProperNameMatching::CaseInsensitive,
        })
        .unwrap();

        assert_eq!(
            guard.protected_reason("mcdonald", Some("mcdonald")),
            Some("proper_name")
        );
        assert_eq!(
            guard.protected_reason("MyProductName", Some("myproductname")),
            Some("proper_name")
        );
        assert_eq!(guard.protected_reason("Other", Some("other")), None);

        let _ = std::fs::remove_file(base);
        let _ = std::fs::remove_file(house);
    }
}
