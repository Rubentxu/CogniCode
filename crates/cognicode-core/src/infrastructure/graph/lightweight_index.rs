//! Lightweight symbol index for fast symbol lookups
//!
//! This module provides a fast, memory-efficient index that maps symbol names
//! to their locations without storing graph edges. It's optimized for quick
//! lookups like "find all definitions of symbol X" or "find all symbols in file Y".

use crate::domain::value_objects::{Location, SymbolKind};
use crate::infrastructure::parser::{Language, TreeSitterParser};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use walkdir::WalkDir;

/// Represents a location of a symbol in source code
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolLocation {
    /// Path to the source file
    pub file: String,
    /// Line number (0-indexed)
    pub line: u32,
    /// Column number (0-indexed)
    pub column: u32,
    /// Kind of symbol (function, class, variable, etc.)
    pub symbol_kind: SymbolKind,
}

impl SymbolLocation {
    /// Creates a new SymbolLocation
    pub fn new(file: impl Into<String>, line: u32, column: u32, symbol_kind: SymbolKind) -> Self {
        Self {
            file: file.into(),
            line,
            column,
            symbol_kind,
        }
    }

    /// Creates a SymbolLocation from a Location and SymbolKind
    pub fn from_location(location: &Location, symbol_kind: SymbolKind) -> Self {
        Self {
            file: location.file().to_string(),
            line: location.line(),
            column: location.column(),
            symbol_kind,
        }
    }

    /// Returns the file path
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the line number
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Returns the column number
    pub fn column(&self) -> u32 {
        self.column
    }

    /// Returns the symbol kind
    pub fn kind(&self) -> &SymbolKind {
        &self.symbol_kind
    }
}

/// Lightweight index mapping symbol names to their locations
///
/// This index stores only symbol definitions (not edges/relationships).
/// It's designed for fast lookups and minimal memory usage.
#[derive(Debug, Clone)]
pub struct LightweightIndex {
    /// Map from lowercase symbol name to list of locations
    index: HashMap<String, Vec<SymbolLocation>>,
    /// Map from file path to list of lowercase symbol names in that file
    file_index: HashMap<String, Vec<String>>,
}

