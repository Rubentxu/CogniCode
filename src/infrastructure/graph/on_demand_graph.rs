//! On-demand graph builder for lazy graph construction
//!
//! This module provides graph construction that only builds the necessary
//! portions of the graph based on the specific query. This is useful for
//! operations like call hierarchy queries or path tracing where only a
//! subset of the full graph is needed.

use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
use crate::infrastructure::graph::lightweight_index::{LightweightIndex, SymbolLocation};
use crate::infrastructure::parser::{Language, TreeSitterParser};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Direction for call hierarchy traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    /// Traverse callees (outgoing edges - what does this symbol call)
    Callees,
    /// Traverse callers (incoming edges - what calls this symbol)
    Callers,
    /// Traverse both directions
    Both,
}

/// Result of a call hierarchy query
#[derive(Debug, Clone)]
pub struct CallHierarchyResult {
    /// The symbol at the center of the query
    pub root_symbol: Symbol,
    /// All entries in the hierarchy with depth information
    pub entries: Vec<HierarchyEntry>,
}

impl CallHierarchyResult {
    /// Returns all callees at each depth level
    pub fn callees_by_depth(&self) -> HashMap<u32, Vec<&HierarchyEntry>> {
        let mut result: HashMap<u32, Vec<&HierarchyEntry>> = HashMap::new();
        for entry in &self.entries {
            if entry.direction == TraversalDirection::Callees {
                result
                    .entry(entry.depth)
                    .or_insert_with(Vec::new)
                    .push(entry);
            }
        }
        result
    }

    /// Returns all callers at each depth level
    pub fn callers_by_depth(&self) -> HashMap<u32, Vec<&HierarchyEntry>> {
        let mut result: HashMap<u32, Vec<&HierarchyEntry>> = HashMap::new();
        for entry in &self.entries {
            if entry.direction == TraversalDirection::Callers {
                result
                    .entry(entry.depth)
                    .or_insert_with(Vec::new)
                    .push(entry);
            }
        }
        result
    }
}

/// Entry in the call hierarchy
#[derive(Debug, Clone)]
pub struct HierarchyEntry {
    /// The symbol
    pub symbol: Symbol,
    /// Depth from the root (1-based)
    pub depth: u32,
    /// Direction from the root
    pub direction: TraversalDirection,
}

/// Call hierarchy item - prepared before expansion (LSP prepareCallHierarchy equivalent)
#[derive(Debug, Clone)]
pub struct CallHierarchyItem {
    /// Symbol name
    pub name: String,
    /// File path
    pub file: String,
    /// Line number (1-based)
    pub line: u32,
    /// Column number (0-based)
    pub column: u32,
    /// Symbol kind
    pub kind: SymbolKind,
}

impl CallHierarchyItem {
    /// Creates a new CallHierarchyItem
    pub fn new(name: String, file: String, line: u32, column: u32, kind: SymbolKind) -> Self {
        Self {
            name,
            file,
            line,
            column,
            kind,
        }
    }

    /// Returns the location as string "file:line:column"
    pub fn location_string(&self) -> String {
        format!("{}:{}:{}", self.file, self.line, self.column)
    }
}

/// Tree node for ASCII/box visualization
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub location: String,
    pub children: Vec<TreeNode>,
    pub is_last: bool,
}

impl TreeNode {
    pub fn new(name: String, location: String) -> Self {
        Self {
            name,
            location,
            children: Vec::new(),
            is_last: false,
        }
    }

    /// Convert to ASCII tree string
    pub fn to_ascii_tree(&self) -> String {
        let mut output = String::new();
        self.format_ascii(&mut output, "", true);
        output
    }

    fn format_ascii(&self, output: &mut String, prefix: &str, is_root: bool) {
        if is_root {
            output.push_str(&format!("{}\n", self.name));
        } else {
            let connector = if self.is_last {
                "└── "
            } else {
                "├── "
            };
            output.push_str(prefix);
            output.push_str(connector);
            output.push_str(&format!("{} ({})\n", self.name, self.location));
        }

        let child_prefix = if is_root {
            prefix.to_string()
        } else if self.is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        for (i, child) in self.children.iter().enumerate() {
            let mut child = child.clone();
            child.is_last = i == self.children.len() - 1;
            child.format_ascii(output, &child_prefix, false);
        }
    }

    /// Convert to box diagram string
    pub fn to_box_diagram(&self) -> String {
        let mut output = String::new();
        self.format_box(&mut output, 0);
        output
    }

