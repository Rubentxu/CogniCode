//! Construccion del grafo del proyecto.
//!
//! Las tres vias de produccion —la completa, la filtrada y la asincrona— y las dos variantes por fichero. Comparten el inventario centralizado y el lector de ficheros, asi que comparten su forma.

#![allow(
    clippy::doc_overindented_list_items,
    clippy::len_zero,
    clippy::type_complexity,
    clippy::useless_vec,
    unused_comparisons
)]

use super::AnalysisService;
use crate::application::dto::GraphCoverageMetrics;
use crate::application::error::{AppError, AppResult};
use crate::domain::aggregates::CallGraph;
use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::traits::DependencyRepository;
use crate::domain::value_objects::{DependencyType, WalkFilter};
use crate::infrastructure::graph::PetGraphStore;
use crate::infrastructure::graph::per_file_graph::{
    GlobalSymbolIndex, TargetResolution, WalkClassification, classify_walk_extension,
};
use crate::infrastructure::parser::{Language, TreeSitterParser};
use sha2::{Digest, Sha256};
use std::path::Path;
use tracing::{debug, info, warn};

/// PRF audit H-01 (F2.W9): SHA-256 of the file bytes, used as a
/// cache-invalidation key when (mtime, size) match but bytes may have
/// changed (e.g. editor rewrites without re-save metadata updates).
///
/// Returns a 32-byte digest; collision-resistance is the only property we
/// need (cache lookup, not security). Switching to BLAKE3 or xxhash is a
/// one-line change here and at the type signature in `AnalysisService`.
fn compute_content_hash(source: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let out = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&out);
    hash
}

/// What one file declares, read the way the product reads it.
///
/// A language that has a reader of its own is read by that reader, so the
/// graph names a Terraform block or an Ansible task the way the tools that
/// ask about them do. Every other language is read the generic way, and a
/// file the generic reader cannot parse is an error the caller turns into a
/// skipped file.
///
/// A reader of its own that falls over is not a file that declares nothing:
/// the file was not read, and the caller turns that into a skipped file
/// with the reader's own reason.
fn read_file_facts(
    language: Language,
    source: &str,
    file_path: &str,
) -> Result<crate::application::ingest::types::FileFacts, crate::domain::traits::ParseError> {
    let config =
        crate::infrastructure::parser::language_config::LanguageConfig::for_language(language);
    read_file_facts_with(config, source, file_path)
}

/// What one file declares, read by the reader it is handed.
///
/// The reader is a parameter because the reader is what decides what the
/// file means, and a caller that holds one must be able to hand it over.
pub(super) fn read_file_facts_with(
    config: &crate::infrastructure::parser::language_config::LanguageConfig,
    source: &str,
    file_path: &str,
) -> Result<crate::application::ingest::types::FileFacts, crate::domain::traits::ParseError> {
    match crate::infrastructure::graph::semantic_layer::semantic_facts_of(config, source, file_path)
    {
        crate::application::ingest::types::SemanticOutcome::Handled(facts) => return Ok(facts),
        // The reader does not apply to this file: the generic reader
        // answers it.
        crate::application::ingest::types::SemanticOutcome::NotApplicable => {}
        // The reader fell over. The file was not read, and the reason why
        // is the reader's own: the caller turns this into a skipped file
        // with that reason, not into a file with nothing in it.
        crate::application::ingest::types::SemanticOutcome::Failed(cause) => {
            return Err(crate::domain::traits::ParseError::InvalidSource(format!(
                "{file_path}: {cause}"
            )));
        }
    }
    let parser = TreeSitterParser::with_cache(config.language)?;
    // The tree is parsed once and read twice: the symbols and the calls
    // come out of the very tree the soundness check looks at.
    let tree = parser.parse_tree(source)?;
    // A tree with error nodes is a file the reader did not understand.
    // Publishing what a broken tree happens to hold would be a partial
    // answer dressed as a whole one, so the file is reported as not
    // read — the same thing the per-file route does with it.
    if TreeSitterParser::has_error_nodes(&tree) {
        return Err(crate::domain::traits::ParseError::InvalidSource(format!(
            "Syntax errors in {file_path}"
        )));
    }
    let symbols = parser.find_all_symbols_in_tree(&tree, source, file_path)?;
    let calls = parser.find_call_relationships_in_tree(&tree, source, file_path)?;
    Ok(crate::application::ingest::types::FileFacts {
        symbols,
        identified: Vec::new(),
        calls,
        pending: Vec::new(),
    })
}

impl AnalysisService {
    /// Builds the full project graph explicitly.
    ///
    /// This method constructs the complete call graph for the project,
    /// parsing all source files and building all symbol relationships.
    /// Use this when you need the complete graph for global analysis.
    pub fn build_full_graph(&self, project_dir: &Path) -> AppResult<()> {
        self.build_project_graph(project_dir)
    }

