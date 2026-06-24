// Data import, filtering, splitting, and dataset stats commands.

fn cmd_import_tsv(args: ImportTsvArgs) -> Result<()> {
    let count = import_tsv(ImportTsvOptions {
        input: args.input,
        output: args.output.clone(),
        locale: args.locale,
        source: args.source,
        license: args.license,
    })?;
    println!("imported {count} records -> {}", args.output.display());
    Ok(())
}

fn cmd_import_moby(args: ImportMobyArgs) -> Result<()> {
    let separator = parse_byte(&args.separator)?;
    let count = import_moby(ImportMobyOptions {
        input: args.input,
        output: args.output.clone(),
        locale: Some(args.locale),
        source: args.source,
        license: args.license,
        separator,
    })?;
    println!("imported {count} records -> {}", args.output.display());
    Ok(())
}

fn cmd_import_wlhamb(args: ImportWlhambArgs) -> Result<()> {
    let report = import_wlhamb(ImportWlhambOptions {
        input: args.input,
        output: args.output.clone(),
        locale: args.locale,
        source: args.source,
        license: args.license,
        skip_invalid: args.skip_invalid,
    })?;
    println!(
        "imported {} records -> {}",
        report.records,
        args.output.display()
    );
    if report.skipped_invalid > 0 {
        println!("skipped_invalid: {}", report.skipped_invalid);
    }
    Ok(())
}

fn cmd_import_wiktextract(args: ImportWiktextractArgs) -> Result<()> {
    let report = import_wiktextract(ImportWiktextractOptions {
        input: args.input,
        output: args.output.clone(),
        locale: args.locale,
        filter_lang_code: args.filter_lang_code,
        source: args.source,
        license: args.license,
        skip_invalid: args.skip_invalid,
    })?;
    println!(
        "imported {} records -> {}",
        report.records,
        args.output.display()
    );
    println!("lines: {}", report.lines);
    if report.skipped_lang_code > 0 {
        println!("skipped_lang_code: {}", report.skipped_lang_code);
    }
    println!("skipped_no_hyphenation: {}", report.skipped_no_hyphenation);
    if report.skipped_invalid > 0 {
        println!("skipped_invalid: {}", report.skipped_invalid);
    }
    Ok(())
}

fn cmd_export_patgen(args: ExportPatgenArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    create_parent(&args.output)?;
    let file =
        File::create(&args.output).with_context(|| format!("create {}", args.output.display()))?;
    let mut writer = BufWriter::new(file);
    let mut emitted = BTreeSet::new();
    let mut skipped_ambiguous = 0usize;
    let mut skipped_non_alpha = 0usize;

    for record in records {
        if record.ambiguous && !args.include_ambiguous {
            skipped_ambiguous += 1;
            continue;
        }

        let word = if args.preserve_case {
            record.word.clone()
        } else {
            record.word.to_ascii_lowercase()
        };
        if args.ascii_alpha_only && !word.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            skipped_non_alpha += 1;
            continue;
        }
        emit_patgen_word(
            &mut writer,
            &mut emitted,
            &word,
            &record.breaks,
            &args.separator,
        )?;

        if args.include_ambiguous {
            for breaks in &record.variants {
                emit_patgen_word(&mut writer, &mut emitted, &word, breaks, &args.separator)?;
            }
        }
    }

    writer.flush()?;
    println!(
        "exported {} patgen entries -> {}",
        emitted.len(),
        args.output.display()
    );
    if skipped_ambiguous > 0 {
        println!("skipped_ambiguous: {skipped_ambiguous}");
    }
    if skipped_non_alpha > 0 {
        println!("skipped_non_alpha: {skipped_non_alpha}");
    }
    Ok(())
}

fn emit_patgen_word(
    writer: &mut impl Write,
    emitted: &mut BTreeSet<String>,
    word: &str,
    breaks: &[GraphemeIndex],
    separator: &str,
) -> Result<()> {
    let hyphenated = insert_separator(word, breaks, separator);
    if emitted.insert(hyphenated.clone()) {
        writeln!(writer, "{hyphenated}")?;
    }
    Ok(())
}

fn cmd_filter_script(args: FilterScriptArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());
    let input_count = records.len();
    let mut skipped_ambiguous = 0usize;
    let mut skipped_script = 0usize;
    let filtered = records
        .into_iter()
        .filter(|record| {
            if !args.include_ambiguous && record.ambiguous {
                skipped_ambiguous += 1;
                return false;
            }
            if !script_filter_matches(&record.word, args.script) {
                skipped_script += 1;
                return false;
            }
            true
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !filtered.is_empty(),
        "{} has no records matching {:?}",
        args.input.display(),
        args.script
    );
    let output_count = write_records(&args.output, filtered)?;
    println!("input_records: {input_count}");
    println!("output_records: {output_count}");
    println!("skipped_ambiguous: {skipped_ambiguous}");
    println!("skipped_script: {skipped_script}");
    println!("output: {}", args.output.display());
    Ok(())
}

