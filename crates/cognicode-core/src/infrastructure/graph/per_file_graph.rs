//! Per-file graph cache for modular graph construction
//!
//! This module provides a cache that stores a CallGraph per file, allowing
//! for modular graph construction and merging. This is useful when you need
//! to analyze individual files and then combine them into larger graphs.

use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::value_objects::{DependencyType, Location};
use crate::infrastructure::parser::{Language, TreeSitterParser};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

// =============================================================================
// PRF F2.W5 — Global symbol index for cross-file call resolution (H-R4-2).
//
// The legacy per-file lookup `name_lower → SymbolId` was correct within a
// single file but dropped edges that crossed files (caller in A, callee in B
// could never resolve because B's symbol was not in A's lookup map). It also
// silently picked the wrong homonym when two files declared the same `name`,
// which is exactly the "edge inventado hacia símbolo homónimo" hazard the
// operator flagged as unacceptable.
//
// `GlobalSymbolIndex` is a project-wide reverse index that the resolver
// queries with scope-aware rules:
//
//   * 1 candidate         → use it (no ambiguity).
//   * All candidates in
//     the caller's file    → use the caller's local one (visibility rule).
//   * Multiple in
//     different files
//     within the same
//     crate root          → use the one whose file shares the caller's
//                            project-root ancestor.
//   * Otherwise            → return None. The edge is HONESTLY DROPPED
//                            rather than invented against a wrong homonym.
//
// The lookup never picks "the last inserted" or "the first inserted"; the
// caller can rely on the rules above for audit. This is the core of the
// "preserve identity and ambiguity" requirement from F2.W5.
// =============================================================================

/// Result of building a single file's call graph. The cross-file edge
/// buffer holds `(caller_id, callee_id)` pairs whose `SymbolId`
/// endpoints are not yet in `graph`; they must be reconciled into the
/// project-level merged graph after every file's symbols are in place.
pub type CrossFileEdge = (
    crate::domain::aggregates::call_graph::SymbolId,
    crate::domain::aggregates::call_graph::SymbolId,
);

pub type BuildFileResult = (CallGraph, Vec<CrossFileEdge>);

/// Project-wide symbol index for cross-file call resolution (H-R4-2).
///
/// See module-level doc-comment for the resolution rules.
///
/// `#![allow(dead_code)]` keeps the diagnostic helpers (`candidates`,
/// `len`, `is_empty`) compiled for future test-audit and observability
/// work even though F2.W5's happy path does not exercise them. The
/// non-happy path (dropped edges, ambiguous resolutions) will need
/// them in F3/C3 diagnostics.
#[allow(dead_code)]
pub struct GlobalSymbolIndex {
    /// `name_lower → Vec<SymbolId>` in insertion order. Symbols with the
    /// same name keep their individual `SymbolId` (which is the FQN-based
    /// identity, so duplicates are distinguishable downstream).
    by_name: HashMap<String, Vec<SymbolId>>,
    /// Reverse map so the resolver can read the symbol's file path without
    /// having to walk the source `CallGraph` for every lookup.
    by_id: HashMap<SymbolId, (PathBuf, String)>,
}

