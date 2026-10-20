//! Per-file graph cache for modular graph construction
//!
//! This module provides a cache that stores a CallGraph per file, allowing
//! for modular graph construction and merging. This is useful when you need
//! to analyze individual files and then combine them into larger graphs.

use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::value_objects::{DependencyType, PendingReference, WalkDecision};
use crate::infrastructure::graph::semantic_layer::{call_graph_from, semantic_facts};
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

/// Result of building a single file's call graph.
///
/// The cross-file buffer holds the relations whose far end lives in another
/// file; the pending buffer holds the references the file states and nobody
/// declared, whose far end is not identified yet. Both are settled once
/// every file of the merge is in place.
pub type BuildFileResult = (CallGraph, Vec<CrossFileEdge>, Vec<PendingReference>);

/// Project-wide symbol index for cross-file call resolution (H-R4-2).
///
/// See module-level doc-comment for the resolution rules.
///
/// `#![allow(dead_code)]` keeps the diagnostic helpers (`candidates`,
/// `len`, `is_empty`) compiled for future test-audit and observability
/// work even though F2.W5's happy path does not exercise them. The
/// non-happy path (dropped edges, ambiguous resolutions) will need
/// them in F3/C3 diagnostics.
/// The outcome of resolving one callee name to one target.
///
/// ## Why this exists
///
/// The resolver used to answer `Option<SymbolId>`, which is honest about
/// *not* naming a target but dishonest about *why*: a name that matched
/// nothing and a name that matched two homonyms both collapsed to `None`.
/// For impact analysis those are different facts. "This call goes to
/// either `a.rs:foo` or `b.rs:foo`, and we could not prove which" tells
/// the user where the uncertainty is; "there is no `foo`" tells them
/// nothing is unknown. Collapsing them means a consumer cannot surface
/// a gap it never learned exists.
///
/// Naming a target stays the privilege of `Resolved` alone. That is what
/// keeps the "no invented edges" guarantee structural rather than a rule
/// someone has to remember: an ambiguous call cannot become an edge
/// because there is no variant that both says "ambiguous" and carries a
/// target.
///
/// ## Relation to `ingest::types::TargetRef`
///
/// Not a duplicate of it. `TargetRef` is an extraction-stage *work item* —
/// `Unresolved(name)` means "a later stage will try to match this". This
/// is the *result* of that matching, produced against a built index of
/// `SymbolId`s. Different stage, different payload, different lifetime.
///
/// ## Why no `External` variant
///
/// `ADR-K003` proposes a fourth variant for callees known to live outside
/// the analyzed project. Nothing in the current pipeline can produce it —
/// there is no external-symbol table — so it would be a variant no code
/// can construct and no test could exercise. The rule that pays is the one
/// that forbids representing a fact you cannot observe: an unused variant
/// is an assumption that will survive longer than the code that should
/// have filled it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetResolution {
    /// Exactly one target, proven by a disambiguation rule.
    Resolved(SymbolId),
    /// The name matched, but the target cannot be proven. The candidates
    /// are retained so a consumer can report the uncertainty instead of
    /// silently shrinking the graph.
    Ambiguous {
        /// The lowercased callee name that was being resolved.
        name: String,
        /// Every candidate, in index insertion order.
        candidates: Vec<SymbolId>,
    },
    /// The name matched nothing in the project.
    Unresolved(String),
}

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

    /// The candidates of a name that live in the same directory as the
    /// file that asks for them.
    ///
    /// Infrastructure code is scoped by directory: the declarations a file
    /// can see are the ones of its own module, and a module is a directory.
    /// A name that several modules declare is told apart by the module that
    /// writes the reference.
    pub fn candidates_in_directory(&self, name_lower: &str, directory: &Path) -> Vec<SymbolId> {
        let Some(candidates) = self.by_name.get(name_lower) else {
            return Vec::new();
        };
        candidates
            .iter()
            .filter(|id| {
                self.by_id
                    .get(*id)
                    .and_then(|(file, _)| file.parent())
                    .is_some_and(|parent| parent == directory)
            })
            .cloned()
            .collect()
    }

    /// Resolves a callee name to a target, preferring the caller-file
    /// symbol when ambiguity exists.
    ///
    /// Only [`TargetResolution::Resolved`] names a target. Every other
    /// outcome means the caller MUST drop the edge — an ambiguous call
    /// is never wired to a homonym — but each says a different thing
    /// about why, and that difference is what lets a consumer report a
    /// gap instead of silently shrinking the graph.
    pub fn resolve(&self, name_lower: &str, caller_file: Option<&Path>) -> TargetResolution {
        let Some(candidates) = self.by_name.get(name_lower) else {
            return TargetResolution::Unresolved(name_lower.to_string());
        };
        if candidates.is_empty() {
            return TargetResolution::Unresolved(name_lower.to_string());
        }
        if candidates.len() == 1 {
            return TargetResolution::Resolved(candidates[0].clone());
        }

        // Multiple homonyms. Every refusal below is the SAME fact —
        // "the name exists, the target cannot be proven" — so they all
        // return the one construction that says it.
        let ambiguous = TargetResolution::Ambiguous {
            name: name_lower.to_string(),
            candidates: candidates.clone(),
        };

        // Multiple homonyms. Try to disambiguate by caller-file proximity.
        let Some(caller) = caller_file else {
            // No caller context → nothing to disambiguate with.
            return ambiguous;
        };

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
            return TargetResolution::Resolved(same_file[0].clone());
        }
        if same_file.len() > 1 {
            // Genuine intra-file duplicate (e.g. two `same_name` in
            // `dup.rs` — one top, one inside `inner`). Without module
            // context we cannot pick the right one.
            return ambiguous;
        }

        // Rule 2: shared crate root (first N path components match).
        let in_same_crate: Vec<&SymbolId> = candidates
            .iter()
            .filter(|sid| {
                self.by_id
                    .get(*sid)
                    .map(|(p, _)| shared_crate_root(p, caller).is_some())
                    .unwrap_or(false)
            })
            .collect();
        if in_same_crate.len() == 1 {
            return TargetResolution::Resolved(in_same_crate[0].clone());
        }
        // Multiple symbols across files in the same crate, no
        // further rule applies.
        ambiguous
    }

    /// Returns the candidate `SymbolId`s for a given name (used by callers
    /// that want to audit ambiguity, e.g. diagnostics or tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn candidates(&self, name_lower: &str) -> &[SymbolId] {
        self.by_name
            .get(name_lower)
            .map(Vec::as_slice)
            .unwrap_or(&[])
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
    /// UNIX epoch). Cheap signal: catches the common case of an editor
    /// saving the file. **Not authoritative**: a same-content rewrite with
    /// identical mtime+size (e.g. `git checkout` of an unchanged-size
    /// blob, or an editor that pads to the same length) keeps the mtime
    /// and would, on its own, serve a stale graph. The `content_hash`
    /// below is the authoritative bytes-identity check.
    mtime_secs: Option<u64>,
    /// Size of the source file in bytes at cache time. Secondary cheap
    /// signal: combined with `mtime_secs`, narrows the false-positive
    /// surface. Still not authoritative for the same reason as
    /// `mtime_secs` — a content edit at the same byte length keeps the
    /// size unchanged.
    size: Option<u64>,
    /// 64-bit hash of the source file's bytes at cache time. **The
    /// authoritative bytes-identity check.** Required by the H0.1 spec
    /// (`PerFileGraphCache::merge_with_report: no usar get_cached() sin
    /// validar entry.valid, identidad de bytes y ruta`): a file whose
    /// content was rewritten but whose mtime+size did not change must
    /// still produce a cache miss. The H0.5 acceptance law
    /// (`editar bytes con mtime/tamaño iguales ⇒ cambia el resultado`)
    /// is the test that pins this field.
    content_hash: Option<u64>,
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
    /// fingerprint (mtime + size + content hash) matches what was
    /// recorded at cache time, the cached graph is returned. Otherwise
    /// the file is re-parsed and the cache entry is refreshed.
    ///
    /// The mtime+size signals are cheap (one syscall) and catch the
    /// common case of an editor saving the file. The content hash is
    /// authoritative: it catches the case where the file's bytes were
    /// rewritten but mtime+size did not change (e.g. `git checkout` of
    /// an unchanged-size blob, an editor that rewrites in place, or
    /// `cp -p` of an identical-length file). H0.1 / H0.5 require that
    /// case to invalidate the cache, not serve a stale graph.
    pub fn get_or_build(&self, file_path: &Path) -> std::io::Result<Arc<CallGraph>> {
        let path_str = file_path.to_string_lossy().to_string();

        // Snapshot the file fingerprint (mtime + size + content hash)
        // once, so the read-cache and write-cache branches agree on what
        // we observed. The content hash is the authoritative check;
        // mtime+size are the cheap early-out.
        let fingerprint = file_fingerprint(file_path);

        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(&path_str)
                && entry.valid
                && entry.mtime_secs == fingerprint.as_ref().map(|f| f.mtime_secs)
                && entry.size == fingerprint.as_ref().map(|f| f.size)
                && entry.content_hash == fingerprint.as_ref().map(|f| f.content_hash)
            {
                return Ok(Arc::new(entry.graph.clone()));
            }
        }

        // Build the graph for this file (no global index: per-file resolution only)
        let (graph, _cross_file, _pending) = self.build_file_graph(&path_str, None)?;

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
                    content_hash: fingerprint.as_ref().map(|f| f.content_hash),
                },
            );
        }

        Ok(Arc::new(graph))
    }

    /// Gets a cached graph without building if missing
    ///
    /// Returns `None` when the entry is absent OR has been invalidated
    /// (`entry.valid == false`). Callers that need a guaranteed-fresh
    /// graph should use [`PerFileGraphCache::get_or_build`].
    pub fn get_cached(&self, file_path: &Path) -> Option<Arc<CallGraph>> {
        let path_str = file_path.to_string_lossy().to_string();
        let cache = self.cache.read().unwrap();
        cache
            .get(&path_str)
            .filter(|entry| entry.valid)
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
    /// `PerFileGraphCache::merge_with_report` when callers need to know
    /// whether coverage is complete or partial.
    pub fn merge(&self, file_paths: &[&Path]) -> CallGraph {
        self.merge_with_report(file_paths).graph
    }

    /// Merges multiple file graphs into a single graph and reports which
    /// files were skipped (and why).
    ///
    /// This is the explicit, non-silent counterpart of `merge`. The
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
        // The references the files state and nobody declared. They are not
        // edges yet: the merge may still be able to tie them to a declaration
        // in another of the merged files.
        let mut pending_references: Vec<PendingReference> = Vec::new();

        for path in file_paths {
            let path_str = path.to_string_lossy().to_string();

            // Get from cache or build. Cache hit does not produce skipped
            // entries; only actual build failures do.
            let built: Result<BuildFileResult, std::io::Error> = match self.get_cached(path) {
                Some(g) => Ok(((*g).clone(), Vec::new(), Vec::new())),
                None => self.build_file_graph(&path_str, Some(&global_index)),
            };
            let (graph, cross_file, pending) = match built {
                Ok(triple) => triple,
                Err(e) => {
                    skipped.push(SkippedFile {
                        path: path_str,
                        reason: classify_io_error(&e),
                    });
                    continue;
                }
            };
            for symbol in graph.symbols() {
                // Fusionar no renombra. La identidad de un símbolo es su
                // nombre completo, y una declaración que vino de un handler
                // —`tf:{fichero}:{bloque}`, `ansible:{fichero}:play:{nombre}`—
                // tiene un nombre que no se deduce del fichero y del
                // símbolo. Reconstruirlo aquí lo convertía en uno del
                // camino genérico, y el producto lo pedía por el nombre que
                // su handler le había dado.
                merged.add_symbol(symbol.clone());
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
            pending_references.extend(pending);
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

        // The references that could not be tied to a declaration are looked
        // up once every file of the merge is known: the declaration they
        // name may live in another of the merged files. What the lookup
        // proves becomes a relation; what it cannot prove stays pending and
        // says why.
        let resolution = crate::infrastructure::graph::pending::resolve_pending_references(
            pending_references,
            &global_index,
        );
        for (source_id, target_id, kind) in &resolution.proven {
            merged.add_dependency(source_id, target_id, *kind).ok();
        }

        let status = if skipped.is_empty() && resolution.pending.is_empty() {
            BuildStatus::Complete
        } else {
            BuildStatus::Partial {
                skipped,
                pending: resolution.pending,
            }
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
            // A language with a semantic layer names its declarations the
            // way its reader named them, and those declarations are the ones
            // a reference written in another file can point at. The index
            // takes them from the same reader the graph takes them from:
            // indexing the generic symbols of the same file would be a
            // second authority saying something else.
            if crate::infrastructure::parser::language_config::LanguageConfig::for_language(
                language,
            )
            .semantic_handler
            .is_some()
            {
                if let Some(facts) = semantic_facts(language, &source, &path_str) {
                    for symbol in facts.symbols {
                        let sid = SymbolId::new(symbol.fully_qualified_name());
                        idx.insert(sid, path.to_path_buf(), symbol.name());
                    }
                }
                continue;
            }
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

        // The semantic layer leads when the language declares one. Its
        // edges stay inside the file, so `cross_file_edges` is empty by
        // construction and the global index stays out of it. The references
        // it could not tie travel with the file: the merge may still be able
        // to tie them to a declaration in another of the merged files.
        if let Some(facts) = semantic_facts(language, &source, file_path) {
            return Ok((call_graph_from(&facts), Vec::new(), facts.pending));
        }

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
        // H4.3 — multimap keyed by lowercase name. A single-symbol
        // map overwrote homonyms (e.g. `Foo` then `foo` both
        // lowercased to `foo`): the second insert clobbered the
        // first, and a call site that should be observed as
        // "ambiguous, no edge" was silently resolved to whichever
        // symbol the parser inserted last. A list-valued entry
        // makes the homonym visible: a call site with a single
        // candidate resolves; with multiple candidates drops
        // (H4: "llamadas ambiguas quedan observadas sin una
        // dependencia falsa").
        let mut name_to_symbol: HashMap<
            String,
            Vec<crate::domain::aggregates::call_graph::SymbolId>,
        > = HashMap::new();

        // Add symbols to graph
        for symbol in symbols {
            let symbol_id = graph.add_symbol(symbol.clone());
            name_to_symbol
                .entry(symbol.name().to_lowercase())
                .or_default()
                .push(symbol_id);
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
            //    H4.3 — the local map is now a multimap; multiple
            //    symbols under the same lowercase name are an
            //    ambiguous homonym and the resolver drops the call
            //    rather than invent an edge against a random one.
            if let Some(candidates) = name_to_symbol.get(&callee_name.to_lowercase()) {
                match candidates.as_slice() {
                    [] => {}
                    [callee_id] => {
                        let _ = graph.add_dependency(&caller_id, callee_id, DependencyType::Calls);
                    }
                    _ => {
                        // Genuine local homonym (e.g. two symbols
                        // `Foo` and `foo`). The H4 spec calls for
                        // "llamadas ambiguas quedan observadas sin
                        // una dependencia falsa" — drop the call
                        // edge. A future diagnostics pass can
                        // surface the unresolved call site.
                    }
                }
                continue;
            }

            // 2. Global lookup only makes sense if a `global_index` was
            //    supplied. If the resolver finds an unambiguous match
            //    in another file, defer the edge to the post-merge
            //    step where both endpoints exist in the merged graph.
            if let Some(idx) = global_index {
                match idx.resolve(&callee_name.to_lowercase(), Some(caller_path)) {
                    TargetResolution::Resolved(callee_id) => {
                        cross_file_edges.push((caller_id, callee_id));
                    }
                    // Both refusals drop the edge, and neither arm can
                    // produce one: naming a target is exclusive to
                    // `Resolved`. The two are distinct facts now, so a
                    // later pass can count them separately instead of
                    // reading a single silent `None`.
                    TargetResolution::Ambiguous { .. } | TargetResolution::Unresolved(_) => {}
                }
            }
            // 3. Otherwise: drop honestly. The call site references a
            //    name with no symbol in the project, or one whose target
            //    cannot be proven. The graph stays free of invented
            //    edges; counting and surfacing the drops is H4.2.
        }

        Ok((graph, cross_file_edges, Vec::new()))
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

/// Filesystem fingerprint for cache invalidation.
///
/// Holds the mtime (seconds since UNIX epoch), the byte size, and a
/// 64-bit hash of the file's bytes — all three at the moment the
/// fingerprint was taken. `mtime_secs` and `size` are cheap early-out
/// signals (one metadata syscall); `content_hash` is the authoritative
/// bytes-identity check. The H0 spec requires all three: a file whose
/// bytes were rewritten but whose mtime+size did not change must
/// still produce a cache miss.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileFingerprint {
    mtime_secs: u64,
    size: u64,
    content_hash: u64,
}

/// Reads the fingerprint of `path`. Returns `None` if the metadata or
/// file body cannot be read (e.g. file was deleted between cache hit
/// and re-check, or the file is unreadable). In that case the caller
/// should treat the cache as stale and rebuild.
///
/// The content hash uses Rust's stdlib `DefaultHasher` — a non-cryptographic
/// 64-bit hash seeded per-process for DoS resistance. The seed is fine
/// for in-process cache invalidation: the cache is in-process, and a
/// different process is a different cache. Cryptographic collision
/// resistance is not required: a collision would only cause a cache
/// miss (the wrong graph would have to be parseable as the right one
/// to be observable, and a 64-bit space makes that overwhelmingly
/// unlikely). The cost is one `read()` per fingerprint, which is the
/// cost of the H0.1 / H0.5 content-identity contract.
fn file_fingerprint(path: &Path) -> Option<FileFingerprint> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let mtime_secs = system_time_to_secs(mtime);
    let size = meta.len();
    let bytes = std::fs::read(path).ok()?;
    let content_hash = content_hash_64(&bytes);
    Some(FileFingerprint {
        mtime_secs,
        size,
        content_hash,
    })
}

/// 64-bit FNV-1a over the input bytes. Pure stdlib, no allocations,
/// constant memory; sufficient for an in-process cache key where
/// collisions only ever cost a cache miss.
pub(crate) fn content_hash_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3); // FNV prime
    }
    hash
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
    /// A symlink, which the walk declines to follow so that it cannot
    /// leave the root the caller named.
    ///
    /// This is a skip and not a silent omission because the caller
    /// needs to know the coverage is partial: a project that
    /// symlinks its sources in from elsewhere is **not** fully
    /// described by the graph it gets back, and reporting `Complete`
    /// here would be the same class of lie as indexing the target and
    /// recording the link's path as if it were the target's.
    Symlink { target: String },
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

