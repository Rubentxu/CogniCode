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

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::sync::mpsc;

/// Start a file watcher for the given workspace root.
/// Returns a receiver that yields batches of changed file paths.
/// The watcher runs in a background thread.
pub fn start_watcher(root: PathBuf) -> mpsc::UnboundedReceiver<Vec<PathBuf>> {
    let (tx, rx) = mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let tx = tx.clone();
        let mut watcher = match notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Only care about file modifications/creations
                    let is_relevant = matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_)
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

        // Keep the watcher alive
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    });

    rx
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
        "rs" | "py" | "pyw" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs"
            | "go" | "java" | "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hxx"
            | "cs" | "tf" | "tfvars" | "hcl" | "yml" | "yaml"
            | "rb" | "php" | "swift"
            | "md" | "mdx" | "txt" | "rst" | "json" | "toml"
    )
}
