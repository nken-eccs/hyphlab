// Shared path helpers and report rendering.

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn parse_byte(value: &str) -> Result<u8> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u8::from_str_radix(hex, 16).with_context(|| format!("parse byte {value:?}"))
    } else {
        trimmed
            .parse::<u8>()
            .with_context(|| format!("parse byte {value:?}"))
    }
}

fn parse_u64_key(value: &str) -> Result<u64> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).with_context(|| format!("parse u64 key {value:?}"))
    } else {
        trimmed
            .parse::<u64>()
            .with_context(|| format!("parse u64 key {value:?}"))
    }
}

fn evaluation_metadata(
    gold: &Path,
    locale: &str,
    patterns: Option<&PathBuf>,
    ambiguous_policy: AmbiguousPolicyArg,
    left_min: Option<usize>,
    right_min: Option<usize>,
    min_word_len: Option<usize>,
) -> EvaluationMetadata {
    EvaluationMetadata {
        gold: gold.display().to_string(),
        locale: locale.to_string(),
        patterns: patterns.map(|path| path.display().to_string()),
        ambiguous_policy: ambiguous_policy.as_str().to_string(),
        left_min,
        right_min,
        min_word_len,
    }
}

fn write_report(
    path: &Path,
    method: &str,
    evaluation: &EvaluationMetadata,
    report: &EvaluationReport,
) -> Result<()> {
    create_parent(path)?;
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let writer = BufWriter::new(file);
    let payload = serde_json::json!({
        "method": method,
        "evaluation": evaluation,
        "metrics": &report.metrics,
        "error_words": report.errors.len(),
        "method_error_words": report.method_errors.len(),
    });
    serde_json::to_writer_pretty(writer, &payload)?;
    Ok(())
}

