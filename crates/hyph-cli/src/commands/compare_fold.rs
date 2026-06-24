// Report comparison and fold-summary commands.

fn cmd_compare(args: CompareArgs) -> Result<()> {
    let mut speeds = HashMap::new();
    for input in &args.speed_input {
        let speed = read_speed_summary(input)?;
        speeds.insert(speed.method.clone(), speed);
    }
    let mut inits = HashMap::new();
    for input in &args.init_input {
        let init = read_init_summary(input)?;
        inits.insert(init.method.clone(), init);
    }

    let mut rows = Vec::new();
    for input in &args.input {
        let file = File::open(input).with_context(|| format!("open {}", input.display()))?;
        let value: serde_json::Value =
            serde_json::from_reader(file).with_context(|| format!("parse {}", input.display()))?;
        let method = value
            .get("method")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| file_stem(input));
        let metrics_value = value
            .get("metrics")
            .cloned()
            .unwrap_or_else(|| value.clone());
        let metrics = serde_json::from_value::<Metrics>(metrics_value)
            .with_context(|| format!("read metrics from {}", input.display()))?;
        let evaluation = value
            .get("evaluation")
            .cloned()
            .map(serde_json::from_value::<EvaluationMetadata>)
            .transpose()
            .with_context(|| format!("read evaluation metadata from {}", input.display()))?;
        let speed = speeds.get(&method).cloned();
        let init = inits.get(&method).cloned();
        rows.push(CompareRow {
            method,
            metrics,
            evaluation,
            speed,
            init,
        });
    }

    let table = render_compare_table(&rows);
    if let Some(path) = &args.output {
        create_parent(path)?;
        std::fs::write(path, &table).with_context(|| format!("write {}", path.display()))?;
    } else {
        print!("{table}");
    }
    Ok(())
}

