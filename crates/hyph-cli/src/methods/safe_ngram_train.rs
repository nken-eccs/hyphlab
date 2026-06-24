// Safe-ngram method parser and rule learning.

fn parse_safe_ngram_veto_options(
    method: &str,
) -> Result<(SafeNgramOptions, Option<SafeNgramOptions>)> {
    if let Some((add_part, veto_part)) = method.split_once("-veto-") {
        let add_options = parse_safe_ngram_options(add_part)?;
        let veto_options = parse_safe_ngram_options(&format!("safe-ngram-{veto_part}"))?;
        Ok((add_options, Some(veto_options)))
    } else {
        Ok((parse_safe_ngram_options(method)?, None))
    }
}

fn learn_safe_ngram_rules(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    options: &SafeNgramOptions,
) -> (U64HashSet, usize) {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();
    let mut trained_records = 0usize;

    for record in records {
        if record.ambiguous {
            continue;
        }
        if options.unicode_aware {
            let family_mask = safe_ngram_options_family_mask(options);
            let tables = safe_ngram_char_tables_if_simple(&record.word, family_mask)
                .unwrap_or_else(|| safe_ngram_grapheme_tables(&record.word, family_mask));
            let grapheme_len = tables.len;
            if grapheme_len < config.min_word_len {
                continue;
            }
            trained_records += 1;
            for boundary in config.left_min..=grapheme_len.saturating_sub(config.right_min) {
                let positive = record.breaks.contains(&(boundary as GraphemeIndex));
                for (spec_idx, spec) in options.specs.iter().enumerate() {
                    let key =
                        safe_ngram_grapheme_key(&tables, grapheme_len, boundary, spec_idx, *spec);
                    let slot = counts.entry(key).or_default();
                    if positive {
                        slot.positive = slot.positive.saturating_add(1);
                    } else {
                        slot.negative = slot.negative.saturating_add(1);
                    }
                }
            }
            continue;
        }

        if !record.word.is_ascii() {
            continue;
        }
        let bytes = record.word.as_bytes();
        if bytes.len() < config.min_word_len {
            continue;
        }
        trained_records += 1;
        for boundary in config.left_min..=bytes.len().saturating_sub(config.right_min) {
            let positive = record.breaks.contains(&(boundary as GraphemeIndex));
            for (spec_idx, spec) in options.specs.iter().enumerate() {
                let key = safe_ngram_key(bytes, boundary, spec_idx, *spec);
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    let rules = counts
        .into_iter()
        .filter_map(|(key, counts)| safe_ngram_counts_selected(counts, options).then_some(key))
        .collect::<U64HashSet>();
    (rules, trained_records)
}

fn learn_safe_ngram_veto_rules(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
    add_options: &SafeNgramOptions,
    add_rules: &U64HashSet,
    veto_options: &SafeNgramOptions,
) -> U64HashSet {
    let mut counts = U64HashMap::<SafeNgramCounts>::default();

    for record in records {
        if record.ambiguous {
            continue;
        }
        if add_options.unicode_aware || veto_options.unicode_aware {
            let family_mask = safe_ngram_family_mask(add_options, Some(veto_options));
            let tables = safe_ngram_char_tables_if_simple(&record.word, family_mask)
                .unwrap_or_else(|| safe_ngram_grapheme_tables(&record.word, family_mask));
            let grapheme_len = tables.len;
            if grapheme_len < config.min_word_len {
                continue;
            }
            for boundary in config.left_min..=grapheme_len.saturating_sub(config.right_min) {
                let add_hit = add_options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        let key = safe_ngram_grapheme_key(
                            &tables,
                            grapheme_len,
                            boundary,
                            spec_idx,
                            *spec,
                        );
                        add_rules.contains(&key)
                    });
                if !add_hit {
                    continue;
                }

                let positive = record.breaks.contains(&(boundary as GraphemeIndex));
                for (spec_idx, spec) in veto_options.specs.iter().enumerate() {
                    let key =
                        safe_ngram_grapheme_key(&tables, grapheme_len, boundary, spec_idx, *spec);
                    let slot = counts.entry(key).or_default();
                    if positive {
                        slot.positive = slot.positive.saturating_add(1);
                    } else {
                        slot.negative = slot.negative.saturating_add(1);
                    }
                }
            }
            continue;
        }

        if !record.word.is_ascii() {
            continue;
        }
        let bytes = record.word.as_bytes();
        if bytes.len() < config.min_word_len {
            continue;
        }
        for boundary in config.left_min..=bytes.len().saturating_sub(config.right_min) {
            let add_hit = add_options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    add_rules.contains(&safe_ngram_key(bytes, boundary, spec_idx, *spec))
                });
            if !add_hit {
                continue;
            }

            let positive = record.breaks.contains(&(boundary as GraphemeIndex));
            for (spec_idx, spec) in veto_options.specs.iter().enumerate() {
                let key = safe_ngram_key(bytes, boundary, spec_idx, *spec);
                let slot = counts.entry(key).or_default();
                if positive {
                    slot.positive = slot.positive.saturating_add(1);
                } else {
                    slot.negative = slot.negative.saturating_add(1);
                }
            }
        }
    }

    counts
        .into_iter()
        .filter_map(|(key, counts)| {
            safe_ngram_veto_counts_selected(counts, veto_options).then_some(key)
        })
        .collect::<U64HashSet>()
}

