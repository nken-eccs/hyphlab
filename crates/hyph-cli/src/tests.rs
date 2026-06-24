use super::*;

#[test]
fn split_counts_keeps_positive_splits_nonempty_when_possible() {
    assert_eq!(split_counts(5, [0.8, 0.1, 0.1], 1.0), [3, 1, 1]);
}

#[test]
fn split_counts_allocates_tiny_inputs_by_largest_ratios() {
    assert_eq!(split_counts(2, [0.8, 0.1, 0.1], 1.0), [1, 1, 0]);
}

#[test]
fn safe_ngram_binary_roundtrip_preserves_model() {
    let options = SafeNgramOptions {
        specs: vec![
            SafeNgramSpec {
                left: 5,
                right: 5,
                bucketed: false,
                family: 0,
            },
            SafeNgramSpec {
                left: 4,
                right: 4,
                bucketed: false,
                family: 0,
            },
            SafeNgramSpec {
                left: 3,
                right: 3,
                bucketed: false,
                family: 0,
            },
        ],
        min_support: 1,
        max_negative: 1,
        min_precision_ppm: None,
        min_wilson_ppm: None,
        cap_vowel_nuclei: false,
        orthographic_veto: false,
        unicode_aware: false,
    };
    let mut rules = U64HashSet::default();
    rules.insert(42);
    rules.insert(7);
    rules.insert(99);
    let model = SafeNgramModelFile::from_parts(
        "safe-ngram-multi-s1-n1".to_string(),
        "en-US".to_string(),
        "fixture".to_string(),
        HyphenationConfig::default(),
        options.clone(),
        rules,
        None,
        U64HashSet::default(),
        3,
    );
    let path = std::env::temp_dir().join(format!(
        "hyphlab-safe-ngram-{}-roundtrip.bin",
        std::process::id()
    ));
    model.save(&path).unwrap();
    let loaded = SafeNgramModelFile::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.method, "safe-ngram-multi-s1-n1");
    assert_eq!(loaded.locale, "en-US");
    assert_eq!(loaded.source, "fixture");
    assert_eq!(loaded.config, HyphenationConfig::default());
    assert_eq!(loaded.options, options);
    assert_eq!(loaded.trained_records, 3);
    assert_eq!(loaded.rules, vec![7, 42, 99]);
}

#[test]
fn unicode_safe_ngram_learns_grapheme_boundaries() {
    let options = parse_safe_ngram_options("safe-ngram-unicode-1x1-s1-p80").unwrap();
    assert!(options.unicode_aware);
    let config = HyphenationConfig {
        left_min: 1,
        right_min: 1,
        min_word_len: 2,
        ..HyphenationConfig::default()
    };
    let record = HyphenationRecord::new("tr:1", "tr", "çağ", SmallVec::from_vec(vec![1]), "test");
    let (rules, trained_records) = learn_safe_ngram_rules(&[record], &config, &options);
    assert_eq!(trained_records, 1);
    assert!(!rules.is_empty());
}

#[test]
fn script_filter_distinguishes_russian_cyrillic_from_mixed_words() {
    assert!(script_filter_matches(
        "перенос",
        ScriptFilterArg::RussianCyrillic
    ));
    assert!(script_filter_matches("нақтылық", ScriptFilterArg::Cyrillic));
    assert!(!script_filter_matches(
        "нақтылық",
        ScriptFilterArg::RussianCyrillic
    ));
    assert!(!script_filter_matches(
        "Gaslieferant",
        ScriptFilterArg::RussianCyrillic
    ));
    assert!(script_filter_matches(
        "Gaslieferant",
        ScriptFilterArg::Latin
    ));
}

#[test]
fn method_train_kind_inference_covers_supported_families() {
    assert_eq!(
        infer_train_kind("safe-ngram-3x3-s1-p80", None).unwrap(),
        TrainKind::SafeNgram
    );
    assert_eq!(
        infer_train_kind("italian-syllable", None).unwrap(),
        TrainKind::ItalianSyllable
    );
    assert_eq!(
        infer_train_kind("trogkanis-elkan-crf", None).unwrap(),
        TrainKind::Crf
    );
    assert_eq!(
        infer_train_kind("my-custom-name", Some("guarded-ngram")).unwrap(),
        TrainKind::SafeNgram
    );
}

#[test]
fn relative_manifest_path_points_from_output_to_model() {
    let base = Path::new("target/hyphlab-manifests/dev-workflow");
    let target = Path::new("target/hyphlab-models/dev-workflow/model.bin");
    assert_eq!(
        relative_path_from(base, target).unwrap(),
        PathBuf::from("../../hyphlab-models/dev-workflow/model.bin")
    );
}

#[test]
fn methods_manifest_accepts_train_blocks() {
    let manifest: MethodsManifest = toml::from_str(
        r#"
[[methods]]
slug = "candidate"
method = "safe-ngram-3x3-s1-p80"
supports = ["en"]

[methods.train]
runtime_method = "safe-ngram-model"
output = "{model_dir}/{slug}.bin"
"#,
    )
    .unwrap();
    let method = &manifest.methods[0];
    assert_eq!(method.slug, "candidate");
    assert_eq!(
        method.train.as_ref().unwrap().runtime_method.as_deref(),
        Some("safe-ngram-model")
    );
}