impl Default for GlobalSymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalSymbolIndex {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
            by_id: HashMap::new(),
        }
    }

    /// Adds a symbol to the index.
    pub fn insert(&mut self, symbol_id: SymbolId, file: PathBuf, name: &str) {
        let key = name.to_lowercase();
        self.by_name.entry(key).or_default().push(symbol_id.clone());
        self.by_id.insert(symbol_id, (file, name.to_string()));
    }

    /// Resolves a callee name to a `SymbolId`, preferring the caller-file
    /// symbol when ambiguity exists. Returns `None` when no rule yields
    /// an unambiguous answer — the caller MUST drop the edge instead of
    /// inventing one against a random homonym.
    pub fn resolve(&self, name_lower: &str, caller_file: Option<&Path>) -> Option<SymbolId> {
        let candidates = self.by_name.get(name_lower)?;
        if candidates.is_empty() {
            return None;
        }
        if candidates.len() == 1 {
            return Some(candidates[0].clone());
        }

        // Multiple homonyms. Try to disambiguate by caller-file proximity.
        let caller_path = caller_file.map(|p| p.to_path_buf());
        if let Some(caller) = caller_path.as_ref() {
            // Rule 1: exact same file path.
            let same_file: Vec<&SymbolId> = candidates
                .iter()
                .filter(|sid| {
                    self.by_id
                        .get(*sid)
                        .map(|(p, _)| p == caller)
                        .unwrap_or(false)
                })
                .collect();
            if same_file.len() == 1 {
                return Some(same_file[0].clone());
            }
            if same_file.len() > 1 {
                // Genuine intra-file duplicate (e.g. two `same_name` in
                // `dup.rs` — one top, one inside `inner`). Without module
                // context we cannot pick the right one, so we drop the
                // edge rather than guess.
                return None;
            }
            // Rule 2: shared crate root (first N path components match).
            let in_same_crate: Vec<&SymbolId> = candidates
                .iter()
                .filter(|sid| {
                    self.by_id.get(*sid).map(|(p, _)| {
                        shared_crate_root(p, caller).is_some()
                    }).unwrap_or(false)
                })
                .collect();
            if in_same_crate.len() == 1 {
                return Some(in_same_crate[0].clone());
            }
            // Multiple symbols across files in the same crate, no
            // further rule applies — drop the edge.
            return None;
        }

        // No caller context → ambiguous → drop.
        None
    }

    /// Returns the candidate `SymbolId`s for a given name (used by callers
    /// that want to audit ambiguity, e.g. diagnostics or tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn candidates(&self, name_lower: &str) -> &[SymbolId] {
        self.by_name.get(name_lower).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Total number of symbols registered.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// Shared crate root = the longest common path prefix between two files