impl LightweightIndex {
    /// Creates a new empty LightweightIndex
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            file_index: HashMap::new(),
        }
    }

    /// Builds an index by scanning all source files in a directory
    ///
    /// Walks the directory recursively, parses each supported source file,
    /// and extracts symbol definitions.
    pub fn build_index<P: AsRef<Path>>(&mut self, project_dir: P) -> std::io::Result<()> {
        let mut parser_cache: HashMap<Language, TreeSitterParser> = HashMap::new();
        let mut file_name_set: HashSet<String> = HashSet::new();

        // Directories to skip during indexing — these contain no relevant source code
        // and can contain thousands of files that slow down indexing.
        const SKIP_DIRS: &[&str] = &[
            "target",       // Rust build output
            "node_modules", // npm packages
            ".git",         // version control
            "dist",         // build output
            "build",        // general build output
            "vendor",       // dependency vendor
            "__pycache__",  // Python bytecode cache
            ".cache",       // general cache
            ".next",        // Next.js build
            ".nuxt",        // Nuxt.js build
            "coverage",     // test coverage reports
            ".tox",         // Python tox environments
            "venv",         // Python virtual env
            ".venv",        // Python virtual env
            ".env",         // Python virtual env
            "env",          // Python virtual env
            ".sandbox",     // sandbox working copies
        ];

        for entry in WalkDir::new(project_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip directories that are known to contain no source code
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip if directory name matches any SKIP_DIRS entry
                    if SKIP_DIRS.iter().any(|&skip| name == skip) {
                        continue;
                    }
                    // Skip hidden directories (except .git which is already handled)
                    if name.starts_with('.') && name != ".git" {
                        continue;
                    }
                }
                continue; // Skip directories - only process files below
            }

            let language = match Language::from_extension(path.extension()) {
                Some(lang) => lang,
                None => continue,
            };

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let file_path = path.to_string_lossy().to_string();

            self.file_index
                .entry(file_path.clone())
                .or_insert_with(Vec::new);

            let ts_parser = parser_cache.entry(language).or_insert_with(|| {
                match TreeSitterParser::new(language) {
                    Ok(p) => p,
                    Err(_) => return TreeSitterParser::new(Language::Rust).unwrap(),
                }
            });

            if let Ok(symbols) = ts_parser.find_all_symbols_with_path(&source, &file_path) {
                for symbol in symbols {
                    let name_lower = symbol.name().to_lowercase();
                    let location =
                        SymbolLocation::from_location(symbol.location(), *symbol.kind());

                    if let Some(file_names) = self.file_index.get_mut(&file_path) {
                        if file_name_set.insert(name_lower.clone()) {
                            file_names.push(name_lower.clone());
                        }
                    }

                    self.index
                        .entry(name_lower)
                        .or_insert_with(Vec::new)
                        .push(location);
                }
            }
        }

        Ok(())
    }

    /// Builds an index from a list of source content
    ///
    /// This is useful for testing or when you have source content already in memory.
    pub fn build_from_sources<'a, I>(&mut self, sources: I)
    where
        I: IntoIterator<Item = (&'a str, &'a str)>, // (file_path, source)
    {
        let mut parser_cache: HashMap<Language, TreeSitterParser> = HashMap::new();

        for (file_path, source) in sources {
            let language = match Language::from_extension(Path::new(file_path).extension()) {
                Some(lang) => lang,
                None => continue,
            };

            self.file_index
                .entry(file_path.to_string())
                .or_insert_with(Vec::new);

            let ts_parser = parser_cache.entry(language).or_insert_with(|| {
                match TreeSitterParser::new(language) {
                    Ok(p) => p,
                    Err(_) => return TreeSitterParser::new(Language::Rust).unwrap(),
                }
            });

            if let Ok(symbols) = ts_parser.find_all_symbols_with_path(source, file_path) {
                for symbol in symbols {
                    let name_lower = symbol.name().to_lowercase();
                    let location =
                        SymbolLocation::from_location(symbol.location(), *symbol.kind());

                    if let Some(file_names) = self.file_index.get_mut(file_path) {
                        if !file_names.contains(&name_lower) {
                            file_names.push(name_lower.clone());
                        }
                    }

                    self.index
                        .entry(name_lower)
                        .or_insert_with(Vec::new)
                        .push(location);
                }
            }
        }
    }

    /// Finds all locations for a symbol by name (case-insensitive).
    /// Returns all symbols if name is empty.
    pub fn find_symbol(&self, name: &str) -> Vec<SymbolLocation> {
        let name_lower = name.to_lowercase();
        if name_lower.is_empty() {
            // Return all indexed symbols
            self.index
                .values()
                .flat_map(|v| v.iter())
                .cloned()
                .collect()
        } else {
            self.index
                .get(&name_lower)
                .map(|v| v.to_vec())
                .unwrap_or_default()
        }
    }

    /// Finds all symbols defined in a specific file
    pub fn find_in_file(&self, file_path: &str) -> Vec<&SymbolLocation> {
        let mut results = Vec::new();
        if let Some(names) = self.file_index.get(file_path) {
            for name in names {
                if let Some(locations) = self.index.get(name) {
                    for loc in locations {
                        if loc.file == file_path {
                            results.push(loc);
                        }
                    }
                }
            }
        }
        results.sort_by_key(|l| l.line);
        results
    }

    /// Returns the number of unique symbols in the index
    pub fn symbol_count(&self) -> usize {
        self.index.len()
    }

    /// Returns an iterator over all (name, locations) pairs in the index.
    ///
    /// Useful for converting the lightweight index into other representations
    /// such as a `CallGraph` or for serialization.
    pub fn entries(&self) -> impl Iterator<Item = (&String, &Vec<SymbolLocation>)> {
        self.index.iter()
    }

    /// Returns the total number of symbol locations (including duplicates across files)
    pub fn location_count(&self) -> usize {
        self.index.values().map(|v| v.len()).sum()
    }

    /// Returns an iterator over all symbol names in the index
    pub fn symbols(&self) -> impl Iterator<Item = &str> {
        self.index.keys().map(|k| k.as_str())
    }

    /// Returns an iterator over all (name, locations) pairs in the index
    pub fn all_entries(&self) -> impl Iterator<Item = (&str, &[SymbolLocation])> {
        self.index.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }

    /// Clears the index
    pub fn clear(&mut self) {
        self.index.clear();
        self.file_index.clear();
    }

    /// Inserts a symbol location into the index
    pub fn insert(&mut self, name: impl Into<String>, location: SymbolLocation) {
        let name_lower = name.into().to_lowercase();
        let file_path = location.file.clone();
        self.index
            .entry(name_lower.clone())
            .or_insert_with(Vec::new)
            .push(location);
        self.file_index
            .entry(file_path)
            .or_insert_with(Vec::new)
            .push(name_lower);
    }

    /// Returns a mutable reference to the underlying index
    ///
    /// # Safety
    /// This bypasses the encapsulation. Use with care.
    #[allow(dead_code)]
    pub(crate) fn index_mut(&mut self) -> &mut HashMap<String, Vec<SymbolLocation>> {
        &mut self.index
    }
}

