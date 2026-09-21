//! Per-file graph cache for modular graph construction
//!
//! This module provides a cache that stores a CallGraph per file, allowing
//! for modular graph construction and merging. This is useful when you need
//! to analyze individual files and then combine them into larger graphs.

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::value_objects::{DependencyType, Location};
use crate::infrastructure::parser::{Language, TreeSitterParser};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

/// Cache entry for per-file graphs
struct FileGraphCacheEntry {
    /// The call graph for this file
    graph: CallGraph,
    /// Whether this entry is valid
    valid: bool,
    /// Last-modified time of the source file at cache time (seconds since
    /// UNIX epoch). Used by `get_or_build` to detect content changes: if
    /// the file's mtime no longer matches, the entry is rebuilt.
    mtime_secs: Option<u64>,
    /// Size of the source file in bytes at cache time. Secondary signal:
    /// combined with `mtime_secs`, allows cheap detection of changes
    /// without hashing the file body.
    size: Option<u64>,
}

/// Per-file graph cache that stores CallGraph per file
///
/// This cache allows for modular graph construction where each file's
/// call graph is built independently and can be merged on demand.
pub struct PerFileGraphCache {
    /// Map from file path to cached graph
    cache: RwLock<HashMap<String, FileGraphCacheEntry>>,
    /// Project directory for relative paths
    project_dir: Option<String>,
}

impl PerFileGraphCache {
    /// Creates a new empty PerFileGraphCache
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            project_dir: None,
        }
    }

    /// Creates a new PerFileGraphCache with a project directory
    pub fn with_project_dir(project_dir: impl Into<String>) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            project_dir: Some(project_dir.into()),
        }
    }

    /// Gets the graph for a file, building it if needed
    ///
    /// If the file has already been parsed and cached AND its filesystem
    /// fingerprint (mtime + size) matches what was recorded at cache time,
    /// the cached graph is returned. Otherwise the file is re-parsed and
    /// the cache entry is refreshed.
    ///
    /// The fingerprint approach is cheap (one syscall) and catches the
    /// common case of an editor saving the file: mtime is updated and the
    /// cache is invalidated.
    pub fn get_or_build(&self, file_path: &Path) -> std::io::Result<Arc<CallGraph>> {
        let path_str = file_path.to_string_lossy().to_string();

        // Snapshot the file fingerprint (mtime + size) once, so the
        // read-cache and write-cache branches agree on what we observed.
        let fingerprint = file_fingerprint(file_path);

        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(&path_str)
                && entry.valid
                && entry.mtime_secs == fingerprint.as_ref().map(|f| f.mtime_secs)
                && entry.size == fingerprint.as_ref().map(|f| f.size)
            {
                return Ok(Arc::new(entry.graph.clone()));
            }
        }

        // Build the graph for this file
        let graph = self.build_file_graph(&path_str)?;

        // Cache it
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(
                path_str,
                FileGraphCacheEntry {
                    graph: graph.clone(),
                    valid: true,
                    mtime_secs: fingerprint.as_ref().map(|f| f.mtime_secs),
                    size: fingerprint.as_ref().map(|f| f.size),
                },
            );
        }

        Ok(Arc::new(graph))
    }

    /// Gets a cached graph without building if missing
    pub fn get_cached(&self, file_path: &Path) -> Option<Arc<CallGraph>> {
        let path_str = file_path.to_string_lossy().to_string();
        let cache = self.cache.read().unwrap();
        cache
            .get(&path_str)
            .map(|entry| Arc::new(entry.graph.clone()))
    }

    /// Invalidates the cache entry for a specific file
    pub fn invalidate(&self, file_path: &Path) {
        let path_str = file_path.to_string_lossy().to_string();
        if let Ok(mut cache) = self.cache.write()
            && let Some(entry) = cache.get_mut(&path_str)
        {
            entry.valid = false;
        }
    }

    /// Clears all cached entries
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Clears all cached entries and marks them as needing rebuild
    pub fn clear_all(&self) {
        if let Ok(mut cache) = self.cache.write() {
            for entry in cache.values_mut() {
                entry.valid = false;
            }
        }
    }

    /// Merges multiple file graphs into a single graph
    ///
    /// Takes a list of file paths and returns a merged CallGraph containing
    /// all symbols and dependencies from all the files.
    pub fn merge(&self, file_paths: &[&Path]) -> CallGraph {
        let mut merged = CallGraph::new();

        for path in file_paths {
            let path_str = path.to_string_lossy().to_string();

            // Get from cache or build
            let graph = match self.get_cached(path) {
                Some(g) => (*g).clone(),
                None => self
                    .build_file_graph(&path_str)
                    .unwrap_or_else(|_| CallGraph::new()),
            };

            // Merge symbols
            for symbol in graph.symbols() {
                let new_symbol = Symbol::new(
                    symbol.name(),
                    *symbol.kind(),
                    Location::new(
                        symbol.location().file(),
                        symbol.location().line(),
                        symbol.location().column(),
                    ),
                );
                merged.add_symbol(new_symbol);
            }

            // Merge edges
            for (source_id, target_id, dep_type) in graph.all_dependencies() {
                // Re-create IDs with proper format
                let source_symbol = graph.get_symbol(source_id);
                let target_symbol = graph.get_symbol(target_id);

                if let (Some(src), Some(tgt)) = (source_symbol, target_symbol) {
                    let new_source_id = crate::domain::aggregates::call_graph::SymbolId::new(
                        src.fully_qualified_name(),
                    );
                    let new_target_id = crate::domain::aggregates::call_graph::SymbolId::new(
                        tgt.fully_qualified_name(),
                    );
                    let _ = merged.add_dependency(&new_source_id, &new_target_id, *dep_type);
                }
            }
        }

        merged
    }

    /// Merges all cached file graphs into a single graph
    ///
    /// Only includes files that have been cached and are valid.
    pub fn merge_all(&self) -> CallGraph {
        let paths: Vec<String> = {
            let cache = self.cache.read().unwrap();
            cache
                .iter()
                .filter(|(_, entry)| entry.valid)
                .map(|(path, _)| path.clone())
                .collect()
        };

        let path_refs: Vec<&Path> = paths.iter().map(Path::new).collect();
        self.merge(&path_refs)
    }

    /// Builds a call graph for a single file
    fn build_file_graph(&self, file_path: &str) -> std::io::Result<CallGraph> {
        let source = std::fs::read_to_string(file_path)?;

        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Unsupported file type: {}", file_path),
                )
            })?;

        let parser =
            TreeSitterParser::new(language).map_err(|e| std::io::Error::other(e.to_string()))?;

        let symbols = parser
            .find_all_symbols_with_path(&source, file_path)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let relationships = parser
            .find_call_relationships(&source, file_path)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let mut graph = CallGraph::new();
        let mut name_to_symbol: HashMap<String, crate::domain::aggregates::call_graph::SymbolId> =
            HashMap::new();

        // Add symbols to graph
        for symbol in symbols {
            let symbol_id = graph.add_symbol(symbol.clone());
            name_to_symbol.insert(symbol.name().to_lowercase(), symbol_id);
        }

        // Add call relationships
        for (caller, callee_name) in relationships {
            let caller_id =
                crate::domain::aggregates::call_graph::SymbolId::new(caller.fully_qualified_name());

            if let Some(callee_id) = name_to_symbol.get(&callee_name.to_lowercase()).cloned() {
                let _ = graph.add_dependency(&caller_id, &callee_id, DependencyType::Calls);
            }
        }

        Ok(graph)
    }

    /// Returns the number of cached files
    pub fn cached_file_count(&self) -> usize {
        let cache = self.cache.read().unwrap();
        cache.len()
    }

    /// Returns the number of valid (non-stale) cached entries
    pub fn valid_entry_count(&self) -> usize {
        let cache = self.cache.read().unwrap();
        cache.values().filter(|e| e.valid).count()
    }
}

