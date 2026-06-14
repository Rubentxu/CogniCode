//! Mock implementations for cognicode-core domain traits.
//!
//! This crate provides mock implementations of all domain traits from
//! `cognicode-core`, useful for testing application code that depends
//! on these traits without requiring real implementations.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use lsp_types::Url;

// Import aggregates
use cognicode_core::domain::aggregates::{CallGraph, Refactor, RefactorKind, RefactorParameters, Symbol, SymbolId};
use cognicode_core::domain::services::CycleDetectionResult;

// Import value objects
use cognicode_core::domain::value_objects::{DependencyType, Location, SourceRange, SymbolKind};

// Import from code_intelligence sub-module directly
use cognicode_core::domain::traits::code_intelligence::{
    DocumentSymbol, DocumentSymbolKind, HoverInfo, Reference, ReferenceKind, TypeHierarchy,
    TypeHierarchyNode,
};

// Import from traits modules directly
use cognicode_core::domain::traits::code_intelligence::CodeIntelligenceProvider;
use cognicode_core::domain::traits::code_intelligence::CodeIntelligenceError;
use cognicode_core::domain::traits::dependency_repository::{DependencyError, DependencyRepository};
use cognicode_core::domain::traits::file_system::{FileSystem, TextEdit, VfsResult};
use cognicode_core::domain::traits::graph_store::{GraphStore, StoreError};
use cognicode_core::domain::traits::parser::{AstScanner, ParseError, Parser, ParsedTree, ScannedNode};
use cognicode_core::domain::traits::refactor_strategy::{
    PreparedEdits, RefactorError, RefactorResult, RefactorValidation, RefactorStrategy,
};
use cognicode_core::domain::traits::repository::{Repository, RepositoryError};
use cognicode_core::domain::traits::search_provider::{
    QueryValidation, Replacement, SearchError, SearchMatch, SearchProvider, SearchQuery, SearchScope,
    SimilarMatch,
};
use cognicode_core::domain::value_objects::EdgeMetadata;

// ============================================================================
// CodeIntelligenceProvider
// ============================================================================

/// Mock implementation of [`CodeIntelligenceProvider`].
#[derive(Debug, Default)]
pub struct MockCodeIntelligenceProvider {
    symbols: Vec<Symbol>,
    references: Vec<Reference>,
    hierarchy: Option<TypeHierarchy>,
    definition: Option<Location>,
    document_symbols: Vec<DocumentSymbol>,
    hover: Option<HoverInfo>,
}

impl MockCodeIntelligenceProvider {
    /// Creates a new empty mock provider.
    pub fn mock() -> Self {
        Self::default()
    }

    /// Configures the mock to return specific symbols.
    pub fn with_symbols(mut self, symbols: Vec<Symbol>) -> Self {
        self.symbols = symbols;
        self
    }

    /// Configures the mock to return specific references.
    pub fn with_references(mut self, references: Vec<Reference>) -> Self {
        self.references = references;
        self
    }

    /// Configures the mock to return a specific type hierarchy.
    pub fn with_hierarchy(mut self, hierarchy: TypeHierarchy) -> Self {
        self.hierarchy = Some(hierarchy);
        self
    }

    /// Configures the mock to return a specific definition location.
    pub fn with_definition(mut self, location: Location) -> Self {
        self.definition = Some(location);
        self
    }

    /// Configures the mock to return specific document symbols.
    pub fn with_document_symbols(mut self, symbols: Vec<DocumentSymbol>) -> Self {
        self.document_symbols = symbols;
        self
    }

    /// Configures the mock to return specific hover info.
    pub fn with_hover(mut self, hover: HoverInfo) -> Self {
        self.hover = Some(hover);
        self
    }
}

#[async_trait]
impl CodeIntelligenceProvider for MockCodeIntelligenceProvider {
    async fn get_symbols(
        &self,
        path: &std::path::Path,
    ) -> Result<Vec<Symbol>, CodeIntelligenceError> {
        if self.symbols.is_empty() {
            let loc = Location::new(path.to_str().unwrap_or("test.rs"), 0, 0);
            Ok(vec![
                Symbol::new("main", SymbolKind::Function, loc.clone()),
                Symbol::new("MyStruct", SymbolKind::Class, loc),
            ])
        } else {
            Ok(self.symbols.clone())
        }
    }

