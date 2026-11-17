//! The hand-labelled oracle the extraction is scored against.
//!
//! `testdata/mode_equivalence/oracle/base.txt` labels, by hand, every
//! function and type definition of the base corpus. A scenario's expectation
//! is the base labels, adjusted by the `ORACLE` file of its overlay when the
//! edit adds or removes definitions. Simple and malformed edits keep the
//! definitions the author meant, so a malformed file is scored against what a
//! reader would still see in it.
//!
//! What this scores is the definitions the production detectors consume, not
//! the detectors' judgement: each finding is one definition. Names are
//! compared as a multiset of `path|name|bucket`; lines are ignored so that an
//! edit that only moves code does not need relabelling.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use cognicode_core::application::change_tracking::revision_facts::ProducedRevision;
use cognicode_core::domain::evidence_kernel::fact::FactValue;

use super::harness::Scenario;

pub type Multiset = BTreeMap<String, usize>;

/// What the extractor reported for the buckets the detectors read.
pub fn observed(revision: &ProducedRevision) -> Multiset {
    let mut found = Multiset::new();
    for fact in &revision.facts.facts {
        if fact.predicate.as_str() != "core:defines" {
            continue;
        }
        let FactValue::Text(fqn) = &fact.object else {
            continue;
        };
        let bucket = match fact.provenance.detail.as_deref() {
            Some("kind=Function") => "function",
            Some("kind=Struct" | "kind=Class" | "kind=Enum" | "kind=Trait") => "type",
            _ => continue,
        };
        let Some((rest, _line)) = fqn.rsplit_once(':') else {
            continue;
        };
        let Some((path, name)) = rest.rsplit_once(':') else {
            continue;
        };
        *found.entry(format!("{path}|{name}|{bucket}")).or_default() += 1;
    }
    found
}

fn labels(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The definitions a reader would expect to find in the scenario's head.
pub fn expected(corpus: &Path, scenario: &Scenario) -> Multiset {
    let mut want = Multiset::new();
    for line in labels(&corpus.join("oracle/base.txt")) {
        *want.entry(line).or_default() += 1;
    }
    for line in labels(&scenario.overlay.join("ORACLE")) {
        if let Some(removed) = line.strip_prefix('-') {
            if let Some(count) = want.get_mut(removed) {
                *count -= 1;
                if *count == 0 {
                    want.remove(removed);
                }
            }
        } else if let Some(added) = line.strip_prefix('+') {
            *want.entry(added.to_string()).or_default() += 1;
        }
    }
    want
}

/// How the extraction fares against the oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Score {
    pub true_positives: usize,
    pub false_positives: Vec<String>,
    pub false_negatives: Vec<String>,
}

impl Score {
    /// Share of reported definitions that are real; `None` when none were reported.
    pub fn precision(&self) -> Option<f64> {
        ratio(self.true_positives, self.false_positives.len())
    }

    /// Share of real definitions that were reported; `None` when none exist.
    pub fn recall(&self) -> Option<f64> {
        ratio(self.true_positives, self.false_negatives.len())
    }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(good: usize, bad: usize) -> Option<f64> {
    let total = good + bad;
    (total > 0).then(|| good as f64 / total as f64)
}

pub fn score(want: &Multiset, got: &Multiset) -> Score {
    let mut true_positives = 0;
    let mut false_positives = Vec::new();
    let mut false_negatives = Vec::new();
    for key in want
        .keys()
        .chain(got.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let wanted = want.get(key).copied().unwrap_or(0);
        let found = got.get(key).copied().unwrap_or(0);
        true_positives += wanted.min(found);
        false_positives.extend(std::iter::repeat_n(
            key.clone(),
            found.saturating_sub(wanted),
        ));
        false_negatives.extend(std::iter::repeat_n(
            key.clone(),
            wanted.saturating_sub(found),
        ));
    }
    Score {
        true_positives,
        false_positives,
        false_negatives,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[(&str, usize)]) -> Multiset {
        items.iter().map(|(k, n)| ((*k).to_string(), *n)).collect()
    }

    #[test]
    fn a_match_scores_perfectly() {
        let both = set(&[("a|f|function", 1)]);
        let s = score(&both, &both);
        assert_eq!((s.precision(), s.recall()), (Some(1.0), Some(1.0)));
    }

    #[test]
    fn an_extra_definition_costs_precision_and_a_missing_one_costs_recall() {
        let s = score(
            &set(&[("a|f|function", 1), ("a|T|type", 1)]),
            &set(&[("a|f|function", 2)]),
        );
        assert_eq!(s.true_positives, 1);
        assert_eq!(s.false_positives, vec!["a|f|function".to_string()]);
        assert_eq!(s.false_negatives, vec!["a|T|type".to_string()]);
        assert_eq!((s.precision(), s.recall()), (Some(0.5), Some(0.5)));
    }

    #[test]
    fn nothing_expected_and_nothing_found_has_no_ratio() {
        let s = score(&Multiset::new(), &Multiset::new());
        assert_eq!((s.precision(), s.recall()), (None, None));
    }
}
