// Safe-ngram and Italian model load/save code.

impl SafeNgramModelFile {
    fn from_parts(
        method: String,
        locale: String,
        source: String,
        config: HyphenationConfig,
        options: SafeNgramOptions,
        rules: U64HashSet,
        veto_options: Option<SafeNgramOptions>,
        veto_rules: U64HashSet,
        trained_records: usize,
    ) -> Self {
        let mut rules = rules.into_iter().collect::<Vec<_>>();
        rules.sort_unstable();
        let mut veto_rules = veto_rules.into_iter().collect::<Vec<_>>();
        veto_rules.sort_unstable();
        Self {
            schema_version: 1,
            id: format!(
                "{method}:{source}:r{}:v{}:n{}",
                rules.len(),
                veto_rules.len(),
                trained_records
            ),
            method,
            locale,
            source,
            config,
            options,
            rules,
            veto_options,
            veto_rules,
            trained_records,
        }
    }

    fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        if path_extension_eq(path, "bin") {
            Self::load_binary(file, path)
        } else if path_extension_eq(path, "zst") {
            let decoder = zstd::stream::read::Decoder::new(file)
                .with_context(|| format!("open zstd decoder for {}", path.display()))?;
            serde_json::from_reader(BufReader::new(decoder))
                .with_context(|| format!("parse {}", path.display()))
        } else {
            serde_json::from_reader(BufReader::new(file))
                .with_context(|| format!("parse {}", path.display()))
        }
    }

    fn save(&self, path: &Path) -> Result<()> {
        create_parent(path)?;
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        if path_extension_eq(path, "bin") {
            self.save_binary(file, path)?;
        } else if path_extension_eq(path, "zst") {
            let writer = BufWriter::new(file);
            let mut encoder = zstd::stream::write::Encoder::new(writer, 3)
                .with_context(|| format!("open zstd encoder for {}", path.display()))?;
            serde_json::to_writer(&mut encoder, self)
                .with_context(|| format!("write {}", path.display()))?;
            encoder.finish()?;
        } else {
            serde_json::to_writer_pretty(BufWriter::new(file), self)
                .with_context(|| format!("write {}", path.display()))?;
        }
        Ok(())
    }

    fn metadata(&self) -> SafeNgramModelMeta {
        SafeNgramModelMeta {
            schema_version: self.schema_version,
            id: self.id.clone(),
            method: self.method.clone(),
            locale: self.locale.clone(),
            source: self.source.clone(),
            config: self.config.clone(),
            options: self.options.clone(),
            trained_records: self.trained_records,
            rule_count: self.rules.len(),
            veto_options: self.veto_options.clone(),
            veto_rule_count: self.veto_rules.len(),
        }
    }

    fn load_binary(file: File, path: &Path) -> Result<Self> {
        let mut reader = BufReader::new(file);
        let mut magic = [0u8; 8];
        reader
            .read_exact(&mut magic)
            .with_context(|| format!("read magic from {}", path.display()))?;
        anyhow::ensure!(
            &magic == b"HYSG1\0\0\0",
            "invalid safe-ngram binary model magic in {}",
            path.display()
        );

        let mut len_bytes = [0u8; 4];
        reader
            .read_exact(&mut len_bytes)
            .with_context(|| format!("read metadata length from {}", path.display()))?;
        let metadata_len = u32::from_le_bytes(len_bytes) as usize;
        let mut metadata_bytes = vec![0u8; metadata_len];
        reader
            .read_exact(&mut metadata_bytes)
            .with_context(|| format!("read metadata from {}", path.display()))?;
        let metadata: SafeNgramModelMeta =
            serde_json::from_slice(&metadata_bytes).context("parse safe-ngram binary metadata")?;

        let mut rule_bytes = vec![
            0u8;
            (metadata.rule_count + metadata.veto_rule_count)
                * std::mem::size_of::<u64>()
        ];
        reader
            .read_exact(&mut rule_bytes)
            .with_context(|| format!("read rules from {}", path.display()))?;
        let all_rules = rule_bytes
            .chunks_exact(8)
            .map(|chunk| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(chunk);
                u64::from_le_bytes(bytes)
            })
            .collect::<Vec<_>>();
        let (rules, veto_rules) = all_rules.split_at(metadata.rule_count);

        Ok(Self {
            schema_version: metadata.schema_version,
            id: metadata.id,
            method: metadata.method,
            locale: metadata.locale,
            source: metadata.source,
            config: metadata.config,
            options: metadata.options,
            rules: rules.to_vec(),
            veto_options: metadata.veto_options,
            veto_rules: veto_rules.to_vec(),
            trained_records: metadata.trained_records,
        })
    }

    fn save_binary(&self, file: File, path: &Path) -> Result<()> {
        let metadata = serde_json::to_vec(&self.metadata())?;
        anyhow::ensure!(
            metadata.len() <= u32::MAX as usize,
            "safe-ngram metadata is too large for binary model"
        );
        let mut writer = BufWriter::new(file);
        writer
            .write_all(b"HYSG1\0\0\0")
            .with_context(|| format!("write magic to {}", path.display()))?;
        writer
            .write_all(&(metadata.len() as u32).to_le_bytes())
            .with_context(|| format!("write metadata length to {}", path.display()))?;
        writer
            .write_all(&metadata)
            .with_context(|| format!("write metadata to {}", path.display()))?;
        for rule in &self.rules {
            writer
                .write_all(&rule.to_le_bytes())
                .with_context(|| format!("write rules to {}", path.display()))?;
        }
        for rule in &self.veto_rules {
            writer
                .write_all(&rule.to_le_bytes())
                .with_context(|| format!("write veto rules to {}", path.display()))?;
        }
        writer.flush()?;
        Ok(())
    }
}