impl Default for LightweightIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lightweight_index_empty() {
        let index = LightweightIndex::new();
        assert!(index.find_symbol("test").is_empty());
        assert_eq!(index.symbol_count(), 0);
    }

    #[test]
    fn test_lightweight_index_build_from_sources() {
        let mut index = LightweightIndex::new();
        index.build_from_sources([(
            "test.py",
            "def hello():\n    pass\n\nclass MyClass:\n    pass\n",
        )]);

        assert_eq!(index.symbol_count(), 2);
        assert!(!index.find_symbol("hello").is_empty());
        assert!(!index.find_symbol("MyClass").is_empty());
        assert!(index.find_symbol("nonexistent").is_empty());
    }

    #[test]
    fn test_lightweight_index_case_insensitive() {
        let mut index = LightweightIndex::new();
        index.build_from_sources([("test.py", "def Hello():\n    pass\n")]);

        assert!(!index.find_symbol("hello").is_empty());
        assert!(!index.find_symbol("HELLO").is_empty());
        assert!(!index.find_symbol("Hello").is_empty());
    }

    #[test]
    fn test_lightweight_index_find_in_file() {
        let mut index = LightweightIndex::new();
        index.build_from_sources([("test.py", "def a():\n    pass\ndef b():\n    pass\n")]);

        let in_file = index.find_in_file("test.py");
        assert_eq!(in_file.len(), 2);
        assert_eq!(in_file[0].line, 0); // def a() is on line 0
    }

    #[test]
    #[ignore = "integration: scans entire project via build_index"]
    fn test_lightweight_index_real_project_benchmark() {
        use std::time::Instant;

        let mut index = LightweightIndex::new();
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let start = Instant::now();
        index.build_index(&project_root).unwrap();
        let build_time = start.elapsed();

        println!("[LIGHTWEIGHT INDEX] Built in {} ms", build_time.as_millis());
        println!(
            "[LIGHTWEIGHT INDEX] Total symbols: {}",
            index.symbol_count()
        );
        println!(
            "[LIGHTWEIGHT INDEX] Total locations: {}",
            index.location_count()
        );

        // Find a symbol
        let find_start = Instant::now();
        let results = index.find_symbol("build_project_graph");
        let find_time = find_start.elapsed();

        println!(
            "[LIGHTWEIGHT INDEX] find_symbol('build_project_graph') in {} ms: {} locations",
            find_time.as_millis(),
            results.len()
        );

        assert!(index.symbol_count() > 0, "Index should have symbols");
    }
}
