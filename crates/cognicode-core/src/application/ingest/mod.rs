//! Ingest pipeline — Scan → Extract → PgUpsert → Resolve → Cluster →
//! Analyze → Report → Refresh → Notify (ADR-017).

pub mod controller;
pub mod extract_stage;
pub mod extractor;
pub mod pg_upsert_stage;
pub mod refresh;
pub mod scan;
pub mod service;
pub mod types;

pub use controller::{GraphStats, IngestController, JobState, JobStatus, ScanAccepted};
pub use extract_stage::{extract_all, extract_streaming};
pub use extractor::extract_file;
pub use pg_upsert_stage::{pg_upsert_streaming, PgUpsertStats};
pub use refresh::{refresh_from_pg, RefreshStats};
pub use scan::{classify_file, hash_file, scan_for_changes, walk_files, ScanEntry};
pub use service::run_scan;
pub use types::{
    ChangeKind, ExtractionEdge, ExtractionResult, FailedFile, FileChange, FileType,
    ScanManifestEntry, ScanProgress, ScanResult, ScanStage, TargetRef,
};
