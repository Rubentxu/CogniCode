//! Extract stage — parse Changed/New files with tree-sitter using
//! LanguageConfig, streaming results via bounded mpsc (ADR-017, ADR-021).
//!
//! Two execution modes:
//! - `extract_all()`: simple parallel extraction, returns all results.
//!   Good for tests and small projects.
//! - `extract_streaming()`: rayon workers send results through a bounded
//!   tokio mpsc channel. Backpressure prevents OOM on large projects.

use std::path::Path;

use tokio::sync::mpsc;

use crate::application::ingest::extractor::extract_file;
use crate::application::ingest::scan::classify_file;
use crate::application::ingest::types::{ChangeKind, ExtractionResult, FileChange};
use crate::infrastructure::parser::LanguageConfig;

/// Channel capacity for streaming extraction (ADR-021).
const EXTRACT_CHANNEL_CAPACITY: usize = 100;

/// Extract all Changed/New files in parallel (rayon), returning all results.
/// Simple mode — collects everything in memory. Use `extract_streaming`
/// for large projects.
pub fn extract_all(changes: Vec<FileChange>) -> Vec<ExtractionResult> {
    use rayon::prelude::*;
    changes
        .into_par_iter()
        .filter_map(|change| match change.kind {
            ChangeKind::Deleted => None, // skip deletions in extract stage
            ChangeKind::New | ChangeKind::Changed => Some(extract_one(change)),
        })
        .collect()
}

/// Extract Changed/New files with streaming via a bounded tokio mpsc channel.
/// Returns the receiver end. The caller (ingester) consumes results as they
/// arrive. Backpressure via the bounded channel prevents OOM.
pub fn extract_streaming(
    changes: Vec<FileChange>,
) -> mpsc::Receiver<ExtractionResult> {
    let (tx, rx) = mpsc::channel(EXTRACT_CHANNEL_CAPACITY);

    // Spawn a rayon task that extracts and sends through the channel
    rayon::spawn(move || {
        use rayon::prelude::*;
        let _ = changes
            .into_par_iter()
            .filter_map(|change| match change.kind {
                ChangeKind::Deleted => None,
                ChangeKind::New | ChangeKind::Changed => Some(extract_one(change)),
            })
            .try_for_each_with(tx, |tx, result| {
                // blocking_send blocks the rayon worker if the channel is full
                tx.blocking_send(result).map_err(|e| {
                    tracing::debug!("extract_streaming receiver dropped: {e}");
                    e
                })
            });
    });

    rx
}

/// Extract a single file with error isolation (ADR-023).
/// Returns `ExtractionResult::failed` on any error.
fn extract_one(change: FileChange) -> ExtractionResult {
    let path = &change.path;
    let hash = change
        .content_hash
        .as_deref()
        .unwrap_or("")
        .to_string();

    // Read source
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return ExtractionResult::failed(
                path.clone(),
                hash,
                format!("read error: {e}"),
            );
        }
    };

    // Find language config
    let (_, lang_name) = classify_file(path);
    let config = match lang_name.and_then(|name| config_by_name(name)) {
        Some(c) => c,
        None => {
            // Not a supported code file — skip silently (no error, no extraction)
            return ExtractionResult::ok(path.clone(), hash, Vec::new(), Vec::new());
        }
    };

    // Extract
    extract_file(config, path, &source, &hash)
}

/// Look up a `LanguageConfig` by its language name (e.g., "rust", "python").
fn config_by_name(name: &str) -> Option<&'static LanguageConfig> {
    match name {
        "rust" => Some(&crate::infrastructure::parser::language_config::RUST_CONFIG),
        "python" => Some(&crate::infrastructure::parser::language_config::PYTHON_CONFIG),
        "typescript" => Some(
            &crate::infrastructure::parser::language_config::TYPESCRIPT_CONFIG,
        ),
        "javascript" => Some(
            &crate::infrastructure::parser::language_config::JAVASCRIPT_CONFIG,
        ),
        "go" => Some(&crate::infrastructure::parser::language_config::GO_CONFIG),
        "java" => Some(&crate::infrastructure::parser::language_config::JAVA_CONFIG),
        "c" => Some(&crate::infrastructure::parser::language_config::C_CONFIG),
        "cpp" => Some(&crate::infrastructure::parser::language_config::CPP_CONFIG),
        "csharp" => Some(&crate::infrastructure::parser::language_config::CSHARP_CONFIG),
        "hcl" => Some(&crate::infrastructure::parser::language_config::HCL_CONFIG),
        "yaml" => Some(&crate::infrastructure::parser::language_config::YAML_CONFIG),
        _ => None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_all_simple() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("main.rs"),
            "fn main() { println!(\"hi\"); }",
        )
        .unwrap();

        let change = FileChange {
            path: dir.path().join("main.rs"),
            kind: ChangeKind::New,
            content_hash: Some("deadbeef".to_string()),
            mtime: 0.0,
            file_type: crate::application::ingest::types::FileType::Code,
            language: Some("rust"),
        };

        let results = extract_all(vec![change]);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        assert!(results[0].nodes.len() >= 1); // at least the file node
    }

    #[test]
    fn test_extract_all_skips_deleted() {
        let dir = TempDir::new().unwrap();
        let change = FileChange {
            path: dir.path().join("deleted.rs"),
            kind: ChangeKind::Deleted,
            content_hash: None,
            mtime: 0.0,
            file_type: crate::application::ingest::types::FileType::Code,
            language: Some("rust"),
        };

        let results = extract_all(vec![change]);
        assert!(results.is_empty(), "Deleted files should be skipped");
    }

    #[test]
    fn test_extract_all_unsupported_file_skipped() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("data.bin"), "binary stuff").unwrap();

        let change = FileChange {
            path: dir.path().join("data.bin"),
            kind: ChangeKind::New,
            content_hash: Some("abc".to_string()),
            mtime: 0.0,
            file_type: crate::application::ingest::types::FileType::Other,
            language: None,
        };

        let results = extract_all(vec![change]);
        assert_eq!(results.len(), 1, "Should produce a result with no nodes");
        assert!(results[0].is_ok());
        assert!(results[0].nodes.is_empty());
    }

    #[test]
    fn test_extract_one_with_error_isolation() {
        // File that doesn't exist — extract_one should return failed
        let dir = TempDir::new().unwrap();
        let change = FileChange {
            path: dir.path().join("nonexistent.rs"),
            kind: ChangeKind::New,
            content_hash: Some("abc".to_string()),
            mtime: 0.0,
            file_type: crate::application::ingest::types::FileType::Code,
            language: Some("rust"),
        };

        let result = extract_one(change);
        assert!(!result.is_ok());
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_extract_streaming_basic() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.py"), "def foo(): pass").unwrap();
        fs::write(dir.path().join("b.py"), "def bar(): pass").unwrap();

        let changes = vec![
            FileChange {
                path: dir.path().join("a.py"),
                kind: ChangeKind::New,
                content_hash: Some("h1".into()),
                mtime: 0.0,
                file_type: crate::application::ingest::types::FileType::Code,
                language: Some("python"),
            },
            FileChange {
                path: dir.path().join("b.py"),
                kind: ChangeKind::New,
                content_hash: Some("h2".into()),
                mtime: 0.0,
                file_type: crate::application::ingest::types::FileType::Code,
                language: Some("python"),
            },
        ];

        let mut rx = extract_streaming(changes);
        let mut count = 0;
        while let Some(result) = rx.recv().await {
            assert!(result.is_ok());
            count += 1;
        }
        assert_eq!(count, 2);
    }
}
