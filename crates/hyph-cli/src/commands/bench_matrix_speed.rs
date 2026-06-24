// Init benchmark, matrix runner, and speed benchmark commands.

fn cmd_init_bench(args: InitBenchArgs) -> Result<()> {
    anyhow::ensure!(
        args.iterations > 0,
        "--iterations must be greater than zero"
    );

    for _ in 0..args.warmup {
        let method = prepare_method(init_method_options(&args)?)?;
        std::hint::black_box(method.id());
    }

    let started = Instant::now();
    let mut method_id = String::new();
    for _ in 0..args.iterations {
        let method = prepare_method(init_method_options(&args)?)?;
        method_id.clear();
        method_id.push_str(method.id());
        std::hint::black_box(method.id());
    }
    let elapsed = started.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let ns_per_init = elapsed_secs * 1_000_000_000.0 / args.iterations as f64;
    let inits_per_sec = args.iterations as f64 / elapsed_secs;
    let payload = serde_json::json!({
        "measurement": "method_prepare",
        "method": method_id,
        "iterations": args.iterations,
        "warmup": args.warmup,
        "elapsed_ms": elapsed_secs * 1000.0,
        "ns_per_init": ns_per_init,
        "inits_per_sec": inits_per_sec,
    });

    if let Some(path) = &args.output {
        create_parent(path)?;
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &payload)?;
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("method: {method_id}");
        println!("measurement: method_prepare");
        println!("warmup: {}", args.warmup);
        println!("iterations: {}", args.iterations);
        println!("elapsed_ms: {:.3}", elapsed_secs * 1000.0);
        println!("ns_per_init: {:.3}", ns_per_init);
        println!("inits_per_sec: {:.3}", inits_per_sec);
        if let Some(path) = &args.output {
            println!("output: {}", path.display());
        }
    }

    Ok(())
}

fn cmd_matrix(args: MatrixArgs) -> Result<()> {
    anyhow::ensure!(
        args.iterations > 0,
        "--iterations must be greater than zero"
    );
    anyhow::ensure!(
        args.init_iterations > 0,
        "--init-iterations must be greater than zero"
    );

    let manifest = read_methods_manifest(&args.manifest)?;
    let only = args
        .only
        .iter()
        .map(|value| normalize_manifest_selector(value))
        .collect::<Vec<_>>();
    for method in &manifest.methods {
        validate_manifest_slug(&method.slug)?;
        if !only.is_empty() && !manifest_method_selected(method, &only) {
            continue;
        }
        if !method.enabled {
            continue;
        }
        if let Some(feature) = &method.requires_feature {
            if !manifest_feature_available(feature) {
                continue;
            }
        }
        if !manifest_method_supports_locale(method, &args.locale) {
            continue;
        }
        if method.train.is_some() {
            anyhow::bail!(
                "manifest method {:?} has a [methods.train] block; run `hyphlab method materialize` before `hyphlab matrix`",
                method.slug
            );
        }
    }

    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("create {}", args.output_dir.display()))?;
    let speed_dir = args.output_dir.join("speed");
    let init_dir = args.output_dir.join("init");
    std::fs::create_dir_all(&speed_dir)
        .with_context(|| format!("create {}", speed_dir.display()))?;
    std::fs::create_dir_all(&init_dir).with_context(|| format!("create {}", init_dir.display()))?;

    let manifest_dir = args.manifest.parent().unwrap_or_else(|| Path::new("."));
    let mut metric_inputs = Vec::new();
    let mut speed_inputs = Vec::new();
    let mut init_inputs = Vec::new();
    let skip_method_errors = !args.abort_method_errors;

    for method in manifest.methods {
        validate_manifest_slug(&method.slug)?;
        if !only.is_empty() && !manifest_method_selected(&method, &only) {
            continue;
        }
        if !method.enabled {
            println!("skip {}: disabled", method.slug);
            continue;
        }
        if let Some(feature) = &method.requires_feature {
            if !manifest_feature_available(feature) {
                println!("skip {}: feature {feature} is not enabled", method.slug);
                continue;
            }
        }
        if !manifest_method_supports_locale(&method, &args.locale) {
            println!("skip {}: unsupported locale {}", method.slug, args.locale);
            continue;
        }
        let patterns = match manifest_method_patterns(&method, manifest_dir, args.patterns.as_ref())
        {
            PatternDecision::Use(path) => Some(path),
            PatternDecision::Skip(reason) => {
                println!("skip {}: {reason}", method.slug);
                continue;
            }
            PatternDecision::None => None,
        };

        println!("\n-- {} ({}) --", method.slug, method.method);
        let dictionary = method
            .dictionary
            .as_ref()
            .map(|path| resolve_manifest_path(manifest_dir, path));
        let metric_path = args.output_dir.join(format!("{}.json", method.slug));
        let speed_path = speed_dir.join(format!("{}.json", method.slug));
        let init_path = init_dir.join(format!("{}.json", method.slug));

        cmd_eval(EvalArgs {
            gold: args.gold.clone(),
            method: method.method.clone(),
            locale: args.locale.clone(),
            patterns: patterns.clone(),
            dictionary: dictionary.clone(),
            external_command: method.external_command.clone(),
            left_min: method.left_min,
            right_min: method.right_min,
            min_word_len: method.min_word_len,
            ambiguous: args.ambiguous,
            json: false,
            output: Some(metric_path.clone()),
            errors_output: None,
            skip_method_errors,
            method_errors_output: Some(
                args.output_dir
                    .join(format!("{}_method_errors.jsonl", method.slug)),
            ),
        })?;

        cmd_speed(SpeedArgs {
            gold: args.gold.clone(),
            method: method.method.clone(),
            locale: args.locale.clone(),
            patterns: patterns.clone(),
            dictionary: dictionary.clone(),
            external_command: method.external_command.clone(),
            iterations: args.iterations,
            warmup: 1,
            left_min: method.left_min,
            right_min: method.right_min,
            min_word_len: method.min_word_len,
            ambiguous: args.ambiguous,
            json: false,
            output: Some(speed_path.clone()),
            skip_method_errors,
            method_errors_output: Some(
                speed_dir.join(format!("{}_method_errors.jsonl", method.slug)),
            ),
        })?;

        cmd_init_bench(InitBenchArgs {
            method: method.method,
            locale: args.locale.clone(),
            patterns,
            dictionary,
            gold: Some(args.gold.clone()),
            external_command: method.external_command,
            iterations: args.init_iterations,
            warmup: args.init_warmup,
            left_min: method.left_min,
            right_min: method.right_min,
            min_word_len: method.min_word_len,
            json: false,
            output: Some(init_path.clone()),
        })?;

        metric_inputs.push(metric_path);
        speed_inputs.push(speed_path);
        init_inputs.push(init_path);
    }

    anyhow::ensure!(
        !metric_inputs.is_empty(),
        "no manifest methods were available for locale {}",
        args.locale
    );
    cmd_compare(CompareArgs {
        input: metric_inputs,
        speed_input: speed_inputs,
        init_input: init_inputs,
        output: Some(args.output_dir.join("compare.md")),
    })?;
    println!("wrote {}", args.output_dir.join("compare.md").display());

    Ok(())
}

