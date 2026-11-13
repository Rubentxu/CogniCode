use crate::domain::aggregates::Symbol;
use crate::domain::traits::code_intelligence::{
    CallHierarchyCall, CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol,
    DocumentSymbolKind, HoverInfo, HoverKind, PrepareRename, Reference, ReferenceKind, RenameEdits,
    TypeHierarchy,
};
use crate::domain::value_objects::{Location, SymbolKind};
use crate::infrastructure::graph::LightweightIndex;
use crate::infrastructure::parser::{Language, TreeSitterParser};
use std::path::Path;
use std::sync::Arc;

/// Tree-sitter parse/heuristic provider with an optional workspace index.
///
/// Serves two tiers of the composite pipeline (LSI M4):
///
/// * **S1** — the index-backed `get_definition` local resolver
///   ([`Self::LOCAL_RESOLVER_PROVIDER_ID`]).
/// * **S0** — all parse/heuristic ops ([`Self::PROVIDER_ID`]).
pub struct TreesitterFallbackProvider {
    index: Option<Arc<LightweightIndex>>,
}

impl Default for TreesitterFallbackProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TreesitterFallbackProvider {
    /// Stable provider identity for the S0 tree-sitter tier.
    pub const PROVIDER_ID: &'static str = "tree-sitter";
    /// Stable provider identity for the S1 index-backed definition resolver.
    pub const LOCAL_RESOLVER_PROVIDER_ID: &'static str = "local-resolver";

    pub fn new() -> Self {
        Self { index: None }
    }

    pub fn with_index(index: Arc<LightweightIndex>) -> Self {
        Self { index: Some(index) }
    }

    /// The answer of a definition lookup that found nothing.
    ///
    /// A miss over everything that could be read is a plain `Ok(None)`. A miss
    /// after some file or directory could not be read is not: the definition
    /// may live in what was never looked at, so it is an error that names the
    /// unread paths, and the composite provider propagates it instead of
    /// reporting "no definition".
    fn miss(
        identifier: &str,
        unread: Vec<String>,
    ) -> Result<Option<Location>, CodeIntelligenceError> {
        if unread.is_empty() {
            return Ok(None);
        }
        Err(CodeIntelligenceError::Internal(format!(
            "no definition of `{identifier}` found in what could be read; could not read: {}",
            unread.join(", ")
        )))
    }

    fn detect_language(file_path: &Path) -> Option<Language> {
        Language::from_extension(file_path.extension())
    }

    fn extract_identifier_at_position(source: &str, line: u32, column: u32) -> Option<String> {
        let lines: Vec<&str> = source.lines().collect();
        if (line as usize) >= lines.len() {
            return None;
        }
        let line_str = lines[line as usize];
        let chars: Vec<char> = line_str.chars().collect();
        let mut start = column as usize;
        let mut end = column as usize;
        if start >= chars.len() {
            return None;
        }
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        if start == end {
            return None;
        }
        Some(chars[start..end].iter().collect())
    }
}

#[async_trait::async_trait]
impl CodeIntelligenceProvider for TreesitterFallbackProvider {
    async fn get_symbols(&self, path: &Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
        let language = Language::from_extension(path.extension()).ok_or_else(|| {
            CodeIntelligenceError::LanguageNotSupported(
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            )
        })?;

        let parser = TreeSitterParser::new(language)
            .map_err(|e| CodeIntelligenceError::Internal(e.to_string()))?;

        let source = std::fs::read_to_string(path)
            .map_err(|e| CodeIntelligenceError::FileNotFound(e.to_string()))?;

        let symbols = parser
            .find_all_symbols_with_path(&source, path.to_string_lossy().as_ref())
            .map_err(|e| CodeIntelligenceError::ParseError(e.to_string()))?;

        Ok(symbols)
    }

    async fn find_references(
        &self,
        location: &Location,
        _include_declaration: bool,
    ) -> Result<Vec<Reference>, CodeIntelligenceError> {
        let file_path = Path::new(location.file());
        let language = Self::detect_language(file_path).ok_or_else(|| {
            CodeIntelligenceError::LanguageNotSupported(location.file().to_string())
        })?;

        let source = std::fs::read_to_string(file_path)
            .map_err(|e| CodeIntelligenceError::FileNotFound(e.to_string()))?;

        let line = location.line().saturating_sub(1);
        let col = location.column().saturating_sub(1);
        let identifier =
            Self::extract_identifier_at_position(&source, line, col).ok_or_else(|| {
                CodeIntelligenceError::InvalidLocation(format!(
                    "No identifier at {}:{}",
                    location.file(),
                    location.line()
                ))
            })?;

        let parser = TreeSitterParser::new(language)
            .map_err(|e| CodeIntelligenceError::Internal(e.to_string()))?;

        let occurrences = parser
            .find_all_occurrences_of_identifier(&source, &identifier)
            .map_err(|e| CodeIntelligenceError::ParseError(e.to_string()))?;

        let references: Vec<Reference> = occurrences
            .into_iter()
            .map(|occ| Reference {
                location: Location::new(location.file().to_string(), occ.line + 1, occ.column + 1),
                reference_kind: ReferenceKind::Read,
                container: None,
            })
            .collect();

        Ok(references)
    }

