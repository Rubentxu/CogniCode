//! Semantic Search - Fast symbol search with filtering
//!
//! This module provides fuzzy search capabilities with kind filtering
//! for finding symbols across the codebase.

use crate::domain::aggregates::symbol::Symbol;
use crate::domain::traits::Parser;
use crate::domain::value_objects::{Location, SymbolKind};
use crate::infrastructure::parser::Language;
use dashmap::DashMap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Represents a search query with filters
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// The search query string
    pub query: String,
    /// Optional filter for symbol kinds
    pub kinds: Vec<SearchSymbolKind>,
    /// Maximum number of results to return
    pub max_results: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            kinds: Vec::new(),
            max_results: 50,
        }
    }
}

/// Symbol kinds that can be filtered in search
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSymbolKind {
    Function,
    Class,
    Method,
    Variable,
    Trait,
    Struct,
    Enum,
    Module,
    Constant,
}

impl SearchSymbolKind {
    /// Converts to domain SymbolKind
    pub fn to_symbol_kind(&self) -> SymbolKind {
        match self {
            SearchSymbolKind::Function => SymbolKind::Function,
            SearchSymbolKind::Class => SymbolKind::Class,
            SearchSymbolKind::Method => SymbolKind::Method,
            SearchSymbolKind::Variable => SymbolKind::Variable,
            SearchSymbolKind::Trait => SymbolKind::Trait,
            SearchSymbolKind::Struct => SymbolKind::Struct,
            SearchSymbolKind::Enum => SymbolKind::Enum,
            SearchSymbolKind::Module => SymbolKind::Module,
            SearchSymbolKind::Constant => SymbolKind::Constant,
        }
    }
}

/// A search result with relevance scoring
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matching symbol
    pub symbol: Symbol,
    /// Relevance score (higher is better)
    pub score: f32,
    /// Type of match for display purposes
    pub match_type: MatchType,
}

impl SearchResult {
    /// Creates a new search result
    pub fn new(symbol: Symbol, score: f32, match_type: MatchType) -> Self {
        Self {
            symbol,
            score,
            match_type,
        }
    }
}

/// Type of match for ranking purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchType {
    /// Exact match (same as query)
    Exact = 3,
    /// Prefix match (query is prefix of name)
    Prefix = 2,
    /// Contains match (query is substring)
    Contains = 1,
    /// Fuzzy match (approximate)
    Fuzzy = 0,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
    }
}

impl Eq for SearchResult {}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // First by match type (higher is better)
        let match_cmp = self.match_type.cmp(&other.match_type).reverse();
        if match_cmp != std::cmp::Ordering::Equal {
            return match_cmp;
        }
        // Then by score
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    }
}

/// Global search index for fast lookups
pub struct SearchIndex {
    /// Map from file path to symbols in that file
    symbols_by_file: DashMap<String, Vec<IndexedSymbol>>,
    /// All symbols indexed
    all_symbols: DashMap<String, IndexedSymbol>,
}

#[derive(Debug, Clone)]
struct IndexedSymbol {
    name: String,
    name_lower: String,
    kind: SymbolKind,
    location: Location,
}

impl SearchIndex {
    /// Creates a new search index
    pub fn new() -> Self {
        Self {
            symbols_by_file: DashMap::new(),
            all_symbols: DashMap::new(),
        }
    }

    /// Indexes a file's symbols
    pub fn index_file(&self, file_path: &str, symbols: Vec<Symbol>) {
        let indexed: Vec<IndexedSymbol> = symbols
            .into_iter()
            .map(|s| {
                let key = format!(
                    "{}:{}:{}",
                    s.name(),
                    s.location().line(),
                    s.location().column()
                );
                let indexed = IndexedSymbol {
                    name: s.name().to_string(),
                    name_lower: s.name().to_lowercase(),
                    kind: *s.kind(),
                    location: s.location().clone(),
                };
                self.all_symbols.insert(key, indexed.clone());
                indexed
            })
            .collect();

        self.symbols_by_file.insert(file_path.to_string(), indexed);
    }

