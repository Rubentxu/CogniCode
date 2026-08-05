//! Domain port for evidence object discovery and persistence.
//!
//! Evidence objects in CogniCode are first-class knowledge artifacts
//! tracked by the graph. They carry supporting data (logs, traces,
//! screenshots, measurements) that backs up a claim or a decision.

pub use cognicode_core::domain::ports::evidence_store::{EvidenceError, EvidenceKind, EvidenceStore, EvidenceSummary};

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
        _workspace: &str,
        kind: Option<EvidenceKind>,
    ) -> Result<Vec<EvidenceSummary>, EvidenceError> {
        Ok(self
            .evidence
            .iter()
            .filter(|e| kind.map(|k| e.kind == k).unwrap_or(true))
            .cloned()
            .collect())
    }

    fn search_evidence(
        &self,
        _workspace: &str,
        query: &str,
        _limit: usize,
    ) -> Result<Vec<EvidenceSummary>, EvidenceError> {
        let q = query.to_lowercase();
        Ok(self
            .evidence
            .iter()
            .filter(|e| e.title.to_lowercase().contains(&q))
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

    #[tokio::test]
    async fn list_evidence_returns_all_when_no_kind_filter() {
        let s = fixture();
        let all = s.list_evidence("ws-1", None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn list_evidence_filters_by_kind() {
        let s = fixture();
        let traces = s.list_evidence("ws-1", Some(EvidenceKind::Trace)).unwrap();
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].id, "evidence:inv-1-ev-1");
    }

    #[tokio::test]
    async fn search_evidence_matches_title_case_insensitively() {
        let s = fixture();
        let hits = s.search_evidence("ws-1", "REGRESSION", 20).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "evidence:inv-1-ev-2");
    }
}
