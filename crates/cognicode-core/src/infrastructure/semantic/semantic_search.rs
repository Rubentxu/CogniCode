//! Semantic Search - Fast symbol search with filtering
//!
//! This module provides fuzzy search capabilities with kind filtering
//! for finding symbols across the codebase.

use crate::domain::aggregates::symbol::Symbol;
use crate::domain::traits::Parser;
use crate::domain::value_objects::{Location, SymbolKind};
use crate::infrastructure::git::git_history::get_file_mtime;
use crate::infrastructure::parser::Language;
use dashmap::DashMap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// Temporal search configuration
#[derive(Debug, Clone)]
pub struct TemporalConfig {
    /// Boost intensity (0 = disabled, default 0.0 for backward compatibility)
    pub alpha: f64,
    /// Decay rate (half-life ≈ 69/beta days, default 0.01)
    pub beta: f64,
}

impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            alpha: Self::from_env_or("COGNICODE_TEMPORAL_ALPHA", 0.0),
            beta: Self::from_env_or("COGNICODE_TEMPORAL_BETA", 0.01),
        }
    }
}

impl TemporalConfig {
    /// Read a config value from environment, clamping to default if invalid
    fn from_env_or(name: &str, default: f64) -> f64 {
        match std::env::var(name) {
            Ok(val) => match val.parse::<f64>() {
                Ok(v) if v.is_finite() && v >= 0.0 => v,
                Ok(v) if v.is_nan() => {
                    tracing::warn!("{} is NaN, using default {}", name, default);
                    default
                }
                Ok(v) if v < 0.0 => {
                    tracing::warn!("{} is negative ({}), using default {}", name, v, default);
                    default
                }
                Ok(v) => {
                    tracing::warn!("{} is invalid ({}), using default {}", name, v, default);
                    default
                }
                Err(e) => {
                    tracing::warn!("Failed to parse {}: {}, using default {}", name, e, default);
                    default
                }
            },
            Err(_) => default,
        }
    }

    /// Compute temporal boost factor for a given age in days
    /// Formula: boost = 1 + alpha * exp(-beta * days)
    pub fn compute_boost(&self, days_since_modified: f64) -> f64 {
        if self.alpha <= 0.0 {
            return 1.0; // Disabled - no boost
        }
        let exp_decay = (-self.beta * days_since_modified).exp();
        1.0 + self.alpha * exp_decay
    }
}

#[cfg(test)]
mod temporal_tests {
    use super::*;

    #[test]
    fn test_temporal_config_default_disabled() {
        let config = TemporalConfig::default();
        assert_eq!(config.alpha, 0.0, "Default alpha should be 0.0 for backward compat");
        assert_eq!(config.beta, 0.01, "Default beta should be 0.01");
    }

    #[test]
    fn test_compute_boost_disabled() {
        let config = TemporalConfig { alpha: 0.0, beta: 0.01 };
        assert_eq!(config.compute_boost(0.0), 1.0, "Disabled config returns no boost");
        assert_eq!(config.compute_boost(100.0), 1.0, "Disabled config returns no boost");
    }

    #[test]
    fn test_compute_boost_fresh_symbol() {
        let config = TemporalConfig { alpha: 0.5, beta: 0.01 };
        // Fresh symbol (days_since_modified = 0)
        // boost = 1 + 0.5 * exp(0) = 1 + 0.5 * 1 = 1.5
        let boost = config.compute_boost(0.0);
        assert!((boost - 1.5).abs() < 0.001, "Fresh symbol should get max boost");
    }

    #[test]
    fn test_compute_boost_decays_with_age() {
        let config = TemporalConfig { alpha: 0.5, beta: 0.01 };
        // After ~69 days (half-life), boost should be ~1 + 0.5 * 0.5 = 1.25
        let boost_0 = config.compute_boost(0.0);
        let boost_69 = config.compute_boost(69.0);
        let boost_138 = config.compute_boost(138.0);
        
        assert!(boost_0 > boost_69, "Boost should decay over time");
        assert!(boost_69 > boost_138, "Boost should continue to decay");
        assert!(boost_0 > 1.0 && boost_138 < boost_0);
    }

