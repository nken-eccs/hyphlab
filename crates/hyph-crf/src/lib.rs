use anyhow::{Context, Result};
use hyph_core::{GraphemeIndex, HyphenationConfig, HyphenationRecord, LanguageTag};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};
use unicode_segmentation::UnicodeSegmentation;

const LABELS: usize = 2;
const EDGES: usize = LABELS * LABELS;
const NEG_INF: f32 = -1.0e30;
const GRAPHEME_SEP: &str = "\u{1f}";
const BINARY_MAGIC: &[u8; 8] = b"HYCRF\x00\x01\x00";

#[derive(Debug, Clone)]
pub struct CrfTrainOptions {
    pub id: String,
    pub language: LanguageTag,
    pub config: HyphenationConfig,
    pub threshold: f32,
    pub min_n: usize,
    pub max_n: usize,
    pub epochs: usize,
    pub learning_rate: f32,
    pub l2: f32,
}

impl Default for CrfTrainOptions {
    fn default() -> Self {
        Self {
            id: "trogkanis-elkan-crf".to_string(),
            language: LanguageTag::default(),
            config: HyphenationConfig::default(),
            threshold: 0.9,
            min_n: 2,
            max_n: 5,
            epochs: 5,
            learning_rate: 0.05,
            l2: 1.0e-5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrfHyphenator {
    id: String,
    language: LanguageTag,
    config: HyphenationConfig,
    threshold: f32,
    min_n: usize,
    max_n: usize,
    features: Vec<CrfFeature>,
    weights: Vec<[f32; EDGES]>,
    transitions: [f32; EDGES],
    #[serde(skip)]
    index: HashMap<FeatureKey, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct CrfFeature {
    offset: u8,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FeatureKey {
    offset: u8,
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CrfBinaryMetadata {
    id: String,
    language: LanguageTag,
    config: HyphenationConfig,
    threshold: f32,
    min_n: usize,
    max_n: usize,
}

impl From<&CrfFeature> for FeatureKey {
    fn from(value: &CrfFeature) -> Self {
        Self {
            offset: value.offset,
            text: value.text.clone(),
        }
    }
}

impl CrfHyphenator {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn language(&self) -> &LanguageTag {
        &self.language
    }

    pub fn config(&self) -> &HyphenationConfig {
        &self.config
    }

    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    pub fn set_id(&mut self, id: impl Into<String>) {
        self.id = id.into();
    }

    pub fn set_threshold(&mut self, threshold: f32) -> Result<()> {
        anyhow::ensure!(
            (0.0..=1.0).contains(&threshold),
            "threshold must be in [0, 1]"
        );
        self.threshold = threshold;
        Ok(())
    }

    pub fn feature_count(&self) -> usize {
        self.features.len()
    }

    pub fn load(path: &Path) -> Result<Self> {
        if is_binary_model_path(path) {
            Self::load_binary(path)
        } else {
            Self::load_json(path)
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if is_binary_model_path(path) {
            self.save_binary(path)
        } else {
            self.save_json(path)
        }
    }

    pub fn load_json(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let mut model: Self = if is_zst_path(path) {
            let decoder = zstd::stream::read::Decoder::new(file)
                .with_context(|| format!("open zstd decoder for {}", path.display()))?;
            serde_json::from_reader(BufReader::new(decoder))
                .with_context(|| format!("parse {}", path.display()))?
        } else {
            serde_json::from_reader(BufReader::new(file))
                .with_context(|| format!("parse {}", path.display()))?
        };
        model.rebuild_index();
        Ok(model)
    }

    pub fn save_json(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create parent directory {}", parent.display()))?;
        }
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        if is_zst_path(path) {
            let writer = BufWriter::new(file);
            let mut encoder = zstd::stream::write::Encoder::new(writer, 3)
                .with_context(|| format!("open zstd encoder for {}", path.display()))?;
            serde_json::to_writer(&mut encoder, self)
                .with_context(|| format!("write {}", path.display()))?;
            encoder.finish()?;
            Ok(())
        } else {
            serde_json::to_writer_pretty(BufWriter::new(file), self)
                .with_context(|| format!("write {}", path.display()))
        }
    }

    pub fn load_binary(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let mut reader: Box<dyn Read> = if is_zst_path(path) {
            let decoder = zstd::stream::read::Decoder::new(file)
                .with_context(|| format!("open zstd decoder for {}", path.display()))?;
            Box::new(BufReader::new(decoder))
        } else {
            Box::new(BufReader::new(file))
        };

        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        anyhow::ensure!(
            &magic == BINARY_MAGIC,
            "{} is not a hyphlab CRF binary model",
            path.display()
        );

        let metadata = read_metadata(&mut reader)?;
        let feature_count = read_u32(&mut reader)? as usize;
        let mut features = Vec::with_capacity(feature_count);
        let mut weights = Vec::with_capacity(feature_count);
        for _ in 0..feature_count {
            let offset = read_u8(&mut reader)?;
            let text = read_string(&mut reader)?;
            let mut feature_weights = [0.0f32; EDGES];
            for value in &mut feature_weights {
                *value = read_f32(&mut reader)?;
            }
            features.push(CrfFeature { offset, text });
            weights.push(feature_weights);
        }
        let mut transitions = [0.0f32; EDGES];
        for value in &mut transitions {
            *value = read_f32(&mut reader)?;
        }

        let mut model = Self {
            id: metadata.id,
            language: metadata.language,
            config: metadata.config,
            threshold: metadata.threshold,
            min_n: metadata.min_n,
            max_n: metadata.max_n,
            features,
            weights,
            transitions,
            index: HashMap::new(),
        };
        model.rebuild_index();
        Ok(model)
    }

    pub fn save_binary(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create parent directory {}", parent.display()))?;
        }
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        if is_zst_path(path) {
            let writer = BufWriter::new(file);
            let mut encoder = zstd::stream::write::Encoder::new(writer, 3)
                .with_context(|| format!("open zstd encoder for {}", path.display()))?;
            self.write_binary_to(&mut encoder)?;
            encoder.finish()?;
        } else {
            let mut writer = BufWriter::new(file);
            self.write_binary_to(&mut writer)?;
            writer.flush()?;
        }
        Ok(())
    }

    fn write_binary_to(&self, writer: &mut dyn Write) -> Result<()> {
        writer.write_all(BINARY_MAGIC)?;
        let metadata = CrfBinaryMetadata {
            id: self.id.clone(),
            language: self.language.clone(),
            config: self.config.clone(),
            threshold: self.threshold,
            min_n: self.min_n,
            max_n: self.max_n,
        };
        write_metadata(writer, &metadata)?;
        write_u32(writer, self.features.len())?;
        for (feature, weights) in self.features.iter().zip(&self.weights) {
            write_u8(writer, feature.offset)?;
            write_string(writer, &feature.text)?;
            for value in weights {
                write_f32(writer, *value)?;
            }
        }
        for value in &self.transitions {
            write_f32(writer, *value)?;
        }
        Ok(())
    }

    pub fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        let tokens = tokenize(word);
        if tokens.len() < self.config.min_word_len {
            return Ok(());
        }
        let active = self.active_features_for_tokens(&tokens);
        let edge_scores = self.edge_scores(&active, tokens.len());
        let marginals = edge_marginals(&edge_scores);

        for (position, edge) in marginals.iter().enumerate() {
            if !hyphen_allowed(position, 1, tokens.len(), &self.config) {
                continue;
            }
            let probability = edge[edge_index(0, 1)] + edge[edge_index(1, 1)];
            if probability >= self.threshold {
                out.push((position + 1).try_into()?);
            }
        }
        Ok(())
    }

    fn rebuild_index(&mut self) {
        self.index = self
            .features
            .iter()
            .enumerate()
            .map(|(idx, feature)| (FeatureKey::from(feature), idx))
            .collect();
    }

    fn active_features_for_tokens(&self, tokens: &[String]) -> Vec<Vec<usize>> {
        active_feature_keys(tokens, self.min_n, self.max_n)
            .into_iter()
            .map(|keys| {
                keys.into_iter()
                    .filter_map(|key| self.index.get(&key).copied())
                    .collect()
            })
            .collect()
    }

    fn edge_scores(&self, active: &[Vec<usize>], len: usize) -> Vec<[f32; EDGES]> {
        let mut scores = vec![[NEG_INF; EDGES]; active.len()];
        for (position, features) in active.iter().enumerate() {
            for previous in 0..LABELS {
                for current in 0..LABELS {
                    if !hyphen_allowed(position, current, len, &self.config) {
                        continue;
                    }
                    let edge = edge_index(previous, current);
                    let mut score = self.transitions[edge];
                    for feature_id in features {
                        score += self.weights[*feature_id][edge];
                    }
                    scores[position][edge] = score;
                }
            }
        }
        scores
    }
}

pub fn train_crf(records: &[HyphenationRecord], options: CrfTrainOptions) -> Result<CrfHyphenator> {
    anyhow::ensure!(options.epochs > 0, "epochs must be greater than zero");
    anyhow::ensure!(
        options.min_n > 0 && options.min_n <= options.max_n,
        "invalid feature n-gram range {}..={}",
        options.min_n,
        options.max_n
    );
    anyhow::ensure!(
        (0.0..=1.0).contains(&options.threshold),
        "threshold must be in [0, 1]"
    );

    let mut feature_index = HashMap::<FeatureKey, usize>::new();
    let mut features = Vec::<CrfFeature>::new();
    let mut examples = Vec::new();
    for record in records {
        let tokens = tokenize(&record.word);
        if tokens.is_empty() {
            continue;
        }
        let active_keys = active_feature_keys(&tokens, options.min_n, options.max_n);
        let mut active = Vec::with_capacity(active_keys.len());
        for keys in active_keys {
            let mut ids = Vec::with_capacity(keys.len());
            for key in keys {
                let id = if let Some(id) = feature_index.get(&key) {
                    *id
                } else {
                    let id = features.len();
                    feature_index.insert(key.clone(), id);
                    features.push(CrfFeature {
                        offset: key.offset,
                        text: key.text,
                    });
                    id
                };
                ids.push(id);
            }
            active.push(ids);
        }
        examples.push(CrfExample {
            labels: labels_for_record(record, tokens.len()),
            active,
            len: tokens.len(),
        });
    }
    anyhow::ensure!(!examples.is_empty(), "no trainable records");

    let mut model = CrfHyphenator {
        id: options.id,
        language: options.language,
        config: options.config,
        threshold: options.threshold,
        min_n: options.min_n,
        max_n: options.max_n,
        features,
        weights: vec![[0.0; EDGES]; feature_index.len()],
        transitions: [0.0; EDGES],
        index: feature_index,
    };

    for epoch in 0..options.epochs {
        let rate = options.learning_rate / (1.0 + epoch as f32 * 0.25);
        for example in &examples {
            train_one(&mut model, example, rate, options.l2);
        }
    }

    Ok(model)
}

#[derive(Debug)]
struct CrfExample {
    labels: Vec<usize>,
    active: Vec<Vec<usize>>,
    len: usize,
}

fn train_one(model: &mut CrfHyphenator, example: &CrfExample, rate: f32, l2: f32) {
    let edge_scores = model.edge_scores(&example.active, example.len);
    let marginals = edge_marginals(&edge_scores);
    let mut touched = HashSet::new();

    for (position, features) in example.active.iter().enumerate() {
        for previous in 0..LABELS {
            for current in 0..LABELS {
                let edge = edge_index(previous, current);
                let expected = marginals[position][edge];
                if expected == 0.0 {
                    continue;
                }
                model.transitions[edge] -= rate * expected;
                for feature_id in features {
                    model.weights[*feature_id][edge] -= rate * expected;
                    touched.insert(*feature_id);
                }
            }
        }

        let previous = if position == 0 {
            0
        } else {
            example.labels[position - 1]
        };
        let current = example.labels[position];
        let gold_edge = edge_index(previous, current);
        model.transitions[gold_edge] += rate;
        for feature_id in features {
            model.weights[*feature_id][gold_edge] += rate;
            touched.insert(*feature_id);
        }
    }

    if l2 > 0.0 {
        let shrink = 1.0 - rate * l2;
        for value in &mut model.transitions {
            *value *= shrink;
        }
        for feature_id in touched {
            for value in &mut model.weights[feature_id] {
                *value *= shrink;
            }
        }
    }
}

fn edge_marginals(edge_scores: &[[f32; EDGES]]) -> Vec<[f32; EDGES]> {
    let len = edge_scores.len();
    if len == 0 {
        return Vec::new();
    }

    let mut alpha = vec![[NEG_INF; LABELS]; len];
    for current in 0..LABELS {
        alpha[0][current] = edge_scores[0][edge_index(0, current)];
    }
    for position in 1..len {
        for current in 0..LABELS {
            let mut scores = [NEG_INF; LABELS];
            for previous in 0..LABELS {
                scores[previous] = alpha[position - 1][previous]
                    + edge_scores[position][edge_index(previous, current)];
            }
            alpha[position][current] = logsumexp2(scores[0], scores[1]);
        }
    }
    let log_z = logsumexp2(alpha[len - 1][0], alpha[len - 1][1]);

    let mut beta = vec![[0.0; LABELS]; len];
    for position in (0..len - 1).rev() {
        for previous in 0..LABELS {
            let mut scores = [NEG_INF; LABELS];
            for current in 0..LABELS {
                scores[current] = edge_scores[position + 1][edge_index(previous, current)]
                    + beta[position + 1][current];
            }
            beta[position][previous] = logsumexp2(scores[0], scores[1]);
        }
    }

    let mut marginals = vec![[0.0; EDGES]; len];
    for position in 0..len {
        for previous in 0..LABELS {
            if position == 0 && previous != 0 {
                continue;
            }
            for current in 0..LABELS {
                let previous_score = if position == 0 {
                    0.0
                } else {
                    alpha[position - 1][previous]
                };
                let score = previous_score
                    + edge_scores[position][edge_index(previous, current)]
                    + beta[position][current];
                marginals[position][edge_index(previous, current)] = (score - log_z).exp();
            }
        }
    }
    marginals
}

fn active_feature_keys(tokens: &[String], min_n: usize, max_n: usize) -> Vec<Vec<FeatureKey>> {
    let len = tokens.len();
    let mut active = vec![Vec::new(); len];
    for (position, features) in active.iter_mut().enumerate() {
        for ngram_len in min_n..=max_n {
            if ngram_len > len {
                continue;
            }
            let min_start = (position + 1).saturating_sub(ngram_len);
            let max_start = position.min(len - ngram_len);
            for start in min_start..=max_start {
                features.push(FeatureKey {
                    offset: (position - start) as u8,
                    text: tokens[start..start + ngram_len].join(GRAPHEME_SEP),
                });
            }
        }
    }
    active
}

fn labels_for_record(record: &HyphenationRecord, len: usize) -> Vec<usize> {
    let breaks = record
        .breaks
        .iter()
        .map(|value| *value as usize)
        .collect::<HashSet<_>>();
    (0..len)
        .map(|position| usize::from(breaks.contains(&(position + 1))))
        .collect()
}

fn tokenize(word: &str) -> Vec<String> {
    word.graphemes(true).map(str::to_string).collect()
}

fn hyphen_allowed(position: usize, current: usize, len: usize, config: &HyphenationConfig) -> bool {
    if current == 0 {
        return true;
    }
    if len < config.min_word_len {
        return false;
    }
    let break_position = position + 1;
    break_position >= config.left_min && len.saturating_sub(break_position) >= config.right_min
}

fn edge_index(previous: usize, current: usize) -> usize {
    previous * LABELS + current
}

fn logsumexp2(a: f32, b: f32) -> f32 {
    if a <= NEG_INF / 2.0 {
        return b;
    }
    if b <= NEG_INF / 2.0 {
        return a;
    }
    let max = a.max(b);
    max + ((a - max).exp() + (b - max).exp()).ln()
}

fn is_zst_path(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("zst")
}

fn is_binary_model_path(path: &Path) -> bool {
    let path = path.as_os_str().to_string_lossy();
    path.ends_with(".bin") || path.ends_with(".bin.zst")
}

fn read_metadata(reader: &mut dyn Read) -> Result<CrfBinaryMetadata> {
    let len = read_u32(reader)? as usize;
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).context("parse CRF binary metadata")
}

fn write_metadata(writer: &mut dyn Write, metadata: &CrfBinaryMetadata) -> Result<()> {
    let bytes = serde_json::to_vec(metadata)?;
    write_u32(writer, bytes.len())?;
    writer.write_all(&bytes)?;
    Ok(())
}

fn read_u8(reader: &mut dyn Read) -> Result<u8> {
    let mut bytes = [0u8; 1];
    reader.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

fn write_u8(writer: &mut dyn Write, value: u8) -> Result<()> {
    writer.write_all(&[value])?;
    Ok(())
}

fn read_u32(reader: &mut dyn Read) -> Result<u32> {
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn write_u32(writer: &mut dyn Write, value: usize) -> Result<()> {
    let value = u32::try_from(value).context("CRF binary value exceeds u32")?;
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_f32(reader: &mut dyn Read) -> Result<f32> {
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(f32::from_le_bytes(bytes))
}

fn write_f32(writer: &mut dyn Write, value: f32) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_string(reader: &mut dyn Read) -> Result<String> {
    let len = read_u32(reader)? as usize;
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes).context("CRF binary string is not UTF-8")
}

fn write_string(writer: &mut dyn Write, value: &str) -> Result<()> {
    write_u32(writer, value.len())?;
    writer.write_all(value.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyph_core::HyphenationRecord;

    #[test]
    fn trains_and_predicts() {
        let records = vec![
            HyphenationRecord::new(
                "1",
                "en-US",
                "hyphenation",
                smallvec::smallvec![2, 6, 7],
                "test",
            ),
            HyphenationRecord::new("2", "en-US", "extensive", smallvec::smallvec![2, 5], "test"),
        ];
        let options = CrfTrainOptions {
            epochs: 2,
            threshold: 0.1,
            ..CrfTrainOptions::default()
        };
        let model = train_crf(&records, options).unwrap();
        let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
        model.hyphenate_into("hyphenation", &mut out).unwrap();
        assert!(out.iter().all(|idx| *idx > 0));
    }

    #[test]
    fn binary_roundtrip_preserves_predictions() {
        let records = vec![
            HyphenationRecord::new(
                "1",
                "en-US",
                "hyphenation",
                smallvec::smallvec![2, 6, 7],
                "test",
            ),
            HyphenationRecord::new("2", "en-US", "extensive", smallvec::smallvec![2, 5], "test"),
        ];
        let options = CrfTrainOptions {
            epochs: 2,
            threshold: 0.1,
            ..CrfTrainOptions::default()
        };
        let model = train_crf(&records, options).unwrap();
        let path = std::env::temp_dir().join(format!("hyphlab-crf-{}.bin", std::process::id()));
        model.save_binary(&path).unwrap();
        let loaded = CrfHyphenator::load_binary(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        let mut left = SmallVec::<[GraphemeIndex; 8]>::new();
        let mut right = SmallVec::<[GraphemeIndex; 8]>::new();
        model.hyphenate_into("hyphenation", &mut left).unwrap();
        loaded.hyphenate_into("hyphenation", &mut right).unwrap();
        assert_eq!(left, right);
    }
}