    async fn find_references(
        &self,
        location: &Location,
        _include_declaration: bool,
    ) -> Result<Vec<Reference>, CodeIntelligenceError> {
        if self.references.is_empty() {
            Ok(vec![Reference {
                location: location.clone(),
                reference_kind: ReferenceKind::Read,
                container: Some("main".to_string()),
            }])
        } else {
            Ok(self.references.clone())
        }
    }

    async fn get_hierarchy(
        &self,
        location: &Location,
    ) -> Result<TypeHierarchy, CodeIntelligenceError> {
        if let Some(ref h) = self.hierarchy {
            Ok(h.clone())
        } else {
            let symbol = Symbol::new("TestClass", SymbolKind::Class, location.clone());
            Ok(TypeHierarchy {
                symbol,
                parents: vec![],
                children: vec![],
            })
        }
    }

    async fn get_definition(
        &self,
        _location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError> {
        Ok(self.definition.clone())
    }

    async fn get_document_symbols(
        &self,
        path: &std::path::Path,
    ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
        if self.document_symbols.is_empty() {
            let loc = Location::new(path.to_str().unwrap_or("test.rs"), 0, 0);
            Ok(vec![DocumentSymbol {
                symbol: Symbol::new("MyFunction", SymbolKind::Function, loc),
                document_kind: DocumentSymbolKind::Function,
                range: SourceRange::new(
                    Location::new(path.to_str().unwrap_or("test.rs"), 0, 0),
                    Location::new(path.to_str().unwrap_or("test.rs"), 10, 0),
                ),
                children: vec![],
            }])
        } else {
            Ok(self.document_symbols.clone())
        }
    }

    async fn hover(&self, _location: &Location) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
        Ok(self.hover.clone())
    }
}

// ============================================================================
// DependencyRepository
// ============================================================================

/// Mock implementation of [`DependencyRepository`].
#[derive(Debug, Default)]
pub struct MockDependencyRepository {
    symbols: HashMap<SymbolId, Symbol>,
    dependencies: HashMap<SymbolId, HashSet<(SymbolId, DependencyType)>>,
}

impl MockDependencyRepository {
    /// Creates a new empty mock repository.
    pub fn mock() -> Self {
        Self::default()
    }

    /// Adds a symbol to the mock repository.
    pub fn add_symbol(mut self, id: SymbolId, symbol: Symbol) -> Self {
        self.symbols.insert(id, symbol);
        self
    }

    /// Adds a dependency to the mock repository.
    pub fn add_dependency(
        mut self,
        source: SymbolId,
        target: SymbolId,
        dep_type: DependencyType,
    ) -> Self {
        self.dependencies
            .entry(source)
            .or_insert_with(HashSet::new)
            .insert((target, dep_type));
        self
    }
}

impl DependencyRepository for MockDependencyRepository {
    fn add_dependency(
        &mut self,
        source_id: &SymbolId,
        target_id: &SymbolId,
        dependency_type: DependencyType,
    ) -> Result<(), DependencyError> {
        self.dependencies
            .entry(source_id.clone())
            .or_insert_with(HashSet::new)
            .insert((target_id.clone(), dependency_type));
        Ok(())
    }

    fn remove_symbol(&mut self, id: &SymbolId) -> Option<Symbol> {
        self.symbols.remove(id)
    }

    fn get_symbol(&self, id: &SymbolId) -> Option<&Symbol> {
        self.symbols.get(id)
    }

    fn get_all_symbols(&self) -> Vec<Symbol> {
        self.symbols.values().cloned().collect()
    }

    fn find_impact_scope(&self, id: &SymbolId) -> HashSet<SymbolId> {
        self.dependencies
            .get(id)
            .map(|deps| deps.iter().map(|(id, _)| id.clone()).collect())
            .unwrap_or_default()
    }

    fn find_dependents(&self, id: &SymbolId) -> HashSet<SymbolId> {
        let mut dependents = HashSet::new();
        for (source_id, deps) in &self.dependencies {
            for (target_id, _) in deps {
                if target_id == id {
                    dependents.insert(source_id.clone());
                }
            }
        }
        dependents
    }

    fn find_dependencies(&self, id: &SymbolId) -> HashSet<SymbolId> {
        self.dependencies
            .get(id)
            .map(|deps| deps.iter().map(|(id, _)| id.clone()).collect())
            .unwrap_or_default()
    }

    fn detect_cycles(&self) -> CycleDetectionResult {
        CycleDetectionResult {
            has_cycles: false,
            cycles: vec![],
            total_sccs: 0,
        }
    }