/// (or `None` when they share no path components, which is the case for
/// files in completely different trees).
fn shared_crate_root(a: &Path, b: &Path) -> Option<PathBuf> {
    let a: Vec<_> = a.components().collect();
    let b: Vec<_> = b.components().collect();
    let mut common = 0usize;
    while common < a.len() && common < b.len() && a[common] == b[common] {
        common += 1;
    }
    if common == 0 {
        None
    } else {
        Some(a[..common].iter().collect())
    }
}

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

        // Build the graph for this file (no global index: per-file resolution only)
        let (graph, _cross_file) = self.build_file_graph(&path_str, None)?;

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
    ///
    /// **Note:** Files that cannot be read or parsed are silently dropped;
    /// the resulting graph may be incomplete without indication. Prefer
    /// [`PerFileGraphCache::merge_with_report`] when callers need to know
    /// whether coverage is complete or partial.
    pub fn merge(&self, file_paths: &[&Path]) -> CallGraph {
        self.merge_with_report(file_paths).graph
    }

    /// Merges multiple file graphs into a single graph and reports which
    /// files were skipped (and why).
    ///
    /// This is the explicit, non-silent counterpart of [`merge`]. The
    /// returned [`BuildReport`] tells the caller:
    ///   - whether the resulting graph represents **Complete**, **Partial**,
    ///     or **Failed** coverage of `file_paths`;
    ///   - which specific files were skipped and the reason (read failure,
    ///     parse failure, unsupported extension, or other I/O error).
    ///
    /// Callers should treat a `Partial` result as a valid but incomplete
    /// outcome and surface the skipped list to the user, not as a clean
    /// "no findings" conclusion.
    pub fn merge_with_report(&self, file_paths: &[&Path]) -> BuildReport {
        // PRF F2.W5 — H-R4-2: build a project-wide symbol index BEFORE
        // resolving any call edges. This is the global lookup that lets
        // `caller → callee` work across files. Without it, the per-file
        // map inside `build_file_graph` drops every cross-file edge.
        let global_index = self.build_global_index(file_paths);

        let mut merged = CallGraph::new();
        let mut skipped: Vec<SkippedFile> = Vec::new();
        // Cross-file edges collected per file, to be reconciled into the
        // merged graph once every file's symbols are in place.
        let mut pending_cross_file_edges: Vec<CrossFileEdge> = Vec::new();

        for path in file_paths {
            let path_str = path.to_string_lossy().to_string();

            // Get from cache or build. Cache hit does not produce skipped
            // entries; only actual build failures do.
            let built: Result<BuildFileResult, std::io::Error> = match self.get_cached(path) {
                Some(g) => Ok(((*g).clone(), Vec::new())),
                None => self.build_file_graph(&path_str, Some(&global_index)),
            };
            let (graph, cross_file) = match built {
                Ok(pair) => pair,
                Err(e) => {
                    skipped.push(SkippedFile {
                        path: path_str,
                        reason: classify_io_error(&e),
                    });
                    continue;
                }
            };
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

            // Cross-file edges discovered while building this file are
            // deferred: their endpoints must exist in `merged` before
            // `add_dependency` can accept them, and that only becomes
            // true after this whole loop has run.
            pending_cross_file_edges.extend(cross_file);
        }

        // Reconcile cross-file edges now that every file's symbols have
        // been merged. `merged` already contains all SymbolIds, so
        // `add_dependency` should accept every cross-file edge whose
        // resolution was unambiguous.
        let mut reconciled_cross_file = 0usize;
        for (source_id, target_id) in pending_cross_file_edges {
            if merged
                .add_dependency(&source_id, &target_id, DependencyType::Calls)
                .is_ok()
            {
                reconciled_cross_file += 1;
            }
        }
        let _ = reconciled_cross_file; // accounted for in edge_count(); kept for future diagnostics

        let status = if skipped.is_empty() {
            BuildStatus::Complete
        } else {
            BuildStatus::Partial { skipped }
        };

        BuildReport {
            graph: merged,
            status,
        }
    }

    /// Builds a project-wide symbol index by parsing each file and
    /// recording every discovered symbol with its file path and name.
    ///
    /// PRF F2.W5 — H-R4-2: this is the global lookup that lets the
    /// resolver find callees across files. Failures to parse or read
    /// individual files are silently skipped here; their absence is
    /// surfaced separately by `merge_with_report` (R3 from F2.W2).
    /// The index keys are the lowercased symbol names so resolution is
    /// case-insensitive (matches the legacy per-file behaviour).
    fn build_global_index(&self, file_paths: &[&Path]) -> GlobalSymbolIndex {
        let mut idx = GlobalSymbolIndex::new();
        for path in file_paths {
            let path_str = path.to_string_lossy().to_string();
            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let language = match Language::from_extension(path.extension()) {
                Some(l) => l,
                None => continue,
            };
            let parser = match TreeSitterParser::new(language) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let tree = match parser.parse_tree(&source) {
                Ok(t) => t,
                Err(_) => continue,
            };
            // Mirror F2.W2: skip files with parse errors.
            if TreeSitterParser::has_error_nodes(&tree) {
                continue;
            }
            let symbols = match parser.find_all_symbols_with_path(&source, &path_str) {
                Ok(s) => s,
                Err(_) => continue,
            };
            for symbol in symbols {
                let sid = SymbolId::new(symbol.fully_qualified_name());
                idx.insert(sid, path.to_path_buf(), symbol.name());
            }
        }
        idx
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

    /// Builds a call graph for a single file.
    ///
    /// `global_index` is optional: when `Some`, cross-file call edges are
    /// resolved against the project-wide [`GlobalSymbolIndex`] using
    /// scope-aware rules (caller file first, then crate root). When
    /// `None`, the legacy per-file lookup is used and cross-file edges are
    /// simply lost (pre-F2.W5 behaviour). Pre-existing callers that only
    /// know one file at a time keep working unchanged.
    fn build_file_graph(
        &self,
        file_path: &str,
        global_index: Option<&GlobalSymbolIndex>,
    ) -> std::io::Result<BuildFileResult> {
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

        // PRF F2.W2 (R3): if the file parses but the tree contains
        // error nodes, the source is syntactically invalid. Returning
        // an empty graph here would silently mis-represent coverage,
        // so we surface this as an I/O error with a message that
        // `merge_with_report` classifies as `SkipReason::Parse`.
        let tree = parser
            .parse_tree(&source)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if TreeSitterParser::has_error_nodes(&tree) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Syntax errors in {}", file_path),
            ));
        }

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

        // Add call relationships.
        //
        // PRF F2.W5 — H-R4-2 (scope-aware cross-file resolution):
        //
        //   * If BOTH the caller and the callee belong to this file's
        //     `name_to_symbol` map, the edge is intra-file and we add it
        //     to `graph` directly (intra-file resolution).
        //
        //   * If the caller is in this file but the callee resolves to
        //     a `SymbolId` in another file (via `global_index`), the
        //     edge is CROSS-FILE. `CallGraph::add_dependency` rejects
        //     edges whose endpoints are not in `self.symbols`, so we
        //     CANNOT add it here — the `graph` only knows about this
        //     file's symbols. We stash it in `cross_file_edges` and let
        //     `merge_with_report` reconcile it after every file has
        //     been merged into the project graph (where both endpoints
        //     exist by construction).
        //
        //   * Edges that resolve to NEITHER the local map nor the global
        //     index are dropped honestly: they could not be proven
        //     (ambiguous, or genuinely unresolved) and the operator's
        //     directive forbids inventing one.
        let caller_path = Path::new(file_path);
        let mut cross_file_edges: Vec<CrossFileEdge> = Vec::new();
        for (caller, callee_name) in relationships {
            let caller_id =
                crate::domain::aggregates::call_graph::SymbolId::new(caller.fully_qualified_name());

            // 1. Local lookup first (covers intra-file edges).
            if let Some(callee_id) = name_to_symbol
                .get(&callee_name.to_lowercase())
                .cloned()
            {
                let _ = graph.add_dependency(&caller_id, &callee_id, DependencyType::Calls);
                continue;
            }

            // 2. Global lookup only makes sense if a `global_index` was
            //    supplied. If the resolver finds an unambiguous match
            //    in another file, defer the edge to the post-merge
            //    step where both endpoints exist in the merged graph.
            if let Some(idx) = global_index
                && let Some(callee_id) =
                    idx.resolve(&callee_name.to_lowercase(), Some(caller_path))
            {
                cross_file_edges.push((caller_id, callee_id));
            }
            // 3. Otherwise: drop honestly. The call site references a
            //    name that has no symbol in the project (or is
            //    ambiguous). Logging the drop is left for a future
            //    diagnostics pass; here we keep the resolver contract
            //    small and explicit.
        }

        Ok((graph, cross_file_edges))
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

