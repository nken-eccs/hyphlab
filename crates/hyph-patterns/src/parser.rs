use crate::{Pattern, PatternSet};
use anyhow::{Context, Result};
use hyph_core::hyphenated_to_breaks;
use std::{fs, path::Path};

pub fn parse_pattern_file(path: impl AsRef<Path>) -> Result<PatternSet> {
    let path = path.as_ref();
    let bytes = fs::read(path).with_context(|| format!("read pattern file {}", path.display()))?;
    let raw = decode_pattern_bytes(&bytes)
        .with_context(|| format!("read pattern file {}", path.display()))?;
    parse_pattern_str(&raw)
}

fn decode_pattern_bytes(bytes: &[u8]) -> Result<String> {
    if let Ok(raw) = std::str::from_utf8(bytes) {
        return Ok(raw.to_owned());
    }

    let encoding = first_ascii_line(bytes).to_ascii_uppercase();
    if matches!(encoding.as_str(), "ISO8859-1" | "ISO-8859-1" | "LATIN1" | "LATIN-1") {
        return Ok(decode_latin1(bytes));
    }
    if matches!(encoding.as_str(), "ISO8859-2" | "ISO-8859-2" | "LATIN2" | "LATIN-2") {
        return Ok(decode_latin2(bytes));
    }

    anyhow::bail!("pattern file is not valid UTF-8 and declares unsupported encoding {encoding:?}")
}

fn first_ascii_line(bytes: &[u8]) -> String {
    bytes
        .iter()
        .copied()
        .take_while(|byte| !matches!(*byte, b'\n' | b'\r'))
        .filter(u8::is_ascii)
        .map(char::from)
        .collect::<String>()
        .trim()
        .to_string()
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().copied().map(char::from).collect()
}

fn decode_latin2(bytes: &[u8]) -> String {
    bytes
        .iter()
        .copied()
        .map(|byte| match byte {
            0x00..=0x7f => char::from(byte),
            0x80..=0x9f => char::from(byte),
            0xa0 => '\u{00a0}',
            0xa1 => '\u{0104}',
            0xa2 => '\u{02d8}',
            0xa3 => '\u{0141}',
            0xa4 => '\u{00a4}',
            0xa5 => '\u{013d}',
            0xa6 => '\u{015a}',
            0xa7 => '\u{00a7}',
            0xa8 => '\u{00a8}',
            0xa9 => '\u{0160}',
            0xaa => '\u{015e}',
            0xab => '\u{0164}',
            0xac => '\u{0179}',
            0xad => '\u{00ad}',
            0xae => '\u{017d}',
            0xaf => '\u{017b}',
            0xb0 => '\u{00b0}',
            0xb1 => '\u{0105}',
            0xb2 => '\u{02db}',
            0xb3 => '\u{0142}',
            0xb4 => '\u{00b4}',
            0xb5 => '\u{013e}',
            0xb6 => '\u{015b}',
            0xb7 => '\u{02c7}',
            0xb8 => '\u{00b8}',
            0xb9 => '\u{0161}',
            0xba => '\u{015f}',
            0xbb => '\u{0165}',
            0xbc => '\u{017a}',
            0xbd => '\u{02dd}',
            0xbe => '\u{017e}',
            0xbf => '\u{017c}',
            0xc0 => '\u{0154}',
            0xc1 => '\u{00c1}',
            0xc2 => '\u{00c2}',
            0xc3 => '\u{0102}',
            0xc4 => '\u{00c4}',
            0xc5 => '\u{0139}',
            0xc6 => '\u{0106}',
            0xc7 => '\u{00c7}',
            0xc8 => '\u{010c}',
            0xc9 => '\u{00c9}',
            0xca => '\u{0118}',
            0xcb => '\u{00cb}',
            0xcc => '\u{011a}',
            0xcd => '\u{00cd}',
            0xce => '\u{00ce}',
            0xcf => '\u{010e}',
            0xd0 => '\u{0110}',
            0xd1 => '\u{0143}',
            0xd2 => '\u{0147}',
            0xd3 => '\u{00d3}',
            0xd4 => '\u{00d4}',
            0xd5 => '\u{0150}',
            0xd6 => '\u{00d6}',
            0xd7 => '\u{00d7}',
            0xd8 => '\u{0158}',
            0xd9 => '\u{016e}',
            0xda => '\u{00da}',
            0xdb => '\u{0170}',
            0xdc => '\u{00dc}',
            0xdd => '\u{00dd}',
            0xde => '\u{0162}',
            0xdf => '\u{00df}',
            0xe0 => '\u{0155}',
            0xe1 => '\u{00e1}',
            0xe2 => '\u{00e2}',
            0xe3 => '\u{0103}',
            0xe4 => '\u{00e4}',
            0xe5 => '\u{013a}',
            0xe6 => '\u{0107}',
            0xe7 => '\u{00e7}',
            0xe8 => '\u{010d}',
            0xe9 => '\u{00e9}',
            0xea => '\u{0119}',
            0xeb => '\u{00eb}',
            0xec => '\u{011b}',
            0xed => '\u{00ed}',
            0xee => '\u{00ee}',
            0xef => '\u{010f}',
            0xf0 => '\u{0111}',
            0xf1 => '\u{0144}',
            0xf2 => '\u{0148}',
            0xf3 => '\u{00f3}',
            0xf4 => '\u{00f4}',
            0xf5 => '\u{0151}',
            0xf6 => '\u{00f6}',
            0xf7 => '\u{00f7}',
            0xf8 => '\u{0159}',
            0xf9 => '\u{016f}',
            0xfa => '\u{00fa}',
            0xfb => '\u{0171}',
            0xfc => '\u{00fc}',
            0xfd => '\u{00fd}',
            0xfe => '\u{0163}',
            0xff => '\u{02d9}',
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_latin1_pattern_files() {
        let raw = decode_pattern_bytes(b"ISO8859-1\n.4gr\xfc\n").unwrap();
        assert!(raw.contains(".4gr\u{00fc}"));
    }

    #[test]
    fn decodes_latin2_pattern_files() {
        let raw = decode_pattern_bytes(b"ISO8859-2\n.a4d\xec\n").unwrap();
        assert!(raw.contains(".a4d\u{011b}"));
    }
}
