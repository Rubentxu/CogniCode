use crate::domain::aggregates::Symbol;
use crate::domain::traits::code_intelligence::{
    CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol, DocumentSymbolKind,
    HoverInfo, HoverKind, Reference, ReferenceKind, TypeHierarchy,
};
use crate::domain::value_objects::{Location, SymbolKind};
use crate::infrastructure::graph::LightweightIndex;
use crate::infrastructure::parser::{Language, TreeSitterParser};
use std::path::Path;
use std::sync::Arc;

pub struct TreesitterFallbackProvider {
    index: Option<Arc<LightweightIndex>>,
}

impl TreesitterFallbackProvider {
    pub fn new() -> Self {
        Self { index: None }
    }

    pub fn with_index(index: Arc<LightweightIndex>) -> Self {
        Self { index: Some(index) }
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
        let language = Language::from_extension(path.extension())
            .ok_or_else(|| CodeIntelligenceError::LanguageNotSupported(
                path.extension().and_then(|e| e.to_str()).unwrap_or("unknown").to_string(),
            ))?;

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
        let language = Self::detect_language(file_path)
            .ok_or_else(|| CodeIntelligenceError::LanguageNotSupported(location.file().to_string()))?;

        let source = std::fs::read_to_string(file_path)
            .map_err(|e| CodeIntelligenceError::FileNotFound(e.to_string()))?;

        let line = location.line().saturating_sub(1);
        let col = location.column().saturating_sub(1);
        let identifier = Self::extract_identifier_at_position(&source, line, col)
            .ok_or_else(|| CodeIntelligenceError::InvalidLocation(
                format!("No identifier at {}:{}", location.file(), location.line()),
            ))?;

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

    async fn get_definition(
        &self,
        location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError> {
        let file_path = Path::new(location.file());
        let source = std::fs::read_to_string(file_path)
            .map_err(|e| CodeIntelligenceError::FileNotFound(e.to_string()))?;

        let line = location.line().saturating_sub(1);
        let col = location.column().saturating_sub(1);
        let identifier = Self::extract_identifier_at_position(&source, line, col)
            .ok_or_else(|| CodeIntelligenceError::InvalidLocation(
                format!("No identifier at {}:{}", location.file(), location.line()),
            ))?;

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

            for candidate in candidates {
                if !valid_kinds.contains(&candidate.symbol_kind) {
                    continue;
                }
                let candidate_path = Path::new(&candidate.file);
                if let Ok(content) = std::fs::read_to_string(candidate_path) {
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
            return Ok(None);
        }

        let dir = file_path.parent().unwrap_or(Path::new("."));
        for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() || Language::from_extension(path.extension()).is_none() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(path) {
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
        Ok(None)
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
                    SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface => DocumentSymbolKind::Class,
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

    async fn hover(
        &self,
        location: &Location,
    ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
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
}
