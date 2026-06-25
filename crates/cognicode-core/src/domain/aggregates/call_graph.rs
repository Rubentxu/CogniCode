//! Aggregate for representing call graphs between symbols
//!
//! A call graph represents the dependencies and call relationships between symbols.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::symbol::Symbol;
use crate::domain::events::GraphEvent;
use crate::domain::services::{ConfidenceRules, ExtractionContext};
use crate::domain::value_objects::{DependencyType, Provenance};

/// Represents a call entry in traversal results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEntry {
    /// Symbol ID of the caller/callee
    pub symbol_id: SymbolId,
    /// Name of the symbol
    pub symbol_name: String,
    /// File location
    pub file: String,
    /// Line number
    pub line: u32,
    /// Column number
    pub column: u32,
    /// Depth from the starting symbol
    pub depth: u8,
}

/// Options for Mermaid diagram export
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MermaidOptions {
    /// Subgraph root symbol - if provided, only exports the subgraph rooted at this symbol
    pub root: Option<String>,
    /// Maximum depth for traversal when root is provided (default: 3)
    pub max_depth: u8,
    /// Theme for SVG rendering (used when format is "svg")
    pub theme: Option<String>,
    /// Output format: "code" or "svg" (default: "code")
    pub format: Option<String>,
}

/// A directed graph representing call dependencies between symbols
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallGraph {
    /// Map from symbol identifier to symbol
    symbols: HashMap<SymbolId, Symbol>,
    /// Map from source symbol to a map of `(target_id, dependency_type)` edge
    /// identity to `(Provenance, confidence)`. Using a `HashMap` here (rather
    /// than a `HashSet`) keeps the identity tuple (which is `Hash + Eq`) as
    /// the key and stores the per-edge metadata as the value. `f64` has no
    /// `Hash` impl, so it can never be part of the key.
    edges: HashMap<SymbolId, HashMap<(SymbolId, DependencyType), (Provenance, f64)>>,
    /// Reverse index: which symbols call this symbol (incoming edges)
    reverse_edges: HashMap<SymbolId, HashSet<SymbolId>>,
    /// Auxiliary index: base_name (lowercase) -> list of SymbolIds
    name_index: HashMap<String, Vec<SymbolId>>,
}