fn script_filter_matches(word: &str, script: ScriptFilterArg) -> bool {
    let mut saw_alpha = false;
    for ch in word.chars() {
        if !ch.is_alphabetic() {
            continue;
        }
        saw_alpha = true;
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        let matches = match script {
            ScriptFilterArg::Cyrillic => safe_ngram_is_cyrillic_letter(lower),
            ScriptFilterArg::RussianCyrillic => safe_ngram_is_russian_cyrillic_letter(lower),
            ScriptFilterArg::Latin => safe_ngram_latin_base_letter(lower).is_some(),
        };
        if !matches {
            return false;
        }
    }
    saw_alpha
}

fn cmd_filter_quality(args: FilterQualityArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());
    let input_count = records.len();
    let mut dropped_long_no_break = 0usize;
    let filtered = records
        .into_iter()
        .filter(|record| {
            if args.drop_long_no_break
                && record.breaks.is_empty()
                && record.grapheme_len() >= args.min_graphemes
                && safe_ngram_word_vowel_count(&record.word) >= args.min_vowels
            {
                dropped_long_no_break += 1;
                return false;
            }
            true
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !filtered.is_empty(),
        "{} has no records after quality filtering",
        args.input.display()
    );
    let output_count = write_records(&args.output, filtered)?;
    println!("input_records: {input_count}");
    println!("output_records: {output_count}");
    println!("dropped_long_no_break: {dropped_long_no_break}");
    println!("output: {}", args.output.display());
    Ok(())
}

fn safe_ngram_word_vowel_count(word: &str) -> usize {
    word.chars()
        .filter(|ch| {
            let lower = ch.to_lowercase().next().unwrap_or(*ch);
            safe_ngram_unicode_is_vowel(lower)
        })
        .count()
}

fn cmd_dedup_variants(args: DedupVariantsArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());
    let input_count = records.len();
    let mut groups = BTreeMap::<String, (HyphenationRecord, BTreeSet<Vec<GraphemeIndex>>)>::new();
    for record in records {
        let key = split_group_key(&record);
        let entry = groups.entry(key).or_insert_with(|| {
            let mut template = record.clone();
            template.ambiguous = false;
            template.variants.clear();
            (template, BTreeSet::new())
        });
        entry.1.insert(record.breaks.clone().into_vec());
        for variant in record.variants {
            entry.1.insert(variant.into_vec());
        }
    }

    let mut ambiguous_words = 0usize;
    let mut deduped = Vec::with_capacity(groups.len());
    for (_key, (mut record, variants)) in groups {
        let mut variants = variants
            .into_iter()
            .map(SmallVec::from_vec)
            .collect::<Vec<_>>();
        variants.sort();
        let Some(first) = variants.first().cloned() else {
            continue;
        };
        record.breaks = first;
        if variants.len() > 1 {
            record.ambiguous = true;
            record.variants = variants;
            ambiguous_words += 1;
        } else {
            record.ambiguous = false;
            record.variants.clear();
        }
        deduped.push(record);
    }

    deduped.sort_by(|left, right| {
        split_group_key(left)
            .cmp(&split_group_key(right))
            .then_with(|| left.id.cmp(&right.id))
    });
    let output_count = write_records(&args.output, deduped)?;
    println!("input_records: {input_count}");
    println!("output_records: {output_count}");
    println!("ambiguous_words: {ambiguous_words}");
    println!(
        "collapsed_duplicates: {}",
        input_count.saturating_sub(output_count)
    );
    println!("output: {}", args.output.display());
    Ok(())
}