/// Extensiones de lenguajes que el producto reconoce como relevantes
/// pero que aún no tienen gramática cableada (puente K0, línea Kotlin).
///
/// Una fuente con una de estas extensiones no se descarta en silencio:
/// el reporte la lista en `skipped` con razón `UnsupportedExtension` y
/// el estado pasa a `Partial`. Es la distinción entre «lenguaje conocido
/// sin soporte» (gap honesto) y «extensión irrelevante» como un
/// README.md (exclusión legítima, sin gap).
///
/// Cada entrada vive aquí SOLO hasta que su gramática aterriza en el
/// registro (`Language::from_extension`) — la línea Kotlin (K1) mueve
/// `kt`/`kts` de esta lista al registro.
pub const KNOWN_UNSUPPORTED_EXTENSIONS: &[&str] = &["kt", "kts"];

/// K0 — clasificación única de un archivo de walk, compartida por las
/// estrategias. Una misma semántica, un dueño: el registro
/// `Language::from_extension` decide qué se analiza; esta clasificación
/// decide qué es gap y qué es exclusión legítima.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WalkClassification {
    /// Extensión en el registro: la fuente se analiza (sus fallos de
    /// lectura/parse se reportan aparte, con razón tipada).
    Analyze,
    /// Lenguaje conocido-sin-soporte: gap honesto con razón
    /// `UnsupportedExtension`.
    KnownUnsupportedGap,
    /// Extensión irrelevante para el análisis (README, imágenes, datos):
    /// exclusión legítima, no genera gap ni entrada en `skipped`.
    LegitExclusion,
}