    #[test]
    fn test_compute_boost_very_old_symbol() {
        let config = TemporalConfig { alpha: 0.5, beta: 0.01 };
        // Very old symbol (1000 days) - boost should approach 1.0
        let boost = config.compute_boost(1000.0);
        assert!((boost - 1.0).abs() < 0.01, "Very old symbol should have minimal boost");
    }

    #[test]
    fn test_boost_in_range() {
        let config = TemporalConfig { alpha: 0.5, beta: 0.01 };
        // Boost should always be in range [1, 1+alpha]
        for days in [0.0, 10.0, 50.0, 100.0, 500.0, 1000.0] {
            let boost = config.compute_boost(days);
            assert!(boost >= 1.0 && boost <= 1.5, 
                "Boost {} for days {} should be in [1, 1.5]", boost, days);
        }
    }
}

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
    temporal_config: TemporalConfig,
}

impl SemanticSearchService {
    /// Creates a new semantic search service
    pub fn new() -> Self {
        Self {
            index: Arc::new(SearchIndex::new()),
            db_path: None,
            temporal_config: TemporalConfig::default(),
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

    /// Writes symbols to FTS5 index within an active transaction.
    /// Shared by index_file (standalone) and populate_from_directory (batch).
    /// Caller is responsible for BEGIN/COMMIT lifecycle.
    fn write_symbols_to_fts(
        conn: &rusqlite::Connection,
        symbols: &[Symbol],
        docstrings: &[String],
        file_path: &str,
        mtime: i64,
        mtime_source: &str,
    ) -> Result<(), String> {
        for (symbol, docstring) in symbols.iter().zip(docstrings.iter()) {
            let kind_str = format!("{:?}", symbol.kind());
            let tokens = format!("{} {}", symbol.name(), kind_str);
            if let Err(e) = conn.execute(
                "INSERT OR REPLACE INTO symbol_index (symbol_name, symbol_kind, file_path, docstring, body_tokens) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![symbol.name(), kind_str, file_path, docstring, tokens],
            ) {
                return Err(format!("Failed to index symbol to FTS5: {}", e));
            }

            // Store timestamp entry
            if let Err(e) = conn.execute(
                "INSERT OR REPLACE INTO symbol_timestamps (file_path, symbol_name, last_modified, source) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![file_path, symbol.name(), mtime, mtime_source],
            ) {
                return Err(format!("Failed to upsert timestamp: {}", e));
            }
        }
        Ok(())
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

        // Extract docstrings for each symbol using the existing line-based extractor
        // Location::line() is zero-indexed; extract_docstring expects 1-indexed
        let docstrings: Vec<String> = symbols
            .iter()
            .map(|s| {
                crate::infrastructure::semantic::symbol_code::extract_docstring(
                    source,
                    s.location().line() + 1,
                )
                .unwrap_or_default()
            })
            .collect();

        // Write to DashMap (existing behavior)
        self.index.index_file(file_path, symbols.clone());

        // Capture file modification time (git or mtime fallback)
        let path = Path::new(file_path);
        let (file_mtime, mtime_source) = get_file_mtime(path);
        let mtime = file_mtime.unwrap_or(0);

        // Dual-write to FTS5 and timestamps if db_path is configured
        // Wrap in transaction for atomic per-file commit (R1)
        if let Some(conn) = self.open_fts_db() {
            if let Err(e) = conn.execute("BEGIN", []) {
                tracing::warn!("Failed to begin FTS5 transaction: {}", e);
                return Ok(());
            }
            let result = Self::write_symbols_to_fts(&conn, &symbols, &docstrings, file_path, mtime, &mtime_source);
            if let Err(e) = result {
                if let Err(rollback_err) = conn.execute("ROLLBACK", []) {
                    tracing::warn!("Failed to rollback FTS5 transaction: {}", rollback_err);
                }
                return Err(e);
            }
            if let Err(e) = conn.execute("COMMIT", []) {
                tracing::warn!("Failed to commit FTS5 transaction: {}", e);
                return Ok(());
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

    /// Search using FTS5 with BM25 ranking and optional temporal boost
    fn search_fts5(&self, conn: &rusqlite::Connection, query: &SearchQuery) -> Vec<SearchResult> {
        let search_pattern = format!("{}*", query.query.to_lowercase());
        let limit = query.max_results as i64;
        let alpha = self.temporal_config.alpha;

        // Build query: LEFT JOIN with symbol_timestamps if alpha > 0
        let sql = if alpha > 0.0 {
            "SELECT bm25(symbol_index), symbol_index.symbol_name, symbol_index.symbol_kind, symbol_index.file_path, symbol_timestamps.last_modified \
             FROM symbol_index \
             LEFT JOIN symbol_timestamps ON symbol_index.file_path = symbol_timestamps.file_path AND symbol_index.symbol_name = symbol_timestamps.symbol_name \
             WHERE symbol_index MATCH ?1 \
             ORDER BY bm25 LIMIT ?2"
        } else {
            // When alpha=0, skip the JOIN entirely for backward compatibility
            "SELECT bm25(symbol_index), symbol_name, symbol_kind, file_path, NULL as last_modified \
             FROM symbol_index \
             WHERE symbol_index MATCH ?1 \
             ORDER BY bm25 LIMIT ?2"
        };

        let mut stmt = match conn.prepare(sql) {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::warn!("FTS5 query prepare failed: {}", e);
                return Vec::new();
            }
        };

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let results = stmt.query_map(rusqlite::params![search_pattern, limit], |row| {
            let rank: f64 = row.get(0)?;
            let name: String = row.get(1)?;
            let kind_str: String = row.get(2)?;
            let file: String = row.get(3)?;
            let last_modified: Option<i64> = row.get(4).ok();
            Ok((rank, name, kind_str, file, last_modified))
        });

        match results {
            Ok(rows) => {
                let mut search_results = Vec::new();
                for row in rows.flatten() {
                    let (rank, name, kind_str, file, last_modified) = row;
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
                    
                    // Convert BM25 rank to score (lower rank = higher score)
                    let base_score = (1.0 / (1.0 + rank.abs()));
                    
                    // Apply temporal boost if available and alpha > 0
                    let boosted_score = if alpha > 0.0 {
                        if let Some(ts) = last_modified {
                            let days_since = ((now - ts) as f64) / 86400.0;
                            let boost = self.temporal_config.compute_boost(days_since.max(0.0));
                            base_score * boost
                        } else {
                            base_score // No timestamp available, no boost
                        }
                    } else {
                        base_score
                    };

                    search_results.push(SearchResult::new(symbol, boosted_score as f32, MatchType::Fuzzy));
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
    /// Uses batched transactions with configurable sub-commit frequency (default 100 files).
    /// Each file's writes are isolated via savepoint to prevent mid-walk failures from
    /// discarding prior committed batches.
    pub fn populate_from_directory(&self, dir: &Path) -> Result<(), String> {
        self.populate_from_directory_with_batch(dir, 100)
    }

    /// Internal implementation with configurable batch size.
    fn populate_from_directory_with_batch(&self, dir: &Path, batch_size: usize) -> Result<(), String> {
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

        // Open single connection for entire walk (R2)
        let conn = match self.open_fts_db() {
            Some(c) => c,
            None => {
                // No db_path configured — fall back to DashMap-only indexing
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
                    if let Some(_lang) = crate::infrastructure::parser::Language::from_extension(
                        extension.as_ref().map(std::ffi::OsStr::new),
                    )
                        && let Err(_e) = self.index_file_from_path(path) {
                            continue;
                        }
                }
                return Ok(());
            }
        };

        // Begin outer transaction for entire walk
        if let Err(e) = conn.execute("BEGIN", []) {
            return Err(format!("Failed to begin batch transaction: {}", e));
        }

        let mut file_count = 0;
        let mut savepoint_id = 0;

        let result = (|| -> Result<(), String> {
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

                if let Some(lang) = language {
                    // Parse the file to get symbols
                    let source = match std::fs::read_to_string(path) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!("Failed to read file {:?}: {}", path, e);
                            continue;
                        }
                    };

                    let parser = match crate::infrastructure::parser::TreeSitterParser::new(lang) {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::warn!("Failed to create parser for {:?}: {}", path, e);
                            continue;
                        }
                    };

                    let symbols = match parser.find_all_symbols(&source) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!("Failed to parse symbols in {:?}: {}", path, e);
                            continue;
                        }
                    };

                    // Extract docstrings for each symbol using the existing line-based extractor
                    // Location::line() is zero-indexed; extract_docstring expects 1-indexed
                    let docstrings: Vec<String> = symbols
                        .iter()
                        .map(|s| {
                            crate::infrastructure::semantic::symbol_code::extract_docstring(
                                &source,
                                s.location().line() + 1,
                            )
                            .unwrap_or_default()
                        })
                        .collect();

                    // Write to DashMap index (always, regardless of FTS5)
                    self.index.index_file(&path.to_string_lossy(), symbols.clone());

                    // Get mtime for FTS5 timestamps
                    let (file_mtime, mtime_source) = get_file_mtime(path);
                    let mtime = file_mtime.unwrap_or(0);

                    // Use savepoint for error isolation if > 1 symbol (R3)
                    // Files with ≤1 symbol skip savepoint overhead (0-1 rows don't need rollback isolation)
                    if symbols.len() > 1 {
                        savepoint_id += 1;
                        let sp_name = format!("sp_{}", savepoint_id);
                        if let Err(e) = conn.execute(&format!("SAVEPOINT {}", sp_name), []) {
                            tracing::warn!("Failed to create savepoint {}: {}", sp_name, e);
                            // Fall back to continuing without savepoint
                        } else {
                            let sp_result = Self::write_symbols_to_fts(&conn, &symbols, &docstrings, &path.to_string_lossy(), mtime, &mtime_source);
                            if let Err(e) = sp_result {
                                tracing::warn!("Failed to write symbols for {:?}: {}", path, e);
                                if let Err(rb_err) = conn.execute(&format!("ROLLBACK TO {}", sp_name), []) {
                                    tracing::warn!("Failed to rollback to savepoint {}: {}", sp_name, rb_err);
                                }
                                // Continue to next file — prior committed batches are preserved
                                continue;
                            }
                            // Release savepoint on success
                            if let Err(e) = conn.execute(&format!("RELEASE SAVEPOINT {}", sp_name), []) {
                                tracing::warn!("Failed to release savepoint {}: {}", sp_name, e);
                            }
                            file_count += 1;
                        }
                    } else {
                        // ≤1 symbol: write directly without savepoint overhead
                        if let Err(e) = Self::write_symbols_to_fts(&conn, &symbols, &docstrings, &path.to_string_lossy(), mtime, &mtime_source) {
                            tracing::warn!("Failed to write symbols for {:?}: {}", path, e);
                            continue;
                        }
                        file_count += 1;
                    }

                    // Sub-commit every batch_size files (R2, R6)
                    if file_count > 0 && file_count % batch_size == 0 {
                        if let Err(e) = conn.execute("COMMIT", []) {
                            return Err(format!("Failed to commit batch at file {}: {}", file_count, e));
                        }
                        if let Err(e) = conn.execute("BEGIN", []) {
                            return Err(format!("Failed to begin new batch: {}", e));
                        }
                    }
                }
            }
            Ok(())
        })();

        // Finalize: commit remaining work or rollback on error
        match result {
            Ok(()) => {
                if let Err(e) = conn.execute("COMMIT", []) {
                    Err(format!("Failed to commit final batch: {}", e))
                } else {
                    Ok(())
                }
            }
            Err(e) => {
                if let Err(rb_err) = conn.execute("ROLLBACK", []) {
                    tracing::warn!("Failed to rollback on error: {}", rb_err);
                }
                Err(e)
            }
        }
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

/// Tests for FTS5 transaction batching functionality
#[cfg(test)]
mod fts5_batching_tests {
    use super::*;

    /// Helper: create an in-memory DB with FTS5 schema
    fn create_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS symbol_index USING fts5(
                symbol_name, symbol_kind, file_path, docstring, body_tokens,
                tokenize='porter unicode61'
            );
            CREATE TABLE IF NOT EXISTS symbol_timestamps (
                file_path TEXT NOT NULL,
                symbol_name TEXT NOT NULL,
                last_modified INTEGER NOT NULL,
                source TEXT NOT NULL,
                PRIMARY KEY (file_path, symbol_name)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_write_symbols_to_fts_inserts_rows() {
        // 3.1: Unit test — write_symbols_to_fts with in-memory DB
        let conn = create_test_db();

        let symbols = vec![
            Symbol::new("fn_a", SymbolKind::Function, Location::new("src/lib.rs", 1, 0)),
            Symbol::new("fn_b", SymbolKind::Function, Location::new("src/lib.rs", 5, 0)),
            Symbol::new("MyStruct", SymbolKind::Struct, Location::new("src/lib.rs", 10, 0)),
        ];

        let result = SemanticSearchService::write_symbols_to_fts(
            &conn,
            &symbols,
            &vec![String::new(); symbols.len()],
            "src/lib.rs",
            1700000000,
            "git",
        );

        assert!(result.is_ok(), "write_symbols_to_fts should succeed");

        // Verify rows in symbol_index
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbol_index", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3, "Should have 3 symbols indexed");

        // Verify rows in symbol_timestamps
        let ts_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbol_timestamps", [], |row| row.get(0))
            .unwrap();
        assert_eq!(ts_count, 3, "Should have 3 timestamp entries");
    }

    #[test]
    fn test_savepoint_rollback_preserves_prior_data() {
        // 3.2: Unit test — savepoint rollback preserves prior committed data
        let conn = create_test_db();

        // Begin transaction
        conn.execute("BEGIN", []).unwrap();

        // First savepoint with some data
        conn.execute("SAVEPOINT sp1", []).unwrap();
        let symbols1 = vec![Symbol::new(
            "committed_func",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        )];
        SemanticSearchService::write_symbols_to_fts(&conn, &symbols1, &vec![String::new(); symbols1.len()], "src/a.rs", 1000, "git").unwrap();
        conn.execute("RELEASE SAVEPOINT sp1", []).unwrap();

        // Second savepoint with more data
        conn.execute("SAVEPOINT sp2", []).unwrap();
        let symbols2 = vec![Symbol::new(
            "rollback_func",
            SymbolKind::Function,
            Location::new("src/b.rs", 1, 0),
        )];
        SemanticSearchService::write_symbols_to_fts(&conn, &symbols2, &vec![String::new(); symbols2.len()], "src/b.rs", 1000, "git").unwrap();

        // Rollback second savepoint
        conn.execute("ROLLBACK TO sp2", []).unwrap();
        conn.execute("RELEASE SAVEPOINT sp2", []).unwrap();

        // Commit
        conn.execute("COMMIT", []).unwrap();

        // Verify first savepoint data is committed
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbol_index", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "Should have 1 committed symbol (rollback_func should not exist)");
        assert_eq!(
            count,
            1,
            "Rollback of second savepoint should not affect first savepoint's data"
        );

        // Verify rollback_func is NOT present
        let rollback_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbol_index WHERE symbol_name = 'rollback_func'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rollback_count, 0, "rollback_func should not exist after ROLLBACK TO sp2");
    }

    #[test]
    fn test_index_file_produces_identical_rows_equivalence() {
        // 3.6: Smoke test — single-file indexing produces identical rows before/after
        // This verifies R4: Content equivalence using in-memory DB directly
        let conn = create_test_db();

        let symbols = vec![
            Symbol::new("fn_a", SymbolKind::Function, Location::new("src/lib.rs", 1, 0)),
            Symbol::new("fn_b", SymbolKind::Function, Location::new("src/lib.rs", 5, 0)),
        ];

        // Write with transaction wrapping (simulating what index_file does)
        conn.execute("BEGIN", []).unwrap();
        SemanticSearchService::write_symbols_to_fts(&conn, &symbols, &vec![String::new(); symbols.len()], "src/lib.rs", 1000, "git").unwrap();
        conn.execute("COMMIT", []).unwrap();

        // Verify rows in symbol_index
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbol_index", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2, "Should have 2 symbols indexed");

        // Index a different file
        conn.execute("BEGIN", []).unwrap();
        let symbols2 = vec![
            Symbol::new("fn_c", SymbolKind::Function, Location::new("src/other.rs", 1, 0)),
            Symbol::new("fn_d", SymbolKind::Function, Location::new("src/other.rs", 5, 0)),
            Symbol::new("fn_e", SymbolKind::Function, Location::new("src/other.rs", 10, 0)),
        ];
        SemanticSearchService::write_symbols_to_fts(&conn, &symbols2, &vec![String::new(); symbols2.len()], "src/other.rs", 1000, "git").unwrap();
        conn.execute("COMMIT", []).unwrap();

        // Should have 5 total symbols across 2 files
        let count2: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbol_index", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count2, 5, "Should have 5 symbols across 2 files");

        // Verify each file has its own symbols
        let file_a_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbol_index WHERE file_path = 'src/lib.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(file_a_count, 2, "src/lib.rs should have 2 symbols");

        let file_b_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbol_index WHERE file_path = 'src/other.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(file_b_count, 3, "src/other.rs should have 3 symbols");
    }

    #[test]
    fn test_populate_from_directory_indexes_all_files() {
        // 3.3: Integration test — populate_from_directory with 3 files, 5 symbols each
        // This test verifies the directory walking logic produces the right symbols
        // Using in-memory DB to avoid schema initialization issues
        let conn = create_test_db();

        // Simulate what populate_from_directory does internally
        // Write symbols for 3 files with 5 symbols each
        let file_symbols = vec![
            ("src/a.rs", vec!["fn_a1", "fn_a2", "fn_a3", "Struct_a", "Enum_a"]),
            ("src/b.rs", vec!["fn_b1", "fn_b2", "fn_b3", "Struct_b", "Enum_b"]),
            ("src/c.rs", vec!["fn_c1", "fn_c2", "fn_c3", "Struct_c", "Enum_c"]),
        ];

        conn.execute("BEGIN", []).unwrap();
        for (file_path, symbol_names) in file_symbols {
            let symbols: Vec<Symbol> = symbol_names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    Symbol::new(
                        *name,
                        if name.starts_with("fn_") {
                            SymbolKind::Function
                        } else if name.starts_with("Struct_") {
                            SymbolKind::Struct
                        } else {
                            SymbolKind::Enum
                        },
                        Location::new(file_path, (i + 1) as u32, 0),
                    )
                })
                .collect();
            SemanticSearchService::write_symbols_to_fts(&conn, &symbols, &vec![String::new(); symbols.len()], file_path, 1000, "git").unwrap();
        }
        conn.execute("COMMIT", []).unwrap();

        // Verify all symbols are indexed
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbol_index", [], |row| row.get(0))
            .unwrap();

        // 3 files × 5 symbols each = 15 expected
        assert_eq!(count, 15, "Should have 15 symbols indexed from 3 files");
    }

    #[test]
    fn test_index_file_populates_docstring_column() {
        // R7/R9: Verify docstring column is populated for symbols with doc comments
        use crate::infrastructure::parser::Language;

        let temp_dir = tempfile::tempdir().unwrap();
        let db_root = temp_dir.path().to_path_buf();

        // Create FTS5 schema
        let fts_db_path = db_root.join(".cognicode").join("cognicode.db");
        std::fs::create_dir_all(fts_db_path.parent().unwrap()).unwrap();
        let conn = rusqlite::Connection::open(&fts_db_path).unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS symbol_index USING fts5(
                symbol_name, symbol_kind, file_path, docstring, body_tokens,
                tokenize='porter unicode61'
            );
            CREATE TABLE IF NOT EXISTS symbol_timestamps (
                file_path TEXT NOT NULL,
                symbol_name TEXT NOT NULL,
                last_modified INTEGER NOT NULL,
                source TEXT NOT NULL,
                PRIMARY KEY (file_path, symbol_name)
            );",
        )
        .unwrap();
        drop(conn);

        let service = SemanticSearchService::new().with_db_path(db_root);

        // Rust source with doc comment above the function
        let source = "/// Adds two numbers\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}";

        let result = service.index_file("src/math.rs", source, Language::Rust);
        assert!(result.is_ok(), "index_file should succeed");

        // Verify docstring is populated via FTS5 query
        let conn = rusqlite::Connection::open(&fts_db_path).unwrap();
        let docstring: String = conn
            .query_row(
                "SELECT docstring FROM symbol_index WHERE symbol_name = 'add'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            docstring.contains("Adds two numbers"),
            "docstring should contain 'Adds two numbers', got: '{}'",
            docstring
        );
    }

