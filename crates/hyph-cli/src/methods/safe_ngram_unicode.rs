// Unicode-aware safe-ngram feature encoding.

struct SafeNgramGraphemeTables {
    len: usize,
    raw: SmallVec<[u8; 32]>,
    cv: SmallVec<[u8; 32]>,
    sonority: SmallVec<[u8; 32]>,
}

impl SafeNgramGraphemeTables {
    fn codes(&self, family: u8) -> &[u8] {
        match family {
            1 => &self.cv,
            2 => &self.sonority,
            _ => &self.raw,
        }
    }
}

fn safe_ngram_uses_unicode_features(
    options: &SafeNgramOptions,
    veto_options: Option<&SafeNgramOptions>,
) -> bool {
    options.unicode_aware || veto_options.is_some_and(|options| options.unicode_aware)
}

fn safe_ngram_family_mask(
    options: &SafeNgramOptions,
    veto_options: Option<&SafeNgramOptions>,
) -> u8 {
    let mut mask = safe_ngram_options_family_mask(options);
    if let Some(veto_options) = veto_options {
        mask |= safe_ngram_options_family_mask(veto_options);
    }
    mask
}

fn safe_ngram_options_family_mask(options: &SafeNgramOptions) -> u8 {
    let mut mask = 0u8;
    for spec in &options.specs {
        mask |= match spec.family {
            1 => 1 << 1,
            2 => 1 << 2,
            _ => 1,
        };
    }
    mask
}

fn safe_ngram_grapheme_tables(word: &str, family_mask: u8) -> SafeNgramGraphemeTables {
    let mut len = 0usize;
    let mut raw = SmallVec::<[u8; 32]>::new();
    let mut cv = SmallVec::<[u8; 32]>::new();
    let mut sonority = SmallVec::<[u8; 32]>::new();
    for grapheme in UnicodeSegmentation::graphemes(word, true) {
        len += 1;
        let codes = safe_ngram_unicode_codes(grapheme);
        if family_mask & 1 != 0 {
            raw.push(codes.raw);
        }
        if family_mask & (1 << 1) != 0 {
            cv.push(codes.cv);
        }
        if family_mask & (1 << 2) != 0 {
            sonority.push(codes.sonority);
        }
    }
    SafeNgramGraphemeTables {
        len,
        raw,
        cv,
        sonority,
    }
}

fn safe_ngram_char_tables_if_simple(
    word: &str,
    family_mask: u8,
) -> Option<SafeNgramGraphemeTables> {
    let mut len = 0usize;
    let mut raw = SmallVec::<[u8; 32]>::new();
    let mut cv = SmallVec::<[u8; 32]>::new();
    let mut sonority = SmallVec::<[u8; 32]>::new();
    for ch in word.chars() {
        if !safe_ngram_char_is_single_grapheme(ch) {
            return None;
        }
        len += 1;
        let ch = safe_ngram_fast_lower_char(ch);
        let codes = safe_ngram_unicode_codes_lower_char(ch);
        if family_mask & 1 != 0 {
            raw.push(codes.raw);
        }
        if family_mask & (1 << 1) != 0 {
            cv.push(codes.cv);
        }
        if family_mask & (1 << 2) != 0 {
            sonority.push(codes.sonority);
        }
    }
    Some(SafeNgramGraphemeTables {
        len,
        raw,
        cv,
        sonority,
    })
}

fn safe_ngram_char_is_single_grapheme(ch: char) -> bool {
    !matches!(
        ch,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{fe20}'..='\u{fe2f}'
            | '\u{200d}'
    )
}

fn safe_ngram_grapheme_key(
    tables: &SafeNgramGraphemeTables,
    grapheme_len: usize,
    boundary: usize,
    spec_idx: usize,
    spec: SafeNgramSpec,
) -> u64 {
    debug_assert!(spec.left + spec.right <= 10);
    let codes = tables.codes(spec.family);
    let padded_boundary = boundary as isize + 1;
    let mut key = (spec_idx as u64) << 56;
    if spec.bucketed {
        key |= safe_ngram_boundary_bucket(grapheme_len, boundary) << 50;
    }
    let mut shift = 0u32;
    for offset in 0..spec.left {
        let position = padded_boundary - spec.left as isize + offset as isize;
        key |= safe_ngram_grapheme_code_at(codes, position) << shift;
        shift += 5;
    }
    for offset in 0..spec.right {
        let position = padded_boundary + offset as isize;
        key |= safe_ngram_grapheme_code_at(codes, position) << shift;
        shift += 5;
    }
    key
}

