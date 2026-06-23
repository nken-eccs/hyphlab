mod boundary;
mod language;
mod record;
mod separator;
mod traits;

pub use boundary::BoundaryMap;
pub use language::LanguageTag;
pub use record::{HyphenationRecord, Normalization};
pub use separator::{hyphenated_to_breaks, insert_separator, strip_separators};
pub use traits::{GraphemeIndex, Hyphenation, HyphenationConfig, Hyphenator};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_hyphenated_word_to_grapheme_breaks() {
        let breaks = hyphenated_to_breaks("hyphenation", "hy-phen-a-tion").unwrap();
        assert_eq!(breaks.as_slice(), &[2, 6, 7]);
    }

    #[test]
    fn parses_combining_mark_breaks_by_grapheme() {
        let word = "e\u{301}clair";
        let hyphenated = "e\u{301}-clair";
        let breaks = hyphenated_to_breaks(word, hyphenated).unwrap();
        assert_eq!(breaks.as_slice(), &[1]);
    }

    #[test]
    fn boundary_map_rejects_middle_of_grapheme() {
        let map = BoundaryMap::new("e\u{301}clair");
        assert_eq!(map.byte_to_grapheme_break(1), None);
        assert_eq!(map.byte_to_grapheme_break("e\u{301}".len()), Some(1));
    }

    #[test]
    fn inserts_separator_at_grapheme_indices() {
        let word = "hyphenation";
        let out = insert_separator(word, &[2, 6, 7], "-");
        assert_eq!(out, "hy-phen-a-tion");
    }
}