    fn has_path(&self, source: &SymbolId, target: &SymbolId) -> bool {
        self.find_dependencies(source).contains(target)
    }

    fn get_call_graph(&self) -> CallGraph {
        CallGraph::new()
    }

    fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    fn dependency_count(&self) -> usize {
        self.dependencies.values().map(|s| s.len()).sum()
    }
}

// ============================================================================
// FileSystem
// ============================================================================

/// Mock implementation of [`FileSystem`].
#[derive(Debug, Default)]
pub struct MockFileSystem {
    files: HashMap<Url, Arc<str>>,
}

impl MockFileSystem {
    /// Creates a new empty mock file system.
    pub fn mock() -> Self {
        Self::default()
    }

    /// Adds a file to the mock file system.
    pub fn with_file(mut self, url: Url, content: impl Into<String>) -> Self {
        self.files.insert(url, Arc::from(content.into()));
        self
    }
}

impl FileSystem for MockFileSystem {
    fn get_content(&self, url: &Url) -> Option<Arc<str>> {
        self.files.get(url).map(|s| s.clone())
    }

    fn set_content(&mut self, url: Url, content: String) {
        self.files.insert(url, Arc::from(content));
    }

    fn apply_edits(&mut self, edits: HashMap<Url, Vec<TextEdit>>) -> VfsResult<()> {
        for (url, text_edits) in edits {
            if let Some(content) = self.files.get_mut(&url) {
                let mut chars: Vec<char> = content.chars().collect();
                for edit in text_edits {
                    let (start, end) = edit.range;
                    let start = start as usize;
                    let end = end as usize;
                    if start <= chars.len() && end <= chars.len() && start <= end {
                        chars.splice(start..end, edit.new_text.chars());
                    }
                }
                let new_content: String = chars.into_iter().collect();
                self.files.insert(url, Arc::from(new_content));
            }
        }
        Ok(())
    }

    fn remove(&mut self, url: &Url) -> bool {
        self.files.remove(url).is_some()
    }

    fn exists(&self, url: &Url) -> bool {
        self.files.contains_key(url)
    }

    fn get_all_urls(&self) -> Vec<Url> {
        self.files.keys().cloned().collect()
    }

    fn file_count(&self) -> usize {
        self.files.len()
    }
}

// ============================================================================
// GraphStore
// ============================================================================

/// Mock implementation of [`GraphStore`].
#[derive(Debug, Default)]
pub struct MockGraphStore {
    graph: std::sync::Mutex<Option<Vec<u8>>>,
    manifest: std::sync::Mutex<Option<Vec<u8>>>,
}

impl MockGraphStore {
    /// Creates a new empty mock store.
    pub fn mock() -> Self {
        Self::default()
    }
}

impl GraphStore for MockGraphStore {
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        let bytes =
            bincode::serde::encode_to_vec(graph, bincode::config::standard())
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
        *self.graph.lock().unwrap() = Some(bytes);
        Ok(())
    }

    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        let guard = self.graph.lock().unwrap();
        match guard.as_ref() {
            Some(bytes) => {
                let (graph, _) = bincode::serde::decode_from_slice::<CallGraph, _>(
                    bytes,
                    bincode::config::standard(),
                )
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(graph))
            }
            None => Ok(None),
        }
    }

    fn save_manifest(
        &self,
        manifest: &cognicode_core::domain::value_objects::FileManifest,
    ) -> Result<(), StoreError> {
        let bytes =
            bincode::serde::encode_to_vec(manifest, bincode::config::standard())
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
        *self.manifest.lock().unwrap() = Some(bytes);
        Ok(())
    }

    fn load_manifest(
        &self,
    ) -> Result<Option<cognicode_core::domain::value_objects::FileManifest>, StoreError> {
        let guard = self.manifest.lock().unwrap();
        match guard.as_ref() {
            Some(bytes) => {
                let (manifest, _) = bincode::serde::decode_from_slice::<
                    cognicode_core::domain::value_objects::FileManifest,
                    _,
                >(bytes, bincode::config::standard())
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(manifest))
            }
            None => Ok(None),
        }
    }

    fn clear(&self) -> Result<(), StoreError> {
        *self.graph.lock().unwrap() = None;
        *self.manifest.lock().unwrap() = None;
        Ok(())
    }

    fn exists(&self) -> Result<bool, StoreError> {
        let graph_exists = self.graph.lock().unwrap().is_some();
        let manifest_exists = self.manifest.lock().unwrap().is_some();
        Ok(graph_exists || manifest_exists)
    }
}