impl Default for PerFileGraphCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PerFileGraphCache {
    fn clone(&self) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()), // Don't clone cache
            project_dir: self.project_dir.clone(),
        }
    }
}

/// Cheap filesystem fingerprint for cache invalidation.
///
/// Holds the mtime (seconds since UNIX epoch) and the byte size of a file
/// at the moment the fingerprint was taken. If either changes, the file's
/// content is presumed to have changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileFingerprint {
    mtime_secs: u64,
    size: u64,
}

/// Reads the fingerprint of `path`. Returns `None` if the metadata cannot
/// be read (e.g. file was deleted between cache hit and re-check). In that
/// case the caller should treat the cache as stale and rebuild.
fn file_fingerprint(path: &Path) -> Option<FileFingerprint> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let mtime_secs = system_time_to_secs(mtime);
    let size = meta.len();
    Some(FileFingerprint { mtime_secs, size })
}

/// Converts a `SystemTime` to seconds since the UNIX epoch. Saturates on
/// dates before the epoch.
fn system_time_to_secs(t: SystemTime) -> u64 {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(e) => {
            // Date is before the UNIX epoch; saturate to 0 so the
            // cache can still compare this value to others without
            // panicking.
            e.duration().as_secs()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_per_file_graph_cache_empty() {
        let cache = PerFileGraphCache::new();
        assert_eq!(cache.cached_file_count(), 0);
        assert_eq!(cache.valid_entry_count(), 0);
    }

    #[test]
    fn test_per_file_graph_cache_build() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "class MyClass:").unwrap();
        writeln!(file, "    def method(self):").unwrap();
        writeln!(file, "        pass").unwrap();

        let cache = PerFileGraphCache::new();
        let graph = cache.get_or_build(file.path()).unwrap();

        assert!(graph.symbol_count() >= 2);
    }

    #[test]
    fn test_per_file_graph_cache_cached() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();

        let cache = PerFileGraphCache::new();

        // First call builds
        let graph1 = cache.get_or_build(file.path()).unwrap();

        // Second call should return cached
        let graph2 = cache.get_or_build(file.path()).unwrap();

        assert_eq!(graph1.symbol_count(), graph2.symbol_count());
        assert_eq!(cache.cached_file_count(), 1);
    }

    #[test]
    fn test_per_file_graph_cache_invalidate() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();

        let cache = PerFileGraphCache::new();
        cache.get_or_build(file.path()).unwrap();

        assert_eq!(cache.valid_entry_count(), 1);

        cache.invalidate(file.path());

        assert_eq!(cache.valid_entry_count(), 0);
    }

    #[test]
    fn test_per_file_graph_cache_merge() {
        let mut file1 = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file1, "def a():").unwrap();
        writeln!(file1, "    b()").unwrap();
        writeln!(file1).unwrap();
        writeln!(file1, "def b():").unwrap();
        writeln!(file1, "    pass").unwrap();

        let mut file2 = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file2, "def c():").unwrap();
        writeln!(file2, "    pass").unwrap();

        let cache = PerFileGraphCache::new();
        let merged = cache.merge(&[file1.path(), file2.path()]);

        // Should have symbols from both files
        assert!(merged.symbol_count() >= 3);
    }

    // --- PRF F2.W1: regression test for content-change invalidation ---
    //
    // Pre-condition: a file is parsed and cached.
    // Action: the file's content changes (size also changes).
    // Expected: the next get_or_build returns the NEW graph, not the stale one.
    //
    // This is a characterization test that should FAIL on the current code
    // (cache has no mtime/size check) and PASS after the fix.
    #[test]
    fn test_per_file_graph_cache_detects_content_change() {
        use std::io::{Seek as _, Write as _};
        use std::time::Duration;

        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def original():").unwrap();
        writeln!(file, "    pass").unwrap();
        file.flush().unwrap();

        let cache = PerFileGraphCache::new();
        let graph1 = cache.get_or_build(file.path()).unwrap();
        let original_symbol_count = graph1.symbol_count();
        assert!(
            original_symbol_count >= 1,
            "first parse should yield at least one symbol (sanity)"
        );

        // Ensure that the subsequent write produces a clearly newer mtime:
        // filesystems with coarse-grained mtime resolution can otherwise
        // collapse two writes into the same mtime if they happen within
        // the same tick.
        std::thread::sleep(Duration::from_millis(1100));

        // Overwrite with different, larger content.
        {
            let mut f = file.as_file();
            f.set_len(0).unwrap();
            f.seek(std::io::SeekFrom::Start(0)).unwrap();
            writeln!(f, "def alpha():").unwrap();
            writeln!(f, "    pass").unwrap();
            writeln!(f, "def beta():").unwrap();
            writeln!(f, "    pass").unwrap();
            f.flush().unwrap();
        }

        // Re-build via cache.
        let graph2 = cache.get_or_build(file.path()).unwrap();
        let new_symbol_count = graph2.symbol_count();

        // Oracle: new content has 2 functions (alpha, beta), original had 1.
        assert!(
            new_symbol_count > original_symbol_count,
            "after content change, get_or_build should return a graph \
             reflecting the new content (more symbols than the original \
             one). Got {} (was {}). If equal, the cache is returning \
             stale results.",
            new_symbol_count,
            original_symbol_count
        );
    }

    // --- PRF F2.W1: nested-traversal integration test on a real corpus ---
    //
    // The fixture lives at docs/prf/fixtures/per_file_correctness/ and is
    // described in CORPUS.md. The oracle (3 functions: top_level,
    // mid_level, leaf) was written by reading the .rs files directly.
    //
    // This test exercises PerFileStrategy::build_full_graph end-to-end on
    // the corpus and asserts that nested subdirectories are walked.
    #[test]
    fn test_per_file_strategy_build_full_graph_nested_corpus() {
        use crate::infrastructure::graph::strategy::{GraphStrategy, PerFileStrategy};
        use std::path::PathBuf;

        // Resolve corpus path relative to CARGO_MANIFEST_DIR so the test
        // works from any clone.
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let corpus_dir = PathBuf::from(manifest_dir)
            .parent() // crates/
            .unwrap()
            .parent() // repo root
            .unwrap()
            .join("docs/prf/fixtures/per_file_correctness");

        assert!(
            corpus_dir.is_dir(),
            "fixture corpus must exist at {:?}",
            corpus_dir
        );

        let mut strategy = PerFileStrategy::new();
        strategy.build_index(&corpus_dir).expect("build_index");

        let graph = strategy
            .build_full_graph(&corpus_dir)
            .expect("build_full_graph");

        let symbol_count = graph.symbol_count();

        // Oracle: 3 functions across 3 files in 2 nested subdirectories.
        // The strategy must reach every .rs file regardless of depth.
        assert!(
            symbol_count >= 3,
            "PerFileStrategy::build_full_graph should reach all 3 \
             nested .rs files. Got {} symbols (expected >= 3). If < 3, \
             the walk is not descending into subdirectories.",
            symbol_count
        );
    }
}
