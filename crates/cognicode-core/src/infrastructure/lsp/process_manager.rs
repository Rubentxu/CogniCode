use crate::infrastructure::lsp::error::{
    LspProcessError, ProgressCallback, ProgressUpdate, ServerStatus,
};
use crate::infrastructure::lsp::process::LspProcess;
use crate::infrastructure::parser::Language;
use dashmap::DashMap;
use lsp_types::ServerCapabilities;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, warn};

const MAX_CRASHES: u32 = 3;
const CRASH_WINDOW_SECS: u64 = 600;
const IDLE_TIMEOUT_SECS: u64 = 300;
const POLL_INTERVAL_MS: u64 = 500;

struct CrashRecord {
    count: AtomicU32,
    first_crash: StdMutex<Instant>,
}

impl CrashRecord {
    fn new() -> Self {
        Self {
            count: AtomicU32::new(0),
            first_crash: StdMutex::new(Instant::now()),
        }
    }

    fn record_crash(&self) -> bool {
        let mut first = self.first_crash.lock().unwrap();
        if first.elapsed() > Duration::from_secs(CRASH_WINDOW_SECS) {
            self.count.store(1, Ordering::Relaxed);
            *first = Instant::now();
            return false;
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        self.count.load(Ordering::Relaxed) >= MAX_CRASHES
    }

    fn is_limited(&self) -> bool {
        let count = self.count.load(Ordering::Relaxed);
        if count < MAX_CRASHES {
            return false;
        }
        let first = self.first_crash.lock().unwrap();
        first.elapsed() <= Duration::from_secs(CRASH_WINDOW_SECS)
    }
}

pub struct LspProcessManager {
    processes: RwLock<Vec<(Language, Mutex<LspProcess>)>>,
    crash_records: DashMap<Language, CrashRecord>,
    statuses: RwLock<HashMap<Language, ServerStatus>>,
    opened_documents: DashMap<String, Language>,
    workspace_root: std::path::PathBuf,
}

impl LspProcessManager {
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            processes: RwLock::new(Vec::new()),
            crash_records: DashMap::new(),
            statuses: RwLock::new(HashMap::new()),
            opened_documents: DashMap::new(),
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    pub async fn wait_for_ready(
        &self,
        language: Language,
        timeout_secs: u64,
        progress_callback: Option<Box<dyn ProgressCallback>>,
    ) -> Result<ServerStatus, LspProcessError> {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(POLL_INTERVAL_MS);
        let mut spawned = false;

        loop {
            let status = self.get_status(language).await;

            if status.is_ready() {
                return Ok(status);
            }

            if status.is_terminal() {
                let waited = start.elapsed().as_secs();
                return Err(LspProcessError::ServerNotReady {
                    language: language.name().to_string(),
                    status,
                    waited_secs: waited,
                });
            }

            // If status is Starting and we haven't spawned yet, spawn the process
            if matches!(status, ServerStatus::Starting) && !spawned {
                spawned = true;
                // Try to spawn and initialize the process
                match self.get_or_spawn(language).await {
                    Ok(_) => {
                        // Process spawned and initialized, continue to wait for ready
                        let new_status = self.get_status(language).await;
                        if new_status.is_ready() {
                            return Ok(new_status);
                        }
                    }
                    Err(e) => {
                        let _waited = start.elapsed().as_secs();
                        return Err(e);
                    }
                }
            }

            let elapsed = start.elapsed().as_secs();

            if elapsed >= timeout_secs {
                let final_status = self.get_status(language).await;
                return Err(LspProcessError::ServerNotReady {
                    language: language.name().to_string(),
                    status: final_status,
                    waited_secs: elapsed,
                });
            }

            if let Some(ref callback) = progress_callback {
                let progress = if elapsed < 5 {
                    Some(format!("Waiting for {} to start...", language.name()))
                } else if elapsed < 15 {
                    Some(format!("{} still {}...", language.name(), status))
                } else {
                    Some(format!(
                        "{} {} ({}s elapsed)...",
                        language.name(),
                        status,
                        elapsed
                    ))
                };

                callback(ProgressUpdate {
                    message: progress.unwrap_or_else(|| status.to_string()),
                    percentage: None,
                    status: status.clone(),
                });
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    async fn get_status(&self, language: Language) -> ServerStatus {
        let processes = self.processes.read().await;
        if let Some((_, proc)) = processes.iter().find(|(l, _)| *l == language) {
            let proc = proc.lock().await;
            return proc.status().clone();
        }
        ServerStatus::Starting
    }

    pub async fn get_or_spawn(
        &self,
        language: Language,
    ) -> Result<ServerCapabilities, LspProcessError> {
        if self.is_crash_limited(language) {
            let crash_count = self
                .crash_records
                .get(&language)
                .map(|r| r.count.load(Ordering::Relaxed))
                .unwrap_or(0);
            return Err(LspProcessError::ServerCrashed {
                language: language.name().to_string(),
                reason: format!(
                    "Server crashed {} times in the last {}s",
                    crash_count, CRASH_WINDOW_SECS
                ),
                crash_count,
            });
        }

        {
            let processes = self.processes.read().await;
            if let Some((_, proc)) = processes.iter().find(|(l, _)| *l == language) {
                let proc = proc.lock().await;
                if proc.is_ready() {
                    return Ok(proc.capabilities().cloned().unwrap_or_default());
                }
            }
        }

        let mut process = LspProcess::spawn(language, &self.workspace_root).await?;
        {
            let mut statuses = self.statuses.write().await;
            statuses.insert(language, ServerStatus::Starting);
        }
        let caps = process.initialize(&self.workspace_root).await?;
        let status = process.status().clone();

        {
            let mut processes = self.processes.write().await;
            processes.retain(|(l, _)| *l != language);
            processes.push((language, Mutex::new(process)));
        }
        {
            let mut statuses = self.statuses.write().await;
            statuses.insert(language, status);
        }

        Ok(caps)
    }

    pub async fn request(
        &self,
        language: Language,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, LspProcessError> {
        self.get_or_spawn(language).await?;

        let processes = self.processes.read().await;
        if let Some((_, proc)) = processes.iter().find(|(l, _)| *l == language) {
            let mut proc = proc.lock().await;
            return proc.request(method, params).await;
        }

        Err(LspProcessError::NotInitialized)
    }

    pub async fn open_document(&self, language: Language, file_path: &str, content: &str) -> Result<(), LspProcessError> {
        self.get_or_spawn(language).await?;

        if self.opened_documents.contains_key(file_path) {
            debug!("Document {} already open, skipping didOpen", file_path);
            return Ok(());
        }

        let processes = self.processes.read().await;
        if let Some((_, proc)) = processes.iter().find(|(l, _)| *l == language) {
            let mut proc = proc.lock().await;
            let result = proc.open_document(file_path, content).await;
            if result.is_ok() {
                self.opened_documents.insert(file_path.to_string(), language);
            }
            return result;
        }

        Err(LspProcessError::NotInitialized)
    }

    pub fn is_ready(&self, language: Language) -> bool {
        // Check crash limit first
        if self.is_crash_limited(language) {
            return false;
        }

        // We can't easily check process status without async,
        // so we return true if not crash limited
        // The actual readiness is checked in wait_for_ready
        true
    }

    pub fn is_crash_limited(&self, language: Language) -> bool {
        if let Some(record) = self.crash_records.get(&language) {
            return record.is_limited();
        }
        false
    }

    pub async fn record_crash(&self, language: Language) -> bool {
        let result = self
            .crash_records
            .entry(language)
            .or_insert_with(CrashRecord::new)
            .record_crash();

        if result {
            let mut statuses = self.statuses.write().await;
            statuses.insert(
                language,
                ServerStatus::Crashed {
                    reason: "Crash limit exceeded".to_string(),
                },
            );
        }

        result
    }

    pub async fn shutdown_all(&self) {
        let processes = self.processes.read().await;
        for (language, proc) in processes.iter() {
            let mut proc = proc.lock().await;
            if let Err(e) = proc.shutdown().await {
                warn!("Error shutting down {}: {}", language.name(), e);
            }
        }
        debug!("All LSP processes shut down");
    }

    pub async fn evict_idle_processes(&self) {
        let mut processes = self.processes.write().await;
        let before_len = processes.len();
        let mut to_remove = Vec::new();

        for (i, (language, proc)) in processes.iter().enumerate() {
            let proc = proc.lock().await;
            if proc.last_activity().elapsed() > Duration::from_secs(IDLE_TIMEOUT_SECS) {
                debug!(
                    "Evicting idle {} process (idle for {:.0}s)",
                    language.name(),
                    proc.last_activity().elapsed().as_secs()
                );
                to_remove.push(i);
            }
        }

        for i in to_remove.into_iter().rev() {
            processes.remove(i);
        }

        if processes.len() < before_len {
            debug!("Evicted {} idle processes", before_len - processes.len());
        }
    }

    pub async fn active_languages(&self) -> Vec<Language> {
        let processes = self.processes.read().await;
        processes.iter().map(|(l, _)| *l).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_record_within_window() {
        let record = CrashRecord::new();
        assert!(!record.record_crash());
        assert!(!record.record_crash());
        assert!(record.record_crash());
    }

    #[test]
    fn test_crash_record_expired_window() {
        let record = CrashRecord::new();
        *record.first_crash.lock().unwrap() =
            Instant::now() - Duration::from_secs(CRASH_WINDOW_SECS + 1);
        assert!(!record.record_crash());
    }

    #[tokio::test]
    async fn test_process_manager_creation() {
        let manager = LspProcessManager::new(Path::new("/tmp"));
        assert!(manager.active_languages().await.is_empty());
    }

    #[tokio::test]
    async fn test_is_crash_limited() {
        let manager = LspProcessManager::new(Path::new("/tmp"));
        assert!(!manager.is_crash_limited(Language::Rust));

        manager.record_crash(Language::Rust).await;
        manager.record_crash(Language::Rust).await;
        let limited = manager.record_crash(Language::Rust).await;
        assert!(limited);
        assert!(manager.is_crash_limited(Language::Rust));
    }
}