enum PatternDecision {
    Use(PathBuf),
    Skip(String),
    None,
}

fn read_methods_manifest(path: &Path) -> Result<MethodsManifest> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn validate_manifest_slug(slug: &str) -> Result<()> {
    anyhow::ensure!(!slug.trim().is_empty(), "manifest method slug is empty");
    anyhow::ensure!(
        !slug.contains('/') && !slug.contains('\\') && slug != "." && slug != "..",
        "manifest method slug must be a file-safe name: {slug:?}"
    );
    Ok(())
}

fn manifest_method_selected(method: &ManifestMethod, selectors: &[String]) -> bool {
    let slug = normalize_manifest_selector(&method.slug);
    let method_name = normalize_manifest_selector(&method.method);
    selectors
        .iter()
        .any(|selector| selector == &slug || selector == &method_name)
}

fn normalize_manifest_selector(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}

fn manifest_method_patterns(
    method: &ManifestMethod,
    manifest_dir: &Path,
    dataset_patterns: Option<&PathBuf>,
) -> PatternDecision {
    if !(method.requires_patterns || method.pass_patterns) {
        return PatternDecision::None;
    }
    let method_patterns = method.patterns.as_ref();
    let Some(path) = method_patterns.or(dataset_patterns) else {
        return if method.requires_patterns {
            PatternDecision::Skip("requires patterns but this dataset has none".to_string())
        } else {
            PatternDecision::None
        };
    };
    let path = if let Some(path) = method_patterns {
        resolve_manifest_path(manifest_dir, path)
    } else {
        path.clone()
    };
    if method.requires_patterns && !path.is_file() {
        return PatternDecision::Skip(format!("patterns file is missing: {}", path.display()));
    }
    PatternDecision::Use(path)
}

fn manifest_method_supports_locale(method: &ManifestMethod, locale: &str) -> bool {
    if method.supports.is_empty() {
        return true;
    }
    let normalized_locale = normalize_locale_match_key(locale);
    let language = normalized_locale
        .split('-')
        .next()
        .unwrap_or(normalized_locale.as_str());
    method.supports.iter().any(|supported| {
        let supported = normalize_locale_match_key(supported);
        supported == "*" || supported == normalized_locale || supported == language
    })
}

fn manifest_feature_available(feature: &str) -> bool {
    match feature {
        "adapters-hyphenation" => cfg!(feature = "adapters-hyphenation"),
        "adapters-hyphenation-embedded" => cfg!(feature = "adapters-hyphenation-embedded"),
        _ => false,
    }
}

fn normalize_locale_match_key(locale: &str) -> String {
    locale.trim().replace('_', "-").to_ascii_lowercase()
}

fn resolve_manifest_path(manifest_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_dir.join(path)
    }
}