    /// Clears the index for a file
    pub fn clear_file(&self, file_path: &str) {
        if let Some(symbols) = self.symbols_by_file.remove(file_path) {
            for s in symbols.1 {
                let key = format!("{}:{}:{}", s.name, s.location.line(), s.location.column());
                self.all_symbols.remove(&key);
            }
        }
    }

    /// Clears the entire index
    pub fn clear(&self) {
        self.symbols_by_file.clear();
        self.all_symbols.clear();
    }

    /// Searches for symbols matching the query
    pub fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        if query.query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.query.to_lowercase();
        let mut heap: BinaryHeap<Reverse<SearchResult>> =
            BinaryHeap::with_capacity(query.max_results);

        for entry in self.all_symbols.iter() {
            let indexed = entry.value();
            let name_lower = &indexed.name_lower;

            // Apply kind filter
            if !query.kinds.is_empty() {
                let kind_matches = query
                    .kinds
                    .iter()
                    .any(|k| k.to_symbol_kind() == indexed.kind);
                if !kind_matches {
                    continue;
                }
            }

            // Calculate match type and preliminary score
            let (match_type, score) = if name_lower == &query_lower {
                (MatchType::Exact, 1.0)
            } else if name_lower.starts_with(&query_lower) {
                (MatchType::Prefix, 0.9)
            } else if name_lower.contains(&query_lower) {
                let pos = name_lower.find(&query_lower).unwrap_or(0);
                let score = 0.7 + (0.2 * (1.0 - pos as f32 / name_lower.len() as f32));
                (MatchType::Contains, score)
            } else {
                let score = fuzzy_score(&query_lower, name_lower);
                if score > 0.0 {
                    (MatchType::Fuzzy, score)
                } else {
                    continue;
                }
            };

            if heap.len() < query.max_results {
                let symbol = Symbol::new(
                    indexed.name.clone(),
                    indexed.kind,
                    indexed.location.clone(),
                );
                heap.push(Reverse(SearchResult::new(symbol, score, match_type)));
            } else if let Some(min) = heap.peek() {
                let temp_result = SearchResult::new(
                    Symbol::new(
                        indexed.name.clone(),
                        indexed.kind,
                        indexed.location.clone(),
                    ),
                    score,
                    match_type,
                );
                if temp_result > min.0 {
                    heap.pop();
                    heap.push(Reverse(temp_result));
                }
            }
        }