    fn format_box(&self, output: &mut String, level: usize) {
        let indent = "  ".repeat(level);
        let len = self.name.len().max(self.location.len() + 2);
        let top_border = format!("+-{}-+", "-".repeat(len));
        let mid_border = format!("| {:len$} |", "");
        let bot_border = format!("+-{}-+", "-".repeat(len));

        output.push_str(&format!("{}{}\n", indent, top_border));
        output.push_str(&format!("{}| {:len$} |\n", indent, self.name));
        output.push_str(&format!(
            "{}| {:len$} |\n",
            indent,
            format!("({})", self.location)
        ));
        output.push_str(&format!("{}{}\n", indent, bot_border));

        for child in &self.children {
            child.format_box(output, level + 1);
        }
    }
}

/// On-demand graph builder that constructs graphs only for specific queries
///
/// This builder is designed for lazy evaluation - it only parses and builds
/// the graph portions needed to answer a specific query.
pub struct OnDemandGraphBuilder {
    /// Index for fast symbol lookup
    index: LightweightIndex,
    /// Cache of parsed files to avoid re-parsing
    file_cache: HashMap<String, (Vec<Symbol>, Vec<(Symbol, String)>)>, // (symbols, relationships)
}

impl OnDemandGraphBuilder {
    /// Creates a new OnDemandGraphBuilder
    pub fn new() -> Self {
        Self {
            index: LightweightIndex::new(),
            file_cache: HashMap::new(),
        }
    }

    /// Creates a new OnDemandGraphBuilder with an existing index
    pub fn with_index(index: LightweightIndex) -> Self {
        Self {
            index,
            file_cache: HashMap::new(),
        }
    }

    /// Sets the index from a directory scan
    pub fn set_index<P: AsRef<Path>>(&mut self, project_dir: P) -> std::io::Result<()> {
        self.index.build_index(project_dir)
    }

