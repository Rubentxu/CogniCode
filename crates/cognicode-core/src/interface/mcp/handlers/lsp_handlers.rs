use super::*;
use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
use crate::infrastructure::lsp::providers::CompositeProvider;

/// Helper to get the code intelligence provider from context or create a default one
fn get_provider(ctx: &HandlerContext) -> Arc<dyn CodeIntelligenceProvider> {
    if let Some(provider) = &ctx.code_intelligence_provider {
        provider.clone()
    } else {
        Arc::new(CompositeProvider::new(&ctx.working_dir))
    }
}

pub async fn handle_go_to_definition(
    ctx: &HandlerContext,
    input: GoToDefinitionInput,
) -> HandlerResult<GoToDefinitionOutput> {
    let provider = get_provider(ctx);
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    match provider.get_definition(&location).await {
        Ok(Some(def_loc)) => {
            let source = std::fs::read_to_string(def_loc.file()).ok();
            let context = source.as_ref().map(|s| {
                let lines: Vec<&str> = s.lines().collect();
                let line_idx = (def_loc.line() as usize).saturating_sub(1);
                if line_idx < lines.len() {
                    lines[line_idx].to_string()
                } else {
                    String::new()
                }
            });
            Ok(GoToDefinitionOutput {
                found: true,
                file: Some(def_loc.file().to_string()),
                line: Some(def_loc.line()),
                column: Some(def_loc.column()),
                context,
                message: None,
            })
        }
        Ok(None) => Ok(GoToDefinitionOutput {
            found: false,
            file: None,
            line: None,
            column: None,
            context: None,
            message: Some("No definition found at this position".to_string()),
        }),
        Err(e) => Ok(GoToDefinitionOutput {
            found: false,
            file: None,
            line: None,
            column: None,
            context: None,
            message: Some(e.to_string()),
        }),
    }
}

/// Handler for hover tool
pub async fn handle_hover(
    ctx: &HandlerContext,
    input: HoverInput,
) -> HandlerResult<HoverOutput> {
    let provider = get_provider(ctx);
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    match provider.hover(&location).await {
        Ok(Some(info)) => Ok(HoverOutput {
            found: true,
            content: Some(info.content),
            documentation: info.documentation,
            kind: Some(format!("{:?}", info.kind)),
        }),
        Ok(None) => Ok(HoverOutput {
            found: false,
            content: None,
            documentation: None,
            kind: None,
        }),
        Err(_) => Ok(HoverOutput {
            found: false,
            content: None,
            documentation: None,
            kind: None,
        }),
    }
}

/// Handler for find_references tool
pub async fn handle_find_references(
    ctx: &HandlerContext,
    input: FindReferencesInput,
) -> HandlerResult<FindReferencesOutput> {
    let provider = get_provider(ctx);
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    match provider.find_references(&location, input.include_declaration).await {
        Ok(refs) => {
            let entries: Vec<ReferenceEntry> = refs.iter().map(|r| ReferenceEntry {
                file: r.location.file().to_string(),
                line: r.location.line(),
                column: r.location.column(),
                kind: format!("{:?}", r.reference_kind),
                context: r.container.clone().unwrap_or_default(),
            }).collect();
            let total = entries.len();
            Ok(FindReferencesOutput {
                symbol: input.file_path,
                references: entries,
                total,
            })
        }
        Err(_) => Ok(FindReferencesOutput {
            symbol: input.file_path,
            references: vec![],
            total: 0,
        }),
    }
}

/// Input for get_document_symbols tool
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetDocumentSymbolsInput {
    pub file_path: String,
}

/// Output for get_document_symbols tool
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetDocumentSymbolsOutput {
    pub file_path: String,
    pub symbols: Vec<DocumentSymbolInfo>,
    pub total: usize,
}

/// A document symbol with simplified structure for MCP response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentSymbolInfo {
    pub name: String,
    pub kind: String,
    pub line: u32,
    pub column: u32,
    pub children: Vec<DocumentSymbolInfo>,
}

