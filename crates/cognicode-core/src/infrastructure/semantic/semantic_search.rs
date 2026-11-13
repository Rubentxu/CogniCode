//! Semantic Search - Fast symbol search with filtering
//!
//! This module provides fuzzy search capabilities with kind filtering
//! for finding symbols across the codebase.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::len_zero)]

use crate::application::ingest::types::SemanticOutcome;
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::evidence_kernel::SymbolFqn;
use crate::domain::value_objects::{Location, SymbolKind, SymbolSearchQuery};
use crate::infrastructure::graph::per_file_graph::{SkipReason, SkippedFile};
use crate::infrastructure::graph::semantic_layer::semantic_facts_of;
use crate::infrastructure::graph::source_files;
use crate::infrastructure::parser::Language;
use crate::infrastructure::parser::language_config::LanguageConfig;
use dashmap::DashMap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::Path;
use std::sync::Arc;

/// How the error of a file that could not be read begins.
///
/// One constant, because the code that writes the error and the code that
/// tells a read failure from a parse failure have to agree on it.
const READ_FAILURE: &str = "Failed to read file";

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
    /// The entries each file contributed, by the file's name: what has to go
    /// when the file is indexed again or forgotten.
    entries_by_file: DashMap<String, Vec<String>>,
    /// All symbols indexed, by the identity of the symbol.
    all_symbols: DashMap<String, IndexedSymbol>,
}

#[derive(Debug, Clone)]
struct IndexedSymbol {
    name: String,
    name_lower: String,
    kind: SymbolKind,
    location: Location,
}

/// The identity an entry has in the index: `{file}:{name}:{line}`.
///
/// The grammar belongs to [`SymbolFqn`], the same one the graph writes for
/// its symbols, so the index and the graph cannot disagree about which
/// symbols are the same one. The file is the one the caller indexed the entry
/// under, because that is the name the entry is forgotten by: a reader that
/// stamped its symbols with a placeholder place must not make two files
/// collide.
fn entry_identity(file_path: &str, symbol: &Symbol) -> String {
    SymbolFqn::from_legacy_side(file_path, symbol.name(), symbol.location().line()).assemble()
}

impl SearchIndex {
    /// Creates a new search index
    pub fn new() -> Self {
        Self {
            entries_by_file: DashMap::new(),
            all_symbols: DashMap::new(),
        }
    }

    /// Indexes a file's symbols, replacing whatever the index held for it.
    ///
    /// A file that is indexed again declares what it declares now: a symbol
    /// it no longer declares (renamed, moved, deleted) must stop being
    /// searchable, so the previous entries go first.
    pub fn index_file(&self, file_path: &str, symbols: Vec<Symbol>) {
        self.clear_file(file_path);

        let identities: Vec<String> = symbols
            .into_iter()
            .map(|s| {
                let identity = entry_identity(file_path, &s);
                self.all_symbols.insert(
                    identity.clone(),
                    IndexedSymbol {
                        name: s.name().to_string(),
                        name_lower: s.name().to_lowercase(),
                        kind: *s.kind(),
                        location: s.location().clone(),
                    },
                );
                identity
            })
            .collect();

        self.entries_by_file
            .insert(file_path.to_string(), identities);
    }

    /// Forgets every entry indexed under `file_path`.
    pub fn clear_file(&self, file_path: &str) {
        if let Some((_, identities)) = self.entries_by_file.remove(file_path) {
            for identity in identities {
                self.all_symbols.remove(&identity);
            }
        }
    }

    /// Clears the entire index
    pub fn clear(&self) {
        self.entries_by_file.clear();
        self.all_symbols.clear();
    }