fn safe_ngram_grapheme_code_at(codes: &[u8], padded_position: isize) -> u64 {
    if padded_position <= 0 || padded_position > codes.len() as isize {
        return 1;
    }
    u64::from(codes[padded_position as usize - 1])
}

#[derive(Clone, Copy)]
struct SafeNgramUnicodeCodes {
    raw: u8,
    cv: u8,
    sonority: u8,
}

fn safe_ngram_unicode_codes(grapheme: &str) -> SafeNgramUnicodeCodes {
    let Some(ch) = safe_ngram_primary_lower_char(grapheme) else {
        return SafeNgramUnicodeCodes {
            raw: 31,
            cv: 8,
            sonority: 12,
        };
    };
    safe_ngram_unicode_codes_lower_char(ch)
}

fn safe_ngram_unicode_codes_lower_char(ch: char) -> SafeNgramUnicodeCodes {
    let base = safe_ngram_latin_base_letter(ch);
    let cyrillic_raw = safe_ngram_cyrillic_raw_code(ch);
    let known_alphabetic = base.is_some() || cyrillic_raw.is_some();
    let is_apostrophe = ch == '\'' || ch == '\u{2019}';
    let is_hyphen = ch == '-' || ch == '\u{2010}' || ch == '\u{2011}' || ch == '\u{2013}';
    let is_numeric = !known_alphabetic && !is_apostrophe && !is_hyphen && ch.is_numeric();
    let is_alphabetic =
        known_alphabetic || (!is_numeric && !is_apostrophe && !is_hyphen && ch.is_alphabetic());
    let is_vowel = base.is_some_and(|base| matches!(base, b'a' | b'e' | b'i' | b'o' | b'u'))
        || matches!(
            ch,
            'а' | 'е'
                | 'ё'
                | 'и'
                | 'о'
                | 'у'
                | 'ы'
                | 'э'
                | 'ю'
                | 'я'
                | 'і'
                | 'ї'
                | 'є'
                | 'ӧ'
                | 'ӱ'
        );

    let raw = if let Some(base) = base {
        base - b'a' + 2
    } else if let Some(code) = cyrillic_raw {
        code as u8
    } else if is_apostrophe {
        28
    } else if is_hyphen {
        29
    } else if is_numeric {
        30
    } else if is_alphabetic {
        (2 + (mix_u64(ch as u64) % 26)) as u8
    } else {
        31
    };

    let cv = if is_vowel {
        2
    } else if matches!(ch, 'y' | 'ý' | 'ÿ' | 'j' | 'w' | 'й') {
        3
    } else if is_alphabetic {
        4
    } else if is_apostrophe {
        5
    } else if is_hyphen {
        6
    } else if is_numeric {
        7
    } else {
        8
    };

    let sonority = if is_vowel {
        2
    } else if matches!(ch, 'y' | 'ý' | 'ÿ' | 'й') {
        3
    } else if base.is_some_and(|base| matches!(base, b'l' | b'r')) || matches!(ch, 'л' | 'р') {
        4
    } else if base.is_some_and(|base| matches!(base, b'm' | b'n')) || matches!(ch, 'м' | 'н') {
        5
    } else if base.is_some_and(|base| matches!(base, b'f' | b'v' | b's' | b'z' | b'h'))
        || matches!(ch, 'ф' | 'в' | 'с' | 'з' | 'х' | 'ш' | 'ж' | 'щ')
    {
        6
    } else if matches!(ch, 'w' | 'j') {
        7
    } else if is_alphabetic {
        8
    } else if is_apostrophe {
        9
    } else if is_hyphen {
        10
    } else if is_numeric {
        11
    } else {
        12
    };

    SafeNgramUnicodeCodes { raw, cv, sonority }
}

fn safe_ngram_fast_lower_char(ch: char) -> char {
    if ch.is_ascii() {
        return ch.to_ascii_lowercase();
    }
    match ch {
        'А'..='Я' => char::from_u32((ch as u32) + 32).unwrap_or(ch),
        'Ё' => 'ё',
        'І' => 'і',
        'Ї' => 'ї',
        'Є' => 'є',
        'Ў' => 'ў',
        'Ґ' => 'ґ',
        _ => ch,
    }
}