/// Handler for get_document_symbols tool
pub async fn handle_get_document_symbols(
    ctx: &HandlerContext,
    input: GetDocumentSymbolsInput,
) -> HandlerResult<GetDocumentSymbolsOutput> {
    let provider = get_provider(ctx);
    let path = std::path::Path::new(&input.file_path);

    match provider.get_document_symbols(path).await {
        Ok(symbols) => {
            fn convert_symbol(ds: &crate::domain::traits::code_intelligence::DocumentSymbol) -> DocumentSymbolInfo {
                DocumentSymbolInfo {
                    name: ds.symbol.name().to_string(),
                    kind: format!("{:?}", ds.document_kind),
                    line: ds.symbol.location().line(),
                    column: ds.symbol.location().column(),
                    children: ds.children.iter().map(convert_symbol).collect(),
                }
            }

            let symbol_infos: Vec<DocumentSymbolInfo> = symbols.iter().map(convert_symbol).collect();
            let total = symbol_infos.len();

            Ok(GetDocumentSymbolsOutput {
                file_path: input.file_path,
                symbols: symbol_infos,
                total,
            })
        }
        Err(_) => Ok(GetDocumentSymbolsOutput {
            file_path: input.file_path,
            symbols: vec![],
            total: 0,
        }),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::traits::code_intelligence::{
        CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol, DocumentSymbolKind,
        HoverInfo, HoverKind, Reference, ReferenceKind,
    };
    use crate::domain::aggregates::Symbol;
    use crate::domain::value_objects::{Location, SourceRange, SymbolKind};
    use std::sync::Arc;

    /// Mock provider for testing LSP handlers
    struct MockLspProvider {
        definition_result: Arc<Result<Option<Location>, CodeIntelligenceError>>,
        hover_result: Arc<Result<Option<HoverInfo>, CodeIntelligenceError>>,
        references_result: Arc<Result<Vec<Reference>, CodeIntelligenceError>>,
        document_symbols_result: Arc<Result<Vec<DocumentSymbol>, CodeIntelligenceError>>,
    }

    impl MockLspProvider {
        fn new() -> Self {
            Self {
                definition_result: Arc::new(Ok(None)),
                hover_result: Arc::new(Ok(None)),
                references_result: Arc::new(Ok(vec![])),
                document_symbols_result: Arc::new(Ok(vec![])),
            }
        }

        fn with_definition(mut self, result: Result<Option<Location>, CodeIntelligenceError>) -> Self {
            self.definition_result = Arc::new(result);
            self
        }

        fn with_hover(mut self, result: Result<Option<HoverInfo>, CodeIntelligenceError>) -> Self {
            self.hover_result = Arc::new(result);
            self
        }

        fn with_references(mut self, result: Result<Vec<Reference>, CodeIntelligenceError>) -> Self {
            self.references_result = Arc::new(result);
            self
        }

        fn with_document_symbols(mut self, result: Result<Vec<DocumentSymbol>, CodeIntelligenceError>) -> Self {
            self.document_symbols_result = Arc::new(result);
            self
        }
    }

    #[async_trait::async_trait]
    impl CodeIntelligenceProvider for MockLspProvider {
        async fn get_symbols(&self, _path: &std::path::Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
            Ok(vec![])
        }

        async fn find_references(
            &self,
            _location: &Location,
            _include_declaration: bool,
        ) -> Result<Vec<Reference>, CodeIntelligenceError> {
            // Use deref clone to extract the inner result and return it
            match self.references_result.as_ref() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(CodeIntelligenceError::Internal(e.to_string())),
            }
        }

        async fn get_hierarchy(&self, _location: &Location) -> Result<crate::domain::traits::code_intelligence::TypeHierarchy, CodeIntelligenceError> {
            Err(CodeIntelligenceError::Internal("Not implemented".to_string()))
        }

        async fn get_definition(&self, _location: &Location) -> Result<Option<Location>, CodeIntelligenceError> {
            match self.definition_result.as_ref() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(CodeIntelligenceError::Internal(e.to_string())),
            }
        }

        async fn get_document_symbols(&self, _path: &std::path::Path) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
            match self.document_symbols_result.as_ref() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(CodeIntelligenceError::Internal(e.to_string())),
            }
        }

        async fn hover(&self, _location: &Location) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
            match self.hover_result.as_ref() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(CodeIntelligenceError::Internal(e.to_string())),
            }
        }
    }

    fn create_test_location(file: &str, line: u32, col: u32) -> Location {
        Location::new(file, line, col)
    }

    fn create_test_symbol(name: &str, kind: SymbolKind, loc: Location) -> Symbol {
        Symbol::new(name, kind, loc)
    }

    fn create_test_source_range(file: &str, start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> SourceRange {
        SourceRange::new(
            Location::new(file, start_line, start_col),
            Location::new(file, end_line, end_col),
        )
    }

    // ========================================================================
    // handle_go_to_definition tests
    // ========================================================================

    #[tokio::test]
    async fn test_handle_go_to_definition_success() {
        let def_location = Location::new("test.rs", 10, 5);
        let provider = Arc::new(MockLspProvider::new().with_definition(Ok(Some(def_location.clone()))));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        // Create the file that contains the definition
        std::fs::write(temp_dir.path().join("test.rs"), "fn test() {}\n").unwrap();

        let input = GoToDefinitionInput {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 3,
        };

        let result = handle_go_to_definition(&ctx, input).await.unwrap();
        assert!(result.found);
        assert_eq!(result.file, Some("test.rs".to_string()));
        assert_eq!(result.line, Some(10));
        assert_eq!(result.column, Some(5));
    }

    #[tokio::test]
    async fn test_handle_go_to_definition_not_found() {
        let provider = Arc::new(MockLspProvider::new().with_definition(Ok(None)));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = GoToDefinitionInput {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 3,
        };

        let result = handle_go_to_definition(&ctx, input).await.unwrap();
        assert!(!result.found);
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("No definition found"));
    }

    #[tokio::test]
    async fn test_handle_go_to_definition_error() {
        let provider = Arc::new(MockLspProvider::new().with_definition(
            Err(CodeIntelligenceError::LspError("Server unavailable".to_string()))
        ));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = GoToDefinitionInput {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 3,
        };

        let result = handle_go_to_definition(&ctx, input).await.unwrap();
        assert!(!result.found);
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("Server unavailable"));
    }

    // ========================================================================
    // handle_hover tests
    // ========================================================================

    #[tokio::test]
    async fn test_handle_hover_success() {
        let hover_info = HoverInfo {
            content: "fn main() -> ()".to_string(),
            documentation: Some("The entry point".to_string()),
            kind: HoverKind::Mixed,
        };
        let provider = Arc::new(MockLspProvider::new().with_hover(Ok(Some(hover_info))));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = HoverInput {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 3,
        };

        let result = handle_hover(&ctx, input).await.unwrap();
        assert!(result.found);
        assert!(result.content.is_some());
        assert_eq!(result.content.unwrap(), "fn main() -> ()");
        assert!(result.documentation.is_some());
        assert_eq!(result.kind.unwrap(), "Mixed");
    }

    #[tokio::test]
    async fn test_handle_hover_not_available() {
        let provider = Arc::new(MockLspProvider::new().with_hover(Ok(None)));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = HoverInput {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 3,
        };

        let result = handle_hover(&ctx, input).await.unwrap();
        assert!(!result.found);
        assert!(result.content.is_none());
        assert!(result.documentation.is_none());
    }

    #[tokio::test]
    async fn test_handle_hover_error() {
        let provider = Arc::new(MockLspProvider::new().with_hover(
            Err(CodeIntelligenceError::LspError("Server unavailable".to_string()))
        ));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = HoverInput {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 3,
        };

        let result = handle_hover(&ctx, input).await.unwrap();
        // Error still returns a valid output with found=false
        assert!(!result.found);
        assert!(result.content.is_none());
    }

    // ========================================================================
    // handle_find_references tests
    // ========================================================================

    #[tokio::test]
    async fn test_handle_find_references_found() {
        let refs = vec![
            Reference {
                location: Location::new("main.rs", 5, 0),
                reference_kind: ReferenceKind::Read,
                container: Some("main".to_string()),
            },
            Reference {
                location: Location::new("other.rs", 10, 0),
                reference_kind: ReferenceKind::Write,
                container: None,
            },
        ];
        let provider = Arc::new(MockLspProvider::new().with_references(Ok(refs)));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = FindReferencesInput {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 3,
            include_declaration: true,
        };

        let result = handle_find_references(&ctx, input).await.unwrap();
        assert_eq!(result.total, 2);
        assert_eq!(result.references.len(), 2);
        assert_eq!(result.references[0].file, "main.rs");
        assert_eq!(result.references[0].line, 5);
        assert_eq!(result.references[1].file, "other.rs");
    }

    #[tokio::test]
    async fn test_handle_find_references_not_found() {
        let provider = Arc::new(MockLspProvider::new().with_references(Ok(vec![])));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = FindReferencesInput {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 3,
            include_declaration: true,
        };

        let result = handle_find_references(&ctx, input).await.unwrap();
        assert_eq!(result.total, 0);
        assert!(result.references.is_empty());
    }

    #[tokio::test]
    async fn test_handle_find_references_error() {
        let provider = Arc::new(MockLspProvider::new().with_references(
            Err(CodeIntelligenceError::LspError("Server unavailable".to_string()))
        ));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = FindReferencesInput {
            file_path: "test.rs".to_string(),
            line: 1,
            column: 3,
            include_declaration: true,
        };

        let result = handle_find_references(&ctx, input).await.unwrap();
        // Error returns empty results but still succeeds
        assert_eq!(result.total, 0);
        assert!(result.references.is_empty());
    }

    // ========================================================================
    // handle_get_document_symbols tests
    // ========================================================================

    #[tokio::test]
    async fn test_handle_get_document_symbols_extracted() {
        let loc1 = Location::new("test.rs", 1, 0);
        let loc2 = Location::new("test.rs", 5, 0);
        let loc3 = Location::new("test.rs", 10, 0);

        let symbols = vec![
            DocumentSymbol {
                symbol: Symbol::new("MyFunction", SymbolKind::Function, loc1.clone()),
                document_kind: DocumentSymbolKind::Function,
                range: SourceRange::new(loc1.clone(), loc2.clone()),
                children: vec![],
            },
            DocumentSymbol {
                symbol: Symbol::new("MyClass", SymbolKind::Class, loc2.clone()),
                document_kind: DocumentSymbolKind::Class,
                range: SourceRange::new(loc2.clone(), loc3.clone()),
                children: vec![
                    DocumentSymbol {
                        symbol: Symbol::new("field1", SymbolKind::Field, loc2.clone()),
                        document_kind: DocumentSymbolKind::Field,
                        range: SourceRange::new(loc2.clone(), loc2.clone()),
                        children: vec![],
                    },
                ],
            },
        ];
        let provider = Arc::new(MockLspProvider::new().with_document_symbols(Ok(symbols)));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = GetDocumentSymbolsInput {
            file_path: "test.rs".to_string(),
        };

        let result = handle_get_document_symbols(&ctx, input).await.unwrap();
        assert_eq!(result.total, 2);
        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].name, "MyFunction");
        assert_eq!(result.symbols[0].kind, "Function");
        assert_eq!(result.symbols[1].name, "MyClass");
        assert_eq!(result.symbols[1].kind, "Class");
        // Check nested children
        assert_eq!(result.symbols[1].children.len(), 1);
        assert_eq!(result.symbols[1].children[0].name, "field1");
    }

    #[tokio::test]
    async fn test_handle_get_document_symbols_empty_file() {
        let provider = Arc::new(MockLspProvider::new().with_document_symbols(Ok(vec![])));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = GetDocumentSymbolsInput {
            file_path: "empty.rs".to_string(),
        };

        let result = handle_get_document_symbols(&ctx, input).await.unwrap();
        assert_eq!(result.total, 0);
        assert!(result.symbols.is_empty());
    }

    #[tokio::test]
    async fn test_handle_get_document_symbols_large_file() {
        // Simulate a large file with many symbols
        let mut symbols = Vec::new();
        for i in 0..100 {
            let loc = Location::new("large.rs", i * 5, 0);
            symbols.push(DocumentSymbol {
                symbol: Symbol::new(&format!("function_{}", i), SymbolKind::Function, loc.clone()),
                document_kind: DocumentSymbolKind::Function,
                range: SourceRange::new(loc.clone(), Location::new("large.rs", (i + 1) * 5, 0)),
                children: vec![],
            });
        }
        let provider = Arc::new(MockLspProvider::new().with_document_symbols(Ok(symbols)));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = GetDocumentSymbolsInput {
            file_path: "large.rs".to_string(),
        };

        let result = handle_get_document_symbols(&ctx, input).await.unwrap();
        assert_eq!(result.total, 100);
        assert_eq!(result.symbols.len(), 100);
    }

    #[tokio::test]
    async fn test_handle_get_document_symbols_error() {
        let provider = Arc::new(MockLspProvider::new().with_document_symbols(
            Err(CodeIntelligenceError::LspError("Server unavailable".to_string()))
        ));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::with_code_intelligence_provider(
            temp_dir.path().to_path_buf(),
            provider,
        );

        let input = GetDocumentSymbolsInput {
            file_path: "test.rs".to_string(),
        };

        let result = handle_get_document_symbols(&ctx, input).await.unwrap();
        // Error returns empty results but still succeeds
        assert_eq!(result.total, 0);
        assert!(result.symbols.is_empty());
    }
}
