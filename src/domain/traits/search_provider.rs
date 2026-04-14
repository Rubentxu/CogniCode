//! Trait for search operations
//!
//! Provides methods for searching and replacing code patterns.

use crate::domain::value_objects::{Location, SourceRange};
use async_trait::async_trait;

/// Provider for search operations
#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Searches for a pattern in the given scope
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchMatch>, SearchError>;

    /// Replaces matches with a replacement pattern
    async fn replace(
        &self,
        matches: &[SearchMatch],
        replacement: &str,
    ) -> Result<Vec<Replacement>, SearchError>;

    /// Searches for similar code patterns
    async fn find_similar(&self, location: &Location) -> Result<Vec<SimilarMatch>, SearchError>;

    /// Validates a search query
    fn validate_query(&self, query: &SearchQuery) -> QueryValidation;
}

/// Query for searching code
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// The pattern to search for
    pub pattern: String,
    /// Scope of the search
    pub scope: SearchScope,
    /// Search options
    pub options: SearchOptions,
}

impl SearchQuery {
    /// Creates a new search query with default options
    pub fn new(pattern: impl Into<String>, scope: SearchScope) -> Self {
        Self {
            pattern: pattern.into(),
            scope,
            options: SearchOptions::default(),
        }
    }

    /// Creates a new search query with custom options
    pub fn with_options(
        pattern: impl Into<String>,
        scope: SearchScope,
        options: SearchOptions,
    ) -> Self {
        Self {
            pattern: pattern.into(),
            scope,
            options,
        }
    }
}

/// Scope of a search operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchScope {
    /// Search in a specific file
    File(std::path::PathBuf),
    /// Search in a directory
    Directory {
        path: std::path::PathBuf,
        recursive: bool,
    },
    /// Search in open files
    OpenFiles,
    /// Search in workspace
    Workspace,
    /// Search in a specific range
    Range {
        file: std::path::PathBuf,
        range: SourceRange,
    },
}

/// Options for search operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOptions {
    /// Case sensitive matching
    pub case_sensitive: bool,
    /// Match whole words only
    pub whole_word: bool,
    /// Use regex pattern
    pub is_regex: bool,
    /// Include comments in results
    pub include_comments: bool,
    /// Include strings in results
    pub include_strings: bool,
    /// Maximum number of results
    pub max_results: usize,
    /// File filters (glob patterns)
    pub file_filters: Vec<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: true,
            whole_word: false,
            is_regex: false,
            include_comments: true,
            include_strings: true,
            max_results: 1000,
            file_filters: Vec::new(),
        }
    }
}

impl SearchOptions {
    /// Creates options for literal search
    pub fn literal() -> Self {
        Self {
            is_regex: false,
            ..Default::default()
        }
    }

    /// Creates options for regex search
    pub fn regex() -> Self {
        Self {
            is_regex: true,
            ..Default::default()
        }
    }
}

/// A match from a search operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// The file containing the match
    pub file: std::path::PathBuf,
    /// The range of the match
    pub range: SourceRange,
    /// The matched text
    pub matched_text: String,
    /// Line number (1-indexed)
    pub line_number: u32,
    /// Column positions (start, end)
    pub column_range: (u32, u32),
    /// Optional capture groups for regex matches
    pub captures: Vec<CapturedGroup>,
}

impl SearchMatch {
    /// Returns the location of the match
    pub fn location(&self) -> Location {
        Location::new(self.file.to_string_lossy().as_ref(), self.line_number, self.column_range.0 + 1)
    }
}

/// A captured group from a regex match
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedGroup {
    /// Group index (0 = full match)
    pub index: usize,
    /// Group name (if named)
    pub name: Option<String>,
    /// The captured text
    pub text: String,
    /// Start column
    pub start: u32,
    /// End column
    pub end: u32,
}

/// A replacement operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// The file to modify
    pub file: std::path::PathBuf,
    /// The range to replace
    pub range: SourceRange,
    /// The replacement text
    pub new_text: String,
}

impl Replacement {
    /// Creates a new replacement
    pub fn new(
        file: impl Into<std::path::PathBuf>,
        range: SourceRange,
        new_text: impl Into<String>,
    ) -> Self {
        Self {
            file: file.into(),
            range,
            new_text: new_text.into(),
        }
    }
}

