//! Evidence store port.
//!
//! Evidence objects in CogniCode are first-class knowledge artifacts
//! tracked by the graph. They carry supporting data (logs, traces,
//! screenshots, measurements) that backs up a claim or a decision.

/// Kind of evidence backing an investigation or claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    /// A log capture (stdout/stderr or test output).
    Log,
    /// A trace capture (flamegraph, stack trace).
    Trace,
    /// A measurement (timing, memory, throughput).
    Measurement,
    /// A reference to an external artifact (screenshot, paper, link).
    External,
}

/// One evidence summary, sufficient for Spotter hits and inspector panes.
#[derive(Debug, Clone)]
pub struct EvidenceSummary {
    /// Stable id (e.g. `"evidence:inv-1-ev-3"`).
    pub id: String,
    /// Short title describing the evidence.
    pub title: String,
    /// What kind of evidence this is.
    pub kind: EvidenceKind,
    /// Optional source path or URL.
    pub source_path: Option<String>,
    /// Optional one-line summary.
    pub excerpt: Option<String>,
    /// Confidence in `[0.0, 1.0]`.
    pub confidence: f32,
}

/// Errors returned by [`EvidenceStore`] methods.
#[derive(Debug, thiserror::Error)]
pub enum EvidenceError {
    #[error("Evidence not found: {0}")]
    NotFound(String),
    #[error("Store error: {0}")]
    Store(String),
}

/// Read-only port for evidence discovery.
pub trait EvidenceStore: Send + Sync {
    /// List evidence for a workspace, optionally filtered by kind.
    fn list_evidence(&self, workspace: &str, kind: Option<EvidenceKind>) -> Result<Vec<EvidenceSummary>, EvidenceError>;

    /// Full-text search across evidence titles and excerpts.
    fn search_evidence(&self, workspace: &str, query: &str, limit: usize) -> Result<Vec<EvidenceSummary>, EvidenceError>;
}
