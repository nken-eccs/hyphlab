use anyhow::Result;
use hyph_core::{GraphemeIndex, HyphenationConfig, HyphenationRecord};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbiguousPolicy {
    Exclude,
    First,
    Union,
    Intersection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionErrorPolicy {
    Abort,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub words: usize,
    pub skipped_ambiguous: usize,
    #[serde(default)]
    pub skipped_method_errors: usize,
    pub exact_words: usize,
    pub serious_word_errors: usize,
    pub no_break_words: usize,
    pub no_break_exact_words: usize,
    pub tp: u64,
    pub fp: u64,
    pub fn_: u64,
    pub tn: u64,
}

impl Metrics {
    pub fn precision(&self) -> f64 {
        if self.tp + self.fp == 0 {
            1.0
        } else {
            ratio(self.tp, self.tp + self.fp)
        }
    }

    pub fn recall(&self) -> f64 {
        ratio(self.tp, self.tp + self.fn_)
    }

    pub fn f1(&self) -> f64 {
        f_beta(self.precision(), self.recall(), 1.0)
    }

    pub fn f05(&self) -> f64 {
        f_beta(self.precision(), self.recall(), 0.5)
    }

    pub fn exact_accuracy(&self) -> f64 {
        ratio(self.exact_words as u64, self.words as u64)
    }

    pub fn serious_word_error_rate(&self) -> f64 {
        ratio(self.serious_word_errors as u64, self.words as u64)
    }

    pub fn no_break_accuracy(&self) -> f64 {
        ratio(self.no_break_exact_words as u64, self.no_break_words as u64)
    }

    pub fn fp_per_100k_boundaries(&self) -> f64 {
        let total = self.tp + self.fp + self.fn_ + self.tn;
        ratio(self.fp * 100_000, total)
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            words: 0,
            skipped_ambiguous: 0,
            skipped_method_errors: 0,
            exact_words: 0,
            serious_word_errors: 0,
            no_break_words: 0,
            no_break_exact_words: 0,
            tp: 0,
            fp: 0,
            fn_: 0,
            tn: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodReport {
    pub method: String,
    pub metrics: Metrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationReport {
    pub metrics: Metrics,
    pub errors: Vec<WordError>,
    pub method_errors: Vec<MethodError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WordError {
    pub id: String,
    pub word: String,
    pub gold: Vec<GraphemeIndex>,
    pub predicted: Vec<GraphemeIndex>,
    pub false_positives: Vec<GraphemeIndex>,
    pub false_negatives: Vec<GraphemeIndex>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodError {
    pub id: String,
    pub word: String,
    pub error: String,
}

pub fn evaluate_predictions<I, F>(
    records: I,
    config: &HyphenationConfig,
    ambiguous_policy: AmbiguousPolicy,
    mut predict: F,
) -> Result<Metrics>
where
    I: IntoIterator<Item = HyphenationRecord>,
    F: FnMut(&HyphenationRecord, &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()>,
{
    let mut metrics = Metrics::default();
    let mut pred = SmallVec::<[GraphemeIndex; 8]>::new();

    for record in records {
        let Some(gold) = gold_breaks(&record, ambiguous_policy, &mut metrics) else {
            continue;
        };
        pred.clear();
        predict(&record, &mut pred)?;
        update_metrics(&mut metrics, &record, &gold, &pred, config);
    }

    Ok(metrics)
}

pub fn evaluate_predictions_report<I, F>(
    records: I,
    config: &HyphenationConfig,
    ambiguous_policy: AmbiguousPolicy,
    predict: F,
) -> Result<EvaluationReport>
where
    I: IntoIterator<Item = HyphenationRecord>,
    F: FnMut(&HyphenationRecord, &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()>,
{
    evaluate_predictions_report_with_policy(
        records,
        config,
        ambiguous_policy,
        PredictionErrorPolicy::Abort,
        predict,
    )
}

pub fn evaluate_predictions_report_with_policy<I, F>(
    records: I,
    config: &HyphenationConfig,
    ambiguous_policy: AmbiguousPolicy,
    prediction_error_policy: PredictionErrorPolicy,
    mut predict: F,
) -> Result<EvaluationReport>
where
    I: IntoIterator<Item = HyphenationRecord>,
    F: FnMut(&HyphenationRecord, &mut SmallVec<[GraphemeIndex; 8]>) -> Result<()>,
{
    let mut metrics = Metrics::default();
    let mut errors = Vec::new();
    let mut method_errors = Vec::new();
    let mut pred = SmallVec::<[GraphemeIndex; 8]>::new();

    for record in records {
        let Some(gold) = gold_breaks(&record, ambiguous_policy, &mut metrics) else {
            continue;
        };
        pred.clear();
        if let Err(error) = predict(&record, &mut pred) {
            match prediction_error_policy {
                PredictionErrorPolicy::Abort => return Err(error),
                PredictionErrorPolicy::Skip => {
                    metrics.skipped_method_errors += 1;
                    method_errors.push(MethodError {
                        id: record.id,
                        word: record.word,
                        error: error.to_string(),
                    });
                    continue;
                }
            }
        }
        update_metrics(&mut metrics, &record, &gold, &pred, config);

        let gold_set = filtered_set(&gold, &record, config);
        let pred_set = filtered_set(&pred, &record, config);
        if gold_set != pred_set {
            errors.push(WordError {
                id: record.id.clone(),
                word: record.word.clone(),
                gold: gold_set.iter().copied().collect(),
                predicted: pred_set.iter().copied().collect(),
                false_positives: pred_set.difference(&gold_set).copied().collect(),
                false_negatives: gold_set.difference(&pred_set).copied().collect(),
            });
        }
    }

    Ok(EvaluationReport {
        metrics,
        errors,
        method_errors,
    })
}

fn gold_breaks(
    record: &HyphenationRecord,
    policy: AmbiguousPolicy,
    metrics: &mut Metrics,
) -> Option<SmallVec<[GraphemeIndex; 8]>> {
    if !record.ambiguous || record.variants.is_empty() {
        return Some(record.breaks.clone());
    }

    match policy {
        AmbiguousPolicy::Exclude => {
            metrics.skipped_ambiguous += 1;
            None
        }
        AmbiguousPolicy::First => record.variants.first().cloned(),
        AmbiguousPolicy::Union => {
            let mut set = BTreeSet::new();
            for variant in &record.variants {
                set.extend(variant.iter().copied());
            }
            Some(set.into_iter().collect())
        }
        AmbiguousPolicy::Intersection => {
            let mut variants = record.variants.iter();
            let first = variants.next()?;
            let mut set: BTreeSet<_> = first.iter().copied().collect();
            for variant in variants {
                let cur: BTreeSet<_> = variant.iter().copied().collect();
                set = set.intersection(&cur).copied().collect();
            }
            Some(set.into_iter().collect())
        }
    }
}

fn update_metrics(
    metrics: &mut Metrics,
    record: &HyphenationRecord,
    gold: &[GraphemeIndex],
    pred: &[GraphemeIndex],
    config: &HyphenationConfig,
) {
    metrics.words += 1;

    let gold_set = filtered_set(gold, record, config);
    let pred_set = filtered_set(pred, record, config);
    if gold_set.is_empty() {
        metrics.no_break_words += 1;
    }

    if gold_set == pred_set {
        metrics.exact_words += 1;
        if gold_set.is_empty() {
            metrics.no_break_exact_words += 1;
        }
    }

    if pred_set.difference(&gold_set).next().is_some() {
        metrics.serious_word_errors += 1;
    }

    let candidates = candidate_boundaries(record, config);
    for boundary in candidates {
        let g = gold_set.contains(&boundary);
        let p = pred_set.contains(&boundary);
        match (g, p) {
            (true, true) => metrics.tp += 1,
            (false, true) => metrics.fp += 1,
            (true, false) => metrics.fn_ += 1,
            (false, false) => metrics.tn += 1,
        }
    }
}

fn filtered_set(
    breaks: &[GraphemeIndex],
    record: &HyphenationRecord,
    config: &HyphenationConfig,
) -> BTreeSet<GraphemeIndex> {
    let candidates = candidate_boundaries(record, config);
    breaks
        .iter()
        .copied()
        .filter(|b| candidates.contains(b))
        .collect()
}

fn candidate_boundaries(
    record: &HyphenationRecord,
    config: &HyphenationConfig,
) -> BTreeSet<GraphemeIndex> {
    let len = record.grapheme_len();
    if len < config.min_word_len {
        return BTreeSet::new();
    }

    (1..len)
        .filter(|idx| *idx >= config.left_min && len.saturating_sub(*idx) >= config.right_min)
        .map(|idx| idx as GraphemeIndex)
        .collect()
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn f_beta(precision: f64, recall: f64, beta: f64) -> f64 {
    if precision == 0.0 && recall == 0.0 {
        return 0.0;
    }
    let beta2 = beta * beta;
    (1.0 + beta2) * precision * recall / (beta2 * precision + recall)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyph_core::HyphenationRecord;
    use smallvec::smallvec;

    #[test]
    fn counts_boundary_confusion() {
        let record =
            HyphenationRecord::new("toy:1", "en-US", "hyphenation", smallvec![2, 6, 7], "toy");
        let config = HyphenationConfig::default();
        let metrics = evaluate_predictions(
            vec![record],
            &config,
            AmbiguousPolicy::Exclude,
            |_record, out| {
                out.clear();
                out.extend([2, 6, 8]);
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(metrics.tp, 2);
        assert_eq!(metrics.fp, 1);
        assert_eq!(metrics.fn_, 1);
        assert_eq!(metrics.serious_word_errors, 1);
    }

    #[test]
    fn no_positive_predictions_have_perfect_precision() {
        let record =
            HyphenationRecord::new("toy:1", "en-US", "hyphenation", smallvec![2, 6, 7], "toy");
        let metrics = evaluate_predictions(
            vec![record],
            &HyphenationConfig::default(),
            AmbiguousPolicy::Exclude,
            |_record, out| {
                out.clear();
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(metrics.precision(), 1.0);
        assert_eq!(metrics.recall(), 0.0);
    }

    #[test]
    fn no_break_accuracy_uses_filtered_gold_boundaries() {
        let record = HyphenationRecord::new("toy:1", "en-US", "abcdef", smallvec![1], "toy");
        let metrics = evaluate_predictions(
            vec![record],
            &HyphenationConfig::default(),
            AmbiguousPolicy::Exclude,
            |_record, out| {
                out.clear();
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(metrics.no_break_words, 1);
        assert_eq!(metrics.no_break_exact_words, 1);
        assert_eq!(metrics.no_break_accuracy(), 1.0);
    }
}
