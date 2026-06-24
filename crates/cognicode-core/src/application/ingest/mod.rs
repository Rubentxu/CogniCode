//! Ingest pipeline — Scan → Extract → PgUpsert → Resolve → Cluster →
//! Analyze → Report → Refresh → Notify (ADR-017).

pub mod analyzer;
pub mod cluster;
pub mod controller;
pub mod edge_diff;
pub mod extract_stage;
pub mod extractor;
pub mod pg_upsert_stage;
pub mod refresh;
pub mod report_stage;
pub mod resolve;
pub mod scan;
pub mod service;
pub mod types;
pub mod watcher;

pub use analyzer::{AnalysisSummary, run_analyze};
pub use cluster::run_cluster;
pub use controller::{
    GraphStats, IngestController, JobState, JobStatus, ScanAccepted, StaticWorkspaceResolver,
    WorkspaceResolver,
};
pub use extract_stage::{extract_all, extract_streaming};
pub use extractor::extract_file;
pub use pg_upsert_stage::{PgUpsertStats, pg_upsert_streaming};
pub use refresh::{RefreshStats, refresh_from_pg};
pub use report_stage::run_report;
pub use resolve::{resolve_cross_file_calls, resolve_imports};
pub use scan::{ScanEntry, classify_file, hash_file, scan_for_changes, walk_files};
pub use service::run_scan;
pub use types::{
    ChangeKind, ExtractionEdge, ExtractionResult, FailedFile, FileChange, FileType,
    ScanManifestEntry, ScanProgress, ScanResult, ScanStage, TargetRef, TypeRef, TypeRefContext,
};
