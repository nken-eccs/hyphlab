// Adapter scaffolding and smoke-test commands.

fn cmd_dev_new_adapter(args: NewAdapterArgs) -> Result<()> {
    let module = to_snake_identifier(&args.slug);
    let method = args
        .method
        .clone()
        .unwrap_or_else(|| module.replace('_', "-"));
    let struct_name = args
        .struct_name
        .clone()
        .unwrap_or_else(|| to_pascal_identifier(&args.slug));
    let root = args.root.clone();
    let adapter_path = root
        .join("crates")
        .join("hyph-adapters")
        .join("src")
        .join(format!("{module}.rs"));
    let adapters_lib_path = root
        .join("crates")
        .join("hyph-adapters")
        .join("src")
        .join("lib.rs");
    let manifest_path = if args.manifest.is_absolute() {
        args.manifest.clone()
    } else {
        root.join(&args.manifest)
    };

    let adapter_source = render_adapter_template(&module, &method, &struct_name);
    let manifest_entry = render_manifest_entry(
        &args.slug,
        &method,
        &args.supports,
        args.requires_patterns,
        args.pass_patterns,
        args.requires_feature.as_deref(),
    );

    if args.dry_run {
        println!("adapter: {}", adapter_path.display());
        println!("{adapter_source}");
        println!("manifest: {}", manifest_path.display());
        println!("{manifest_entry}");
        return Ok(());
    }

    if adapter_path.exists() && !args.force {
        anyhow::bail!(
            "{} already exists; pass --force to replace it",
            adapter_path.display()
        );
    }
    create_parent(&adapter_path)?;
    std::fs::write(&adapter_path, adapter_source)
        .with_context(|| format!("write {}", adapter_path.display()))?;

    update_adapter_registry(&adapters_lib_path, &module, &method, &struct_name)?;
    append_manifest_entry(&manifest_path, &args.slug, &method, &manifest_entry)?;

    println!("created {}", adapter_path.display());
    println!("updated {}", adapters_lib_path.display());
    println!("updated {}", manifest_path.display());
    println!();
    println!("next:");
    println!("  cargo fmt --all");
    println!("  cargo check -p hyph-cli --features adapters-hyphenation-embedded");
    println!("  cargo run -p hyph-cli -- dev smoke {}", args.slug);
    Ok(())
}

fn cmd_dev_smoke(args: SmokeArgs) -> Result<()> {
    cmd_matrix(MatrixArgs {
        manifest: args.manifest,
        gold: args.gold,
        locale: args.locale,
        patterns: Some(args.patterns),
        output_dir: args.output_dir.join(&args.slug),
        iterations: args.iterations,
        init_iterations: args.init_iterations,
        init_warmup: 0,
        ambiguous: AmbiguousPolicyArg::Exclude,
        only: vec![args.slug],
        abort_method_errors: false,
    })
}


fn render_adapter_template(module: &str, method: &str, struct_name: &str) -> String {
    format!(
        r#"use crate::MethodAdapter;
use anyhow::Result;
use hyph_core::{{GraphemeIndex, HyphenationConfig, LanguageTag}};
use smallvec::SmallVec;

#[derive(Debug, Clone)]
pub struct {struct_name} {{
    language: LanguageTag,
    config: HyphenationConfig,
    id: String,
}}

impl {struct_name} {{
    pub fn new(language: LanguageTag) -> Self {{
        let id = format!("{method}-{{}}", language.language);
        Self {{
            language,
            config: HyphenationConfig::default(),
            id,
        }}
    }}
}}

impl MethodAdapter for {struct_name} {{
    fn id(&self) -> &str {{
        &self.id
    }}

    fn language(&self) -> &LanguageTag {{
        &self.language
    }}

    fn config(&self) -> &HyphenationConfig {{
        &self.config
    }}

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {{
        out.clear();
        let _ = word;
        Ok(())
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn {module}_returns_sorted_breaks() {{
        let adapter = {struct_name}::new("en-US".parse().unwrap());
        let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
        adapter.hyphenate_into("hyphenation", &mut out).unwrap();
        assert!(out.windows(2).all(|pair| pair[0] < pair[1]));
    }}
}}
"#
    )
}

fn render_manifest_entry(
    slug: &str,
    method: &str,
    supports: &[String],
    requires_patterns: bool,
    pass_patterns: bool,
    requires_feature: Option<&str>,
) -> String {
    let mut out = format!("\n[[methods]]\nslug = {slug:?}\nmethod = {method:?}\n");
    if !supports.is_empty() {
        out.push_str("supports = [");
        for (index, locale) in supports.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("{locale:?}"));
        }
        out.push_str("]\n");
    }
    if let Some(feature) = requires_feature {
        out.push_str(&format!("requires_feature = {feature:?}\n"));
    }
    if requires_patterns {
        out.push_str("requires_patterns = true\n");
    }
    if pass_patterns {
        out.push_str("pass_patterns = true\n");
    }
    out
}

