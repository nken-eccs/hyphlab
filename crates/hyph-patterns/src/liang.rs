use anyhow::Result;
use hyph_core::{BoundaryMap, GraphemeIndex, HyphenationConfig, LanguageTag};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pattern {
    pub letters: Vec<char>,
    pub weights: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternSet {
    pub patterns: Vec<Pattern>,
    pub exceptions: HashMap<String, SmallVec<[GraphemeIndex; 8]>>,
    pub left_min: Option<usize>,
    pub right_min: Option<usize>,
}

pub struct LiangHyphenator {
    id: String,
    language: LanguageTag,
    config: HyphenationConfig,
    trie: PatternTrie,
    set: PatternSet,
}

#[derive(Debug, Clone, Default)]
struct PatternTrie {
    nodes: Vec<PatternTrieNode>,
}

#[derive(Debug, Clone, Default)]
struct PatternTrieNode {
    children: SmallVec<[(char, usize); 8]>,
    weights: Option<Vec<u8>>,
}

impl LiangHyphenator {
    pub fn new(
        id: impl Into<String>,
        language: LanguageTag,
        mut config: HyphenationConfig,
        set: PatternSet,
    ) -> Self {
        if let Some(left_min) = set.left_min {
            config.left_min = left_min;
        }
        if let Some(right_min) = set.right_min {
            config.right_min = right_min;
        }
        let trie = PatternTrie::from_patterns(&set.patterns);
        Self {
            id: id.into(),
            language,
            config,
            trie,
            set,
        }
    }

    pub fn pattern_count(&self) -> usize {
        self.set.patterns.len()
    }

    pub fn hyphenate_into(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()> {
        out.clear();
        let normalized = word.nfc().collect::<String>().to_lowercase();
        if let Some(exception) = self.set.exceptions.get(&normalized) {
            out.extend(exception.iter().copied());
            self.filter_minima(word, out);
            return Ok(());
        }

        let dotted = format!(".{}.", normalized);
        let chars: Vec<char> = dotted.chars().collect();
        let mut weights = vec![0u8; chars.len() + 1];

        self.trie.apply(&chars, &mut weights);

        let original_map = BoundaryMap::new(word);
        let lower_char_to_original_grapheme =
            lower_char_boundary_to_original_grapheme(word, &normalized);

        for split in 2..chars.len().saturating_sub(1) {
            if weights[split] % 2 == 0 {
                continue;
            }
            let lower_char_boundary = split - 1;
            let Some(Some(grapheme)) = lower_char_to_original_grapheme
                .get(lower_char_boundary)
                .copied()
            else {
                continue;
            };
            if grapheme > 0 && (grapheme as usize) < original_map.grapheme_len() {
                out.push(grapheme);
            }
        }

        out.sort_unstable();
        out.dedup();
        self.filter_minima(word, out);
        Ok(())
    }

    fn filter_minima(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) {
        let len = BoundaryMap::new(word).grapheme_len();
        if len < self.config.min_word_len {
            out.clear();
            return;
        }
        out.retain(|idx| {
            let idx = *idx as usize;
            idx >= self.config.left_min && len.saturating_sub(idx) >= self.config.right_min
        });
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn language(&self) -> &LanguageTag {
        &self.language
    }

    pub fn config(&self) -> &HyphenationConfig {
        &self.config
    }
}

impl PatternTrie {
    fn from_patterns(patterns: &[Pattern]) -> Self {
        let mut trie = Self {
            nodes: vec![PatternTrieNode::default()],
        };

        for pattern in patterns {
            trie.insert(pattern);
        }

        trie
    }

    fn insert(&mut self, pattern: &Pattern) {
        let mut node_idx = 0usize;
        for ch in &pattern.letters {
            node_idx = self.child_or_insert(node_idx, *ch);
        }

        match &mut self.nodes[node_idx].weights {
            Some(existing) => merge_weights(existing, &pattern.weights),
            slot @ None => *slot = Some(pattern.weights.clone()),
        }
    }

    fn child_or_insert(&mut self, node_idx: usize, ch: char) -> usize {
        if let Some((_, child_idx)) = self.nodes[node_idx]
            .children
            .iter()
            .find(|(candidate, _)| *candidate == ch)
        {
            return *child_idx;
        }

        let child_idx = self.nodes.len();
        self.nodes.push(PatternTrieNode::default());
        self.nodes[node_idx].children.push((ch, child_idx));
        child_idx
    }

    fn apply(&self, chars: &[char], scores: &mut [u8]) {
        for start in 0..chars.len() {
            let mut node_idx = 0usize;
            for ch in &chars[start..] {
                let Some(next_idx) = self.nodes[node_idx].child(*ch) else {
                    break;
                };
                node_idx = next_idx;
                if let Some(pattern_weights) = &self.nodes[node_idx].weights {
                    apply_weights(scores, start, pattern_weights);
                }
            }
        }
    }
}

impl PatternTrieNode {
    fn child(&self, ch: char) -> Option<usize> {
        self.children
            .iter()
            .find_map(|(candidate, child_idx)| (*candidate == ch).then_some(*child_idx))
    }
}

fn merge_weights(existing: &mut Vec<u8>, incoming: &[u8]) {
    if incoming.len() > existing.len() {
        existing.resize(incoming.len(), 0);
    }
    for (idx, weight) in incoming.iter().copied().enumerate() {
        existing[idx] = existing[idx].max(weight);
    }
}

fn apply_weights(scores: &mut [u8], start: usize, pattern_weights: &[u8]) {
    for (offset, weight) in pattern_weights.iter().copied().enumerate() {
        let slot = start + offset;
        if slot < scores.len() {
            scores[slot] = scores[slot].max(weight);
        }
    }
}

fn lower_char_boundary_to_original_grapheme(
    original: &str,
    lower: &str,
) -> Vec<Option<GraphemeIndex>> {
    let original_map = BoundaryMap::new(original);
    let lower_char_boundaries = char_boundaries(lower);
    let mut map = vec![None; lower_char_boundaries.len()];

    // This fast path is correct for the primary TeX/Hunspell use case where
    // lowercasing does not change byte length. If it does, we conservatively
    // fall back to char boundary ranks.
    if original.len() == lower.len() {
        for (idx, byte) in lower_char_boundaries.iter().copied().enumerate() {
            map[idx] = original_map.byte_to_grapheme_break(byte);
        }
        return map;
    }

    let original_char_boundaries = char_boundaries(original);
    for (char_rank, slot) in map.iter_mut().enumerate() {
        if let Some(original_byte) = original_char_boundaries.get(char_rank).copied() {
            *slot = original_map.byte_to_grapheme_break(original_byte);
        }
    }
    map
}

fn char_boundaries(text: &str) -> Vec<usize> {
    let mut boundaries = text
        .char_indices()
        .map(|(byte, _)| byte)
        .collect::<Vec<_>>();
    boundaries.push(text.len());
    boundaries
}