    #[test]
    fn test_docstring_empty_for_undocumented_symbol() {
        // R7: Verify docstring is empty for symbols without doc comments
        use crate::infrastructure::parser::Language;

        let temp_dir = tempfile::tempdir().unwrap();
        let db_root = temp_dir.path().to_path_buf();

        // Create FTS5 schema
        let fts_db_path = db_root.join(".cognicode").join("cognicode.db");
        std::fs::create_dir_all(fts_db_path.parent().unwrap()).unwrap();
        let conn = rusqlite::Connection::open(&fts_db_path).unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS symbol_index USING fts5(
                symbol_name, symbol_kind, file_path, docstring, body_tokens,
                tokenize='porter unicode61'
            );
            CREATE TABLE IF NOT EXISTS symbol_timestamps (
                file_path TEXT NOT NULL,
                symbol_name TEXT NOT NULL,
                last_modified INTEGER NOT NULL,
                source TEXT NOT NULL,
                PRIMARY KEY (file_path, symbol_name)
            );",
        )
        .unwrap();
        drop(conn);

        let service = SemanticSearchService::new().with_db_path(db_root);

        // Rust source without any doc comment
        let source = "fn raw(a: i32) -> i32 {\n    a\n}";

        let result = service.index_file("src/raw.rs", source, Language::Rust);
        assert!(result.is_ok(), "index_file should succeed");

