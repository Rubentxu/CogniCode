// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::needless_borrows_for_generic_args, dead_code)]
use super::*;
use crate::domain::traits::code_intelligence::{
    CodeIntelligenceProvider, PrecisionTier, Reference, TieredCodeIntelligenceProvider,
    TieredOutcome,
};
use crate::infrastructure::lsp::providers::CompositeProvider;
use crate::interface::mcp::schemas::{RenameEditEntry, RenamePreviewInput, RenamePreviewOutput};
use crate::interface::symbol_identity::{
    default_provider_for_tool, precision_floor_for_tool, provider_for_tier,
};

/// Helper to get the code intelligence provider from context or create a default one
fn get_provider(ctx: &HandlerContext) -> Arc<dyn CodeIntelligenceProvider> {
    if let Some(provider) = &ctx.code_intelligence_provider {
        provider.clone()
    } else {
        Arc::new(CompositeProvider::new(&ctx.working_dir))
    }
}

#[cognicode_macros::aix_tool(
    name = "go_to_definition",
    description = "Navigate to the definition of a symbol using LSP.",
    input_schema = GoToDefinitionInput
)]
pub async fn handle_go_to_definition(
    ctx: &HandlerContext,
    input: GoToDefinitionInput,
) -> HandlerResult<GoToDefinitionOutput> {
    let provider = get_provider(ctx);
    // The path that reaches the provider is the path that was authorised.
    let authorised = authorise_file_path(ctx, &input.file_path)?;
    let location = crate::domain::value_objects::Location::new(
        authorised.to_string_lossy().into_owned(),
        input.line,
        input.column,
    );

    match provider.get_definition(&location).await {
        Ok(Some(def_loc)) => {
            // A definition the provider places outside the workspace is reported
            // but its source is never read.
            let source = authorise_file_path(ctx, def_loc.file())
                .ok()
                .and_then(|path| std::fs::read_to_string(path).ok());
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
                precision: precision_floor_for_tool("go_to_definition"),
                provider: default_provider_for_tool("go_to_definition").to_string(),
            })
        }
        Ok(None) => Ok(GoToDefinitionOutput {
            found: false,
            file: None,
            line: None,
            column: None,
            context: None,
            message: Some("No definition found at this position".to_string()),
            precision: precision_floor_for_tool("go_to_definition"),
            provider: default_provider_for_tool("go_to_definition").to_string(),
        }),
        Err(e) => Ok(GoToDefinitionOutput {
            found: false,
            file: None,
            line: None,
            column: None,
            context: None,
            message: Some(e.to_string()),
            precision: precision_floor_for_tool("go_to_definition"),
            provider: default_provider_for_tool("go_to_definition").to_string(),
        }),
    }
}