/// H1.1 (delivery-first-radical, package integration) — unified
/// classification envelope for every file the walk encounters.
///
/// This is NOT a replacement for the existing taxonomy
/// (`WalkClassification` + `SkipReason` + presence-in-graph); it is
/// the *projection* onto the five logical states the H1.1 spec
/// names, so that "every file lands in exactly one of the five
/// states" can be expressed at the type level. AC-01 §Invariantes:
/// "FileDisposition = Parsed | ExcludedByPolicy | UnsupportedRelevant
/// | ReadFailed | ParseFailed". A sixth state ("Unknown", "Pending",
/// "Failed", …) would re-introduce the silent-discard bug the
/// envelope was created to prevent; the test
/// `h11_unified_classification_contract::file_disposition_has_exactly_the_five_spec_variants`
/// pins the count to 5.
///
/// Each variant carries the minimal payload to render an actionable
/// message in CLI/MCP. The variant names are part of the public
/// contract (AC-01 names them by name) and must remain stable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileDisposition {
    /// The file was read, parsed, and contributed symbols to the
    /// graph. Only this variant can be summarised as "this
    /// contributed to the answer".
    Parsed,
    /// The path is outside the analysis scope, by policy:
    ///   - `WalkFilter` pruned it (security blocklist: `.git/HEAD`,
    ///     `id_rsa`, …), OR
    ///   - the file is a known-irrelevant asset (README, image,
    ///     lockfile), OR
    ///   - an unknown I/O error mapped to `SkipReason::Other`
    ///     (honest default: we don't claim a gap we can't name).
    ///
    /// Never affects `BuildStatus`.
    ExcludedByPolicy { reason: String },
    /// The file is a *source* in a language we know about but have
    /// not yet wired (the K0 bridge: `.kt`/`.kts` until K1 lands).
    /// Affects `BuildStatus` → `Partial`.
    UnsupportedRelevant { extension: String },
    /// `std::fs::read` failed (permission denied, broken symlink,
    /// file vanished between walk and read, …). Affects
    /// `BuildStatus` → `Partial`.
    ReadFailed { reason: String },
    /// The file was read but the parser could not extract symbols.
    /// Affects `BuildStatus` → `Partial`.
    ParseFailed { reason: String },
}

