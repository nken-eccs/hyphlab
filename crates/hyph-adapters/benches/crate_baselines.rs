use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hyph_adapters::{HypherAdapter, MethodAdapter, NoHyphen};
use hyph_core::{GraphemeIndex, LanguageTag};
use smallvec::SmallVec;

#[cfg(feature = "rust-hyphenation-embedded")]
use hyph_adapters::HyphenationCrateAdapter;

const WORDS: &[&str] = &[
    "hyphenation",
    "dictionary",
    "extensive",
    "antidisestablishmentarianism",
    "diagnostics",
    "internationalization",
    "probability",
    "recognize",
];

fn bench_hypher(c: &mut Criterion) {
    let adapter = HypherAdapter::for_locale("en-US").unwrap();
    c.bench_function("hypher_native_en_us_segments", |b| {
        b.iter(|| {
            for word in WORDS {
                let bytes = adapter
                    .native_segments(black_box(word))
                    .map(str::len)
                    .sum::<usize>();
                black_box(bytes);
            }
        })
    });
    c.bench_function("hypher_adapter_en_us_words", |b| {
        b.iter(|| {
            let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
            for word in WORDS {
                adapter.hyphenate_into(black_box(word), &mut out).unwrap();
                black_box(&out);
            }
        })
    });
}

fn bench_no_hyphen(c: &mut Criterion) {
    let adapter = NoHyphen::new(LanguageTag::default());
    c.bench_function("no_hyphen_words", |b| {
        b.iter(|| {
            let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
            for word in WORDS {
                adapter.hyphenate_into(black_box(word), &mut out).unwrap();
                black_box(&out);
            }
        })
    });
}

#[cfg(feature = "rust-hyphenation-embedded")]
fn bench_hyphenation_crate(c: &mut Criterion) {
    let adapter = HyphenationCrateAdapter::embedded_en_us().unwrap();
    c.bench_function("hyphenation_0_8_4_embedded_native_en_us_words", |b| {
        b.iter(|| {
            for word in WORDS {
                let breaks = adapter.native_breaks(black_box(word));
                black_box(breaks);
            }
        })
    });
    c.bench_function("hyphenation_0_8_4_embedded_en_us_words", |b| {
        b.iter(|| {
            let mut out = SmallVec::<[GraphemeIndex; 8]>::new();
            for word in WORDS {
                adapter.hyphenate_into(black_box(word), &mut out).unwrap();
                black_box(&out);
            }
        })
    });
    c.bench_function("hyphenation_0_8_4_embedded_load_en_us", |b| {
        b.iter(|| {
            let adapter = HyphenationCrateAdapter::embedded_en_us().unwrap();
            black_box(adapter);
        })
    });
}

#[cfg(feature = "rust-hyphenation-embedded")]
criterion_group!(
    benches,
    bench_hypher,
    bench_no_hyphen,
    bench_hyphenation_crate
);

#[cfg(not(feature = "rust-hyphenation-embedded"))]
criterion_group!(benches, bench_hypher, bench_no_hyphen);
criterion_main!(benches);