fn cmd_speed(args: SpeedArgs) -> Result<()> {
    anyhow::ensure!(
        args.iterations > 0,
        "--iterations must be greater than zero"
    );
    let mut records = read_records(&args.gold)?;
    let evaluation = evaluation_metadata(
        &args.gold,
        &args.locale,
        args.patterns.as_ref(),
        args.ambiguous,
        args.left_min,
        args.right_min,
        args.min_word_len,
    );
    let skipped_ambiguous = if args.ambiguous == AmbiguousPolicyArg::Exclude {
        let before = records.len();
        records.retain(|record| !record.ambiguous || record.variants.is_empty());
        before - records.len()
    } else {
        0
    };
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.gold.display());
    let dictionary_is_gold_oracle = is_dictionary_method(&args.method) && args.dictionary.is_none();
    let method = prepare_method(MethodOptions {
        method: args.method.clone(),
        locale: args.locale.clone(),
        patterns: args.patterns.clone(),
        dictionary: args.dictionary.clone().or_else(|| {
            if is_dictionary_method(&args.method) {
                Some(args.gold.clone())
            } else {
                None
            }
        }),
        dictionary_is_gold_oracle,
        external_command: args.external_command.clone(),
        left_min: args.left_min,
        right_min: args.right_min,
        min_word_len: args.min_word_len,
    })?;

    let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
    let mut method_errors = Vec::new();
    if args.skip_method_errors {
        let mut filtered = Vec::with_capacity(records.len());
        for record in records {
            out.clear();
            match method.hyphenate_record_into(&record, &mut out) {
                Ok(()) => filtered.push(record),
                Err(error) => method_errors.push(MethodError {
                    id: record.id,
                    word: record.word,
                    error: error.to_string(),
                }),
            }
        }
        records = filtered;
    }
    anyhow::ensure!(
        !records.is_empty(),
        "{} has no records after method-error filtering",
        args.gold.display()
    );

    for _ in 0..args.warmup {
        for record in &records {
            method.hyphenate_record_into(std::hint::black_box(record), &mut out)?;
            std::hint::black_box(&out);
        }
    }

    let started = Instant::now();
    for _ in 0..args.iterations {
        for record in &records {
            method.hyphenate_record_into(std::hint::black_box(record), &mut out)?;
            std::hint::black_box(&out);
        }
    }
    let elapsed = started.elapsed();
    let total_predictions = records.len() * args.iterations;
    let elapsed_secs = elapsed.as_secs_f64();
    let ns_per_word = elapsed_secs * 1_000_000_000.0 / total_predictions as f64;
    let words_per_sec = total_predictions as f64 / elapsed_secs;
    let payload = serde_json::json!({
        "measurement": "steady_state_prediction",
        "method": method.id(),
        "evaluation": &evaluation,
        "words": records.len(),
        "iterations": args.iterations,
        "warmup": args.warmup,
        "skipped_ambiguous": skipped_ambiguous,
        "skipped_method_errors": method_errors.len(),
        "total_predictions": total_predictions,
        "elapsed_ms": elapsed_secs * 1000.0,
        "ns_per_word": ns_per_word,
        "words_per_sec": words_per_sec,
    });

    if let Some(path) = &args.output {
        create_parent(path)?;
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &payload)?;
    }
    if let Some(path) = &args.method_errors_output {
        write_method_errors(path, &method_errors)?;
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("method: {}", method.id());
        println!("words: {}", records.len());
        println!("skipped_ambiguous: {skipped_ambiguous}");
        println!("skipped_method_errors: {}", method_errors.len());
        println!("iterations: {}", args.iterations);
        println!("total_predictions: {total_predictions}");
        println!("elapsed_ms: {:.3}", elapsed_secs * 1000.0);
        println!("ns_per_word: {:.3}", ns_per_word);
        println!("words_per_sec: {:.3}", words_per_sec);
        if let Some(path) = &args.output {
            println!("output: {}", path.display());
        }
        if let Some(path) = &args.method_errors_output {
            println!("method_errors_output: {}", path.display());
        }
    }

    Ok(())
}

fn init_method_options(args: &InitBenchArgs) -> Result<MethodOptions> {
    let dictionary_is_gold_oracle =
        is_dictionary_method(&args.method) && args.dictionary.is_none() && args.gold.is_some();
    let dictionary = args.dictionary.clone().or_else(|| {
        if is_dictionary_method(&args.method) {
            args.gold.clone()
        } else {
            None
        }
    });
    if is_dictionary_method(&args.method) && dictionary.is_none() {
        anyhow::bail!(
            "--dictionary or --gold is required for --method {}",
            args.method
        );
    }

    Ok(MethodOptions {
        method: args.method.clone(),
        locale: args.locale.clone(),
        patterns: args.patterns.clone(),
        dictionary,
        dictionary_is_gold_oracle,
        external_command: args.external_command.clone(),
        left_min: args.left_min,
        right_min: args.right_min,
        min_word_len: args.min_word_len,
    })
}