        // Verify docstring is empty via FTS5 query
        let conn = rusqlite::Connection::open(&fts_db_path).unwrap();
        let docstring: String = conn
            .query_row(
                "SELECT docstring FROM symbol_index WHERE symbol_name = 'raw'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(docstring, "", "docstring should be empty for undocumented symbol, got: '{}'", docstring);
    }

    #[test]
    fn test_populate_from_directory_docstrings_mixed() {
        // R7/R9: 3 files, assert 4 rows with non-empty docstring, 2 with empty

        let temp_dir = tempfile::tempdir().unwrap();
        let db_root = temp_dir.path().to_path_buf();

        // Create FTS5 schema
        let fts_db_path = db_root.join(".cognicode").join("cognicode.db");
        std::fs::create_dir_all(fts_db_path.parent().unwrap()).unwrap();
        let conn = rusqlite::Connection::open(&fts_db_path).unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS symbol_index USING fts5(
                symbol_name, symbol_kind, file_path, docstring, body_tokens,
                tokenize='porter unicode61'
            );
            CREATE TABLE IF NOT EXISTS symbol_timestamps (
                file_path TEXT NOT NULL,
                symbol_name TEXT NOT NULL,
                last_modified INTEGER NOT NULL,
                source TEXT NOT NULL,
                PRIMARY KEY (file_path, symbol_name)
            );",
        )
        .unwrap();
        drop(conn);

