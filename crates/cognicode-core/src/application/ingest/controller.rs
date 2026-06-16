//! IngestController — HTTP surface for the ingest pipeline (ADR-017, ADR-025).
//!
//! Exposes three endpoints:
//! - `POST /api/workspaces/:id/scan` — start async scan, returns 202 + job_id
//! - `GET  /api/jobs/:job_id` — poll job status
//! - `GET  /api/workspaces/:id/graph/stats` — counts + last scan timestamp
//!
//! Jobs run in background tokio tasks. The controller holds an in-memory
//! `HashMap<JobId, JobStatus>`. For production, the status map would
//! persist in PG (ADR-025 Phase 2).
//!
//! Use with axum:
//! ```ignore
//! let controller = IngestController::new(repo, graph_cache);
//! let router = axum::Router::new()
//!     .route("/api/workspaces/:id/scan", post(controller.scan))
//!     .route("/api/jobs/:id", get(controller.job_status))
//!     .route("/api/workspaces/:id/graph/stats", get(controller.graph_stats))
//!     .with_state(controller);
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::application::ingest::service::run_scan;
use crate::application::ingest::types::{ScanProgress, ScanResult};
use crate::infrastructure::graph::graph_cache::GraphCache;
use crate::infrastructure::persistence::PostgresRepository;

/// In-memory controller state. Holds the monotonically-increasing job
/// counter for generating unique job IDs.
#[derive(Clone)]
pub struct IngestController {
    repo: Arc<PostgresRepository>,
    cache: Arc<GraphCache>,
    jobs: Arc<RwLock<HashMap<String, JobStatus>>>,
    /// Resolver from workspace_id to its root path. Set externally
    /// by the caller (e.g. the Explorer's WorkspaceService).
    workspace_resolver: Arc<dyn WorkspaceResolver>,
    /// Monotonic counter for job ID generation.
    job_counter: Arc<AtomicU64>,
}

/// Resolves a workspace_id to its root path on disk.
pub trait WorkspaceResolver: Send + Sync {
    fn resolve(&self, workspace_id: &str) -> Option<PathBuf>;
}

impl std::fmt::Debug for dyn WorkspaceResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WorkspaceResolver(..)")
    }
}

/// A simple in-memory workspace resolver (for tests / standalone mode).
#[derive(Default)]
pub struct StaticWorkspaceResolver {
    paths: std::sync::Mutex<HashMap<String, PathBuf>>,
}

impl StaticWorkspaceResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a workspace: (id, path) mapping.
    pub fn register(&self, id: impl Into<String>, path: PathBuf) {
        self.paths.lock().unwrap().insert(id.into(), path);
    }
}

impl WorkspaceResolver for StaticWorkspaceResolver {
    fn resolve(&self, workspace_id: &str) -> Option<PathBuf> {
        self.paths.lock().unwrap().get(workspace_id).cloned()
    }
}

/// In-memory job status.
#[derive(Debug, Clone, Serialize)]
pub struct JobStatus {
    pub job_id: String,
    pub workspace_id: String,
    pub status: JobState,
    pub progress: Option<ScanProgressPayload>,
    pub result: Option<ScanResultPayload>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

/// Serializable proxy for `ScanProgress` (avoids pulling Deserialize into
/// pipeline types just for the API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgressPayload {
    pub stage: String,
    pub processed: usize,
    pub total: usize,
    pub failed: usize,
}

impl From<&ScanProgress> for ScanProgressPayload {
    fn from(p: &ScanProgress) -> Self {
        Self {
            stage: format!("{:?}", p.stage),
            processed: p.processed,
            total: p.total,
            failed: p.failed,
        }
    }
}

/// Serializable proxy for `ScanResult`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResultPayload {
    pub symbols: usize,
    pub edges: usize,
    pub duration_ms: u64,
    pub failed_files: Vec<FailedFilePayload>,
}

/// Serializable proxy for `FailedFile` (avoids pulling Deserialize into the
/// pipeline type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedFilePayload {
    pub path: String,
    pub error: String,
}

impl From<&crate::application::ingest::types::FailedFile> for FailedFilePayload {
    fn from(f: &crate::application::ingest::types::FailedFile) -> Self {
        Self {
            path: f.path.clone(),
            error: f.error.clone(),
        }
    }
}