    async fn get_hierarchy(
        &self,
        _location: &Location,
    ) -> Result<TypeHierarchy, CodeIntelligenceError> {
        Err(CodeIntelligenceError::Internal(
            "Type hierarchy not supported in tree-sitter fallback".to_string(),
        ))
    }

    async fn prepare_rename(
        &self,
        _location: &Location,
    ) -> Result<Option<PrepareRename>, CodeIntelligenceError> {
        Err(CodeIntelligenceError::Internal(
            "prepareRename not supported in tree-sitter fallback".to_string(),
        ))
    }

    async fn rename(
        &self,
        _location: &Location,
        _new_name: &str,
    ) -> Result<Option<RenameEdits>, CodeIntelligenceError> {
        Err(CodeIntelligenceError::Internal(
            "rename not supported in tree-sitter fallback".to_string(),
        ))
    }

    async fn incoming_calls(
        &self,
        _location: &Location,
    ) -> Result<Vec<CallHierarchyCall>, CodeIntelligenceError> {
        Err(CodeIntelligenceError::Internal(
            "incomingCalls not supported in tree-sitter fallback".to_string(),
        ))
    }

    async fn outgoing_calls(
        &self,
        _location: &Location,
    ) -> Result<Vec<CallHierarchyCall>, CodeIntelligenceError> {
        Err(CodeIntelligenceError::Internal(
            "outgoingCalls not supported in tree-sitter fallback".to_string(),
        ))
    }

