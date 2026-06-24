// Method dispatch, adapters, dictionary methods, and external JSONL bridge.

impl PreparedMethod {
    fn id(&self) -> &str {
        match self {
            Self::Adapter { inner, .. } => inner.id(),
            Self::Liang(inner) => inner.id(),
            Self::Dictionary { id, .. } => id,
            Self::DictionaryFallback { id, .. } => id,
            Self::SafeNgram(inner) => inner.id(),
            Self::ItalianSyllable(inner) => inner.id(),
            Self::IdentityOracle { .. } => "identity-oracle",
            Self::Crf(inner) => inner.id(),
            Self::Intersection { id, .. } => id,
            Self::ExternalJsonl(inner) => inner.id(),
        }
    }

    fn config(&self) -> &HyphenationConfig {
        match self {
            Self::Adapter { config, .. } => config,
            Self::Liang(inner) => inner.config(),
            Self::Dictionary { config, .. } => config,
            Self::DictionaryFallback { config, .. } => config,
            Self::SafeNgram(inner) => inner.config(),
            Self::ItalianSyllable(inner) => inner.config(),
            Self::IdentityOracle { config } => config,
            Self::Crf(inner) => inner.config(),
            Self::Intersection { config, .. } => config,
            Self::ExternalJsonl(inner) => inner.config(),
        }
    }

    fn hyphenate_record_into(
        &self,
        record: &HyphenationRecord,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) -> Result<()> {
        match self {
            Self::IdentityOracle { .. } => {
                out.clear();
                out.extend(record.breaks.iter().copied());
                Ok(())
            }
            _ => self.hyphenate_into(&record.word, out),
        }
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        match self {
            Self::Adapter { inner, .. } => inner.hyphenate_into(word, out),
            Self::Liang(inner) => inner.hyphenate_into(word, out),
            Self::Crf(inner) => inner.hyphenate_into(word, out),
            Self::SafeNgram(inner) => inner.hyphenate_into(word, out),
            Self::ItalianSyllable(inner) => inner.hyphenate_into(word, out),
            Self::Dictionary { entries, .. } => {
                out.clear();
                if let Some(breaks) = entries.get(word) {
                    out.extend(breaks.iter().copied());
                } else {
                    let lower = word.to_lowercase();
                    if let Some(breaks) = entries.get(&lower) {
                        out.extend(breaks.iter().copied());
                    }
                }
                Ok(())
            }
            Self::DictionaryFallback {
                entries, fallback, ..
            } => {
                out.clear();
                if let Some(breaks) = entries.get(word) {
                    out.extend(breaks.iter().copied());
                    return Ok(());
                }
                let lower = word.to_lowercase();
                if let Some(breaks) = entries.get(&lower) {
                    out.extend(breaks.iter().copied());
                    return Ok(());
                }
                fallback.hyphenate_into(word, out)
            }
            Self::IdentityOracle { .. } => {
                anyhow::bail!(
                    "identity-oracle requires an evaluation record and cannot predict plain words"
                )
            }
            Self::Intersection { first, second, .. } => {
                let mut left = SmallVec::<[GraphemeIndex; 8]>::new();
                let mut right = SmallVec::<[GraphemeIndex; 8]>::new();
                first.hyphenate_into(word, &mut left)?;
                second.hyphenate_into(word, &mut right)?;
                right.sort_unstable();
                out.clear();
                out.extend(
                    left.into_iter()
                        .filter(|idx| right.binary_search(idx).is_ok()),
                );
                out.sort_unstable();
                out.dedup();
                Ok(())
            }
            Self::ExternalJsonl(inner) => inner.hyphenate_into(word, out),
        }
    }
}

struct ExternalJsonlMethod {
    id: String,
    language: LanguageTag,
    config: HyphenationConfig,
    process: Mutex<ExternalJsonlProcess>,
}

struct ExternalJsonlProcess {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    sequence: u64,
}