/// Handler for hover tool
#[cognicode_macros::aix_tool(
    name = "hover",
    description = "Get type information and documentation for a symbol at a position using LSP.",
    input_schema = HoverInput
)]
pub async fn handle_hover(ctx: &HandlerContext, input: HoverInput) -> HandlerResult<HoverOutput> {
    let provider = get_provider(ctx);
    // The path that reaches the provider is the path that was authorised.
    let authorised = authorise_file_path(ctx, &input.file_path)?;
    let location = crate::domain::value_objects::Location::new(
        authorised.to_string_lossy().into_owned(),
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

/// Handler for rename_preview tool
///
/// Asks the language server what a rename would change and returns the edits.
/// It never writes: applying them is a separate capability.
#[cognicode_macros::aix_tool(
    name = "rename_preview",
    description = "Preview the edits an LSP rename would make, without applying them.",
    input_schema = RenamePreviewInput
)]
pub async fn handle_rename_preview(
    ctx: &HandlerContext,
    input: RenamePreviewInput,
) -> HandlerResult<RenamePreviewOutput> {
    if input.new_name.trim().is_empty() {
        return Err(HandlerError::InvalidInput(
            "new_name must not be empty".to_string(),
        ));
    }
    let provider = get_provider(ctx);
    // The path that reaches the provider is the path that was authorised.
    let authorised = authorise_file_path(ctx, &input.file_path)?;
    let location = crate::domain::value_objects::Location::new(
        authorised.to_string_lossy().into_owned(),
        input.line,
        input.column,
    );
    let unavailable = |e: crate::domain::traits::code_intelligence::CodeIntelligenceError| {
        HandlerError::Internal(format!("rename preview unavailable: {e}"))
    };
    let answer = |renamable, placeholder, edits: Vec<RenameEditEntry>, skipped, message| {
        let total_edits = edits.len();
        RenamePreviewOutput {
            renamable,
            placeholder,
            edits,
            total_edits,
            skipped_resource_operations: skipped,
            partial: skipped > 0,
            message,
            precision: precision_floor_for_tool("rename_preview"),
            provider: provider_for_tier(PrecisionTier::S2).to_string(),
        }
    };

    let prepared = match provider
        .prepare_rename(&location)
        .await
        .map_err(unavailable)?
    {
        Some(prepared) => prepared,
        None => {
            return Ok(answer(
                false,
                None,
                Vec::new(),
                0,
                Some("the server reports nothing renamable at this position".to_string()),
            ));
        }
    };
    let Some(renamed) = provider
        .rename(&location, &input.new_name)
        .await
        .map_err(unavailable)?
    else {
        return Ok(answer(
            false,
            prepared.placeholder,
            Vec::new(),
            0,
            Some("the server declined to rename".to_string()),
        ));
    };
    let edits = renamed
        .edits
        .iter()
        .map(|e| RenameEditEntry {
            file: e.range.start().file().to_string(),
            start_line: e.range.start().line(),
            start_column: e.range.start().column(),
            end_line: e.range.end().line(),
            end_column: e.range.end().column(),
            new_text: e.new_text.clone(),
        })
        .collect();
    let skipped = renamed.skipped_resource_operations;
    let message = (skipped > 0)
        .then(|| format!("{skipped} file operation(s) in the server's answer are not represented"));
    Ok(answer(true, prepared.placeholder, edits, skipped, message))
}

/// Builds the `find_references` answer from a tiered outcome, labelling
/// `provider` with the tier that actually served the references.
fn references_output(
    symbol: String,
    outcome: TieredOutcome<Vec<Reference>>,
) -> FindReferencesOutput {
    let (references, provider) = match outcome {
        TieredOutcome::Served(tiered) => (tiered.value, provider_for_tier(tiered.tier)),
        TieredOutcome::Unresolved(_) => (Vec::new(), default_provider_for_tool("find_references")),
    };
    let entries: Vec<ReferenceEntry> = references
        .iter()
        .map(|r| ReferenceEntry {
            file: r.location.file().to_string(),
            line: r.location.line(),
            column: r.location.column(),
            kind: format!("{:?}", r.reference_kind),
            context: r.container.clone().unwrap_or_default(),
        })
        .collect();
    let total = entries.len();
    FindReferencesOutput {
        symbol,
        references: entries,
        total,
        precision: precision_floor_for_tool("find_references"),
        provider: provider.to_string(),
    }
}