    pub fn build_project_graph(&self, project_dir: &Path) -> AppResult<()> {
        use rayon::prelude::*;

        info!("Building project graph for directory: {:?}", project_dir);

        // A root that does not exist is not a project with zero files: it
        // is a project that was never there. Without this, the walk below
        // yields one `Err` entry that `.filter_map(|e| e.ok())` discards,
        // `files` comes out empty, and the build reports `Ok` with a
        // `Partial` status — so `cognicode graph full /nonexistent` exits
        // 0 having built nothing. The MCP `build_graph` handler already
        // rejects this case (`handlers/mod.rs`: "Directory does not exist"),
        // and two interfaces answering the same question differently is the
        // defect AGENTS.md §6 forbids; this puts the answer in the service
        // they share.
        if !project_dir.exists() {
            return Err(AppError::InvalidParameter(format!(
                "Directory does not exist: {}",
                project_dir.display()
            )));
        }

        let mut store = PetGraphStore::new();
        // F2.W7: replace the legacy `name_lower → SymbolId` map with a
        // `GlobalSymbolIndex` that supports scope-aware resolution
        // (see `infrastructure/graph/per_file_graph.rs::GlobalSymbolIndex`
        // — F2.W5 introduced it but never wired it into the real
        // binary path, which goes through `build_project_graph`).
        let mut global_index =
            crate::infrastructure::graph::per_file_graph::GlobalSymbolIndex::new();

        // Coverage tracking
        let mut parsed_files: usize = 0;
        let mut cached_files: usize = 0;
        let mut unresolved_edges: usize = 0;

        // F2.W8: collect skipped files instead of dropping them
        // silently. The closure inside `filter_map` would otherwise
        // use `?` to short-circuit on read/parse errors, leaving no
        // trace of the file. We accumulate the SkippedFile records
        // in an Arc<Mutex<...>> because the iterator runs in
        // parallel and the closure captures it by reference.
        let skipped_files: std::sync::Arc<
            std::sync::Mutex<Vec<crate::infrastructure::graph::per_file_graph::SkippedFile>>,
        > = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        // The files the graph is built from are decided in one place, so
        // the manifest that says whether the graph is still fresh covers
        // exactly the same set. Two rules would drift, and a drifted
        // manifest serves a graph that no longer describes the tree.
        let inventory = crate::infrastructure::graph::source_files::inventory(project_dir);
        {
            // What the walk DECLINED (a symlink it refused to follow)
            // and what the walk could not DO (a directory it could not
            // read, an entry it could not measure) are both holes in
            // the coverage, so both travel with the report. The K0 gap
            // is decided by the classifier that owns it, and the
            // inventory already applied it: a second copy of that
            // rule here is a second source of truth waiting to drift.
            let mut guard = skipped_files.lock().unwrap();
            guard.extend(inventory.declined.iter().cloned());
            guard.extend(inventory.errors.iter().cloned());
            guard.extend(inventory.known_gaps());
        }

        let files: Vec<_> = inventory
            .files
            .into_iter()
            .map(|file| {
                let file_path = file.path.to_string_lossy().into_owned();
                let mtime = file.mtime_secs();
                let size = file.size;
                (file.path, file.language, file_path, mtime, size)
            })
            .collect();

        let total_files = files.len();
        let files_with_lang: usize = files
            .iter()
            .filter(|(_, lang, _, _, _)| lang.is_some())
            .count();

        info!(
            "WalkBuilder found {} files, {} have recognized language extensions",
            total_files, files_with_lang
        );

        if files_with_lang == 0 && total_files > 0 {
            warn!(
                "build_project_graph: stage=walk — {} total files but 0 recognized languages. \
                 Check that source files have supported extensions (.rs, .py, .go, .java, .ts, .js, etc.)",
                total_files
            );
        }

        // Log first few files for debugging
        for (path, lang, _, _, _) in files.iter().take(5) {
            debug!(
                "  Found file: {:?} with language: {:?}",
                path.file_name(),
                lang
            );
        }

        // U15 (PRF_ANA_05 corpus mixto LSP-ausente): archivos sin
        // lenguaje reconocido deben aparecer EXPLÍCITAMENTE en
        // BuildReport.skipped, no filtrarse silenciosamente. Sin este
        // paso, un cliente que ejecute `cognicode graph full` sobre
        // corpus mixto recibiría status=Complete aunque el WalkBuilder
        // descubrió archivos que nunca se procesaron.
        //
        // MEDIDO 2026-10-19. La condición era `language.is_none()`, un
        // predicado binario que decía "no hay parser" para un README y
        // para un lockfile igual que para un `.kt`. Los tres acababan en
        // `skipped`, y `skipped` no vacío es `BuildStatus::Partial`; como
        // `mod.rs:1312` BORRA el snapshot durable cuando el build no es
        // `Complete`, un repo Rust normal —Cargo.toml y README incluidos—
        // nunca persistía nada. MEDIDO: 2 "huecos" falsos frente a los 0
        // reales de la ruta per-file sobre el mismo árbol.
        //
        // Ese arreglo cayó en un segundo clasificador COPIADO aquí: el
        // bloque que se quitó de este sitio reimplementaba
        // `classify_walk_extension` a medias y decidía otra vez qué es
        // hueco. Hoy el inventario decide —con el clasificador que es su
        // dueño— y esta ruta sólo lo publica: `inventory.known_gaps()`
        // son los huecos honestos, y lo que es una exclusión legítima
        // no se registra porque no es un hueco. Un segundo clasificador
        // es una segunda fuente de verdad, y las dos se separan solas.
        //
        // Lo que queda aquí es el filtro de lo que el grafo analiza: lo
        // que el registro reconoce. El hueco ya está publicado arriba.

        let files: Vec<_> = files
            .into_iter()
            .filter(|(_, lang, _, _, _)| lang.is_some())
            .collect();

        let results: Vec<_> = files
            .into_par_iter()
            .filter_map(|(path, language, file_path, mtime, size)| {
                let language = language.unwrap();
                let path_for_skip = file_path.clone();
                let skipped = skipped_files.clone();

                {
                    // PRF audit H-01: invalidate on mtime OR size OR
                    // content_hash mismatch. The hash covers the case
                    // where bytes change while mtime AND size are
                    // preserved (e.g., editor rewrite without re-save
                    // metadata change). To compare the hash, we must
                    // read the file once; the cache saves only the
                    // parse cost (TreeSitter), not the I/O cost.
                    let source = match std::fs::read_to_string(&path) {
                        Ok(s) => s,
                        Err(e) => {
                            skipped.lock().unwrap().push(
                                crate::infrastructure::graph::per_file_graph::SkippedFile {
                                    path: path_for_skip.clone(),
                                    reason: crate::infrastructure::graph::per_file_graph::classify_io_error(&e),
                                },
                            );
                            return None;
                        }
                    };
                    let content_hash = compute_content_hash(&source);

                    let cache = self.file_cache.lock().unwrap();
                    if let Some((cached_mtime, cached_size, cached_hash, cached_facts)) =
                        cache.get(&file_path).cloned()
                        && cached_mtime == mtime
                        && cached_size == size
                        && cached_hash == content_hash
                    {
                        drop(cache);
                        return Some((
                            file_path,
                            mtime,
                            size,
                            content_hash,
                            cached_facts,
                            false, // from_cache = true
                        ));
                    }
                    drop(cache);

                    // Cache miss path: read the source we already read.
                    // A file whose reader fails is classified as skipped,
                    // never silently defaulted to empty results.
                    let facts = match read_file_facts(language, &source, &file_path) {
                        Ok(facts) => facts,
                        Err(e) => {
                            skipped.lock().unwrap().push(
                                crate::infrastructure::graph::per_file_graph::SkippedFile {
                                    path: file_path.clone(),
                                    reason: crate::infrastructure::graph::per_file_graph::classify_parse_error(&e),
                                },
                            );
                            return None;
                        }
                    };

                    Some((
                        file_path,
                        mtime,
                        size,
                        content_hash,
                        facts,
                        true, // from_cache = false (parsed)
                    ))
                }
            })
            .collect();

        // Stage: parse — count what we got from the parallel extraction
        let files_cached = results
            .iter()
            .filter(|(_, _, _, _, _, was_parsed)| !was_parsed)
            .count();
        let files_parsed = results.len() - files_cached;
        let total_symbols_extracted: usize = results
            .iter()
            .map(|(_, _, _, _, facts, _)| facts.symbols.len())
            .sum();
        let total_relationships_found: usize = results
            .iter()
            .map(|(_, _, _, _, facts, _)| facts.identified.len() + facts.calls.len())
            .sum();

        info!(
            "build_project_graph: stage=parse — {} files ({} parsed, {} cached), {} symbols, {} relationships",
            results.len(),
            files_parsed,
            files_cached,
            total_symbols_extracted,
            total_relationships_found
        );

        if files_parsed == 0 && files_cached == 0 && files_with_lang > 0 {
            warn!(
                "build_project_graph: stage=parse — {} language files found but 0 parsed or cached.                  Possible tree-sitter parsing failure.",
                files_with_lang
            );
        }

        let mut cache = self.file_cache.lock().unwrap();
        // F2.W7: collect (caller_fqn, caller_file, callee_name) per
        // relationship so the resolver has both the caller SymbolId
        // AND the caller-side path context to apply scope-aware rules.
        // Before F2.W7 this was just `(caller_symbol, callee_name)` and
        // the resolution lost the file path of the caller.
        let mut all_relationships: Vec<(String, String, String)> = Vec::new();
        let mut identified_relations: Vec<crate::application::ingest::types::FileRelation> =
            Vec::new();
        // The references the files state and nobody declared. They are not
        // relations yet: the project may still be able to tie them to a
        // declaration in another file.
        let mut pending_references: Vec<crate::domain::value_objects::PendingReference> =
            Vec::new();

        // ENGINE-DET (e38.2): rayon's parallel collection order is
        // nondeterministic, so the results are folded in deterministic
        // `file_path` order — the index below must never depend on
        // directory-walk order.
        let mut results = results;
        results.sort_by(|a, b| a.0.cmp(&b.0));

        for (file_path, mtime, size, content_hash, facts, was_parsed) in results {
            if was_parsed {
                parsed_files += 1;
            } else {
                cached_files += 1;
            }
            // PRF audit H-01: cache entries keyed by (mtime, size,
            // content_hash). The hash covers the case where bytes
            // change while mtime AND size stay the same (e.g.,
            // editor rewrite without re-save metadata change).
            cache.insert(
                file_path.clone(),
                (mtime, size, content_hash, facts.clone()),
            );

            for symbol in facts.symbols {
                let symbol_id = SymbolId::new(symbol.fully_qualified_name());
                store.add_symbol_with_location(&symbol_id, symbol.clone());
                // F2.W7: feed every symbol into the GlobalSymbolIndex
                // regardless of how many homonyms exist. Resolution
                // happens later, with caller-file context.
                global_index.insert(
                    symbol_id,
                    std::path::PathBuf::from(&file_path),
                    symbol.name(),
                );
            }

            // A relation whose two ends are already identified is not
            // looked up by name: the reader that produced it already
            // knew both ends.
            identified_relations.extend(facts.identified.iter().cloned());

            // A reference whose far end is not identified is kept, with its
            // origin: the project may still be able to tie it to a
            // declaration in another file, and if it cannot, the consumer
            // has to hear about it.
            pending_references.extend(facts.pending.iter().cloned());

            for (caller, callee_name) in facts.calls {
                // Preserve the caller's file AND FQN so the resolver can
                // apply the "same file" / "same crate root"
                // disambiguation AND recover the caller SymbolId.
                let caller_fqn = caller.fully_qualified_name().to_string();
                let caller_file = caller.location().file().to_string();
                all_relationships.push((caller_fqn, caller_file, callee_name));
            }
        }

        // The relations whose ends are already identified go in now
        // that every symbol of every file is in the store. They needed no
        // lookup, so they are resolved relations and count as such.
        let identified_count = identified_relations.len();
        for relation in identified_relations {
            store
                .add_dependency(&relation.caller, &relation.callee, relation.kind)
                .ok();
        }

        // The references the files could not tie to a declaration are looked
        // up now that every symbol of every file is in the index: the
        // declaration they name may live in another file of the project.
        // What the lookup proves becomes a relation; what it cannot prove
        // stays pending and travels with the report.
        let pending_resolution = crate::infrastructure::graph::pending::resolve_pending_references(
            pending_references,
            &global_index,
        );
        for (source_id, target_id, kind) in &pending_resolution.proven {
            store.add_dependency(source_id, target_id, *kind).ok();
        }

        let total_relationships = all_relationships.len() + identified_count;
        for (caller_fqn, caller_file, callee_name) in all_relationships {
            let caller_id = SymbolId::new(&caller_fqn);
            let name_lower = callee_name.to_lowercase();
            let caller_path = std::path::Path::new(&caller_file);
            match global_index.resolve(&name_lower, Some(caller_path)) {
                TargetResolution::Resolved(callee_id) => {
                    store
                        .add_dependency(&caller_id, &callee_id, DependencyType::Calls)
                        .ok();
                }
                // Both refusals mean the same thing to this statistic: no
                // edge was created. They are now distinguishable upstream,
                // but `unresolved_edges` is published in the summary, and
                // splitting it there is a report change, not a resolver
                // one — H4.2.
                TargetResolution::Ambiguous { .. } | TargetResolution::Unresolved(_) => {
                    unresolved_edges += 1;
                }
            }
        }

        // Stage: resolve — log edge resolution outcome
        info!(
            "build_project_graph: stage=resolve — {} relationships, {} resolved, {} unresolved",
            total_relationships + pending_resolution.proven.len(),
            (total_relationships + pending_resolution.proven.len())
                .saturating_sub(unresolved_edges),
            unresolved_edges
        );

        // F2.W8: drain skipped files before building the call graph.
        let skipped_vec = {
            let guard = skipped_files.lock().unwrap();
            guard.clone()
        };
        let call_graph = store.to_call_graph();
        // The status carries the two coverages: the files the walk could not
        // process, and the references it could not tie to a declaration.
        // "Complete" is only said when both of them are whole.
        let status = if skipped_vec.is_empty() && pending_resolution.pending.is_empty() {
            crate::infrastructure::graph::per_file_graph::BuildStatus::Complete
        } else {
            crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                skipped: skipped_vec,
                pending: pending_resolution.pending,
            }
        };
        let report = crate::infrastructure::graph::per_file_graph::BuildReport {
            graph: call_graph.clone(),
            status,
        };
        *self.last_build_report.lock().unwrap() = Some(report);
        let symbol_count = call_graph.symbol_count();
        let edge_count = call_graph.edge_count();

