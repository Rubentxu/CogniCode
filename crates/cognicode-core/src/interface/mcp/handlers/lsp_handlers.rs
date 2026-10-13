// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::needless_borrows_for_generic_args, dead_code)]
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

/// B3.W1: the tier-aware provider, when the caller configured one.
///
/// A bare `Arc<dyn CodeIntelligenceProvider>` cannot be downcast to
/// `dyn TieredCodeIntelligenceProvider`, and `get_definition` on
/// `CompositeProvider` erases the tier on the way out, so without this
/// the LSP handlers would have to hardcode the catalog ceiling and claim
/// `lsp` even when tree-sitter answered. When no tiered provider is
/// configured the handlers fall back to the floor and say so in
/// `provider` — honest degradation, not a fabricated upgrade.
fn get_tiered_provider(
    ctx: &HandlerContext,
) -> Option<Arc<dyn crate::domain::traits::code_intelligence::TieredCodeIntelligenceProvider>> {
    ctx.tiered_code_intelligence_provider.clone()
}

/// Resolve the `(precision, provider)` pair to publish for a navigation
/// response.
///
/// `tier` is the tier that actually served the request. `None` means the
/// tier is not observable, in which case the published value is the
/// catalog floor and the provider is suffixed so a reader can tell the
/// difference between "this was LSP-grade" and "we could not verify it".
fn report_precision(
    tool: &str,
    tier: Option<crate::domain::traits::code_intelligence::PrecisionTier>,
) -> (crate::interface::symbol_identity::Precision, String) {
    let base = crate::interface::symbol_identity::default_provider_for_tool(tool);
    match tier {
        Some(t) => (
            crate::interface::symbol_identity::precision_from_tier(t),
            format!("{base} (tier {})", t.as_str()),
        ),
        None => (
            crate::interface::symbol_identity::precision_floor_for_tool(tool),
            format!("{base} (tier unverified)"),
        ),
    }
}