    async fn get_definition(
        &self,
        location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError> {
        let file_path = Path::new(location.file());
        let source = std::fs::read_to_string(file_path)
            .map_err(|e| CodeIntelligenceError::FileNotFound(e.to_string()))?;

        let line = location.line().saturating_sub(1);
        let col = location.column().saturating_sub(1);
        let identifier =
            Self::extract_identifier_at_position(&source, line, col).ok_or_else(|| {
                CodeIntelligenceError::InvalidLocation(format!(
                    "No identifier at {}:{}",
                    location.file(),
                    location.line()
                ))
            })?;

        if let Some(ref index) = self.index {
            let candidates = index.find_symbol(&identifier);
            let valid_kinds = [
                SymbolKind::Function,
                SymbolKind::Method,
                SymbolKind::Struct,
                SymbolKind::Class,
                SymbolKind::Enum,
                SymbolKind::Interface,
            ];

            let mut unread: Vec<String> = Vec::new();
            for candidate in candidates {
                if !valid_kinds.contains(&candidate.symbol_kind) {
                    continue;
                }
                let candidate_path = Path::new(&candidate.file);
                match std::fs::read_to_string(candidate_path) {
                    Err(error) => unread.push(format!("{} ({error})", candidate.file)),
                    Ok(content) => {
                        let line_idx = candidate.line as usize;
                        let lines: Vec<&str> = content.lines().collect();
                        if line_idx < lines.len() {
                            let line_str = lines[line_idx];
                            let is_def = line_str.contains(&format!("fn {}(", &identifier))
                                || line_str.contains(&format!("def {}(", &identifier))
                                || line_str.contains(&format!("function {}(", &identifier))
                                || line_str.contains(&format!("struct {}", &identifier))
                                || line_str.contains(&format!("class {}", &identifier));
                            if is_def {
                                return Ok(Some(Location::new(
                                    candidate.file.clone(),
                                    candidate.line + 1,
                                    candidate.column + 1,
                                )));
                            }
                        }
                    }
                }
            }
            return Self::miss(&identifier, unread);
        }

        let dir = file_path.parent().unwrap_or(Path::new("."));
        // The directory is read through the one walk the product owns: what
        // a definition lookup may open is what the graph may open, so it
        // does not read `target/`, a `.gitignore`d tree or a secrets
        // directory just because a file happens to live next to them.
        let mut unread: Vec<String> = Vec::new();
        for entry in crate::infrastructure::graph::source_files::walk(dir) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    unread.push(error.to_string());
                    continue;
                }
            };
            let path = entry.path();
            if !crate::infrastructure::walk_policy::ignore_is_real_file(&entry)
                || Language::from_extension(path.extension()).is_none()
            {
                continue;
            }
            match std::fs::read_to_string(path) {
                Err(error) => unread.push(format!("{} ({error})", path.display())),
                Ok(content) => {
                    for (i, line_str) in content.lines().enumerate() {
                        if let Some(c) = line_str.find(&identifier) {
                            let is_def = line_str.contains(&format!("fn {}(", &identifier))
                                || line_str.contains(&format!("def {}(", &identifier))
                                || line_str.contains(&format!("function {}(", &identifier))
                                || line_str.contains(&format!("struct {}", &identifier))
                                || line_str.contains(&format!("class {}", &identifier));
                            if is_def {
                                return Ok(Some(Location::new(
                                    path.to_string_lossy().to_string(),
                                    (i + 1) as u32,
                                    (c + 1) as u32,
                                )));
                            }
                        }
                    }
                }
            }
        }
        Self::miss(&identifier, unread)
    }

    async fn get_document_symbols(
        &self,
        path: &Path,
    ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
        let symbols = self.get_symbols(path).await?;
        Ok(symbols
            .iter()
            .map(|s| DocumentSymbol {
                symbol: s.clone(),
                document_kind: match s.kind() {
                    SymbolKind::Function | SymbolKind::Method => DocumentSymbolKind::Function,
                    SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface => {
                        DocumentSymbolKind::Class
                    }
                    SymbolKind::Variable => DocumentSymbolKind::Variable,
                    SymbolKind::Constant => DocumentSymbolKind::Constant,
                    SymbolKind::Module | SymbolKind::Namespace => DocumentSymbolKind::Module,
                    SymbolKind::Enum => DocumentSymbolKind::Enum,
                    SymbolKind::Constructor => DocumentSymbolKind::Constructor,
                    SymbolKind::Property | SymbolKind::Field => DocumentSymbolKind::Field,
                    _ => DocumentSymbolKind::Variable,
                },
                range: crate::domain::value_objects::SourceRange::new(
                    s.location().clone(),
                    s.location().clone(),
                ),
                children: Vec::new(),
            })
            .collect())
    }

    async fn hover(&self, location: &Location) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
        let file_path = Path::new(location.file());
        let source = std::fs::read_to_string(file_path)
            .map_err(|e| CodeIntelligenceError::FileNotFound(e.to_string()))?;

        let line = location.line().saturating_sub(1);
        let col = location.column().saturating_sub(1);

        if let Some(identifier) = Self::extract_identifier_at_position(&source, line, col) {
            return Ok(Some(HoverInfo {
                content: format!("`{}` (type: unknown — tree-sitter fallback)", identifier),
                documentation: None,
                kind: HoverKind::Snippet,
            }));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_identifier_basic() {
        let source = "let foo = bar();";
        assert_eq!(
            TreesitterFallbackProvider::extract_identifier_at_position(source, 0, 4),
            Some("foo".to_string())
        );
    }

    #[test]
    fn test_extract_identifier_nothing() {
        assert_eq!(
            TreesitterFallbackProvider::extract_identifier_at_position("    ", 0, 2),
            None
        );
    }

    /// The fallback has no rename or call hierarchy: it says so, rather than
    /// answering "nothing to rename" or "no callers".
    #[tokio::test]
    async fn rename_and_call_hierarchy_are_declared_unsupported() {
        let provider = TreesitterFallbackProvider::new();
        let location = Location::new("/tmp/test.rs", 1, 1);
        let errors = [
            provider.prepare_rename(&location).await.unwrap_err(),
            provider.rename(&location, "x").await.unwrap_err(),
            provider.incoming_calls(&location).await.unwrap_err(),
            provider.outgoing_calls(&location).await.unwrap_err(),
        ];
        for error in errors {
            let message = error.to_string();
            assert!(
                message.contains("tree-sitter fallback"),
                "the reason must name the fallback: {message}"
            );
        }
    }

    #[tokio::test]
    async fn test_get_hierarchy_not_supported() {
        let provider = TreesitterFallbackProvider::new();
        let location = Location::new("/tmp/test.rs", 1, 1);
        assert!(provider.get_hierarchy(&location).await.is_err());
    }

    // Task 4.2: TreesitterFallbackProvider unit tests

    #[tokio::test]
    async fn test_fallback_hover_with_identifier() {
        use std::io::Write;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("test.rs");
        let source = "fn hello() {}\nlet x = 5;";
        std::fs::File::create(&file_path)
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();

        let provider = TreesitterFallbackProvider::new();
        let loc = Location::new(file_path.to_string_lossy().to_string(), 1, 4);
        let result = provider.hover(&loc).await;
        // File exists and has identifier at line 1 col 4 ("hello" in "fn hello()")
        assert!(result.is_ok());
        let hover = result.unwrap();
        // Should detect "hello" identifier
        assert!(hover.is_some());
        let info = hover.unwrap();
        assert!(info.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_fallback_hover_nonexistent_file() {
        let provider = TreesitterFallbackProvider::new();
        let loc = Location::new("/nonexistent/path/test.rs", 1, 1);
        let result = provider.hover(&loc).await;
        // Should return FileNotFound error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fallback_get_symbols_real_file() {
        use std::io::Write;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("sample.rs");
        let source = "fn greet(name: &str) -> String { format!(\"Hello, {}\", name) }\nstruct Point { x: f64, y: f64 }";
        std::fs::File::create(&file_path)
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();

        let provider = TreesitterFallbackProvider::new();
        let result = provider.get_symbols(&file_path).await;
        assert!(result.is_ok());
        let symbols = result.unwrap();
        // Should find at least the function and struct
        assert!(!symbols.is_empty());
        let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();
        assert!(names.contains(&"greet") || names.contains(&"Point"));
    }

    /// Gives a path its permissions back when the test ends, so the temporary
    /// directory can be removed even if an assertion failed halfway.
    struct Unlock(std::path::PathBuf, u32);

    impl Drop for Unlock {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(self.1));
        }
    }

    fn lock(path: &Path, restore: u32) -> Unlock {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000))
            .expect("the fixture must be lockable");
        Unlock(path.to_path_buf(), restore)
    }

    /// `m_caller.rs` calls `helper()` on line 1; the cursor sits on the call.
    fn caller_location(dir: &Path) -> Location {
        Location::new(dir.join("m_caller.rs").to_string_lossy().to_string(), 1, 18)
    }

    fn write_caller(dir: &Path) {
        std::fs::write(dir.join("m_caller.rs"), "fn run() { helper(); }\n").unwrap();
    }

    #[tokio::test]
    async fn a_definition_that_may_live_in_an_unreadable_file_is_not_reported_as_absent() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_caller(tmp.path());
        let locked = tmp.path().join("z_locked.rs");
        std::fs::write(&locked, "fn helper() {}\n").unwrap();
        let _unlock = lock(&locked, 0o644);

        let result = TreesitterFallbackProvider::new()
            .get_definition(&caller_location(tmp.path()))
            .await;

        let error = result.expect_err("a miss after an unreadable file is not a clean miss");
        let message = error.to_string();
        assert!(
            message.contains("z_locked.rs"),
            "the error must name the file it could not read: {message}"
        );
    }

    #[tokio::test]
    async fn a_definition_found_in_a_readable_file_survives_an_unreadable_one() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_caller(tmp.path());
        let locked = tmp.path().join("a_locked.rs");
        std::fs::write(&locked, "fn other() {}\n").unwrap();
        std::fs::write(tmp.path().join("z_defs.rs"), "fn helper() {}\n").unwrap();
        let _unlock = lock(&locked, 0o644);

        let found = TreesitterFallbackProvider::new()
            .get_definition(&caller_location(tmp.path()))
            .await
            .expect("a hit is a hit even when another file could not be read")
            .expect("the definition lives in a readable file");

        assert!(found.file().ends_with("z_defs.rs"), "{}", found.file());
    }

    #[tokio::test]
    async fn a_directory_that_was_fully_read_still_answers_a_clean_miss() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_caller(tmp.path());

        let result = TreesitterFallbackProvider::new()
            .get_definition(&caller_location(tmp.path()))
            .await
            .expect("nothing was unreadable, so a miss is a plain miss");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn an_unreadable_directory_met_during_the_walk_is_not_a_clean_miss() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_caller(tmp.path());
        let sealed = tmp.path().join("sealed");
        std::fs::create_dir(&sealed).unwrap();
        std::fs::write(sealed.join("defs.rs"), "fn helper() {}\n").unwrap();
        let _unlock = lock(&sealed, 0o755);

        let error = TreesitterFallbackProvider::new()
            .get_definition(&caller_location(tmp.path()))
            .await
            .expect_err("a directory that could not be listed may hold the definition");

        assert!(
            error.to_string().contains("sealed"),
            "the error must name the directory: {error}"
        );
    }

    #[tokio::test]
    async fn an_indexed_candidate_that_cannot_be_read_is_not_a_clean_miss() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_caller(tmp.path());
        let locked = tmp.path().join("z_locked.rs");
        std::fs::write(&locked, "fn helper() {}\n").unwrap();
        let mut index = LightweightIndex::new();
        index.build_from_sources([(locked.to_str().unwrap(), "fn helper() {}\n")]);
        let _unlock = lock(&locked, 0o644);

        let error = TreesitterFallbackProvider::with_index(Arc::new(index))
            .get_definition(&caller_location(tmp.path()))
            .await
            .expect_err("the indexed file exists but could not be read");

        assert!(
            error.to_string().contains("z_locked.rs"),
            "the error must name the file: {error}"
        );
    }
}
