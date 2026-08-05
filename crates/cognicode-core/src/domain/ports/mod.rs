//! Hexagonal "driven" ports for the Generic Graph Layer.
//!
//! Hosts the [`GraphRepository`] trait and the [`GraphError`] type
//! that the trait's methods return. Adapters (the in-memory mock,
//! the canonical adapter) implement the trait; domain + service
//! code depend on the trait, not on the concrete adapters.
//!
//! ## Port modules (12 active ports + 1 utility = 13 modules)
//!
//! | Module | Trait | Status |
//! |--------|-------|--------|
//! | `graph_error` | `GraphError`, `GraphResult` | utility (not a port) |
//! | `call_graph_projection` | `CallGraphProjectionPort` | new (e29-3) |
//! | `call_graph_store` | `CallGraphStore` | existing |
//! | `federation_store` | `FederationStore` | new (PR1) · cfg `multimodal` |
//! | `graph_repository` | `GraphRepository` | existing |
//! | `ingest_commit_port` | `IngestCommitPort` | new (PR1) · cfg `multimodal` |
//! | `manifest_store` | `ManifestStore` | existing |
//! | `node_property_repository` | `NodePropertyRepository` | renamed (e29-3) |
//! | `quality_store` | `QualityStore` | new (PR2 — relocated from cognicode-explorer) |
//! | `report_store` | `ReportStore` | existing |
//! | `revision_store` | `RevisionStore` | new (PR1) |
//! | `session_store` | `SessionStore` | existing |
//! | `view_spec_store` | `ViewSpecStore` | new (PR2 — relocated from cognicode-explorer) |
//!
//! The `multimodal` feature gates the WRITE/exTRACTION path only;
//! cfg-gated entries (`federation_store`, `ingest_commit_port`) appear
//! only under `--features multimodal`.

pub mod adr_repository;
pub mod call_graph_projection;
pub mod call_graph_store;
pub mod doc_repository;
#[cfg(feature = "multimodal")]
pub mod federation_store;
pub mod evidence_store;
pub mod graph_error;
pub mod graph_repository;
#[cfg(feature = "multimodal")]
pub mod ingest_commit_port;
pub mod manifest_store;
pub mod narrative_store;
pub mod node_property_repository;
pub mod quality_store;
pub mod report_store;
pub mod revision_store;
pub mod session_store;
pub mod view_spec_store;

pub use adr_repository::{AdrError, AdrRepository, AdrStatus, AdrSummary};
pub use call_graph_projection::{
    CallGraphProjectionPort, ExplanationHop, ExplanationView, ProjectionError, SubgraphDirection,
    SubgraphEdge, SubgraphView, project_call_graph,
};
pub use call_graph_store::{CallGraphError, CallGraphStore};
pub use doc_repository::{DocError, DocRepository, DocSummary};
#[cfg(feature = "multimodal")]
pub use federation_store::{FederationError, FederationStore};
pub use evidence_store::{EvidenceError, EvidenceKind, EvidenceStore, EvidenceSummary};
pub use graph_error::{GraphError, GraphResult};
pub use graph_repository::{GraphRepository, SearchPage};
#[cfg(feature = "multimodal")]
pub use ingest_commit_port::{
    CommitError, GraphDelta, IngestCommitPort, ManifestDelta, ReportIntent,
};
pub use manifest_store::{ManifestError, ManifestStore, ScanManifest};
pub use narrative_store::{NarrativeError, NarrativeSnapshot, NarrativeStore};
pub use node_property_repository::NodePropertyRepository;
pub use quality_store::{
    IssueFilter, NewIssue, QualityError, QualityGateSummary, QualityIssue, QualityStore,
    RuleSummary, UpsertSummary,
};
pub use report_store::{ReportError, ReportStore, ReportSummary};
pub use revision_store::{RevisionError, RevisionStore};
pub use session_store::{SessionError, SessionRow, SessionStore};
pub use view_spec_store::{ViewSpecPayload, ViewSpecStore, ViewSpecStoreError};
