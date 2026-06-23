use anyhow::{Context, Result};
use hyph_core::HyphenationRecord;
use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
};

fn is_zst(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("zst")
}

pub fn read_records(path: impl AsRef<Path>) -> Result<Vec<HyphenationRecord>> {
    let path = path.as_ref();
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader: Box<dyn BufRead> = if is_zst(path) {
        let decoder = zstd::stream::read::Decoder::new(file)
            .with_context(|| format!("open zstd decoder for {}", path.display()))?;
        Box::new(BufReader::new(decoder))
    } else {
        Box::new(BufReader::new(file))
    };

    let mut records = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read line {}", line_no + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str::<HyphenationRecord>(&line)
            .with_context(|| format!("parse JSONL line {}", line_no + 1))?;
        records.push(record);
    }

    Ok(records)
}

pub fn write_records(
    path: impl AsRef<Path>,
    records: impl IntoIterator<Item = HyphenationRecord>,
) -> Result<usize> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory {}", parent.display()))?;
    }

    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut count = 0usize;

    if is_zst(path) {
        let writer = BufWriter::new(file);
        let mut encoder = zstd::stream::write::Encoder::new(writer, 3)
            .with_context(|| format!("open zstd encoder for {}", path.display()))?;
        for record in records {
            serde_json::to_writer(&mut encoder, &record)?;
            encoder.write_all(b"\n")?;
            count += 1;
        }
        encoder.finish()?;
    } else {
        let mut writer = BufWriter::new(file);
        for record in records {
            serde_json::to_writer(&mut writer, &record)?;
            writer.write_all(b"\n")?;
            count += 1;
        }
        writer.flush()?;
    }

    Ok(count)
}
