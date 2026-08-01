//! Hexagonal "driven" ports for the Generic Graph Layer.
//!
//! Hosts the [`GraphRepository`] trait and the [`GraphError`] type
//! that the trait's methods return. Adapters (the in-memory mock,
//! the canonical adapter) implement the trait; domain + service
//! code depend on the trait, not on the concrete adapters.
//!
//! ## Port modules (8 trait modules total)
//!
//! | Module | Trait | Status |
//! |--------|-------|--------|
//! | `graph_error` | `GraphError`, `GraphResult` | existing |
//! | `graph_repository` | `GraphRepository` | existing |
//! | `graph_write_port` | `GraphWritePort` | existing (multimodal) |
//! | `manifest_store` | `ManifestStore` | existing |
//! | `named_view_store` | `NamedViewStore` | existing |
//! | `node_property_reader` | `NodePropertyReader` | existing |
//! | `report_store` | `ReportStore` | existing |
//! | `session_store` | `SessionStore` | existing |
//! | `revision_store` | `RevisionStore` | new (PR1) |
//! | `federation_store` | `FederationStore` | new (PR1) |
//! | `ingest_commit` | `IngestCommit` | new (PR1) |
//! | `quality_store` | `QualityStore` | new (PR2 — relocated from cognicode-explorer) |
//! | `view_spec_store` | `ViewSpecStore` | new (PR2 — relocated from cognicode-explorer) |
//!
//! The `multimodal` feature gates the WRITE/exTRACTION path only.

#[cfg(feature = "postgres")]
pub mod call_graph_store;
#[cfg(all(feature = "postgres", feature = "multimodal"))]
pub mod federation_store;
pub mod graph_error;
pub mod graph_repository;
#[cfg(all(feature = "postgres", feature = "multimodal"))]
pub mod graph_write_port;
#[cfg(feature = "multimodal")]
pub mod ingest_commit;
#[cfg(feature = "postgres")]
pub mod manifest_store;
#[cfg(feature = "postgres")]
pub mod named_view_store;
pub mod node_property_reader;
#[cfg(feature = "postgres")]
pub mod quality_store;
#[cfg(feature = "postgres")]
pub mod report_store;
#[cfg(feature = "postgres")]
pub mod revision_store;
#[cfg(feature = "postgres")]
pub mod session_store;
#[cfg(feature = "postgres")]
pub mod view_spec_store;

#[cfg(feature = "postgres")]
pub use call_graph_store::{CallGraphError, CallGraphStore, PostgresCallGraphStore};
#[cfg(all(feature = "postgres", feature = "multimodal"))]
pub use federation_store::{FederationError, FederationStore, PostgresFederationStore};
pub use graph_error::{GraphError, GraphResult};
pub use graph_repository::{GraphRepository, SearchPage};
#[cfg(all(feature = "postgres", feature = "multimodal"))]
pub use graph_write_port::{GraphWritePort, PostgresGraphWritePort};
#[cfg(feature = "multimodal")]
pub use ingest_commit::{
    CommitError, GraphDelta, IngestCommit, ManifestDelta, PostgresIngestCommit, ReportIntent,
};
#[cfg(feature = "postgres")]
pub use manifest_store::{ManifestError, ManifestStore, PostgresManifestStore};
#[cfg(feature = "postgres")]
pub use named_view_store::{NamedViewError, NamedViewStore, PostgresNamedViewStore};
pub use node_property_reader::NodePropertyReader;
#[cfg(feature = "postgres")]
pub use quality_store::{
    IssueFilter, NewIssue, PostgresQualityStore, QualityError, QualityGateSummary, QualityIssue,
    QualityStore, RuleSummary, UpsertSummary,
};
#[cfg(feature = "postgres")]
pub use report_store::{PostgresReportStore, ReportError, ReportStore};
#[cfg(feature = "postgres")]
pub use revision_store::{PostgresRevisionStore, RevisionError, RevisionStore};
#[cfg(feature = "postgres")]
pub use session_store::{PostgresSessionStore, SessionError, SessionRow, SessionStore};
#[cfg(feature = "postgres")]
pub use view_spec_store::{
    PostgresViewSpecStore, ViewSpecPayload, ViewSpecStore, ViewSpecStoreError,
};
