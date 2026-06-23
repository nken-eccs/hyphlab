use crate::{Pattern, PatternSet};
use anyhow::{Context, Result};
use hyph_core::hyphenated_to_breaks;
use std::{fs, path::Path};

pub fn parse_pattern_file(path: impl AsRef<Path>) -> Result<PatternSet> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read pattern file {}", path.display()))?;
    parse_pattern_str(&raw)
}

pub fn parse_pattern_str(input: &str) -> Result<PatternSet> {
    let mut set = PatternSet::default();
    let cleaned = strip_comments(input);

    for body in extract_tex_blocks(&cleaned, "patterns") {
        for token in body.split_whitespace() {
            if let Some(pattern) = parse_token_maybe_pattern(token)? {
                set.patterns.push(pattern);
            }
        }
    }

    for body in extract_tex_blocks(&cleaned, "hyphenation") {
        for token in body.split_whitespace() {
            let word = token.replace('-', "");
            if word.is_empty() {
                continue;
            }
            let breaks = hyphenated_to_breaks(&word, token)
                .with_context(|| format!("parse exception {token:?}"))?;
            set.exceptions.insert(word.to_lowercase(), breaks);
        }
    }

    if !set.patterns.is_empty() || !set.exceptions.is_empty() {
        return Ok(set);
    }

    parse_flat_or_dic(&cleaned)
}

pub fn parse_pattern_token(token: &str) -> Result<Pattern> {
    let token = token.trim();
    anyhow::ensure!(!token.is_empty(), "empty pattern");

    let mut letters = Vec::new();
    let mut weights = vec![0u8];

    for ch in token.chars() {
        if let Some(digit) = ch.to_digit(10) {
            let last = weights
                .last_mut()
                .expect("weights always has at least one slot");
            *last = digit as u8;
        } else {
            letters.push(ch.to_ascii_lowercase());
            weights.push(0);
        }
    }

    anyhow::ensure!(!letters.is_empty(), "pattern {token:?} contains no letters");
    Ok(Pattern { letters, weights })
}

fn parse_token_maybe_pattern(token: &str) -> Result<Option<Pattern>> {
    let token = token.trim();
    if token.is_empty() || token.starts_with('%') || token.starts_with('#') {
        return Ok(None);
    }
    Ok(Some(parse_pattern_token(token)?))
}

fn parse_flat_or_dic(input: &str) -> Result<PatternSet> {
    let mut set = PatternSet::default();
    let mut saw_encoding_line = false;

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('%') || line.starts_with('#') {
            continue;
        }
        if line.eq_ignore_ascii_case("UTF-8") {
            saw_encoding_line = true;
            continue;
        }
        if !saw_encoding_line && looks_like_encoding_line(line) {
            saw_encoding_line = true;
            continue;
        }
        if let Some(value) = directive_value(line, "LEFTHYPHENMIN") {
            set.left_min = parse_min_value(value);
            continue;
        }
        if let Some(value) = directive_value(line, "RIGHTHYPHENMIN") {
            set.right_min = parse_min_value(value);
            continue;
        }
        if is_ignored_directive(line) {
            continue;
        }
        if line.contains('/') {
            // Non-standard libhyphen replacement patterns are out of scope for
            // this first standard Liang engine.
            continue;
        }
        if let Some(pattern) = parse_token_maybe_pattern(line)? {
            set.patterns.push(pattern);
        }
    }

    Ok(set)
}

fn parse_min_value(value: &str) -> Option<usize> {
    value
        .split_whitespace()
        .find_map(|part| part.parse::<usize>().ok())
}

fn directive_value<'a>(line: &'a str, directive: &str) -> Option<&'a str> {
    let mut parts = line.splitn(2, char::is_whitespace);
    let first = parts.next()?;
    first
        .eq_ignore_ascii_case(directive)
        .then(|| parts.next().unwrap_or(""))
}

fn looks_like_encoding_line(line: &str) -> bool {
    let upper = line.to_ascii_uppercase();
    upper.starts_with("ISO") || upper.starts_with("WINDOWS-") || upper.starts_with("CP")
}

fn is_ignored_directive(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let first = parts.next().unwrap_or("").to_ascii_uppercase();
    matches!(
        first.as_str(),
        "NEXTLEVEL" | "COMPOUNDLEFTHYPHENMIN" | "COMPOUNDRIGHTHYPHENMIN" | "NOHYPHEN"
    )
}

fn strip_comments(input: &str) -> String {
    input
        .lines()
        .map(|line| line.split('%').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_tex_blocks(input: &str, name: &str) -> Vec<String> {
    let needle = format!("\\{name}");
    let mut blocks = Vec::new();
    let mut rest = input;

    while let Some(start) = rest.find(&needle) {
        rest = &rest[start + needle.len()..];
        let Some(open_rel) = rest.find('{') else {
            break;
        };
        let after_open = &rest[open_rel + 1..];
        let mut depth = 1usize;
        let mut end_byte = None;
        for (idx, ch) in after_open.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_byte = Some(idx);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end_byte else {
            break;
        };
        blocks.push(after_open[..end].to_string());
        rest = &after_open[end + 1..];
    }

    blocks
}
