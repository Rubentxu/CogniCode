//! Domain port for evidence object discovery and persistence.
//!
//! Evidence objects in CogniCode are first-class knowledge artifacts
//! tracked by the graph as `NodeKind::Evidence`. They typically carry
//! supporting data (logs, traces, screenshots, measurements) that
//! backs up a claim or a decision.
//!
//! # Implementation
//!
//! Production adapter reads from the graph (PostgreSQL `graph_nodes`
//! table where `kind = Evidence`). In-memory adapter returns a small
//! fixture for tests and previews.

use crate::error::ExplorerResult;
use serde::{Deserialize, Serialize};

/// Kind of evidence backing an investigation or claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// One evidence summary, sufficient for Spotter hits, inspector
/// panes, evidence-pack views, and knowledge rail listings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceSummary {
    /// Stable id (e.g. `"evidence:inv-1-ev-3"`).
    pub id: String,
    /// Short title describing the evidence (e.g. "Stack trace under load").
    pub title: String,
    /// What kind of evidence this is.
    pub kind: EvidenceKind,
    /// Optional source path or URL.
    pub source_path: Option<String>,
    /// Optional one-line summary.
    pub excerpt: Option<String>,
    /// Confidence in `[0.0, 1.0]`. Forwarded to the Evidence Pack view.
    pub confidence: f32,
}

/// Read-only port for evidence discovery.
pub trait EvidenceStore: Send + Sync {
    /// List evidence for a workspace, optionally filtered by kind.
    ///
    /// Empty `workspace_id` returns the global evidence set.
    fn list_evidence(
        &self,
        workspace_id: &str,
        kind: Option<EvidenceKind>,
    ) -> ExplorerResult<Vec<EvidenceSummary>>;

    /// Full-text search across evidence titles and excerpts.
    fn search_evidence(
        &self,
        workspace_id: &str,
        query: &str,
        limit: usize,
    ) -> ExplorerResult<Vec<EvidenceSummary>>;
}

/// In-memory adapter backed by a Vec — useful for tests and previews.
#[derive(Debug, Default, Clone)]
pub struct InMemoryEvidenceStore {
    evidence: Vec<EvidenceSummary>,
}

impl InMemoryEvidenceStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_evidence(mut self, evidence: Vec<EvidenceSummary>) -> Self {
        self.evidence = evidence;
        self
    }
}

impl EvidenceStore for InMemoryEvidenceStore {
    fn list_evidence(
        &self,
        _workspace_id: &str,
        kind: Option<EvidenceKind>,
    ) -> ExplorerResult<Vec<EvidenceSummary>> {
        Ok(self
            .evidence
            .iter()
            .filter(|e| kind.map(|k| e.kind == k).unwrap_or(true))
            .cloned()
            .collect())
    }

    fn search_evidence(
        &self,
        _workspace_id: &str,
        query: &str,
        limit: usize,
    ) -> ExplorerResult<Vec<EvidenceSummary>> {
        let q = query.to_lowercase();
        Ok(self
            .evidence
            .iter()
            .filter(|e| {
                e.title.to_lowercase().contains(&q)
                    || e.excerpt.as_deref().map(|x| x.to_lowercase().contains(&q)).unwrap_or(false)
            })
            .take(limit)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> InMemoryEvidenceStore {
        InMemoryEvidenceStore::new().with_evidence(vec![
            EvidenceSummary {
                id: "evidence:inv-1-ev-1".into(),
                title: "Stack trace under load".into(),
                kind: EvidenceKind::Trace,
                source_path: Some("/logs/inv-1/trace.json".into()),
                excerpt: Some("Cypher query timeout in semantic_subgraph_executor".into()),
                confidence: 0.92,
            },
            EvidenceSummary {
                id: "evidence:inv-1-ev-2".into(),
                title: "Regression timing measurement".into(),
                kind: EvidenceKind::Measurement,
                source_path: Some("/metrics/inv-1/timing.csv".into()),
                excerpt: Some("p95 query time: 850ms (baseline 120ms)".into()),
                confidence: 0.88,
            },
            EvidenceSummary {
                id: "evidence:inv-2-ev-1".into(),
                title: "External reference: GToolkit paper".into(),
                kind: EvidenceKind::External,
                source_path: Some("https://gtoolkit.com/papers/moldable.pdf".into()),
                excerpt: Some("Background on moldable development".into()),
                confidence: 0.95,
            },
        ])
    }

    #[test]
    fn list_evidence_returns_all_when_no_kind_filter() {
        let s = fixture();
        let all = s.list_evidence("ws-1", None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn list_evidence_filters_by_kind() {
        let s = fixture();
        let traces = s.list_evidence("ws-1", Some(EvidenceKind::Trace)).unwrap();
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].id, "evidence:inv-1-ev-1");
    }

    #[test]
    fn search_evidence_matches_title_case_insensitively() {
        let s = fixture();
        let hits = s.search_evidence("ws-1", "REGRESSION", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "evidence:inv-1-ev-2");
    }

    #[test]
    fn search_evidence_matches_excerpt() {
        let s = fixture();
        let hits = s.search_evidence("ws-1", "moldable", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "evidence:inv-2-ev-1");
    }

    #[test]
    fn search_evidence_respects_limit() {
        let s = fixture();
        let hits = s.search_evidence("ws-1", "stack", 1).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn confidence_values_are_preserved() {
        let s = fixture();
        let all = s.list_evidence("ws-1", None).unwrap();
        assert!(all.iter().all(|e| (0.0..=1.0).contains(&e.confidence)));
    }
}