fn safe_ngram_cyrillic_raw_code(ch: char) -> Option<u64> {
    Some(match ch {
        'а' => 2,
        'е' | 'ё' => 3,
        'и' | 'й' | 'і' | 'ї' => 4,
        'о' => 5,
        'у' | 'ў' => 6,
        'ы' => 7,
        'э' | 'є' => 8,
        'ю' => 9,
        'я' => 10,
        'б' => 11,
        'в' => 12,
        'г' | 'ґ' => 13,
        'д' => 14,
        'ж' => 15,
        'з' => 16,
        'к' => 17,
        'л' => 18,
        'м' => 19,
        'н' => 20,
        'п' => 21,
        'р' => 22,
        'с' => 23,
        'т' => 24,
        'ф' => 25,
        'х' => 26,
        'ц' => 27,
        'ч' => 28,
        'ш' => 29,
        'щ' => 30,
        'ь' | 'ъ' => 31,
        _ => return None,
    })
}

fn safe_ngram_is_cyrillic_letter(ch: char) -> bool {
    matches!(
        ch,
        '\u{0400}'..='\u{052f}'
            | '\u{1c80}'..='\u{1c8f}'
            | '\u{2de0}'..='\u{2dff}'
            | '\u{a640}'..='\u{a69f}'
    )
}

fn safe_ngram_is_russian_cyrillic_letter(ch: char) -> bool {
    matches!(
        ch,
        'а' | 'б'
            | 'в'
            | 'г'
            | 'д'
            | 'е'
            | 'ё'
            | 'ж'
            | 'з'
            | 'и'
            | 'й'
            | 'к'
            | 'л'
            | 'м'
            | 'н'
            | 'о'
            | 'п'
            | 'р'
            | 'с'
            | 'т'
            | 'у'
            | 'ф'
            | 'х'
            | 'ц'
            | 'ч'
            | 'ш'
            | 'щ'
            | 'ъ'
            | 'ы'
            | 'ь'
            | 'э'
            | 'ю'
            | 'я'
    )
}

fn safe_ngram_primary_lower_char(grapheme: &str) -> Option<char> {
    let ch = grapheme
        .chars()
        .find(|ch| ch.is_alphanumeric())
        .or_else(|| grapheme.chars().next())?;
    ch.to_lowercase().next().or(Some(ch))
}

fn safe_ngram_latin_base_letter(ch: char) -> Option<u8> {
    if ch.is_ascii_alphabetic() {
        return Some(ch.to_ascii_lowercase() as u8);
    }
    match ch {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'ǎ' | 'ǟ' | 'ǡ' => {
            Some(b'a')
        }
        'æ' => Some(b'a'),
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => Some(b'c'),
        'ď' | 'đ' => Some(b'd'),
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => Some(b'e'),
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => Some(b'g'),
        'ĥ' | 'ħ' => Some(b'h'),
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => Some(b'i'),
        'ĵ' => Some(b'j'),
        'ķ' => Some(b'k'),
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => Some(b'l'),
        'ñ' | 'ń' | 'ņ' | 'ň' => Some(b'n'),
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'ơ' => Some(b'o'),
        'œ' => Some(b'o'),
        'ŕ' | 'ŗ' | 'ř' => Some(b'r'),
        'ś' | 'ŝ' | 'ş' | 'š' | 'ß' => Some(b's'),
        'ţ' | 'ť' | 'ŧ' => Some(b't'),
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'ư' => Some(b'u'),
        'ŵ' => Some(b'w'),
        'ý' | 'ÿ' | 'ŷ' => Some(b'y'),
        'ź' | 'ż' | 'ž' => Some(b'z'),
        _ => None,
    }
}

fn safe_ngram_unicode_is_vowel(ch: char) -> bool {
    if let Some(base) = safe_ngram_latin_base_letter(ch) {
        return matches!(base, b'a' | b'e' | b'i' | b'o' | b'u');
    }
    matches!(
        ch,
        'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я' | 'і' | 'ї' | 'є' | 'ӧ' | 'ӱ'
    )
}

