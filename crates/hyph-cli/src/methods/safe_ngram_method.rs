// Safe-ngram method construction and runtime dispatch.

impl SafeNgramMethod {
    fn train(
        method: &str,
        locale: &str,
        path: &Path,
        config: HyphenationConfig,
        records: &[HyphenationRecord],
    ) -> Result<Self> {
        let (options, veto_options) = parse_safe_ngram_veto_options(method)?;
        let _language = locale
            .parse::<LanguageTag>()
            .map_err(|err| anyhow::anyhow!("parse locale {locale:?}: {err}"))?;
        let (rules, trained_records) = learn_safe_ngram_rules(records, &config, &options);
        let veto_rules = if let Some(veto_options) = &veto_options {
            learn_safe_ngram_veto_rules(records, &config, &options, &rules, veto_options)
        } else {
            U64HashSet::default()
        };

        anyhow::ensure!(
            !rules.is_empty(),
            "safe-ngram learned no rules from {} with method {method:?}",
            path.display()
        );

        let id = format!(
            "{method}:{}:r{}:v{}:n{}",
            file_stem(path),
            rules.len(),
            veto_rules.len(),
            trained_records
        );
        let rules_dense = SafeNgramDenseSet::from_options(&rules, &options);
        let rules_dual_dense = SafeNgramDualDenseSet::from_options(&rules, &options);
        let veto_rules_dense = veto_options
            .as_ref()
            .and_then(|options| SafeNgramDenseSet::from_options(&veto_rules, options));
        let veto_rules_dual_dense = veto_options
            .as_ref()
            .and_then(|options| SafeNgramDualDenseSet::from_options(&veto_rules, options));
        let uses_unicode_features =
            safe_ngram_uses_unicode_features(&options, veto_options.as_ref());
        let family_mask = safe_ngram_family_mask(&options, veto_options.as_ref());
        Ok(Self {
            id,
            config,
            options,
            uses_unicode_features,
            family_mask,
            rules,
            rules_dense,
            rules_dual_dense,
            veto_options,
            veto_rules,
            veto_rules_dense,
            veto_rules_dual_dense,
        })
    }

    fn from_model(path: &Path, locale: &str, model: SafeNgramModelFile) -> Result<Self> {
        anyhow::ensure!(
            model.schema_version == 1,
            "unsupported safe-ngram model schema version {} in {}",
            model.schema_version,
            path.display()
        );
        anyhow::ensure!(
            normalize_locale_match_key(locale) == normalize_locale_match_key(&model.locale),
            "safe-ngram model locale {} does not match requested locale {}",
            model.locale,
            locale
        );
        anyhow::ensure!(
            !model.rules.is_empty(),
            "safe-ngram model {} has no rules",
            path.display()
        );
        let options = model.options;
        let veto_options = model.veto_options;
        let rules = model.rules.into_iter().collect::<U64HashSet>();
        let veto_rules = model.veto_rules.into_iter().collect::<U64HashSet>();
        let rules_dense = SafeNgramDenseSet::from_options(&rules, &options);
        let rules_dual_dense = SafeNgramDualDenseSet::from_options(&rules, &options);
        let veto_rules_dense = veto_options
            .as_ref()
            .and_then(|options| SafeNgramDenseSet::from_options(&veto_rules, options));
        let veto_rules_dual_dense = veto_options
            .as_ref()
            .and_then(|options| SafeNgramDualDenseSet::from_options(&veto_rules, options));
        let uses_unicode_features =
            safe_ngram_uses_unicode_features(&options, veto_options.as_ref());
        let family_mask = safe_ngram_family_mask(&options, veto_options.as_ref());
        Ok(Self {
            id: format!("{}:model:{}", model.id, file_stem(path)),
            config: model.config,
            options,
            uses_unicode_features,
            family_mask,
            rules,
            rules_dense,
            rules_dual_dense,
            veto_options,
            veto_rules,
            veto_rules_dense,
            veto_rules_dual_dense,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn rules_lookup(&self) -> SafeNgramRuleLookup<'_> {
        if let Some(rules) = &self.rules_dense {
            SafeNgramRuleLookup::Dense(rules)
        } else {
            SafeNgramRuleLookup::Hash(&self.rules)
        }
    }

    fn rules_dual_lookup(&self) -> SafeNgramDualRuleLookup<'_> {
        if let Some(rules) = &self.rules_dual_dense {
            SafeNgramDualRuleLookup::Dense(rules)
        } else {
            SafeNgramDualRuleLookup::Hash(&self.rules)
        }
    }

    fn veto_rules_lookup(&self) -> SafeNgramRuleLookup<'_> {
        if let Some(rules) = &self.veto_rules_dense {
            SafeNgramRuleLookup::Dense(rules)
        } else {
            SafeNgramRuleLookup::Hash(&self.veto_rules)
        }
    }