// ============================================================================
// Parser
// ============================================================================

/// Mock implementation of [`Parser`].
#[derive(Debug, Default)]
pub struct MockParser {
    symbols: Vec<Symbol>,
    language_name: String,
}

impl MockParser {
    /// Creates a new mock parser that returns default symbols.
    pub fn mock() -> Self {
        Self {
            symbols: vec![],
            language_name: "rust".to_string(),
        }
    }

    /// Configures the mock to return specific symbols.
    pub fn with_symbols(mut self, symbols: Vec<Symbol>) -> Self {
        self.symbols = symbols;
        self
    }

    /// Configures the mock's language name.
    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language_name = lang.into();
        self
    }
}

impl Parser for MockParser {
    fn parse(&self, source: &str) -> Result<ParsedTree, ParseError> {
        // For mock purposes, we create a dummy tree using tree-sitter
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|e| ParseError::ParseFailed(e.to_string()))?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| ParseError::ParseFailed("Parse failed".to_string()))?;
        Ok(ParsedTree {
            tree,
            source: source.to_string(),
        })
    }

    fn find_function_definitions(&self, _source: &str) -> Result<Vec<Symbol>, ParseError> {
        if self.symbols.is_empty() {
            let loc = Location::new("test.rs", 0, 0);
            Ok(vec![Symbol::new("mock_func", SymbolKind::Function, loc)])
        } else {
            Ok(self.symbols.clone())
        }
    }

    fn find_all_symbols(&self, _source: &str) -> Result<Vec<Symbol>, ParseError> {
        if self.symbols.is_empty() {
            let loc1 = Location::new("test.rs", 0, 0);
            let loc2 = Location::new("test.rs", 10, 0);
            Ok(vec![
                Symbol::new("mock_func", SymbolKind::Function, loc1),
                Symbol::new("MockClass", SymbolKind::Class, loc2),
            ])
        } else {
            Ok(self.symbols.clone())
        }
    }

    fn language(&self) -> &str {
        &self.language_name
    }
}

// ============================================================================
// AstScanner
// ============================================================================

/// Mock implementation of [`AstScanner`].
#[derive(Debug, Default)]
pub struct MockAstScanner {
    scanned_nodes: Vec<ScannedNode<'static>>,
}

impl MockAstScanner {
    /// Creates a new mock scanner.
    pub fn mock() -> Self {
        Self::default()
    }

    /// Configures the mock to return specific scanned nodes.
    pub fn with_nodes(mut self, nodes: Vec<ScannedNode<'static>>) -> Self {
        self.scanned_nodes = nodes;
        self
    }
}

impl AstScanner for MockAstScanner {
    fn scan<'a>(
        &self,
        root: &'a tree_sitter::Tree,
        source: &str,
    ) -> Result<Vec<ScannedNode<'a>>, ParseError> {
        let _ = root;
        let _ = source;
        Ok(vec![])
    }

    fn find_nodes_by_type<'a>(
        &self,
        root: &'a tree_sitter::Tree,
        node_type: &str,
    ) -> Result<Vec<ScannedNode<'a>>, ParseError> {
        let _ = root;
        let _ = node_type;
        Ok(self
            .scanned_nodes
            .iter()
            .cloned()
            .map(|n| ScannedNode {
                node_type: Cow::Owned(n.node_type.into_owned()),
                range: n.range,
                children: n.children,
                symbol: n.symbol,
            })
            .collect())
    }

    fn get_node_text(&self, node: &tree_sitter::Node, source: &str) -> String {
        node.utf8_text(source.as_bytes()).unwrap_or_default().to_string()
    }

    fn node_to_range(&self, node: &tree_sitter::Node) -> SourceRange {
        let start = node.start_position();
        let end = node.end_position();
        SourceRange::new(
            Location::new("test.rs", start.row as u32 + 1, start.column as u32 + 1),
            Location::new("test.rs", end.row as u32 + 1, end.column as u32 + 1),
        )
    }
}

// ============================================================================
// RefactorStrategy
// ============================================================================

/// Mock implementation of [`RefactorStrategy`].
#[derive(Debug)]
pub struct MockRefactorStrategy {
    validation_result: RefactorValidation,
    prepare_edits_result: Option<PreparedEdits>,
    execute_result: Option<RefactorResult>,
    supported_kinds: Vec<RefactorKind>,
}

