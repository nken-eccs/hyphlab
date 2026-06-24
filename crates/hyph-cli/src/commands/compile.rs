// Saved-model compilation commands.

fn cmd_compile_safe_ngram(args: CompileSafeNgramArgs) -> Result<()> {
    let mut records = read_records(&args.gold)?;
    if !args.include_ambiguous {
        records.retain(|record| !record.ambiguous || record.variants.is_empty());
    }
    anyhow::ensure!(
        !records.is_empty(),
        "{} has no training records",
        args.gold.display()
    );

    let mut config = HyphenationConfig::default();
    if let Some(left_min) = args.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = args.right_min {
        config.right_min = right_min;
    }
    if let Some(min_word_len) = args.min_word_len {
        config.min_word_len = min_word_len;
    }

    let (options, veto_options) = parse_safe_ngram_veto_options(&args.method)?;
    let (rules, trained_records) = learn_safe_ngram_rules(&records, &config, &options);
    let veto_rules = if let Some(veto_options) = &veto_options {
        learn_safe_ngram_veto_rules(&records, &config, &options, &rules, veto_options)
    } else {
        U64HashSet::default()
    };
    anyhow::ensure!(
        !rules.is_empty(),
        "safe-ngram learned no rules from {} with method {:?}",
        args.gold.display(),
        args.method
    );

    let model = SafeNgramModelFile::from_parts(
        args.method,
        args.locale,
        file_stem(&args.gold),
        config,
        options,
        rules,
        veto_options,
        veto_rules,
        trained_records,
    );
    model.save(&args.output)?;
    let output_size = std::fs::metadata(&args.output)
        .with_context(|| format!("stat {}", args.output.display()))?
        .len();
    println!("records: {}", records.len());
    println!("trained_records: {}", model.trained_records);
    println!("rules: {}", model.rules.len());
    println!("veto_rules: {}", model.veto_rules.len());
    println!("id: {}", model.id);
    println!("output: {} bytes ({})", output_size, args.output.display());
    Ok(())
}

fn cmd_compile_italian_syllable(args: CompileItalianSyllableArgs) -> Result<()> {
    anyhow::ensure!(
        normalize_locale_match_key(&args.locale).starts_with("it"),
        "compile-italian-syllable requires an Italian locale, got {}",
        args.locale
    );
    let mut records = read_records(&args.gold)?;
    if !args.include_ambiguous {
        records.retain(|record| !record.ambiguous || record.variants.is_empty());
    }
    anyhow::ensure!(
        !records.is_empty(),
        "{} has no training records",
        args.gold.display()
    );

    let mut config = italian_syllable_default_config();
    if let Some(left_min) = args.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = args.right_min {
        config.right_min = right_min;
    }
    if let Some(min_word_len) = args.min_word_len {
        config.min_word_len = min_word_len;
    }

    let learned_splits = learn_italian_syllable_splits(&records, &config);
    let trained_records = count_italian_syllable_training_records(&records, &config);
    let model = ItalianSyllableModelFile::from_parts(
        args.method,
        args.locale,
        file_stem(&args.gold),
        config,
        learned_splits,
        trained_records,
    );
    model.save(&args.output)?;
    let output_size = std::fs::metadata(&args.output)
        .with_context(|| format!("stat {}", args.output.display()))?
        .len();
    println!("records: {}", records.len());
    println!("trained_records: {}", model.trained_records);
    println!("clusters: {}", model.learned_splits.len());
    println!("id: {}", model.id);
    println!("output: {} bytes ({})", output_size, args.output.display());
    Ok(())
}