/// A match for similar code
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarMatch {
    /// The file containing the similar code
    pub file: std::path::PathBuf,
    /// The range of the similar code
    pub range: SourceRange,
    /// Similarity score (0.0 to 1.0)
    pub similarity: f32,
    /// The similar code text
    pub code: String,
}

/// Validation result for a search query
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryValidation {
    /// Whether the query is valid
    pub is_valid: bool,
    /// Error message if invalid
    pub error: Option<String>,
    /// Warnings if valid
    pub warnings: Vec<String>,
}

impl QueryValidation {
    /// Creates a valid validation
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            error: None,
            warnings: Vec::new(),
        }
    }

    /// Creates an invalid validation
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            error: Some(message.into()),
            warnings: Vec::new(),
        }
    }
}

/// Error type for search operations
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),

    #[error("Invalid scope: {0}")]
    InvalidScope(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Regex error: {0}")]
    RegexError(String),

    #[error("Search failed: {0}")]
    SearchFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSearchProvider;

    impl MockSearchProvider {
        fn new() -> Self {
            MockSearchProvider
        }
    }

    impl SearchQuery {
        fn mock() -> Self {
            SearchQuery::new("test_pattern", SearchScope::Workspace)
        }
    }

    impl SearchMatch {
        fn mock() -> Self {
            let start = Location::new("test.rs", 0, 10);
            let end = Location::new("test.rs", 0, 20);
            SearchMatch {
                file: std::path::PathBuf::from("test.rs"),
                range: crate::domain::value_objects::SourceRange::new(start, end),
                matched_text: "test_pattern".to_string(),
                line_number: 1,
                column_range: (0, 11),
                captures: Vec::new(),
            }
        }
    }

    impl QueryValidation {
        fn mock_valid() -> Self {
            QueryValidation::valid()
        }

        fn mock_invalid() -> Self {
            QueryValidation::invalid("Invalid pattern")
        }
    }

    impl Replacement {
        fn mock() -> Self {
            let start = Location::new("test.rs", 0, 10);
            let end = Location::new("test.rs", 0, 20);
            Replacement::new(
                "test.rs",
                crate::domain::value_objects::SourceRange::new(start, end),
                "replacement_text",
            )
        }
    }

    impl SimilarMatch {
        fn mock() -> Self {
            let start = Location::new("similar.rs", 5, 10);
            let end = Location::new("similar.rs", 5, 25);
            SimilarMatch {
                file: std::path::PathBuf::from("similar.rs"),
                range: crate::domain::value_objects::SourceRange::new(start, end),
                similarity: 0.85,
                code: "similar code".to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl SearchProvider for MockSearchProvider {
        async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchMatch>, SearchError> {
            let _ = query;
            Ok(vec![SearchMatch::mock()])
        }

        async fn replace(
            &self,
            matches: &[SearchMatch],
            replacement: &str,
        ) -> Result<Vec<Replacement>, SearchError> {
            let _ = matches;
            let _ = replacement;
            Ok(vec![Replacement::mock()])
        }

        async fn find_similar(&self, location: &Location) -> Result<Vec<SimilarMatch>, SearchError> {
            let _ = location;
            Ok(vec![SimilarMatch::mock()])
        }

        fn validate_query(&self, query: &SearchQuery) -> QueryValidation {
            let _ = query;
            QueryValidation::mock_valid()
        }
    }

    #[tokio::test]
    async fn test_mock_search() {
        let provider = MockSearchProvider::new();
        let query = SearchQuery::mock();

        let result = provider.search(&query).await;
        assert!(result.is_ok());
        let matches = result.unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].matched_text, "test_pattern");
    }

    #[tokio::test]
    async fn test_mock_replace() {
        let provider = MockSearchProvider::new();
        let matches = vec![SearchMatch::mock()];

        let result = provider.replace(&matches, "new_text").await;
        assert!(result.is_ok());
        let replacements = result.unwrap();
        assert_eq!(replacements.len(), 1);
        assert_eq!(replacements[0].new_text, "replacement_text");
    }

    #[tokio::test]
    async fn test_mock_find_similar() {
        let provider = MockSearchProvider::new();
        let location = Location::new("test.rs", 5, 10);

        let result = provider.find_similar(&location).await;
        assert!(result.is_ok());
        let similar_matches = result.unwrap();
        assert_eq!(similar_matches.len(), 1);
        assert_eq!(similar_matches[0].similarity, 0.85);
    }

    #[test]
    fn test_mock_validate_query_valid() {
        let provider = MockSearchProvider::new();
        let query = SearchQuery::mock();

        let validation = provider.validate_query(&query);
        assert!(validation.is_valid);
        assert!(validation.error.is_none());
    }

    #[test]
    fn test_query_validation_valid() {
        let validation = QueryValidation::valid();
        assert!(validation.is_valid);
        assert!(validation.error.is_none());
        assert!(validation.warnings.is_empty());
    }

    #[test]
    fn test_query_validation_invalid() {
        let validation = QueryValidation::invalid("Test error");
        assert!(!validation.is_valid);
        assert_eq!(validation.error, Some("Test error".to_string()));
    }

    #[test]
    fn test_search_query_new() {
        let query = SearchQuery::new("pattern", SearchScope::Workspace);
        assert_eq!(query.pattern, "pattern");
        assert_eq!(query.scope, SearchScope::Workspace);
        assert!(query.options.case_sensitive);
    }

    #[test]
    fn test_search_query_with_options() {
        let options = SearchOptions {
            case_sensitive: false,
            whole_word: true,
            is_regex: true,
            include_comments: false,
            include_strings: false,
            max_results: 100,
            file_filters: vec!["*.rs".to_string()],
        };
        let query = SearchQuery::with_options("pattern", SearchScope::Workspace, options);
        assert!(!query.options.case_sensitive);
        assert!(query.options.whole_word);
        assert!(query.options.is_regex);
    }

    #[test]
    fn test_search_options_default() {
        let options = SearchOptions::default();
        assert!(options.case_sensitive);
        assert!(!options.whole_word);
        assert!(!options.is_regex);
        assert_eq!(options.max_results, 1000);
    }

    #[test]
    fn test_search_options_literal() {
        let options = SearchOptions::literal();
        assert!(!options.is_regex);
    }

    #[test]
    fn test_search_options_regex() {
        let options = SearchOptions::regex();
        assert!(options.is_regex);
    }

    #[tokio::test]
    async fn test_search_error_variants() {
        let provider = MockSearchProvider::new();
        let query = SearchQuery::new("pattern", SearchScope::Workspace);

        let result = provider.search(&query).await;
        assert!(result.is_ok());

        let validation = provider.validate_query(&query);
        assert_eq!(validation.is_valid, true);
    }

    #[test]
    fn test_replacement_new() {
        let start = Location::new("test.rs", 0, 0);
        let end = Location::new("test.rs", 0, 10);
        let replacement = Replacement::new("test.rs", crate::domain::value_objects::SourceRange::new(start, end), "new_content");
        assert_eq!(replacement.file, std::path::PathBuf::from("test.rs"));
        assert_eq!(replacement.new_text, "new_content");
    }

    #[test]
    fn test_search_match_location() {
        let search_match = SearchMatch::mock();
        let location = search_match.location();
        assert_eq!(location.file(), "test.rs");
        assert_eq!(location.line(), 1);
    }

    #[test]
    fn test_search_error_display() {
        let error = SearchError::InvalidPattern("bad pattern".to_string());
        assert!(error.to_string().contains("Invalid pattern"));
        assert!(error.to_string().contains("bad pattern"));
    }

    #[test]
    fn test_search_scope_file() {
        let scope = SearchScope::File(std::path::PathBuf::from("test.rs"));
        match scope {
            SearchScope::File(path) => assert_eq!(path, std::path::PathBuf::from("test.rs")),
            _ => panic!("Expected File scope"),
        }
    }

    #[test]
    fn test_search_scope_directory() {
        let scope = SearchScope::Directory {
            path: std::path::PathBuf::from("./src"),
            recursive: true,
        };
        match scope {
            SearchScope::Directory { path, recursive } => {
                assert_eq!(path, std::path::PathBuf::from("./src"));
                assert!(recursive);
            }
            _ => panic!("Expected Directory scope"),
        }
    }
}
