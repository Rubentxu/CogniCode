//! Ingest pipeline — Scan → Extract → Resolve → Cluster →
//! Analyze → Report (ADR-017).
//!
//! The PG-bound stages (pg_upsert_stage, service run_scan, and the
//! `IngestController` job orchestrator) were removed with the full
//! postgres removal (e29-7). The remaining stages are PG-free; the
//! IngestCommit port (implemented by ladybug) is the persistence seam.

pub mod analyzer;
pub mod edge_diff;
pub mod extract_stage;
pub mod extractor;
pub mod refresh;
pub mod scan;
pub mod types;
pub mod watcher;

#[cfg(feature = "ownership")]
pub mod codeowners;
#[cfg(feature = "ownership")]
pub use codeowners::CodeOwnersMap;
#[cfg(feature = "ownership")]
pub mod blame;
#[cfg(feature = "ownership")]
pub use blame::enrich_with_blame;

pub use analyzer::{AnalysisSummary, run_analyze};
pub use controller::{
    StaticWorkspaceResolver, WorkspaceResolver, workspace_id_for_path,
};
pub mod controller;
pub use extract_stage::{extract_all, extract_streaming};
pub use extractor::extract_file;
pub use refresh::{RefreshStats, refresh_from_pg};
pub use scan::{ScanEntry, classify_file, hash_file, scan_for_changes, walk_files};
pub use types::{
    ChangeKind, ExtractionEdge, ExtractionResult, FailedFile, FileChange, FileType,
    ScanManifestEntry, ScanProgress, ScanResult, ScanStage, TargetRef, TypeRef, TypeRefContext,
};