/// Handler for find_references tool
#[cognicode_macros::aix_tool(
    name = "find_references",
    description = "Find all references to a symbol using LSP.",
    input_schema = FindReferencesInput
)]
pub async fn handle_find_references(
    ctx: &HandlerContext,
    input: FindReferencesInput,
) -> HandlerResult<FindReferencesOutput> {
    let provider = get_provider(ctx);
    // The path that reaches the provider is the path that was authorised.
    let authorised = authorise_file_path(ctx, &input.file_path)?;
    let location = crate::domain::value_objects::Location::new(
        authorised.to_string_lossy().into_owned(),
        input.line,
        input.column,
    );

    // Without an injected provider the composite chain answers, and its tier
    // says who served; an injected provider has no tier to report.
    if ctx.code_intelligence_provider.is_none() {
        let outcome = CompositeProvider::new(&ctx.working_dir)
            .find_references_tiered(&location, input.include_declaration)
            .await;
        return Ok(references_output(input.file_path, outcome));
    }
    match provider
        .find_references(&location, input.include_declaration)
        .await
    {
        Ok(refs) => {
            let entries: Vec<ReferenceEntry> = refs
                .iter()
                .map(|r| ReferenceEntry {
                    file: r.location.file().to_string(),
                    line: r.location.line(),
                    column: r.location.column(),
                    kind: format!("{:?}", r.reference_kind),
                    context: r.container.clone().unwrap_or_default(),
                })
                .collect();
            let total = entries.len();
            Ok(FindReferencesOutput {
                symbol: input.file_path,
                references: entries,
                total,
                precision: precision_floor_for_tool("find_references"),
                provider: default_provider_for_tool("find_references").to_string(),
            })
        }
        Err(_) => Ok(FindReferencesOutput {
            symbol: input.file_path,
            references: vec![],
            total: 0,
            precision: precision_floor_for_tool("find_references"),
            provider: default_provider_for_tool("find_references").to_string(),
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
            fn convert_symbol(
                ds: &crate::domain::traits::code_intelligence::DocumentSymbol,
            ) -> DocumentSymbolInfo {
                DocumentSymbolInfo {
                    name: ds.symbol.name().to_string(),
                    kind: format!("{:?}", ds.document_kind),
                    line: ds.symbol.location().line(),
                    column: ds.symbol.location().column(),
                    children: ds.children.iter().map(convert_symbol).collect(),
                }
            }

            let symbol_infos: Vec<DocumentSymbolInfo> =
                symbols.iter().map(convert_symbol).collect();
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
    use crate::domain::aggregates::Symbol;
    use crate::domain::traits::code_intelligence::{
        CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol, DocumentSymbolKind,
        HoverInfo, HoverKind, Reference, ReferenceKind,
    };
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

        fn with_definition(
            mut self,
            result: Result<Option<Location>, CodeIntelligenceError>,
        ) -> Self {
            self.definition_result = Arc::new(result);
            self
        }

        fn with_hover(mut self, result: Result<Option<HoverInfo>, CodeIntelligenceError>) -> Self {
            self.hover_result = Arc::new(result);
            self
        }

        fn with_references(
            mut self,
            result: Result<Vec<Reference>, CodeIntelligenceError>,
        ) -> Self {
            self.references_result = Arc::new(result);
            self
        }

        fn with_document_symbols(
            mut self,
            result: Result<Vec<DocumentSymbol>, CodeIntelligenceError>,
        ) -> Self {
            self.document_symbols_result = Arc::new(result);
            self
        }
    }

    #[async_trait::async_trait]
    impl CodeIntelligenceProvider for MockLspProvider {
        async fn get_symbols(
            &self,
            _path: &std::path::Path,
        ) -> Result<Vec<Symbol>, CodeIntelligenceError> {
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

        async fn get_hierarchy(
            &self,
            _location: &Location,
        ) -> Result<crate::domain::traits::code_intelligence::TypeHierarchy, CodeIntelligenceError>
        {
            Err(CodeIntelligenceError::Internal(
                "Not implemented".to_string(),
            ))
        }

        async fn get_definition(
            &self,
            _location: &Location,
        ) -> Result<Option<Location>, CodeIntelligenceError> {
            match self.definition_result.as_ref() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(CodeIntelligenceError::Internal(e.to_string())),
            }
        }

        async fn get_document_symbols(
            &self,
            _path: &std::path::Path,
        ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
            match self.document_symbols_result.as_ref() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(CodeIntelligenceError::Internal(e.to_string())),
            }
        }

        async fn hover(
            &self,
            _location: &Location,
        ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
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

    fn create_test_source_range(
        file: &str,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> SourceRange {
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
        let provider =
            Arc::new(MockLspProvider::new().with_definition(Ok(Some(def_location.clone()))));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
        let provider = Arc::new(MockLspProvider::new().with_definition(Err(
            CodeIntelligenceError::LspError("Server unavailable".to_string()),
        )));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
        let provider = Arc::new(MockLspProvider::new().with_hover(Err(
            CodeIntelligenceError::LspError("Server unavailable".to_string()),
        )));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
        let provider = Arc::new(MockLspProvider::new().with_references(Err(
            CodeIntelligenceError::LspError("Server unavailable".to_string()),
        )));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
                children: vec![DocumentSymbol {
                    symbol: Symbol::new("field1", SymbolKind::Field, loc2.clone()),
                    document_kind: DocumentSymbolKind::Field,
                    range: SourceRange::new(loc2.clone(), loc2.clone()),
                    children: vec![],
                }],
            },
        ];
        let provider = Arc::new(MockLspProvider::new().with_document_symbols(Ok(symbols)));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

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
                symbol: Symbol::new(
                    &format!("function_{}", i),
                    SymbolKind::Function,
                    loc.clone(),
                ),
                document_kind: DocumentSymbolKind::Function,
                range: SourceRange::new(loc.clone(), Location::new("large.rs", (i + 1) * 5, 0)),
                children: vec![],
            });
        }
        let provider = Arc::new(MockLspProvider::new().with_document_symbols(Ok(symbols)));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

        let input = GetDocumentSymbolsInput {
            file_path: "large.rs".to_string(),
        };

        let result = handle_get_document_symbols(&ctx, input).await.unwrap();
        assert_eq!(result.total, 100);
        assert_eq!(result.symbols.len(), 100);
    }

    #[tokio::test]
    async fn test_handle_get_document_symbols_error() {
        let provider = Arc::new(MockLspProvider::new().with_document_symbols(Err(
            CodeIntelligenceError::LspError("Server unavailable".to_string()),
        )));

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_code_intelligence_arc(provider)
            .build();

        let input = GetDocumentSymbolsInput {
            file_path: "test.rs".to_string(),
        };

        let result = handle_get_document_symbols(&ctx, input).await.unwrap();
        // Error returns empty results but still succeeds
        assert_eq!(result.total, 0);
        assert!(result.symbols.is_empty());
    }
    // ========================================================================
    // path confinement
    // ========================================================================

    fn outside_file() -> tempfile::NamedTempFile {
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), "secret line\n").unwrap();
        outside
    }

    fn confined_ctx(workspace: &tempfile::TempDir, provider: MockLspProvider) -> HandlerContext {
        HandlerContext::builder()
            .with_working_dir(workspace.path().to_path_buf())
            .with_code_intelligence_arc(Arc::new(provider))
            .build()
    }

    #[tokio::test]
    async fn hover_rejects_a_file_outside_the_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = outside_file();
        let ctx = confined_ctx(&workspace, MockLspProvider::new());
        let input = HoverInput {
            file_path: outside.path().to_string_lossy().into_owned(),
            line: 1,
            column: 1,
        };
        let result = handle_hover(&ctx, input).await;
        assert!(
            matches!(result, Err(HandlerError::Security(_))),
            "a path outside the workspace must be refused: {result:?}"
        );
    }

    #[tokio::test]
    async fn go_to_definition_rejects_a_file_outside_the_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = outside_file();
        let ctx = confined_ctx(&workspace, MockLspProvider::new());
        let input = GoToDefinitionInput {
            file_path: outside.path().to_string_lossy().into_owned(),
            line: 1,
            column: 1,
        };
        let result = handle_go_to_definition(&ctx, input).await;
        assert!(
            matches!(result, Err(HandlerError::Security(_))),
            "a path outside the workspace must be refused: {result:?}"
        );
    }

    #[tokio::test]
    async fn find_references_rejects_a_file_outside_the_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = outside_file();
        let ctx = confined_ctx(&workspace, MockLspProvider::new());
        let input = FindReferencesInput {
            file_path: outside.path().to_string_lossy().into_owned(),
            line: 1,
            column: 1,
            include_declaration: true,
        };
        let result = handle_find_references(&ctx, input).await;
        assert!(
            matches!(result, Err(HandlerError::Security(_))),
            "a path outside the workspace must be refused: {result:?}"
        );
    }

    #[tokio::test]
    async fn a_definition_outside_the_workspace_is_not_read() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("a.rs"), "fn a() {}\n").unwrap();
        let outside = outside_file();
        let target = Location::new(outside.path().to_string_lossy().into_owned(), 1, 1);
        let ctx = confined_ctx(
            &workspace,
            MockLspProvider::new().with_definition(Ok(Some(target))),
        );
        let input = GoToDefinitionInput {
            file_path: "a.rs".to_string(),
            line: 1,
            column: 4,
        };
        let out = handle_go_to_definition(&ctx, input).await.unwrap();
        assert!(
            out.context.is_none(),
            "the source of a file outside the workspace must not be returned: {:?}",
            out.context
        );
    }

    #[test]
    fn references_served_by_the_language_server_report_provider_lsp() {
        use crate::domain::traits::code_intelligence::{PrecisionTier, ReferenceKind};
        let refs = vec![Reference {
            location: Location::new("a.cs", 3, 5),
            reference_kind: ReferenceKind::Read,
            container: None,
        }];
        let out = references_output(
            "a.cs".to_string(),
            TieredOutcome::served(refs.clone(), PrecisionTier::S2),
        );
        assert_eq!(out.provider, "lsp");
        assert_eq!(out.total, 1);
        let heuristic = references_output(
            "a.cs".to_string(),
            TieredOutcome::served(refs, PrecisionTier::S0),
        );
        assert_eq!(heuristic.provider, "tree-sitter");
    }

    #[test]
    fn unresolved_references_do_not_claim_a_serving_provider() {
        let out = references_output("a.cs".to_string(), TieredOutcome::unresolved(Vec::new()));
        assert_eq!(out.total, 0);
        assert_ne!(out.provider, "lsp");
    }

    // ========================================================================
    // handle_rename_preview tests
    // ========================================================================

    use crate::domain::traits::code_intelligence::{PrepareRename, RenameEdits, TextEdit};

    /// Provider that only knows rename; every other operation is unused.
    struct RenameMock {
        prepare: Result<Option<PrepareRename>, String>,
        rename: Result<Option<RenameEdits>, String>,
        rename_calls: std::sync::atomic::AtomicUsize,
    }

    impl RenameMock {
        fn new(
            prepare: Result<Option<PrepareRename>, String>,
            rename: Result<Option<RenameEdits>, String>,
        ) -> Self {
            Self {
                prepare,
                rename,
                rename_calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl CodeIntelligenceProvider for RenameMock {
        async fn get_symbols(
            &self,
            _path: &std::path::Path,
        ) -> Result<Vec<Symbol>, CodeIntelligenceError> {
            Ok(vec![])
        }
        async fn find_references(
            &self,
            _location: &Location,
            _include_declaration: bool,
        ) -> Result<Vec<Reference>, CodeIntelligenceError> {
            Ok(vec![])
        }
        async fn get_hierarchy(
            &self,
            _location: &Location,
        ) -> Result<crate::domain::traits::code_intelligence::TypeHierarchy, CodeIntelligenceError>
        {
            Err(CodeIntelligenceError::Internal("unused".to_string()))
        }
        async fn get_definition(
            &self,
            _location: &Location,
        ) -> Result<Option<Location>, CodeIntelligenceError> {
            Ok(None)
        }
        async fn get_document_symbols(
            &self,
            _path: &std::path::Path,
        ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
            Ok(vec![])
        }
        async fn hover(
            &self,
            _location: &Location,
        ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
            Ok(None)
        }
        async fn prepare_rename(
            &self,
            _location: &Location,
        ) -> Result<Option<PrepareRename>, CodeIntelligenceError> {
            self.prepare
                .clone()
                .map_err(CodeIntelligenceError::Internal)
        }
        async fn rename(
            &self,
            _location: &Location,
            _new_name: &str,
        ) -> Result<Option<RenameEdits>, CodeIntelligenceError> {
            self.rename_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.rename.clone().map_err(CodeIntelligenceError::Internal)
        }
    }

    fn renamable() -> Result<Option<PrepareRename>, String> {
        Ok(Some(PrepareRename {
            range: Some(create_test_source_range("a.rs", 1, 4, 1, 7)),
            placeholder: Some("foo".to_string()),
        }))
    }

    fn one_edit(skipped: usize) -> Result<Option<RenameEdits>, String> {
        Ok(Some(RenameEdits {
            edits: vec![TextEdit {
                range: create_test_source_range("a.rs", 1, 4, 1, 7),
                new_text: "bar".to_string(),
            }],
            skipped_resource_operations: skipped,
        }))
    }

    fn rename_input(file: &str, new_name: &str) -> RenamePreviewInput {
        RenamePreviewInput {
            file_path: file.to_string(),
            line: 1,
            column: 4,
            new_name: new_name.to_string(),
        }
    }

    #[tokio::test]
    async fn rename_preview_returns_the_edits_and_never_writes_them() {
        let workspace = tempfile::tempdir().unwrap();
        let file = workspace.path().join("a.rs");
        std::fs::write(&file, "fn foo() {}\n").unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(workspace.path().to_path_buf())
            .with_code_intelligence_arc(Arc::new(RenameMock::new(renamable(), one_edit(0))))
            .build();

        let out = handle_rename_preview(&ctx, rename_input("a.rs", "bar"))
            .await
            .unwrap();

        assert!(out.renamable);
        assert_eq!(out.placeholder.as_deref(), Some("foo"));
        assert_eq!(out.total_edits, 1);
        assert_eq!(out.edits[0].new_text, "bar");
        assert_eq!(out.edits[0].file, "a.rs");
        assert_eq!((out.edits[0].start_line, out.edits[0].start_column), (1, 4));
        assert_eq!((out.edits[0].end_line, out.edits[0].end_column), (1, 7));
        assert!(!out.partial);
        assert_eq!(out.provider, "lsp");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "fn foo() {}\n",
            "a preview must not touch the file"
        );
    }

    #[tokio::test]
    async fn rename_preview_flags_a_partial_result() {
        let workspace = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(workspace.path().to_path_buf())
            .with_code_intelligence_arc(Arc::new(RenameMock::new(renamable(), one_edit(2))))
            .build();

        let out = handle_rename_preview(&ctx, rename_input("a.rs", "bar"))
            .await
            .unwrap();

        assert!(out.partial, "skipped resource operations make it partial");
        assert_eq!(out.skipped_resource_operations, 2);
    }

    #[tokio::test]
    async fn rename_preview_not_renamable_is_an_answer_and_skips_rename() {
        let workspace = tempfile::tempdir().unwrap();
        let mock = Arc::new(RenameMock::new(Ok(None), one_edit(0)));
        let ctx = HandlerContext::builder()
            .with_working_dir(workspace.path().to_path_buf())
            .with_code_intelligence_arc(mock.clone())
            .build();

        let out = handle_rename_preview(&ctx, rename_input("a.rs", "bar"))
            .await
            .unwrap();

        assert!(!out.renamable);
        assert!(out.edits.is_empty());
        assert_eq!(
            mock.rename_calls.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }

    #[tokio::test]
    async fn rename_preview_without_support_is_an_error_not_not_renamable() {
        let workspace = tempfile::tempdir().unwrap();
        // MockLspProvider keeps the port defaults: rename unsupported.
        let ctx = confined_ctx(&workspace, MockLspProvider::new());

        let result = handle_rename_preview(&ctx, rename_input("a.rs", "bar")).await;

        assert!(
            matches!(&result, Err(HandlerError::Internal(m)) if m.contains("not supported")),
            "an unsupported operation must not read as 'not renamable': {result:?}"
        );
    }

    #[tokio::test]
    async fn rename_preview_reports_a_failing_rename_after_a_positive_prepare() {
        let workspace = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(workspace.path().to_path_buf())
            .with_code_intelligence_arc(Arc::new(RenameMock::new(
                renamable(),
                Err("server crashed".to_string()),
            )))
            .build();

        let result = handle_rename_preview(&ctx, rename_input("a.rs", "bar")).await;

        assert!(
            matches!(&result, Err(HandlerError::Internal(m)) if m.contains("server crashed")),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn rename_preview_rejects_a_file_outside_the_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = outside_file();
        let ctx = HandlerContext::builder()
            .with_working_dir(workspace.path().to_path_buf())
            .with_code_intelligence_arc(Arc::new(RenameMock::new(renamable(), one_edit(0))))
            .build();

        let result =
            handle_rename_preview(&ctx, rename_input(&outside.path().to_string_lossy(), "bar"))
                .await;

        assert!(
            matches!(result, Err(HandlerError::Security(_))),
            "a path outside the workspace must be refused: {result:?}"
        );
    }

    #[tokio::test]
    async fn rename_preview_rejects_an_empty_new_name() {
        let workspace = tempfile::tempdir().unwrap();
        let ctx = confined_ctx(&workspace, MockLspProvider::new());

        let result = handle_rename_preview(&ctx, rename_input("a.rs", "  ")).await;

        assert!(
            matches!(result, Err(HandlerError::InvalidInput(_))),
            "{result:?}"
        );
    }
}
