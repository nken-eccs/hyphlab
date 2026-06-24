// Italian onset-syllable learner and runtime.

impl ItalianSyllableMethod {
    fn new(method: &str, options: &MethodOptions) -> Result<Self> {
        anyhow::ensure!(
            normalize_locale_match_key(&options.locale).starts_with("it"),
            "italian-syllable requires an Italian locale, got {}",
            options.locale
        );
        let mut config = italian_syllable_default_config();
        apply_config_overrides(&mut config, options);
        let learned_splits = if let Some(path) = options.dictionary.as_ref() {
            let records = read_records(path)?;
            learn_italian_syllable_splits(&records, &config)
        } else {
            U64HashMap::default()
        };
        Ok(Self {
            id: format!("{method}:clusters{}", learned_splits.len()),
            config,
            learned_splits,
        })
    }

    fn from_model(path: &Path, locale: &str, model: ItalianSyllableModelFile) -> Result<Self> {
        anyhow::ensure!(
            model.schema_version == 1,
            "unsupported Italian syllable model schema version {} in {}",
            model.schema_version,
            path.display()
        );
        anyhow::ensure!(
            normalize_locale_match_key(locale).starts_with("it"),
            "italian-syllable-model requires an Italian locale, got {}",
            locale
        );
        anyhow::ensure!(
            normalize_locale_match_key(&model.locale).starts_with("it"),
            "Italian syllable model locale {} is not Italian in {}",
            model.locale,
            path.display()
        );
        let id = model.id.clone();
        let config = model.config.clone();
        let learned_splits = model.into_learned_splits(path)?;
        Ok(Self {
            id: format!("{id}:model:{}", file_stem(path)),
            config,
            learned_splits,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        let chars = word
            .chars()
            .map(italian_lower_char)
            .collect::<SmallVec<[char; 32]>>();
        let len = chars.len();
        if len < self.config.min_word_len {
            return Ok(());
        }

        let mut vowels = SmallVec::<[usize; 8]>::new();
        for idx in 0..len {
            if italian_is_vowel_nucleus(&chars, idx) {
                vowels.push(idx);
            }
        }
        if vowels.len() < 2 {
            return Ok(());
        }

        for pair in vowels.windows(2) {
            let left_vowel = pair[0];
            let right_vowel = pair[1];
            if right_vowel <= left_vowel + 1 {
                let key = italian_adjacent_vowels_key(&chars, left_vowel, right_vowel);
                let learned = self.learned_splits.get(&key).copied();
                let should_break = learned.map(|split| split != 0).unwrap_or_else(|| {
                    italian_adjacent_vowels_break(&chars, left_vowel, right_vowel)
                });
                if should_break {
                    self.push_italian_boundary(left_vowel + 1, len, out);
                }
                continue;
            }

            let cluster_start = left_vowel + 1;
            let cluster_end = right_vowel;
            let cluster_len = cluster_end - cluster_start;
            if cluster_len == 0 {
                continue;
            }
            if italian_cluster_is_all_non_letters(&chars[cluster_start..cluster_end]) {
                continue;
            }

            let key = italian_cluster_key(&chars[cluster_start..cluster_end]);
            let learned = self.learned_splits.get(&key).copied();
            if learned == Some(0) {
                continue;
            }
            let onset_len = learned
                .map(usize::from)
                .unwrap_or_else(|| italian_best_onset_len(&chars, cluster_start, cluster_end));
            let boundary = cluster_end.saturating_sub(onset_len.max(1));
            self.push_italian_boundary(boundary, len, out);
        }

        out.sort_unstable();
        out.dedup();
        Ok(())
    }

    fn push_italian_boundary(
        &self,
        boundary: usize,
        len: usize,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) {
        if boundary >= self.config.left_min && len.saturating_sub(boundary) >= self.config.right_min
        {
            out.push(boundary as GraphemeIndex);
        }
    }
}

fn italian_syllable_default_config() -> HyphenationConfig {
    HyphenationConfig {
        right_min: 2,
        min_word_len: 4,
        ..HyphenationConfig::default()
    }
}

fn italian_lower_char(ch: char) -> char {
    if ch.is_ascii() {
        ch.to_ascii_lowercase()
    } else {
        ch.to_lowercase().next().unwrap_or(ch)
    }
}

fn learn_italian_syllable_splits(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
) -> U64HashMap<u8> {
    let mut counts = U64HashMap::<ItalianSplitCounts>::default();
    for record in records {
        if record.ambiguous {
            continue;
        }
        let chars = record
            .word
            .chars()
            .map(italian_lower_char)
            .collect::<SmallVec<[char; 32]>>();
        let len = chars.len();
        if len < config.min_word_len {
            continue;
        }
        let mut vowels = SmallVec::<[usize; 8]>::new();
        for idx in 0..len {
            if italian_is_vowel_nucleus(&chars, idx) {
                vowels.push(idx);
            }
        }
        for pair in vowels.windows(2) {
            let left_vowel = pair[0];
            let right_vowel = pair[1];
            let cluster_start = left_vowel + 1;
            let cluster_end = right_vowel;
            let Some(gold_boundary) =
                italian_gold_boundary_in_interval(&record.breaks, cluster_start, cluster_end)
            else {
                continue;
            };
            if right_vowel <= left_vowel + 1 {
                let key = italian_adjacent_vowels_key(&chars, left_vowel, right_vowel);
                let split = if gold_boundary == 0 { 0 } else { 1 };
                let slot = counts.entry(key).or_default();
                slot.counts[split] = slot.counts[split].saturating_add(1);
                continue;
            }
            if italian_cluster_is_all_non_letters(&chars[cluster_start..cluster_end]) {
                continue;
            }
            let key = italian_cluster_key(&chars[cluster_start..cluster_end]);
            let split = if gold_boundary == 0 {
                0
            } else {
                (cluster_end.saturating_sub(gold_boundary as usize)).min(4)
            };
            let slot = counts.entry(key).or_default();
            slot.counts[split] = slot.counts[split].saturating_add(1);
        }
    }

    let mut learned = U64HashMap::<u8>::default();
    for (key, split_counts) in counts {
        let total = split_counts.counts.iter().copied().sum::<u32>();
        if total < 2 {
            continue;
        }
        let (best_split, best_count) = split_counts
            .counts
            .iter()
            .copied()
            .enumerate()
            .max_by_key(|(_, count)| *count)
            .unwrap_or((0, 0));
        if best_count.saturating_mul(4) >= total.saturating_mul(3) {
            learned.insert(key, best_split as u8);
        }
    }
    learned
}

fn count_italian_syllable_training_records(
    records: &[HyphenationRecord],
    config: &HyphenationConfig,
) -> usize {
    records
        .iter()
        .filter(|record| !record.ambiguous && record.word.chars().count() >= config.min_word_len)
        .count()
}

fn italian_gold_boundary_in_interval(
    breaks: &[GraphemeIndex],
    cluster_start: usize,
    cluster_end: usize,
) -> Option<GraphemeIndex> {
    let mut found = None;
    for boundary in breaks.iter().copied() {
        let boundary_usize = boundary as usize;
        if (cluster_start..=cluster_end).contains(&boundary_usize) {
            if found.is_some() {
                return None;
            }
            found = Some(boundary);
        }
    }
    Some(found.unwrap_or(0))
}

fn italian_adjacent_vowels_key(chars: &[char], left: usize, right: usize) -> u64 {
    0xA0u64 << 56
        | (u64::from(italian_char_code(chars[left])) << 8)
        | u64::from(italian_char_code(chars[right]))
}

fn italian_cluster_key(cluster: &[char]) -> u64 {
    let mut key = (cluster.len().min(7) as u64) << 56;
    for (idx, ch) in cluster.iter().take(7).enumerate() {
        key |= u64::from(italian_char_code(*ch)) << (idx * 8);
    }
    key
}

fn italian_char_code(ch: char) -> u8 {
    match ch {
        'a' | 'à' | 'á' | 'â' | 'ä' => b'a',
        'e' | 'è' | 'é' | 'ê' | 'ë' => b'e',
        'i' | 'ì' | 'í' | 'î' | 'ï' => b'i',
        'o' | 'ò' | 'ó' | 'ô' | 'ö' => b'o',
        'u' | 'ù' | 'ú' | 'û' | 'ü' => b'u',
        ch if ch.is_ascii() => ch as u8,
        _ => 0x7f,
    }
}

fn italian_is_vowel_char(ch: char) -> bool {
    matches!(
        ch,
        'a' | 'à'
            | 'á'
            | 'â'
            | 'ä'
            | 'e'
            | 'è'
            | 'é'
            | 'ê'
            | 'ë'
            | 'i'
            | 'ì'
            | 'í'
            | 'î'
            | 'ï'
            | 'o'
            | 'ò'
            | 'ó'
            | 'ô'
            | 'ö'
            | 'u'
            | 'ù'
            | 'ú'
            | 'û'
            | 'ü'
    )
}

fn italian_is_vowel_nucleus(chars: &[char], idx: usize) -> bool {
    let ch = chars[idx];
    if !italian_is_vowel_char(ch) {
        return false;
    }
    if ch == 'u'
        && idx > 0
        && matches!(chars[idx - 1], 'q' | 'g')
        && idx + 1 < chars.len()
        && italian_is_vowel_char(chars[idx + 1])
    {
        return false;
    }
    true
}

fn italian_is_consonant_char(ch: char) -> bool {
    (ch.is_alphabetic() || ch == '\'') && !italian_is_vowel_char(ch)
}

fn italian_cluster_is_all_non_letters(cluster: &[char]) -> bool {
    cluster.iter().all(|ch| !ch.is_alphabetic())
}

fn italian_adjacent_vowels_break(chars: &[char], left: usize, right: usize) -> bool {
    let l = chars[left];
    let r = chars[right];
    if matches!((l, r), ('i', _) | ('u', _) | (_, 'i') | (_, 'u')) {
        return false;
    }
    l != r
}

fn italian_best_onset_len(chars: &[char], start: usize, end: usize) -> usize {
    let cluster_len = end - start;
    let max_onset = cluster_len.min(3);
    for onset_len in (1..=max_onset).rev() {
        let onset_start = end - onset_len;
        if italian_legal_onset(&chars[onset_start..end]) {
            return onset_len;
        }
    }
    1
}

fn italian_legal_onset(onset: &[char]) -> bool {
    match onset {
        [a] => italian_is_consonant_char(*a) && *a != 'h',
        ['q', 'u'] | ['g', 'u'] | ['c', 'h'] | ['g', 'h'] | ['g', 'n'] => true,
        ['g', 'l'] => true,
        ['s', b] => italian_is_consonant_char(*b),
        [a, b] if matches!(*b, 'l' | 'r') => {
            matches!(*a, 'b' | 'c' | 'd' | 'f' | 'g' | 'p' | 't' | 'v')
        }
        ['s', a, b] => italian_legal_onset(&[*a, *b]),
        _ => false,
    }
}

