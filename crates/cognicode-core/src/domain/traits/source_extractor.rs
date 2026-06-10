//! `SourceExtractor` — port trait for ingesting a source into the
//! generic (multimodal) graph.
//!
//! The trait is intentionally dyn-compatible: concrete extractors
//! (markdown/ADR parser, in the next batches) live behind
//! `Box<dyn SourceExtractor + Send + Sync>` so the ingestion
//! pipeline can fan out to N extractors without generics.
//!
//! ```text
//! SourceExtractor
//!   .extract(SourcePath) -> Result<Vec<ExtractedNode>, SourceExtractorError>
//! ```
//!
//! Every call returns *candidates* (`ExtractedNode` carries a
//! `potential_node` and `potential_edges`). Downstream code is free
//! to filter, deduplicate, or score the candidates before they are
//! upserted into the `GraphRepository`.

use std::path::PathBuf;

use async_trait::async_trait;
use thiserror::Error;

use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode};

/// Where to look for the source bytes / artifacts to extract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePath {
    /// A single file on disk (markdown, ADR, …).
    File(PathBuf),
    /// A directory tree to walk recursively.
    Directory(PathBuf),
    /// A remote URL to fetch and extract.
    Url(String),
}

impl SourcePath {
    /// Returns the path component if this is a `File` or `Directory`.
    pub fn as_path(&self) -> Option<&PathBuf> {
        match self {
            SourcePath::File(p) | SourcePath::Directory(p) => Some(p),
            SourcePath::Url(_) => None,
        }
    }

    /// Returns the URL string if this is a `Url`.
    pub fn as_url(&self) -> Option<&str> {
        match self {
            SourcePath::Url(u) => Some(u),
            _ => None,
        }
    }
}

/// A candidate (node + outgoing edges) produced by an extractor.
///
/// The "potential" naming reflects the contract: an extractor emits
/// candidates, the pipeline validates and persists them. An extractor
/// MAY emit zero edges for a node (e.g. a doc with no links).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedNode {
    /// The candidate node.
    pub potential_node: GraphNode,
    /// The candidate outgoing edges from `potential_node.id`.
    pub potential_edges: Vec<GraphEdge>,
}

impl ExtractedNode {
    /// Constructs a new `ExtractedNode` with no edges.
    pub fn new(node: GraphNode) -> Self {
        Self {
            potential_node: node,
            potential_edges: Vec::new(),
        }
    }

    /// Constructs a new `ExtractedNode` with the given node and edges.
    pub fn with_edges(node: GraphNode, edges: Vec<GraphEdge>) -> Self {
        Self {
            potential_node: node,
            potential_edges: edges,
        }
    }

    /// Appends an edge, returning `self` for chaining.
    pub fn push_edge(mut self, edge: GraphEdge) -> Self {
        self.potential_edges.push(edge);
        self
    }
}

/// Errors a `SourceExtractor` can return.
#[derive(Debug, Error)]
pub enum SourceExtractorError {
    /// The path does not exist on disk.
    #[error("source path does not exist: {0}")]
    NotFound(String),
    /// The path exists but cannot be read (permission, I/O, …).
    #[error("failed to read source {path}: {source}")]
    ReadFailed {
        /// The path that failed to read.
        path: String,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// The source bytes are not valid UTF-8.
    #[error("source {0} is not valid UTF-8")]
    InvalidUtf8(String),
    /// The source is unsupported (wrong extension, wrong URL scheme).
    #[error("unsupported source: {0}")]
    Unsupported(String),
    /// The extractor encountered a fatal internal error.
    #[error("extractor error: {0}")]
    Internal(String),
}

/// Result alias for `SourceExtractor` operations.
pub type SourceExtractorResult<T> = Result<T, SourceExtractorError>;

/// Pluggable ingestion port. Implementations are responsible for
/// turning a `SourcePath` (file / directory / URL) into a stream of
/// `ExtractedNode` candidates.
///
/// The trait is dyn-compatible: use `Box<dyn SourceExtractor + Send + Sync>`.
/// Methods take `&self` and the trait is `Sync`, so multiple tasks can
/// share a single extractor instance.
#[async_trait]
pub trait SourceExtractor: Send + Sync {
    /// Returns a stable, lowercase identifier for the source kinds
    /// this extractor accepts (e.g. `"markdown"`, `"adr"`,
    /// `"rst"`). Used for logging and the MCP `kinds[]` filter.
    fn source_kind(&self) -> &'static str;