impl Default for MockRefactorStrategy {
    fn default() -> Self {
        // Create a default RefactorValidation with a dummy refactor
        let symbol = Symbol::new(
            "default_func",
            SymbolKind::Function,
            Location::new("default.rs", 0, 0),
        );
        let refactor = Refactor::new(RefactorKind::Rename, symbol, RefactorParameters::new());
        Self {
            validation_result: RefactorValidation::success(refactor),
            prepare_edits_result: None,
            execute_result: None,
            supported_kinds: vec![RefactorKind::Rename, RefactorKind::Extract],
        }
    }
}

impl MockRefactorStrategy {
    /// Creates a new mock strategy that succeeds by default.
    pub fn mock() -> Self {
        let symbol = Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let refactor = Refactor::new(RefactorKind::Rename, symbol, RefactorParameters::new());
        Self {
            validation_result: RefactorValidation::success(refactor),
            prepare_edits_result: None,
            execute_result: None,
            supported_kinds: vec![RefactorKind::Rename, RefactorKind::Extract],
        }
    }

    /// Configures the validation result.
    pub fn with_validation(mut self, result: RefactorValidation) -> Self {
        self.validation_result = result;
        self
    }

    /// Configures the prepare_edits result.
    pub fn with_prepared_edits(mut self, edits: PreparedEdits) -> Self {
        self.prepare_edits_result = Some(edits);
        self
    }

    /// Configures the execute result.
    pub fn with_execute_result(mut self, result: RefactorResult) -> Self {
        self.execute_result = Some(result);
        self
    }
}

impl RefactorStrategy for MockRefactorStrategy {
    fn validate(&self, _refactor: &Refactor) -> RefactorValidation {
        self.validation_result.clone()
    }

    fn prepare_edits(&self, _refactor: &Refactor) -> Result<PreparedEdits, RefactorError> {
        self.prepare_edits_result.clone().ok_or_else(|| {
            RefactorError::PreparationFailed("No mock result configured".to_string())
        })
    }

    fn execute(&self, refactor: &Refactor) -> Result<RefactorResult, RefactorError> {
        if let Some(ref result) = self.execute_result {
            Ok(result.clone())
        } else {
            Ok(RefactorResult::success(refactor.clone()))
        }
    }

    fn supported_kinds(&self) -> Vec<RefactorKind> {
        self.supported_kinds.clone()
    }
}

// ============================================================================
// Repository
// ============================================================================

/// Mock implementation of [`Repository`].
#[derive(Debug, Default)]
pub struct MockRepository {
    symbols: HashMap<String, Symbol>,
    edge_count: usize,
    edges_by_caller: HashMap<String, Vec<EdgeMetadata>>,
    edges_by_callee: HashMap<String, Vec<EdgeMetadata>>,
}

impl MockRepository {
    /// Creates a new empty mock repository.
    pub fn mock() -> Self {
        Self::default()
    }

    /// Adds a symbol to the mock repository.
    pub fn with_symbol(mut self, name: String, symbol: Symbol) -> Self {
        self.symbols.insert(name, symbol);
        self
    }

    /// Sets the edge count.
    pub fn with_edge_count(mut self, count: usize) -> Self {
        self.edge_count = count;
        self
    }

    /// Adds edges by caller.
    pub fn with_edges_by_caller(mut self, caller: String, edges: Vec<EdgeMetadata>) -> Self {
        self.edges_by_caller.insert(caller, edges);
        self
    }

    /// Adds edges by callee.
    pub fn with_edges_by_callee(mut self, callee: String, edges: Vec<EdgeMetadata>) -> Self {
        self.edges_by_callee.insert(callee, edges);
        self
    }
}

#[async_trait]
impl Repository for MockRepository {
    async fn find_symbol_by_qualified_name(
        &self,
        name: &str,
    ) -> Result<Option<Symbol>, RepositoryError> {
        Ok(self.symbols.get(name).cloned())
    }

    async fn count_symbols(&self) -> Result<usize, RepositoryError> {
        Ok(self.symbols.len())
    }

    async fn find_edges_by_caller(
        &self,
        caller_id: &str,
    ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
        Ok(self.edges_by_caller.get(caller_id).cloned().unwrap_or_default())
    }

