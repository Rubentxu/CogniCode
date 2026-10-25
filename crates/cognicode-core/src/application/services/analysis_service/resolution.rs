//! Resolucion de simbolos: donde se estan y donde se usan.
//!
//! Busqueda por nombre, usos de un simbolo y recorrido del grafo.

#![allow(
    clippy::doc_overindented_list_items,
    clippy::len_zero,
    clippy::type_complexity,
    clippy::useless_vec,
    unused_comparisons
)]

use super::AnalysisService;
use crate::application::dto::SymbolDto;
use crate::application::error::{AppError, AppResult};
use crate::domain::aggregates::CallGraph;
use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::value_objects::WalkFilter;
use crate::infrastructure::parser::{Language, TreeSitterParser};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct UsageSearchParams {
    pub project_dir: std::path::PathBuf,
    pub symbol_name: String,
    pub include_declaration: bool,
    pub context_lines: Option<usize>,
    pub first_only_definition: bool,
}

#[derive(Debug, Clone)]
pub struct UsageResult {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub context: String,
    pub is_definition: bool,
    pub context_lines: Option<ContextData>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextData {
    pub before: Vec<String>,
    pub current: String,
    pub after: Vec<String>,
}

/// Gets surrounding context lines for a given 0-indexed line
fn context_data(source: &str, line: usize, context_size: usize) -> ContextData {
    let lines: Vec<&str> = source.lines().collect();

    let before_start = line.saturating_sub(context_size);
    let after_end = (line + 1 + context_size).min(lines.len());

    let before: Vec<String> = (before_start..line)
        .map(|i| lines.get(i).unwrap_or(&"").to_string())
        .collect();

    let current = lines.get(line).unwrap_or(&"").to_string();

    let after: Vec<String> = ((line + 1)..after_end)
        .map(|i| lines.get(i).unwrap_or(&"").to_string())
        .collect();

    ContextData {
        before,
        current,
        after,
    }
}

impl AnalysisService {
    /// Finds symbols by name using the lightweight index.
    ///
    /// Returns all locations where the symbol is defined.
    pub fn find_symbol(
        &self,
        symbol_name: &str,
    ) -> Vec<crate::infrastructure::graph::SymbolLocation> {
        self.ensure_symbol_index().find_symbol(symbol_name).to_vec()
    }