    /// Extracts candidate nodes + edges from `source`.
    ///
    /// Contract:
    /// - Returns `Ok(vec![])` for a well-formed but empty source.
    /// - Returns `Err(SourceExtractorError::NotFound)` when the path
    ///   does not exist.
    /// - Returns `Err(SourceExtractorError::Unsupported)` for a path
    ///   the extractor does not handle.
    async fn extract(&self, source: SourcePath) -> SourceExtractorResult<Vec<ExtractedNode>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
    use crate::domain::value_objects::dependency_type::DependencyType;
    use crate::domain::value_objects::edge_kind::EdgeKind;
    use crate::domain::value_objects::node_kind::NodeKind;
    use crate::domain::value_objects::provenance::Provenance;
    use crate::domain::value_objects::symbol_kind::SymbolKind;
    use std::path::PathBuf;

    /// A trivial extractor used to prove the trait is dyn-compatible
    /// (`Box<dyn SourceExtractor + Send + Sync>` compiles + is callable
    /// from a `tokio::spawn`).
    struct MockExtractor;

    #[async_trait]
    impl SourceExtractor for MockExtractor {
        fn source_kind(&self) -> &'static str {
            "mock"
        }

        async fn extract(
            &self,
            _source: SourcePath,
        ) -> SourceExtractorResult<Vec<ExtractedNode>> {
            Ok(Vec::new())
        }
    }

    // ---- T6 RED gate tests ----

    /// The trait must be dyn-compatible (`Box<dyn SourceExtractor>`)
    /// and implementable without generic methods.
    #[test]
    fn source_extractor_trait_is_dyn_compatible() {
        // 1) Static: `MockExtractor` implements the trait.
        let extractor = MockExtractor;
        assert_eq!(extractor.source_kind(), "mock");

        // 2) Dyn: heap-allocated trait object compiles.
        let boxed: Box<dyn SourceExtractor + Send + Sync> = Box::new(MockExtractor);
        assert_eq!(boxed.source_kind(), "mock");

        // 3) Dyn: also works through `Arc<dyn …>` (the typical fan-out shape).
        use std::sync::Arc;
        let arc: Arc<dyn SourceExtractor + Send + Sync> = Arc::new(MockExtractor);
        assert_eq!(arc.source_kind(), "mock");
    }

    /// `ExtractedNode` must carry a `potential_node` and a
    /// `potential_edges` vec. Both accessors and the chainable builder
    /// methods must work.
    #[test]
    fn extracted_node_structure() {
        let node = GraphNode::builder(NodeId::new("doc:foo.md#intro"), NodeKind::Doc)
            .label("intro")
            .source_path("/repo/foo.md")
            .build();
        let edge = GraphEdge::new(
            NodeId::new("doc:foo.md#intro"),
            NodeId::new("src/main.rs:main:1"),
            EdgeKind::Dependency(DependencyType::References),
            Provenance::Inferred,
            0.7,
        )
        .unwrap();

        // `new` starts with no edges.
        let bare = ExtractedNode::new(node.clone());
        assert_eq!(bare.potential_node, node);
        assert!(bare.potential_edges.is_empty());

        // `with_edges` populates the vec.
        let full = ExtractedNode::with_edges(node.clone(), vec![edge.clone()]);
        assert_eq!(full.potential_edges.len(), 1);
        assert_eq!(full.potential_edges[0], edge);

        // `push_edge` appends.
        let pushed = ExtractedNode::new(node.clone()).push_edge(edge.clone());
        assert_eq!(pushed.potential_edges.len(), 1);

        // Field accessors work directly.
        let direct = ExtractedNode {
            potential_node: node.clone(),
            potential_edges: vec![],
        };
        assert_eq!(direct.potential_node, node);
        assert!(direct.potential_edges.is_empty());
    }

