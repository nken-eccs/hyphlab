// Unified method workflow: train models and materialize runtime manifests.

fn cmd_method_train(args: MethodTrainArgs) -> Result<()> {
    train_method_model(args)
}

fn cmd_method_materialize(args: MethodMaterializeArgs) -> Result<()> {
    let manifest = read_methods_manifest(&args.manifest)?;
    let source_dir = args.manifest.parent().unwrap_or_else(|| Path::new("."));
    let output_dir = args.output.parent().unwrap_or_else(|| Path::new("."));

    std::fs::create_dir_all(&args.model_dir)
        .with_context(|| format!("create {}", args.model_dir.display()))?;
    create_parent(&args.output)?;

    let mut rendered = String::new();
    rendered.push_str("# Runtime manifest generated from a method manifest.\n");
    rendered.push_str(&format!("# source = {}\n", args.manifest.display()));
    rendered.push_str(&format!("# train_gold = {}\n\n", args.gold.display()));

    for method in manifest.methods {
        validate_manifest_slug(&method.slug)?;
        let runtime = if let Some(train) = &method.train {
            let kind = infer_train_kind(&method.method, train.kind.as_deref())?;
            let model_path = materialized_model_path(&args, source_dir, &method, train, kind);
            train_method_model(MethodTrainArgs {
                method: method.method.clone(),
                gold: args.gold.clone(),
                output: model_path.clone(),
                locale: args.locale.clone(),
                id: train.id.clone(),
                left_min: method.left_min,
                right_min: method.right_min,
                min_word_len: method.min_word_len,
                include_ambiguous: train.include_ambiguous,
                epochs: train.epochs.unwrap_or(5),
                learning_rate: train.learning_rate.unwrap_or(0.05),
                l2: train.l2.unwrap_or(1.0e-5),
                threshold: train.threshold.unwrap_or(0.9),
                min_n: train.min_n.unwrap_or(2),
                max_n: train.max_n.unwrap_or(5),
                limit: train.limit,
            })?;

            RuntimeManifestMethod {
                slug: method.slug.clone(),
                method: train
                    .runtime_method
                    .clone()
                    .unwrap_or_else(|| kind.default_runtime_method().to_string()),
                enabled: method.enabled,
                supports: method.supports.clone(),
                requires_feature: method.requires_feature.clone(),
                requires_patterns: method.requires_patterns,
                pass_patterns: method.pass_patterns,
                patterns: rewrite_manifest_path(source_dir, output_dir, method.patterns.as_ref())?,
                dictionary: Some(manifest_output_path(output_dir, &model_path)?),
                external_command: method.external_command.clone(),
                left_min: None,
                right_min: None,
                min_word_len: None,
            }
        } else {
            RuntimeManifestMethod::from_manifest(&method, source_dir, output_dir)?
        };
        render_runtime_manifest_method(&mut rendered, &runtime);
    }

    std::fs::write(&args.output, rendered)
        .with_context(|| format!("write {}", args.output.display()))?;
    println!("runtime_manifest: {}", args.output.display());
    Ok(())
}