impl ItalianSyllableModelFile {
    fn from_parts(
        method: String,
        locale: String,
        source: String,
        config: HyphenationConfig,
        learned_splits: U64HashMap<u8>,
        trained_records: usize,
    ) -> Self {
        let mut learned_splits = learned_splits.into_iter().collect::<Vec<_>>();
        learned_splits.sort_unstable_by_key(|(key, _)| *key);
        let learned_splits = learned_splits
            .into_iter()
            .map(|(key, split)| ItalianSyllableSplit {
                key: format!("0x{key:016x}"),
                split,
            })
            .collect::<Vec<_>>();
        Self {
            schema_version: 1,
            id: format!(
                "{method}:{source}:clusters{}:n{}",
                learned_splits.len(),
                trained_records
            ),
            method,
            locale,
            source,
            config,
            learned_splits,
            trained_records,
        }
    }

    fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        if path_extension_eq(path, "zst") {
            let decoder = zstd::stream::read::Decoder::new(file)
                .with_context(|| format!("open zstd decoder for {}", path.display()))?;
            serde_json::from_reader(BufReader::new(decoder))
                .with_context(|| format!("parse {}", path.display()))
        } else {
            serde_json::from_reader(BufReader::new(file))
                .with_context(|| format!("parse {}", path.display()))
        }
    }

    fn save(&self, path: &Path) -> Result<()> {
        create_parent(path)?;
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        if path_extension_eq(path, "zst") {
            let writer = BufWriter::new(file);
            let mut encoder = zstd::stream::write::Encoder::new(writer, 3)
                .with_context(|| format!("open zstd encoder for {}", path.display()))?;
            serde_json::to_writer(&mut encoder, self)
                .with_context(|| format!("write {}", path.display()))?;
            encoder.finish()?;
        } else {
            serde_json::to_writer_pretty(BufWriter::new(file), self)
                .with_context(|| format!("write {}", path.display()))?;
        }
        Ok(())
    }

    fn into_learned_splits(self, path: &Path) -> Result<U64HashMap<u8>> {
        let mut learned = U64HashMap::<u8>::default();
        for entry in self.learned_splits {
            let key = parse_u64_key(&entry.key)
                .with_context(|| format!("parse split key in {}", path.display()))?;
            anyhow::ensure!(
                entry.split <= 4,
                "invalid Italian syllable split {} in {}",
                entry.split,
                path.display()
            );
            learned.insert(key, entry.split);
        }
        Ok(learned)
    }
}

fn path_extension_eq(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