    fn veto_rules_dual_lookup(&self) -> SafeNgramDualRuleLookup<'_> {
        if let Some(rules) = &self.veto_rules_dual_dense {
            SafeNgramDualRuleLookup::Dense(rules)
        } else {
            SafeNgramDualRuleLookup::Hash(&self.veto_rules)
        }
    }

    fn uses_unicode_features(&self) -> bool {
        self.uses_unicode_features
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        if self.uses_unicode_features() && !word.is_ascii() {
            self.hyphenate_unicode_into(word, out);
            return Ok(());
        }
        if !word.is_ascii() || word.len() < self.config.min_word_len {
            return Ok(());
        }
        let bytes = word.as_bytes();
        if self.options.specs.len() == 1 && !self.options.cap_vowel_nuclei {
            let spec = self.options.specs[0];
            if let Some(veto_options) = &self.veto_options {
                if veto_options.specs.len() == 1
                    && !spec.bucketed
                    && !veto_options.specs[0].bucketed
                {
                    let veto_spec = veto_options.specs[0];
                    match (spec.family, veto_spec.family) {
                        (1, 1) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_cv_code_at,
                            safe_ngram_cv_code_at,
                        ),
                        (1, 2) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_cv_code_at,
                            safe_ngram_sonority_code_at,
                        ),
                        (1, _) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_cv_code_at,
                            safe_ngram_raw_code_at,
                        ),
                        (2, 1) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_sonority_code_at,
                            safe_ngram_cv_code_at,
                        ),
                        (2, 2) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_sonority_code_at,
                            safe_ngram_sonority_code_at,
                        ),
                        (2, _) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_sonority_code_at,
                            safe_ngram_raw_code_at,
                        ),
                        (_, 1) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_raw_code_at,
                            safe_ngram_cv_code_at,
                        ),
                        (_, 2) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_raw_code_at,
                            safe_ngram_sonority_code_at,
                        ),
                        (_, _) => safe_ngram_hyphenate_single_add_veto_lookup(
                            bytes,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                            safe_ngram_raw_code_at,
                            safe_ngram_raw_code_at,
                        ),
                    }
                    if self.options.orthographic_veto {
                        safe_ngram_apply_orthographic_veto(bytes, out);
                    }
                    return Ok(());
                }
            } else if !spec.bucketed {
                let use_rule_lookup = self.rules_dense.is_some();
                match spec.family {
                    1 if use_rule_lookup => safe_ngram_hyphenate_single_spec_lookup(
                        bytes,
                        &self.config,
                        spec,
                        self.rules_lookup(),
                        out,
                        safe_ngram_cv_code_at,
                    ),
                    1 => safe_ngram_hyphenate_single_spec(
                        bytes,
                        &self.config,
                        spec,
                        &self.rules,
                        out,
                        safe_ngram_cv_code_at,
                    ),
                    2 if use_rule_lookup => safe_ngram_hyphenate_single_spec_lookup(
                        bytes,
                        &self.config,
                        spec,
                        self.rules_lookup(),
                        out,
                        safe_ngram_sonority_code_at,
                    ),
                    2 => safe_ngram_hyphenate_single_spec(
                        bytes,
                        &self.config,
                        spec,
                        &self.rules,
                        out,
                        safe_ngram_sonority_code_at,
                    ),
                    _ if use_rule_lookup => safe_ngram_hyphenate_single_spec_lookup(
                        bytes,
                        &self.config,
                        spec,
                        self.rules_lookup(),
                        out,
                        safe_ngram_raw_code_at,
                    ),
                    _ => safe_ngram_hyphenate_single_spec(
                        bytes,
                        &self.config,
                        spec,
                        &self.rules,
                        out,
                        safe_ngram_raw_code_at,
                    ),
                }
                if self.options.orthographic_veto {
                    safe_ngram_apply_orthographic_veto(bytes, out);
                }
            } else {
                let rules = self.rules_lookup();
                for boundary in
                    self.config.left_min..=bytes.len().saturating_sub(self.config.right_min)
                {
                    if rules.contains(safe_ngram_key(bytes, boundary, 0, spec)) {
                        out.push(boundary as GraphemeIndex);
                    }
                }
                if self.options.orthographic_veto {
                    safe_ngram_apply_orthographic_veto(bytes, out);
                }
            }
            return Ok(());
        }
        let rules = self.rules_lookup();
        let veto_rules = self.veto_rules_lookup();
        if !self.options.cap_vowel_nuclei
            && self.options.specs.len() == 2
            && self.options.specs.iter().all(|spec| !spec.bucketed)
        {
            let add_spec0 = self.options.specs[0];
            let add_spec1 = self.options.specs[1];
            if let Some(veto_options) = &self.veto_options {
                if veto_options.specs.len() == 2
                    && veto_options.specs.iter().all(|spec| !spec.bucketed)
                {
                    let veto_spec0 = veto_options.specs[0];
                    let veto_spec1 = veto_options.specs[1];
                    safe_ngram_hyphenate_dual_add_veto_lookup(
                        bytes,
                        &self.config,
                        add_spec0,
                        add_spec1,
                        self.rules_dual_lookup(),
                        veto_spec0,
                        veto_spec1,
                        self.veto_rules_dual_lookup(),
                        out,
                    );
                    if self.options.orthographic_veto {
                        safe_ngram_apply_orthographic_veto(bytes, out);
                    }
                    return Ok(());
                }
                if veto_options.specs.len() == 1 && !veto_options.specs[0].bucketed {
                    let veto_spec = veto_options.specs[0];
                    safe_ngram_hyphenate_dual_add_single_veto_lookup(
                        bytes,
                        &self.config,
                        add_spec0,
                        add_spec1,
                        self.rules_dual_lookup(),
                        veto_spec,
                        self.veto_rules_lookup(),
                        out,
                    );
                    if self.options.orthographic_veto {
                        safe_ngram_apply_orthographic_veto(bytes, out);
                    }
                    return Ok(());
                }
            } else {
                safe_ngram_hyphenate_dual_spec_lookup(
                    bytes,
                    &self.config,
                    add_spec0,
                    add_spec1,
                    self.rules_dual_lookup(),
                    out,
                );
                if self.options.orthographic_veto {
                    safe_ngram_apply_orthographic_veto(bytes, out);
                }
                return Ok(());
            }
        }
        if !self.options.cap_vowel_nuclei {
            for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min)
            {
                let add_hit = self
                    .options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        rules.contains(safe_ngram_key(bytes, boundary, spec_idx, *spec))
                    });
                if !add_hit {
                    continue;
                }
                let veto_hit = self.veto_options.as_ref().is_some_and(|veto_options| {
                    veto_options
                        .specs
                        .iter()
                        .enumerate()
                        .any(|(spec_idx, spec)| {
                            veto_rules.contains(safe_ngram_key(bytes, boundary, spec_idx, *spec))
                        })
                });
                if !veto_hit {
                    out.push(boundary as GraphemeIndex);
                }
            }
            if self.options.orthographic_veto {
                safe_ngram_apply_orthographic_veto(bytes, out);
            }
            return Ok(());
        }
        let mut scored = SmallVec::<[(u32, GraphemeIndex); 8]>::new();
        for boundary in self.config.left_min..=bytes.len().saturating_sub(self.config.right_min) {
            let add_score = self
                .options
                .specs
                .iter()
                .enumerate()
                .filter(|(spec_idx, spec)| {
                    rules.contains(safe_ngram_key(bytes, boundary, *spec_idx, **spec))
                })
                .count() as u32;
            if add_score == 0 {
                continue;
            }
            let veto_hit = self.veto_options.as_ref().is_some_and(|veto_options| {
                veto_options
                    .specs
                    .iter()
                    .enumerate()
                    .any(|(spec_idx, spec)| {
                        veto_rules.contains(safe_ngram_key(bytes, boundary, spec_idx, *spec))
                    })
            });
            if !veto_hit {
                let boundary = boundary as GraphemeIndex;
                out.push(boundary);
                if self.options.cap_vowel_nuclei {
                    scored.push((add_score, boundary));
                }
            }
        }
        if self.options.orthographic_veto {
            safe_ngram_apply_orthographic_veto(bytes, out);
            if self.options.cap_vowel_nuclei {
                scored.retain(|(_, boundary)| out.contains(boundary));
            }
        }
        if self.options.cap_vowel_nuclei {
            let cap = safe_ngram_vowel_break_cap(bytes);
            if out.len() > cap {
                scored.sort_by(|left, right| {
                    right
                        .0
                        .cmp(&left.0)
                        .then_with(|| {
                            left.1
                                .abs_diff(bytes.len() as GraphemeIndex / 2)
                                .cmp(&right.1.abs_diff(bytes.len() as GraphemeIndex / 2))
                        })
                        .then_with(|| left.1.cmp(&right.1))
                });
                scored.truncate(cap);
                out.clear();
                out.extend(scored.into_iter().map(|(_, boundary)| boundary));
                out.sort_unstable();
            }
        }
        Ok(())
    }

    fn hyphenate_unicode_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) {
        let char_tables = safe_ngram_char_tables_if_simple(word, self.family_mask);
        let grapheme_tables;
        let tables = if let Some(tables) = char_tables.as_ref() {
            tables
        } else {
            grapheme_tables = safe_ngram_grapheme_tables(word, self.family_mask);
            &grapheme_tables
        };
        let grapheme_len = tables.len;
        if grapheme_len < self.config.min_word_len {
            return;
        }
        let start = self.config.left_min;
        let end = grapheme_len.saturating_sub(self.config.right_min);
        if start > end {
            return;
        }

        if self.options.specs.len() == 1 {
            let spec = self.options.specs[0];
            if !spec.bucketed {
                if let Some(veto_options) = &self.veto_options {
                    if veto_options.specs.len() == 1 && !veto_options.specs[0].bucketed {
                        let veto_spec = veto_options.specs[0];
                        safe_ngram_hyphenate_grapheme_single_add_veto_lookup(
                            tables.codes(spec.family),
                            tables.codes(veto_spec.family),
                            grapheme_len,
                            &self.config,
                            spec,
                            self.rules_lookup(),
                            veto_spec,
                            self.veto_rules_lookup(),
                            out,
                        );
                        return;
                    }
                } else if self.rules_dense.is_some() {
                    safe_ngram_hyphenate_grapheme_single_spec_lookup(
                        tables.codes(spec.family),
                        grapheme_len,
                        &self.config,
                        spec,
                        self.rules_lookup(),
                        out,
                    );
                    return;
                } else {
                    safe_ngram_hyphenate_grapheme_single_spec(
                        tables.codes(spec.family),
                        grapheme_len,
                        &self.config,
                        spec,
                        &self.rules,
                        out,
                    );
                    return;
                }
            }
        }

        let rules = self.rules_lookup();
        let veto_rules = self.veto_rules_lookup();
        if !self.options.cap_vowel_nuclei
            && self.options.specs.len() == 2
            && self.options.specs.iter().all(|spec| !spec.bucketed)
        {
            let add_spec0 = self.options.specs[0];
            let add_spec1 = self.options.specs[1];
            if let Some(veto_options) = &self.veto_options {
                if veto_options.specs.len() == 2
                    && veto_options.specs.iter().all(|spec| !spec.bucketed)
                {
                    let veto_spec0 = veto_options.specs[0];
                    let veto_spec1 = veto_options.specs[1];
                    safe_ngram_hyphenate_grapheme_dual_add_veto_lookup(
                        tables.codes(add_spec0.family),
                        tables.codes(add_spec1.family),
                        tables.codes(veto_spec0.family),
                        tables.codes(veto_spec1.family),
                        grapheme_len,
                        &self.config,
                        add_spec0,
                        add_spec1,
                        self.rules_dual_lookup(),
                        veto_spec0,
                        veto_spec1,
                        self.veto_rules_dual_lookup(),
                        out,
                    );
                    return;
                }
                if veto_options.specs.len() == 1 && !veto_options.specs[0].bucketed {
                    let veto_spec = veto_options.specs[0];
                    safe_ngram_hyphenate_grapheme_dual_add_single_veto_lookup(
                        tables.codes(add_spec0.family),
                        tables.codes(add_spec1.family),
                        tables.codes(veto_spec.family),
                        grapheme_len,
                        &self.config,
                        add_spec0,
                        add_spec1,
                        self.rules_dual_lookup(),
                        veto_spec,
                        self.veto_rules_lookup(),
                        out,
                    );
                    return;
                }
            } else {
                safe_ngram_hyphenate_grapheme_dual_spec_lookup(
                    tables.codes(add_spec0.family),
                    tables.codes(add_spec1.family),
                    grapheme_len,
                    &self.config,
                    add_spec0,
                    add_spec1,
                    self.rules_dual_lookup(),
                    out,
                );
                return;
            }
        }
        for boundary in start..=end {
            let add_hit = self
                .options
                .specs
                .iter()
                .enumerate()
                .any(|(spec_idx, spec)| {
                    let key =
                        safe_ngram_grapheme_key(&tables, grapheme_len, boundary, spec_idx, *spec);
                    rules.contains(key)
                });
            if !add_hit {
                continue;
            }
            let veto_hit = self.veto_options.as_ref().is_some_and(|veto_options| {
                veto_options
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
                        veto_rules.contains(key)
                    })
            });
            if !veto_hit {
                out.push(boundary as GraphemeIndex);
            }
        }
    }
}