impl Drop for ExternalJsonlProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl ExternalJsonlMethod {
    fn new(command: &str, locale: &str, config: HyphenationConfig) -> Result<Self> {
        let language = locale
            .parse::<LanguageTag>()
            .map_err(|err| anyhow::anyhow!("parse locale {locale:?}: {err}"))?;
        let mut child = ProcessCommand::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn external-jsonl command {command:?}"))?;
        let stdin = child
            .stdin
            .take()
            .context("external-jsonl command did not expose stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("external-jsonl command did not expose stdout")?;
        Ok(Self {
            id: format!("external-jsonl:{command}"),
            language,
            config,
            process: Mutex::new(ExternalJsonlProcess {
                child,
                stdin: BufWriter::new(stdin),
                stdout: BufReader::new(stdout),
                sequence: 0,
            }),
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        let mut process = self
            .process
            .lock()
            .map_err(|_| anyhow::anyhow!("external-jsonl process lock poisoned"))?;
        process.sequence += 1;
        let id = process.sequence.to_string();
        let input = serde_json::json!({
            "id": id,
            "word": word,
            "lang": self.language.to_string(),
        });
        serde_json::to_writer(&mut process.stdin, &input)?;
        process.stdin.write_all(b"\n")?;
        process.stdin.flush()?;

        let mut line = String::new();
        let bytes = process
            .stdout
            .read_line(&mut line)
            .with_context(|| format!("read external-jsonl response for {word:?}"))?;
        anyhow::ensure!(
            bytes > 0,
            "external-jsonl command closed stdout while processing {word:?}"
        );
        let value = serde_json::from_str::<serde_json::Value>(&line)
            .with_context(|| format!("parse external-jsonl response for {word:?}: {line:?}"))?;
        if let Some(error) = value.get("error").and_then(|value| value.as_str()) {
            anyhow::bail!("external-jsonl command returned error for {word:?}: {error}");
        }
        if let Some(response_id) = value.get("id").and_then(|value| value.as_str()) {
            anyhow::ensure!(
                response_id == id,
                "external-jsonl response id mismatch for {word:?}: expected {id}, got {response_id}"
            );
        }

        out.clear();
        if let Some(breaks) = value.get("breaks").and_then(|value| value.as_array()) {
            for item in breaks {
                let idx = item
                    .as_u64()
                    .with_context(|| format!("external-jsonl non-integer break for {word:?}"))?;
                out.push(idx.try_into().with_context(|| {
                    format!("external-jsonl break out of range for {word:?}: {idx}")
                })?);
            }
        } else if let Some(hyphenated) = value.get("hyphenated").and_then(|value| value.as_str()) {
            out.extend(hyphenated_to_breaks(word, hyphenated)?);
        } else {
            anyhow::bail!(
                "external-jsonl response for {word:?} must contain `breaks` or `hyphenated`"
            );
        }
        out.sort_unstable();
        out.dedup();
        Ok(())
    }
}

fn prepare_method(options: MethodOptions) -> Result<PreparedMethod> {
    let method = options.method.to_ascii_lowercase();
    match method.as_str() {
        "identity-oracle" | "record-oracle" => prepare_identity_oracle(options),
        "liang" | "patterns" | "tex" => prepare_liang(options),
        "dict" | "dictionary" | "lookup" => prepare_dictionary(options),
        "dict-fallback-safe-ngram-model" | "dictionary-fallback-safe-ngram-model" => {
            prepare_dictionary_fallback_safe_ngram_model(options)
        }
        "italian-syllable-model"
        | "it-syllable-model"
        | "italian-onset-model"
        | "it-onset-model" => prepare_italian_syllable_model(options),
        method
            if method.starts_with("italian-syllable")
                || method.starts_with("it-syllable")
                || method.starts_with("italian-onset")
                || method.starts_with("it-onset") =>
        {
            prepare_italian_syllable(options)
        }
        "safe-ngram-model" => prepare_safe_ngram_model(options),
        method if method.starts_with("safe-ngram") => prepare_safe_ngram(options),
        "trogkanis-elkan-crf" => prepare_crf(options),
        "hyphenation-runtime" | "hyphenation-standard-runtime" | "hyphenation-file" => {
            prepare_hyphenation_runtime(options)
        }
        "hyphenation-extended-runtime" | "hyphenation-extended" => {
            prepare_hyphenation_extended_runtime(options)
        }
        "hypher-liang-consensus" | "consensus" => prepare_hypher_liang_consensus(options),
        "external-jsonl" | "external" | "subprocess" => prepare_external_jsonl(options),
        _ => {
            let adapter = adapter_for_method(&options.method, &options.locale)?;
            let mut config = adapter.config().clone();
            apply_config_overrides(&mut config, &options);
            Ok(PreparedMethod::Adapter {
                inner: adapter,
                config,
            })
        }
    }
}

fn prepare_identity_oracle(options: MethodOptions) -> Result<PreparedMethod> {
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::IdentityOracle { config })
}

fn prepare_crf(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options.dictionary.as_ref().context(
        "--dictionary is required as the CRF model path for --method trogkanis-elkan-crf",
    )?;
    let model = CrfHyphenator::load(path)?;
    let mut config = model.config().clone();
    apply_config_overrides(&mut config, &options);
    if config != *model.config() {
        anyhow::bail!(
            "CRF config overrides are not supported at load time; train a model with the desired config"
        );
    }
    Ok(PreparedMethod::Crf(model))
}

#[cfg(feature = "adapters-hyphenation")]
fn prepare_hyphenation_runtime(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required for --method hyphenation-runtime")?;
    let adapter = HyphenationCrateAdapter::from_path(&options.locale, path)?;
    let mut config = adapter.config().clone();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::Adapter {
        inner: Box::new(adapter),
        config,
    })
}

#[cfg(feature = "adapters-hyphenation")]
fn prepare_hyphenation_extended_runtime(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required for --method hyphenation-extended-runtime")?;
    let adapter = HyphenationCrateAdapter::from_extended_path(&options.locale, path)?;
    let mut config = adapter.config().clone();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::Adapter {
        inner: Box::new(adapter),
        config,
    })
}

