// Shared fragment policy for typesetting-oriented corpora and runtime models.
//
// Fragment files use one entry per line. Bare entries and `both:` entries block
// the fragment on either side of a hyphenation break; `prefix:`/`left:` and
// `suffix:`/`right:` block only the visible fragment before or after the break.
// This keeps Spanish/Turkish-style false positives out of the active policy.

#[derive(Debug, Clone)]
struct SensitiveFragmentRules {
    prefix: BTreeSet<String>,
    suffix: BTreeSet<String>,
}

impl SensitiveFragmentRules {
    fn new() -> Self {
        Self {
            prefix: BTreeSet::new(),
            suffix: BTreeSet::new(),
        }
    }

    fn insert_both(&mut self, fragment: String) {
        self.prefix.insert(fragment.clone());
        self.suffix.insert(fragment);
    }
}

struct TypesetFragmentFilter {
    unicode: SensitiveFragmentRules,
    ascii_prefix_by_len: BTreeMap<usize, Vec<Vec<u8>>>,
    ascii_suffix_by_len: BTreeMap<usize, Vec<Vec<u8>>>,
    max_ascii_len: usize,
}

impl TypesetFragmentFilter {
    fn new(unicode: SensitiveFragmentRules) -> Self {
        let mut ascii_prefix_by_len = BTreeMap::<usize, Vec<Vec<u8>>>::new();
        let mut ascii_suffix_by_len = BTreeMap::<usize, Vec<Vec<u8>>>::new();
        let mut max_ascii_len = 0usize;
        for fragment in &unicode.prefix {
            if fragment.is_ascii() {
                max_ascii_len = max_ascii_len.max(fragment.len());
                ascii_prefix_by_len
                    .entry(fragment.len())
                    .or_default()
                    .push(fragment.as_bytes().to_vec());
            }
        }
        for fragment in &unicode.suffix {
            if fragment.is_ascii() {
                max_ascii_len = max_ascii_len.max(fragment.len());
                ascii_suffix_by_len
                    .entry(fragment.len())
                    .or_default()
                    .push(fragment.as_bytes().to_vec());
            }
        }
        Self {
            unicode,
            ascii_prefix_by_len,
            ascii_suffix_by_len,
            max_ascii_len,
        }
    }
}

fn load_sensitive_fragments(path: &Path) -> Result<SensitiveFragmentRules> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut fragments = SensitiveFragmentRules::new();
    for (line_no, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        let mut value = line.split('#').next().unwrap_or("").trim();
        if value.is_empty() {
            continue;
        }
        let side = if let Some(rest) = value.strip_prefix("prefix:") {
            value = rest.trim();
            "prefix"
        } else if let Some(rest) = value.strip_prefix("left:") {
            value = rest.trim();
            "prefix"
        } else if let Some(rest) = value.strip_prefix("suffix:") {
            value = rest.trim();
            "suffix"
        } else if let Some(rest) = value.strip_prefix("right:") {
            value = rest.trim();
            "suffix"
        } else if let Some(rest) = value.strip_prefix("both:") {
            value = rest.trim();
            "both"
        } else {
            "both"
        };
        anyhow::ensure!(
            value.chars().all(|ch| ch.is_alphabetic()),
            "{}:{} sensitive fragment must be alphabetic",
            path.display(),
            line_no + 1
        );
        let value = value.to_lowercase();
        match side {
            "prefix" => {
                fragments.prefix.insert(value);
            }
            "suffix" => {
                fragments.suffix.insert(value);
            }
            _ => fragments.insert_both(value),
        }
    }
    Ok(fragments)
}

fn curate_break_set(
    word: &str,
    breaks: &[GraphemeIndex],
    left_min: usize,
    right_min: usize,
    sensitive: &SensitiveFragmentRules,
) -> (SmallVec<[GraphemeIndex; 8]>, Vec<String>) {
    let graphemes = word.graphemes(true).collect::<Vec<_>>();
    let spans = alphabetic_spans(&graphemes);
    let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
    let mut reasons = Vec::new();

    for break_idx in breaks.iter().copied() {
        let idx = usize::from(break_idx);
        let Some((start, end)) = spans
            .iter()
            .copied()
            .find(|(start, end)| *start < idx && idx < *end)
        else {
            reasons.push(format!("remove:{break_idx}:outside_alpha_token"));
            continue;
        };

        let left_len = idx.saturating_sub(start);
        let right_len = end.saturating_sub(idx);
        if left_len < left_min {
            reasons.push(format!("remove:{break_idx}:left_min"));
            continue;
        }
        if right_len < right_min {
            reasons.push(format!("remove:{break_idx}:right_min"));
            continue;
        }

        let prefix = graphemes[start..idx].concat().to_lowercase();
        let suffix = graphemes[idx..end].concat().to_lowercase();
        if sensitive.prefix.contains(&prefix) {
            reasons.push(format!("remove:{break_idx}:sensitive_prefix:{prefix}"));
            continue;
        }
        if sensitive.suffix.contains(&suffix) {
            reasons.push(format!("remove:{break_idx}:sensitive_suffix:{suffix}"));
            continue;
        }

        out.push(break_idx);
    }

    out.sort_unstable();
    out.dedup();
    (out, reasons)
}

