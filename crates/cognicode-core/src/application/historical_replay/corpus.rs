//! Immutable historical corpus (e81 — M13).
//!
//! A corpus is constructed once, validated once, and then read-only. There is
//! no add/remove after construction, so dataset membership cannot drift while an
//! evaluation is running.
//!
//! Duplicate case ids are a **fail-loud** error: silently deduplicating would
//! hide a caller mistake and could quietly change a split's meaning.

use crate::application::historical_replay::case::HistoricalCase;
use crate::application::historical_replay::case::HistoricalCaseId;
use crate::application::portable_execution::content_digest;

/// An immutable, canonically ordered set of historical cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalCorpus {
    /// Sorted by case id (canonical order).
    cases: Vec<HistoricalCase>,
    digest: String,
}

impl HistoricalCorpus {
    /// Build a corpus, rejecting duplicates and empty corpora.
    ///
    /// Cases are sorted by id before digesting, so the digest is invariant to
    /// the caller's insertion order.
    pub fn try_new(mut cases: Vec<HistoricalCase>) -> Result<Self, CorpusError> {
        if cases.is_empty() {
            return Err(CorpusError::EmptyCorpus);
        }
        cases.sort_by(|a, b| a.id().cmp(b.id()));
        for pair in cases.windows(2) {
            if pair[0].id() == pair[1].id() {
                return Err(CorpusError::DuplicateCaseId(pair[0].id().clone()));
            }
        }
        let digest = compute_corpus_digest(&cases);
        Ok(Self { cases, digest })
    }

    /// The cases, in canonical (id-sorted) order.
    pub fn cases(&self) -> &[HistoricalCase] {
        &self.cases
    }

    /// Look up a case by id.
    pub fn get(&self, id: &HistoricalCaseId) -> Option<&HistoricalCase> {
        self.cases
            .binary_search_by(|c| c.id().cmp(id))
            .ok()
            .map(|i| &self.cases[i])
    }

    /// Whether an id is present.
    pub fn contains(&self, id: &HistoricalCaseId) -> bool {
        self.get(id).is_some()
    }

    /// The stable corpus digest (order-invariant).
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Number of cases.
    pub fn len(&self) -> usize {
        self.cases.len()
    }

    /// Whether the corpus has no cases. Always `false` for a constructed
    /// corpus; provided for lint symmetry.
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}

/// Order-invariant digest over the corpus content.
fn compute_corpus_digest(cases: &[HistoricalCase]) -> String {
    let mut canonical = String::new();
    canonical.push_str("historical-corpus.v1\n");
    for case in cases {
        canonical.push_str("case-digest\n");
        canonical.push_str(&case.content_digest());
        canonical.push('\n');
    }
    content_digest(canonical.as_bytes()).as_str().to_string()
}

/// Why a corpus could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusError {
    /// No cases were supplied.
    EmptyCorpus,
    /// The same case id appeared more than once.
    DuplicateCaseId(HistoricalCaseId),
}

impl std::fmt::Display for CorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCorpus => f.write_str("historical corpus must not be empty"),
            Self::DuplicateCaseId(id) => {
                write!(f, "duplicate historical case id in corpus: {id}")
            }
        }
    }
}

impl std::error::Error for CorpusError {}
