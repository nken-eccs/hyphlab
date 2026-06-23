mod liang;
mod parser;

pub use liang::{LiangHyphenator, Pattern, PatternSet};
pub use parser::{parse_pattern_file, parse_pattern_str, parse_pattern_token};

#[cfg(test)]
mod tests {
    use super::*;
    use hyph_core::{GraphemeIndex, HyphenationConfig};
    use smallvec::SmallVec;

    #[test]
    fn parses_pattern_token() {
        let pattern = parse_pattern_token(".hy3phen").unwrap();
        assert_eq!(pattern.letters.iter().collect::<String>(), ".hyphen");
        assert_eq!(pattern.weights[3], 3);
    }

    #[test]
    fn applies_toy_patterns() {
        let mut set = PatternSet::default();
        set.patterns.push(parse_pattern_token("hy3phen").unwrap());
        set.patterns.push(parse_pattern_token("phen3a").unwrap());
        set.patterns.push(parse_pattern_token("a3tion").unwrap());
        let engine = LiangHyphenator::new(
            "toy",
            "en-US".parse().unwrap(),
            HyphenationConfig::default(),
            set,
        );
        let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
        engine.hyphenate_into("hyphenation", &mut out).unwrap();
        assert_eq!(out.as_slice(), &[2, 6, 7]);
    }

    #[test]
    fn skips_libhyphen_directive_lines() {
        let set = parse_pattern_str(
            "\
UTF-8
LEFTHYPHENMIN 2
RIGHTHYPHENMIN 3
COMPOUNDLEFTHYPHENMIN 2
COMPOUNDRIGHTHYPHENMIN 3
NOHYPHEN ',-
hy3phen
",
        )
        .unwrap();

        assert_eq!(set.left_min, Some(2));
        assert_eq!(set.right_min, Some(3));
        assert_eq!(set.patterns.len(), 1);
        assert_eq!(set.patterns[0].letters.iter().collect::<String>(), "hyphen");
    }
}