    async fn find_edges_by_callee(
        &self,
        callee_id: &str,
    ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
        Ok(self.edges_by_callee.get(callee_id).cloned().unwrap_or_default())
    }

    async fn count_edges(&self) -> Result<usize, RepositoryError> {
        Ok(self.edge_count)
    }
}

// ============================================================================
// SearchProvider
// ============================================================================

/// Mock implementation of [`SearchProvider`].
#[derive(Debug)]
pub struct MockSearchProvider {
    search_results: Vec<SearchMatch>,
    replace_results: Vec<Replacement>,
    similar_results: Vec<SimilarMatch>,
    validation_result: QueryValidation,
}

impl Default for MockSearchProvider {
    fn default() -> Self {
        Self {
            search_results: vec![],
            replace_results: vec![],
            similar_results: vec![],
            validation_result: QueryValidation::valid(),
        }
    }
}

impl MockSearchProvider {
    /// Creates a new mock search provider.
    pub fn mock() -> Self {
        Self {
            search_results: vec![],
            replace_results: vec![],
            similar_results: vec![],
            validation_result: QueryValidation::valid(),
        }
    }

    /// Configures search results.
    pub fn with_search_results(mut self, results: Vec<SearchMatch>) -> Self {
        self.search_results = results;
        self
    }

    /// Configures replace results.
    pub fn with_replace_results(mut self, results: Vec<Replacement>) -> Self {
        self.replace_results = results;
        self
    }

    /// Configures similar results.
    pub fn with_similar_results(mut self, results: Vec<SimilarMatch>) -> Self {
        self.similar_results = results;
        self
    }

    /// Configures validation result.
    pub fn with_validation(mut self, result: QueryValidation) -> Self {
        self.validation_result = result;
        self
    }
}

#[async_trait]
impl SearchProvider for MockSearchProvider {
    async fn search(&self, _query: &SearchQuery) -> Result<Vec<SearchMatch>, SearchError> {
        Ok(self.search_results.clone())
    }

    async fn replace(
        &self,
        _matches: &[SearchMatch],
        _replacement: &str,
    ) -> Result<Vec<Replacement>, SearchError> {
        Ok(self.replace_results.clone())
    }

    async fn find_similar(&self, _location: &Location) -> Result<Vec<SimilarMatch>, SearchError> {
        Ok(self.similar_results.clone())
    }

    fn validate_query(&self, _query: &SearchQuery) -> QueryValidation {
        self.validation_result.clone()
    }
}

// ============================================================================
// SourceExtractor (multimodal feature)
// ============================================================================

#[cfg(feature = "multimodal")]
use cognicode_core::domain::traits::source_extractor::{ExtractedNode, SourceExtractor, SourceExtractorResult, SourcePath};

#[cfg(feature = "multimodal")]
use std::path::PathBuf;

#[cfg(feature = "multimodal")]
/// Mock implementation of [`SourceExtractor`].
#[derive(Debug, Default)]
pub struct MockSourceExtractor {
    source_kind_name: &'static str,
    extracted_nodes: Vec<ExtractedNode>,
}

#[cfg(feature = "multimodal")]
impl MockSourceExtractor {
    /// Creates a new mock source extractor.
    pub fn mock() -> Self {
        Self {
            source_kind_name: "mock",
            extracted_nodes: vec![],
        }
    }

    /// Sets the source kind name.
    pub fn with_source_kind(mut self, name: &'static str) -> Self {
        self.source_kind_name = name;
        self
    }

    /// Configures extracted nodes.
    pub fn with_extracted_nodes(mut self, nodes: Vec<ExtractedNode>) -> Self {
        self.extracted_nodes = nodes;
        self
    }
}