fn update_adapter_registry(
    lib_path: &Path,
    module: &str,
    method: &str,
    struct_name: &str,
) -> Result<()> {
    let factory = format!("{module}_factory");
    let mut text = std::fs::read_to_string(lib_path)
        .with_context(|| format!("read {}", lib_path.display()))?;
    text = insert_after_marker_once(
        text,
        "// hyphlab:adapter-modules",
        &format!("mod {module};"),
        &format!("module {module}"),
    )?;
    text = insert_after_marker_once(
        text,
        "// hyphlab:adapter-exports",
        &format!("pub use {module}::{struct_name};"),
        &format!("export {struct_name}"),
    )?;
    text = insert_after_marker_once(
        text,
        "// hyphlab:adapter-registrations",
        &format!(
            "        AdapterRegistration {{\n            names: &[{method:?}],\n            factory: {factory},\n        }},"
        ),
        &format!("registration {method}"),
    )?;
    text = insert_after_marker_once(
        text,
        "// hyphlab:adapter-factories",
        &format!(
            "fn {factory}(locale: &str) -> Result<Box<dyn MethodAdapter>> {{\n    Ok(Box::new({struct_name}::new(locale.parse().unwrap_or_default())))\n}}\n"
        ),
        &format!("factory {factory}"),
    )?;
    std::fs::write(lib_path, text).with_context(|| format!("write {}", lib_path.display()))?;
    Ok(())
}

fn insert_after_marker_once(
    text: String,
    marker: &str,
    insertion: &str,
    label: &str,
) -> Result<String> {
    if text.contains(insertion.trim()) {
        return Ok(text);
    }
    let marker_index = text
        .find(marker)
        .with_context(|| format!("missing scaffold marker {marker:?} for {label}"))?;
    let line_end = text[marker_index..]
        .find('\n')
        .map(|offset| marker_index + offset + 1)
        .unwrap_or(text.len());
    let mut updated = String::with_capacity(text.len() + insertion.len() + 1);
    updated.push_str(&text[..line_end]);
    updated.push_str(insertion);
    updated.push('\n');
    updated.push_str(&text[line_end..]);
    Ok(updated)
}

fn append_manifest_entry(path: &Path, slug: &str, method: &str, entry: &str) -> Result<()> {
    let mut text =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if manifest_contains_key(&text, "slug", slug) || manifest_contains_key(&text, "method", method)
    {
        println!(
            "manifest already contains slug {slug:?} or method {method:?}; leaving it unchanged"
        );
        return Ok(());
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(entry);
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn manifest_contains_key(text: &str, key: &str, value: &str) -> bool {
    let expected = format!("{key} = {value:?}");
    text.lines().any(|line| line.trim() == expected)
}

fn to_snake_identifier(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_underscore = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_underscore = false;
        } else if !last_was_underscore && !out.is_empty() {
            out.push('_');
            last_was_underscore = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("method");
    }
    if out
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        out.insert_str(0, "method_");
    }
    out
}

fn to_pascal_identifier(value: &str) -> String {
    let snake = to_snake_identifier(value);
    let mut out = String::new();
    for part in snake.split('_').filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    if out
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        out.insert_str(0, "Method");
    }
    if out.is_empty() {
        out.push_str("Method");
    }
    out
}
