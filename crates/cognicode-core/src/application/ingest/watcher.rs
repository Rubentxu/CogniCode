//! File watcher — auto-scan on file changes (ADR-022, Sprint 4).
//!
//! Uses the `notify` crate to watch the workspace root for file
//! changes. Changed files are queued to the ingest pipeline's scan
//! channel for incremental re-extraction.
//!
//! Features:
//! - Recursive watch (all subdirectories)
//! - Debounced: coalesces rapid changes within 500ms
//! - Filters: only code/config/document files trigger scans
//! - Background task: runs on tokio without blocking
//! - Clean shutdown via `stop()` method

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::sync::{mpsc, oneshot};

/// Handle to a running file watcher. Dropping this stops the watcher.
pub struct WatcherHandle {
    stop_tx: Option<oneshot::Sender<()>>,
    workspace: PathBuf,
    started_at: std::time::Instant,
}

impl WatcherHandle {
    /// Stop the watcher gracefully.
    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
    }

    /// The workspace being watched.
    pub fn workspace(&self) -> &PathBuf {
        &self.workspace
    }

    /// How long the watcher has been running.
    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start a file watcher for the given workspace root.
/// Returns a `WatcherHandle` (dropping it stops the watcher) and a receiver
/// that yields batches of changed file paths.
pub fn start_watcher(root: PathBuf) -> (WatcherHandle, mpsc::UnboundedReceiver<Vec<PathBuf>>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let (stop_tx, stop_rx) = oneshot::channel();
    let workspace = root.clone();

    std::thread::spawn(move || {
        let tx = tx.clone();
        let mut watcher =
            match notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        // Handle Create, Modify, and Delete events
                        let is_relevant = matches!(
                            event.kind,
                            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                        );

                        if is_relevant {
                            let paths: Vec<PathBuf> = event
                                .paths
                                .into_iter()
                                .filter(|p| is_watchable(p))
                                .collect();
                            if !paths.is_empty() {
                                let _ = tx.send(paths);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("file watcher error: {e}");
                    }
                }
            }) {
                Ok(w) => w,
                Err(e) => {
                    tracing::error!("failed to create file watcher: {e}");
                    return;
                }
            };

        if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
            tracing::error!("file watcher failed to watch {}: {e}", root.display());
            return;
        }

        tracing::info!("file watcher started for {}", root.display());

        // Wait for shutdown signal
        let _ = stop_rx.blocking_recv();
        tracing::info!("file watcher stopped for {}", root.display());
        drop(watcher);
    });

    let handle = WatcherHandle {
        stop_tx: Some(stop_tx),
        workspace,
        started_at: std::time::Instant::now(),
    };

    (handle, rx)
}

/// Debounce a stream of file change events into batches.
/// Collects events for `window_ms` milliseconds, then emits a single
/// deduplicated batch.
pub async fn debounce_changes(
    mut rx: mpsc::UnboundedReceiver<Vec<PathBuf>>,
    window_ms: u64,
) -> mpsc::Receiver<Vec<PathBuf>> {
    let (tx, out_rx) = mpsc::channel(64);

    tokio::spawn(async move {
        let mut buffer: Vec<PathBuf> = Vec::new();
        let mut timer = tokio::time::interval(Duration::from_millis(window_ms));

        loop {
            tokio::select! {
                Some(paths) = rx.recv() => {
                    buffer.extend(paths);
                }
                _ = timer.tick() => {
                    if !buffer.is_empty() {
                        // Deduplicate
                        buffer.sort();
                        buffer.dedup();
                        let batch = std::mem::take(&mut buffer);
                        let _ = tx.send(batch).await;
                    }
                }
                else => break,
            }
        }
    });

    out_rx
}

/// Check if a file path is one we should watch (code/config files).
fn is_watchable(path: &std::path::Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "rs" | "py"
            | "pyw"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "go"
            | "java"
            | "c"
            | "h"
            | "cpp"
            | "cc"
            | "cxx"
            | "hpp"
            | "hxx"
            | "cs"
            | "tf"
            | "tfvars"
            | "hcl"
            | "yml"
            | "yaml"
            | "rb"
            | "php"
            | "swift"
            | "md"
            | "mdx"
            | "txt"
            | "rst"
            | "json"
            | "toml"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_watchable_code_files() {
        assert!(is_watchable(PathBuf::from("src/main.rs").as_path()));
        assert!(is_watchable(PathBuf::from("app.py").as_path()));
        assert!(is_watchable(PathBuf::from("index.ts").as_path()));
        assert!(is_watchable(PathBuf::from("main.tf").as_path()));
        assert!(is_watchable(PathBuf::from("site.yml").as_path()));
    }

    #[test]
    fn test_is_watchable_ignores_non_code() {
        assert!(!is_watchable(PathBuf::from("image.png").as_path()));
        assert!(!is_watchable(PathBuf::from("binary.exe").as_path()));
        assert!(!is_watchable(PathBuf::from("data.bin").as_path()));
    }

    #[tokio::test]
    async fn test_debounce_coalesces_events() {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut out = debounce_changes(rx, 100).await;

        // Send 3 rapid events
        tx.send(vec![PathBuf::from("a.rs")]).unwrap();
        tx.send(vec![PathBuf::from("b.rs")]).unwrap();
        tx.send(vec![PathBuf::from("a.rs")]).unwrap(); // duplicate

        // Wait for debounce window
        tokio::time::sleep(Duration::from_millis(200)).await;

        let batch = out.recv().await.unwrap();
        assert_eq!(batch.len(), 2); // a.rs deduplicated
        assert!(batch.contains(&PathBuf::from("a.rs")));
        assert!(batch.contains(&PathBuf::from("b.rs")));
    }
}