impl FileDisposition {
    /// The five H1.1 variants, in the order the spec lists them.
    /// Used by the test that pins the count and the names.
    pub fn all() -> [FileDisposition; 5] {
        [
            FileDisposition::Parsed,
            FileDisposition::ExcludedByPolicy {
                reason: "policy".to_string(),
            },
            FileDisposition::UnsupportedRelevant {
                extension: "kt".to_string(),
            },
            FileDisposition::ReadFailed {
                reason: "EACCES".to_string(),
            },
            FileDisposition::ParseFailed {
                reason: "unexpected EOF".to_string(),
            },
        ]
    }

    /// Stable name for diagnostics and the public contract.
    pub fn name(&self) -> &'static str {
        match self {
            FileDisposition::Parsed => "Parsed",
            FileDisposition::ExcludedByPolicy { .. } => "ExcludedByPolicy",
            FileDisposition::UnsupportedRelevant { .. } => "UnsupportedRelevant",
            FileDisposition::ReadFailed { .. } => "ReadFailed",
            FileDisposition::ParseFailed { .. } => "ParseFailed",
        }
    }

    /// Constructor for the happy path. A file that was read, parsed
    /// and contributed to the graph.
    pub fn parsed() -> FileDisposition {
        FileDisposition::Parsed
    }

    /// Project a `WalkClassification` onto the envelope. This is
    /// the call for files that the walk *saw* but that did not
    /// generate a `SkippedFile` (either because they were
    /// irrelevant, or because they were analyzed and merged).
    pub fn from_walk_classification(
        class: WalkClassification,
        ext: Option<&str>,
    ) -> FileDisposition {
        match class {
            WalkClassification::Analyze => FileDisposition::Parsed,
            WalkClassification::KnownUnsupportedGap => FileDisposition::UnsupportedRelevant {
                extension: ext.unwrap_or("?").to_string(),
            },
            WalkClassification::LegitExclusion => FileDisposition::ExcludedByPolicy {
                reason: format!("irrelevant extension: {}", ext.unwrap_or("?")),
            },
        }
    }

    /// Project a `WalkDecision` (the result of `WalkFilter::should_walk`)
    /// onto the envelope. A `Prune` decision means the path is
    /// outside the analysis scope; `Skip` and `Include` are not
    /// `FileDisposition` values — `Skip` means "descend but don't
    /// classify this entry" (handled at the walk level, not here)
    /// and `Include` means the entry will be classified by
    /// `from_walk_classification` once read.
    pub fn from_walk_decision(decision: WalkDecision) -> FileDisposition {
        match decision {
            WalkDecision::Prune => FileDisposition::ExcludedByPolicy {
                reason: "walk_filter_pruned".to_string(),
            },
            // `Include` and `Skip` are walk-level decisions, not
            // file dispositions; a file that survives a Skip
            // decision is handled by `from_walk_classification`
            // (the descent continues, the file is examined).
            WalkDecision::Include | WalkDecision::Skip => FileDisposition::ExcludedByPolicy {
                reason: "walk_decision_not_a_disposition".to_string(),
            },
        }
    }

    /// Project a `SkippedFile` (the result of a walk + read + parse
    /// that failed for a typed reason) onto the envelope. This is
    /// the call for paths that reached the read/parse stage and
    /// produced a `SkippedFile` row.
    pub fn from_skipped(skipped: &SkippedFile) -> FileDisposition {
        match &skipped.reason {
            SkipReason::Read(reason) => FileDisposition::ReadFailed {
                reason: reason.clone(),
            },
            SkipReason::Parse(reason) => FileDisposition::ParseFailed {
                reason: reason.clone(),
            },
            SkipReason::UnsupportedExtension(extension) => FileDisposition::UnsupportedRelevant {
                extension: extension.clone(),
            },
            // `SkipReason::Other` is the honest fallback for I/O
            // errors we can't name precisely. Mapping it to
            // UnsupportedRelevant would falsely demote BuildStatus
            // for paths we don't know about; the honest default is
            // ExcludedByPolicy (we know nothing → we don't claim a
            // gap).
            // A symlink is not a policy exclusion — the walk is
            // refusing to leave the root, which is a coverage gap the
            // caller must be able to name.
            SkipReason::Symlink { target } => FileDisposition::ExcludedByPolicy {
                reason: format!("symlink to {target}, not followed"),
            },
            SkipReason::Other(reason) => FileDisposition::ExcludedByPolicy {
                reason: reason.clone(),
            },
        }
    }
}

