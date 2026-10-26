//! Scan stage — walk the filesystem, hash files, compare against the
//! `scan_manifest` for incremental change detection.
//!
//! Three-step flow:
//! 1. Walk: `ignore::WalkBuilder` + `WalkFilter` to find source files
//! 2. Hash: SHA256 per file (rayon parallel)
//! 3. Diff: mtime-first (skip hash if mtime unchanged), then content
//!    hash against the `scan_manifest`
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ignore::WalkBuilder;
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use crate::application::ingest::types::{ChangeKind, FileChange, FileType};
use crate::domain::value_objects::Language;
use crate::domain::value_objects::file_time::measured_millis;

/// Walk a directory, returning all source files (code, documents, config).
/// Uses `ignore` crate for `.gitignore` awareness, with a WalkFilter-like
/// approach to skip noise directories.
pub fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = workspace_rules(root)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .map(|e| e.into_path())
        .collect();
    files.sort();
    files
}

/// The rules that decide where the product looks inside a workspace.
///
/// The inventory walks with them and the watcher watches with them, so both
/// agree on the same places. The rules are declared once because a second
/// declaration is a second answer: the walk would skip a directory the watch
/// reports, or the other way round, and the difference would only show up as
/// a rebuild nobody asked for.
pub(crate) fn workspace_rules(root: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false) // don't skip dotfiles
        .git_ignore(true) // respect .gitignore
        .git_global(true) // respect global .gitignore
        .git_exclude(true) // respect .git/info/exclude
        .require_git(false) // work even if not a git repo
        .filter_entry(|entry| {
            // Skip noise directories
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy();
                if is_noise_dir(&name) {
                    return false;
                }
            }
            true
        });
    builder
}

/// Compute the SHA256 content hash of a file, returned as a lowercase hex
/// string.
pub fn hash_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute SHA256 for many files in parallel (rayon).
/// Files that fail to read are logged and skipped.
pub fn hash_files_parallel(paths: &[PathBuf]) -> Vec<(PathBuf, Option<String>)> {
    paths
        .par_iter()
        .map(|p| {
            let hash = hash_file(p).ok();
            (p.clone(), hash)
        })
        .collect()
}

/// The extensions that make a file a document.
const DOCUMENT_EXTENSIONS: &[&str] = &["md", "mdx", "qmd", "txt", "rst"];

/// The extensions that make a file a configuration.
const CONFIG_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml", "ini"];

/// Classify a file by its extension into a `FileType` and optional
/// `LanguageConfig` (for code files).
///
/// The language comes from the registry of languages the product can read,
/// which is the only list that says so. An extension no language claims is
/// an extension the product does not read, whatever any other list says.
pub fn classify_file(path: &Path) -> (FileType, Option<&'static str>) {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_lowercase();
    // A JSON file is a configuration and not a source of declarations: the
    // registry can parse it, but nothing in the graph comes from it.
    let language = Language::from_extension(path.extension())
        .filter(|language| *language != Language::Json)
        .map(|language| language.id());
    let file_type = if DOCUMENT_EXTENSIONS.contains(&extension.as_str()) {
        FileType::Document
    } else if CONFIG_EXTENSIONS.contains(&extension.as_str()) {
        FileType::Config
    } else if language.is_some() {
        FileType::Code
    } else {
        FileType::Other
    };
    (file_type, language)
}

/// The extensions of the files the product observes.
///
/// Every consumer that has to name them reads this: the watcher watches
/// them, and the endpoint that announces the watcher publishes them. A
/// second list would be a second answer to the same question, and the two
/// would drift apart without anybody noticing.
pub fn observed_extensions() -> &'static [&'static str] {
    static EXTENSIONS: OnceLock<Vec<&'static str>> = OnceLock::new();
    EXTENSIONS
        .get_or_init(|| {
            let mut extensions: Vec<&'static str> = Language::all_languages()
                .iter()
                .flat_map(|language| language.extensions().iter().copied())
                .chain(DOCUMENT_EXTENSIONS.iter().copied())
                .chain(CONFIG_EXTENSIONS.iter().copied())
                .collect();
            extensions.sort_unstable();
            extensions.dedup();
            extensions
        })
        .as_slice()
}