#[cfg(feature = "multimodal")]
#[async_trait]
impl SourceExtractor for MockSourceExtractor {
    fn source_kind(&self) -> &'static str {
        self.source_kind_name
    }

    async fn extract(
        &self,
        _source: SourcePath,
    ) -> SourceExtractorResult<Vec<ExtractedNode>> {
        Ok(self.extracted_nodes.clone())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----- CodeIntelligenceProvider -----

    #[tokio::test]
    async fn mock_code_intelligence_get_symbols() {
        let mock = MockCodeIntelligenceProvider::mock();
        let path = std::path::Path::new("test.rs");
        let result = mock.get_symbols(path).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn mock_code_intelligence_find_references() {
        let mock = MockCodeIntelligenceProvider::mock();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.find_references(&loc, true).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn mock_code_intelligence_get_definition() {
        let mock = MockCodeIntelligenceProvider::mock();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.get_definition(&loc).await.unwrap();
        assert!(result.is_none());
    }

    // ----- DependencyRepository -----

    #[test]
    fn mock_dependency_repo_basic() {
        let mut repo = MockDependencyRepository::mock();
        let loc1 = Location::new("test.rs", 0, 0);
        let _symbol1 = Symbol::new("func1", SymbolKind::Function, loc1);
        let id1 = SymbolId::new("test.rs:func1:0");
        let id2 = SymbolId::new("test.rs:func2:10");

        // Call the trait method directly
        DependencyRepository::add_dependency(&mut repo, &id1, &id2, DependencyType::Calls).unwrap();

        let deps = repo.find_dependencies(&id1);
        assert!(deps.contains(&id2));
        assert_eq!(repo.dependency_count(), 1);
    }

    #[test]
    fn mock_dependency_repo_symbol_count() {
        let repo = MockDependencyRepository::mock();
        assert_eq!(repo.symbol_count(), 0);
    }

    // ----- FileSystem -----

    #[test]
    fn mock_file_system_basic() {
        let mut fs = MockFileSystem::mock();
        let url = Url::parse("file:///test.rs").unwrap();
        fs.set_content(url.clone(), "fn main() {}".to_string());

        let content = fs.get_content(&url);
        assert!(content.is_some());
        assert_eq!(&*content.unwrap(), "fn main() {}");
    }

    #[test]
    fn mock_file_system_exists() {
        let mut fs = MockFileSystem::mock();
        let url = Url::parse("file:///exists.rs").unwrap();
        fs.set_content(url.clone(), "content".to_string());

        assert!(fs.exists(&url));
        assert!(!fs.exists(&Url::parse("file:///missing.rs").unwrap()));
    }

    // ----- GraphStore -----

    #[test]
    fn mock_graph_store_save_load() {
        let store = MockGraphStore::mock();
        let graph = CallGraph::new();
        store.save_graph(&graph).unwrap();
        let loaded = store.load_graph().unwrap();
        assert!(loaded.is_some());
    }

    #[test]
    fn mock_graph_store_clear() {
        let store = MockGraphStore::mock();
        let graph = CallGraph::new();
        store.save_graph(&graph).unwrap();
        store.clear().unwrap();
        assert!(!store.exists().unwrap());
    }

    // ----- Parser -----

    #[test]
    fn mock_parser_language() {
        let parser = MockParser::mock();
        assert_eq!(parser.language(), "rust");
    }

    #[test]
    fn mock_parser_find_symbols() {
        let parser = MockParser::mock();
        let result = parser.find_all_symbols("fn test() {}");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    // ----- RefactorStrategy -----

    #[test]
    fn mock_refactor_strategy_supported_kinds() {
        let strategy = MockRefactorStrategy::mock();
        let kinds = strategy.supported_kinds();
        assert!(kinds.contains(&RefactorKind::Rename));
        assert!(kinds.contains(&RefactorKind::Extract));
    }

    #[test]
    fn mock_refactor_strategy_validate() {
        let strategy = MockRefactorStrategy::mock();
        let symbol = Symbol::new(
            "test",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let refactor = Refactor::new(RefactorKind::Rename, symbol, RefactorParameters::new());
        let validation = strategy.validate(&refactor);
        assert!(validation.is_valid);
    }

    // ----- Repository -----

    #[tokio::test]
    async fn mock_repository_count() {
        let repo = MockRepository::mock();
        assert_eq!(repo.count_symbols().await.unwrap(), 0);
        assert_eq!(repo.count_edges().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn mock_repository_find_symbol() {
        let repo = MockRepository::mock();
        let result = repo
            .find_symbol_by_qualified_name("test")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    // ----- SearchProvider -----

    #[tokio::test]
    async fn mock_search_provider_validate() {
        let provider = MockSearchProvider::mock();
        let query = SearchQuery::new("test", SearchScope::Workspace);
        let validation = provider.validate_query(&query);
        assert!(validation.is_valid);
    }

    // ----- SourceExtractor (multimodal) -----

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn mock_source_extractor_basic() {
        let extractor = MockSourceExtractor::mock();
        assert_eq!(extractor.source_kind(), "mock");
        let result = extractor
            .extract(SourcePath::File(PathBuf::from("/dev/null")))
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