fn parse_safe_ngram_options(method: &str) -> Result<SafeNgramOptions> {
    let lower = method.to_ascii_lowercase();
    let mut specs = vec![SafeNgramSpec {
        left: 4,
        right: 4,
        bucketed: false,
        family: 0,
    }];
    let mut min_support = 2u32;
    let mut max_negative = 0u32;
    let mut max_negative_set = false;
    let mut min_precision_ppm = None;
    let mut min_wilson_ppm = None;
    let mut bucketed = false;
    let mut mix_bucketed = false;
    let mut family = 0u8;
    let mut mix_cv = false;
    let mut mix_sonority = false;
    let mut cap_vowel_nuclei = false;
    let mut orthographic_veto = false;
    let mut unicode_aware = false;

    for part in lower.split('-') {
        if matches!(
            part,
            "unicode" | "unicodeaware" | "unicode_aware" | "uni" | "uchar"
        ) {
            unicode_aware = true;
            continue;
        }
        if matches!(part, "ucv" | "unicodecv") {
            unicode_aware = true;
            family = 1;
            continue;
        }
        if matches!(
            part,
            "uson" | "usonority" | "unicodeson" | "unicodesonority"
        ) {
            unicode_aware = true;
            family = 2;
            continue;
        }
        if matches!(part, "cap" | "vowelcap" | "nucleuscap") {
            cap_vowel_nuclei = true;
            continue;
        }
        if matches!(
            part,
            "orthoveto" | "orthographicveto" | "structveto" | "shapeveto"
        ) {
            orthographic_veto = true;
            continue;
        }
        if matches!(part, "cv" | "shape" | "consonantvowel") {
            family = 1;
            continue;
        }
        if matches!(part, "mixcv" | "cvraw" | "rawcv" | "mixshape") {
            mix_cv = true;
            continue;
        }
        if matches!(part, "son" | "sonority") {
            family = 2;
            continue;
        }
        if matches!(part, "mixson" | "mixsonority" | "sonraw" | "rawson") {
            mix_sonority = true;
            continue;
        }
        if matches!(part, "bucket" | "bucketed" | "pos" | "position") {
            bucketed = true;
            continue;
        }
        if matches!(part, "mixbucket" | "bucketmix" | "mixedbucket") {
            mix_bucketed = true;
            continue;
        }
        if part == "multi" {
            specs = vec![
                SafeNgramSpec {
                    left: 5,
                    right: 5,
                    bucketed: false,
                    family,
                },
                SafeNgramSpec {
                    left: 4,
                    right: 4,
                    bucketed: false,
                    family,
                },
                SafeNgramSpec {
                    left: 3,
                    right: 3,
                    bucketed: false,
                    family,
                },
            ];
            continue;
        }
        if let Some((left, right)) = part.split_once('x') {
            let left = left
                .parse::<usize>()
                .with_context(|| format!("parse safe-ngram left context from {part:?}"))?;
            let right = right
                .parse::<usize>()
                .with_context(|| format!("parse safe-ngram right context from {part:?}"))?;
            anyhow::ensure!(
                left <= 5 && right <= 5 && left + right <= 10,
                "safe-ngram context must fit the packed key, got {left}x{right}"
            );
            specs = vec![SafeNgramSpec {
                left,
                right,
                bucketed: false,
                family,
            }];
            continue;
        }
        if let Some(value) = part.strip_prefix('s') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                min_support = value
                    .parse::<u32>()
                    .with_context(|| format!("parse safe-ngram support from {part:?}"))?;
            }
        }
        if let Some(value) = part.strip_prefix('n') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                max_negative = value
                    .parse::<u32>()
                    .with_context(|| format!("parse safe-ngram max negatives from {part:?}"))?;
                max_negative_set = true;
            }
        }
        if let Some(value) = part.strip_prefix('p') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let value = value
                    .parse::<u32>()
                    .with_context(|| format!("parse safe-ngram precision from {part:?}"))?;
                anyhow::ensure!(
                    (1..=999_999).contains(&value),
                    "safe-ngram precision threshold must be in 1..=999999"
                );
                min_precision_ppm = Some(if value <= 100 {
                    value * 10_000
                } else if value <= 1000 {
                    value * 1000
                } else {
                    value
                });
            }
        }
        if let Some(value) = part.strip_prefix('w') {
            if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
                let value = value
                    .parse::<u32>()
                    .with_context(|| format!("parse safe-ngram Wilson threshold from {part:?}"))?;
                anyhow::ensure!(
                    (1..=999_999).contains(&value),
                    "safe-ngram Wilson threshold must be in 1..=999999"
                );
                min_wilson_ppm = Some(if value <= 100 {
                    value * 10_000
                } else if value <= 1000 {
                    value * 1000
                } else {
                    value
                });
            }
        }
    }

    anyhow::ensure!(min_support > 0, "safe-ngram support must be positive");
    if (min_precision_ppm.is_some() || min_wilson_ppm.is_some()) && !max_negative_set {
        max_negative = u32::MAX;
    }
    for spec in &mut specs {
        spec.family = family;
    }
    if bucketed {
        for spec in &mut specs {
            spec.bucketed = true;
        }
    } else if mix_bucketed {
        let mut bucketed_specs = specs.clone();
        for spec in &mut bucketed_specs {
            spec.bucketed = true;
        }
        specs.extend(bucketed_specs);
    }
    if mix_cv {
        let mut cv_specs = specs.clone();
        for spec in &mut cv_specs {
            spec.family = 1;
        }
        specs.extend(cv_specs);
    }
    if mix_sonority {
        let mut sonority_specs = specs.clone();
        for spec in &mut sonority_specs {
            spec.family = 2;
        }
        specs.extend(sonority_specs);
    }
    Ok(SafeNgramOptions {
        specs,
        min_support,
        max_negative,
        min_precision_ppm,
        min_wilson_ppm,
        cap_vowel_nuclei,
        orthographic_veto,
        unicode_aware,
    })
}

