use std::{
    collections::{HashMap, HashSet},
    hash::{BuildHasherDefault, Hasher},
};

pub(crate) type U64HashMap<V> = HashMap<u64, V, BuildHasherDefault<IdentityHasher>>;
pub(crate) type U64HashSet = HashSet<u64, BuildHasherDefault<IdentityHasher>>;

// Many learned models use pre-hashed u64 feature keys. This hasher avoids
// re-hashing those keys through SipHash while still mixing accidental byte writes.
#[derive(Debug, Default)]
pub(crate) struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut value = 0u64;
        for (shift, byte) in bytes.iter().take(8).enumerate() {
            value |= (*byte as u64) << (shift * 8);
        }
        self.0 = mix_u64(value);
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = mix_u64(value);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub(crate) fn mix_u64(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51afd7ed558ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ceb9fe1a85ec53);
    value ^ (value >> 33)
}