impl CallGraph {
    /// Creates a new empty CallGraph
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
            name_index: HashMap::new(),
        }
    }

    /// Adds a symbol to the graph
    pub fn add_symbol(&mut self, symbol: Symbol) -> SymbolId {
        let id = SymbolId::new(symbol.fully_qualified_name());
        self.symbols.entry(id.clone()).or_insert_with(|| {
            self.edges.entry(id.clone()).or_default();
            self.reverse_edges.entry(id.clone()).or_default();
            self.name_index
                .entry(symbol.name().to_lowercase())
                .or_default()
                .push(id.clone());
            symbol
        });
        id
    }

    /// Adds a dependency edge between two symbols.
    ///
    /// The default extraction context is [`ExtractionContext::DirectExtraction`],
    /// which routes through [`ConfidenceRules`] and assigns
    /// `(Extracted, 1.0)`. Use [`Self::add_dependency_with_provenance`] for
    /// edges that come from a heuristic resolver or are unresolved.
    ///
    /// The public signature is preserved from the pre-metadata version: this
    /// method is the backward-compatible path used by every existing caller
    /// in the workspace.
    pub fn add_dependency(
        &mut self,
        source_id: &SymbolId,
        target_id: &SymbolId,
        dependency_type: DependencyType,
    ) -> Result<(), CallGraphError> {
        self.add_dependency_with_provenance(
            source_id,
            target_id,
            dependency_type,
            ExtractionContext::DirectExtraction,
        )
    }

    /// Adds a dependency edge with an explicit extraction context.
    ///
    /// The `(Provenance, confidence)` metadata is assigned by
    /// [`ConfidenceRules::assign`]. This is the **sole sanctioned path** for
    /// edge metadata assignment.
    ///
    /// # Errors
    ///
    /// * [`CallGraphError::SymbolNotFound`] if either symbol is unknown.
    /// * [`CallGraphError::InvalidConfidence`] if the rules service rejects
    ///   the `Heuristic` score (NaN, infinite, or out of `[0.0, 1.0]`).
    pub fn add_dependency_with_provenance(
        &mut self,
        source_id: &SymbolId,
        target_id: &SymbolId,
        dependency_type: DependencyType,
        ctx: ExtractionContext,
    ) -> Result<(), CallGraphError> {
        // Ensure both symbols exist
        if !self.symbols.contains_key(source_id) {
            return Err(CallGraphError::SymbolNotFound(source_id.clone()));
        }
        if !self.symbols.contains_key(target_id) {
            return Err(CallGraphError::SymbolNotFound(target_id.clone()));
        }

        // Route through the rules service. The only failure case is a bad
        // Heuristic score; the resulting (Provenance, f64) is guaranteed
        // to be in [0.0, 1.0] and finite.
        let (provenance, confidence) = ConfidenceRules::new()
            .assign(ctx)
            .map_err(CallGraphError::InvalidConfidence)?;

        // Add edge (overwrites any previous metadata for the same key).
        self.edges.entry(source_id.clone()).or_default().insert(
            (target_id.clone(), dependency_type),
            (provenance, confidence),
        );

        // Add reverse edge
        self.reverse_edges
            .entry(target_id.clone())
            .or_default()
            .insert(source_id.clone());

        Ok(())
    }

    /// Returns the symbol with the given ID
    pub fn get_symbol(&self, id: &SymbolId) -> Option<&Symbol> {
        self.symbols.get(id)
    }

    /// Returns all symbols matching the given name (case-insensitive)
    pub fn find_by_name(&self, name: &str) -> Vec<&Symbol> {
        let name_lower = name.to_lowercase();
        let mut results = Vec::new();
        if let Some(ids) = self.name_index.get(&name_lower) {
            for id in ids {
                if let Some(symbol) = self.symbols.get(id) {
                    results.push(symbol);
                }
            }
        }
        results
    }

    /// Returns all symbols in the graph
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.values()
    }

    /// Returns an iterator over symbol IDs and symbols
    pub fn symbol_ids(&self) -> impl Iterator<Item = (&SymbolId, &Symbol)> {
        self.symbols.iter()
    }

    /// Returns the number of symbols in the graph
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Returns the number of edges in the graph
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|e| e.len()).sum()
    }

    /// Returns an iterator over all dependencies (edges) in the graph
    pub fn all_dependencies(
        &self,
    ) -> impl Iterator<Item = (&SymbolId, &SymbolId, &DependencyType)> {
        self.edges.iter().flat_map(|(source, targets)| {
            targets
                .iter()
                .map(move |((target, dep_type), _)| (source, target, dep_type))
        })
    }

    /// Returns all dependencies (outgoing edges) for a symbol
    pub fn dependencies(
        &self,
        id: &SymbolId,
    ) -> impl Iterator<Item = (&SymbolId, &DependencyType)> {
        self.edges
            .get(id)
            .map(|e| e.iter())
            .into_iter()
            .flatten()
            .map(|((target, dep_type), _)| (target, dep_type))
    }

    /// Returns the outgoing edges for a symbol along with their metadata.
    ///
    /// This is the additive metadata-aware counterpart of [`Self::dependencies`]
    /// and is the new API used by consumers that need to differentiate
    /// AST-extracted from heuristic edges.
    pub fn dependencies_with_metadata(
        &self,
        id: &SymbolId,
    ) -> impl Iterator<Item = (&SymbolId, &DependencyType, Provenance, f64)> {
        self.edges
            .get(id)
            .map(|e| e.iter())
            .into_iter()
            .flatten()
            .map(|((target, dep_type), (provenance, confidence))| {
                (target, dep_type, *provenance, *confidence)
            })
    }

    /// Returns an iterator over every edge in the graph with full metadata.
    ///
    /// Used by the persistence layer (cognicode-db) and the explorer
    /// adapter. The order is unspecified.
    pub fn edges_with_metadata(
        &self,
    ) -> impl Iterator<Item = (SymbolId, SymbolId, DependencyType, Provenance, f64)> {
        self.edges
            .iter()
            .flat_map(|(source, targets)| {
                targets
                    .iter()
                    .map(move |((target, dep_type), (provenance, confidence))| {
                        (
                            source.clone(),
                            target.clone(),
                            *dep_type,
                            *provenance,
                            *confidence,
                        )
                    })
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Returns all dependents (incoming edges) for a symbol
    pub fn dependents(&self, id: &SymbolId) -> impl Iterator<Item = &SymbolId> + '_ {
        self.reverse_edges
            .get(id)
            .map(|deps| deps.iter())
            .into_iter()
            .flatten()
    }

    /// Returns true if there's a path from source to target
    pub fn has_path(&self, source: &SymbolId, target: &SymbolId) -> bool {
        self.find_path(source, target).is_some()
    }

    /// Finds a path from source to target if one exists (BFS)
    pub fn find_path(&self, source: &SymbolId, target: &SymbolId) -> Option<Vec<SymbolId>> {
        self.find_path_with_max_depth(source, target, 0)
    }

    /// Finds a path from source to target if one exists, limited by max_depth (BFS)
    ///
    /// If max_depth is 0, no depth limit is applied.
    // TODO(Phase 4b): Delegate to petgraph::algo::astar when CallGraph migrates to StableGraph internally
    pub fn find_path_with_max_depth(
        &self,
        source: &SymbolId,
        target: &SymbolId,
        max_depth: usize,
    ) -> Option<Vec<SymbolId>> {
        if source == target {
            return Some(vec![source.clone()]);
        }
        let mut visited = HashSet::new();
        let mut predecessor: HashMap<SymbolId, SymbolId> = HashMap::new();
        let mut queue = vec![(source.clone(), 0)];
        visited.insert(source.clone());

        while let Some((current, depth)) = queue.pop() {
            if max_depth > 0 && depth >= max_depth {
                continue;
            }
            if let Some(dependencies) = self.edges.get(&current) {
                for ((next, _), _) in dependencies {
                    if next == target {
                        let mut path = vec![target.clone()];
                        let mut step = &current;
                        while let Some(prev) = predecessor.get(step) {
                            path.push(prev.clone());
                            step = prev;
                        }
                        path.push(source.clone());
                        path.reverse();
                        return Some(path);
                    }
                    if visited.insert(next.clone()) {
                        predecessor.insert(next.clone(), current.clone());
                        queue.push((next.clone(), depth + 1));
                    }
                }
            }
        }
        None
    }

    /// Returns all symbols that depend on the given symbol (transitively)
    pub fn find_all_dependents(&self, id: &SymbolId) -> HashSet<SymbolId> {
        let mut result = HashSet::new();
        let mut to_visit: Vec<SymbolId> = vec![id.clone()];

        while let Some(current) = to_visit.pop() {
            for dependent in self.dependents(&current) {
                if result.insert(dependent.clone()) {
                    to_visit.push(dependent.clone());
                }
            }
            // Also search for dependents by name pattern when the exact ID isn't found
            // This handles cases where callee IDs have placeholder formats like "name:0:0"
            let id_str = current.to_string();
            let base_name = id_str.split(':').next().unwrap_or(&id_str);
            let base_name_lower = base_name.to_lowercase();
            if let Some(symbol_ids) = self.name_index.get(&base_name_lower) {
                for symbol_id in symbol_ids {
                    if !result.contains(symbol_id)
                        && symbol_id != &current
                        && result.insert(symbol_id.clone())
                    {
                        to_visit.push(symbol_id.clone());
                    }
                }
            }
        }

        result
    }

    /// Returns all symbols that this symbol depends on (transitively)
    pub fn find_all_dependencies(&self, id: &SymbolId) -> HashSet<SymbolId> {
        let mut result = HashSet::new();
        let mut to_visit: Vec<SymbolId> = vec![id.clone()];

        while let Some(current) = to_visit.pop() {
            for (dependency, _) in self.dependencies(&current) {
                if result.insert(dependency.clone()) {
                    to_visit.push(dependency.clone());
                }
            }
        }

        result
    }

    /// Returns the direct callers of a symbol
    pub fn callers(&self, id: &SymbolId) -> Vec<SymbolId> {
        self.reverse_edges
            .get(id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Returns the direct callees of a symbol
    pub fn callees(&self, id: &SymbolId) -> Vec<(SymbolId, DependencyType)> {
        self.edges
            .get(id)
            .map(|e| e.iter().map(|((id, dep), _)| (id.clone(), *dep)).collect())
            .unwrap_or_default()
    }

    /// Returns the direct callees of a symbol along with their metadata.
    ///
    /// Mirrors [`Self::callees`] but also returns `Provenance` and
    /// `confidence` for every edge. Used by the explorer adapter when it
    /// needs to surface edge trust information to downstream consumers.
    pub fn callees_with_metadata(
        &self,
        id: &SymbolId,
    ) -> Vec<(SymbolId, DependencyType, Provenance, f64)> {
        self.edges
            .get(id)
            .map(|e| {
                e.iter()
                    .map(|((target, dep), (provenance, confidence))| {
                        (target.clone(), *dep, *provenance, *confidence)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Traverses callees (outgoing edges) up to max_depth
    ///
    /// Returns a list of CallEntry for each callee found at each depth level.
    /// Uses MAX_DEPTH as the default maximum if not specified.
    pub fn traverse_callees(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry> {
        self.traverse_callees_impl(id, max_depth, 0)
    }

    fn traverse_callees_impl(
        &self,
        id: &SymbolId,
        max_depth: u8,
        current_depth: u8,
    ) -> Vec<CallEntry> {
        let mut result = Vec::new();
        if current_depth >= max_depth {
            return result;
        }
        if let Some(edges) = self.edges.get(id) {
            for ((callee_id, _), _) in edges.iter() {
                if let Some(symbol) = self.get_symbol(callee_id) {
                    let location = symbol.location();
                    result.push(CallEntry {
                        symbol_id: callee_id.clone(),
                        symbol_name: symbol.name().to_string(),
                        file: location.file().to_string(),
                        line: location.line(),
                        column: location.column(),
                        depth: current_depth + 1,
                    });
                    if current_depth + 1 < max_depth {
                        let sub_entries =
                            self.traverse_callees_impl(callee_id, max_depth, current_depth + 1);
                        result.extend(sub_entries);
                    }
                }
            }
        }
        result
    }

    /// Traverses callers (incoming edges) up to max_depth
    ///
    /// Returns a list of CallEntry for each caller found at each depth level.
    /// Uses MAX_DEPTH as the default maximum if not specified.
    pub fn traverse_callers(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry> {
        self.traverse_callers_impl(id, max_depth, 0)
    }

    fn traverse_callers_impl(
        &self,
        id: &SymbolId,
        max_depth: u8,
        current_depth: u8,
    ) -> Vec<CallEntry> {
        let mut result = Vec::new();
        if current_depth >= max_depth {
            return result;
        }
        if let Some(callers_set) = self.reverse_edges.get(id) {
            for caller_id in callers_set.iter() {
                if let Some(symbol) = self.get_symbol(caller_id) {
                    let location = symbol.location();
                    result.push(CallEntry {
                        symbol_id: caller_id.clone(),
                        symbol_name: symbol.name().to_string(),
                        file: location.file().to_string(),
                        line: location.line(),
                        column: location.column(),
                        depth: current_depth + 1,
                    });
                    if current_depth + 1 < max_depth {
                        let sub_entries =
                            self.traverse_callers_impl(caller_id, max_depth, current_depth + 1);
                        result.extend(sub_entries);
                    }
                }
            }
        }
        result
    }

    /// Returns the fan-in (number of direct callers) for a symbol
    pub fn fan_in(&self, id: &SymbolId) -> usize {
        self.reverse_edges.get(id).map(|s| s.len()).unwrap_or(0)
    }

    /// Returns the fan-out (number of direct callees) for a symbol
    pub fn fan_out(&self, id: &SymbolId) -> usize {
        self.edges.get(id).map(|e| e.len()).unwrap_or(0)
    }

    /// Exports the call graph as a Mermaid flowchart
    ///
    /// Generates a directed graph representation suitable for Mermaid flowchart syntax.
    pub fn to_mermaid(&self, title: &str) -> String {
        let mut mermaid = String::from("flowchart TD\n");
        mermaid.push_str(&format!("    %% {}\n", title));

        // Add node declarations
        for symbol in self.symbols() {
            let id = SymbolId::new(symbol.fully_qualified_name());
            let safe_id = id
                .as_str()
                .replace([':', '(', ')', '<', '>', '{', '}'], "_");
            let name = symbol.name();
            let kind = format!("{:?}", symbol.kind());
            mermaid.push_str(&format!("    {}[{} ({})]\n", safe_id, name, kind));
        }

        // Add edges
        for (source_id, edges) in &self.edges {
            let safe_source = source_id
                .as_str()
                .replace([':', '(', ')', '<', '>', '{', '}'], "_");
            for ((target_id, dep_type), _) in edges {
                let safe_target = target_id
                    .as_str()
                    .replace([':', '(', ')', '<', '>', '{', '}'], "_");
                let edge_label = match dep_type {
                    DependencyType::Calls => "calls",
                    DependencyType::Imports => "imports",
                    DependencyType::Inherits => "inherits",
                    DependencyType::UsesGeneric => "uses_generic",
                    DependencyType::References => "references",
                    DependencyType::Defines => "defines",
                    DependencyType::AnnotatedBy => "annotated_by",
                    DependencyType::Contains => "contains",
                };
                mermaid.push_str(&format!(
                    "    {} -->|{}| {}\n",
                    safe_source, edge_label, safe_target
                ));
            }
        }

        mermaid
    }

    /// Exports the call graph as a Mermaid flowchart with options
    ///
    /// When root is provided, exports only the subgraph rooted at that symbol.
    /// When max_depth is provided, limits the traversal depth.
    pub fn to_mermaid_with_options(&self, title: &str, options: &MermaidOptions) -> String {
        let mut mermaid = String::new();

        // Determine which symbols to include
        let symbols_to_export: Vec<Symbol> = if let Some(ref root_name) = options.root {
            let root_symbols = self.find_by_name(root_name);
            if root_symbols.is_empty() {
                // Root not found, return empty graph
                return "flowchart TD\n    %% Root symbol not found".to_string();
            }
            // Get the FQN of the root symbol to filter descendants
            let root_fqn = root_symbols[0].fully_qualified_name().to_lowercase();
            self.symbols
                .values()
                .filter(|s| {
                    let fqn = s.fully_qualified_name().to_lowercase();
                    fqn == root_fqn || fqn.starts_with(&format!("{}::", root_fqn))
                })
                .cloned()
                .collect()
        } else {
            self.symbols.values().cloned().collect()
        };

        // Build node declarations
        mermaid.push_str("flowchart TD\n");
        mermaid.push_str(&format!("    %% {}\n", title));

        // Add node declarations for selected symbols
        for symbol in &symbols_to_export {
            let id = SymbolId::new(symbol.fully_qualified_name());
            let safe_id = id
                .as_str()
                .replace([':', '(', ')', '<', '>', '{', '}'], "_");
            let name = symbol.name();
            let kind = format!("{:?}", symbol.kind());
            mermaid.push_str(&format!("    {}[{} ({})]\n", safe_id, name, kind));
        }

        // Create a set of symbol IDs for quick lookup
        let symbol_ids: std::collections::HashSet<_> = symbols_to_export
            .iter()
            .map(|s| SymbolId::new(s.fully_qualified_name()))
            .collect();

        // Add edges based on whether we have a root or not
        if options.root.is_some() {
            // For subgraph with root, traverse up to max_depth
            let max_depth = if options.max_depth > 0 {
                options.max_depth
            } else {
                3
            };
            let mut visited = std::collections::HashSet::new();
            for symbol_id in &symbol_ids {
                self.collect_mermaid_edges_recursive(
                    symbol_id,
                    max_depth,
                    0,
                    &symbol_ids,
                    &mut visited,
                    &mut mermaid,
                );
            }
        } else {
            // For full graph, add all edges between exported symbols
            for (source_id, edges) in &self.edges {
                if !symbol_ids.contains(source_id) {
                    continue;
                }
                let safe_source = source_id
                    .as_str()
                    .replace([':', '(', ')', '<', '>', '{', '}'], "_");
                for ((target_id, dep_type), _) in edges {
                    if !symbol_ids.contains(target_id) {
                        continue;
                    }
                    let safe_target = target_id
                        .as_str()
                        .replace([':', '(', ')', '<', '>', '{', '}'], "_");
                    let edge_label = match dep_type {
                        DependencyType::Calls => "calls",
                        DependencyType::Imports => "imports",
                        DependencyType::Inherits => "inherits",
                        DependencyType::UsesGeneric => "uses_generic",
                        DependencyType::References => "references",
                        DependencyType::Defines => "defines",
                        DependencyType::AnnotatedBy => "annotated_by",
                        DependencyType::Contains => "contains",
                    };
                    mermaid.push_str(&format!(
                        "    {} -->|{}| {}\n",
                        safe_source, edge_label, safe_target
                    ));
                }
            }
        }

        mermaid
    }

    /// Helper to collect Mermaid edges recursively for a subgraph
    fn collect_mermaid_edges_recursive(
        &self,
        symbol_id: &SymbolId,
        max_depth: u8,
        current_depth: u8,
        allowed_symbols: &std::collections::HashSet<SymbolId>,
        visited: &mut std::collections::HashSet<SymbolId>,
        mermaid: &mut String,
    ) {
        if current_depth >= max_depth || visited.contains(symbol_id) {
            return;
        }
        visited.insert(symbol_id.clone());

        if let Some(dependencies) = self.edges.get(symbol_id) {
            let safe_source = symbol_id
                .as_str()
                .replace([':', '(', ')', '<', '>', '{', '}'], "_");
            for ((target_id, dep_type), _) in dependencies {
                if !allowed_symbols.contains(target_id) {
                    continue;
                }
                let safe_target = target_id
                    .as_str()
                    .replace([':', '(', ')', '<', '>', '{', '}'], "_");
                let edge_label = match dep_type {
                    DependencyType::Calls => "calls",
                    DependencyType::Imports => "imports",
                    DependencyType::Inherits => "inherits",
                    DependencyType::UsesGeneric => "uses_generic",
                    DependencyType::References => "references",
                    DependencyType::Defines => "defines",
                    DependencyType::AnnotatedBy => "annotated_by",
                    DependencyType::Contains => "contains",
                };
                mermaid.push_str(&format!(
                    "    {} -->|{}| {}\n",
                    safe_source, edge_label, safe_target
                ));
                self.collect_mermaid_edges_recursive(
                    target_id,
                    max_depth,
                    current_depth + 1,
                    allowed_symbols,
                    visited,
                    mermaid,
                );
            }
        }
    }

    /// Returns all root symbols (symbols with no incoming edges)
    pub fn roots(&self) -> Vec<SymbolId> {
        self.symbols
            .keys()
            .filter(|id| !self.reverse_edges.contains_key(id) || self.reverse_edges[id].is_empty())
            .cloned()
            .collect()
    }

    /// Returns all leaf symbols (symbols with no outgoing edges)
    pub fn leaves(&self) -> Vec<SymbolId> {
        self.symbols
            .keys()
            .filter(|id| !self.edges.contains_key(id) || self.edges[id].is_empty())
            .cloned()
            .collect()
    }

    /// Returns all dead code symbols (not reachable from any entry point).
    ///
    /// Dead code = callable or type definition symbols that are NOT reachable
    /// from any entry point via outgoing edges.
    ///
    /// Entry points are symbols with no incoming edges (roots).
    pub fn find_dead_code(&self) -> Vec<SymbolId> {
        // BFS from all roots to find reachable symbols
        let mut live = HashSet::new();
        let mut queue: Vec<SymbolId> = self.roots();

        while let Some(id) = queue.pop() {
            if live.insert(id.clone()) {
                // Add all callees to the queue
                for (target, _) in self.dependencies(&id) {
                    if !live.contains(target) {
                        queue.push(target.clone());
                    }
                }
            }
        }

        // Dead code = callable or type_def symbols NOT in live set
        self.symbols
            .keys()
            .filter(|id| {
                !live.contains(id)
                    && self
                        .get_symbol(id)
                        .map(|s| {
                            let kind = s.kind();
                            kind.is_callable() || kind.is_type_definition()
                        })
                        .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Extracts the module path from a file path (parent directory).
    fn module_from_file(file: &str) -> String {
        std::path::Path::new(file)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| file.to_string())
    }

    /// Returns all modules in the graph (unique parent directories of symbol files).
    pub fn modules(&self) -> HashSet<String> {
        self.symbols
            .values()
            .map(|s| Self::module_from_file(s.location().file()))
            .collect()
    }

    /// Finds module dependencies using Tarjan SCC for cycle detection.
    ///
    /// Returns a tuple of:
    /// - modules: Vec of (module, depends_on, depended_by, coupling_score)
    /// - cycles: Vec of cycles (each cycle is a Vec of module names)
    /// - coupling_matrix: Map of (from, to) -> edge count
    pub fn find_module_dependencies(
        &self,
    ) -> (
        Vec<(String, Vec<String>, Vec<String>, usize)>,
        Vec<Vec<String>>,
        HashMap<(String, String), usize>,
    ) {
        // Step 1: Build module-level edges from symbol edges
        let mut module_edges: HashMap<(String, String), usize> = HashMap::new();
        let mut module_outgoing: HashMap<String, HashSet<String>> = HashMap::new();
        let mut module_incoming: HashMap<String, HashSet<String>> = HashMap::new();

        for (source_id, targets) in &self.edges {
            let source_module = self
                .get_symbol(source_id)
                .map(|s| Self::module_from_file(s.location().file()))
                .unwrap_or_default();

            for ((target_id, _), _) in targets {
                let target_module = self
                    .get_symbol(target_id)
                    .map(|s| Self::module_from_file(s.location().file()))
                    .unwrap_or_default();

                if source_module != target_module
                    && !source_module.is_empty()
                    && !target_module.is_empty()
                {
                    *module_edges
                        .entry((source_module.clone(), target_module.clone()))
                        .or_insert(0) += 1;
                    module_outgoing
                        .entry(source_module.clone())
                        .or_default()
                        .insert(target_module.clone());
                    module_incoming
                        .entry(target_module.clone())
                        .or_default()
                        .insert(source_module.clone());
                }
            }
        }

        // Step 2: Get all modules
        let all_modules: HashSet<String> = module_outgoing
            .keys()
            .cloned()
            .chain(module_incoming.keys().cloned())
            .collect();

        // Step 3: Build module graph for Tarjan SCC
        // Map module name to index for petgraph
        let module_list: Vec<String> = all_modules.into_iter().collect();
        let module_index: HashMap<&String, usize> = module_list
            .iter()
            .enumerate()
            .map(|(i, m)| (m, i))
            .collect();

        use petgraph::graph::{DiGraph, NodeIndex};
        let mut graph: DiGraph<(), ()> = DiGraph::new();
        // Add nodes
        for _ in &module_list {
            graph.add_node(());
        }
        // Add edges
        for (src, dst) in module_edges.keys() {
            if let (Some(&si), Some(&di)) = (module_index.get(src), module_index.get(dst)) {
                graph.add_edge(NodeIndex::new(si), NodeIndex::new(di), ());
            }
        }

        // Step 4: Run Tarjan SCC to find cycles
        use petgraph::algo::tarjan_scc;
        let sccs = tarjan_scc(&graph);
        let cycles: Vec<Vec<String>> = sccs
            .into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                scc.iter()
                    .map(|&idx| module_list[idx.index()].clone())
                    .collect()
            })
            .collect();

        // Step 5: Build result
        let modules: Vec<(String, Vec<String>, Vec<String>, usize)> = module_list
            .into_iter()
            .map(|module| {
                let depends_on: Vec<String> = module_outgoing
                    .get(&module)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                let depended_by: Vec<String> = module_incoming
                    .get(&module)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                let coupling_score: usize = depends_on
                    .iter()
                    .map(|m| {
                        module_edges
                            .get(&(module.clone(), m.clone()))
                            .copied()
                            .unwrap_or(0)
                    })
                    .sum();
                (module, depends_on, depended_by, coupling_score)
            })
            .collect();

        (modules, cycles, module_edges)
    }

    /// Removes a symbol and all its edges from the graph
    pub fn remove_symbol(&mut self, id: &SymbolId) -> Option<Symbol> {
        if let Some(symbol) = self.symbols.remove(id) {
            // Remove all outgoing edges
            if let Some(deps) = self.edges.remove(id) {
                for ((target, _), _) in deps {
                    if let Some(rev) = self.reverse_edges.get_mut(&target) {
                        rev.remove(id);
                    }
                }
            }
            // Remove all incoming edges
            if let Some(callers) = self.reverse_edges.remove(id) {
                for caller in callers {
                    if let Some(edges) = self.edges.get_mut(&caller) {
                        edges.retain(|(t, _), _| t != id);
                    }
                }
            }
            // Clean up name_index
            let base_name = symbol.name().to_lowercase();
            if let Some(ids) = self.name_index.get_mut(&base_name) {
                ids.retain(|sid| sid != id);
                if ids.is_empty() {
                    self.name_index.remove(&base_name);
                }
            }
            Some(symbol)
        } else {
            None
        }
    }

    /// Applies a batch of graph events incrementally
    ///
    /// This allows for efficient updates when files change, rather than
    /// rebuilding the entire graph from scratch.
    pub fn apply_events(&mut self, events: &[GraphEvent]) -> Result<(), CallGraphError> {
        for event in events {
            match event {
                GraphEvent::SymbolAdded(e) => {
                    let symbol = Symbol::new(e.name.clone(), e.kind, e.location.clone());
                    self.add_symbol(symbol);
                }
                GraphEvent::SymbolRemoved(e) => {
                    let id = SymbolId::new(e.location.fully_qualified_name());
                    self.remove_symbol(&id);
                }
                GraphEvent::SymbolModified(e) => {
                    // For modifications, we update the symbol's location and signature
                    let old_id = SymbolId::new(e.old_location.fully_qualified_name());
                    if let Some(symbol) = self.symbols.remove(&old_id) {
                        // Create updated symbol
                        let new_symbol =
                            Symbol::new(e.name.clone(), *symbol.kind(), e.new_location.clone());
                        let new_id = SymbolId::new(e.new_location.fully_qualified_name());

                        // Re-add with new ID, preserving edges if possible
                        let old_edges = self.edges.remove(&old_id);
                        let old_callers = self.reverse_edges.remove(&old_id);

                        self.symbols.insert(new_id.clone(), new_symbol);
                        self.edges
                            .insert(new_id.clone(), old_edges.unwrap_or_default());
                        if let Some(callers) = old_callers {
                            self.reverse_edges.insert(new_id.clone(), callers);
                        }

                        // Update references in callers' edges
                        for caller_id in
                            self.reverse_edges.get(&new_id).cloned().unwrap_or_default()
                        {
                            if let Some(edges) = self.edges.get_mut(&caller_id) {
                                let old_target = (old_id.clone(), DependencyType::Calls);
                                let new_target = (new_id.clone(), DependencyType::Calls);
                                // Preserve the metadata (Provenance, confidence)
                                // when remapping the key from old to new id.
                                if let Some(meta) = edges.remove(&old_target) {
                                    edges.insert(new_target, meta);
                                }
                            }
                        }
                    }
                }
                GraphEvent::DependencyAdded(e) => {
                    let source_id = SymbolId::new(format!("{}:0:0", e.source_name));
                    let target_id = SymbolId::new(format!("{}:0:0", e.target_name));
                    let _ = self.add_dependency(&source_id, &target_id, e.dependency_type);
                }
                GraphEvent::DependencyRemoved(e) => {
                    let source_id = SymbolId::new(format!("{}:0:0", e.source_name));
                    let target_id = SymbolId::new(format!("{}:0:0", e.target_name));
                    if let Some(edges) = self.edges.get_mut(&source_id) {
                        edges.retain(|(t, dt), _| t != &target_id || dt != &e.dependency_type);
                    }
                    if let Some(callers) = self.reverse_edges.get_mut(&target_id) {
                        callers.remove(&source_id);
                    }
                }
                // Graph-level events are not applicable to symbol-level apply_events
                GraphEvent::GraphReplaced
                | GraphEvent::GraphCleared
                | GraphEvent::GraphModified => {
                    // No-op: these events are handled at the GraphCache level
                }
            }
        }
        Ok(())
    }
}

impl Default for CallGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for a symbol in the call graph
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId(String);

impl SymbolId {
    /// Creates a new SymbolId
    pub fn new(id: impl AsRef<str>) -> Self {
        Self(id.as_ref().to_string())
    }

    /// Returns the identifier as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Error type for CallGraph operations
#[derive(Debug, thiserror::Error)]
pub enum CallGraphError {
    #[error("Symbol not found: {0}")]
    SymbolNotFound(SymbolId),

    #[error("Symbol already exists: {0}")]
    SymbolAlreadyExists(SymbolId),

    /// The extraction context (typically a `Heuristic` score) was rejected
    /// by [`crate::domain::services::ConfidenceRules`].
    #[error("invalid confidence: {0}")]
    InvalidConfidence(#[source] crate::domain::services::ConfidenceError),
}

// ---------------------------------------------------------------------------
// CallGraphV1 — legacy bincode shadow type.
//
// The pre-metadata `CallGraph` stored edges as a `HashSet<(SymbolId, DependencyType)>`
// without any provenance or confidence. The bincode wire-format of the old
// `CallGraph` is therefore *not* the same as the new one (HashSet vs
// HashMap, missing metadata tuple). To keep existing on-disk blobs loadable
// across the upgrade, this shadow struct mirrors the **old** shape so that
// `bincode` can decode legacy blobs into it. [`CallGraphV1::into_v2`] then
// lifts the data into a v2 `CallGraph` with `(Extracted, 1.0)` defaults
// applied to every edge.
//
// This type is `#[deprecated]` and only exists to support the one-time
// migration from v1 to v2 blobs. It will be removed after one release
// cycle.
// ---------------------------------------------------------------------------
/// Pre-metadata shape of `CallGraph` used by the v1 bincode blob format.
///
/// **Deprecated** — only used to decode legacy v1 blobs. Use [`CallGraph`]
/// for all new code.
#[allow(deprecated)] // allow the deprecated attribute below to apply cleanly
#[deprecated(
    since = "0.0.0",
    note = "CallGraphV1 is a one-time migration shim; remove after one release cycle"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallGraphV1 {
    // `pub` because the only consumer is migration code (e.g.
    // `cognicode-db::VersionedBlob::decode`) plus integration tests
    // that need to hand-craft v1 blobs. Keeping the fields public lets
    // those tests build a v1 graph directly without exposing a builder
    // API that will be deleted in one release cycle anyway.
    pub symbols: HashMap<SymbolId, Symbol>,
    pub edges: HashMap<SymbolId, HashSet<(SymbolId, DependencyType)>>,
    pub reverse_edges: HashMap<SymbolId, HashSet<SymbolId>>,
    pub name_index: HashMap<String, Vec<SymbolId>>,
}

#[allow(deprecated)]
impl CallGraphV1 {
    /// Construct an empty `CallGraphV1`. Visible so migration tests
    /// (in `cognicode-db` and `cognicode-core`) can hand-craft a v1
    /// graph and roundtrip it through the v2 read path.
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
            name_index: HashMap::new(),
        }
    }
}

#[allow(deprecated)]
impl Default for CallGraphV1 {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(deprecated)]
impl CallGraphV1 {
    /// Lift a legacy v1 graph into the current v2 shape, assigning
    /// `(Provenance::Extracted, 1.0)` to every edge. The metadata is
    /// always `Extracted` because pre-metadata v1 graphs had no notion
    /// of confidence — they were all produced by direct AST extraction.
    pub fn into_v2(self) -> CallGraph {
        let mut out = CallGraph::new();
        // Copy symbols verbatim.
        for (id, sym) in self.symbols.into_iter() {
            // We have to bypass `add_symbol` (which derives an id from
            // the symbol's fully-qualified name) to preserve the
            // original id exactly.
            out.symbols.insert(id.clone(), sym);
            out.edges.entry(id.clone()).or_default();
            out.reverse_edges.entry(id).or_default();
        }
        // Copy name_index. We do best-effort: if a name was already
        // added by `out.symbols.insert` we would have lost it, so we
        // re-derive it by walking the symbols map.
        for (id, sym) in out.symbols.iter() {
            out.name_index
                .entry(sym.name().to_lowercase())
                .or_default()
                .push(id.clone());
        }
        // Copy edges, attaching (Extracted, 1.0) to each.
        for (source, deps) in self.edges.into_iter() {
            for (target, dep) in deps.into_iter() {
                out.edges
                    .entry(source.clone())
                    .or_default()
                    .insert((target.clone(), dep), (Provenance::Extracted, 1.0));
                out.reverse_edges
                    .entry(target)
                    .or_default()
                    .insert(source.clone());
            }
        }
        out
    }
}

use std::fmt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_graph_add_symbol() {
        let mut graph = CallGraph::new();
        let symbol = Symbol::new("test", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let id = graph.add_symbol(symbol);
        assert_eq!(graph.symbol_count(), 1);
        assert_eq!(graph.get_symbol(&id).unwrap().name(), "test");
    }

    #[test]
    fn test_call_graph_add_dependency() {
        let mut graph = CallGraph::new();
        let symbol1 = Symbol::new(
            "caller",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let symbol2 = Symbol::new(
            "callee",
            SymbolKind::Function,
            Location::new("test.rs", 10, 1),
        );

        let id1 = graph.add_symbol(symbol1);
        let id2 = graph.add_symbol(symbol2);

        graph
            .add_dependency(&id1, &id2, DependencyType::Calls)
            .unwrap();

        assert_eq!(graph.edge_count(), 1);
        assert!(graph.has_path(&id1, &id2));
    }

    #[test]
    fn test_call_graph_dependents() {
        let mut graph = CallGraph::new();
        let symbol1 = Symbol::new(
            "caller",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let symbol2 = Symbol::new(
            "callee",
            SymbolKind::Function,
            Location::new("test.rs", 10, 1),
        );

        let id1 = graph.add_symbol(symbol1);
        let id2 = graph.add_symbol(symbol2);

        graph
            .add_dependency(&id1, &id2, DependencyType::Calls)
            .unwrap();

        let callers = graph.callers(&id2);
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0], id1);
    }

    #[test]
    fn test_call_graph_find_path() {
        let mut graph = CallGraph::new();
        let symbol1 = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let symbol2 = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));
        let symbol3 = Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 1));

        let id1 = graph.add_symbol(symbol1);
        let id2 = graph.add_symbol(symbol2);
        let id3 = graph.add_symbol(symbol3);

        graph
            .add_dependency(&id1, &id2, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id2, &id3, DependencyType::Calls)
            .unwrap();

        let path = graph.find_path(&id1, &id3);
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 3);
    }

    #[test]
    fn test_call_graph_transitive_dependents() {
        let mut graph = CallGraph::new();
        let symbol1 = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let symbol2 = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));
        let symbol3 = Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 1));

        let id1 = graph.add_symbol(symbol1);
        let id2 = graph.add_symbol(symbol2);
        let id3 = graph.add_symbol(symbol3);

        graph
            .add_dependency(&id1, &id2, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&id2, &id3, DependencyType::Calls)
            .unwrap();

        let all_dependents = graph.find_all_dependents(&id3);
        assert!(all_dependents.contains(&id2));
        assert!(all_dependents.contains(&id1));
    }

    #[test]
    fn test_call_graph_remove_symbol() {
        let mut graph = CallGraph::new();
        let symbol1 = Symbol::new(
            "caller",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let symbol2 = Symbol::new(
            "callee",
            SymbolKind::Function,
            Location::new("test.rs", 10, 1),
        );

        let id1 = graph.add_symbol(symbol1);
        let id2 = graph.add_symbol(symbol2);

        graph
            .add_dependency(&id1, &id2, DependencyType::Calls)
            .unwrap();
        graph.remove_symbol(&id2).unwrap();

        assert_eq!(graph.symbol_count(), 1);
        assert!(!graph.has_path(&id1, &id2));
    }

    use crate::domain::value_objects::{Location, SymbolKind};

    #[test]
    fn test_call_graph_roots_and_leaves() {
        let mut graph = CallGraph::new();
        let symbol1 = Symbol::new("root", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let symbol2 = Symbol::new(
            "leaf",
            SymbolKind::Function,
            Location::new("test.rs", 10, 1),
        );

        let id1 = graph.add_symbol(symbol1);
        let id2 = graph.add_symbol(symbol2);

        graph
            .add_dependency(&id1, &id2, DependencyType::Calls)
            .unwrap();

        let roots = graph.roots();
        assert!(roots.contains(&id1));

        let leaves = graph.leaves();
        assert!(leaves.contains(&id2));
    }

    #[cfg(feature = "persistence")]
    #[test]
    fn test_call_graph_bincode_roundtrip() {
        use bincode::config::standard;
        use bincode::serde::{decode_from_slice, encode_to_vec};

        let mut graph = CallGraph::new();
        let symbol1 = Symbol::new(
            "func_a",
            SymbolKind::Function,
            Location::new("test.rs", 10, 1),
        );
        let symbol2 = Symbol::new(
            "func_b",
            SymbolKind::Function,
            Location::new("test.rs", 20, 1),
        );

        let id1 = graph.add_symbol(symbol1);
        let id2 = graph.add_symbol(symbol2);

        graph
            .add_dependency(&id1, &id2, DependencyType::Calls)
            .unwrap();

        // Serialize
        let bytes = encode_to_vec(&graph, standard()).expect("Failed to serialize CallGraph");

        // Deserialize
        let (deserialized_graph, _): (CallGraph, usize) =
            decode_from_slice(&bytes, standard()).expect("Failed to deserialize CallGraph");

        // Assert equality
        assert_eq!(graph.symbol_count(), deserialized_graph.symbol_count());
        assert_eq!(graph.edge_count(), deserialized_graph.edge_count());
    }

    // -------------------------------------------------------------------------
    // New metadata-aware tests (Phase 2 of the explorer-graph-foundation slice)
    // -------------------------------------------------------------------------

    use crate::domain::services::ExtractionContext;
    use crate::domain::value_objects::Provenance;

    fn build_three_node_graph() -> (CallGraph, SymbolId, SymbolId, SymbolId) {
        let mut graph = CallGraph::new();
        let a = graph.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("a.rs", 1, 0),
        ));
        let b = graph.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("b.rs", 1, 0),
        ));
        let c = graph.add_symbol(Symbol::new(
            "c",
            SymbolKind::Function,
            Location::new("c.rs", 1, 0),
        ));
        (graph, a, b, c)
    }

    #[test]
    fn add_dependency_defaults_to_extracted_one() {
        let (mut graph, a, b, _) = build_three_node_graph();
        graph
            .add_dependency(&a, &b, DependencyType::Calls)
            .expect("direct call");

        let metas = graph.callees_with_metadata(&a);
        assert_eq!(metas.len(), 1);
        let (_target, dep, prov, conf) = &metas[0];
        assert_eq!(*dep, DependencyType::Calls);
        assert_eq!(*prov, Provenance::Extracted);
        assert_eq!(*conf, 1.0_f64);
    }

    #[test]
    fn add_dependency_with_provenance_direct_extraction() {
        let (mut graph, a, b, _) = build_three_node_graph();
        graph
            .add_dependency_with_provenance(
                &a,
                &b,
                DependencyType::Calls,
                ExtractionContext::DirectExtraction,
            )
            .expect("direct extraction");

        let metas = graph.callees_with_metadata(&a);
        assert_eq!(metas.len(), 1);
        let (_, _, prov, conf) = &metas[0];
        assert_eq!(*prov, Provenance::Extracted);
        assert_eq!(*conf, 1.0_f64);
    }

    #[test]
    fn add_dependency_with_provenance_heuristic_clamps_into_band() {
        let (mut graph, a, _, c) = build_three_node_graph();
        // Score above the band (0.5, 0.9) must clamp to 0.9.
        graph
            .add_dependency_with_provenance(
                &a,
                &c,
                DependencyType::Calls,
                ExtractionContext::Heuristic { score: 0.99 },
            )
            .expect("in-range heuristic");
        let metas = graph.callees_with_metadata(&a);
        assert_eq!(metas[0].2, Provenance::Inferred);
        assert_eq!(metas[0].3, 0.9_f64);
    }

    #[test]
    fn add_dependency_with_provenance_heuristic_clamps_below_band() {
        let (mut graph, a, b, _) = build_three_node_graph();
        // Score inside the band must pass through unchanged.
        graph
            .add_dependency_with_provenance(
                &a,
                &b,
                DependencyType::Imports,
                ExtractionContext::Heuristic { score: 0.7 },
            )
            .expect("in-range heuristic");
        let metas = graph.callees_with_metadata(&a);
        assert_eq!(metas[0].2, Provenance::Inferred);
        assert_eq!(metas[0].3, 0.7_f64);
    }

    #[test]
    fn add_dependency_with_provenance_unresolved() {
        let (mut graph, a, b, _) = build_three_node_graph();
        graph
            .add_dependency_with_provenance(
                &a,
                &b,
                DependencyType::References,
                ExtractionContext::Unresolved,
            )
            .expect("unresolved");
        let metas = graph.callees_with_metadata(&a);
        assert_eq!(metas[0].2, Provenance::Ambiguous);
        assert_eq!(metas[0].3, 0.3_f64);
    }

    #[test]
    fn add_dependency_with_provenance_rejects_nan() {
        let (mut graph, a, b, _) = build_three_node_graph();
        let result = graph.add_dependency_with_provenance(
            &a,
            &b,
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: f64::NAN },
        );
        assert!(matches!(
            result,
            Err(CallGraphError::InvalidConfidence(
                crate::domain::services::ConfidenceError::NotANumber
            ))
        ));
        // The edge must not be inserted.
        assert!(graph.callees(&a).is_empty());
    }

    #[test]
    fn add_dependency_with_provenance_rejects_out_of_range() {
        let (mut graph, a, b, _) = build_three_node_graph();
        let result = graph.add_dependency_with_provenance(
            &a,
            &b,
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: 1.5 },
        );
        assert!(matches!(
            result,
            Err(CallGraphError::InvalidConfidence(
                crate::domain::services::ConfidenceError::OutOfRange(_)
            ))
        ));
        assert!(graph.callees(&a).is_empty());
    }

    #[test]
    fn callees_with_metadata_returns_empty_for_unknown_symbol() {
        let (graph, _, _, _) = build_three_node_graph();
        let unknown = SymbolId::new("ghost.rs:ghost:1");
        assert!(graph.callees_with_metadata(&unknown).is_empty());
    }

    #[test]
    fn edges_with_metadata_yields_one_per_edge() {
        let (mut graph, a, b, c) = build_three_node_graph();
        graph
            .add_dependency_with_provenance(
                &a,
                &b,
                DependencyType::Calls,
                ExtractionContext::DirectExtraction,
            )
            .unwrap();
        graph
            .add_dependency_with_provenance(
                &a,
                &c,
                DependencyType::Imports,
                ExtractionContext::Heuristic { score: 0.6 },
            )
            .unwrap();

        let all: Vec<_> = graph.edges_with_metadata().collect();
        assert_eq!(all.len(), 2);
        // Every entry must have a finite confidence in [0.0, 1.0].
        for (src, tgt, _dep, prov, conf) in &all {
            assert!(!src.as_str().is_empty());
            assert!(!tgt.as_str().is_empty());
            assert!(!prov.to_string().is_empty());
            assert!(
                (0.0..=1.0).contains(conf),
                "conf {conf} out of range for {src}->{tgt}"
            );
            assert!(!conf.is_nan());
            assert!(conf.is_finite());
        }
    }

    /// Spec post-condition: every edge in the graph must satisfy
    /// `confidence ∈ [0.0, 1.0] && !is_nan() && is_finite()`.
    /// This is the invariant test from spec requirement "Testability".
    #[test]
    fn invariant_every_edge_has_finite_in_range_confidence() {
        let (mut graph, a, b, c) = build_three_node_graph();

        // Add a mix of edges covering all three contexts.
        graph
            .add_dependency_with_provenance(
                &a,
                &b,
                DependencyType::Calls,
                ExtractionContext::DirectExtraction,
            )
            .unwrap();
        graph
            .add_dependency_with_provenance(
                &a,
                &c,
                DependencyType::Imports,
                ExtractionContext::Heuristic { score: 0.55 },
            )
            .unwrap();
        graph
            .add_dependency_with_provenance(
                &b,
                &c,
                DependencyType::References,
                ExtractionContext::Unresolved,
            )
            .unwrap();

        for (_src, _tgt, _dep, _prov, conf) in graph.edges_with_metadata() {
            assert!(
                (0.0..=1.0).contains(&conf),
                "confidence {conf} out of [0.0, 1.0]"
            );
            assert!(!conf.is_nan(), "NaN confidence leaked into edge");
            assert!(conf.is_finite(), "non-finite confidence leaked into edge");
        }
    }

    #[test]
    fn pre_existing_api_dependencies_still_works() {
        // Backward-compat: dependencies()/callees()/callers() must work as
        // before, without exposing metadata. The signature is unchanged.
        let (mut graph, a, b, _) = build_three_node_graph();
        graph.add_dependency(&a, &b, DependencyType::Calls).unwrap();
        let deps: Vec<_> = graph
            .dependencies(&a)
            .map(|(t, d)| (t.clone(), *d))
            .collect();
        assert_eq!(deps, vec![(b.clone(), DependencyType::Calls)]);
        assert_eq!(graph.callees(&a), vec![(b.clone(), DependencyType::Calls)]);
        assert!(graph.callers(&b).contains(&a));
    }

    // -------------------------------------------------------------------------
    // CallGraphV1 → CallGraph migration tests (Phase 3 of the foundation slice)
    // -------------------------------------------------------------------------

    #[test]
    #[allow(deprecated)]
    fn callgraph_v1_into_v2_assigns_extracted_one_to_every_edge() {
        // Build a v1 graph (legacy shape) and lift it.
        let mut v1 = CallGraphV1::new();
        let sa = Symbol::new("alpha", SymbolKind::Function, Location::new("a.rs", 1, 0));
        let sb = Symbol::new("beta", SymbolKind::Function, Location::new("b.rs", 1, 0));
        let id_a = SymbolId::new(sa.fully_qualified_name());
        let id_b = SymbolId::new(sb.fully_qualified_name());
        v1.symbols.insert(id_a.clone(), sa);
        v1.symbols.insert(id_b.clone(), sb);
        v1.edges.insert(
            id_a.clone(),
            std::iter::once((id_b.clone(), DependencyType::Calls)).collect(),
        );

        let v2 = v1.into_v2();

        assert_eq!(v2.symbol_count(), 2);
        assert_eq!(v2.edge_count(), 1);

        // Every edge must carry (Extracted, 1.0).
        for (_src, _tgt, _dep, prov, conf) in v2.edges_with_metadata() {
            assert_eq!(prov, Provenance::Extracted);
            assert_eq!(conf, 1.0_f64);
        }
    }

    #[test]
    #[allow(deprecated)]
    fn callgraph_v1_into_v2_bincode_roundtrip() {
        // A v1 graph encoded with bincode must decode into a v2 graph
        // with all edges tagged (Extracted, 1.0).
        let mut v1 = CallGraphV1::new();
        let sa = Symbol::new("alpha", SymbolKind::Function, Location::new("a.rs", 1, 0));
        let sb = Symbol::new("beta", SymbolKind::Function, Location::new("b.rs", 1, 0));
        let id_a = SymbolId::new(sa.fully_qualified_name());
        let id_b = SymbolId::new(sb.fully_qualified_name());
        v1.symbols.insert(id_a.clone(), sa);
        v1.symbols.insert(id_b.clone(), sb);
        v1.edges.insert(
            id_a.clone(),
            std::iter::once((id_b.clone(), DependencyType::Calls)).collect(),
        );

        let bytes =
            bincode::serde::encode_to_vec(&v1, bincode::config::standard()).expect("encode v1");
        let (decoded, _): (CallGraphV1, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                .expect("decode v1");
        let v2 = decoded.into_v2();
        assert_eq!(v2.symbol_count(), 2);
        assert_eq!(v2.edge_count(), 1);
    }
}
