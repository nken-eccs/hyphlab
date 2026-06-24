// CRF training, threshold tuning, and conversion commands.

fn cmd_crf_train(args: CrfTrainArgs) -> Result<()> {
    let mut records = read_records(&args.gold)?;
    if !args.include_ambiguous {
        records.retain(|record| !record.ambiguous || record.variants.is_empty());
    }
    if let Some(limit) = args.limit {
        records.truncate(limit);
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
    let language = args
        .locale
        .parse::<LanguageTag>()
        .map_err(|err| anyhow::anyhow!("parse locale {:?}: {err}", args.locale))?;

    let model = train_crf(
        &records,
        CrfTrainOptions {
            id: args.id,
            language,
            config,
            threshold: args.threshold,
            min_n: args.min_n,
            max_n: args.max_n,
            epochs: args.epochs,
            learning_rate: args.learning_rate,
            l2: args.l2,
        },
    )?;
    model.save(&args.output)?;
    println!("records: {}", records.len());
    println!("features: {}", model.feature_count());
    println!("threshold: {:.3}", model.threshold());
    println!("output: {}", args.output.display());
    Ok(())
}

fn cmd_crf_tune_threshold(args: CrfTuneThresholdArgs) -> Result<()> {
    anyhow::ensure!((0.0..=1.0).contains(&args.min), "--min must be in [0, 1]");
    anyhow::ensure!((0.0..=1.0).contains(&args.max), "--max must be in [0, 1]");
    anyhow::ensure!(args.min <= args.max, "--min must be <= --max");
    anyhow::ensure!(args.step > 0.0, "--step must be greater than zero");

    let records = read_records(&args.gold)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.gold.display());
    let mut model = CrfHyphenator::load(&args.model)?;
    let ambiguous = AmbiguousPolicy::from(args.ambiguous);
    let mut rows = Vec::new();
    let mut best_threshold = args.min;
    let mut best_metrics = Metrics::default();
    let mut best_score = f64::NEG_INFINITY;

    let mut current_threshold = args.min;
    while current_threshold <= args.max + args.step * 0.5 {
        let threshold = current_threshold.min(args.max);
        model.set_threshold(threshold)?;
        let metrics = evaluate_crf_metrics(&model, &records, ambiguous)?;
        let score = threshold_objective_score(&metrics, args.objective);
        rows.push(serde_json::json!({
            "threshold": threshold,
            "score": score,
            "precision": metrics.precision(),
            "recall": metrics.recall(),
            "f1": metrics.f1(),
            "f0.5": metrics.f05(),
            "exact": metrics.exact_accuracy(),
            "serious_error": metrics.serious_word_error_rate(),
            "fp_per_100k": metrics.fp_per_100k_boundaries(),
            "tp": metrics.tp,
            "fp": metrics.fp,
            "fn": metrics.fn_,
            "tn": metrics.tn,
        }));
        if score > best_score
            || (score == best_score
                && (metrics.fp, std::cmp::Reverse(metrics.tp))
                    < (best_metrics.fp, std::cmp::Reverse(best_metrics.tp)))
        {
            best_score = score;
            best_threshold = threshold;
            best_metrics = metrics;
        }
        current_threshold += args.step;
    }

    model.set_threshold(best_threshold)?;
    if let Some(id) = &args.id {
        model.set_id(id.clone());
    }
    if let Some(path) = &args.output {
        model.save(path)?;
    }
    if let Some(path) = &args.report {
        create_parent(path)?;
        let payload = serde_json::json!({
            "model": args.model.display().to_string(),
            "gold": args.gold.display().to_string(),
            "objective": threshold_objective_name(args.objective),
            "best_threshold": best_threshold,
            "best_score": best_score,
            "best_metrics": {
                "precision": best_metrics.precision(),
                "recall": best_metrics.recall(),
                "f1": best_metrics.f1(),
                "f0.5": best_metrics.f05(),
                "exact": best_metrics.exact_accuracy(),
                "serious_error": best_metrics.serious_word_error_rate(),
                "fp_per_100k": best_metrics.fp_per_100k_boundaries(),
                "tp": best_metrics.tp,
                "fp": best_metrics.fp,
                "fn": best_metrics.fn_,
                "tn": best_metrics.tn,
            },
            "rows": rows,
        });
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &payload)?;
    }

    println!("objective: {}", threshold_objective_name(args.objective));
    println!("best_threshold: {:.4}", best_threshold);
    println!("best_score: {:.6}", best_score);
    print_metrics(model.id(), &best_metrics);
    if let Some(path) = &args.output {
        println!("output: {}", path.display());
    }
    if let Some(path) = &args.report {
        println!("report: {}", path.display());
    }
    Ok(())
}

fn cmd_crf_convert(args: CrfConvertArgs) -> Result<()> {
    let mut model = CrfHyphenator::load(&args.input)?;
    if let Some(threshold) = args.threshold {
        model.set_threshold(threshold)?;
    }
    if let Some(id) = args.id {
        model.set_id(id);
    }
    model.save(&args.output)?;
    let input_size = std::fs::metadata(&args.input)
        .with_context(|| format!("stat {}", args.input.display()))?
        .len();
    let output_size = std::fs::metadata(&args.output)
        .with_context(|| format!("stat {}", args.output.display()))?
        .len();
    println!("features: {}", model.feature_count());
    println!("threshold: {:.4}", model.threshold());
    println!("input: {} bytes ({})", input_size, args.input.display());
    println!("output: {} bytes ({})", output_size, args.output.display());
    if input_size > 0 {
        println!("size_ratio: {:.4}", output_size as f64 / input_size as f64);
    }
    Ok(())
}


fn evaluate_crf_metrics(
    model: &CrfHyphenator,
    records: &[HyphenationRecord],
    ambiguous: AmbiguousPolicy,
) -> Result<Metrics> {
    evaluate_predictions(
        records.iter().cloned(),
        model.config(),
        ambiguous,
        |record, out| model.hyphenate_into(&record.word, out),
    )
}

fn threshold_objective_score(metrics: &Metrics, objective: ThresholdObjectiveArg) -> f64 {
    match objective {
        ThresholdObjectiveArg::F1 => metrics.f1(),
        ThresholdObjectiveArg::F05 => metrics.f05(),
        ThresholdObjectiveArg::Precision => metrics.precision(),
        ThresholdObjectiveArg::Recall => metrics.recall(),
        ThresholdObjectiveArg::Exact => metrics.exact_accuracy(),
    }
}

fn threshold_objective_name(objective: ThresholdObjectiveArg) -> &'static str {
    match objective {
        ThresholdObjectiveArg::F1 => "f1",
        ThresholdObjectiveArg::F05 => "f0.5",
        ThresholdObjectiveArg::Precision => "precision",
        ThresholdObjectiveArg::Recall => "recall",
        ThresholdObjectiveArg::Exact => "exact",
    }
}

