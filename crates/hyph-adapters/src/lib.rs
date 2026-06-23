mod hypher_adapter;
mod no_hyphen;
// hyphlab:adapter-modules

#[cfg(feature = "rust-hyphenation")]
mod rust_hyphenation;

pub use hypher_adapter::HypherAdapter;
pub use no_hyphen::NoHyphen;
// hyphlab:adapter-exports

#[cfg(feature = "rust-hyphenation")]
pub use rust_hyphenation::HyphenationCrateAdapter;

use anyhow::Result;
use hyph_core::{GraphemeIndex, HyphenationConfig, LanguageTag};
use smallvec::SmallVec;

pub trait MethodAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn language(&self) -> &LanguageTag;
    fn config(&self) -> &HyphenationConfig;
    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()>;
}

pub struct AdapterRegistration {
    pub names: &'static [&'static str],
    pub factory: fn(&str) -> Result<Box<dyn MethodAdapter>>,
}

impl AdapterRegistration {
    fn matches(&self, method: &str) -> bool {
        self.names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(method))
    }
}

pub fn native_adapter_registry() -> &'static [AdapterRegistration] {
    &[
        AdapterRegistration {
            names: &["no-hyphen", "none"],
            factory: no_hyphen_factory,
        },
        AdapterRegistration {
            names: &["hypher"],
            factory: hypher_factory,
        },
        // hyphlab:adapter-registrations
        #[cfg(feature = "rust-hyphenation-embedded")]
        AdapterRegistration {
            names: &["hyphenation-embedded"],
            factory: hyphenation_embedded_factory,
        },
    ]
}

pub fn adapter_for_method(method: &str, locale: &str) -> Result<Box<dyn MethodAdapter>> {
    for registration in native_adapter_registry() {
        if registration.matches(method) {
            return (registration.factory)(locale);
        }
    }

    #[cfg(not(feature = "rust-hyphenation-embedded"))]
    if method.eq_ignore_ascii_case("hyphenation-embedded") {
        anyhow::bail!(
            "method `hyphenation-embedded` requires feature `adapters-hyphenation-embedded`"
        );
    }

    anyhow::bail!(
        "unknown adapter method {method:?}; add a MethodAdapter registration or use external-jsonl"
    )
}

fn no_hyphen_factory(locale: &str) -> Result<Box<dyn MethodAdapter>> {
    Ok(Box::new(NoHyphen::new(locale.parse().unwrap_or_default())))
}

fn hypher_factory(locale: &str) -> Result<Box<dyn MethodAdapter>> {
    Ok(Box::new(HypherAdapter::for_locale(locale)?))
}

#[cfg(feature = "rust-hyphenation-embedded")]
fn hyphenation_embedded_factory(_locale: &str) -> Result<Box<dyn MethodAdapter>> {
    Ok(Box::new(HyphenationCrateAdapter::embedded_en_us()?))
}

// hyphlab:adapter-factories