fn write_errors(path: &Path, errors: &[WordError]) -> Result<()> {
    create_parent(path)?;
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for error in errors {
        serde_json::to_writer(&mut writer, error)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn write_method_errors(path: &Path, errors: &[MethodError]) -> Result<()> {
    create_parent(path)?;
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for error in errors {
        serde_json::to_writer(&mut writer, error)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory {}", parent.display()))?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct CompareRow {
    method: String,
    metrics: Metrics,
    evaluation: Option<EvaluationMetadata>,
    speed: Option<SpeedSummary>,
    init: Option<InitSummary>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct EvaluationMetadata {
    gold: String,
    locale: String,
    patterns: Option<String>,
    ambiguous_policy: String,
    left_min: Option<usize>,
    right_min: Option<usize>,
    min_word_len: Option<usize>,
}

#[derive(Debug, Clone)]
struct SpeedSummary {
    method: String,
    ns_per_word: f64,
    words_per_sec: f64,
}

#[derive(Debug, Clone)]
struct InitSummary {
    method: String,
    ns_per_init: f64,
}

fn read_speed_summary(path: &Path) -> Result<SpeedSummary> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_reader(file).with_context(|| format!("parse {}", path.display()))?;
    let method = value
        .get("method")
        .and_then(|value| value.as_str())
        .with_context(|| format!("read method from {}", path.display()))?
        .to_string();
    let ns_per_word = value
        .get("ns_per_word")
        .and_then(|value| value.as_f64())
        .with_context(|| format!("read ns_per_word from {}", path.display()))?;
    let words_per_sec = value
        .get("words_per_sec")
        .and_then(|value| value.as_f64())
        .with_context(|| format!("read words_per_sec from {}", path.display()))?;
    Ok(SpeedSummary {
        method,
        ns_per_word,
        words_per_sec,
    })
}

fn read_init_summary(path: &Path) -> Result<InitSummary> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_reader(file).with_context(|| format!("parse {}", path.display()))?;
    let method = value
        .get("method")
        .and_then(|value| value.as_str())
        .with_context(|| format!("read method from {}", path.display()))?
        .to_string();
    let ns_per_init = value
        .get("ns_per_init")
        .and_then(|value| value.as_f64())
        .with_context(|| format!("read ns_per_init from {}", path.display()))?;
    Ok(InitSummary {
        method,
        ns_per_init,
    })
}

fn render_compare_table(rows: &[CompareRow]) -> String {
    let include_speed = rows.iter().any(|row| row.speed.is_some());
    let include_init = rows.iter().any(|row| row.init.is_some());
    let include_method_errors = rows.iter().any(|row| row.metrics.skipped_method_errors > 0);

    let mut headers = vec![
        "method",
        "words",
        "precision",
        "recall",
        "f1",
        "f0.5",
        "exact",
        "serious_error",
        "fp/100k",
    ];
    if include_method_errors {
        headers.insert(2, "method_errors");
    }
    if include_speed {
        headers.push("steady ns/word");
        headers.push("steady words/sec");
    }
    if include_init {
        headers.push("init ms");
    }

    let mut out = render_evaluation_metadata(rows);
    out.push_str("| ");
    out.push_str(&headers.join(" | "));
    out.push_str(" |\n");
    out.push_str("| --- |");
    for _ in 1..headers.len() {
        out.push_str(" ---: |");
    }
    out.push('\n');

    for row in rows {
        let metrics = &row.metrics;
        out.push_str(&format!("| {} | {} |", row.method, metrics.words));
        if include_method_errors {
            out.push_str(&format!(" {} |", metrics.skipped_method_errors));
        }
        out.push_str(&format!(
            " {:.6} | {:.6} | {:.6} | {:.6} | {:.6} | {:.6} | {:.3} |",
            metrics.precision(),
            metrics.recall(),
            metrics.f1(),
            metrics.f05(),
            metrics.exact_accuracy(),
            metrics.serious_word_error_rate(),
            metrics.fp_per_100k_boundaries()
        ));
        if include_speed {
            if let Some(speed) = &row.speed {
                out.push_str(&format!(
                    " {:.3} | {:.3} |",
                    speed.ns_per_word, speed.words_per_sec
                ));
            } else {
                out.push_str("  |  |");
            }
        }
        if include_init {
            if let Some(init) = &row.init {
                out.push_str(&format!(" {:.3} |", init.ns_per_init / 1_000_000.0));
            } else {
                out.push_str("  |");
            }
        }
        out.push('\n');
    }
    out
}

fn render_evaluation_metadata(rows: &[CompareRow]) -> String {
    let evaluations = rows
        .iter()
        .filter_map(|row| row.evaluation.as_ref())
        .collect::<Vec<_>>();
    if evaluations.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("## Evaluation Data\n\n");
    if evaluations
        .iter()
        .all(|evaluation| *evaluation == evaluations[0])
    {
        let evaluation = evaluations[0];
        out.push_str(&format!("- gold: `{}`\n", evaluation.gold));
        out.push_str(&format!("- locale: `{}`\n", evaluation.locale));
        out.push_str(&format!(
            "- patterns: `{}`\n",
            evaluation.patterns.as_deref().unwrap_or("none")
        ));
        out.push_str(&format!(
            "- ambiguous_policy: `{}`\n",
            evaluation.ambiguous_policy
        ));
        out.push_str(&format!(
            "- boundary_config: left_min=`{}`, right_min=`{}`, min_word_len=`{}`\n",
            optional_usize(evaluation.left_min),
            optional_usize(evaluation.right_min),
            optional_usize(evaluation.min_word_len)
        ));
    } else {
        out.push_str("Rows have mixed evaluation metadata.\n\n");
        render_metadata_set(
            &mut out,
            "gold",
            evaluations.iter().map(|item| item.gold.as_str()),
        );
        render_metadata_set(
            &mut out,
            "locale",
            evaluations.iter().map(|item| item.locale.as_str()),
        );
        render_metadata_set(
            &mut out,
            "patterns",
            evaluations
                .iter()
                .map(|item| item.patterns.as_deref().unwrap_or("none")),
        );
        render_metadata_set(
            &mut out,
            "ambiguous_policy",
            evaluations
                .iter()
                .map(|item| item.ambiguous_policy.as_str()),
        );
    }
    out.push('\n');
    out
}

fn render_metadata_set<'a>(out: &mut String, label: &str, values: impl Iterator<Item = &'a str>) {
    let values = values.collect::<BTreeSet<_>>();
    let rendered = values
        .into_iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!("- {label}: {rendered}\n"));
}

fn optional_usize(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "default".to_string())
}

fn print_metrics(method: &str, metrics: &Metrics) {
    println!("method: {method}");
    println!("words: {}", metrics.words);
    println!("skipped_ambiguous: {}", metrics.skipped_ambiguous);
    println!("skipped_method_errors: {}", metrics.skipped_method_errors);
    println!("precision: {:.6}", metrics.precision());
    println!("recall: {:.6}", metrics.recall());
    println!("f1: {:.6}", metrics.f1());
    println!("f0.5: {:.6}", metrics.f05());
    println!("exact_accuracy: {:.6}", metrics.exact_accuracy());
    println!(
        "serious_word_error_rate: {:.6}",
        metrics.serious_word_error_rate()
    );
    println!("no_break_accuracy: {:.6}", metrics.no_break_accuracy());
    println!(
        "fp_per_100k_boundaries: {:.3}",
        metrics.fp_per_100k_boundaries()
    );
    println!(
        "confusion: tp={} fp={} fn={} tn={}",
        metrics.tp, metrics.fp, metrics.fn_, metrics.tn
    );
}