#[cfg(not(feature = "adapters-hyphenation"))]
fn prepare_hyphenation_runtime(_options: MethodOptions) -> Result<PreparedMethod> {
    anyhow::bail!(
        "method `hyphenation-runtime` requires feature `adapters-hyphenation` or `adapters-hyphenation-embedded`"
    )
}

#[cfg(not(feature = "adapters-hyphenation"))]
fn prepare_hyphenation_extended_runtime(_options: MethodOptions) -> Result<PreparedMethod> {
    anyhow::bail!("method `hyphenation-extended-runtime` requires feature `adapters-hyphenation`")
}

fn prepare_hypher_liang_consensus(options: MethodOptions) -> Result<PreparedMethod> {
    let adapter = adapter_for_method("hypher", &options.locale)?;
    let mut config = adapter.config().clone();
    apply_config_overrides(&mut config, &options);
    let first = PreparedMethod::Adapter {
        inner: adapter,
        config: config.clone(),
    };
    let second = prepare_liang(MethodOptions {
        method: "liang".to_string(),
        locale: options.locale.clone(),
        patterns: options.patterns.clone(),
        dictionary: None,
        dictionary_is_gold_oracle: false,
        external_command: None,
        left_min: options.left_min,
        right_min: options.right_min,
        min_word_len: options.min_word_len,
    })?;
    Ok(PreparedMethod::Intersection {
        id: format!("hypher-liang-consensus:{}", second.id()),
        config,
        first: Box::new(first),
        second: Box::new(second),
    })
}

fn prepare_external_jsonl(options: MethodOptions) -> Result<PreparedMethod> {
    let command = options
        .external_command
        .as_ref()
        .context("--external-command is required for --method external-jsonl")?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::ExternalJsonl(ExternalJsonlMethod::new(
        command,
        &options.locale,
        config,
    )?))
}

fn prepare_liang(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .patterns
        .as_ref()
        .context("--patterns is required for --method liang")?;
    let mut set = parse_pattern_file(path)?;
    let mut config = HyphenationConfig::default();
    if let Some(left_min) = set.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = set.right_min {
        config.right_min = right_min;
    }
    set.left_min = None;
    set.right_min = None;
    apply_config_overrides(&mut config, &options);

    let language = options
        .locale
        .parse::<LanguageTag>()
        .map_err(|err| anyhow::anyhow!("parse locale {:?}: {err}", options.locale))?;
    let id = format!("liang:{}", file_stem(path));
    Ok(PreparedMethod::Liang(LiangHyphenator::new(
        id, language, config, set,
    )))
}

