use crate::{BoundaryMap, GraphemeIndex};
use anyhow::{Context, Result};
use smallvec::SmallVec;
use unicode_segmentation::UnicodeSegmentation;

const SEPARATORS: &[char] = &['-', '\u{2010}', '\u{2011}', '\u{00b7}', '\u{2027}', '|'];

fn is_separator(ch: char) -> bool {
    SEPARATORS.contains(&ch)
}

pub fn strip_separators(input: &str) -> String {
    input.chars().filter(|ch| !is_separator(*ch)).collect()
}

pub fn hyphenated_to_breaks(word: &str, hyphenated: &str) -> Result<SmallVec<[GraphemeIndex; 8]>> {
    let stripped = strip_separators(hyphenated);
    anyhow::ensure!(
        stripped == word,
        "hyphenated form does not match word after separator removal: word={word:?}, stripped={stripped:?}"
    );

    let map = BoundaryMap::new(word);
    let mut breaks = SmallVec::new();
    let mut byte_offset = 0usize;

    for part in hyphenated.split(is_separator) {
        byte_offset += part.len();
        if byte_offset >= word.len() {
            continue;
        }

        let idx = map
            .byte_to_grapheme_break(byte_offset)
            .with_context(|| format!("separator falls inside grapheme at byte {byte_offset}"))?;
        breaks.push(idx);
    }

    breaks.sort_unstable();
    breaks.dedup();
    Ok(breaks)
}

pub fn insert_separator(word: &str, breaks: &[GraphemeIndex], sep: &str) -> String {
    if breaks.is_empty() {
        return word.to_string();
    }

    let mut out = String::with_capacity(word.len() + sep.len() * breaks.len());
    let mut break_iter = breaks.iter().copied().peekable();

    for (idx, grapheme) in word.graphemes(true).enumerate() {
        if break_iter.peek() == Some(&(idx as GraphemeIndex)) {
            out.push_str(sep);
            while break_iter.peek() == Some(&(idx as GraphemeIndex)) {
                break_iter.next();
            }
        }
        out.push_str(grapheme);
    }

    let end_idx = word.graphemes(true).count() as GraphemeIndex;
    if break_iter.peek() == Some(&end_idx) {
        out.push_str(sep);
    }

    out
}