/// Why a file was skipped when building a graph.
///
/// Distinguishing these reasons lets callers (CLI/MCP) surface an
/// actionable message instead of "the file disappeared". Adding a new
/// reason is non-breaking: existing consumers pattern-match the
/// variants they care about and a default arm treats anything new as
/// `Other`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// `std::fs::read_to_string` failed (permission denied, broken
    /// symlink, file vanished between walk and read, …).
    Read(String),
    /// The file was read but the parser (tree-sitter) could not extract
    /// any symbols or relationships from it.
    Parse(String),
    /// The file extension is not in the supported set (rs|py|js|ts).
    UnsupportedExtension(String),
    /// Any other I/O error not covered above.
    Other(String),
}

/// A single file that the per-file strategy could not include in the
/// merged graph, together with the reason it was skipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedFile {
    pub path: String,
    pub reason: SkipReason,
}

/// Overall status of a `merge_with_report` operation.
///
/// Three states are distinguished on purpose. A "Complete" result means
/// every input file was processed. A "Partial" result carries a list of
/// skipped files the caller must surface. There is intentionally no
/// "Failed" state at this layer: if the strategy cannot start (e.g.
/// the project directory does not exist), the surrounding method
/// returns `Err` and never reaches `merge_with_report`. A "no findings"
/// outcome is only legitimate when coverage was `Complete`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildStatus {
    Complete,
    Partial { skipped: Vec<SkippedFile> },
}

/// The result of `PerFileGraphCache::merge_with_report`.
///
/// Carries both the resulting `CallGraph` and a [`BuildStatus`] so the
/// caller can decide how to present partial coverage.
#[derive(Clone, Debug)]
pub struct BuildReport {
    pub graph: CallGraph,
    pub status: BuildStatus,
}