    /// Searches for symbols matching the query
    pub fn search(&self, query: &SymbolSearchQuery) -> Vec<SearchResult> {
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
                let kind_matches = query.kinds.contains(&indexed.kind);
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
                let symbol =
                    Symbol::new(indexed.name.clone(), indexed.kind, indexed.location.clone());
                heap.push(Reverse(SearchResult::new(symbol, score, match_type)));
            } else if let Some(min) = heap.peek() {
                let temp_result = SearchResult::new(
                    Symbol::new(indexed.name.clone(), indexed.kind, indexed.location.clone()),
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
}

/// How long the reading, the interpreting and the writing of one file
/// took.
///
/// The reading and the interpreting are one phase —the file has to be
/// read before it can be understood— and the writing into the index is
/// another.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexTimings {
    /// Reading the file and interpreting it.
    pub parse_ms: u64,
    /// Writing the symbols into the index.
    pub index_ms: u64,
}

impl SemanticSearchService {
    /// Creates a new semantic search service
    pub fn new() -> Self {
        Self {
            index: Arc::new(SearchIndex::new()),
        }
    }

    /// Returns a reference to the search index
    pub fn index(&self) -> &SearchIndex {
        &self.index
    }

    /// Indexes a file's symbols into the DashMap index
    ///
    /// A language with a reader of its own is read by that reader, the one
    /// the graph is built from and `reparse_on_edit` hands to the index
    /// ([`index_symbols`](Self::index_symbols)): a Terraform block or an
    /// Ansible task is a symbol under the name the tools that ask about
    /// them use. Every other file — and a file whose own reader fell over,
    /// because the index is more tolerant than the graph — is read the
    /// generic way, so that the same file does not declare one set of
    /// symbols after an edit and another after a reindex.
    pub fn index_file(
        &self,
        file_path: &str,
        source: &str,
        language: Language,
    ) -> Result<(), String> {
        let own_reader =
            semantic_facts_of(LanguageConfig::for_language(language), source, file_path);
        if let SemanticOutcome::Handled(facts) = own_reader {
            self.index_symbols(file_path, facts.symbols);
            return Ok(());
        }

        let parser = crate::infrastructure::parser::TreeSitterParser::new(language)
            .map_err(|e| e.to_string())?;

        // The symbols carry the path they were read from: a result that
        // names the file "source" cannot be opened by whoever asked.
        let symbols = parser
            .find_all_symbols_with_path(source, file_path)
            .map_err(|e| e.to_string())?;

        self.index_symbols(file_path, symbols);

        Ok(())
    }

    /// Indexes symbols that have already been read.
    ///
    /// A caller that interpreted the file for another purpose hands the
    /// result here instead of paying for a second reading of the same
    /// bytes. Writes to the DashMap (single source of truth after the Graph
    /// Intelligence v2 cleanup removed the FTS5 dual-write path).
    pub fn index_symbols(&self, file_path: &str, symbols: Vec<Symbol>) {
        self.index.index_file(file_path, symbols);
    }

    /// Forgets every symbol indexed for `file_path`.
    ///
    /// `file_path` is the name the file was indexed under: the same
    /// `to_string_lossy` form [`index_file_from_path`](Self::index_file_from_path)
    /// and the incremental routes use.
    pub fn forget_file(&self, file_path: &Path) {
        self.index.clear_file(&file_path.to_string_lossy());
    }

    /// Indexes a single file from path
    pub fn index_file_from_path(&self, file_path: &Path) -> Result<(), String> {
        self.index_file_from_path_timed(file_path).map(|_| ())
    }

    /// Indexes a single file from path, saying how long each part took.
    ///
    /// The reading and the interpreting are one phase, and the writing
    /// into the index is another: they are told apart because they do
    /// not cost the same, and a consumer that wants to know what an
    /// operation paid for needs to know which one it paid for.
    pub fn index_file_from_path_timed(&self, file_path: &Path) -> Result<IndexTimings, String> {
        let read_start = std::time::Instant::now();
        let extension = file_path.extension().and_then(|e| e.to_str());

        let language = Language::from_extension(extension.as_ref().map(std::ffi::OsStr::new))
            .ok_or_else(|| "Unsupported file type".to_string())?;

        let source =
            std::fs::read_to_string(file_path).map_err(|e| format!("{READ_FAILURE}: {e}"))?;

        let parse_ms = read_start.elapsed().as_millis() as u64;

        let write_start = std::time::Instant::now();
        self.index_file(&file_path.to_string_lossy(), &source, language)?;
        let index_ms = write_start.elapsed().as_millis() as u64;

        Ok(IndexTimings { parse_ms, index_ms })
    }

