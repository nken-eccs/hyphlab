// Dense lookup tables for safe-ngram rule sets.

// Safe-ngram learned rule tables and runtime dispatch.

const SAFE_NGRAM_SPEC1_PREFIX: u64 = 1u64 << 56;

impl SafeNgramDenseSet {
    fn from_options(rules: &U64HashSet, options: &SafeNgramOptions) -> Option<Self> {
        let spec = options.specs.first().copied()?;
        if options.specs.len() != 1 || spec.bucketed || rules.is_empty() {
            return None;
        }
        let total_bits = (spec.left + spec.right).checked_mul(5)?;
        if total_bits > 25 {
            return None;
        }
        Self::from_unprefixed_keys(total_bits, rules.iter().copied())
    }

    fn from_unprefixed_keys<I>(total_bits: usize, keys: I) -> Option<Self>
    where
        I: IntoIterator<Item = u64>,
    {
        let bit_count = 1usize.checked_shl(total_bits as u32)?;
        let mut bits = vec![0u64; bit_count.div_ceil(64)];
        for key in keys {
            let key = usize::try_from(key).ok()?;
            if key >= bit_count {
                return None;
            }
            bits[key >> 6] |= 1u64 << (key & 63);
        }
        Some(Self { bit_count, bits })
    }

    #[inline]
    fn contains(&self, key: u64) -> bool {
        let Ok(key) = usize::try_from(key) else {
            return false;
        };
        if key >= self.bit_count {
            return false;
        }
        (self.bits[key >> 6] & (1u64 << (key & 63))) != 0
    }
}

impl SafeNgramDualDenseSet {
    fn from_options(rules: &U64HashSet, options: &SafeNgramOptions) -> Option<Self> {
        if options.specs.len() != 2
            || options.specs.iter().any(|spec| spec.bucketed)
            || rules.is_empty()
        {
            return None;
        }
        let first_bits = (options.specs[0].left + options.specs[0].right).checked_mul(5)?;
        let second_bits = (options.specs[1].left + options.specs[1].right).checked_mul(5)?;
        if first_bits > 25 || second_bits > 25 {
            return None;
        }

        let mut first = Vec::new();
        let mut second = Vec::new();
        for key in rules {
            if key & SAFE_NGRAM_SPEC1_PREFIX != 0 {
                second.push(key & !SAFE_NGRAM_SPEC1_PREFIX);
            } else {
                first.push(*key);
            }
        }
        Some(Self {
            first: SafeNgramDenseSet::from_unprefixed_keys(first_bits, first)?,
            second: SafeNgramDenseSet::from_unprefixed_keys(second_bits, second)?,
        })
    }

    #[inline]
    fn contains(&self, key0: u64, key1: u64) -> bool {
        self.first.contains(key0) || self.second.contains(key1)
    }
}

impl<'a> SafeNgramRuleLookup<'a> {
    fn contains(self, key: u64) -> bool {
        match self {
            Self::Hash(rules) => rules.contains(&key),
            Self::Dense(rules) => rules.contains(key),
        }
    }
}

impl<'a> SafeNgramDualRuleLookup<'a> {
    #[inline]
    fn contains(self, key0: u64, key1: u64) -> bool {
        match self {
            Self::Hash(rules) => {
                rules.contains(&key0) || rules.contains(&(SAFE_NGRAM_SPEC1_PREFIX | key1))
            }
            Self::Dense(rules) => rules.contains(key0, key1),
        }
    }
}