        // Stage: done — final graph with per-language breakdown
        let mut lang_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for symbol in call_graph.symbols() {
            let file = symbol.location().file();
            let lang = std::path::Path::new(file)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown")
                .to_string();
            *lang_counts.entry(lang).or_default() += 1;
        }
        info!(
            "build_project_graph: stage=done — {} symbols, {} edges ({} parsed files, {} unresolved). Languages: {:?}",
            symbol_count, edge_count, parsed_files, unresolved_edges, lang_counts
        );

        if symbol_count == 0 {
            warn!(
                "No symbols extracted! total_files={}, parsed_files={}",
                total_files, parsed_files
            );
        }

        self.graph_cache.replace(call_graph);

        // Update coverage metrics. The denominator is the source files the
        // graph was built from — a file with no recognized language was
        // never a candidate — and the numerator is every one of them the
        // graph has facts for, parsed again or reused from the cache.
        let coverage = GraphCoverageMetrics::new(
            files_with_lang,
            parsed_files,
            cached_files,
            unresolved_edges,
        );
        *self.coverage_metrics.lock().unwrap() = Some(coverage);

        Ok(())
    }

    /// Builds the project graph using the named strategy.
    ///
    /// | Strategy     | Behaviour                                                         |
    /// |--------------|-------------------------------------------------------------------|
    /// | `"full"`     | Full rayon-parallel parse of all source files (default)           |
    /// | `"lightweight"` | Lightweight symbol-only index, faster but no call edges        |
    /// | `"per_file"` | Per-file graphs merged into one; good for incremental workloads   |
    /// | `"on_demand"` | Alias for `"full"` — on-demand sub-graphs are query-time only   |
    /// | anything else | Falls back to `"full"`                                           |
    ///
    /// All strategies update `graph_cache` so that `get_project_graph()` reflects
    /// the result immediately after this call returns.
    pub fn build_graph_with_strategy(&self, project_dir: &Path, strategy: &str) -> AppResult<()> {
        match strategy {
            "lightweight" => self.build_graph_lightweight(project_dir),
            "per_file" => self.build_graph_per_file(project_dir),
            // "full", "on_demand", or any other value
            _ => self.build_project_graph(project_dir),
        }
    }

    /// Lightweight strategy: builds a symbol-only index (no call edges) using
    /// `LightweightIndex::build_index`, then converts the index entries into a
    /// minimal `CallGraph` (symbols only, 0 edges) and stores it in `graph_cache`.
    ///
    /// Significantly faster than the full strategy for large codebases when only
    /// symbol lookup is required (e.g. `query_symbol_index`).
    fn build_graph_lightweight(&self, project_dir: &Path) -> AppResult<()> {
        use crate::domain::aggregates::Symbol;
        use crate::domain::aggregates::call_graph::SymbolId;
        use crate::domain::value_objects::Location;
        use crate::infrastructure::graph::LightweightIndex;

        let mut index = LightweightIndex::new();
        index
            .build_index(project_dir)
            .map_err(|e| AppError::AnalysisError(e.to_string()))?;

        // Convert index entries to a minimal CallGraph (symbols, no call edges)
        let mut store = PetGraphStore::new();
        for (name, locations) in index.entries() {
            for loc in locations {
                let sym_kind = loc.symbol_kind;
                let location = Location::new(loc.file.clone(), loc.line, loc.column);
                let symbol = Symbol::new(name.clone(), sym_kind, location);
                let id = SymbolId::new(symbol.fully_qualified_name());
                store.add_symbol_with_location(&id, symbol);
            }
        }
        let graph = store.to_call_graph();
        self.graph_cache.replace(graph);
        Ok(())
    }

    /// Per-file strategy: builds a per-file graph for every source file in
    /// `project_dir` and merges them into a single `CallGraph` stored in
    /// `graph_cache`.
    ///
    /// Useful when incremental, file-level invalidation is important.
    ///
    /// H1.3 (delivery-first-radical, package integration): this is now
    /// a thin shim over `build_graph_per_file_with_report`. The
    /// typed method is the single source of truth; the legacy method
    /// keeps the `AppResult<()>` signature for callers that do not
    /// need the report. `get_last_build_report()` is updated either
    /// way.
    pub fn build_graph_per_file(&self, project_dir: &Path) -> AppResult<()> {
        self.build_graph_per_file_with_report(project_dir)
            .map(|_| ())
    }

    /// Per-file strategy with the H1.1 typed report. Walks the tree
    /// through [`WalkFilter::default`] (security blocklist + perf
    /// skips), routes every file through `classify_walk_extension`,
    /// feeds the `Analyze` set to `PerFileGraphCache::merge_with_report`,
    /// and records the K0 honest gap (`.kt`/`.kts`) as
    /// `SkipReason::UnsupportedExtension` so the consumer can see the
    /// K0 bridge instead of a silent discard.
    ///
    /// The returned `BuildReport` is also stored in `last_build_report`
    /// and `graph_cache` is updated with the merged graph. The legacy
    /// `build_graph_per_file` is a shim over this.
    pub fn build_graph_per_file_with_report(
        &self,
        project_dir: &Path,
    ) -> AppResult<crate::infrastructure::graph::per_file_graph::BuildReport> {
        use crate::infrastructure::graph::PerFileGraphCache;
        use crate::infrastructure::graph::per_file_graph::{
            BuildReport as PfgBuildReport, BuildStatus as PfgBuildStatus,
            SkipReason as PfgSkipReason, SkippedFile as PfgSkippedFile,
        };

        let cache = PerFileGraphCache::with_project_dir(project_dir.to_string_lossy().into_owned());
        let walk = WalkFilter::default();

        let mut file_paths: Vec<std::path::PathBuf> = Vec::new();
        let mut k0_skipped: Vec<PfgSkippedFile> = Vec::new();
        let mut walk_skipped: Vec<PfgSkippedFile> = Vec::new();

        for entry in crate::infrastructure::walk_policy::contained_walk(project_dir)
            .into_iter()
            .filter_entry(|e| !walk.matches_any_component(e.path(), project_dir))
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    // An error of the walk is not a file it left out on
                    // purpose: it is a hole in the coverage, and it
                    // travels with the report like any other.
                    walk_skipped.push(crate::infrastructure::graph::per_file_graph::walk_error(
                        err.path(),
                        &err,
                    ));
                    continue;
                }
            };
            let path = entry.path();
            if !crate::infrastructure::walk_policy::is_real_file(&entry) {
                // This method exists so a caller can see what it did
                // not cover. Dropping the symlink without recording it
                // would return `Complete`, which is the same lie the
                // walk was fixed to stop telling, one layer down.
                if let Some(reason) =
                    crate::infrastructure::graph::per_file_graph::classify_declined_entry(&entry)
                {
                    walk_skipped.push(PfgSkippedFile {
                        path: path.to_string_lossy().into_owned(),
                        reason,
                    });
                }
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str());
            match classify_walk_extension(ext) {
                WalkClassification::Analyze => file_paths.push(path.to_path_buf()),
                WalkClassification::KnownUnsupportedGap => {
                    // K0 honest gap: the file is a real source in a
                    // language we know about but have not yet wired
                    // (Kotlin). The H1.1 envelope requires it to
                    // surface in `skipped` so the consumer can see
                    // the K0 bridge, not a silent "no findings".
                    k0_skipped.push(PfgSkippedFile {
                        path: path.to_string_lossy().into_owned(),
                        reason: PfgSkipReason::UnsupportedExtension(ext.unwrap_or("").to_string()),
                    });
                }
                WalkClassification::LegitExclusion => {
                    // README, image, lockfile: outside the analysis
                    // scope by policy. Not a gap, not a skip — just
                    // not part of the answer.
                }
            }
        }

        let file_path_refs: Vec<&Path> = file_paths.iter().map(|p| p.as_path()).collect();
        let cache_report = cache.merge_with_report(&file_path_refs);

        // Merge the per-file cache's own report (.rs parse/read
        // failures) with our K0 honest-gap skips. The envelope
        // collapses to Complete only when BOTH lists are empty.
        let (graph, mut all_skipped, all_pending) = match cache_report.status {
            PfgBuildStatus::Complete => (cache_report.graph, Vec::new(), Vec::new()),
            PfgBuildStatus::Partial { skipped, pending } => (cache_report.graph, skipped, pending),
        };
        all_skipped.extend(k0_skipped);
        all_skipped.extend(walk_skipped);
        let final_status = if all_skipped.is_empty() && all_pending.is_empty() {
            PfgBuildStatus::Complete
        } else {
            PfgBuildStatus::Partial {
                skipped: all_skipped,
                pending: all_pending,
            }
        };
        let final_report = PfgBuildReport {
            graph,
            status: final_status,
        };

        *self.last_build_report.lock().unwrap() = Some(final_report.clone());
        self.graph_cache.replace(final_report.graph.clone());
        Ok(final_report)
    }

    /// Builds the project graph for the directories in `include_dirs`,
    /// and publishes what it could not cover.
    ///
    /// This route is the whole-project one with a filter over the
    /// files: it walks with the same inventory, classifies with the
    /// same classifier, and publishes the same kind of report. Two
    /// routes of one product that give two different guarantees are one
    /// defect waiting to be found, so this one says what it covered too.
    ///
    /// # Arguments
    /// * `project_dir` - The project directory to scan
    /// * `include_dirs` - Directories to include (as an allowlist). Files outside
    ///                   these directories are excluded.
    ///
    /// # Returns
    /// * `AppResult<BuildReport>` - The graph of the included
    ///   directories, with the report of what was not covered.
    pub fn build_project_graph_filtered(
        &self,
        project_dir: &Path,
        include_dirs: &[&Path],
    ) -> AppResult<crate::infrastructure::graph::per_file_graph::BuildReport> {
        use rayon::prelude::*;

        // A root that does not exist is not a project with zero files: it
        // is a project that was never there. Without this, the walk yields
        // nothing, the graph comes out empty and the route answers `Ok`
        // with a `Complete` status — a claim of completeness about a
        // tree nobody walked. The whole-project route says so, and two
        // routes answering the same question differently is the defect
        // this file exists to prevent.
        if !project_dir.exists() {
            return Err(AppError::InvalidParameter(format!(
                "Directory does not exist: {}",
                project_dir.display()
            )));
        }

        let mut store = PetGraphStore::new();
        // The resolver keeps the multiconset of the homonyms and only
        // resolves when a rule demonstrates it, so a call is never
        // wired against whichever homonym the traversal happened to
        // reach first.
        let mut global_index = GlobalSymbolIndex::new();

        // Coverage tracking
        let mut parsed_files: usize = 0;
        let mut cached_files: usize = 0;
        let mut unresolved_edges: usize = 0;

        // The files are decided by the inventory the whole-project route
        // uses, so the two routes agree on what a source is, on what is
        // a gap and on what is a legitimate exclusion. The inventory
        // also keeps what the walk declined and what it could not do:
        // both are holes in the coverage, and both travel with the
        // report.
        let inventory = crate::infrastructure::graph::source_files::inventory(project_dir);
        let included = |path: &Path| include_dirs.iter().any(|dir| path.starts_with(dir));

        let mut skipped: Vec<crate::infrastructure::graph::per_file_graph::SkippedFile> =
            Vec::new();
        for entry in inventory.declined.iter().chain(inventory.errors.iter()) {
            if included(Path::new(&entry.path)) {
                skipped.push(entry.clone());
            }
        }
        for gap in inventory.known_gaps() {
            if included(Path::new(&gap.path)) {
                skipped.push(gap);
            }
        }

        let files: Vec<_> = inventory
            .files
            .into_iter()
            .filter(|file| included(&file.path))
            .filter(|file| file.language.is_some())
            .map(|file| {
                let file_path = file.path.to_string_lossy().into_owned();
                let mtime = file.mtime_secs();
                let size = file.size;
                (file.path, file.language, file_path, mtime, size)
            })
            .collect();

        let total_files = files.len();

        // The skips are collected from the parallel pass, so they live
        // behind a lock.
        let skipped: std::sync::Arc<
            std::sync::Mutex<Vec<crate::infrastructure::graph::per_file_graph::SkippedFile>>,
        > = std::sync::Arc::new(std::sync::Mutex::new(skipped));

        let results: Vec<_> = files
            .into_par_iter()
            .filter_map(|(path, language, file_path, mtime, size)| {
                let language = language.unwrap();
                let path_for_skip = file_path.clone();
                let skipped = skipped.clone();

                {
                    // A file that cannot be read is a hole in the
                    // coverage, and it is named in the report. It is
                    // never counted as if it had been read.
                    let source = match std::fs::read_to_string(&path) {
                        Ok(s) => s,
                        Err(e) => {
                            skipped.lock().unwrap().push(
                                crate::infrastructure::graph::per_file_graph::SkippedFile {
                                    path: path_for_skip.clone(),
                                    reason: crate::infrastructure::graph::per_file_graph::classify_io_error(&e),
                                },
                            );
                            return None;
                        }
                    };
                    let content_hash = compute_content_hash(&source);

                    let cache = self.file_cache.lock().unwrap();
                    if let Some((cached_mtime, cached_size, cached_hash, cached_facts)) =
                        cache.get(&file_path).cloned()
                        && cached_mtime == mtime
                        && cached_size == size
                        && cached_hash == content_hash
                    {
                        drop(cache);
                        return Some((
                            file_path,
                            mtime,
                            size,
                            content_hash,
                            cached_facts,
                            false, // from_cache = true
                        ));
                    }
                    drop(cache);

                    // A file whose reader fails is excluded from parsed
                    // coverage and reported, never silently counted as
                    // parsed with empty results.
                    let facts = match read_file_facts(language, &source, &file_path) {
                        Ok(facts) => facts,
                        Err(e) => {
                            skipped.lock().unwrap().push(
                                crate::infrastructure::graph::per_file_graph::SkippedFile {
                                    path: file_path.clone(),
                                    reason: crate::infrastructure::graph::per_file_graph::classify_parse_error(&e),
                                },
                            );
                            return None;
                        }
                    };

                    Some((
                        file_path,
                        mtime,
                        size,
                        content_hash,
                        facts,
                        true, // from_cache = false (parsed)
                    ))
                }
            })
            .collect();

        // The order in which the files are folded is the order in which
        // the index receives their symbols, and the index reports the
        // candidates of a repeated name in the order it received them. A
        // directory listing is the machine's order, not the project's,
        // so the files are folded in path order: what the graph reports
        // must not depend on where the tree happens to live.
        let mut results = results;
        results.sort_by(|a, b| a.0.cmp(&b.0));

        let mut cache = self.file_cache.lock().unwrap();
        let mut all_relationships: Vec<(String, String, String)> = Vec::new();
        let mut identified_relations: Vec<crate::application::ingest::types::FileRelation> =
            Vec::new();
        // The references the files state and nobody declared. They are
        // not relations yet: the project may still be able to tie them
        // to a declaration in another file.
        let mut pending_references: Vec<crate::domain::value_objects::PendingReference> =
            Vec::new();

        for (file_path, mtime, size, content_hash, facts, was_parsed) in results {
            if was_parsed {
                parsed_files += 1;
            } else {
                cached_files += 1;
            }
            cache.insert(
                file_path.clone(),
                (mtime, size, content_hash, facts.clone()),
            );

            for symbol in facts.symbols {
                let symbol_id = SymbolId::new(symbol.fully_qualified_name());
                store.add_symbol_with_location(&symbol_id, symbol.clone());
                global_index.insert(
                    symbol_id,
                    std::path::PathBuf::from(&file_path),
                    symbol.name(),
                );
            }

            // A relation whose two ends are already identified is not
            // looked up by name: the reader that produced it already
            // knew both ends.
            identified_relations.extend(facts.identified.iter().cloned());

            // A reference whose far end is not identified is kept, with
            // its origin: the project may still be able to tie it to a
            // declaration in another file, and if it cannot, the
            // consumer has to hear about it.
            pending_references.extend(facts.pending.iter().cloned());

            for (caller, callee_name) in facts.calls {
                // The file of the caller is what allows the rules
                // "same file" and "same crate root".
                all_relationships.push((
                    caller.fully_qualified_name().to_string(),
                    caller.location().file().to_string(),
                    callee_name,
                ));
            }
        }

        // The relations whose ends are already identified go in now
        // that every symbol of every file is in the store.
        for relation in identified_relations {
            store
                .add_dependency(&relation.caller, &relation.callee, relation.kind)
                .ok();
        }

        // The references the files could not tie to a declaration are
        // looked up now that every symbol of every file is in the
        // index: the declaration they name may live in another file of
        // the project. What the lookup proves becomes a relation; what
        // it cannot prove stays pending and travels with the report.
        let pending_resolution = crate::infrastructure::graph::pending::resolve_pending_references(
            pending_references,
            &global_index,
        );
        for (source_id, target_id, kind) in &pending_resolution.proven {
            store.add_dependency(source_id, target_id, *kind).ok();
        }

        for (caller_fqn, caller_file, callee_name) in all_relationships {
            let caller_id = SymbolId::new(&caller_fqn);
            let caller_path = std::path::Path::new(&caller_file);
            match global_index.resolve(&callee_name.to_lowercase(), Some(caller_path)) {
                TargetResolution::Resolved(callee_id) => {
                    store
                        .add_dependency(&caller_id, &callee_id, DependencyType::Calls)
                        .ok();
                }
                // Sin destino probado no hay arista: ambigua y ausente se
                // cuentan igual porque ambas dejan el grafo igual de
                // completo, y distinguirlas es trabajo de reporte, no del
                // grafo.
                TargetResolution::Ambiguous { .. } | TargetResolution::Unresolved(_) => {
                    unresolved_edges += 1;
                }
            }
        }

        let call_graph = store.to_call_graph();

        // The status carries the two coverages: the files the walk could
        // not process, and the references it could not tie to a
        // declaration. "Complete" is only said when both of them are
        // whole.
        let skipped_vec = {
            let guard = skipped.lock().unwrap();
            guard.clone()
        };
        let status = if skipped_vec.is_empty() && pending_resolution.pending.is_empty() {
            crate::infrastructure::graph::per_file_graph::BuildStatus::Complete
        } else {
            crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                skipped: skipped_vec,
                pending: pending_resolution.pending,
            }
        };

        // Update coverage metrics for this subgraph (separate from cached graph)
        let coverage =
            GraphCoverageMetrics::new(total_files, parsed_files, cached_files, unresolved_edges);
        *self.coverage_metrics.lock().unwrap() = Some(coverage);

        Ok(crate::infrastructure::graph::per_file_graph::BuildReport {
            graph: call_graph,
            status,
        })
    }

    /// Builds the project graph asynchronously without blocking the tokio runtime.
    ///
    /// This method wraps the synchronous `build_project_graph()` in `tokio::task::spawn_blocking()`
    /// to execute heavy file I/O and parsing on a blocking thread pool, keeping the async
    /// runtime responsive.
    ///
    /// # Arguments
    /// * `dir` - The project directory to scan
    ///
    /// # Returns
    /// * `AppResult<()>` - Success or error
    pub async fn build_project_graph_async(&self, dir: &Path) -> AppResult<()> {
        // A root that does not exist is not a project with zero files: it
        // is a project that was never there. The whole-project route says
        // so, and two routes answering the same question differently is
        // the defect this file exists to prevent.
        if !dir.exists() {
            return Err(AppError::InvalidParameter(format!(
                "Directory does not exist: {}",
                dir.display()
            )));
        }

        let graph_cache = self.graph_cache.clone();
        let file_cache = self.file_cache.clone();
        let dir_path = dir.to_path_buf();

        // Spawn blocking task to run the heavy computation. Devuelve tambien el
        // grafo y los huecos: el `BuildReport` se publica fuera del
        // closure, que exige `'static` y no puede tocar `self`.
        let result: Result<
            Result<
                (
                    GraphCoverageMetrics,
                    CallGraph,
                    Vec<crate::infrastructure::graph::per_file_graph::SkippedFile>,
                    Vec<crate::domain::value_objects::PendingReference>,
                ),
                AppError,
            >,
            tokio::task::JoinError,
        > = tokio::task::spawn_blocking(move || {
                // Create a minimal service context for building the graph
                let store =
                    std::sync::Mutex::new(crate::infrastructure::graph::PetGraphStore::new());
                // H4.2: mismo defecto que `build_project_graph_filtered` — un
                // `HashMap<String, SymbolId>` no distingue dos homónimos, así que
                // toda llamada se cableaba contra el que sobrevivió por orden de
                // recorrido. `GlobalSymbolIndex` guarda el multiconjunto y solo
                // resuelve cuando una regla lo demuestra.
                let mut global_index = GlobalSymbolIndex::new();

                // Coverage tracking
                let mut parsed_files: usize = 0;
                let mut cached_files: usize = 0;
                let mut unresolved_edges: usize = 0;

                use rayon::prelude::*;

                // The files the graph is built from are decided in one
                // place, so the three routes that build the graph decide
                // the same thing. This route used to walk on its own, and
                // that had two consequences: a symlink the walk declined
                // disappeared in silence —and a route that says `Complete`
                // over a tree it does not describe is the direction that
                // costs the most— and the rules of the walk had a second
                // copy that could drift from the one the inventory owns.
                let inventory = crate::infrastructure::graph::source_files::inventory(&dir_path);

                // What the walk DECLINED (a symlink it refused to follow)
                // and what the walk could not DO (a directory it could not
                // read, an entry it could not measure) are holes in the
                // coverage, and they travel with the report. The gap of a
                // language nobody supports yet is a hole too, and the
                // inventory already knows which files are one.
                let mut k0_skipped: Vec<crate::infrastructure::graph::per_file_graph::SkippedFile> =
                    Vec::new();
                k0_skipped.extend(inventory.declined.iter().cloned());
                k0_skipped.extend(inventory.errors.iter().cloned());
                k0_skipped.extend(inventory.known_gaps());

                // The denominator of the coverage is the files the graph
                // was built from: a file no language claims was never a
                // candidate. A README or a lockfile is a decision, not a
                // lack.
                let files: Vec<_> = inventory
                    .files
                    .into_iter()
                    .filter(|file| file.language.is_some())
                    .map(|file| {
                        let file_path = file.path.to_string_lossy().into_owned();
                        let mtime = file.mtime_secs();
                        let size = file.size;
                        (file.path, file.language, file_path, mtime, size)
                    })
                    .collect();

                let total_files = files.len();

                // A file the walk could not read or parse is a hole in the
                // file coverage, and it is recorded as one —the mismo
                // contrato que cumple la ruta sincrona—. Dejarlo caer en
                // silencio haria que las dos rutas dieran veredictos
                // distintos sobre el mismo arbol.
                let dropped: std::sync::Mutex<
                    Vec<crate::infrastructure::graph::per_file_graph::SkippedFile>,
                > = std::sync::Mutex::new(Vec::new());

                let results: Vec<_> = files
                    .into_par_iter()
                    .filter_map(|(path, language, file_path, mtime, size)| {
                        let language = language.unwrap();
                        let path_for_skip = file_path.clone();
                        let dropped = &dropped;

                        {
                            // PRF audit H-01: invalidate on mtime
                            // OR size OR content_hash mismatch.
                            let source = match std::fs::read_to_string(&path) {
                                Ok(source) => source,
                                Err(e) => {
                                    dropped.lock().unwrap().push(
                                        crate::infrastructure::graph::per_file_graph::SkippedFile {
                                            path: path_for_skip.clone(),
                                            reason: crate::infrastructure::graph::per_file_graph::classify_io_error(&e),
                                        },
                                    );
                                    return None;
                                }
                            };
                            let content_hash = compute_content_hash(&source);

                            let cache = file_cache.lock().unwrap();
                            if let Some((cached_mtime, cached_size, cached_hash, cached_facts)) =
                                cache.get(&file_path).cloned()
                                && cached_mtime == mtime
                                && cached_size == size
                                && cached_hash == content_hash
                            {
                                drop(cache);
                                return Some((
                                    file_path,
                                    mtime,
                                    size,
                                    content_hash,
                                    cached_facts,
                                    false, // from_cache = true
                                ));
                            }
                            drop(cache);

                            // Same contract as the sync builder: a file
                            // whose reader fails is excluded from parsed
                            // coverage and reported, never silently
                            // defaulted to empty results.
                            let facts = match read_file_facts(language, &source, &file_path) {
                                Ok(facts) => facts,
                                Err(e) => {
                                    tracing::warn!("reading {} failed: {}", file_path, e);
                                    dropped.lock().unwrap().push(
                                        crate::infrastructure::graph::per_file_graph::SkippedFile {
                                            path: path_for_skip.clone(),
                                            reason: crate::infrastructure::graph::per_file_graph::classify_parse_error(&e),
                                        },
                                    );
                                    return None;
                                }
                            };

                            Some((
                                file_path,
                                mtime,
                                size,
                                content_hash,
                                facts,
                                true, // from_cache = false (parsed)
                            ))
                        }
                    })
                    .collect();

                // The fold order must not depend on the order the machine
                // listed the files in: the index of repeated names keeps
                // the candidates in the order it received them, so another
                // order is another answer. The other two routes fold in
                // `file_path` order, and this one now does the same.
                let mut results = results;
                results.sort_by(|a, b| a.0.cmp(&b.0));

                let mut cache = file_cache.lock().unwrap();
                let mut all_relationships = Vec::new();
                let mut identified_relations: Vec<crate::application::ingest::types::FileRelation> =
                    Vec::new();
                let mut pending_references: Vec<crate::domain::value_objects::PendingReference> =
                    Vec::new();

                for (file_path, mtime, size, content_hash, facts, was_parsed) in results {
                    if was_parsed {
                        parsed_files += 1;
                    } else {
                        cached_files += 1;
                    }
                    // PRF audit H-01: cache entries keyed by
                    // (mtime, size, content_hash).
                    cache.insert(file_path.clone(), (mtime, size, content_hash, facts.clone()));

                    for symbol in facts.symbols {
                        let symbol_id = SymbolId::new(symbol.fully_qualified_name());
                        store
                            .lock()
                            .unwrap()
                            .add_symbol_with_location(&symbol_id, symbol.clone());
                        global_index.insert(
                            symbol_id,
                            std::path::PathBuf::from(&file_path),
                            symbol.name(),
                        );
                    }

                    // A relation whose two ends are already identified is
                    // not looked up by name: the reader that produced it
                    // already knew both ends.
                    identified_relations.extend(facts.identified.iter().cloned());

                    // A reference whose far end is not identified is kept,
                    // with its origin: the project may still be able to tie
                    // it to a declaration in another file.
                    pending_references.extend(facts.pending.iter().cloned());

                    for (caller, callee_name) in facts.calls {
                        // El fichero del llamante es lo que permite las reglas
                        // "mismo fichero" y "misma raíz de crate".
                        all_relationships.push((
                            caller.fully_qualified_name().to_string(),
                            caller.location().file().to_string(),
                            callee_name,
                        ));
                    }
                }

                // The relations whose ends are already identified go in now
                // that every symbol of every file is in the store.
                {
                    let mut guard = store.lock().unwrap();
                    for relation in identified_relations {
                        guard
                            .add_dependency(&relation.caller, &relation.callee, relation.kind)
                            .ok();
                    }
                }

                // The references the files could not tie to a declaration are
                // looked up now that every symbol of every file is in the
                // index: the declaration they name may live in another file
                // of the project. What the lookup proves becomes a relation;
                // what it cannot prove stays pending and travels with the
                // report. This is the same rule the synchronous path
                // applies, and both must apply it: two paths that answer
                // differently about the same project are two graphs.
                let pending_resolution =
                    crate::infrastructure::graph::pending::resolve_pending_references(
                        pending_references,
                        &global_index,
                    );
                for (source_id, target_id, kind) in &pending_resolution.proven {
                    store
                        .lock()
                        .unwrap()
                        .add_dependency(source_id, target_id, *kind)
                        .ok();
                }

                for (caller_fqn, caller_file, callee_name) in all_relationships {
                    let caller_id = SymbolId::new(&caller_fqn);
                    let caller_path = std::path::Path::new(&caller_file);
                    match global_index.resolve(&callee_name.to_lowercase(), Some(caller_path)) {
                        TargetResolution::Resolved(callee_id) => {
                            store
                                .lock()
                                .unwrap()
                                .add_dependency(&caller_id, &callee_id, DependencyType::Calls)
                                .ok();
                        }
                        TargetResolution::Ambiguous { .. } | TargetResolution::Unresolved(_) => {
                            unresolved_edges += 1;
                        }
                    }
                }

                let call_graph = store.into_inner().unwrap().to_call_graph();
                graph_cache.replace(call_graph.clone());

                // Compute coverage
                let coverage = GraphCoverageMetrics::new(
                    total_files,
                    parsed_files,
                    cached_files,
                    unresolved_edges,
                );

                // The files the walk could not read or parse are part of
                // the file coverage, and they travel with the report: a
                // build that dropped a file cannot say it read them all.
                k0_skipped.extend(dropped.lock().unwrap().iter().cloned());

                // And so does what the walk could not do at all: a
                // directory it could not read is a hole in the coverage,
                // and a hole that nobody counts is not a hole — it is a
                // claim of completeness that nothing backs. The inventory
                // already published them with the declined ones.

                Ok((coverage, call_graph, k0_skipped, pending_resolution.pending))
            })
            .await;

        let (coverage, call_graph, k0_skipped, k0_pending) = result
            .map_err(|e| AppError::AnalysisError(format!("Task join error: {}", e)))?
            .map_err(|e| AppError::AnalysisError(format!("Build error: {}", e)))?;

        // Store coverage metrics
        *self.coverage_metrics.lock().unwrap() = Some(coverage);

        // MEDIDO 2026-10-19: esta ruta no publicaba `BuildReport`, asi que
        // `get_last_build_report()` devolvia lo que hubiera dejado la ultima
        // ruta CORRIDA, no lo que acababa de pasar esta. `reparse_on_edit`
        // usa esta ruta, asi que tras editar un consumidor leia cobertura
        // pre-edicion. Se publica aqui, fuera del `spawn_blocking`, porque
        // el reporte pertenece al servicio y no a la tarea bloqueante.
        // El estado lleva las dos coberturas: los ficheros que el recorrido
        // no pudo procesar, y las referencias que no se pudieron atar a una
        // declaración. "Complete" sólo se dice cuando las dos están
        // completas; esta ruta no puede decir "complete" sobre una
        // cobertura de relaciones que no midió.
        let status = if k0_skipped.is_empty() && k0_pending.is_empty() {
            crate::infrastructure::graph::per_file_graph::BuildStatus::Complete
        } else {
            crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                skipped: k0_skipped,
                pending: k0_pending,
            }
        };
        *self.last_build_report.lock().unwrap() =
            Some(crate::infrastructure::graph::per_file_graph::BuildReport {
                graph: call_graph,
                status,
            });

        Ok(())
    }
}