fn cmd_split(args: SplitArgs) -> Result<()> {
    anyhow::ensure!(
        args.train_ratio >= 0.0,
        "--train-ratio must be non-negative"
    );
    anyhow::ensure!(args.dev_ratio >= 0.0, "--dev-ratio must be non-negative");
    anyhow::ensure!(args.test_ratio >= 0.0, "--test-ratio must be non-negative");
    let ratio_sum = args.train_ratio + args.dev_ratio + args.test_ratio;
    anyhow::ensure!(ratio_sum > 0.0, "at least one split ratio must be positive");

    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());

    let mut grouped = BTreeMap::<String, Vec<HyphenationRecord>>::new();
    for record in records {
        grouped
            .entry(split_group_key(&record))
            .or_default()
            .push(record);
    }

    let mut groups = grouped
        .into_iter()
        .map(|(key, records)| (stable_hash64(&args.seed, &key), key, records))
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let total_groups = groups.len();
    let [train_group_count, dev_group_count, _test_group_count] = split_counts(
        total_groups,
        [args.train_ratio, args.dev_ratio, args.test_ratio],
        ratio_sum,
    );
    let train_group_cut = train_group_count;
    let dev_group_cut = train_group_count
        .saturating_add(dev_group_count)
        .min(total_groups);

    let mut train_records = Vec::new();
    let mut dev_records = Vec::new();
    let mut test_records = Vec::new();
    let mut train_groups = 0usize;
    let mut dev_groups = 0usize;
    let mut test_groups = 0usize;

    for (index, (_hash, _key, records)) in groups.into_iter().enumerate() {
        if index < train_group_cut {
            train_groups += 1;
            train_records.extend(records);
        } else if index < dev_group_cut {
            dev_groups += 1;
            dev_records.extend(records);
        } else {
            test_groups += 1;
            test_records.extend(records);
        }
    }

    let train_path = args.output_dir.join("train.jsonl.zst");
    let dev_path = args.output_dir.join("dev.jsonl.zst");
    let test_path = args.output_dir.join("test.jsonl.zst");
    let train_count = write_records(&train_path, train_records)?;
    let dev_count = write_records(&dev_path, dev_records)?;
    let test_count = write_records(&test_path, test_records)?;

    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("create {}", args.output_dir.display()))?;
    let summary_path = args.output_dir.join("split.json");
    let summary = serde_json::json!({
        "input": args.input.display().to_string(),
        "seed": args.seed,
        "group_key": "lang + lowercase(word)",
        "ratios": {
            "train": args.train_ratio,
            "dev": args.dev_ratio,
            "test": args.test_ratio,
        },
        "groups": {
            "train": train_groups,
            "dev": dev_groups,
            "test": test_groups,
        },
        "records": {
            "train": train_count,
            "dev": dev_count,
            "test": test_count,
        },
        "outputs": {
            "train": train_path.display().to_string(),
            "dev": dev_path.display().to_string(),
            "test": test_path.display().to_string(),
        },
    });
    std::fs::write(&summary_path, serde_json::to_vec_pretty(&summary)?)
        .with_context(|| format!("write {}", summary_path.display()))?;

    println!(
        "train: {train_count} records, {train_groups} groups -> {}",
        train_path.display()
    );
    println!(
        "dev: {dev_count} records, {dev_groups} groups -> {}",
        dev_path.display()
    );
    println!(
        "test: {test_count} records, {test_groups} groups -> {}",
        test_path.display()
    );
    println!("summary: {}", summary_path.display());
    Ok(())
}

fn cmd_kfold(args: KfoldArgs) -> Result<()> {
    anyhow::ensure!(args.folds >= 2, "--folds must be at least 2");
    anyhow::ensure!(
        (0.0..1.0).contains(&args.dev_ratio),
        "--dev-ratio must be in [0, 1)"
    );

    let records = read_records(&args.input)?;
    anyhow::ensure!(!records.is_empty(), "{} is empty", args.input.display());

    let mut grouped = BTreeMap::<String, Vec<HyphenationRecord>>::new();
    for record in records {
        grouped
            .entry(split_group_key(&record))
            .or_default()
            .push(record);
    }

    let mut groups = grouped
        .into_iter()
        .map(|(key, records)| (stable_hash64(&args.seed, &key), key, records))
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    anyhow::ensure!(
        groups.len() >= args.folds,
        "not enough word groups ({}) for {} folds",
        groups.len(),
        args.folds
    );

    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("create {}", args.output_dir.display()))?;
    let mut fold_summaries = Vec::new();
    for fold in 0..args.folds {
        let fold_dir = args.output_dir.join(format!("fold-{fold}"));
        let mut train_records = Vec::new();
        let mut dev_records = Vec::new();
        let mut test_records = Vec::new();
        let mut train_groups = 0usize;
        let mut dev_groups = 0usize;
        let mut test_groups = 0usize;

        for (index, (_hash, key, records)) in groups.iter().enumerate() {
            if index % args.folds == fold {
                test_groups += 1;
                test_records.extend(records.iter().cloned());
            } else if args.dev_ratio > 0.0
                && stable_unit_interval(&format!("{}:dev:{fold}", args.seed), key) < args.dev_ratio
            {
                dev_groups += 1;
                dev_records.extend(records.iter().cloned());
            } else {
                train_groups += 1;
                train_records.extend(records.iter().cloned());
            }
        }

        if args.dev_ratio > 0.0 && dev_records.is_empty() && train_records.len() > 1 {
            let moved = train_records
                .pop()
                .expect("train_records checked non-empty before pop");
            dev_records.push(moved);
            train_groups = train_groups.saturating_sub(1);
            dev_groups += 1;
        }

        let train_path = fold_dir.join("train.jsonl.zst");
        let dev_path = fold_dir.join("dev.jsonl.zst");
        let test_path = fold_dir.join("test.jsonl.zst");
        let train_count = write_records(&train_path, train_records)?;
        let dev_count = write_records(&dev_path, dev_records)?;
        let test_count = write_records(&test_path, test_records)?;

        let fold_summary = serde_json::json!({
            "fold": fold,
            "input": args.input.display().to_string(),
            "seed": args.seed,
            "group_key": "lang + lowercase(word)",
            "folds": args.folds,
            "dev_ratio": args.dev_ratio,
            "groups": {
                "train": train_groups,
                "dev": dev_groups,
                "test": test_groups,
            },
            "records": {
                "train": train_count,
                "dev": dev_count,
                "test": test_count,
            },
            "outputs": {
                "train": train_path.display().to_string(),
                "dev": dev_path.display().to_string(),
                "test": test_path.display().to_string(),
            },
        });
        std::fs::write(
            fold_dir.join("fold.json"),
            serde_json::to_vec_pretty(&fold_summary)?,
        )
        .with_context(|| format!("write {}", fold_dir.join("fold.json").display()))?;
        fold_summaries.push(fold_summary);
        println!(
            "fold-{fold}: train={train_count} dev={dev_count} test={test_count} -> {}",
            fold_dir.display()
        );
    }

    let summary_path = args.output_dir.join("kfold.json");
    let summary = serde_json::json!({
        "input": args.input.display().to_string(),
        "seed": args.seed,
        "group_key": "lang + lowercase(word)",
        "folds": args.folds,
        "dev_ratio": args.dev_ratio,
        "folds_detail": fold_summaries,
    });
    std::fs::write(&summary_path, serde_json::to_vec_pretty(&summary)?)
        .with_context(|| format!("write {}", summary_path.display()))?;
    println!("summary: {}", summary_path.display());
    Ok(())
}

