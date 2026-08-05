//! Domain port for documentation discovery.
//!
//! Separated from [`super::search_repository::FuzzySymbolSearch`] per
//! ISP: documentation has its own identity, retrieval, and discovery
//! model that doesn't fit symbol-level search.

pub use cognicode_core::domain::ports::doc_repository::{DocError, DocRepository, DocSummary};

/// In-memory adapter backed by a Vec — useful for tests and previews.
#[derive(Debug, Default, Clone)]
pub struct InMemoryDocRepository {
    docs: Vec<DocSummary>,
}

impl InMemoryDocRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_docs(mut self, docs: Vec<DocSummary>) -> Self {
        self.docs = docs;
        self
    }
}

impl DocRepository for InMemoryDocRepository {
    fn list_docs(&self, _workspace: &str, section: Option<&str>) -> Result<Vec<DocSummary>, DocError> {
        Ok(self
            .docs
            .iter()
            .filter(|d| {
                section.map(|s| d.section == s).unwrap_or(true)
            })
            .cloned()
            .collect())
    }

    fn search_docs(&self, _workspace: &str, query: &str, _limit: usize) -> Result<Vec<DocSummary>, DocError> {
        let q = query.to_lowercase();
        Ok(self
            .docs
            .iter()
            .filter(|d| {
                d.title.to_lowercase().contains(&q)
                    || d.section.to_lowercase().contains(&q)
                    || d.excerpt.to_lowercase().contains(&q)
            })
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> InMemoryDocRepository {
        InMemoryDocRepository::new().with_docs(vec![
            DocSummary {
                id: "doc:guide.md".into(),
                title: "Getting Started Guide".into(),
                section: "Introduction".into(),
                source_path: "docs/guide.md".into(),
                excerpt: "A gentle introduction to CogniCode Explorer.".into(),
            },
            DocSummary {
                id: "doc:architecture.md".into(),
                title: "Architecture Overview".into(),
                section: "Architecture".into(),
                source_path: "docs/architecture.md".into(),
                excerpt: "How the workbench shell is structured.".into(),
            },
            DocSummary {
                id: "doc:adr-001.md".into(),
                title: "ADR-001: Knowledge layer ports".into(),
                section: "".into(),
                source_path: "docs/adr/ADR-001.md".into(),
                excerpt: "Ports for docs, ADRs, and evidence.".into(),
            },
        ])
    }

    #[tokio::test]
    async fn list_docs_returns_all_when_no_section_filter() {
        let r = fixture();
        let all = r.list_docs("ws-1", None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn list_docs_filters_by_section() {
        let r = fixture();
        let arch = r.list_docs("ws-1", Some("Architecture")).unwrap();
        assert_eq!(arch.len(), 1);
        assert_eq!(arch[0].id, "doc:architecture.md");
    }

    #[tokio::test]
    async fn search_docs_matches_title_case_insensitively() {
        let r = fixture();
        let hits = r.search_docs("ws-1", "ARCHITECTURE", 20).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "doc:architecture.md");
    }

    #[tokio::test]
    async fn search_docs_matches_excerpt() {
        let r = fixture();
        let hits = r.search_docs("ws-1", "introduction", 20).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "doc:guide.md");
    }
}