fn safe_ngram_counts_selected(counts: SafeNgramCounts, options: &SafeNgramOptions) -> bool {
    if counts.positive < options.min_support || counts.negative > options.max_negative {
        return false;
    }
    if let Some(min_precision_ppm) = options.min_precision_ppm {
        let total = counts.positive.saturating_add(counts.negative);
        return u64::from(counts.positive) * 1_000_000
            >= u64::from(total) * u64::from(min_precision_ppm);
    }
    if let Some(min_wilson_ppm) = options.min_wilson_ppm {
        return safe_ngram_wilson_lower_ppm(counts.positive, counts.negative)
            >= f64::from(min_wilson_ppm);
    }
    true
}

fn safe_ngram_veto_counts_selected(counts: SafeNgramCounts, options: &SafeNgramOptions) -> bool {
    safe_ngram_counts_selected(
        SafeNgramCounts {
            positive: counts.negative,
            negative: counts.positive,
        },
        options,
    )
}

fn safe_ngram_vowel_break_cap(bytes: &[u8]) -> usize {
    let mut nuclei = 0usize;
    let mut in_vowel = false;
    for byte in bytes {
        if is_safe_ngram_vowelish(*byte) {
            if !in_vowel {
                nuclei += 1;
            }
            in_vowel = true;
        } else {
            in_vowel = false;
        }
    }
    nuclei.saturating_sub(1)
}

