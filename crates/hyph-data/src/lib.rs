mod io;
mod moby;
mod tsv;
mod wiktextract;
mod wlhamb;

pub use io::{read_records, write_records};
pub use moby::{import_moby, ImportMobyOptions};
pub use tsv::{import_tsv, ImportTsvOptions};
pub use wiktextract::{import_wiktextract, ImportWiktextractOptions, ImportWiktextractReport};
pub use wlhamb::{import_wlhamb, ImportWlhambOptions, ImportWlhambReport};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn imports_tsv_fixture() {
        let dir = std::env::temp_dir().join(format!("hyphlab-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let input = dir.join("toy.tsv");
        let output = dir.join("toy.jsonl");
        fs::write(
            &input,
            "word\thyphenated\tlocale\nhyphenation\thy-phen-a-tion\ten-US\nabout\tabout\ten-US\n",
        )
        .unwrap();

        let count = import_tsv(ImportTsvOptions {
            input: input.clone(),
            output: output.clone(),
            locale: Some("en-US".to_string()),
            source: Some("toy".to_string()),
            license: None,
        })
        .unwrap();

        assert_eq!(count, 2);
        let records = read_records(&output).unwrap();
        assert_eq!(records[0].breaks.as_slice(), &[2, 6, 7]);
        assert!(records[1].breaks.is_empty());
    }

    #[test]
    fn imports_wlhamb_fixture() {
        let dir = std::env::temp_dir().join(format!("hyphlab-wlhamb-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let input = dir.join("sample.wlhamb");
        let output = dir.join("sample.jsonl");
        fs::write(&input, "aa-chen-ský\naak\n").unwrap();

        let count = import_wlhamb(ImportWlhambOptions {
            input,
            output: output.clone(),
            locale: Some("cs".to_string()),
            source: Some("hyph-bench-test".to_string()),
            license: None,
            skip_invalid: false,
        })
        .unwrap();

        assert_eq!(count.records, 2);
        assert_eq!(count.skipped_invalid, 0);
        let records = read_records(&output).unwrap();
        assert_eq!(records[0].word, "aachenský");
        assert_eq!(records[0].breaks.as_slice(), &[2, 6]);
        assert!(records[1].breaks.is_empty());
    }

    #[test]
    fn can_skip_invalid_wlhamb_breaks() {
        let dir =
            std::env::temp_dir().join(format!("hyphlab-wlhamb-skip-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let input = dir.join("sample.wlhamb");
        let output = dir.join("sample.jsonl");
        fs::write(&input, "a-\u{301}\nok\n").unwrap();

        let report = import_wlhamb(ImportWlhambOptions {
            input,
            output: output.clone(),
            locale: Some("und".to_string()),
            source: Some("hyph-bench-test".to_string()),
            license: None,
            skip_invalid: true,
        })
        .unwrap();

        assert_eq!(report.records, 1);
        assert_eq!(report.skipped_invalid, 1);
        let records = read_records(&output).unwrap();
        assert_eq!(records[0].word, "ok");
    }

    #[test]
    fn imports_wiktextract_hyphenation_fields() {
        let dir =
            std::env::temp_dir().join(format!("hyphlab-wiktextract-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let input = dir.join("sample.jsonl");
        let output = dir.join("sample.jsonl");
        fs::write(
            &input,
            "{\"word\":\"hyphenation\",\"lang_code\":\"en\",\"sounds\":[{\"hyphenation\":\"hy-phen-a-tion\"}]}\n\
             {\"word\":\"parts\",\"lang_code\":\"en\",\"hyphenations\":[{\"parts\":[\"pa\",\"rts\"]}]}\n\
             {\"word\":\"plain\",\"lang_code\":\"en\"}\n",
        )
        .unwrap();

        let report = import_wiktextract(ImportWiktextractOptions {
            input,
            output: output.clone(),
            locale: Some("en-US".to_string()),
            filter_lang_code: None,
            source: Some("wiktextract-test".to_string()),
            license: None,
            skip_invalid: false,
        })
        .unwrap();

        assert_eq!(report.records, 2);
        assert_eq!(report.skipped_no_hyphenation, 1);
        let records = read_records(&output).unwrap();
        assert_eq!(records[0].word, "hyphenation");
        assert_eq!(records[0].breaks.as_slice(), &[2, 6, 7]);
        assert_eq!(records[1].word, "parts");
        assert_eq!(records[1].breaks.as_slice(), &[2]);
    }

    #[test]
    fn imports_wiktextract_can_filter_lang_code() {
        let dir = std::env::temp_dir().join(format!(
            "hyphlab-wiktextract-lang-filter-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let input = dir.join("sample-lang.jsonl");
        let output = dir.join("sample-lang-out.jsonl");
        fs::write(
            &input,
            "{\"word\":\"домик\",\"lang_code\":\"ru\",\"hyphenation\":\"до-мик\"}\n\
             {\"word\":\"акмеизъм\",\"lang_code\":\"bg\",\"hyphenation\":\"ак-ме-и-зъм\"}\n",
        )
        .unwrap();

        let report = import_wiktextract(ImportWiktextractOptions {
            input,
            output: output.clone(),
            locale: Some("ru".to_string()),
            filter_lang_code: Some("ru".to_string()),
            source: Some("wiktextract-test".to_string()),
            license: None,
            skip_invalid: false,
        })
        .unwrap();

        assert_eq!(report.records, 1);
        assert_eq!(report.skipped_lang_code, 1);
        let records = read_records(&output).unwrap();
        assert_eq!(records[0].word, "домик");
        assert_eq!(records[0].lang, "ru");
    }
}