    /// Searches for symbols via the DashMap index.
    ///
    /// The FTS5 + temporal-boost path was removed in the Graph
    /// Intelligence v2 cleanup; this implementation relies entirely
    /// on the in-memory `SearchIndex`.
    pub fn search(&self, query: SymbolSearchQuery) -> Vec<SearchResult> {
        self.index.search(&query)
    }

    /// Clears the search index
    pub fn clear(&self) {
        self.index.clear();
    }

    /// Populates the index with the files of `dir` and reports what it
    /// could not cover.
    ///
    /// The set of files is the inventory's — the one the graph is built
    /// from — so the search index and the graph describe the same tree.
    /// Non-fatal: a file that cannot be read or indexed does not stop the
    /// others, and it is not forgotten either: it comes back as a hole.
    ///
    /// An empty list is the claim that nothing was left out.
    pub fn populate_from_directory_with_holes(
        &self,
        dir: &Path,
    ) -> Result<Vec<SkippedFile>, String> {
        if !dir.exists() {
            return Err(format!("Directory does not exist: {}", dir.display()));
        }

        let inventory = source_files::inventory(dir);
        let mut holes = inventory.coverage_holes();

        for file in inventory.source_files() {
            if let Err(err) = self.index_file_from_path(&file.path) {
                holes.push(SkippedFile {
                    path: file.path.to_string_lossy().into_owned(),
                    reason: if err.starts_with(READ_FAILURE) {
                        SkipReason::Read(err)
                    } else {
                        SkipReason::Parse(err)
                    },
                });
            }
        }

        Ok(holes)
    }

    /// Populates the index by walking a directory and indexing all supported files.
    ///
    /// The same walk as [`populate_from_directory_with_holes`](Self::populate_from_directory_with_holes),
    /// for the caller that does not publish the holes.
    pub fn populate_from_directory(&self, dir: &Path) -> Result<(), String> {
        self.populate_from_directory_with_holes(dir).map(|_| ())
    }
}

impl Default for SemanticSearchService {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::application::ports::SymbolSearch for SemanticSearchService {
    fn index_workspace(&self, root: &Path) -> crate::application::AppResult<()> {
        SemanticSearchService::populate_from_directory(self, root)
            .map_err(|e| crate::application::ports::parser_failure("indexing the workspace", e))
    }

    fn index_file(&self, path: &Path) -> crate::application::AppResult<()> {
        SemanticSearchService::index_file_from_path(self, path)
            .map_err(|e| crate::application::ports::parser_failure("indexing a file", e))
    }

    fn forget_file(&self, path: &Path) {
        SemanticSearchService::forget_file(self, path);
    }

