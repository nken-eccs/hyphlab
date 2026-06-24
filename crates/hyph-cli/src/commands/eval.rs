// Single-method evaluation command.

// Evaluation, speed, init benchmark, and matrix execution commands.

fn cmd_eval(args: EvalArgs) -> Result<()> {
    let records = read_records(&args.gold)?;
    let evaluation = evaluation_metadata(
        &args.gold,
        &args.locale,
        args.patterns.as_ref(),
        args.ambiguous,
        args.left_min,
        args.right_min,
        args.min_word_len,
    );
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
    let config = method.config().clone();

    let prediction_error_policy = if args.skip_method_errors {
        PredictionErrorPolicy::Skip
    } else {
        PredictionErrorPolicy::Abort
    };
    let report = evaluate_predictions_report_with_policy(
        records,
        &config,
        args.ambiguous.into(),
        prediction_error_policy,
        |record, out| {
            method
                .hyphenate_record_into(record, out)
                .with_context(|| format!("hyphenate {:?}", record.word))
        },
    )?;

    if let Some(path) = &args.output {
        write_report(path, method.id(), &evaluation, &report)?;
    }
    if let Some(path) = &args.errors_output {
        write_errors(path, &report.errors)?;
    }
    if let Some(path) = &args.method_errors_output {
        write_method_errors(path, &report.method_errors)?;
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report.metrics)?);
    } else {
        print_metrics(method.id(), &report.metrics);
        if let Some(path) = &args.output {
            println!("metrics_output: {}", path.display());
        }
        if let Some(path) = &args.errors_output {
            println!("errors_output: {}", path.display());
        }
        if let Some(path) = &args.method_errors_output {
            println!("method_errors_output: {}", path.display());
        }
    }
    Ok(())
}