/// Scan a directory, returning `FileChange`s for files that have changed
/// since the last scan. `previous` is the map of `file_path → content_hash`
/// from the last scan_manifest.
///
/// Every file the walk found is scanned. A file whose date cannot be read is
/// not a file the scan may drop: it comes out with its gap declared — no
/// date — instead of a number that only looks like one.
pub fn scan_for_changes(
    root: &Path,
    previous: &std::collections::HashMap<String, ScanEntry>,
) -> Vec<FileChange> {
    let files = walk_files(root);

    // 1. Measure the date of every file the walk found (fast — just stat).
    //
    // The measurement is not a filter: a file whose date the filesystem does
    // not publish stays in the scan with its date absent. A file the scan
    // does not know about is a file nobody can say anything about, and the
    // gap would disappear with it.
    let current_files: Vec<(String, Option<i64>, Option<&'static str>)> = files
        .par_iter()
        .map(|p| {
            let rel = relative_to(root, p);
            let (_, lang) = classify_file(p);
            let mtime_millis = measured_millis(p.metadata().ok().and_then(|m| m.modified().ok()));
            (rel, mtime_millis, lang)
        })
        .collect();

    // 2. Hash only files that are new or whose date changed.
    //
    // A date nobody measured is not proof of anything: on either side of
    // the comparison the file is hashed again, because the content hash is
    // what decides. The tolerance is one millisecond, the same one the
    // seconds-with-fraction measure allowed before.
    let to_hash: Vec<(String, Option<i64>, Option<&'static str>)> = current_files
        .iter()
        .filter(|(rel, mtime_millis, _)| {
            match previous.get(rel.as_str()).map(|prev| prev.mtime_millis) {
                None => true, // new file
                Some(Some(previous_millis)) => match mtime_millis {
                    Some(millis) => previous_millis.abs_diff(*millis) > 1,
                    None => true, // the date is unknown now: look at the content
                },
                Some(None) => true, // the date was unknown then: look at the content
            }
        })
        .cloned()
        .collect();

    let new_hashes: std::collections::HashMap<String, String> = to_hash
        .iter()
        .filter_map(|(rel, _, _)| {
            let abs = root.join(rel);
            hash_file(&abs).ok().map(|h| (rel.clone(), h))
        })
        .collect();

    // 3. Build FileChange list
    let mut changes: Vec<FileChange> = Vec::new();
    let mut current_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (rel, mtime_millis, lang) in &current_files {
        current_paths.insert(rel.clone());
        let prev = previous.get(rel.as_str());

        let (kind, hash) = match prev {
            None => {
                // New file — must hash
                let hash = new_hashes.get(rel).cloned().unwrap_or_default();
                (ChangeKind::New, Some(hash))
            }
            Some(prev_entry) => {
                // Existing file. If we hashed it and the hash differs, it's
                // Changed. If we didn't hash it (mtime unchanged), the
                // content is assumed unchanged — skip.
                match new_hashes.get(rel) {
                    Some(new_hash) => {
                        if new_hash != &prev_entry.content_hash {
                            (ChangeKind::Changed, Some(new_hash.clone()))
                        } else {
                            // Unchanged — mtime may have changed but content didn't
                            continue;
                        }
                    }
                    None => {
                        // Not hashed (mtime unchanged) — content is unchanged
                        continue;
                    }
                }
            }
        };

        let (file_type, _) = classify_file(&root.join(rel));
        let path = root.join(rel);

        changes.push(FileChange {
            path,
            kind,
            content_hash: hash,
            mtime_millis: *mtime_millis,
            file_type,
            language: *lang,
        });
    }

    // 4. Detect deleted files
    for (rel, prev) in previous {
        if !current_paths.contains(rel) {
            changes.push(FileChange {
                path: root.join(rel),
                kind: ChangeKind::Deleted,
                content_hash: None,
                mtime_millis: prev.mtime_millis,
                file_type: FileType::Other, // unknown
                language: None,
            });
        }
    }

    changes
}

/// Compute relative path from base to path.
fn relative_to(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

/// Lightweight manifest entry for diff comparisons (not the full row).
#[derive(Debug, Clone)]
pub struct ScanEntry {
    pub content_hash: String,
    /// Last modification time in milliseconds since the epoch, signed.
    ///
    /// `None` when the filesystem did not report a time. A file whose clock
    /// nobody read is not a file that did not change: the content hash is
    /// what decides, and the entry says that the date is missing.
    pub mtime_millis: Option<i64>,
}

/// The directories the product does not look into.
///
/// What lives in them is not the project the product describes: build
/// output, caches, dependencies and version control. The names are
/// published so that whoever asks where the watcher looks is told the
/// same thing the watcher decides.
const NOISE_DIRECTORIES: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    ".svn",
    ".hg",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "venv",
    ".venv",
    "env",
    ".env",
    "coverage",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".terraform",
    "graphify-out",
    "cognicode-out",
    ".cognicode",
];

/// The directories the product does not look into.
pub fn noise_directories() -> &'static [&'static str] {
    NOISE_DIRECTORIES
}

/// Whether a directory is one the product does not look into.
pub fn is_noise_dir(name: &str) -> bool {
    NOISE_DIRECTORIES.contains(&name)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_dir() -> TempDir {
        let dir = TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub fn add() {}").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/artifact"), "binary").unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules/pkg.json"), "{}").unwrap();
        dir
    }

    #[test]
    fn test_walk_files_finds_code() {
        let dir = setup_dir();
        let files = walk_files(dir.path());
        assert!(files.iter().any(|p| p.ends_with("main.rs")));
        assert!(files.iter().any(|p| p.ends_with("src/lib.rs")));
        // Noise dirs should be skipped
        assert!(!files.iter().any(|p| p.to_string_lossy().contains("target")));
        assert!(
            !files
                .iter()
                .any(|p| p.to_string_lossy().contains("node_modules"))
        );
    }

    #[test]
    fn test_hash_file_known_value() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("f.txt");
        fs::write(&path, "hello").unwrap();
        // SHA256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let hash = hash_file(&path).unwrap();
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_classify_file() {
        assert_eq!(
            classify_file(Path::new("foo.rs")),
            (FileType::Code, Some("rust"))
        );
        assert_eq!(
            classify_file(Path::new("foo.py")),
            (FileType::Code, Some("python"))
        );
        assert_eq!(
            classify_file(Path::new("foo.ts")),
            (FileType::Code, Some("typescript"))
        );
        assert_eq!(
            classify_file(Path::new("foo.js")),
            (FileType::Code, Some("javascript"))
        );
        assert_eq!(
            classify_file(Path::new("foo.go")),
            (FileType::Code, Some("go"))
        );
        assert_eq!(
            classify_file(Path::new("foo.java")),
            (FileType::Code, Some("java"))
        );
        assert_eq!(
            classify_file(Path::new("foo.md")),
            (FileType::Document, None)
        );
        assert_eq!(
            classify_file(Path::new("foo.json")),
            (FileType::Config, None)
        );
        assert_eq!(classify_file(Path::new("foo.xyz")), (FileType::Other, None));
    }

    #[test]
    fn test_scan_for_changes_detects_new() {
        let dir = setup_dir();
        let previous = std::collections::HashMap::new();
        let changes = scan_for_changes(dir.path(), &previous);
        // All current files should be detected as New
        assert!(changes.iter().all(|c| c.kind == ChangeKind::New));
        assert!(changes.len() >= 2);
    }

    #[test]
    fn test_scan_for_changes_detects_unchanged() {
        let dir = setup_dir();
        // Build a "previous" map with the same hashes
        let previous = walk_files(dir.path())
            .iter()
            .filter_map(|p| {
                let rel = relative_to(dir.path(), p);
                let hash = hash_file(p).ok()?;
                let mtime_millis =
                    measured_millis(p.metadata().ok().and_then(|m| m.modified().ok()));
                Some((
                    rel,
                    ScanEntry {
                        content_hash: hash,
                        mtime_millis,
                    },
                ))
            })
            .collect();
        let changes = scan_for_changes(dir.path(), &previous);
        assert!(
            changes.is_empty(),
            "No changes expected, got: {:?}",
            changes
        );
    }

    #[test]
    fn test_scan_for_changes_detects_modified() {
        let dir = setup_dir();
        let path = dir.path().join("main.rs");
        let hash1 = hash_file(&path).unwrap();

        // Build previous manifest
        let mut previous = std::collections::HashMap::new();
        previous.insert(
            "main.rs".to_string(),
            ScanEntry {
                content_hash: hash1.clone(),
                mtime_millis: Some(1_000),
            },
        );

        // Modify the file
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(&path, "fn main() { println!(\"changed\"); }").unwrap();

        let changes = scan_for_changes(dir.path(), &previous);
        let main_change = changes
            .iter()
            .find(|c| c.path.ends_with("main.rs"))
            .expect("main.rs changed");
        assert_eq!(main_change.kind, ChangeKind::Changed);
    }

    #[test]
    fn test_scan_for_changes_detects_deleted() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("existing.rs"), "// old").unwrap();
        let hash = hash_file(&dir.path().join("existing.rs")).unwrap();
        let mut previous = std::collections::HashMap::new();
        previous.insert(
            "existing.rs".to_string(),
            ScanEntry {
                content_hash: hash,
                mtime_millis: Some(1_000),
            },
        );

        // Delete the file
        fs::remove_file(dir.path().join("existing.rs")).unwrap();

        let changes = scan_for_changes(dir.path(), &previous);
        let deleted = changes.iter().find(|c| c.kind == ChangeKind::Deleted);
        assert!(deleted.is_some(), "Should detect deleted file");
    }
}