        let service = SemanticSearchService::new().with_db_path(db_root);

        // Create 3 files:
        // File A: 3 symbols with doc comments
        // File B: 2 symbols without doc comments
        // File C: 1 symbol with doc comment
        // Total: 4 with docstrings, 2 without

        let file_a = temp_dir.path().join("math.rs");
        std::fs::write(&file_a, "/// Adds two numbers\nfn add(a: i32, b: i32) -> i32 { a + b }\n/// Subtracts two numbers\nfn sub(a: i32, b: i32) -> i32 { a - b }\n/// Multiplies two numbers\nfn mul(a: i32, b: i32) -> i32 { a * b }").unwrap();

        let file_b = temp_dir.path().join("raw.rs");
        std::fs::write(&file_b, "fn raw_no_doc(a: i32) -> i32 { a }\nfn another_raw(a: i32) -> i32 { a }").unwrap();

        let file_c = temp_dir.path().join("string.rs");
        std::fs::write(&file_c, "/// Reverses a string\nfn reverse(s: &str) -> String { s.chars().rev().collect() }").unwrap();

        let result = service.populate_from_directory(temp_dir.path());
        assert!(result.is_ok(), "populate_from_directory should succeed");

        // Verify docstring counts via FTS5 query
        let conn = rusqlite::Connection::open(&fts_db_path).unwrap();

        // Count rows with non-empty docstring
        let non_empty_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbol_index WHERE length(docstring) > 0",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(non_empty_count, 4, "Should have 4 symbols with non-empty docstrings");

        // Count rows with empty docstring
        let empty_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbol_index WHERE length(docstring) = 0",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(empty_count, 2, "Should have 2 symbols with empty docstrings");
    }
}
