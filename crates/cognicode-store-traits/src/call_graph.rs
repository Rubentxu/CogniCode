//! Aggregate for representing call graphs between symbols
//!
//! A call graph represents the dependencies and call relationships between symbols.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use crate::symbol::Symbol;
use crate::value_objects::DependencyType;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallGraph {
    /// Map from symbol identifier to symbol
    symbols: HashMap<SymbolId, Symbol>,
    /// Map from symbol identifier to set of (target_id, dependency_type) edges
    edges: HashMap<SymbolId, HashSet<(SymbolId, DependencyType)>>,
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
            self.reverse_edges
                .entry(id.clone())
                .or_default();
            self.name_index
                .entry(symbol.name().to_lowercase())
                .or_default()
                .push(id.clone());
            symbol
        });
        id
    }

    /// Adds a dependency edge between two symbols
    pub fn add_dependency(
        &mut self,
        source_id: &SymbolId,
        target_id: &SymbolId,
        dependency_type: DependencyType,
    ) -> Result<(), CallGraphError> {
        // Ensure both symbols exist
        if !self.symbols.contains_key(source_id) {
            return Err(CallGraphError::SymbolNotFound(source_id.clone()));
        }
        if !self.symbols.contains_key(target_id) {
            return Err(CallGraphError::SymbolNotFound(target_id.clone()));
        }

        // Add edge
        self.edges
            .entry(source_id.clone())
            .or_default()
            .insert((target_id.clone(), dependency_type));

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
                .map(move |(target, dep_type)| (source, target, dep_type))
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
            .map(|(target, dep_type)| (target, dep_type))
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
                for (next, _) in dependencies {
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
                    if !result.contains(symbol_id) && symbol_id != &current {
                        if result.insert(symbol_id.clone()) {
                            to_visit.push(symbol_id.clone());
                        }
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
            .map(|e| e.iter().map(|(id, dep)| (id.clone(), *dep)).collect())
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
            for (callee_id, _) in edges.iter() {
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
            for (target_id, dep_type) in edges {
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

    /// Removes a symbol and all its edges from the graph
    pub fn remove_symbol(&mut self, id: &SymbolId) -> Option<Symbol> {
        if let Some(symbol) = self.symbols.remove(id) {
            // Remove all outgoing edges
            if let Some(deps) = self.edges.remove(id) {
                for (target, _) in deps {
                    if let Some(rev) = self.reverse_edges.get_mut(&target) {
                        rev.remove(id);
                    }
                }
            }
            // Remove all incoming edges
            if let Some(callers) = self.reverse_edges.remove(id) {
                for caller in callers {
                    if let Some(edges) = self.edges.get_mut(&caller) {
                        edges.retain(|(t, _)| t != id);
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
}

impl Default for CallGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for CallGraph operations
#[derive(Debug, thiserror::Error)]
pub enum CallGraphError {
    #[error("Symbol not found: {0}")]
    SymbolNotFound(SymbolId),

    #[error("Symbol already exists: {0}")]
    SymbolAlreadyExists(SymbolId),
}

use std::fmt;