fn cmd_fold_summary(args: FoldSummaryArgs) -> Result<()> {
    let mut fold_dirs = std::fs::read_dir(&args.input_dir)
        .with_context(|| format!("read {}", args.input_dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("read entries from {}", args.input_dir.display()))?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    fold_dirs.sort();
    anyhow::ensure!(
        !fold_dirs.is_empty(),
        "{} has no fold directories",
        args.input_dir.display()
    );

    let mut points_by_method = BTreeMap::<String, Vec<FoldPoint>>::new();
    let mut points_by_fold = BTreeMap::<String, BTreeMap<String, FoldPoint>>::new();

    for fold_dir in fold_dirs {
        let fold_name = fold_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("fold")
            .to_string();
        let speed = read_fold_speed_or_init(&fold_dir.join("speed"), true)?;
        let init = read_fold_speed_or_init(&fold_dir.join("init"), false)?;
        let mut metric_paths = std::fs::read_dir(&fold_dir)
            .with_context(|| format!("read {}", fold_dir.display()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("read entries from {}", fold_dir.display()))?
            .into_iter()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        metric_paths.sort();

        for metric_path in metric_paths {
            let file = File::open(&metric_path)
                .with_context(|| format!("open {}", metric_path.display()))?;
            let value: serde_json::Value = serde_json::from_reader(file)
                .with_context(|| format!("parse {}", metric_path.display()))?;
            let Some(metrics_value) = value.get("metrics").cloned() else {
                continue;
            };
            let metrics = serde_json::from_value::<Metrics>(metrics_value)
                .with_context(|| format!("parse metrics from {}", metric_path.display()))?;
            let slug = file_stem(&metric_path);
            let point = FoldPoint {
                words: metrics.words as f64,
                precision: metrics.precision(),
                recall: metrics.recall(),
                f1: metrics.f1(),
                f05: metrics.f05(),
                exact: metrics.exact_accuracy(),
                serious_error: metrics.serious_word_error_rate(),
                fp_per_100k: metrics.fp_per_100k_boundaries(),
                ns_per_word: speed.get(&slug).copied(),
                init_ms: init.get(&slug).map(|ns| ns / 1_000_000.0),
            };
            points_by_method
                .entry(slug.clone())
                .or_default()
                .push(point.clone());
            points_by_fold
                .entry(fold_name.clone())
                .or_default()
                .insert(slug, point);
        }
    }

    let summary = render_fold_summary(&points_by_method, &points_by_fold);
    if let Some(path) = &args.output {
        create_parent(path)?;
        std::fs::write(path, &summary).with_context(|| format!("write {}", path.display()))?;
        println!("wrote {}", path.display());
    } else {
        print!("{summary}");
    }
    Ok(())
}

fn read_fold_speed_or_init(path: &Path, speed: bool) -> Result<HashMap<String, f64>> {
    let mut out = HashMap::new();
    if !path.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let path = entry
            .with_context(|| format!("read entry from {}", path.display()))?
            .path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let key = file_stem(&path);
        let value = if speed {
            read_speed_summary(&path)?.ns_per_word
        } else {
            read_init_summary(&path)?.ns_per_init
        };
        out.insert(key, value);
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct FoldPoint {
    words: f64,
    precision: f64,
    recall: f64,
    f1: f64,
    f05: f64,
    exact: f64,
    serious_error: f64,
    fp_per_100k: f64,
    ns_per_word: Option<f64>,
    init_ms: Option<f64>,
}

fn render_fold_summary(
    points_by_method: &BTreeMap<String, Vec<FoldPoint>>,
    points_by_fold: &BTreeMap<String, BTreeMap<String, FoldPoint>>,
) -> String {
    let mut out = String::new();
    out.push_str("## 5-Fold Summary\n\n");
    out.push_str("| method | folds | words | precision | recall | f1 | f0.5 | exact | serious_error | fp/100k | steady ns/word | init ms | delta f0.5 | delta recall | delta serious | delta ns/word |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");

    for (method, points) in points_by_method {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            method,
            points.len(),
            fmt_mean_sd(points.iter().map(|point| point.words)),
            fmt_mean_sd(points.iter().map(|point| point.precision)),
            fmt_mean_sd(points.iter().map(|point| point.recall)),
            fmt_mean_sd(points.iter().map(|point| point.f1)),
            fmt_mean_sd(points.iter().map(|point| point.f05)),
            fmt_mean_sd(points.iter().map(|point| point.exact)),
            fmt_mean_sd(points.iter().map(|point| point.serious_error)),
            fmt_mean_sd(points.iter().map(|point| point.fp_per_100k)),
            fmt_optional_mean_sd(points.iter().filter_map(|point| point.ns_per_word)),
            fmt_optional_mean_sd(points.iter().filter_map(|point| point.init_ms)),
            fmt_delta(points_by_fold, method, |point| point.f05),
            fmt_delta(points_by_fold, method, |point| point.recall),
            fmt_delta(points_by_fold, method, |point| point.serious_error),
            fmt_optional_delta(points_by_fold, method, |point| point.ns_per_word),
        ));
    }

    out.push('\n');
    out.push_str("Deltas are paired against the `hypher` row in the same fold. Higher is better except `serious_error`, `fp/100k`, `steady ns/word`, and `init ms`.\n");
    out
}

fn fmt_mean_sd(values: impl Iterator<Item = f64>) -> String {
    let values = values.collect::<Vec<_>>();
    let (mean, sd) = mean_sd(&values);
    format!("{mean:.6} (sd {sd:.6})")
}

fn fmt_optional_mean_sd(values: impl Iterator<Item = f64>) -> String {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        String::new()
    } else {
        let (mean, sd) = mean_sd(&values);
        format!("{mean:.3} (sd {sd:.3})")
    }
}

fn fmt_delta(
    points_by_fold: &BTreeMap<String, BTreeMap<String, FoldPoint>>,
    method: &str,
    value: impl Fn(&FoldPoint) -> f64,
) -> String {
    let values = points_by_fold
        .values()
        .filter_map(|fold| Some(value(fold.get(method)?) - value(fold.get("hypher")?)))
        .collect::<Vec<_>>();
    if values.is_empty() {
        String::new()
    } else {
        let (mean, sd) = mean_sd(&values);
        format!("{mean:.6} (sd {sd:.6})")
    }
}

fn fmt_optional_delta(
    points_by_fold: &BTreeMap<String, BTreeMap<String, FoldPoint>>,
    method: &str,
    value: impl Fn(&FoldPoint) -> Option<f64>,
) -> String {
    let values = points_by_fold
        .values()
        .filter_map(|fold| Some(value(fold.get(method)?)? - value(fold.get("hypher")?)?))
        .collect::<Vec<_>>();
    if values.is_empty() {
        String::new()
    } else {
        let (mean, sd) = mean_sd(&values);
        format!("{mean:.3} (sd {sd:.3})")
    }
}

fn mean_sd(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if values.len() == 1 {
        return (mean, 0.0);
    }
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    (mean, variance.sqrt())
}

