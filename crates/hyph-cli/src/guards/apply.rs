use anyhow::Result;
use smallvec::SmallVec;
use unicode_segmentation::UnicodeSegmentation;

use super::GuardPolicySet;
use crate::{
    alphabetic_spans, curate_break_set, filter_typeset_fragments, GraphemeIndex, GuardedMethod,
    HyphenationRecord,
};

impl GuardPolicySet {
    pub(crate) fn apply_runtime(
        &self,
        word: &str,
        config: &crate::HyphenationConfig,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) {
        if out.is_empty() {
            return;
        }
        if let Some(fragments) = &self.fragment_filter {
            filter_typeset_fragments(word, config, fragments, out);
        }
        if self.case.is_some() || self.proper_names.is_some() {
            self.apply_token_guards(word, out);
        }
    }

    pub(crate) fn curate_breaks(
        &self,
        word: &str,
        breaks: &[GraphemeIndex],
        left_min: usize,
        right_min: usize,
    ) -> (SmallVec<[GraphemeIndex; 8]>, Vec<String>) {
        let (out, mut reasons) = if let Some(rules) = &self.fragment_rules {
            curate_break_set(word, breaks, left_min, right_min, rules)
        } else {
            curate_break_set_min_only(word, breaks, left_min, right_min)
        };

        if self.case.is_none() && self.proper_names.is_none() {
            return (out, reasons);
        }

        let tokens = self.guarded_tokens(word);
        let mut kept = SmallVec::<[GraphemeIndex; 8]>::new();
        for break_idx in out.iter().copied() {
            if let Some(reason) = self.token_guard_reason(&tokens, break_idx) {
                reasons.push(format!("remove:{break_idx}:{reason}"));
            } else {
                kept.push(break_idx);
            }
        }
        kept.sort_unstable();
        kept.dedup();
        (kept, reasons)
    }

    fn apply_token_guards(&self, word: &str, out: &mut SmallVec<[GraphemeIndex; 8]>) {
        if self.apply_lowercase_word_fast_path(word, out) {
            return;
        }
        let tokens = self.guarded_tokens(word);
        out.retain(|break_idx| self.token_guard_reason(&tokens, *break_idx).is_none());
    }

    fn apply_lowercase_word_fast_path(
        &self,
        word: &str,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) -> bool {
        if word.is_empty() || !is_single_lowercase_alpha_token(word) {
            return false;
        }
        if self
            .proper_names
            .as_ref()
            .is_some_and(|proper_names| proper_names.protects_lowercase_token(word))
        {
            out.clear();
        }
        true
    }

    fn guarded_tokens(&self, word: &str) -> Vec<GuardedToken> {
        let needs_folded = self
            .proper_names
            .as_ref()
            .is_some_and(|proper_names| proper_names.needs_folded_token());
        guarded_tokens(word, needs_folded)
    }

    fn token_guard_reason(
        &self,
        tokens: &[GuardedToken],
        break_idx: GraphemeIndex,
    ) -> Option<&'static str> {
        let idx = usize::from(break_idx);
        let token = tokens
            .iter()
            .find(|token| token.start < idx && idx < token.end)?;
        if let Some(proper_names) = &self.proper_names {
            if let Some(reason) =
                proper_names.protected_reason(&token.text, token.folded.as_deref())
            {
                return Some(reason);
            }
        }
        if let Some(case) = &self.case {
            if let Some(reason) = case.protected_reason(&token.text) {
                return Some(reason);
            }
        }
        None
    }
}

impl GuardedMethod {
    pub(crate) fn hyphenate_record_into(
        &self,
        record: &HyphenationRecord,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) -> Result<()> {
        self.inner.hyphenate_record_into(record, out)?;
        self.guards.apply_runtime(&record.word, &self.config, out);
        Ok(())
    }

    pub(crate) fn hyphenate_into(
        &self,
        word: &str,
        out: &mut SmallVec<[GraphemeIndex; 8]>,
    ) -> Result<()> {
        self.inner.hyphenate_into(word, out)?;
        self.guards.apply_runtime(word, &self.config, out);
        Ok(())
    }
}

struct GuardedToken {
    start: usize,
    end: usize,
    text: String,
    folded: Option<String>,
}

fn guarded_tokens(word: &str, needs_folded: bool) -> Vec<GuardedToken> {
    if word.is_ascii() {
        return guarded_tokens_ascii(word, needs_folded);
    }
    let graphemes = word.graphemes(true).collect::<Vec<_>>();
    alphabetic_spans(&graphemes)
        .into_iter()
        .filter_map(|(start, end)| {
            let text = graphemes[start..end].concat();
            if text.is_empty() {
                return None;
            }
            let folded = needs_folded.then(|| text.to_lowercase());
            Some(GuardedToken {
                start,
                end,
                text,
                folded,
            })
        })
        .collect()
}

fn guarded_tokens_ascii(word: &str, needs_folded: bool) -> Vec<GuardedToken> {
    let bytes = word.as_bytes();
    let mut spans = Vec::new();
    let mut start = None;
    for (idx, byte) in bytes.iter().enumerate() {
        let is_alpha = byte.is_ascii_alphabetic();
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
        spans.push((open, bytes.len()));
    }

    spans
        .into_iter()
        .map(|(start, end)| GuardedToken {
            start,
            end,
            text: word[start..end].to_string(),
            folded: needs_folded.then(|| word[start..end].to_ascii_lowercase()),
        })
        .collect()
}

fn is_single_lowercase_alpha_token(word: &str) -> bool {
    word.chars().all(|ch| {
        ch.is_alphabetic()
            && !ch.is_uppercase()
            && (ch.is_lowercase() || !ch.to_uppercase().any(|upper| upper != ch))
    })
}

fn curate_break_set_min_only(
    word: &str,
    breaks: &[GraphemeIndex],
    left_min: usize,
    right_min: usize,
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
        if idx.saturating_sub(start) < left_min {
            reasons.push(format!("remove:{break_idx}:left_min"));
            continue;
        }
        if end.saturating_sub(idx) < right_min {
            reasons.push(format!("remove:{break_idx}:right_min"));
            continue;
        }
        out.push(break_idx);
    }

    out.sort_unstable();
    out.dedup();
    (out, reasons)
}