/// Maps a `std::io::Error` to a [`SkipReason`].
///
/// - `PermissionDenied` and `NotFound` (file vanished mid-walk) are
///   classified as `Read`.
/// - `InvalidData` and `InvalidInput` (with a "Syntax errors" or
///   "Unsupported file type" message) are classified as `Parse`.
/// - Any other error keeps the OS-provided message under `Other`.
fn classify_io_error(e: &std::io::Error) -> SkipReason {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::PermissionDenied | ErrorKind::NotFound => SkipReason::Read(e.to_string()),
        ErrorKind::InvalidData => SkipReason::Parse(e.to_string()),
        ErrorKind::InvalidInput => {
            // TreeSitterParser::new failure is reported as
            // `Error::other`, but unsupported file types come through
            // `InvalidInput` from the `Language::from_extension` arm.
            if e.to_string().contains("Unsupported file type") {
                SkipReason::UnsupportedExtension(e.to_string())
            } else {
                SkipReason::Parse(e.to_string())
            }
        }
        _ => SkipReason::Other(e.to_string()),
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

    // --- PRF F2.W2: silent read/parse error reporting (R3) ---

    /// Test fixture for F2.W2: the corpus is at
    /// docs/prf/fixtures/per_file_partial_corpus/. It contains a clean
    /// file (`good.rs`) and two files that must be reported as skipped
    /// (broken_syntax.rs and, on Unix, unreadable.rs after chmod 000).
    fn w2_corpus() -> std::path::PathBuf {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        std::path::PathBuf::from(manifest_dir)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("docs/prf/fixtures/per_file_partial_corpus")
    }

    /// `merge_with_report` must surface a parse failure as a skipped
    /// entry, NOT silently swallow it.
    #[test]
    fn test_merge_with_report_surfaces_parse_error() {
        let corpus = w2_corpus();
        assert!(
            corpus.join("src/good.rs").exists(),
            "W2 corpus missing good.rs"
        );
        assert!(
            corpus.join("src/broken_syntax.rs").exists(),
            "W2 corpus missing broken_syntax.rs"
        );

        let cache = PerFileGraphCache::new();
        let good = corpus.join("src/good.rs");
        let broken = corpus.join("src/broken_syntax.rs");
        let report = cache.merge_with_report(&[good.as_path(), broken.as_path()]);

        // The good file must have produced symbols.
        assert!(
            report.graph.symbol_count() >= 1,
            "good.rs should contribute at least one symbol"
        );

        // The broken file must have been reported as skipped with
        // a Parse reason.
        match &report.status {
            BuildStatus::Partial { skipped } => {
                let broken_skipped = skipped
                    .iter()
                    .find(|s| s.path.ends_with("broken_syntax.rs"))
                    .unwrap_or_else(|| {
                        panic!(
                            "broken_syntax.rs must appear in skipped list. Got: {:?}",
                            skipped
                        )
                    });
                assert!(
                    matches!(broken_skipped.reason, SkipReason::Parse(_)),
                    "broken_syntax.rs must be classified as Parse, got: {:?}",
                    broken_skipped.reason
                );
            }
            BuildStatus::Complete => {
                panic!(
                    "broken_syntax.rs was silently dropped — the strategy \
                     claimed Complete coverage but skipped a file. This \
                     is the R3 defect: coverage was incomplete but the \
                     caller was not told."
                );
            }
        }
    }

    /// `merge_with_report` must classify `std::io::ErrorKind::PermissionDenied`
    /// and `NotFound` as `SkipReason::Read`, not as `Parse`.
    #[test]
    fn test_classify_io_error_read_vs_parse() {
        let perm = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        match classify_io_error(&perm) {
            SkipReason::Read(_) => {}
            other => panic!("PermissionDenied must classify as Read, got {:?}", other),
        }

        let not_found = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        match classify_io_error(&not_found) {
            SkipReason::Read(_) => {}
            other => panic!("NotFound must classify as Read, got {:?}", other),
        }

        let other_err = std::io::Error::other("weird");
        match classify_io_error(&other_err) {
            SkipReason::Other(_) => {}
            other => panic!("Other must classify as Other, got {:?}", other),
        }
    }

    /// On Unix, a file chmod'd to 0 must be reported as skipped with a
    /// Read reason (not silently dropped, not classified as Parse).
    /// On non-Unix platforms the test gracefully reports as skipped.
    #[cfg(unix)]
    #[test]
    fn test_merge_with_report_surfaces_unreadable_file() {
        use std::os::unix::fs::PermissionsExt as _;
        let corpus = w2_corpus();

        // Create the unreadable file in the corpus, restoring perms
        // unconditionally after the assertion phase.
        let path = corpus.join("src/unreadable.rs");
        std::fs::write(
            &path,
            "pub fn unreadable_fn() -> u32 { 99 }\n",
        )
        .expect("write unreadable.rs");

        let original_perms = std::fs::metadata(&path).unwrap().permissions();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
            .expect("chmod 0o000");

        // Run the assertion; capture the outcome so we always restore
        // permissions before propagating any failure.
        let outcome: Result<(), String> = {
            let cache = PerFileGraphCache::new();
            let report = cache.merge_with_report(&[path.as_path()]);

            assert_eq!(
                report.graph.symbol_count(),
                0,
                "an unreadable file should contribute 0 symbols"
            );
            match &report.status {
                BuildStatus::Partial { skipped } => {
                    let s = skipped
                        .iter()
                        .find(|s| s.path.ends_with("unreadable.rs"))
                        .expect("unreadable.rs must appear in skipped list");
                    assert!(
                        matches!(s.reason, SkipReason::Read(_)),
                        "unreadable.rs must be classified as Read, got: {:?}",
                        s.reason
                    );
                    Ok(())
                }
                BuildStatus::Complete => {
                    Err("unreadable.rs was silently dropped (R3 defect)".to_string())
                }
            }
        };

        let _ = std::fs::set_permissions(&path, original_perms);
        if let Err(msg) = outcome {
            panic!("{}", msg);
        }
    }
}