    // ---- Additional TDD coverage ----

    #[test]
    fn source_path_accessors() {
        let f = SourcePath::File(PathBuf::from("/a/b/c.md"));
        assert_eq!(f.as_path(), Some(&PathBuf::from("/a/b/c.md")));
        assert_eq!(f.as_url(), None);

        let d = SourcePath::Directory(PathBuf::from("/docs"));
        assert_eq!(d.as_path(), Some(&PathBuf::from("/docs")));
        assert_eq!(d.as_url(), None);

        let u = SourcePath::Url("https://example.com/x.md".to_string());
        assert_eq!(u.as_path(), None);
        assert_eq!(u.as_url(), Some("https://example.com/x.md"));
    }

    #[test]
    fn source_extractor_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        // The trait object type must be Send + Sync so it can be
        // moved into a tokio task and shared across worker threads.
        assert_send_sync::<Box<dyn SourceExtractor + Send + Sync>>();
        assert_send_sync::<std::sync::Arc<dyn SourceExtractor + Send + Sync>>();
    }

    /// Compile-time proof that the `SourceExtractor::extract` method
    /// can be called from a `tokio::spawn`-ed future, which is the
    /// canonical fan-out pattern in the ingestion pipeline.
    #[tokio::test]
    async fn extract_callable_from_tokio_spawn() {
        let arc: std::sync::Arc<dyn SourceExtractor + Send + Sync> =
            std::sync::Arc::new(MockExtractor);
        let handle = tokio::spawn(async move {
            arc.extract(SourcePath::File(PathBuf::from("/dev/null")))
                .await
        });
        let result = handle.await.expect("task did not panic");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn extracted_node_with_realistic_payload() {
        // Multi-edge doc node: cites one symbol, justifies another.
        let node = GraphNode::builder(NodeId::new("doc:adr/0007.md#decision"), NodeKind::Decision)
            .label("ADR-0007: Adopt GraphQL")
            .source_path("/repo/docs/adr/0007.md")
            .property("status", "accepted")
            .property("date", "2026-06-10")
            .build();
        let sym = NodeId::new("src/api/schema.rs:build_schema:10");
        let sym_node = GraphNode::builder(sym.clone(), NodeKind::Symbol(SymbolKind::Function))
            .label("build_schema")
            .source_path("/repo/src/api/schema.rs")
            .build();

        let cites = GraphEdge::new(
            node.id.clone(),
            sym.clone(),
            EdgeKind::Cites,
            Provenance::Extracted,
            0.9,
        )
        .unwrap();
        let justifies = GraphEdge::new(
            node.id.clone(),
            sym.clone(),
            EdgeKind::Justifies,
            Provenance::Inferred,
            0.7,
        )
        .unwrap();

        let extracted = ExtractedNode::new(node.clone())
            .push_edge(cites.clone())
            .push_edge(justifies.clone());

        assert_eq!(extracted.potential_node.label, "ADR-0007: Adopt GraphQL");
        assert_eq!(extracted.potential_node.properties.get("status").unwrap(), "accepted");
        assert_eq!(extracted.potential_edges.len(), 2);
        assert_eq!(extracted.potential_edges[0].kind, EdgeKind::Cites);
        assert_eq!(extracted.potential_edges[1].kind, EdgeKind::Justifies);
        // The symbol node is a separate candidate; extractors emit one
        // `ExtractedNode` per GraphNode.
        let _ = sym_node;
    }
}