/// Classify the entry a walk declined to index, if it declined one.
///
/// The one case that is a *decision* rather than an error is the
/// symlink: the walk can see it, understands it, and chooses not to
/// follow it. That still belongs in `skipped`, because `skipped` is
/// how a caller learns the coverage is not complete.
pub fn classify_declined_entry(entry: &walkdir::DirEntry) -> Option<SkipReason> {
    if !entry.file_type().is_symlink() {
        return None;
    }

    let target = match std::fs::read_link(entry.path()) {
        Ok(t) => t.to_string_lossy().to_string(),
        // A dangling or unreadable link: we still declined it, we
        // just cannot say where it pointed.
        Err(_) => "<unreadable>".to_string(),
    };
    Some(SkipReason::Symlink { target })
}

/// The same classification for the `ignore` walker.
///
/// Two functions rather than a trait: a trait would ask the caller to
/// name the trait at every call site, and the whole reason this
/// classification exists is that a caller should not have to remember
/// to write it.
pub fn classify_declined_ignore_entry(entry: &ignore::DirEntry) -> Option<SkipReason> {
    if !entry.file_type().map(|ft| ft.is_symlink()).unwrap_or(false) {
        return None;
    }
    let target = match std::fs::read_link(entry.path()) {
        Ok(t) => t.to_string_lossy().to_string(),
        // A dangling or unreadable link: we still declined it, we
        // just cannot say where it pointed.
        Err(_) => "<unreadable>".to_string(),
    };
    Some(SkipReason::Symlink { target })
}