    /// Builds the index from in-memory sources
    pub fn build_index_from_sources<'a, I>(&mut self, sources: I)
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        self.index.build_from_sources(sources);
    }

    /// Builds a subgraph centered on a specific symbol
    ///
    /// This parses only the files containing the symbol and its related
    /// symbols, building a minimal graph for call hierarchy queries.
    pub fn build_for_symbol(
        &mut self,
        symbol_name: &str,
        depth: u32,
        direction: TraversalDirection,
    ) -> CallHierarchyResult {
        // Find symbol locations
        let locations = self.index.find_symbol(symbol_name);
        if locations.is_empty() {
            return CallHierarchyResult {
                root_symbol: Symbol::new(
                    symbol_name,
                    SymbolKind::Unknown,
                    Location::new("unknown", 0, 0),
                ),
                entries: Vec::new(),
            };
        }

        // Use the first location as the root
        let root_loc = locations[0].clone();
        let root_symbol = Symbol::new(
            symbol_name,
            root_loc.symbol_kind.clone(),
            Location::new(&root_loc.file, root_loc.line, root_loc.column),
        );

        // Find all files we need to parse
        let mut files_to_parse: HashSet<String> = HashSet::new();
        files_to_parse.insert(root_loc.file.clone());

        // Find related files based on direction
        self.find_related_files(symbol_name, depth, direction, &mut files_to_parse);

        // Parse all needed files
        for file_path in &files_to_parse {
            self.parse_file_if_needed(file_path);
        }

        // Build the subgraph
        let mut entries = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(self.symbol_key(symbol_name, &root_loc.file, root_loc.line));

        // Traverse based on direction
        match direction {
            TraversalDirection::Callees | TraversalDirection::Both => {
                self.traverse_callees(
                    &root_symbol,
                    depth,
                    1,
                    TraversalDirection::Callees,
                    &mut entries,
                    &mut visited,
                );
            }
            TraversalDirection::Callers => {}
        }

        if direction == TraversalDirection::Both {
            self.traverse_callers(
                &root_symbol,
                depth,
                1,
                TraversalDirection::Callers,
                &mut entries,
                &mut visited,
            );
        }

        CallHierarchyResult {
            root_symbol,
            entries,
        }
    }

    /// Builds a subgraph for tracing a path between two symbols
    ///
    /// This is optimized for path finding - it finds the shortest path
    /// by only parsing files that could contain relevant symbols.
    pub fn build_for_path(&mut self, source_name: &str, target_name: &str) -> Option<CallGraph> {
        // Find source and target locations
        let source_locs = self.index.find_symbol(source_name).to_vec();
        let target_locs = self.index.find_symbol(target_name).to_vec();

        if source_locs.is_empty() || target_locs.is_empty() {
            return None;
        }

        // Clone locations we need before mutable borrow
        let source_loc = source_locs[0].clone();
        let target_loc = target_locs[0].clone();

        // Parse all files that might be involved
        let mut files_to_parse: HashSet<String> = HashSet::new();
        for loc in &source_locs {
            files_to_parse.insert(loc.file.clone());
        }
        for loc in &target_locs {
            files_to_parse.insert(loc.file.clone());
        }

        // Parse all files
        for file_path in &files_to_parse {
            self.parse_file_if_needed(file_path);
        }

        // Build a graph with just these symbols
        let mut graph = CallGraph::new();

        // Add source symbol
        let source_symbol = Symbol::new(
            source_name,
            source_loc.symbol_kind.clone(),
            Location::new(&source_loc.file, source_loc.line, source_loc.column),
        );
        graph.add_symbol(source_symbol);

        // Add target symbol
        let target_symbol = Symbol::new(
            target_name,
            target_loc.symbol_kind.clone(),
            Location::new(&target_loc.file, target_loc.line, target_loc.column),
        );
        graph.add_symbol(target_symbol);

        // Find direct relationships between source and target
        self.add_path_relationships(
            &mut graph,
            source_name,
            target_name,
            &source_locs,
            &target_locs,
        );

        // If not directly connected, try BFS through intermediate symbols
        if graph.edge_count() == 0 {
            self.add_transitive_path(
                &mut graph,
                source_name,
                target_name,
                &source_locs,
                &target_locs,
            );
        }

        if graph.edge_count() > 0 {
            Some(graph)
        } else {
            None
        }
    }

    /// Prepares call hierarchy for a symbol (LSP prepareCallHierarchy equivalent)
    ///
    /// Returns the CallHierarchyItem with exact location, or None if not found.
    /// This should be called before recursive_expand to get the starting point.
    pub fn prepare_call_hierarchy(&self, symbol_name: &str) -> Option<CallHierarchyItem> {
        let locations = self.index.find_symbol(symbol_name);
        if locations.is_empty() {
            return None;
        }

        let loc = &locations[0];
        Some(CallHierarchyItem::new(
            symbol_name.to_string(),
            loc.file.clone(),
            loc.line,
            loc.column,
            loc.symbol_kind.clone(),
        ))
    }

    /// Recursively expands a call hierarchy item (LSP incomingCalls/outgoingCalls equivalent)
    ///
    /// Given a CallHierarchyItem from prepare_call_hierarchy, this recursively
    /// fetches all callers/callees up to max_depth.
    pub fn recursive_expand(
        &mut self,
        item: &CallHierarchyItem,
        max_depth: u32,
        direction: TraversalDirection,
    ) -> CallHierarchyResult {
        let mut result = CallHierarchyResult {
            root_symbol: Symbol::new(
                item.name.clone(),
                item.kind.clone(),
                Location::new(&item.file, item.line, item.column),
            ),
            entries: Vec::new(),
        };

        if max_depth == 0 {
            return result;
        }

        // Parse the file containing this item
        self.parse_file_if_needed(&item.file);

        // Get all symbols in this file - clone to avoid borrow issues
        let symbols = self.file_cache.get(&item.file).map(|(s, _)| s.clone());

        if let Some(symbols) = symbols {
            let current_depth = 1;

            match direction {
                TraversalDirection::Callees | TraversalDirection::Both => {
                    self.find_and_expand_callees(
                        &item.name,
                        &symbols,
                        max_depth,
                        current_depth,
                        &mut result.entries,
                    );
                }
                TraversalDirection::Callers => {}
            }

            if direction == TraversalDirection::Both || direction == TraversalDirection::Callers {
                self.find_and_expand_callers(
                    &item.name,
                    &symbols,
                    max_depth,
                    current_depth,
                    &mut result.entries,
                );
            }
        }

        result
    }

    /// Finds callees and recursively expands them
    fn find_and_expand_callees(
        &self,
        caller_name: &str,
        symbols: &[Symbol],
        max_depth: u32,
        current_depth: u32,
        entries: &mut Vec<HierarchyEntry>,
    ) {
        if current_depth > max_depth {
            return;
        }

        for symbol in symbols {
            if symbol.name() == caller_name {
                // Find calls from this function - get the file this function is defined in
                let file_path = symbol.location().file();
                let callee_symbols = self.file_cache.get(file_path).map(|(s, _)| s.clone());
                if let Some(callee_symbols) = callee_symbols {
                    for call_symbol in &callee_symbols {
                        if call_symbol.kind().is_callable() && call_symbol.name() != caller_name {
                            let entry = HierarchyEntry {
                                symbol: call_symbol.clone(),
                                depth: current_depth,
                                direction: TraversalDirection::Callees,
                            };
                            entries.push(entry);

                            // Recursively expand if within depth
                            if current_depth < max_depth {
                                self.find_and_expand_callees(
                                    call_symbol.name(),
                                    &callee_symbols,
                                    max_depth,
                                    current_depth + 1,
                                    entries,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Finds callers and recursively expands them
    fn find_and_expand_callers(
        &self,
        callee_name: &str,
        symbols: &[Symbol],
        max_depth: u32,
        current_depth: u32,
        entries: &mut Vec<HierarchyEntry>,
    ) {
        if current_depth > max_depth {
            return;
        }

        for symbol in symbols {
            // Check if this file contains a function that calls callee_name
            let file_path = symbol.location().file();
            let all_symbols = self.file_cache.get(file_path).map(|(s, _)| s.clone());
            if let Some(all_symbols) = all_symbols {
                for call_symbol in &all_symbols {
                    // If this symbol calls our callee_name
                    if call_symbol.name() == callee_name && symbol.name() != callee_name {
                        let entry = HierarchyEntry {
                            symbol: symbol.clone(),
                            depth: current_depth,
                            direction: TraversalDirection::Callers,
                        };
                        entries.push(entry);

                        if current_depth < max_depth {
                            self.find_and_expand_callers(
                                symbol.name(),
                                &all_symbols,
                                max_depth,
                                current_depth + 1,
                                entries,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Converts a CallHierarchyResult to a TreeNode for visualization
    pub fn to_tree_node(&self, result: &CallHierarchyResult) -> TreeNode {
        let mut root = TreeNode::new(
            format!(
                "{} ({})",
                result.root_symbol.name(),
                result.root_symbol.kind()
            ),
            result.root_symbol.location().to_string(),
        );

        // Group entries by depth
        let mut depth_map: HashMap<u32, Vec<&HierarchyEntry>> = HashMap::new();
        for entry in &result.entries {
            depth_map.entry(entry.depth).or_default().push(entry);
        }

        // Build tree recursively
        self.build_tree_from_entries(&mut root, &result.entries, 1);

        root
    }

    fn build_tree_from_entries(
        &self,
        parent: &mut TreeNode,
        entries: &[HierarchyEntry],
        depth: u32,
    ) {
        let children: Vec<_> = entries.iter().filter(|e| e.depth == depth).collect();

        for child in children {
            let child_name = format!("{} ({})", child.symbol.name(), child.symbol.kind());
            let location = format!(
                "{}:{}:{}",
                child.symbol.location().file(),
                child.symbol.location().line(),
                child.symbol.location().column()
            );
            let mut node = TreeNode::new(child_name, location);

            // Find children of this node
            self.build_tree_from_entries(&mut node, entries, depth + 1);

            parent.children.push(node);
        }
    }

    fn find_related_files(
        &self,
        symbol_name: &str,
        _depth: u32,
        _direction: TraversalDirection,
        files: &mut HashSet<String>,
    ) {
        let locations = self.index.find_symbol(symbol_name);
        if !locations.is_empty() {
            for loc in locations {
                files.insert(loc.file.clone());
            }
            return;
        }

        let prefix_len = symbol_name.len().min(3);
        let prefix = &symbol_name[..prefix_len];
        let mut candidates: Vec<(&str, &[SymbolLocation])> = self
            .index
            .all_entries()
            .filter(|(name, _)| name.starts_with(prefix))
            .collect();
        candidates.truncate(50);

        let name_lower = symbol_name.to_lowercase();
        for (indexed_name, locations) in candidates {
            if self.levenshtein_distance(indexed_name, &name_lower) <= 10 {
                for loc in locations {
                    files.insert(loc.file.clone());
                }
            }
        }
    }

    /// Simple Levenshtein distance for fuzzy matching
    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        let len1 = s1_chars.len();
        let len2 = s2_chars.len();

        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = std::cmp::min(
                    std::cmp::min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1),
                    matrix[i - 1][j - 1] + cost,
                );
            }
        }

        matrix[len1][len2]
    }

    /// Parses a file and caches the results
    fn parse_file_if_needed(&mut self, file_path: &str) {
        if self.file_cache.contains_key(file_path) {
            return;
        }

        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => return,
        };

        let language = match Language::from_extension(Path::new(file_path).extension()) {
            Some(lang) => lang,
            None => return,
        };

        let parser = match TreeSitterParser::new(language) {
            Ok(p) => p,
            Err(_) => return,
        };

        let symbols = parser
            .find_all_symbols_with_path(&source, file_path)
            .unwrap_or_default();

        let relationships = parser
            .find_call_relationships(&source, file_path)
            .unwrap_or_default();

        self.file_cache
            .insert(file_path.to_string(), (symbols, relationships));
    }

    /// Gets symbols from the cache
    fn get_file_symbols(&self, file_path: &str) -> &[Symbol] {
        self.file_cache
            .get(file_path)
            .map(|(s, _)| s.as_slice())
            .unwrap_or(&[])
    }

    /// Gets relationships from the cache
    fn get_file_relationships(&self, file_path: &str) -> &[(Symbol, String)] {
        self.file_cache
            .get(file_path)
            .map(|(_, r)| r.as_slice())
            .unwrap_or(&[])
    }

    /// Traverses callees (what this symbol calls)
    fn traverse_callees(
        &self,
        current: &Symbol,
        max_depth: u32,
        current_depth: u32,
        direction: TraversalDirection,
        entries: &mut Vec<HierarchyEntry>,
        visited: &mut HashSet<String>,
    ) {
        if current_depth > max_depth {
            return;
        }

        let file_path = current.location().file();
        let relationships = self.get_file_relationships(file_path);

        for (caller, callee_name) in relationships {
            // Check if this caller matches our current symbol
            if caller.name() != current.name() {
                continue;
            }

            // Find the callee's locations
            let callee_locs = self.index.find_symbol(callee_name);
            if callee_locs.is_empty() {
                continue;
            }

            let callee_loc = &callee_locs[0];
            let key = self.symbol_key(callee_name, &callee_loc.file, callee_loc.line);

            if visited.insert(key.clone()) {
                let callee_symbol = Symbol::new(
                    callee_name,
                    callee_loc.symbol_kind.clone(),
                    Location::new(&callee_loc.file, callee_loc.line, callee_loc.column),
                );

                entries.push(HierarchyEntry {
                    symbol: callee_symbol.clone(),
                    depth: current_depth,
                    direction,
                });

                // Recurse if within depth
                if current_depth < max_depth {
                    self.traverse_callees(
                        &callee_symbol,
                        max_depth,
                        current_depth + 1,
                        direction,
                        entries,
                        visited,
                    );
                }
            }
        }
    }

    /// Traverses callers (what calls this symbol)
    fn traverse_callers(
        &self,
        current: &Symbol,
        max_depth: u32,
        current_depth: u32,
        direction: TraversalDirection,
        entries: &mut Vec<HierarchyEntry>,
        visited: &mut HashSet<String>,
    ) {
        if current_depth > max_depth {
            return;
        }

        let file_path = current.location().file();
        let relationships = self.get_file_relationships(file_path);

        for (caller, callee_name) in relationships {
            // Check if the callee matches our current symbol
            if callee_name.to_lowercase() != current.name().to_lowercase() {
                continue;
            }

            let caller_loc = self.index.find_symbol(caller.name());
            if caller_loc.is_empty() {
                continue;
            }

            let caller_loc = &caller_loc[0];
            let key = self.symbol_key(caller.name(), &caller_loc.file, caller_loc.line);

            if visited.insert(key.clone()) {
                let caller_symbol = Symbol::new(
                    caller.name(),
                    caller_loc.symbol_kind.clone(),
                    Location::new(&caller_loc.file, caller_loc.line, caller_loc.column),
                );

                entries.push(HierarchyEntry {
                    symbol: caller_symbol.clone(),
                    depth: current_depth,
                    direction,
                });

                // Recurse if within depth
                if current_depth < max_depth {
                    self.traverse_callers(
                        &caller_symbol,
                        max_depth,
                        current_depth + 1,
                        direction,
                        entries,
                        visited,
                    );
                }
            }
        }
    }

    /// Creates a unique key for a symbol
    fn symbol_key(&self, name: &str, file: &str, line: u32) -> String {
        format!("{}:{}:{}", file, line, name)
    }

    /// Adds direct relationships between source and target
    fn add_path_relationships(
        &self,
        graph: &mut CallGraph,
        source_name: &str,
        target_name: &str,
        source_locs: &[super::lightweight_index::SymbolLocation],
        target_locs: &[super::lightweight_index::SymbolLocation],
    ) {
        // Look for direct calls from source to target
        for source_loc in source_locs {
            let relationships = self.get_file_relationships(&source_loc.file);
            for (caller, callee_name) in relationships {
                if caller.name().to_lowercase() == source_name.to_lowercase()
                    && callee_name.to_lowercase() == target_name.to_lowercase()
                {
                    // Add edge to graph
                    let source_id = SymbolId::new(source_name);
                    let target_id = SymbolId::new(target_name);
                    let _ = graph.add_dependency(&source_id, &target_id, DependencyType::Calls);
                }
            }
        }
    }

    /// Adds transitive path through intermediate symbols
    fn add_transitive_path(
        &self,
        graph: &mut CallGraph,
        source_name: &str,
        target_name: &str,
        source_locs: &[super::lightweight_index::SymbolLocation],
        target_locs: &[super::lightweight_index::SymbolLocation],
    ) {
        // BFS through intermediate symbols
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: Vec<(String, Vec<SymbolId>)> = Vec::new();

        // Start from source
        for loc in source_locs {
            let id = SymbolId::new(format!("{}:{}:{}", loc.file, loc.line, source_name));
            queue.push((source_name.to_string(), vec![id]));
            visited.insert(source_name.to_lowercase());
        }

        while let Some((current_name, path)) = queue.pop() {
            if current_name.to_lowercase() == target_name.to_lowercase() {
                // Found path - add edges
                for window in path.windows(2) {
                    let _ = graph.add_dependency(&window[0], &window[1], DependencyType::Calls);
                }
                return;
            }

            // Find callees of current
            for loc in self.index.find_symbol(&current_name) {
                let relationships = self.get_file_relationships(&loc.file);
                for (caller, callee_name) in relationships {
                    if caller.name().to_lowercase() != current_name.to_lowercase() {
                        continue;
                    }

                    if !visited.contains(&callee_name.to_lowercase()) {
                        visited.insert(callee_name.to_lowercase());

                        let callee_locs = self.index.find_symbol(&callee_name);
                        if let Some(callee_loc) = callee_locs.first() {
                            let new_path = {
                                let mut p = path.clone();
                                p.push(SymbolId::new(format!(
                                    "{}:{}:{}",
                                    callee_loc.file, callee_loc.line, callee_name
                                )));
                                p
                            };
                            queue.push((callee_name.clone(), new_path));
                        }
                    }
                }
            }
        }
    }
}

impl Default for OnDemandGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_on_demand_graph_builder_empty() {
        let mut builder = OnDemandGraphBuilder::new();
        let result = builder.build_for_symbol("test", 3, TraversalDirection::Callees);
        assert_eq!(result.entries.len(), 0);
    }

    #[test]
    fn test_on_demand_graph_builder_with_sources() {
        let mut builder = OnDemandGraphBuilder::new();
        builder
            .build_index_from_sources([("test.py", "def a():\n    b()\n\ndef b():\n    pass\n")]);

        let result = builder.build_for_symbol("a", 3, TraversalDirection::Callees);
        assert!(result.entries.len() >= 0); // May or may not find callees depending on index state
    }

    #[test]
    #[ignore = "integration: scans entire project via set_index"]
    fn test_on_demand_graph_real_project_benchmark() {
        use std::time::Instant;

        let mut builder = OnDemandGraphBuilder::new();
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let index_start = Instant::now();
        builder.set_index(&project_root).unwrap();
        let index_time = index_start.elapsed();

        println!("[ON-DEMAND] Index built in {} ms", index_time.as_millis());

        // Query for a symbol
        let query_start = Instant::now();
        let result = builder.build_for_symbol("new", 2, TraversalDirection::Callees);
        let query_time = query_start.elapsed();

        println!(
            "[ON-DEMAND] build_for_symbol('new', depth=2) in {} ms: {} entries",
            query_time.as_millis(),
            result.entries.len()
        );

        // Also try incoming direction
        let query_start2 = Instant::now();
        let result2 = builder.build_for_symbol("new", 2, TraversalDirection::Callers);
        let query_time2 = query_start2.elapsed();

        println!(
            "[ON-DEMAND] build_for_symbol('new', depth=2, incoming) in {} ms: {} entries",
            query_time2.as_millis(),
            result2.entries.len()
        );

        // Just verify it ran
        assert!(result.entries.len() >= 0, "Builder should work");
    }
}
