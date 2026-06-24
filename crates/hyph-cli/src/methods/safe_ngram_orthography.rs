// Orthographic vetoes and byte-level feature tables.

fn safe_ngram_apply_orthographic_veto(bytes: &[u8], out: &mut SmallVec<[GraphemeIndex; 8]>) {
    out.retain(|boundary| safe_ngram_orthographic_break_allowed(bytes, *boundary as usize));
}

fn safe_ngram_orthographic_break_allowed(bytes: &[u8], boundary: usize) -> bool {
    if boundary == 0 || boundary >= bytes.len() {
        return false;
    }
    let left = bytes[boundary - 1].to_ascii_lowercase();
    let right = bytes[boundary].to_ascii_lowercase();

    if matches!(
        (left, right),
        (b'c', b'h')
            | (b'c', b'k')
            | (b'p', b'h')
            | (b'q', b'u')
            | (b's', b'h')
            | (b't', b'h')
            | (b'w', b'h')
    ) {
        return false;
    }

    if is_safe_ngram_vowelish(left)
        && is_safe_ngram_vowelish(right)
        && matches!(
            (left, right),
            (b'a', b'i')
                | (b'a', b'u')
                | (b'a', b'w')
                | (b'a', b'y')
                | (b'e', b'a')
                | (b'e', b'e')
                | (b'e', b'i')
                | (b'e', b'w')
                | (b'e', b'y')
                | (b'i', b'e')
                | (b'o', b'a')
                | (b'o', b'e')
                | (b'o', b'i')
                | (b'o', b'o')
                | (b'o', b'u')
                | (b'o', b'w')
                | (b'o', b'y')
                | (b'u', b'e')
                | (b'u', b'i')
                | (b'u', b'y')
        )
    {
        return false;
    }

    true
}

fn is_safe_ngram_vowelish(byte: u8) -> bool {
    matches!(byte, b'a' | b'e' | b'i' | b'o' | b'u' | b'y')
}

fn safe_ngram_key_with<F>(
    bytes: &[u8],
    boundary: usize,
    spec_idx: usize,
    spec: SafeNgramSpec,
    code_at: F,
) -> u64
where
    F: Fn(&[u8], isize) -> u64,
{
    debug_assert!(spec.left + spec.right <= 10);
    let padded_boundary = boundary as isize + 1;
    let mut key = (spec_idx as u64) << 56;
    if spec.bucketed {
        key |= safe_ngram_boundary_bucket(bytes.len(), boundary) << 50;
    }
    let mut shift = 0u32;
    for offset in 0..spec.left {
        let position = padded_boundary - spec.left as isize + offset as isize;
        key |= code_at(bytes, position) << shift;
        shift += 5;
    }
    for offset in 0..spec.right {
        let position = padded_boundary + offset as isize;
        key |= code_at(bytes, position) << shift;
        shift += 5;
    }
    key
}

fn safe_ngram_grapheme_key_from_codes(codes: &[u8], boundary: usize, spec: SafeNgramSpec) -> u64 {
    debug_assert!(spec.left + spec.right <= 10);
    let padded_boundary = boundary as isize + 1;
    let mut key = 0u64;
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

fn safe_ngram_boundary_bucket(byte_len: usize, boundary: usize) -> u64 {
    let right = byte_len.saturating_sub(boundary);
    let edge_bucket = if boundary <= 2 {
        0
    } else if right <= 3 {
        1
    } else if boundary <= 3 {
        2
    } else if right <= 4 {
        3
    } else {
        4
    };
    let len_bucket = if byte_len <= 6 {
        0
    } else if byte_len <= 8 {
        1
    } else if byte_len <= 11 {
        2
    } else {
        3
    };
    ((len_bucket << 3) | edge_bucket) as u64
}

fn safe_ngram_cv_code_at(bytes: &[u8], padded_position: isize) -> u64 {
    if padded_position <= 0 || padded_position > bytes.len() as isize {
        return 1;
    }
    let byte = bytes[padded_position as usize - 1].to_ascii_lowercase();
    match byte {
        b'a' | b'e' | b'i' | b'o' | b'u' => 2,
        b'y' => 3,
        b'a'..=b'z' => 4,
        b'\'' => 5,
        b'-' => 6,
        b'0'..=b'9' => 7,
        _ => 8,
    }
}

fn safe_ngram_sonority_code_at(bytes: &[u8], padded_position: isize) -> u64 {
    if padded_position <= 0 || padded_position > bytes.len() as isize {
        return 1;
    }
    let byte = bytes[padded_position as usize - 1].to_ascii_lowercase();
    match byte {
        b'a' | b'e' | b'i' | b'o' | b'u' => 2,
        b'y' => 3,
        b'l' | b'r' => 4,
        b'm' | b'n' => 5,
        b'f' | b'v' | b's' | b'z' | b'h' => 6,
        b'w' | b'j' => 7,
        b'b' | b'c' | b'd' | b'g' | b'k' | b'p' | b'q' | b't' | b'x' => 8,
        b'\'' => 9,
        b'-' => 10,
        b'0'..=b'9' => 11,
        _ => 12,
    }
}

const SAFE_NGRAM_RAW_CODES: [u64; 256] = build_safe_ngram_raw_codes();

const fn build_safe_ngram_raw_codes() -> [u64; 256] {
    let mut codes = [31u64; 256];
    let mut idx = 0usize;
    while idx < 26 {
        codes[b'a' as usize + idx] = idx as u64 + 2;
        codes[b'A' as usize + idx] = idx as u64 + 2;
        idx += 1;
    }
    codes[b'\'' as usize] = 28;
    codes[b'-' as usize] = 29;
    idx = 0;
    while idx < 10 {
        codes[b'0' as usize + idx] = 30;
        idx += 1;
    }
    codes
}

fn safe_ngram_raw_code_at(bytes: &[u8], padded_position: isize) -> u64 {
    if padded_position <= 0 || padded_position > bytes.len() as isize {
        return 1;
    }
    SAFE_NGRAM_RAW_CODES[bytes[padded_position as usize - 1] as usize]
}

