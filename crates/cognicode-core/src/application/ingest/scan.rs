//! Scan stage — walk the filesystem, hash files, compare against the
//! `scan_manifest` for incremental change detection (ADR-017).
//!
//! Three-step flow:
//! 1. Walk: `ignore::WalkBuilder` + `WalkFilter` to find source files
//! 2. Hash: SHA256 per file (rayon parallel)
//! 3. Diff: mtime-first (skip hash if mtime unchanged), then content
//!    hash against the `scan_manifest`

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ignore::WalkBuilder;
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use crate::application::ingest::types::{ChangeKind, FileChange, FileType};
use crate::infrastructure::parser::LanguageConfig;

/// Walk a directory, returning all source files (code, documents, config).
/// Uses `ignore` crate for `.gitignore` awareness, with a WalkFilter-like
/// approach to skip noise directories.
pub fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkBuilder::new(root)
        .hidden(false)        // don't skip dotfiles
        .git_ignore(true)     // respect .gitignore
        .git_global(true)     // respect global .gitignore
        .git_exclude(true)    // respect .git/info/exclude
        .require_git(false)   // work even if not a git repo
        .filter_entry(|entry| {
            // Skip noise directories
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy();
                if is_noise_dir(&name) {
                    return false;
                }
            }
            true
        })
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .map(|e| e.into_path())
        .collect();
    files.sort();
    files
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

/// Classify a file by its extension into a `FileType` and optional
/// `LanguageConfig` (for code files).
pub fn classify_file(path: &Path) -> (FileType, Option<&'static str>) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let lang = match ext.to_lowercase().as_str() {
        "rs" => Some("rust"),
        "py" | "pyw" => Some("python"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp"),
        "cs" => Some("csharp"),
        "tf" | "tfvars" | "hcl" => Some("hcl"),
        "yml" | "yaml" => Some("yaml"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "swift" => Some("swift"),
        "scala" => Some("scala"),
        "lua" | "luau" => Some("lua"),
        "zig" => Some("zig"),
        "dart" => Some("dart"),
        "groovy" | "gradle" => Some("groovy"),
        "ex" | "exs" => Some("elixir"),
        "erl" | "hrl" => Some("erlang"),
        "hs" => Some("haskell"),
        "jl" => Some("julia"),
        "sh" | "bash" => Some("bash"),
        "ex" | "exs" => Some("elixir"),
        "erl" | "hrl" => Some("erlang"),
        "hs" => Some("haskell"),
        "jl" => Some("julia"),
        "sh" | "bash" => Some("bash"),
        _ => None,
    };
    let file_type = match ext.to_lowercase().as_str() {
        "md" | "mdx" | "qmd" | "txt" | "rst" => FileType::Document,
        "json" | "yaml" | "yml" | "toml" | "ini" => FileType::Config,
        _ if lang.is_some() => FileType::Code,
        _ => FileType::Other,
    };
    (file_type, lang)
}

/// Scan a directory, returning `FileChange`s for files that have changed
/// since the last scan. `previous` is the map of `file_path → content_hash`
/// from the last scan_manifest.
pub fn scan_for_changes(
    root: &Path,
    previous: &std::collections::HashMap<String, ScanEntry>,
) -> Vec<FileChange> {
    let files = walk_files(root);

    // 1. Get mtime for all files (fast — just stat)
    let current_files: Vec<(String, f64, Option<&'static str>)> = {
        let collected: Vec<(String, f64, Option<&'static str>)> = files
            .par_iter()
            .filter_map(|p| {
                let rel = relative_to(root, p);
                let (_, lang) = classify_file(p);
                p.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        let mtime = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64();
                        (rel, mtime, lang)
                    })
            })
            .collect();
        collected
    };

    // 2. Hash only files that are new or whose mtime changed
    let to_hash: Vec<(String, f64, Option<&'static str>)> = current_files
        .iter()
        .filter(|(rel, mtime, _)| {
            previous
                .get(rel.as_str())
                .map(|prev| (prev.mtime - mtime).abs() > 0.001)
                .unwrap_or(true) // new file
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

    for (rel, mtime, lang) in &current_files {
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
        let path = root.join(&rel);

        changes.push(FileChange {
            path,
            kind,
            content_hash: hash,
            mtime: *mtime,
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
                mtime: prev.mtime,
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
    pub mtime: f64,
}

/// Noise directories to skip during the walk.
fn is_noise_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".git"
            | ".svn"
            | ".hg"
            | "__pycache__"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
            | "venv"
            | ".venv"
            | "env"
            | ".env"
            | "coverage"
            | ".next"
            | ".nuxt"
            | ".svelte-kit"
            | ".terraform"
            | "graphify-out"
            | "cognicode-out"
            | ".cognicode"
    )
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
        assert!(!files.iter().any(|p| p.to_string_lossy().contains("node_modules")));
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
        assert_eq!(classify_file(Path::new("foo.rs")), (FileType::Code, Some("rust")));
        assert_eq!(classify_file(Path::new("foo.py")), (FileType::Code, Some("python")));
        assert_eq!(classify_file(Path::new("foo.ts")), (FileType::Code, Some("typescript")));
        assert_eq!(classify_file(Path::new("foo.js")), (FileType::Code, Some("javascript")));
        assert_eq!(classify_file(Path::new("foo.go")), (FileType::Code, Some("go")));
        assert_eq!(classify_file(Path::new("foo.java")), (FileType::Code, Some("java")));
        assert_eq!(classify_file(Path::new("foo.md")), (FileType::Document, None));
        assert_eq!(classify_file(Path::new("foo.json")), (FileType::Config, None));
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
                let mtime = p
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64())
                    .unwrap_or(0.0);
                Some((rel, ScanEntry { content_hash: hash, mtime }))
            })
            .collect();
        let changes = scan_for_changes(dir.path(), &previous);
        assert!(changes.is_empty(), "No changes expected, got: {:?}", changes);
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
            ScanEntry { content_hash: hash1.clone(), mtime: 1000.0 },
        );

        // Modify the file
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(&path, "fn main() { println!(\"changed\"); }").unwrap();

        let changes = scan_for_changes(dir.path(), &previous);
        let main_change = changes.iter().find(|c| c.path.ends_with("main.rs")).expect("main.rs changed");
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
            ScanEntry { content_hash: hash, mtime: 1000.0 },
        );

        // Delete the file
        fs::remove_file(dir.path().join("existing.rs")).unwrap();

        let changes = scan_for_changes(dir.path(), &previous);
        let deleted = changes.iter().find(|c| c.kind == ChangeKind::Deleted);
        assert!(deleted.is_some(), "Should detect deleted file");
    }
}