fn filter_typeset_fragments(
    word: &str,
    config: &HyphenationConfig,
    fragments: &TypesetFragmentFilter,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if out.is_empty() {
        return;
    }
    if word.is_ascii() {
        filter_typeset_fragments_ascii(word.as_bytes(), config, fragments, out);
        return;
    }

    let graphemes = word.graphemes(true).collect::<Vec<_>>();
    let spans = alphabetic_spans(&graphemes);
    out.retain(|break_idx| {
        let idx = usize::from(*break_idx);
        let Some((start, end)) = spans
            .iter()
            .copied()
            .find(|(start, end)| *start < idx && idx < *end)
        else {
            return false;
        };

        if idx.saturating_sub(start) < config.left_min || end.saturating_sub(idx) < config.right_min
        {
            return false;
        }

        let prefix = graphemes[start..idx].concat().to_lowercase();
        let suffix = graphemes[idx..end].concat().to_lowercase();
        !fragments.unicode.prefix.contains(&prefix) && !fragments.unicode.suffix.contains(&suffix)
    });
}

fn filter_typeset_fragments_ascii(
    bytes: &[u8],
    config: &HyphenationConfig,
    fragments: &TypesetFragmentFilter,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    out.retain(|break_idx| {
        let idx = usize::from(*break_idx);
        if idx == 0 || idx >= bytes.len() {
            return false;
        }
        if !bytes[idx - 1].is_ascii_alphabetic() || !bytes[idx].is_ascii_alphabetic() {
            return false;
        }

        let cap = fragments
            .max_ascii_len
            .max(config.left_min)
            .max(config.right_min);
        let left_len = ascii_left_alpha_len_capped(bytes, idx, cap);
        let right_len = ascii_right_alpha_len_capped(bytes, idx, cap);
        if left_len < config.left_min || right_len < config.right_min {
            return false;
        }

        let prefix_blocked = left_len <= fragments.max_ascii_len
            && ascii_fragment_blocked(
                &bytes[idx - left_len..idx],
                &fragments.ascii_prefix_by_len,
            );
        let suffix_blocked = right_len <= fragments.max_ascii_len
            && ascii_fragment_blocked(
                &bytes[idx..idx + right_len],
                &fragments.ascii_suffix_by_len,
            );
        !prefix_blocked && !suffix_blocked
    });
}

fn alphabetic_spans(graphemes: &[&str]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (idx, grapheme) in graphemes.iter().enumerate() {
        let is_alpha = grapheme.chars().any(|ch| ch.is_alphabetic());
        match (start, is_alpha) {
            (None, true) => start = Some(idx),
            (Some(open), false) => {
                spans.push((open, idx));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(open) = start {
        spans.push((open, graphemes.len()));
    }
    spans
}

fn ascii_left_alpha_len_capped(bytes: &[u8], idx: usize, cap: usize) -> usize {
    let mut len = 0usize;
    let mut pos = idx;
    while pos > 0 && bytes[pos - 1].is_ascii_alphabetic() {
        if len >= cap {
            return cap + 1;
        }
        len += 1;
        pos -= 1;
    }
    len
}

fn ascii_right_alpha_len_capped(bytes: &[u8], idx: usize, cap: usize) -> usize {
    let mut len = 0usize;
    let mut pos = idx;
    while pos < bytes.len() && bytes[pos].is_ascii_alphabetic() {
        if len >= cap {
            return cap + 1;
        }
        len += 1;
        pos += 1;
    }
    len
}

fn ascii_fragment_blocked(
    fragment: &[u8],
    by_len: &BTreeMap<usize, Vec<Vec<u8>>>,
) -> bool {
    by_len.get(&fragment.len()).is_some_and(|candidates| {
        candidates
            .iter()
            .any(|blocked| fragment.eq_ignore_ascii_case(blocked))
    })
}