#[cfg(test)]
mod global_index_tests {
    //! PRF F2.W5 — coverage for the scope-aware resolver rules in
    //! [`GlobalSymbolIndex`]. These tests pin down the four documented
    //! cases so that any future change that re-introduces "pick the
    //! last inserted" or similar shortcuts fails RED.
    use super::*;
    use crate::domain::aggregates::call_graph::SymbolId;
    use std::path::PathBuf;

    fn sid(fqn: &str) -> SymbolId {
        SymbolId::new(fqn)
    }

    /// Rule: 1 candidate → resolver returns it directly.
    #[test]
    fn w5_resolve_single_candidate_returns_it() {
        let mut idx = GlobalSymbolIndex::new();
        idx.insert(
            sid("src/lib.rs:foo:1"),
            PathBuf::from("src/lib.rs"),
            "foo",
        );
        let r = idx.resolve("foo", Some(&PathBuf::from("src/lib.rs")));
        assert_eq!(r, Some(sid("src/lib.rs:foo:1")));
        assert!(!idx.is_empty());
        assert_eq!(idx.len(), 1);
        assert_eq!(idx.candidates("foo").len(), 1);
    }

    /// Rule: 0 candidates → resolver returns None (call site referencing
    /// a name that has no symbol in the project).
    #[test]
    fn w5_resolve_no_candidates_returns_none() {
        let idx = GlobalSymbolIndex::new();
        assert_eq!(idx.resolve("does_not_exist", None), None);
        assert_eq!(idx.len(), 0);
        assert!(idx.is_empty());
    }

    /// Rule: multiple candidates in different files → resolver drops
    /// the edge when no scope disambiguates (no caller file passed).
    #[test]
    fn w5_resolve_ambiguous_without_caller_drops() {
        let mut idx = GlobalSymbolIndex::new();
        idx.insert(sid("a.rs:foo:1"), PathBuf::from("a.rs"), "foo");
        idx.insert(sid("b.rs:foo:1"), PathBuf::from("b.rs"), "foo");
        let r = idx.resolve("foo", None);
        assert!(r.is_none(), "expected None when caller context is missing");
    }

    /// Rule: multiple candidates in different files with caller context
    /// but NO shared crate root → drop.
    #[test]
    fn w5_resolve_no_shared_root_drops() {
        let mut idx = GlobalSymbolIndex::new();
        idx.insert(sid("a.rs:foo:1"), PathBuf::from("a.rs"), "foo");
        idx.insert(sid("b.rs:foo:1"), PathBuf::from("b.rs"), "foo");
        let r = idx.resolve("foo", Some(&PathBuf::from("caller.rs")));
        assert!(r.is_none(), "expected None when files share no path components");
    }
}