fn prepare_safe_ngram(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the train corpus for --method safe-ngram")?;
    let records = read_records(path)?;
    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::SafeNgram(SafeNgramMethod::train(
        &options.method,
        &options.locale,
        path,
        config,
        &records,
    )?))
}

fn prepare_italian_syllable(options: MethodOptions) -> Result<PreparedMethod> {
    Ok(PreparedMethod::ItalianSyllable(ItalianSyllableMethod::new(
        &options.method,
        &options,
    )?))
}

fn prepare_italian_syllable_model(options: MethodOptions) -> Result<PreparedMethod> {
    anyhow::ensure!(
        options.left_min.is_none() && options.right_min.is_none() && options.min_word_len.is_none(),
        "italian-syllable-model uses the saved model config; CLI config overrides are not supported"
    );
    let path = options.dictionary.as_ref().context(
        "--dictionary is required as the model path for --method italian-syllable-model",
    )?;
    let model = ItalianSyllableModelFile::load(path)?;
    Ok(PreparedMethod::ItalianSyllable(
        ItalianSyllableMethod::from_model(path, &options.locale, model)?,
    ))
}

fn prepare_safe_ngram_model(options: MethodOptions) -> Result<PreparedMethod> {
    anyhow::ensure!(
        options.left_min.is_none() && options.right_min.is_none() && options.min_word_len.is_none(),
        "safe-ngram-model uses the saved model config; CLI config overrides are not supported"
    );
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the model path for --method safe-ngram-model")?;
    let model = SafeNgramModelFile::load(path)?;
    Ok(PreparedMethod::SafeNgram(SafeNgramMethod::from_model(
        path,
        &options.locale,
        model,
    )?))
}

fn prepare_dictionary(options: MethodOptions) -> Result<PreparedMethod> {
    let path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required for --method dict")?;
    let records = read_records(path)?;
    let mut entries = HashMap::new();
    for record in records {
        entries.insert(record.word, record.breaks);
    }

    let mut config = HyphenationConfig::default();
    apply_config_overrides(&mut config, &options);
    Ok(PreparedMethod::Dictionary {
        id: if options.dictionary_is_gold_oracle {
            format!("dict-oracle:{}", file_stem(path))
        } else {
            format!("dict:{}", file_stem(path))
        },
        config,
        entries,
    })
}

fn is_dictionary_method(method: &str) -> bool {
    matches!(
        method.to_ascii_lowercase().as_str(),
        "dict" | "dictionary" | "lookup"
    )
}

fn prepare_dictionary_fallback_safe_ngram_model(options: MethodOptions) -> Result<PreparedMethod> {
    let dictionary_path = options
        .dictionary
        .as_ref()
        .context("--dictionary is required as the primary dictionary for --method dict-fallback-safe-ngram-model")?;
    let fallback_path = options
        .patterns
        .as_ref()
        .context("--patterns is required as the fallback safe-ngram model path for --method dict-fallback-safe-ngram-model")?;
    let records = read_records(dictionary_path)?;
    let mut entries = HashMap::new();
    for record in records {
        entries.insert(record.word, record.breaks);
    }
    let fallback_model = SafeNgramModelFile::load(fallback_path)?;
    let fallback = SafeNgramMethod::from_model(fallback_path, &options.locale, fallback_model)
        .with_context(|| format!("load fallback safe-ngram model {}", fallback_path.display()))?;
    let mut config = fallback.config().clone();
    apply_config_overrides(&mut config, &options);
    anyhow::ensure!(
        config == *fallback.config(),
        "dict-fallback-safe-ngram-model does not support config overrides that differ from the saved fallback model"
    );
    let fallback = PreparedMethod::SafeNgram(fallback);
    Ok(PreparedMethod::DictionaryFallback {
        id: format!(
            "dict-fallback:{}->{}",
            file_stem(dictionary_path),
            fallback.id()
        ),
        config,
        entries,
        fallback: Box::new(fallback),
    })
}

fn apply_config_overrides(config: &mut HyphenationConfig, options: &MethodOptions) {
    if let Some(left_min) = options.left_min {
        config.left_min = left_min;
    }
    if let Some(right_min) = options.right_min {
        config.right_min = right_min;
    }
    if let Some(min_word_len) = options.min_word_len {
        config.min_word_len = min_word_len;
    }
}