/// Resolve a definition through the tiered pipeline when one is
/// configured, so the response can report the tier that answered.
///
/// Returns `(value, tier)`. `tier` is `None` on the fallback path: the
/// plain `get_definition` erases it, and inventing an `S2` there would
/// be exactly the false LSP claim AAT-B3-01 forbids.
async fn definition_with_tier(
    ctx: &HandlerContext,
    location: &crate::domain::value_objects::Location,
) -> (
    Result<
        Option<crate::domain::value_objects::Location>,
        crate::domain::traits::code_intelligence::CodeIntelligenceError,
    >,
    Option<crate::domain::traits::code_intelligence::PrecisionTier>,
) {
    if let Some(tiered) = get_tiered_provider(ctx) {
        use crate::domain::traits::code_intelligence::TieredOutcome;
        return match tiered.get_definition_tiered(location).await {
            TieredOutcome::Served(answer) => (Ok(answer.value), Some(answer.tier)),
            TieredOutcome::Unresolved(_) => (Ok(None), None),
        };
    }
    (get_provider(ctx).get_definition(location).await, None)
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
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    let (definition, tier) = definition_with_tier(ctx, &location).await;

    match definition {
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
            let (precision, provider) = report_precision("go_to_definition", tier);
            Ok(GoToDefinitionOutput {
                found: true,
                file: Some(def_loc.file().to_string()),
                line: Some(def_loc.line()),
                column: Some(def_loc.column()),
                context,
                message: None,
                precision,
                provider,
            })
        }
        Ok(None) => {
            let (precision, provider) = report_precision("go_to_definition", tier);
            Ok(GoToDefinitionOutput {
                found: false,
                file: None,
                line: None,
                column: None,
                context: None,
                message: Some("No definition found at this position".to_string()),
                precision,
                provider,
            })
        }
        Err(e) => {
            let (precision, provider) = report_precision("go_to_definition", tier);
            Ok(GoToDefinitionOutput {
                found: false,
                file: None,
                line: None,
                column: None,
                context: None,
                message: Some(e.to_string()),
                precision,
                provider,
            })
        }
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
/// Resolve references through the tiered pipeline when one is
/// configured, so the response can report the tier that answered.
///
/// Same rule as [`definition_with_tier`]: a missing tiered provider
/// yields `tier: None`, never an assumed `S2`.
async fn references_with_tier(
    ctx: &HandlerContext,
    location: &crate::domain::value_objects::Location,
    include_declaration: bool,
) -> (
    Result<
        Vec<crate::domain::traits::code_intelligence::Reference>,
        crate::domain::traits::code_intelligence::CodeIntelligenceError,
    >,
    Option<crate::domain::traits::code_intelligence::PrecisionTier>,
) {
    if let Some(tiered) = get_tiered_provider(ctx) {
        use crate::domain::traits::code_intelligence::TieredOutcome;
        return match tiered
            .find_references_tiered(location, include_declaration)
            .await
        {
            TieredOutcome::Served(answer) => (Ok(answer.value), Some(answer.tier)),
            // `TieredOutcome::Unresolved` documents that "callers must not
            // fabricate a value". This returned `Ok(Vec::new())`, which made
            // a symbol the provider could not resolve look identical to a
            // symbol with no references: `{"references":[],"total":0}` with
            // the provider string still saying "tier unverified". A caller
            // could not tell "not checked" from "nothing there".
            //
            // Measured against the release server: a position with no
            // identifier at it now returns an error naming both attempted
            // tiers, instead of a confident total:0. A position that DOES
            // resolve returns its references (verified at plan.rs:343:8,
            // which has 20 same-file occurrences and reports them).
            //
            // The failure is now reported as a failure.
            TieredOutcome::Unresolved(diagnostics) => {
                let detail = diagnostics
                    .iter()
                    .map(|d| format!("{:?}", d))
                    .collect::<Vec<_>>()
                    .join("; ");
                (
                    Err(crate::domain::traits::code_intelligence::CodeIntelligenceError::Internal(
                        format!(
                            "find_references could not resolve the symbol at {}:{}:{}. \
                             No tier served a reference list, so no answer is reported. \
                             Diagnostics: {}",
                            location.file().to_string(),
                            location.line(),
                            location.column(),
                            if detail.is_empty() { "none" } else { &detail }
                        ),
                    )),
                    None,
                )
            }
        };
    }
    (
        get_provider(ctx)
            .find_references(location, include_declaration)
            .await,
        None,
    )
}

#[cognicode_macros::aix_tool(
    name = "find_references",
    description = "Find all references to a symbol using LSP.",
    input_schema = FindReferencesInput
)]
pub async fn handle_find_references(
    ctx: &HandlerContext,
    input: FindReferencesInput,
) -> HandlerResult<FindReferencesOutput> {
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    let (refs, tier) = references_with_tier(ctx, &location, input.include_declaration).await;

    // The input is a position, not a symbol name: `FindReferencesInput` has
    // only `file_path`, `line` and `column`. The output field is still called
    // `symbol`, and it was filled with `input.file_path`, so the response read
    // `{"symbol":"crates/.../plan.rs"}` - a path presented as a symbol name.
    // Report the position that was actually searched, which is what the caller
    // can act on and cannot be mistaken for a resolved symbol name.
    let searched_at = format!(
        "{}:{}:{}",
        input.file_path, input.line, input.column
    );

    match refs {
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
            let (precision, provider) = report_precision("find_references", tier);
            Ok(FindReferencesOutput {
                symbol: searched_at,
                references: entries,
                total,
                precision,
                provider,
            })
        }
        Err(e) => {
            // Previously this arm returned `total: 0` with an empty list, so
            // a failed lookup was indistinguishable from a completed one that
            // found nothing. The failure is now reported as a failure.
            Err(HandlerError::App(crate::application::error::AppError::AnalysisError(
                format!("find_references: {}", e),
            )))
        }
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
}