    /// Extracts symbols from a file
    pub fn get_file_symbols(&self, path: &Path) -> AppResult<Vec<SymbolDto>> {
        // Read the source file
        let source = std::fs::read_to_string(path).map_err(|e| {
            AppError::InvalidParameter(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        // Detect language from file extension
        let language = Language::from_extension(path.extension()).ok_or_else(|| {
            AppError::InvalidParameter(format!("Unsupported file type: {}", path.display()))
        })?;

        // Create parser for the detected language (uses thread-local cache)
        let parser = TreeSitterParser::with_cache(language)
            .map_err(|e| AppError::AnalysisError(format!("Failed to create parser: {}", e)))?;

        // Parse the file and extract symbols
        let file_path = path.to_str().unwrap_or("unknown");
        let symbols = parser
            .find_all_symbols_with_path(&source, file_path)
            .map_err(|e| AppError::AnalysisError(format!("Failed to parse symbols: {}", e)))?;

        // Convert to DTOs
        Ok(symbols.iter().map(SymbolDto::from_symbol).collect())
    }

    /// Returns all symbols in the project graph with optional pagination.
    ///
    /// Returns symbols sorted by (file_path, line).
    pub fn get_all_symbols(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Vec<crate::application::dto::SymbolDto> {
        let graph = self.get_project_graph();

        // Collect all symbols and sort by (file_path, line)
        let mut symbols: Vec<_> = graph
            .symbols()
            .map(crate::application::dto::SymbolDto::from_symbol)
            .collect();

        symbols.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.line.cmp(&b.line)));

        // Apply pagination
        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(symbols.len());

        if offset >= symbols.len() {
            return Vec::new();
        }

        let end = std::cmp::min(offset + limit, symbols.len());
        symbols[offset..end].to_vec()
    }

    /// Traces an execution path between two symbols using BFS
    ///
    /// Returns the path from source to target if one exists, or None if no path
    /// is found. The path includes both source and target symbols.
    ///
    /// When max_depth is 0, no depth limit is applied.
    pub fn trace_path(
        &self,
        source_name: &str,
        target_name: &str,
        max_depth: usize,
    ) -> AppResult<Option<Vec<SymbolDto>>> {
        let graph = self.get_project_graph();

        // Find source and target symbol IDs by name
        let source_id = self.find_symbol_id_by_name(&graph, source_name)?;
        let target_id = self.find_symbol_id_by_name(&graph, target_name)?;

        // Find path using BFS, respecting max_depth if specified
        let path = if max_depth > 0 {
            graph.find_path_with_max_depth(&source_id, &target_id, max_depth)
        } else {
            graph.find_path(&source_id, &target_id)
        };

        Ok(path.map(|symbol_ids| {
            symbol_ids
                .iter()
                .filter_map(|id| graph.get_symbol(id).map(SymbolDto::from_symbol))
                .collect()
        }))
    }

    /// Finds a symbol ID by its name
    fn find_symbol_id_by_name(&self, graph: &CallGraph, name: &str) -> AppResult<SymbolId> {
        let name_lower = name.to_lowercase();

        // First try exact match on the fully qualified name
        for symbol in graph.symbols() {
            let fqn = symbol.fully_qualified_name().to_lowercase();
            if fqn == name_lower {
                return Ok(SymbolId::new(symbol.fully_qualified_name()));
            }
        }

        // Then try matching just the base name
        for symbol in graph.symbols() {
            if symbol.name().to_lowercase() == name_lower {
                return Ok(SymbolId::new(symbol.fully_qualified_name()));
            }
        }

        Err(AppError::SymbolNotFound(format!(
            "Symbol not found: {}",
            name
        )))
    }

    /// Finds all textual/AST occurrences of a symbol across the project
    /// source tree, skipping dependency/build/cache directories.
    pub fn find_symbol_usages(&self, params: UsageSearchParams) -> AppResult<Vec<UsageResult>> {
        let mut usages = Vec::new();
        let mut seen_first_definition = false;
        // H1.3: WalkFilter::default() is the canonical
        // security-blocklist + performance-skip set. The previous
        // local USAGE_SKIP_DIRS const duplicated only the perf
        // skips and missed `.env` and friends.
        let walk = WalkFilter::default();

        for entry in crate::infrastructure::walk_policy::contained_walk(&params.project_dir)
            .into_iter()
            .filter_entry(|e| !walk.matches_any_component(e.path(), &params.project_dir))
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !crate::infrastructure::walk_policy::is_real_file(&entry) {
                continue;
            }

            let language = match Language::from_extension(path.extension()) {
                Some(lang) => lang,
                None => continue,
            };

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let parser = match TreeSitterParser::new(language) {
                Ok(p) => p,
                Err(_) => continue,
            };

            if let Ok(occurrences) =
                parser.find_all_occurrences_of_identifier(&source, &params.symbol_name)
            {
                for occ in occurrences {
                    let has_def_keyword = occ.context.contains("def ")
                        || occ.context.contains("class ")
                        || occ.context.contains("struct ")
                        || occ.context.contains("fn ")
                        || occ.context.contains("function ")
                        || occ.context.contains("const ")
                        || occ.context.contains("let ");

                    let is_definition = if params.first_only_definition {
                        has_def_keyword && !seen_first_definition
                    } else {
                        has_def_keyword
                    };

                    if has_def_keyword && !seen_first_definition {
                        seen_first_definition = true;
                    }

                    if !params.include_declaration && is_definition {
                        continue;
                    }

                    let context_lines = params
                        .context_lines
                        .map(|ctx| context_data(&source, occ.line as usize, ctx));

                    usages.push(UsageResult {
                        file: path.to_string_lossy().into_owned(),
                        line: occ.line + 1,
                        column: occ.column,
                        context: occ.context.clone(),
                        is_definition,
                        context_lines,
                    });
                }
            }
        }

        Ok(usages)
    }
}