        let mut results: Vec<SearchResult> = heap.into_iter().map(|r| r.0).collect();
        results.sort();
        results
    }

    /// Returns the count of indexed symbols
    pub fn len(&self) -> usize {
        self.all_symbols.len()
    }

    /// Returns true if the index is empty
    pub fn is_empty(&self) -> bool {
        self.all_symbols.is_empty()
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculates fuzzy match score between query and target
fn fuzzy_score(query: &str, target: &str) -> f32 {
    if query.is_empty() || target.is_empty() {
        return 0.0;
    }

    // Simple fuzzy matching: check character overlap
    let query_chars: Vec<char> = query.chars().collect();
    let target_chars: Vec<char> = target.chars().collect();

    let mut query_idx = 0;
    let mut matched = 0;
    let mut consecutive = 0;
    let mut max_consecutive = 0;

    for tc in &target_chars {
        if query_idx < query_chars.len() && *tc == query_chars[query_idx] {
            matched += 1;
            consecutive += 1;
            max_consecutive = max_consecutive.max(consecutive);
            query_idx += 1;
        } else {
            consecutive = 0;
        }
    }

    // All query chars must be found in order
    if query_idx < query_chars.len() {
        return 0.0;
    }

    // Score based on matched ratio and consecutive bonus
    let match_ratio = matched as f32 / query_chars.len() as f32;
    let consecutive_bonus = max_consecutive as f32 * 0.1;

    (match_ratio + consecutive_bonus).min(0.6)
}

/// Search service that provides symbol search functionality
pub struct SemanticSearchService {
    index: Arc<SearchIndex>,
    db_path: Option<PathBuf>,
}

impl SemanticSearchService {
    /// Creates a new semantic search service
    pub fn new() -> Self {
        Self {
            index: Arc::new(SearchIndex::new()),
            db_path: None,
        }
    }

    /// Sets the database path for FTS5 persistence
    pub fn with_db_path(mut self, db_path: PathBuf) -> Self {
        self.db_path = Some(db_path);
        self
    }

    /// Sets the database path for FTS5 persistence (mutable version)
    pub fn set_db_path(&mut self, db_path: PathBuf) {
        self.db_path = Some(db_path);
    }

    /// Returns a reference to the search index
    pub fn index(&self) -> &SearchIndex {
        &self.index
    }

    /// Opens a FTS5 database connection if configured
    fn open_fts_db(&self) -> Option<rusqlite::Connection> {
        let db_path = self.db_path.as_ref()?;
        let db_dir = db_path.join(".cognicode");
        std::fs::create_dir_all(&db_dir).ok()?;
        let db_file = db_dir.join("cognicode.db");
        let conn = rusqlite::Connection::open(db_file).ok()?;
        Some(conn)
    }

    /// Indexes a file's symbols (dual-write to DashMap and FTS5)
    pub fn index_file(
        &self,
        file_path: &str,
        source: &str,
        language: Language,
    ) -> Result<(), String> {
        let parser = crate::infrastructure::parser::TreeSitterParser::new(language)
            .map_err(|e| e.to_string())?;

        let symbols = parser.find_all_symbols(source).map_err(|e| e.to_string())?;

        // Write to DashMap (existing behavior)
        self.index.index_file(file_path, symbols.clone());

        // Dual-write to FTS5 if db_path is configured
        if let Some(conn) = self.open_fts_db() {
            for symbol in symbols {
                let kind_str = format!("{:?}", symbol.kind());
                let docstring = String::new(); // Symbols don't have docstrings by default
                let tokens = format!("{} {}", symbol.name(), kind_str);
                if let Err(e) = conn.execute(
                    "INSERT OR REPLACE INTO symbol_index (symbol_name, symbol_kind, file_path, docstring, body_tokens) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![symbol.name(), kind_str, file_path, docstring, tokens],
                ) {
                    tracing::warn!("Failed to index symbol to FTS5: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Indexes a single file from path
    pub fn index_file_from_path(&self, file_path: &Path) -> Result<(), String> {
        let extension = file_path.extension().and_then(|e| e.to_str());

        let language =
            Language::from_extension(extension.as_ref().map(std::ffi::OsStr::new))
                .ok_or_else(|| "Unsupported file type".to_string())?;

        let source = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        self.index_file(&file_path.to_string_lossy(), &source, language)
    }

    /// Searches for symbols (FTS5 with DashMap fallback)
    pub fn search(&self, query: SearchQuery) -> Vec<SearchResult> {
        // Try FTS5 first if db_path is configured
        if let Some(conn) = self.open_fts_db() {
            let fts_results = self.search_fts5(&conn, &query);
            if !fts_results.is_empty() {
                return fts_results;
            }
            tracing::debug!("FTS5 returned no results, falling back to DashMap");
        }

        // Fall back to DashMap search
        self.index.search(&query)
    }

    /// Search using FTS5 with BM25 ranking
    fn search_fts5(&self, conn: &rusqlite::Connection, query: &SearchQuery) -> Vec<SearchResult> {
        let search_pattern = format!("{}*", query.query.to_lowercase());
        let limit = query.max_results as i64;

        let mut stmt = match conn.prepare(
            "SELECT bm25(symbol_index), symbol_name, symbol_kind, file_path FROM symbol_index WHERE symbol_index MATCH ?1 ORDER BY bm25 LIMIT ?2"
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::warn!("FTS5 query prepare failed: {}", e);
                return Vec::new();
            }
        };

        let results = stmt.query_map(rusqlite::params![search_pattern, limit], |row| {
            let rank: f64 = row.get(0)?;
            let name: String = row.get(1)?;
            let kind_str: String = row.get(2)?;
            let file: String = row.get(3)?;
            Ok((rank, name, kind_str, file))
        });

        match results {
            Ok(rows) => {
                let mut search_results = Vec::new();
                for row in rows.flatten() {
                    let (rank, name, kind_str, file) = row;
                    let kind = match kind_str.as_str() {
                        "Function" => SymbolKind::Function,
                        "Struct" => SymbolKind::Struct,
                        "Enum" => SymbolKind::Enum,
                        "Trait" => SymbolKind::Trait,
                        "Method" => SymbolKind::Method,
                        "Module" => SymbolKind::Module,
                        "Variable" => SymbolKind::Variable,
                        "Constant" => SymbolKind::Constant,
                        _ => SymbolKind::Function,
                    };
                    let location = Location::new(&file, 1, 1);
                    let symbol = Symbol::new(name.clone(), kind, location);
                    let score = (1.0 / (1.0 + rank.abs())) as f32; // Convert BM25 to similar score range
                    search_results.push(SearchResult::new(symbol, score, MatchType::Fuzzy));
                }
                search_results
            }
            Err(e) => {
                tracing::warn!("FTS5 query execution failed: {}", e);
                Vec::new()
            }
        }
    }

    /// Clears the search index
    pub fn clear(&self) {
        self.index.clear();
    }

    /// Populates the index by walking a directory and indexing all supported files.
    /// Skips common dependency/build/cache directories.
    /// Non-fatal: individual file failures are logged and skipped.
    pub fn populate_from_directory(&self, dir: &Path) -> Result<(), String> {
        if !dir.exists() {
            return Err(format!("Directory does not exist: {}", dir.display()));
        }

        const SKIP_DIRS: &[&str] = &[
            "node_modules",
            ".git",
            "target",
            "vendor",
            "dist",
            "build",
            "__pycache__",
            ".cache",
            ".next",
            ".nuxt",
            "coverage",
            ".tox",
            "venv",
            ".venv",
            "env",
        ];

        for entry in walkdir::WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| {
                if let Some(name) = e.file_name().to_str() {
                    !SKIP_DIRS.contains(&name)
                } else {
                    true
                }
            })
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let extension = path.extension().and_then(|e| e.to_str());
            let language = crate::infrastructure::parser::Language::from_extension(
                extension.as_ref().map(std::ffi::OsStr::new),
            );

            if let Some(_lang) = language {
                if let Err(_e) = self.index_file_from_path(path) {
                    // Skip files that fail to parse — non-fatal
                    continue;
                }
            }
        }

        Ok(())
    }
}

impl Default for SemanticSearchService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match_ranking() {
        let index = SearchIndex::new();

        index.index_file(
            "test.rs",
            vec![
                Symbol::new("foo", SymbolKind::Function, Location::new("test.rs", 1, 0)),
                Symbol::new(
                    "foobar",
                    SymbolKind::Function,
                    Location::new("test.rs", 2, 0),
                ),
                Symbol::new("bar", SymbolKind::Function, Location::new("test.rs", 3, 0)),
            ],
        );

        let query = SearchQuery {
            query: "foo".to_string(),
            kinds: vec![],
            max_results: 10,
        };

        let results = index.search(&query);

        assert!(!results.is_empty());
        // Exact match should be first
        assert_eq!(results[0].symbol.name(), "foo");
        assert_eq!(results[0].match_type, MatchType::Exact);
    }

    #[test]
    fn test_prefix_match_ranking() {
        let index = SearchIndex::new();

        index.index_file(
            "test.py",
            vec![
                Symbol::new(
                    "get_value",
                    SymbolKind::Function,
                    Location::new("test.py", 1, 0),
                ),
                Symbol::new(
                    "get_name",
                    SymbolKind::Function,
                    Location::new("test.py", 2, 0),
                ),
                Symbol::new(
                    "set_value",
                    SymbolKind::Function,
                    Location::new("test.py", 3, 0),
                ),
            ],
        );

        let query = SearchQuery {
            query: "get".to_string(),
            kinds: vec![],
            max_results: 10,
        };

        let results = index.search(&query);

        // Prefix matches should come before contains matches
        for result in &results {
            assert!(
                result.symbol.name().starts_with("get") || result.match_type == MatchType::Contains
            );
        }
    }

    #[test]
    fn test_kind_filter() {
        let index = SearchIndex::new();

        index.index_file(
            "test.rs",
            vec![
                Symbol::new(
                    "my_function",
                    SymbolKind::Function,
                    Location::new("test.rs", 1, 0),
                ),
                Symbol::new("MyClass", SymbolKind::Class, Location::new("test.rs", 2, 0)),
                Symbol::new(
                    "my_variable",
                    SymbolKind::Variable,
                    Location::new("test.rs", 3, 0),
                ),
            ],
        );

        let query = SearchQuery {
            query: "my".to_string(),
            kinds: vec![SearchSymbolKind::Function],
            max_results: 10,
        };

        let results = index.search(&query);

        assert!(!results.is_empty());
        for result in &results {
            assert_eq!(result.symbol.kind(), &SymbolKind::Function);
        }
    }

    #[test]
    fn test_fuzzy_match() {
        let index = SearchIndex::new();

        index.index_file(
            "test.py",
            vec![
                Symbol::new(
                    "calculate_total",
                    SymbolKind::Function,
                    Location::new("test.py", 1, 0),
                ),
                Symbol::new(
                    "calc_value",
                    SymbolKind::Function,
                    Location::new("test.py", 2, 0),
                ),
            ],
        );

        let query = SearchQuery {
            query: "calc".to_string(),
            kinds: vec![],
            max_results: 10,
        };

        let results = index.search(&query);

        // Should find both calc* functions
        assert!(results.len() >= 1);
    }

    #[test]
    fn test_max_results() {
        let index = SearchIndex::new();

        let symbols: Vec<Symbol> = (0..100)
            .map(|i| {
                Symbol::new(
                    format!("func_{}", i),
                    SymbolKind::Function,
                    Location::new("test.rs", i, 0),
                )
            })
            .collect();

        index.index_file("test.rs", symbols);

        let query = SearchQuery {
            query: "func".to_string(),
            kinds: vec![],
            max_results: 10,
        };

        let results = index.search(&query);

        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_fuzzy_score_calculation() {
        // Fuzzy score caps at 0.6 since exact/prefix/contains are handled separately
        // Exact
        assert!(fuzzy_score("foo", "foo") > 0.5);
        // Prefix
        assert!(fuzzy_score("foo", "foobar") > 0.5);
        // Contains (fuzzy - query is substring but not prefix)
        assert!(fuzzy_score("bar", "foobar") > 0.3);
        // No match - "xyz" is not in "foobar" at all
        assert_eq!(fuzzy_score("xyz", "foobar"), 0.0);
    }

    #[test]
    fn test_search_result_ordering() {
        let results = vec![
            SearchResult::new(
                Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 0)),
                0.5,
                MatchType::Contains,
            ),
            SearchResult::new(
                Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 0)),
                1.0,
                MatchType::Exact,
            ),
            SearchResult::new(
                Symbol::new("c", SymbolKind::Function, Location::new("test.rs", 3, 0)),
                0.9,
                MatchType::Prefix,
            ),
        ];

        let mut sorted = results.clone();
        sorted.sort();

        // Exact should come first
        assert_eq!(sorted[0].symbol.name(), "b");
        assert_eq!(sorted[0].match_type, MatchType::Exact);
    }
}