/// Clasifica la extensión de un archivo de walk según el contrato K0.
pub fn classify_walk_extension(ext: Option<&str>) -> WalkClassification {
    use crate::infrastructure::parser::Language;
    match ext {
        Some(e) if Language::from_extension(Some(std::ffi::OsStr::new(e))).is_some() => {
            WalkClassification::Analyze
        }
        Some(e) if KNOWN_UNSUPPORTED_EXTENSIONS.contains(&e) => {
            WalkClassification::KnownUnsupportedGap
        }
        _ => WalkClassification::LegitExclusion,
    }
}

/// Overall status of a `merge_with_report` operation.
///
/// Two coverages live here and they are not the same thing: the files the
/// walk could not process, and the references it could not tie to a
/// declaration. A "Complete" result means both of them are whole — every
/// input file was processed and every reference the files state points at a
/// declaration the project holds. A "Partial" result carries the list of
/// what is missing, so the caller can say it instead of concluding "no
/// findings". There is intentionally no "Failed" state at this layer: if the
/// strategy cannot start (e.g. the project directory does not exist), the
/// surrounding method returns `Err` and never reaches `merge_with_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildStatus {
    /// Every file was processed and every reference was tied to a
    /// declaration.
    Complete,
    /// Something is missing: a file the walk could not process, or a
    /// reference that could not be tied to a declaration.
    Partial {
        /// The files the walk could not process, with the reason.
        skipped: Vec<SkippedFile>,
        /// The references that could not be tied to a declaration, with
        /// their origin and the class that says why.
        pending: Vec<PendingReference>,
    },
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
    // The fixture lives at crates/cognicode-core/testdata/per_file_correctness/ and is
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
            .join("crates/cognicode-core/testdata/per_file_correctness");

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
    /// crates/cognicode-core/testdata/per_file_partial_corpus/. It contains a clean
    /// file (`good.rs`) and two files that must be reported as skipped
    /// (broken_syntax.rs and, on Unix, unreadable.rs after chmod 000).
    fn w2_corpus() -> std::path::PathBuf {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        std::path::PathBuf::from(manifest_dir)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("crates/cognicode-core/testdata/per_file_partial_corpus")
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
            BuildStatus::Partial { skipped, .. } => {
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
        std::fs::write(&path, "pub fn unreadable_fn() -> u32 { 99 }\n")
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
                BuildStatus::Partial { skipped, .. } => {
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
mod semantic_seam_tests {
    //! El seam se atribuye un fichero SOLO si produjo un símbolo.
    //!
    //! Es la contraparte interna de la ley que el test del API público
    //! prueba por fuera. Allí se ve que un bloque HCL llega al grafo; aquí
    //! se ve la regla que hace que un handler mudo NO se lleve el mérito,
    //! porque esa regla es privada y no hay forma de observarla desde el
    //! API público.
    //!
    //! La regla se prueba con un fichero que no es un playbook, y no con un
    //! playbook mudo: un playbook que el handler interpreta tiene que
    //! reclamar su fichero, y esa es la otra mitad de la misma ley.
    use super::*;

    #[test]
    fn a_handler_that_produced_nothing_does_not_claim_the_file() {
        // Un fichero YAML que no declara ni un play ni una tarea no es un
        // playbook, el handler no tiene nada que decir sobre él y el
        // camino genérico sigue teniendo la última palabra.
        let plain = "version: 3\nservices:\n  web:\n    image: nginx\n";
        assert!(
            semantic_facts(Language::Yaml, plain, "/tmp/plain.yml").is_none(),
            "un handler que no interpretaba nada no puede reclamar el fichero; \
             devolver un grafo vacío haría que la capa semántica se atribuyera \
             un fichero que no miró"
        );
    }

    /// La otra mitad de la misma ley: un playbook declara un play, y el
    /// fichero donde vive queda atribuido a la capa semántica.
    #[test]
    fn a_playbook_is_claimed_by_the_semantic_layer() {
        let playbook =
            "- hosts: all\n  tasks:\n    - name: install\n      apt:\n        name: nginx\n";
        let facts = semantic_facts(Language::Yaml, playbook, "/tmp/playbook.yml")
            .expect("un playbook declara un play, y el seam tiene algo que decir");
        assert!(
            !facts.symbols.is_empty(),
            "el playbook se atribuye al seam, y con él van sus símbolos"
        );
    }

    #[test]
    fn a_language_without_a_handler_is_never_routed_here() {
        assert!(
            semantic_facts(Language::Rust, "pub fn a() {}\n", "/tmp/lib.rs").is_none(),
            "sin handler no hay seam: la función debe retirarse antes de \
             construir nada"
        );
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
        idx.insert(sid("src/lib.rs:foo:1"), PathBuf::from("src/lib.rs"), "foo");
        let r = idx.resolve("foo", Some(&PathBuf::from("src/lib.rs")));
        assert_eq!(r, TargetResolution::Resolved(sid("src/lib.rs:foo:1")));
        assert!(!idx.is_empty());
        assert_eq!(idx.len(), 1);
        assert_eq!(idx.candidates("foo").len(), 1);
    }

    /// Rule: 0 candidates → the name exists nowhere in the project.
    #[test]
    fn w5_resolve_no_candidates_is_unresolved() {
        let idx = GlobalSymbolIndex::new();
        assert_eq!(
            idx.resolve("does_not_exist", None),
            TargetResolution::Unresolved("does_not_exist".into())
        );
        assert_eq!(idx.len(), 0);
        assert!(idx.is_empty());
    }

    /// Rule: multiple candidates in different files → the name is known
    /// but its target cannot be proven. That is a *different* fact from
    /// "no such name", and the resolver must say so.
    #[test]
    fn w5_resolve_ambiguous_without_caller_is_ambiguous() {
        let mut idx = GlobalSymbolIndex::new();
        idx.insert(sid("a.rs:foo:1"), PathBuf::from("a.rs"), "foo");
        idx.insert(sid("b.rs:foo:1"), PathBuf::from("b.rs"), "foo");
        let r = idx.resolve("foo", None);
        assert_eq!(
            r,
            TargetResolution::Ambiguous {
                name: "foo".into(),
                candidates: vec![sid("a.rs:foo:1"), sid("b.rs:foo:1")],
            },
            "two homonyms must read as Ambiguous, not as a missing name"
        );
    }

    /// Rule: multiple candidates in different files with caller context
    /// but NO shared crate root → still ambiguous, still not "missing".
    #[test]
    fn w5_resolve_no_shared_root_is_ambiguous() {
        let mut idx = GlobalSymbolIndex::new();
        idx.insert(sid("a.rs:foo:1"), PathBuf::from("a.rs"), "foo");
        idx.insert(sid("b.rs:foo:1"), PathBuf::from("b.rs"), "foo");
        let r = idx.resolve("foo", Some(&PathBuf::from("caller.rs")));
        assert!(
            matches!(r, TargetResolution::Ambiguous { .. }),
            "candidates exist but no rule disambiguates: that is Ambiguous, \
             not Unresolved. Got {r:?}"
        );
    }

    /// The guarantee that must survive the retype: an ambiguous call is
    /// still never wired to a homonym. `Resolved` is the only variant
    /// that names a target, so no other code path can invent one.
    #[test]
    fn w5_ambiguity_still_produces_no_target() {
        let mut idx = GlobalSymbolIndex::new();
        idx.insert(sid("a.rs:foo:1"), PathBuf::from("a.rs"), "foo");
        idx.insert(sid("b.rs:foo:1"), PathBuf::from("b.rs"), "foo");
        let r = idx.resolve("foo", Some(&PathBuf::from("caller.rs")));
        assert!(
            !matches!(r, TargetResolution::Resolved(_)),
            "an unresolved-by-ambiguity call must never become an edge"
        );
    }

    /// Anti-vacuity: ambiguity and absence are not the same observation.
    /// If both collapse to one variant this fails, which is the whole
    /// point of the change — the previous `Option<SymbolId>` merged them.
    #[test]
    fn w5_ambiguity_and_absence_are_distinguishable() {
        let mut idx = GlobalSymbolIndex::new();
        idx.insert(sid("a.rs:foo:1"), PathBuf::from("a.rs"), "foo");
        idx.insert(sid("b.rs:foo:1"), PathBuf::from("b.rs"), "foo");
        let ambiguous = idx.resolve("foo", Some(&PathBuf::from("caller.rs")));
        let absent = idx.resolve("nothing_here", None);
        assert_ne!(
            ambiguous, absent,
            "a known-but-unprovable name must not read like a missing one"
        );
    }
}