impl From<&ScanResult> for ScanResultPayload {
    fn from(r: &ScanResult) -> Self {
        Self {
            symbols: r.symbols,
            edges: r.edges,
            duration_ms: r.duration_ms,
            failed_files: r.failed_files.iter().map(FailedFilePayload::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Completed,
    Failed,
}

/// Response for `POST /scan`.
#[derive(Debug, Serialize)]
pub struct ScanAccepted {
    pub job_id: String,
    pub status: String,
    pub message: String,
}

/// Response for `GET /graph/stats`.
#[derive(Debug, Serialize)]
pub struct GraphStats {
    pub workspace_id: String,
    pub symbol_count: usize,
    pub edge_count: usize,
    pub last_scan_at: Option<String>,
}

impl IngestController {
    /// Create a new controller. The caller must provide a workspace resolver
    /// (e.g. `StaticWorkspaceResolver` for tests, or a wrapper around
    /// `WorkspaceService` for production).
    pub fn new(
        repo: Arc<PostgresRepository>,
        cache: Arc<GraphCache>,
        workspace_resolver: Arc<dyn WorkspaceResolver>,
    ) -> Self {
        Self {
            repo,
            cache,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            workspace_resolver,
            job_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Generate a unique job ID: `scan-{epoch_ms}-{counter}`.
    fn next_job_id(&self) -> String {
        let counter = self.job_counter.fetch_add(1, Ordering::SeqCst);
        let epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("scan-{epoch_ms}-{counter}")
    }

    /// Start an async scan job. Returns immediately with 202 + job_id.
    /// The job runs in a background tokio task.
    pub async fn start_scan(&self, workspace_id: &str) -> Result<ScanAccepted, String> {
        let root = self
            .workspace_resolver
            .resolve(workspace_id)
            .ok_or_else(|| format!("workspace '{workspace_id}' not registered"))?;

        let job_id = self.next_job_id();
        let started_at = chrono::Utc::now().to_rfc3339();

        let status = JobStatus {
            job_id: job_id.clone(),
            workspace_id: workspace_id.to_string(),
            status: JobState::Running,
            progress: None,
            result: None,
            started_at,
            finished_at: None,
        };

        self.jobs.write().await.insert(job_id.clone(), status);

        // Spawn background task
        let repo = self.repo.clone();
        let cache = self.cache.clone();
        let jobs = self.jobs.clone();
        let job_id_bg = job_id.clone();
        let ws_id_bg = workspace_id.to_string();
        tokio::spawn(async move {
            let result = run_scan(&repo, &cache, &ws_id_bg, &root, None).await;
            let mut map = jobs.write().await;
            if let Some(s) = map.get_mut(&job_id_bg) {
                s.result = Some(ScanResultPayload::from(&result));
                s.status = JobState::Completed;
                s.finished_at = Some(chrono::Utc::now().to_rfc3339());
            }
        });

        Ok(ScanAccepted {
            job_id,
            status: "running".to_string(),
            message: "Scan started".to_string(),
        })
    }

    /// Get the current status of a job.
    pub async fn get_job(&self, job_id: &str) -> Option<JobStatus> {
        self.jobs.read().await.get(job_id).cloned()
    }

    /// Get graph stats for a workspace (loads from the in-memory cache).
    pub async fn graph_stats(&self, workspace_id: &str) -> GraphStats {
        let graph = self.cache.get();
        GraphStats {
            workspace_id: workspace_id.to_string(),
            symbol_count: graph.symbol_count(),
            edge_count: graph.edge_count(),
            last_scan_at: None, // TODO: load from scan_manifest
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_static_workspace_resolver() {
        let resolver = StaticWorkspaceResolver::new();
        resolver.register("ws1", PathBuf::from("/tmp/ws1"));
        assert_eq!(resolver.resolve("ws1"), Some(PathBuf::from("/tmp/ws1")));
        assert_eq!(resolver.resolve("nonexistent"), None);
    }

    #[tokio::test]
    async fn test_job_status_lifecycle() {
        let cache = Arc::new(GraphCache::new());
        let resolver = Arc::new(StaticWorkspaceResolver::new());
        let _ = (cache, resolver);
        // The actual scan integration test is in pg_upsert_stage
    }
}
