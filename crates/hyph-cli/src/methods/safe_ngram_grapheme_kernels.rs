// Grapheme-aware safe-ngram kernels.

// Grapheme-aware safe-ngram scoring and lookup kernels.

fn safe_ngram_hyphenate_grapheme_single_spec(
    codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    spec: SafeNgramSpec,
    rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width = spec.left + spec.right;
    debug_assert!(width > 0 && width <= 10);
    let last_shift = ((width - 1) * 5) as u32;
    let mut key = safe_ngram_grapheme_key_from_codes(codes, start, spec);
    for boundary in start..=end {
        if rules.contains(&key) {
            out.push(boundary as GraphemeIndex);
        }
        let next_code_position = boundary as isize + 1 + spec.right as isize;
        key = (key >> 5) | (safe_ngram_grapheme_code_at(codes, next_code_position) << last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_single_spec_lookup(
    codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    spec: SafeNgramSpec,
    rules: SafeNgramRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let SafeNgramRuleLookup::Hash(rules) = rules {
        safe_ngram_hyphenate_grapheme_single_spec(codes, grapheme_len, config, spec, rules, out);
        return;
    }

    debug_assert!(!spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width = spec.left + spec.right;
    debug_assert!(width > 0 && width <= 10);
    let last_shift = ((width - 1) * 5) as u32;
    let mut key = safe_ngram_grapheme_key_from_codes(codes, start, spec);
    for boundary in start..=end {
        if rules.contains(key) {
            out.push(boundary as GraphemeIndex);
        }
        let next_code_position = boundary as isize + 1 + spec.right as isize;
        key = (key >> 5) | (safe_ngram_grapheme_code_at(codes, next_code_position) << last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_single_add_veto(
    add_codes: &[u8],
    veto_codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec: SafeNgramSpec,
    add_rules: &U64HashSet,
    veto_spec: SafeNgramSpec,
    veto_rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!add_spec.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width = add_spec.left + add_spec.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width > 0 && add_width <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift = ((add_width - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key = safe_ngram_grapheme_key_from_codes(add_codes, start, add_spec);
    let mut veto_key = safe_ngram_grapheme_key_from_codes(veto_codes, start, veto_spec);
    for boundary in start..=end {
        if add_rules.contains(&add_key) && !veto_rules.contains(&veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next_code_position = boundary as isize + 1 + add_spec.right as isize;
        let veto_next_code_position = boundary as isize + 1 + veto_spec.right as isize;
        add_key = (add_key >> 5)
            | (safe_ngram_grapheme_code_at(add_codes, add_next_code_position) << add_last_shift);
        veto_key = (veto_key >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes, veto_next_code_position) << veto_last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_single_add_veto_lookup(
    add_codes: &[u8],
    veto_codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec: SafeNgramSpec,
    add_rules: SafeNgramRuleLookup<'_>,
    veto_spec: SafeNgramSpec,
    veto_rules: SafeNgramRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let (SafeNgramRuleLookup::Hash(add_rules), SafeNgramRuleLookup::Hash(veto_rules)) =
        (add_rules, veto_rules)
    {
        safe_ngram_hyphenate_grapheme_single_add_veto(
            add_codes,
            veto_codes,
            grapheme_len,
            config,
            add_spec,
            add_rules,
            veto_spec,
            veto_rules,
            out,
        );
        return;
    }

    debug_assert!(!add_spec.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width = add_spec.left + add_spec.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width > 0 && add_width <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift = ((add_width - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key = safe_ngram_grapheme_key_from_codes(add_codes, start, add_spec);
    let mut veto_key = safe_ngram_grapheme_key_from_codes(veto_codes, start, veto_spec);
    for boundary in start..=end {
        if add_rules.contains(add_key) && !veto_rules.contains(veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next_code_position = boundary as isize + 1 + add_spec.right as isize;
        let veto_next_code_position = boundary as isize + 1 + veto_spec.right as isize;
        add_key = (add_key >> 5)
            | (safe_ngram_grapheme_code_at(add_codes, add_next_code_position) << add_last_shift);
        veto_key = (veto_key >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes, veto_next_code_position) << veto_last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_spec(
    codes0: &[u8],
    codes1: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    spec0: SafeNgramSpec,
    spec1: SafeNgramSpec,
    rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!spec0.bucketed);
    debug_assert!(!spec1.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width0 = spec0.left + spec0.right;
    let width1 = spec1.left + spec1.right;
    debug_assert!(width0 > 0 && width0 <= 10);
    debug_assert!(width1 > 0 && width1 <= 10);
    let last_shift0 = ((width0 - 1) * 5) as u32;
    let last_shift1 = ((width1 - 1) * 5) as u32;
    let mut key0 = safe_ngram_grapheme_key_from_codes(codes0, start, spec0);
    let mut key1 = safe_ngram_grapheme_key_from_codes(codes1, start, spec1);
    const SPEC1_PREFIX: u64 = 1u64 << 56;
    for boundary in start..=end {
        if rules.contains(&key0) || rules.contains(&(SPEC1_PREFIX | key1)) {
            out.push(boundary as GraphemeIndex);
        }
        let next0 = boundary as isize + 1 + spec0.right as isize;
        let next1 = boundary as isize + 1 + spec1.right as isize;
        key0 = (key0 >> 5) | (safe_ngram_grapheme_code_at(codes0, next0) << last_shift0);
        key1 = (key1 >> 5) | (safe_ngram_grapheme_code_at(codes1, next1) << last_shift1);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_spec_lookup(
    codes0: &[u8],
    codes1: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    spec0: SafeNgramSpec,
    spec1: SafeNgramSpec,
    rules: SafeNgramDualRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let SafeNgramDualRuleLookup::Hash(rules) = rules {
        safe_ngram_hyphenate_grapheme_dual_spec(
            codes0,
            codes1,
            grapheme_len,
            config,
            spec0,
            spec1,
            rules,
            out,
        );
        return;
    }

    debug_assert!(!spec0.bucketed);
    debug_assert!(!spec1.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let width0 = spec0.left + spec0.right;
    let width1 = spec1.left + spec1.right;
    debug_assert!(width0 > 0 && width0 <= 10);
    debug_assert!(width1 > 0 && width1 <= 10);
    let last_shift0 = ((width0 - 1) * 5) as u32;
    let last_shift1 = ((width1 - 1) * 5) as u32;
    let mut key0 = safe_ngram_grapheme_key_from_codes(codes0, start, spec0);
    let mut key1 = safe_ngram_grapheme_key_from_codes(codes1, start, spec1);
    for boundary in start..=end {
        if rules.contains(key0, key1) {
            out.push(boundary as GraphemeIndex);
        }
        let next0 = boundary as isize + 1 + spec0.right as isize;
        let next1 = boundary as isize + 1 + spec1.right as isize;
        key0 = (key0 >> 5) | (safe_ngram_grapheme_code_at(codes0, next0) << last_shift0);
        key1 = (key1 >> 5) | (safe_ngram_grapheme_code_at(codes1, next1) << last_shift1);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_add_veto(
    add_codes0: &[u8],
    add_codes1: &[u8],
    veto_codes0: &[u8],
    veto_codes1: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: &U64HashSet,
    veto_spec0: SafeNgramSpec,
    veto_spec1: SafeNgramSpec,
    veto_rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec0.bucketed);
    debug_assert!(!veto_spec1.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width0 = veto_spec0.left + veto_spec0.right;
    let veto_width1 = veto_spec1.left + veto_spec1.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width0 > 0 && veto_width0 <= 10);
    debug_assert!(veto_width1 > 0 && veto_width1 <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift0 = ((veto_width0 - 1) * 5) as u32;
    let veto_last_shift1 = ((veto_width1 - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_grapheme_key_from_codes(add_codes0, start, add_spec0);
    let mut add_key1 = safe_ngram_grapheme_key_from_codes(add_codes1, start, add_spec1);
    let mut veto_key0 = safe_ngram_grapheme_key_from_codes(veto_codes0, start, veto_spec0);
    let mut veto_key1 = safe_ngram_grapheme_key_from_codes(veto_codes1, start, veto_spec1);
    const SPEC1_PREFIX: u64 = 1u64 << 56;
    for boundary in start..=end {
        let add_hit =
            add_rules.contains(&add_key0) || add_rules.contains(&(SPEC1_PREFIX | add_key1));
        if add_hit {
            let veto_hit =
                veto_rules.contains(&veto_key0) || veto_rules.contains(&(SPEC1_PREFIX | veto_key1));
            if !veto_hit {
                out.push(boundary as GraphemeIndex);
            }
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next0 = boundary as isize + 1 + veto_spec0.right as isize;
        let veto_next1 = boundary as isize + 1 + veto_spec1.right as isize;
        add_key0 = (add_key0 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes0, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes1, add_next1) << add_last_shift1);
        veto_key0 = (veto_key0 >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes0, veto_next0) << veto_last_shift0);
        veto_key1 = (veto_key1 >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes1, veto_next1) << veto_last_shift1);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_add_veto_lookup(
    add_codes0: &[u8],
    add_codes1: &[u8],
    veto_codes0: &[u8],
    veto_codes1: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: SafeNgramDualRuleLookup<'_>,
    veto_spec0: SafeNgramSpec,
    veto_spec1: SafeNgramSpec,
    veto_rules: SafeNgramDualRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let (SafeNgramDualRuleLookup::Hash(add_rules), SafeNgramDualRuleLookup::Hash(veto_rules)) =
        (add_rules, veto_rules)
    {
        safe_ngram_hyphenate_grapheme_dual_add_veto(
            add_codes0,
            add_codes1,
            veto_codes0,
            veto_codes1,
            grapheme_len,
            config,
            add_spec0,
            add_spec1,
            add_rules,
            veto_spec0,
            veto_spec1,
            veto_rules,
            out,
        );
        return;
    }

    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec0.bucketed);
    debug_assert!(!veto_spec1.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width0 = veto_spec0.left + veto_spec0.right;
    let veto_width1 = veto_spec1.left + veto_spec1.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width0 > 0 && veto_width0 <= 10);
    debug_assert!(veto_width1 > 0 && veto_width1 <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift0 = ((veto_width0 - 1) * 5) as u32;
    let veto_last_shift1 = ((veto_width1 - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_grapheme_key_from_codes(add_codes0, start, add_spec0);
    let mut add_key1 = safe_ngram_grapheme_key_from_codes(add_codes1, start, add_spec1);
    let mut veto_key0 = safe_ngram_grapheme_key_from_codes(veto_codes0, start, veto_spec0);
    let mut veto_key1 = safe_ngram_grapheme_key_from_codes(veto_codes1, start, veto_spec1);
    for boundary in start..=end {
        if add_rules.contains(add_key0, add_key1) && !veto_rules.contains(veto_key0, veto_key1) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next0 = boundary as isize + 1 + veto_spec0.right as isize;
        let veto_next1 = boundary as isize + 1 + veto_spec1.right as isize;
        add_key0 = (add_key0 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes0, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes1, add_next1) << add_last_shift1);
        veto_key0 = (veto_key0 >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes0, veto_next0) << veto_last_shift0);
        veto_key1 = (veto_key1 >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes1, veto_next1) << veto_last_shift1);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_add_single_veto(
    add_codes0: &[u8],
    add_codes1: &[u8],
    veto_codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: &U64HashSet,
    veto_spec: SafeNgramSpec,
    veto_rules: &U64HashSet,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_grapheme_key_from_codes(add_codes0, start, add_spec0);
    let mut add_key1 = safe_ngram_grapheme_key_from_codes(add_codes1, start, add_spec1);
    let mut veto_key = safe_ngram_grapheme_key_from_codes(veto_codes, start, veto_spec);
    const SPEC1_PREFIX: u64 = 1u64 << 56;
    for boundary in start..=end {
        let add_hit =
            add_rules.contains(&add_key0) || add_rules.contains(&(SPEC1_PREFIX | add_key1));
        if add_hit && !veto_rules.contains(&veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next = boundary as isize + 1 + veto_spec.right as isize;
        add_key0 = (add_key0 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes0, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes1, add_next1) << add_last_shift1);
        veto_key = (veto_key >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes, veto_next) << veto_last_shift);
    }
}

fn safe_ngram_hyphenate_grapheme_dual_add_single_veto_lookup(
    add_codes0: &[u8],
    add_codes1: &[u8],
    veto_codes: &[u8],
    grapheme_len: usize,
    config: &HyphenationConfig,
    add_spec0: SafeNgramSpec,
    add_spec1: SafeNgramSpec,
    add_rules: SafeNgramDualRuleLookup<'_>,
    veto_spec: SafeNgramSpec,
    veto_rules: SafeNgramRuleLookup<'_>,
    out: &mut SmallVec<[GraphemeIndex; 8]>,
) {
    if let (SafeNgramDualRuleLookup::Hash(add_rules), SafeNgramRuleLookup::Hash(veto_rules)) =
        (add_rules, veto_rules)
    {
        safe_ngram_hyphenate_grapheme_dual_add_single_veto(
            add_codes0,
            add_codes1,
            veto_codes,
            grapheme_len,
            config,
            add_spec0,
            add_spec1,
            add_rules,
            veto_spec,
            veto_rules,
            out,
        );
        return;
    }

    debug_assert!(!add_spec0.bucketed);
    debug_assert!(!add_spec1.bucketed);
    debug_assert!(!veto_spec.bucketed);
    let start = config.left_min;
    let end = grapheme_len.saturating_sub(config.right_min);
    if start > end {
        return;
    }

    let add_width0 = add_spec0.left + add_spec0.right;
    let add_width1 = add_spec1.left + add_spec1.right;
    let veto_width = veto_spec.left + veto_spec.right;
    debug_assert!(add_width0 > 0 && add_width0 <= 10);
    debug_assert!(add_width1 > 0 && add_width1 <= 10);
    debug_assert!(veto_width > 0 && veto_width <= 10);
    let add_last_shift0 = ((add_width0 - 1) * 5) as u32;
    let add_last_shift1 = ((add_width1 - 1) * 5) as u32;
    let veto_last_shift = ((veto_width - 1) * 5) as u32;
    let mut add_key0 = safe_ngram_grapheme_key_from_codes(add_codes0, start, add_spec0);
    let mut add_key1 = safe_ngram_grapheme_key_from_codes(add_codes1, start, add_spec1);
    let mut veto_key = safe_ngram_grapheme_key_from_codes(veto_codes, start, veto_spec);
    for boundary in start..=end {
        if add_rules.contains(add_key0, add_key1) && !veto_rules.contains(veto_key) {
            out.push(boundary as GraphemeIndex);
        }
        let add_next0 = boundary as isize + 1 + add_spec0.right as isize;
        let add_next1 = boundary as isize + 1 + add_spec1.right as isize;
        let veto_next = boundary as isize + 1 + veto_spec.right as isize;
        add_key0 = (add_key0 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes0, add_next0) << add_last_shift0);
        add_key1 = (add_key1 >> 5)
            | (safe_ngram_grapheme_code_at(add_codes1, add_next1) << add_last_shift1);
        veto_key = (veto_key >> 5)
            | (safe_ngram_grapheme_code_at(veto_codes, veto_next) << veto_last_shift);
    }
}

