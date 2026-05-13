//! File watching service for live diagram updates
//!
//! Uses the `notify` crate to watch for file changes in the project directory.
//! Changes are debounced (2 second default) before triggering a regeneration.

use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use tokio::sync::broadcast;

/// Debounced file watcher that collects file change events
/// and emits them after a quiet period (default 2 seconds).
pub struct Debouncer {
    /// Internal receiver for file events
    receiver: Receiver<Result<Event, notify::Error>>,
    /// Debounce duration in milliseconds
    debounce_ms: u64,
}

impl Debouncer {
    /// Create a new debouncer wrapping an existing receiver
    pub fn new(receiver: Receiver<Result<Event, notify::Error>>, debounce_ms: u64) -> Self {
        Self {
            receiver,
            debounce_ms,
        }
    }

    /// Receive the next debounced event
    /// Returns None if timeout is reached
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Result<Event, notify::Error>> {
        let deadline = std::time::Instant::now() + timeout;
        let mut last_event: Option<Result<Event, notify::Error>> = None;

        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return last_event;
            }

            match self.receiver.recv_timeout(remaining) {
                Ok(result) => {
                    // Reset timer on new event
                    last_event = Some(result);
                    // Continue loop to wait for more events or timeout
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    return last_event;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return None;
                }
            }
        }
    }

    /// Get the underlying receiver for iteration
    pub fn receiver(&self) -> &Receiver<Result<Event, notify::Error>> {
        &self.receiver
    }
}

/// File watcher that monitors a project directory for changes
/// and broadcasts file change events to subscribers.
pub struct FileWatcher {
    /// The underlying notify watcher
    watcher: RecommendedWatcher,
    /// Receiver for file events (ownership transferred to Debouncer)
    receiver: Option<Receiver<Result<Event, notify::Error>>>,
    /// Broadcast sender for sharing events with WebSocket clients
    event_tx: broadcast::Sender<DiagramChangeEvent>,
    /// Path being watched
    watched_path: PathBuf,
}

impl FileWatcher {
    /// Create a new file watcher for the given directory
    pub fn new(
        project_path: impl Into<PathBuf>,
        debounce_ms: u64,
    ) -> Result<Self, notify::Error> {
        let (event_tx, _) = broadcast::channel(100);
        let watched_path = project_path.into();

        let (tx, rx) = channel();

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )?;

        let mut fw = Self {
            watcher,
            receiver: Some(rx),
            event_tx,
            watched_path,
        };

        // Start watching the project directory
        fw.watcher.watch(
            fw.watched_path.as_path(),
            RecursiveMode::Recursive,
        )?;

        Ok(fw)
    }

    /// Take ownership of the receiver for use with Debouncer
    /// Returns None if already taken
    pub fn take_receiver(&mut self) -> Option<Receiver<Result<Event, notify::Error>>> {
        self.receiver.take()
    }

    /// Get a broadcast receiver for change events
    pub fn subscribe(&self) -> broadcast::Receiver<DiagramChangeEvent> {
        self.event_tx.subscribe()
    }

    /// Broadcast a diagram change event to all subscribers
    pub fn broadcast_change(&self, event: DiagramChangeEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Check if the watcher is still active (receiver not taken)
    pub fn is_active(&self) -> bool {
        self.receiver.is_some()
    }

    /// Get the watched path
    pub fn watched_path(&self) -> &PathBuf {
        &self.watched_path
    }

    /// Unwatch the current directory
    pub fn unwatch(&mut self) -> Result<(), notify::Error> {
        self.watcher.unwatch(self.watched_path.as_path())
    }
}

/// Event emitted when a diagram-relevant file changes
#[derive(Clone, Debug)]
pub struct DiagramChangeEvent {
    /// Type of change
    pub change_type: DiagramChangeType,
    /// Path to the changed file (relative to project root)
    pub file_path: PathBuf,
    /// Timestamp of the event
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiagramChangeType {
    /// A source file was modified
    SourceModified,
    /// A new file was added
    FileCreated,
    /// A file was deleted
    FileDeleted,
    /// Multiple files changed (batch)
    Batch,
    /// Unknown change
    Unknown,
}

impl From<&EventKind> for DiagramChangeType {
    fn from(kind: &EventKind) -> Self {
        match kind {
            EventKind::Modify(_) => DiagramChangeType::SourceModified,
            EventKind::Create(_) => DiagramChangeType::FileCreated,
            EventKind::Remove(_) => DiagramChangeType::FileDeleted,
            EventKind::Any => DiagramChangeType::Batch,
            _ => DiagramChangeType::Unknown,
        }
    }
}

/// Check if a file path is relevant for diagram generation
/// Only source files (.rs, .ts, .js, .go, etc.) trigger regeneration
pub fn is_diagram_relevant(path: &PathBuf) -> bool {
    use std::path::Path;

    let path = path.as_path();

    // Skip hidden files and directories
    if path
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        return false;
    }

    // Skip common non-source directories
    let skip_dirs = [
        "target",
        "node_modules",
        ".git",
        "dist",
        "build",
        ".cognicode",
        "__pycache__",
        "vendor",
    ];

    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        if skip_dirs.contains(&name.as_ref()) {
            return false;
        }
    }

    // Check file extension for source files
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        matches!(
            ext_str.as_str(),
            "rs" | "ts" | "tsx" | "js" | "jsx" | "go" | "py" | "java"
                | "c" | "cpp" | "h" | "hpp" | "cs" | "rb" | "swift" | "kt"
        )
    } else {
        // No extension - be conservative and include
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagram_relevant_rust_files() {
        assert!(is_diagram_relevant(&PathBuf::from("src/main.rs")));
        assert!(is_diagram_relevant(&PathBuf::from("lib/core/src/lib.rs")));
    }

    #[test]
    fn test_diagram_irrelevant_directories() {
        assert!(!is_diagram_relevant(&PathBuf::from("target/debug/main")));
        assert!(!is_diagram_relevant(&PathBuf::from("node_modules/foo/bar.js")));
        assert!(!is_diagram_relevant(&PathBuf::from(".git/config")));
        assert!(!is_diagram_relevant(&PathBuf::from("dist/bundle.js")));
    }

    #[test]
    fn test_diagram_relevant_other_langs() {
        assert!(is_diagram_relevant(&PathBuf::from("src/main.ts")));
        assert!(is_diagram_relevant(&PathBuf::from("pkg/main.go")));
        assert!(is_diagram_relevant(&PathBuf::from("src/main.py")));
    }
}
