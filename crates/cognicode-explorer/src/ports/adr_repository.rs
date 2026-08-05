//! Domain port for ADR (Architecture Decision Record) discovery.
//!
//! Separated from [`super::search_repository::FuzzySymbolSearch`] per
//! ISP: "find me an ADR by identity / status / workspace" is a distinct
//! concern from full-text symbol search.

pub use cognicode_core::domain::ports::adr_repository::{
    AdrError, AdrRepository, AdrStatus, AdrSummary,
};

/// In-memory adapter backed by a Vec — useful for tests and previews.
#[derive(Debug, Default, Clone)]
pub struct InMemoryAdrRepository {
    adrs: Vec<AdrSummary>,
}

impl InMemoryAdrRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_adrs(mut self, adrs: Vec<AdrSummary>) -> Self {
        self.adrs = adrs;
        self
    }
}

impl AdrRepository for InMemoryAdrRepository {
    fn list_adrs(&self, _workspace: &str) -> Result<Vec<AdrSummary>, AdrError> {
        Ok(self.adrs.clone())
    }

    fn search_adrs(
        &self,
        _workspace: &str,
        query: &str,
        _limit: usize,
    ) -> Result<Vec<AdrSummary>, AdrError> {
        let q = query.to_lowercase();
        Ok(self
            .adrs
            .iter()
            .filter(|a| {
                a.title.to_lowercase().contains(&q)
                    || a.topics.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .cloned()
            .collect())
    }

    fn get_adr(&self, id: &str) -> Result<String, AdrError> {
        self.adrs
            .iter()
            .find(|a| a.id == id)
            .map(|_| format!("# {}\n\nADR content for {}", id, id))
            .ok_or_else(|| AdrError::NotFound(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> InMemoryAdrRepository {
        InMemoryAdrRepository::new().with_adrs(vec![
            AdrSummary {
                id: "ADR-001".into(),
                title: "Knowledge layer ports".into(),
                status: AdrStatus::Accepted,
                date: "2026-07-22".into(),
                topics: vec!["knowledge".into(), "ports".into()],
            },
            AdrSummary {
                id: "ADR-002".into(),
                title: "Diagram artifacts".into(),
                status: AdrStatus::Accepted,
                date: "2026-07-15".into(),
                topics: vec!["diagrams".into()],
            },
            AdrSummary {
                id: "ADR-003".into(),
                title: "Superseded design".into(),
                status: AdrStatus::Superseded,
                date: "2026-06-01".into(),
                topics: vec!["legacy".into()],
            },
        ])
    }

    #[tokio::test]
    async fn list_adrs_returns_all() {
        let r = fixture();
        let all = r.list_adrs("ws-1").unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn search_adrs_matches_title_case_insensitively() {
        let r = fixture();
        let hits = r.search_adrs("ws-1", "KNOWLEDGE", 20).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "ADR-001");
    }

    #[tokio::test]
    async fn search_adrs_matches_topics() {
        let r = fixture();
        let hits = r.search_adrs("ws-1", "diagrams", 20).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "ADR-002");
    }

    #[tokio::test]
    async fn get_adr_returns_content_when_found() {
        let r = fixture();
        let content = r.get_adr("ADR-001").unwrap();
        assert!(content.contains("ADR-001"));
    }

    #[tokio::test]
    async fn get_adr_returns_error_when_not_found() {
        let r = fixture();
        let result = r.get_adr("NONEXISTENT");
        assert!(result.is_err());
    }
}