    fn search(&self, query: &SymbolSearchQuery) -> Vec<crate::domain::aggregates::symbol::Symbol> {
        SemanticSearchService::search(self, query.clone())
            .into_iter()
            .map(|r| r.symbol)
            .collect()
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

        let query = SymbolSearchQuery {
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

        let query = SymbolSearchQuery {
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

        let query = SymbolSearchQuery {
            query: "my".to_string(),
            kinds: vec![SymbolKind::Function],
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

        let query = SymbolSearchQuery {
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

        let query = SymbolSearchQuery {
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

    /// The names the index answers for `query`, as `(name, file)` pairs.
    fn found(index: &SearchIndex, query: &str) -> Vec<(String, String)> {
        let mut hits: Vec<(String, String)> = index
            .search(&SymbolSearchQuery {
                query: query.to_string(),
                kinds: vec![],
                max_results: 50,
            })
            .into_iter()
            .map(|hit| {
                (
                    hit.symbol.name().to_string(),
                    hit.symbol.location().file().to_string(),
                )
            })
            .collect();
        hits.sort();
        hits
    }

    fn function(name: &str, file: &str, line: u32, column: u32) -> Symbol {
        Symbol::new(
            name,
            SymbolKind::Function,
            Location::new(file, line, column),
        )
    }

    /// Indexing a file again replaces what the index knew of it. A symbol the
    /// file no longer declares must not stay searchable: the index would keep
    /// answering for code that is not there.
    #[test]
    fn a_renamed_symbol_is_not_searchable_after_the_file_is_indexed_again() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("old_name", "src/a.rs", 3, 0)]);
        assert_eq!(
            found(&index, "old_name").len(),
            1,
            "the premise: it was indexed"
        );

        // The same declaration, renamed: the position does not change, the name does.
        index.index_file("src/a.rs", vec![function("new_name", "src/a.rs", 3, 0)]);

        assert!(
            found(&index, "old_name").is_empty(),
            "the file no longer declares `old_name`, and the index still finds it: {:?}",
            found(&index, "old_name")
        );
        assert_eq!(found(&index, "new_name").len(), 1);
        assert_eq!(
            index.len(),
            1,
            "one symbol in the file, one entry in the index"
        );
    }

    /// A symbol that moved to another line is the same case with the position
    /// changed instead of the name.
    #[test]
    fn a_symbol_that_moved_is_found_once_after_the_file_is_indexed_again() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("helper", "src/a.rs", 3, 0)]);
        index.index_file("src/a.rs", vec![function("helper", "src/a.rs", 9, 0)]);

        assert_eq!(
            found(&index, "helper").len(),
            1,
            "`helper` is declared once and is listed {} times: {:?}",
            found(&index, "helper").len(),
            found(&index, "helper")
        );
        assert_eq!(index.len(), 1);
    }

    /// Two files that declare a symbol with the same name at the same line and
    /// column are two symbols. The identity of a symbol includes its file.
    #[test]
    fn two_files_with_a_symbol_at_the_same_line_and_column_stay_two_results() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("new", "src/a.rs", 5, 4)]);
        index.index_file("src/b.rs", vec![function("new", "src/b.rs", 5, 4)]);

        assert_eq!(
            found(&index, "new"),
            vec![
                ("new".to_string(), "src/a.rs".to_string()),
                ("new".to_string(), "src/b.rs".to_string()),
            ],
            "the second file overwrote the first one in the index"
        );
        assert_eq!(index.len(), 2);
    }

    /// Forgetting one file leaves the other one alone, even when their symbols
    /// share a name, a line and a column.
    #[test]
    fn clearing_a_file_does_not_forget_the_symbols_of_another_file() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("new", "src/a.rs", 5, 4)]);
        index.index_file("src/b.rs", vec![function("new", "src/b.rs", 5, 4)]);

        index.clear_file("src/a.rs");

        assert_eq!(
            found(&index, "new"),
            vec![("new".to_string(), "src/b.rs".to_string())],
            "clearing `src/a.rs` took the symbol of `src/b.rs` with it"
        );
    }

    /// The file a caller names when it indexes is the file the entries belong
    /// to, even if a reader stamped its symbols with another place: the entry
    /// can be forgotten by the name it was indexed under.
    #[test]
    fn a_file_is_forgotten_by_the_name_it_was_indexed_under() {
        let index = SearchIndex::new();
        index.index_file("src/a.rs", vec![function("only_here", "source", 2, 0)]);
        index.index_file("src/b.rs", vec![function("only_here", "source", 2, 0)]);
        assert_eq!(index.len(), 2, "two files, two entries");

        index.clear_file("src/a.rs");

        assert_eq!(index.len(), 1);
        index.clear_file("src/b.rs");
        assert!(index.is_empty());
    }

    /// The service forgets a file as a whole, and says so through the port the
    /// application depends on.
    #[test]
    fn the_service_forgets_a_deleted_file() {
        use crate::application::ports::SymbolSearch;

        let service = SemanticSearchService::new();
        service.index_symbols(
            "src/gone.rs",
            vec![function("vanished", "src/gone.rs", 1, 0)],
        );
        service.index_symbols("src/kept.rs", vec![function("stays", "src/kept.rs", 1, 0)]);

        SymbolSearch::forget_file(&service, Path::new("src/gone.rs"));

        assert!(found(service.index(), "vanished").is_empty());
        assert_eq!(found(service.index(), "stays").len(), 1);
    }
}