fn train_method_model(args: MethodTrainArgs) -> Result<()> {
    match infer_train_kind(&args.method, None)? {
        TrainKind::SafeNgram => cmd_compile_safe_ngram(CompileSafeNgramArgs {
            gold: args.gold,
            output: args.output,
            locale: args.locale,
            method: args.method,
            left_min: args.left_min,
            right_min: args.right_min,
            min_word_len: args.min_word_len,
            include_ambiguous: args.include_ambiguous,
        }),
        TrainKind::ItalianSyllable => cmd_compile_italian_syllable(CompileItalianSyllableArgs {
            gold: args.gold,
            output: args.output,
            locale: args.locale,
            method: args.method,
            left_min: args.left_min,
            right_min: args.right_min,
            min_word_len: args.min_word_len,
            include_ambiguous: args.include_ambiguous,
        }),
        TrainKind::Crf => cmd_crf_train(CrfTrainArgs {
            gold: args.gold,
            output: args.output,
            locale: args.locale,
            id: args.id.unwrap_or_else(|| args.method.clone()),
            epochs: args.epochs,
            learning_rate: args.learning_rate,
            l2: args.l2,
            threshold: args.threshold,
            min_n: args.min_n,
            max_n: args.max_n,
            left_min: args.left_min,
            right_min: args.right_min,
            min_word_len: args.min_word_len,
            limit: args.limit,
            include_ambiguous: args.include_ambiguous,
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrainKind {
    SafeNgram,
    ItalianSyllable,
    Crf,
}

impl TrainKind {
    fn default_runtime_method(self) -> &'static str {
        match self {
            Self::SafeNgram => "safe-ngram-model",
            Self::ItalianSyllable => "italian-syllable-model",
            Self::Crf => "trogkanis-elkan-crf",
        }
    }

    fn model_extension(self) -> &'static str {
        match self {
            Self::SafeNgram => "bin",
            Self::ItalianSyllable | Self::Crf => "json",
        }
    }
}

fn infer_train_kind(method: &str, explicit: Option<&str>) -> Result<TrainKind> {
    let key = normalize_manifest_selector(explicit.unwrap_or(method));
    if key == "safe-ngram" || key == "guarded-ngram" || key.starts_with("safe-ngram-") {
        return Ok(TrainKind::SafeNgram);
    }
    if key == "italian-syllable"
        || key == "it-syllable"
        || key == "italian-onset"
        || key.starts_with("italian-syllable-")
        || key.starts_with("it-syllable-")
        || key.starts_with("italian-onset-")
        || key.starts_with("it-onset-")
    {
        return Ok(TrainKind::ItalianSyllable);
    }
    if key == "crf" || key == "trogkanis-elkan-crf" {
        return Ok(TrainKind::Crf);
    }
    anyhow::bail!(
        "method {method:?} is not trainable by `hyphlab method train`; supported families are safe-ngram, italian-syllable, and trogkanis-elkan-crf"
    )
}

fn materialized_model_path(
    args: &MethodMaterializeArgs,
    source_dir: &Path,
    method: &ManifestMethod,
    train: &ManifestTrain,
    kind: TrainKind,
) -> PathBuf {
    let Some(template) = &train.output else {
        return args
            .model_dir
            .join(format!("{}.{}", method.slug, kind.model_extension()));
    };
    let template = template.to_string_lossy();
    let expanded = template
        .replace("{model_dir}", &args.model_dir.to_string_lossy())
        .replace("{slug}", &method.slug)
        .replace("{method}", &normalize_manifest_selector(&method.method));
    let path = PathBuf::from(expanded);
    if path.is_absolute() || template.contains("{model_dir}") {
        path
    } else {
        source_dir.join(path)
    }
}

#[derive(Debug)]
struct RuntimeManifestMethod {
    slug: String,
    method: String,
    enabled: bool,
    supports: Vec<String>,
    requires_feature: Option<String>,
    requires_patterns: bool,
    pass_patterns: bool,
    patterns: Option<PathBuf>,
    dictionary: Option<PathBuf>,
    external_command: Option<String>,
    left_min: Option<usize>,
    right_min: Option<usize>,
    min_word_len: Option<usize>,
}

impl RuntimeManifestMethod {
    fn from_manifest(
        method: &ManifestMethod,
        source_dir: &Path,
        output_dir: &Path,
    ) -> Result<Self> {
        Ok(Self {
            slug: method.slug.clone(),
            method: method.method.clone(),
            enabled: method.enabled,
            supports: method.supports.clone(),
            requires_feature: method.requires_feature.clone(),
            requires_patterns: method.requires_patterns,
            pass_patterns: method.pass_patterns,
            patterns: rewrite_manifest_path(source_dir, output_dir, method.patterns.as_ref())?,
            dictionary: rewrite_manifest_path(source_dir, output_dir, method.dictionary.as_ref())?,
            external_command: method.external_command.clone(),
            left_min: method.left_min,
            right_min: method.right_min,
            min_word_len: method.min_word_len,
        })
    }
}

fn rewrite_manifest_path(
    source_dir: &Path,
    output_dir: &Path,
    path: Option<&PathBuf>,
) -> Result<Option<PathBuf>> {
    path.map(|path| {
        let resolved = resolve_manifest_path(source_dir, path);
        manifest_output_path(output_dir, &resolved)
    })
    .transpose()
}

fn manifest_output_path(output_dir: &Path, path: &Path) -> Result<PathBuf> {
    relative_path_from(output_dir, path)
}

fn relative_path_from(base_dir: &Path, target: &Path) -> Result<PathBuf> {
    let base = absolute_normal_path(base_dir)?;
    let target = absolute_normal_path(target)?;
    let base_parts = normal_components(&base);
    let target_parts = normal_components(&target);
    if base_parts.is_empty() || target_parts.is_empty() || base_parts[0] != target_parts[0] {
        return Ok(target);
    }

    let common = base_parts
        .iter()
        .zip(target_parts.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let mut out = PathBuf::new();
    for _ in common..base_parts.len() {
        out.push("..");
    }
    for part in &target_parts[common..] {
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    Ok(out)
}

fn absolute_normal_path(path: &Path) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(path
        .components()
        .fold(PathBuf::new(), |mut out, component| {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    out.pop();
                }
                other => out.push(other.as_os_str()),
            }
            out
        }))
}

fn normal_components(path: &Path) -> Vec<std::ffi::OsString> {
    path.components()
        .map(|component| component.as_os_str().to_os_string())
        .collect()
}

fn render_runtime_manifest_method(out: &mut String, method: &RuntimeManifestMethod) {
    out.push_str("[[methods]]\n");
    push_toml_string(out, "slug", &method.slug);
    push_toml_string(out, "method", &method.method);
    if !method.enabled {
        out.push_str("enabled = false\n");
    }
    if !method.supports.is_empty() {
        push_toml_string_array(out, "supports", &method.supports);
    }
    if let Some(value) = &method.requires_feature {
        push_toml_string(out, "requires_feature", value);
    }
    if method.requires_patterns {
        out.push_str("requires_patterns = true\n");
    }
    if method.pass_patterns {
        out.push_str("pass_patterns = true\n");
    }
    if let Some(path) = &method.patterns {
        push_toml_path(out, "patterns", path);
    }
    if let Some(path) = &method.dictionary {
        push_toml_path(out, "dictionary", path);
    }
    if let Some(command) = &method.external_command {
        push_toml_string(out, "external_command", command);
    }
    if let Some(value) = method.left_min {
        out.push_str(&format!("left_min = {value}\n"));
    }
    if let Some(value) = method.right_min {
        out.push_str(&format!("right_min = {value}\n"));
    }
    if let Some(value) = method.min_word_len {
        out.push_str(&format!("min_word_len = {value}\n"));
    }
    out.push('\n');
}

fn push_toml_string(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(&toml_quoted(value));
    out.push('\n');
}

fn push_toml_path(out: &mut String, key: &str, path: &Path) {
    push_toml_string(out, key, &path.to_string_lossy());
}

fn push_toml_string_array(out: &mut String, key: &str, values: &[String]) {
    out.push_str(key);
    out.push_str(" = [");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&toml_quoted(value));
    }
    out.push_str("]\n");
}

fn toml_quoted(value: &str) -> String {
    format!("{value:?}")
}