fn split_group_key(record: &HyphenationRecord) -> String {
    format!("{}\u{1f}{}", record.lang, record.word.to_lowercase())
}

fn split_counts(total: usize, ratios: [f64; 3], ratio_sum: f64) -> [usize; 3] {
    if total == 0 {
        return [0; 3];
    }

    let positive = ratios.iter().filter(|ratio| **ratio > 0.0).count();
    if positive == 0 {
        return [total, 0, 0];
    }

    if positive > total {
        let mut order = [0usize, 1, 2];
        order.sort_by(|left, right| {
            ratios[*right]
                .partial_cmp(&ratios[*left])
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.cmp(right))
        });
        let mut counts = [0usize; 3];
        for index in order.into_iter().take(total) {
            counts[index] = 1;
        }
        return counts;
    }

    let mut counts = [0usize; 3];
    for (index, ratio) in ratios.iter().enumerate() {
        if *ratio > 0.0 {
            counts[index] = 1;
        }
    }

    let remaining = total - positive;
    let mut remainders = [(0usize, 0.0f64); 3];
    for (index, ratio) in ratios.iter().enumerate() {
        if *ratio <= 0.0 {
            remainders[index] = (index, -1.0);
            continue;
        }
        let exact = remaining as f64 * *ratio / ratio_sum;
        let floor = exact.floor() as usize;
        counts[index] += floor;
        remainders[index] = (index, exact - floor as f64);
    }

    let assigned = counts.iter().sum::<usize>();
    let mut leftover = total.saturating_sub(assigned);
    remainders.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    for (index, _remainder) in remainders {
        if leftover == 0 {
            break;
        }
        if ratios[index] > 0.0 {
            counts[index] += 1;
            leftover -= 1;
        }
    }

    counts
}

fn stable_hash64(seed: &str, value: &str) -> u64 {
    const OFFSET: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;

    let mut hash = OFFSET;
    for byte in seed.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash ^= 0xff;
    hash = hash.wrapping_mul(PRIME);
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn stable_unit_interval(seed: &str, value: &str) -> f64 {
    let hash = stable_hash64(seed, value);
    (hash as f64) / (u64::MAX as f64)
}

fn cmd_stats(args: StatsArgs) -> Result<()> {
    let records = read_records(&args.input)?;
    let words = records.len();
    let breaks: usize = records.iter().map(|r| r.breaks.len()).sum();
    let no_break = records.iter().filter(|r| r.breaks.is_empty()).count();
    let ambiguous = records.iter().filter(|r| r.ambiguous).count();
    let mut locales: Vec<_> = records
        .iter()
        .filter_map(|r| r.locale.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    locales.truncate(12);

    println!("records: {words}");
    println!("breaks: {breaks}");
    println!("no_break_words: {no_break}");
    println!("ambiguous_words: {ambiguous}");
    println!("locales: {}", locales.join(", "));
    Ok(())
}

