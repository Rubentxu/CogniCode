//! Aggregate for representing call graphs between symbols
//!
//! A call graph represents the dependencies and call relationships between symbols.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use super::symbol::Symbol;
use crate::domain::events::GraphEvent;
use crate::domain::value_objects::DependencyType;

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
                for (target_id, dep_type) in edges {
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
            for (target_id, dep_type) in dependencies {
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
                        let new_symbol = Symbol::new(
                            e.name.clone(),
                            *symbol.kind(),
                            e.new_location.clone(),
                        );
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
                                if let Some(_entry) = edges.take(&old_target) {
                                    edges.insert(new_target);
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
                        edges.retain(|(t, dt)| t != &target_id || dt != &e.dependency_type);
                    }
                    if let Some(callers) = self.reverse_edges.get_mut(&target_id) {
                        callers.remove(&source_id);
                    }
                }
                // Graph-level events are not applicable to symbol-level apply_events
                GraphEvent::GraphReplaced | GraphEvent::GraphCleared | GraphEvent::GraphModified => {
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
        use bincode::serde::{decode_from_slice, encode_to_vec};
        use bincode::config::standard;

        let mut graph = CallGraph::new();
        let symbol1 = Symbol::new("func_a", SymbolKind::Function, Location::new("test.rs", 10, 1));
        let symbol2 = Symbol::new("func_b", SymbolKind::Function, Location::new("test.rs", 20, 1));

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
}
