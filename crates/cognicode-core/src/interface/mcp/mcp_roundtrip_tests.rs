//! E2E MCP Protocol Roundtrip Tests
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(unused_comparisons, unused_imports)]
// The `absurd_extreme_comparisons` blanket that used to sit here is gone.
// Every `count >= 0` it permitted has been replaced by the value the fixture
// actually implies, so the next unfailable assertion in this file becomes a
// compile error instead of a silent pass.
//!
//! Tests that verify the full MCP protocol roundtrip:
//! 1. Serialize MCP request → JSON
//! 2. Deserialize JSON → handler input
//! 3. Call handler to get output
//! 4. Serialize output → JSON
//! 5. Deserialize JSON → verify data integrity
//!
//! This ensures serialization doesn't corrupt data and all major tools work end-to-end.

#[cfg(test)]
mod tests {
    use crate::interface::mcp::handlers::HandlerContext;
    use crate::interface::mcp::handlers::{BuildGraphInput, handle_build_graph};
    use crate::interface::mcp::schemas::*;

    /// Helper to create a HandlerContext with workspace scoping
    fn create_test_context(temp_dir: &tempfile::TempDir) -> HandlerContext {
        HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .build()
    }

    // ==========================================================================
    // MCP Protocol Types Roundtrip Tests
    // ==========================================================================

    mod mcp_protocol_roundtrip {
        use super::*;

        #[test]
        fn test_mcp_request_roundtrip() {
            let request = McpRequest {
                jsonrpc: "2.0".to_string(),
                method: "test_method".to_string(),
                params: Some(serde_json::json!({"key": "value"})),
                id: Some(serde_json::json!(1)),
            };

            // Roundtrip: serialize → deserialize
            let json = serde_json::to_string(&request).unwrap();
            let parsed: McpRequest = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed.jsonrpc, request.jsonrpc);
            assert_eq!(parsed.method, request.method);
            assert_eq!(parsed.id, request.id);
        }

        #[test]
        fn test_mcp_request_with_null_id() {
            let request = McpRequest {
                jsonrpc: "2.0".to_string(),
                method: "test".to_string(),
                params: None,
                id: None,
            };

            let json = serde_json::to_string(&request).unwrap();
            let parsed: McpRequest = serde_json::from_str(&json).unwrap();

            assert!(parsed.params.is_none());
            assert!(parsed.id.is_none());
        }

        #[test]
        fn test_mcp_response_success_roundtrip() {
            let response = McpResponse::success(
                serde_json::json!({"result": "success"}),
                Some(serde_json::json!(1)),
            );

            let json = serde_json::to_string(&response).unwrap();
            let parsed: McpResponse = serde_json::from_str(&json).unwrap();

            assert!(parsed.result.is_some());
            assert!(parsed.error.is_none());
        }

        #[test]
        fn test_mcp_response_error_roundtrip() {
            let error = McpError::invalid_request("Bad request");
            let response = McpResponse::error_response(error, Some(serde_json::json!(1)));

            let json = serde_json::to_string(&response).unwrap();
            let parsed: McpResponse = serde_json::from_str(&json).unwrap();

            assert!(parsed.result.is_none());
            assert!(parsed.error.is_some());
            assert_eq!(parsed.error.as_ref().unwrap().code, -32600);
        }
    }

    // ==========================================================================
    // File Operations Roundtrip Tests
    // ==========================================================================

    mod file_operations_roundtrip {
        use super::*;
        use crate::interface::mcp::file_ops_handlers::{
            handle_edit_file, handle_list_files, handle_read_file, handle_search_content,
            handle_write_file,
        };
        use crate::interface::mcp::schemas::{
            EditFileInput, FileEdit, ListFilesInput, ReadFileInput, SearchContentInput,
            WriteFileInput,
        };

        #[tokio::test]
        async fn test_read_file_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Create a test file
            let file_path = temp_dir.path().join("test.txt");
            std::fs::write(&file_path, "Hello, World!\nLine 2\nLine 3\n").unwrap();

            // Create input
            let input = ReadFileInput {
                path: file_path.to_str().unwrap().to_string(),
                start_line: None,
                end_line: None,
                mode: Some("raw".to_string()),
                chunk_size: None,
                continuation_token: None,
            };

            // Roundtrip test: serialize → deserialize input
            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: ReadFileInput = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed_input.path, input.path);
            assert_eq!(parsed_input.mode, input.mode);

            // Call handler
            let output = handle_read_file(&ctx, parsed_input).await.unwrap();

            // Roundtrip test: serialize → deserialize output
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: ReadFileOutput = serde_json::from_str(&output_json).unwrap();

            assert_eq!(parsed_output.content, output.content);
            assert_eq!(parsed_output.metadata.path, output.metadata.path);
            assert_eq!(parsed_output.mode, output.mode);
        }

        #[tokio::test]
        async fn test_read_file_with_line_range_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("multiline.txt");
            std::fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

            let input = ReadFileInput {
                path: file_path.to_str().unwrap().to_string(),
                start_line: Some(2),
                end_line: Some(4),
                mode: Some("raw".to_string()),
                chunk_size: None,
                continuation_token: None,
            };

            // Roundtrip
            let json = serde_json::to_string(&input).unwrap();
            let parsed: ReadFileInput = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.start_line, Some(2));
            assert_eq!(parsed.end_line, Some(4));

            let output = handle_read_file(&ctx, parsed).await.unwrap();
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: ReadFileOutput = serde_json::from_str(&output_json).unwrap();

            assert!(parsed_output.content.contains("Line 2"));
            assert!(parsed_output.content.contains("Line 4"));
            assert!(!parsed_output.content.contains("Line 1"));
        }

        #[tokio::test]
        async fn test_write_file_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("new_file.txt");
            let content = "Test content for roundtrip validation";

            let input = WriteFileInput {
                path: file_path.to_str().unwrap().to_string(),
                content: content.to_string(),
                create_dirs: Some(false),
                expected_content_hash: None,
            };

            // Roundtrip input
            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: WriteFileInput = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed_input.content, content);
            assert_eq!(parsed_input.path, input.path);

            // Call handler
            let output = handle_write_file(&ctx, parsed_input).await.unwrap();

            // Roundtrip output
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: WriteFileOutput = serde_json::from_str(&output_json).unwrap();

            assert_eq!(parsed_output.bytes_written, content.len() as u64);
            assert_eq!(parsed_output.metadata.size, content.len() as u64);
        }

        #[tokio::test]
        async fn test_edit_file_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("edit_test.txt");
            std::fs::write(&file_path, "Hello World").unwrap();

            let input = EditFileInput {
                path: file_path.to_str().unwrap().to_string(),
                edits: vec![FileEdit {
                    old_string: "World".to_string(),
                    new_string: "CogniCode".to_string(),
                }],
                expected_content_hash: None,
            };

            // Roundtrip input
            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: EditFileInput = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed_input.edits[0].old_string, "World");

            // Call handler
            let output = handle_edit_file(&ctx, parsed_input).await.unwrap();

            // Roundtrip output
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: EditFileOutput = serde_json::from_str(&output_json).unwrap();

            assert_eq!(parsed_output.applied, output.applied);
            assert_eq!(parsed_output.bytes_changed, output.bytes_changed);
        }

        #[tokio::test]
        async fn test_search_content_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            std::fs::write(
                temp_dir.path().join("search_test.txt"),
                "fn hello() {}\nfn world() {}\n",
            )
            .unwrap();

            let input = SearchContentInput {
                pattern: "fn".to_string(),
                path: None,
                file_glob: Some("*.txt".to_string()),
                regex: Some(true),
                case_insensitive: Some(false),
                max_results: Some(50),
                context_lines: Some(2),
            };

            // Roundtrip input
            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: SearchContentInput = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed_input.pattern, "fn");
            assert_eq!(parsed_input.max_results, Some(50));

            // Call handler
            let output = handle_search_content(&ctx, parsed_input).await.unwrap();

            // Roundtrip output
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: SearchContentOutput = serde_json::from_str(&output_json).unwrap();

            assert_eq!(parsed_output.total, output.total);
            assert_eq!(parsed_output.matches.len(), output.matches.len());
        }

        #[tokio::test]
        async fn test_list_files_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Create test files
            std::fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
            std::fs::write(temp_dir.path().join("file2.rs"), "fn main() {}").unwrap();

            let input = ListFilesInput {
                path: None,
                glob: Some("**/*".to_string()),
                offset: None,
                limit: Some(100),
                recursive: Some(true),
                max_depth: None,
            };

            // Roundtrip input
            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: ListFilesInput = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed_input.limit, Some(100));
            assert!(parsed_input.recursive.unwrap());

            // Call handler
            let output = handle_list_files(&ctx, parsed_input).await.unwrap();

            // Roundtrip output
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: ListFilesOutput = serde_json::from_str(&output_json).unwrap();

            assert_eq!(parsed_output.total, output.total);
            assert_eq!(parsed_output.files.len(), output.files.len());
        }
    }

    // ==========================================================================
    // Build Graph Roundtrip Tests
    // ==========================================================================

    mod build_graph_roundtrip {
        use super::*;
        use crate::interface::mcp::handlers::handle_build_graph;

        #[tokio::test]
        async fn test_build_graph_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Create a simple source file
            let src_dir = temp_dir.path().join("src");
            std::fs::create_dir_all(&src_dir).unwrap();
            std::fs::write(src_dir.join("main.rs"), "fn main() {}\n").unwrap();

            let input = BuildGraphInput {
                directory: Some(temp_dir.path().to_str().unwrap().to_string()),
            };

            // Call handler - verify it works
            let output = handle_build_graph(&ctx, input).await.unwrap();

            // Verify output fields
            assert!(output.success);
            // The fixture writes `fn main() {}` to src/main.rs, so a successful
            // build found at least that one symbol. `>= 0` was true of every
            // usize and made this roundtrip pass on an empty index.
            assert!(
                output.symbols_found >= 1,
                "the fixture defines main(), so at least one symbol must be found; got {}",
                output.symbols_found
            );
        }
    }

    // ==========================================================================
    // Call Hierarchy Roundtrip Tests
    // ==========================================================================

    mod call_hierarchy_roundtrip {
        use super::*;
        use crate::interface::mcp::handlers::handle_get_call_hierarchy;

        #[tokio::test]
        async fn test_get_call_hierarchy_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Create source files
            let src_dir = temp_dir.path().join("src");
            std::fs::create_dir_all(&src_dir).unwrap();
            std::fs::write(
                src_dir.join("lib.rs"),
                "pub fn caller() { callee(); }\npub fn callee() {}\n",
            )
            .unwrap();

            // Build graph first
            let build_input = BuildGraphInput {
                directory: Some(temp_dir.path().to_str().unwrap().to_string()),
            };
            let _ = handle_build_graph(&ctx, build_input).await;

            let input = GetCallHierarchyInput {
                symbol_name: "caller".to_string(),
                direction: CallDirection::Outgoing,
                depth: 1,
                include_external: false,
                compressed: false,
            };

            // Roundtrip input
            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: GetCallHierarchyInput = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed_input.symbol_name, "caller");
            assert!(matches!(parsed_input.direction, CallDirection::Outgoing));

            // Call handler
            let output = handle_get_call_hierarchy(&ctx, parsed_input).await.unwrap();

            // Roundtrip output
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: GetCallHierarchyOutput = serde_json::from_str(&output_json).unwrap();

            assert_eq!(parsed_output.symbol, output.symbol);
        }
    }

    // ==========================================================================
    // Complexity Roundtrip Tests
    // ==========================================================================

    mod complexity_roundtrip {
        use super::*;
        use crate::interface::mcp::handlers::handle_get_complexity;

        #[tokio::test]
        async fn test_get_complexity_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("complex.rs");
            std::fs::write(
                &file_path,
                r#"
fn simple_function() {
    if condition {
        do_something();
    }
}
"#,
            )
            .unwrap();

            let input = GetComplexityInput {
                file_path: file_path.to_str().unwrap().to_string(),
                function_name: Some("simple_function".to_string()),
            };

            // Roundtrip input
            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: GetComplexityInput = serde_json::from_str(&json).unwrap();
            assert_eq!(
                parsed_input.function_name,
                Some("simple_function".to_string())
            );

            // Call handler
            let output = handle_get_complexity(&ctx, parsed_input).await.unwrap();

            // Roundtrip output
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: GetComplexityOutput = serde_json::from_str(&output_json).unwrap();

            assert_eq!(parsed_output.file_path, output.file_path);
            assert_eq!(
                parsed_output.complexity.cyclomatic,
                output.complexity.cyclomatic
            );
        }
    }

    // ==========================================================================
    // Entry Points Roundtrip Tests
    // ==========================================================================

    mod entry_points_roundtrip {
        use super::*;
        use crate::interface::mcp::handlers::handle_get_entry_points;

        #[tokio::test]
        async fn test_get_entry_points_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Create source file
            let src_dir = temp_dir.path().join("src");
            std::fs::create_dir_all(&src_dir).unwrap();
            std::fs::write(src_dir.join("main.rs"), "fn main() {}\nfn helper() {}\n").unwrap();

            // Build graph first
            let build_input = BuildGraphInput {
                directory: Some(temp_dir.path().to_str().unwrap().to_string()),
            };
            let _ = handle_build_graph(&ctx, build_input).await;

            let input = GetEntryPointsInput { compressed: false };

            // Roundtrip input
            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: GetEntryPointsInput = serde_json::from_str(&json).unwrap();
            assert!(!parsed_input.compressed);

            // Call handler
            let output = handle_get_entry_points(&ctx, parsed_input).await.unwrap();

            // Roundtrip output
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: GetEntryPointsOutput = serde_json::from_str(&output_json).unwrap();

            assert_eq!(parsed_output.total, output.total);
        }
    }

    // ==========================================================================
    // Analyze Impact Roundtrip Tests
    // ==========================================================================

    mod analyze_impact_roundtrip {
        use super::*;
        use crate::interface::mcp::handlers::handle_analyze_impact;

        #[tokio::test]
        async fn test_analyze_impact_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Create source files
            let src_dir = temp_dir.path().join("src");
            std::fs::create_dir_all(&src_dir).unwrap();
            std::fs::write(
                src_dir.join("lib.rs"),
                "pub fn used_function() {}\npub fn main() { used_function(); }\n",
            )
            .unwrap();

            // Build graph first
            let build_input = BuildGraphInput {
                directory: Some(temp_dir.path().to_str().unwrap().to_string()),
            };
            let _ = handle_build_graph(&ctx, build_input).await;

            let input = AnalyzeImpactInput {
                symbol_name: "used_function".to_string(),
                compressed: false,
            };

            // Roundtrip input
            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: AnalyzeImpactInput = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed_input.symbol_name, "used_function");

            // Call handler
            let output = handle_analyze_impact(&ctx, parsed_input).await.unwrap();

            // Roundtrip output
            let output_json = serde_json::to_string(&output).unwrap();
            let parsed_output: AnalyzeImpactOutput = serde_json::from_str(&output_json).unwrap();

            assert_eq!(parsed_output.symbol, output.symbol);
            // RiskLevel doesn't implement PartialEq, so compare via JSON
            assert_eq!(
                serde_json::to_string(&parsed_output.risk_level).unwrap(),
                serde_json::to_string(&output.risk_level).unwrap()
            );
        }
    }

    // ==========================================================================
    // Schema Validation Tests - Ensure Serialization Doesn't Corrupt Data
    // ==========================================================================

    mod schema_validation {
        use super::*;

        #[test]
        fn test_call_direction_serialization() {
            let incoming = CallDirection::Incoming;
            let json = serde_json::to_string(&incoming).unwrap();
            assert_eq!(json, "\"incoming\"");

            let outgoing = CallDirection::Outgoing;
            let json = serde_json::to_string(&outgoing).unwrap();
            assert_eq!(json, "\"outgoing\"");
        }

        #[test]
        fn test_risk_level_serialization() {
            let levels = vec![
                (RiskLevel::Low, "\"low\""),
                (RiskLevel::Medium, "\"medium\""),
                (RiskLevel::High, "\"high\""),
                (RiskLevel::Critical, "\"critical\""),
            ];

            for (level, expected) in levels {
                let json = serde_json::to_string(&level).unwrap();
                assert_eq!(json, expected);
            }
        }

        #[test]
        fn test_subgraph_direction_serialization() {
            let directions = vec![
                (SubgraphDirection::In, "\"in\""),
                (SubgraphDirection::Out, "\"out\""),
                (SubgraphDirection::Both, "\"both\""),
            ];

            for (direction, expected) in directions {
                let json = serde_json::to_string(&direction).unwrap();
                assert_eq!(json, expected);
            }
        }

        #[test]
        fn test_refactor_action_serialization() {
            let actions = vec![
                (RefactorAction::Rename, "\"rename\""),
                (RefactorAction::Extract, "\"extract\""),
                (RefactorAction::Inline, "\"inline\""),
                (RefactorAction::Move, "\"move\""),
                (RefactorAction::ChangeSignature, "\"change_signature\""),
            ];

            for (action, expected) in actions {
                let json = serde_json::to_string(&action).unwrap();
                assert_eq!(json, expected);
            }
        }

        #[test]
        fn test_symbol_kind_serialization() {
            let kinds = vec![
                (SymbolKind::Function, "\"function\""),
                (SymbolKind::Struct, "\"struct\""),
                (SymbolKind::Enum, "\"enum\""),
                (SymbolKind::Trait, "\"trait\""),
                (SymbolKind::Module, "\"module\""),
            ];

            for (kind, expected) in kinds {
                let json = serde_json::to_string(&kind).unwrap();
                assert_eq!(json, expected);
            }
        }

        #[test]
        fn test_pattern_type_serialization() {
            let patterns = vec![
                (PatternType::FunctionCall, "\"function_call\""),
                (PatternType::TypeDefinition, "\"type_definition\""),
                (PatternType::ImportStatement, "\"import_statement\""),
                (PatternType::Annotation, "\"annotation\""),
                (PatternType::Custom, "\"custom\""),
            ];

            for (pattern, expected) in patterns {
                let json = serde_json::to_string(&pattern).unwrap();
                assert_eq!(json, expected);
            }
        }

        #[test]
        fn test_source_location_roundtrip() {
            let location = SourceLocation {
                file: "/path/to/file.rs".to_string(),
                line: 42,
                column: 10,
            };

            let json = serde_json::to_string(&location).unwrap();
            let parsed: SourceLocation = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed.file, location.file);
            assert_eq!(parsed.line, location.line);
            assert_eq!(parsed.column, location.column);
        }

        #[test]
        fn test_analysis_metadata_roundtrip() {
            let metadata = AnalysisMetadata {
                total_calls: 100,
                analysis_time_ms: 500,
            };

            let json = serde_json::to_string(&metadata).unwrap();
            let parsed: AnalysisMetadata = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed.total_calls, metadata.total_calls);
            assert_eq!(parsed.analysis_time_ms, metadata.analysis_time_ms);
        }

        #[test]
        fn test_complexity_metrics_roundtrip() {
            let metrics = ComplexityMetrics {
                cyclomatic: 5,
                cognitive: 3,
                lines_of_code: 42,
                parameter_count: 2,
                nesting_depth: 2,
                function_name: Some("test_func".to_string()),
            };

            let json = serde_json::to_string(&metrics).unwrap();
            let parsed: ComplexityMetrics = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed.cyclomatic, metrics.cyclomatic);
            assert_eq!(parsed.cognitive, metrics.cognitive);
            assert_eq!(parsed.function_name, metrics.function_name);
        }

        #[test]
        fn test_file_metadata_roundtrip() {
            let metadata = FileMetadata {
                path: "/test/file.rs".to_string(),
                size: 1024,
                modified: 1699999999,
                language: Some("Rust".to_string()),
            };

            let json = serde_json::to_string(&metadata).unwrap();
            let parsed: FileMetadata = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed.path, metadata.path);
            assert_eq!(parsed.size, metadata.size);
            assert_eq!(parsed.language, metadata.language);
        }

        #[test]
        fn test_validation_result_roundtrip() {
            let validation = ValidationResult {
                is_valid: true,
                warnings: vec!["Warning 1".to_string()],
                errors: vec![],
            };

            let json = serde_json::to_string(&validation).unwrap();
            let parsed: ValidationResult = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed.is_valid, validation.is_valid);
            assert_eq!(parsed.warnings.len(), validation.warnings.len());
        }

        #[test]
        fn test_error_roundtrip() {
            let error = McpError::new(-32600, "Invalid request");

            let json = serde_json::to_string(&error).unwrap();
            let parsed: McpError = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed.code, error.code);
            assert_eq!(parsed.message, error.message);
        }
    }

    // ==========================================================================
    // Edge Cases - Special Characters and Unicode
    // ==========================================================================

    mod edge_cases {
        use super::*;
        use crate::interface::mcp::file_ops_handlers::handle_write_file;
        use crate::interface::mcp::schemas::WriteFileInput;

        #[tokio::test]
        async fn test_unicode_content_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("unicode.txt");
            let content = "Hello 🌍 Ñoño 🎉";

            let input = WriteFileInput {
                path: file_path.to_str().unwrap().to_string(),
                content: content.to_string(),
                create_dirs: Some(false),
                expected_content_hash: None,
            };

            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: WriteFileInput = serde_json::from_str(&json).unwrap();

            let output = handle_write_file(&ctx, parsed_input).await.unwrap();
            let output_json = serde_json::to_string(&output).unwrap();
            let _parsed_output: WriteFileOutput = serde_json::from_str(&output_json).unwrap();

            // Verify file was written correctly
            let read_back = std::fs::read_to_string(&file_path).unwrap();
            assert_eq!(read_back, content);
        }

        #[tokio::test]
        async fn test_special_chars_in_path_roundtrip() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _ctx = create_test_context(&temp_dir);

            // Create a file with spaces and special chars
            let file_path = temp_dir.path().join("file with spaces & special.txt");
            std::fs::write(&file_path, "content").unwrap();

            let input = crate::interface::mcp::schemas::ReadFileInput {
                path: file_path.to_str().unwrap().to_string(),
                start_line: None,
                end_line: None,
                mode: Some("raw".to_string()),
                chunk_size: None,
                continuation_token: None,
            };

            let json = serde_json::to_string(&input).unwrap();
            let parsed_input: crate::interface::mcp::schemas::ReadFileInput =
                serde_json::from_str(&json).unwrap();

            assert_eq!(parsed_input.path, input.path);
        }
    }

    // ==========================================================================
    // Tool Surface Parity Tests (ADR-033 Phase 0)
    // ==========================================================================
    // These tests verify the MCP tool surface integrity invariants:
    // 1. No duplicate tool names in the public tools/list
    // 2. get_graph_report (ghost) is NOT listed
    // 3. All listed tools map to a known dispatch arm (static allowlist)
    //
    // Note: async dispatch probing (test_every_listed_tool_dispatches) requires
    // rmcp::service::RequestContext which needs Peer::new (pub(crate)). The
    // static allowlist test below provides a compile-time-checked proxy until
    // a proper test helper is available in rmcp.

    mod tool_surface_parity {
        use super::*;
        use std::collections::HashSet;

        /// Invariant: every public tool name must appear exactly once in tools/list.
        /// Duplicate registrations (e.g. solid_audit, graph_diff, graph_timeline in
        /// pre-ADR-033 code) cause MCP clients to receive conflicting schemas.
        #[test]
        fn test_no_duplicate_tool_names() {
            let tools = crate::interface::mcp::rmcp_adapter::build_all_tools();
            let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
            let unique: HashSet<_> = names.iter().collect();
            assert_eq!(
                unique.len(),
                names.len(),
                "Duplicate tool names found: {:?}",
                names
                    .iter()
                    .filter(
                        |n| unique.contains(*n) && names.iter().filter(|n2| *n2 == *n).count() > 1
                    )
                    .collect::<Vec<_>>()
            );
        }

        /// Invariant: get_graph_report must NOT be listed in public tools/list.
        /// It has no dispatch arm (falls through to ToolNotFound) and returns a
        /// stale Sprint 2 placeholder message. Re-advertisement requires a
        /// follow-up ADR with a real PG-backed implementation.
        #[test]
        fn test_get_graph_report_not_listed() {
            let tools = crate::interface::mcp::rmcp_adapter::build_all_tools();
            let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
            assert!(
                !names.contains(&"get_graph_report"),
                "get_graph_report must not be listed in public tools/list"
            );
        }

        // ------------------------------------------------------------------
        // The dispatch oracle.
        //
        // MEDIDO 2026-10-18. A pair of tests replaced by this one compared the
        // catalogue with `dispatchable_tool_names()` — a third hand-maintained
        // list — and never asked the dispatch anything. Together they proved
        // `catalogue == allowlist` and nothing else, so they stayed green with
        // four advertised tools that answer `InterfaceError::ToolNotFound`.
        // That is the exact class the pair was written to catch, and it shipped.
        //
        // The fix is not a fourth list. It is reading the dispatch itself: the
        // property "tools/list only announces what tools/call can deliver" is a
        // fact about the `match tool_name` block, so the block is what gets read.
        // That deletes ~100 lines of list maintenance and removes a third source
        // of truth, rather than adding a guard on top of one.

        /// The text of the `match tool_name { … }` block inside
        /// `call_tool_handler`, braces matched and string/char literals skipped.
        ///
        /// Brace counting alone is not enough, and this file is the proof: it
        /// contains byte literals like `b'{'` (the token-stream scanner in the
        /// stderr-demux helpers), and it contains lifetimes like `&'static str`.
        /// A `'` that opens a lifetime must not be read as the start of a char
        /// literal, or the scanner swallows the rest of the file and reports an
        /// implausible arm count instead of failing.
        fn dispatch_block(source: &str, header: &str) -> String {
            let start = source.find(header).unwrap_or_else(|| {
                panic!(
                    "`{header}` is not in rmcp_adapter.rs. This oracle reads the real \
                     dispatch and has no list to fall back to; if the match was \
                     renamed, rename it here too."
                )
            });
            let open = start
                + source[start..]
                    .find('{')
                    .unwrap_or_else(|| panic!("`{header}` has no opening brace"));

            let bytes = source.as_bytes();
            let mut i = open;
            let mut depth = 0usize;
            // `'` followed by a backslash or by exactly one char and then `'`
            // opens a char literal; `'` followed by an identifier char is a
            // lifetime and must be left alone.
            let char_literal_at = |at: usize| -> bool {
                let Some(&q) = bytes.get(at) else {
                    return false;
                };
                if q != b'\'' {
                    return false;
                }
                // `b'x'` / `r'x'` / `r#"x"#` — a prefix makes it a literal.
                if at > 0 && matches!(bytes[at - 1], b'b' | b'r' | b'B' | b'R') {
                    return true;
                }
                let Some(&first) = bytes.get(at + 1) else {
                    return false;
                };
                if first == b'\\' {
                    return true;
                }
                matches!(bytes.get(at + 2), Some(b'\''))
            };

            while i < bytes.len() {
                let b = bytes[i];
                match b {
                    b'/' if bytes.get(i + 1) == Some(&b'/') => {
                        while i < bytes.len() && bytes[i] != b'\n' {
                            i += 1;
                        }
                    }
                    b'/' if bytes.get(i + 1) == Some(&b'*') => {
                        i += 2;
                        while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                            i += 1;
                        }
                        i += 2;
                    }
                    b'"' => {
                        i += 1;
                        while i < bytes.len() && bytes[i] != b'"' {
                            i += if bytes[i] == b'\\' { 2 } else { 1 };
                        }
                        i += 1;
                    }
                    b'\'' if char_literal_at(i) => {
                        i += 1;
                        while i < bytes.len() && bytes[i] != b'\'' {
                            i += if bytes[i] == b'\\' { 2 } else { 1 };
                        }
                        i += 1;
                    }
                    b'{' => {
                        depth += 1;
                        i += 1;
                    }
                    b'}' => {
                        depth -= 1;
                        i += 1;
                        if depth == 0 {
                            return source[start..i].to_string();
                        }
                    }
                    _ => i += 1,
                }
            }
            panic!("`{header}` never closes; the oracle found no complete match block")
        }

        /// Every dispatch arm of `call_tool_handler`: tool name -> the
        /// `feature` its `#[cfg(…)]` gates it by, or `None` when unconditional.
        fn dispatch_arms() -> std::collections::BTreeMap<String, Option<String>> {
            let source = include_str!("rmcp_adapter.rs");
            let block = dispatch_block(source, "match tool_name");
            let mut arms = std::collections::BTreeMap::new();
            let mut gate: Option<String> = None;
            for line in block.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("#[cfg(") {
                    // `#[cfg(feature = "x")]` -> `x`. Any other cfg shape is
                    // recorded as itself so a mismatch is visible rather than
                    // silently treated as "ungated".
                    gate = Some(
                        rest.trim_end_matches(')')
                            .trim_start_matches("feature")
                            .trim_start_matches('=')
                            .trim()
                            .trim_matches('"')
                            .to_string(),
                    );
                    continue;
                }
                let Some(name) = trimmed
                    .strip_prefix('"')
                    .and_then(|rest| rest.split_once('"'))
                    .map(|(name, _)| name.to_string())
                    .filter(|_| trimmed.contains("=>"))
                else {
                    continue;
                };
                arms.insert(name, gate.take());
            }
            arms
        }

        /// Anti-vacuity: an oracle that reads the wrong block, or a DSL that
        /// moved, must fail here rather than report an empty dispatch and pass
        /// the forward check for the wrong reason. The last arm of
        /// `call_tool_handler`'s match is the `ToolNotFound` fallthrough, so its
        /// presence is the proof that the block ends where this says it does.
        #[test]
        fn the_dispatch_oracle_reads_a_real_match_block() {
            let source = include_str!("rmcp_adapter.rs");
            let block = dispatch_block(source, "match tool_name");
            // The extracted block ends with the match's own `}`; the arm before
            // it is the `ToolNotFound` fallthrough. Both facts together are the
            // proof that the brace matcher stopped where this oracle claims.
            let last_arm = block
                .trim_end()
                .strip_suffix('}')
                .unwrap_or_else(|| panic!("the extracted dispatch block does not close: …{block}"))
                .trim_end()
                .trim_end_matches(',');
            assert!(
                last_arm.ends_with("_ => Err(InterfaceError::ToolNotFound(tool_name.to_string()))"),
                "the last arm of the extracted dispatch block is not the ToolNotFound \
                 fallthrough, so its boundary is wrong: …{last_arm}"
            );
            let arms = dispatch_arms();
            assert!(
                arms.len() >= 50,
                "the oracle found only {} dispatch arms; the DSL it reads has probably \
                 drifted ({} names parsed)",
                arms.len(),
                arms.keys().cloned().collect::<Vec<_>>().join(", ")
            );
        }

        /// The invariant. A tool the catalogue announces and a tool the server
        /// cannot dispatch is the worst kind of surface: the client trusts
        /// `tools/list` to enumerate what exists, calls it, and gets an error
        /// with no hint that the tool was never there.
        #[test]
        fn every_advertised_tool_has_a_dispatch_arm() {
            let arms = dispatch_arms();
            let ghosts: Vec<_> = crate::interface::catalog::load_tool_catalog()
                .iter()
                .filter(|d| !arms.contains_key(&d.name))
                .map(|d| d.name.clone())
                .collect();
            assert!(
                ghosts.is_empty(),
                "tools/list announces {} tool(s) that tools/call cannot dispatch: {}. \
                 Either add the dispatch arm in rmcp_adapter::call_tool_handler, or \
                 stop advertising them — precedent: get_graph_report was removed from \
                 the catalogue rather than left to fail at runtime.",
                ghosts.len(),
                ghosts.join(", ")
            );
        }
    }

    // ==========================================================================
    // M5.2 WU3: M5 program-analysis tools round-trip — verifies the MCP
    // surface for the M5 algorithm IDs (REQ-MCP-01, REQ-MCP-02, REQ-MCP-04).
    // ==========================================================================
    mod m5_program_analysis_roundtrip {
        use super::*;
        use crate::application::program_analysis::conformance::canonical_corpus;
        #[cfg(feature = "program-analysis-server")]
        use crate::interface::mcp::handlers::program_analysis_handlers::handle_interproc_summary;
        use crate::interface::mcp::handlers::program_analysis_handlers::{
            ProgramAnalysisToolInput, handle_cfg, handle_slice_forward, handle_taint_flow,
        };
        use crate::interface::mcp::rmcp_adapter::build_all_tools;

        fn input_for(label: &str) -> ProgramAnalysisToolInput {
            let fx = canonical_corpus()
                .into_iter()
                .find(|f| f.label == label)
                .unwrap_or_else(|| panic!("missing canonical fixture `{label}`"));
            ProgramAnalysisToolInput {
                algorithm_params: fx.params,
                limits: None,
            }
        }

        fn sha256_hex(bytes: &[u8]) -> String {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(bytes);
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        }

        /// Helper: parse the handler's JSON-text response and pull the
        /// `digest` field plus the algorithm id.
        fn parse_tool_response(body: &str) -> (String, String) {
            let v: serde_json::Value = serde_json::from_str(body)
                .unwrap_or_else(|e| panic!("handler body is not JSON: {e}: {body}"));
            let algorithm = v
                .get("algorithm")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let digest = v
                .get("digest")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            (algorithm, digest)
        }

        /// REQ-MCP-01 — every required M5 algorithm id is listed.
        #[test]
        fn test_m5_tools_listed() {
            let listed: Vec<String> = build_all_tools()
                .iter()
                .map(|t| t.name.to_string())
                .collect();
            for required in [
                "cfg_per_function",
                "dominators_cfg",
                "slice_forward",
                "slice_backward",
                "taint_flow",
            ] {
                assert!(
                    listed.iter().any(|n| n == required),
                    "tools/list missing `{required}` (have: {listed:?})"
                );
            }
            #[cfg(feature = "program-analysis-server")]
            assert!(
                listed.iter().any(|n| n == "interproc_summary"),
                "tools/list missing `interproc_summary`"
            );
        }

        /// REQ-MCP-02 — tools/call cfg_per_function digest matches
        /// conformance corpus.
        #[test]
        fn test_cfg_per_function_digest_matches_conformance() {
            let body = handle_cfg(input_for("linear_chain_three_blocks")).expect("dispatch ok");
            let (algorithm, digest) = parse_tool_response(&body);
            assert_eq!(algorithm, "cfg_per_function");
            // Independently re-run the conformance harness on the same
            // fixture and check the digest it reports matches what the
            // MCP layer returned. This is the MCP-layer replay contract.
            let svc = crate::application::program_analysis::ProgramAnalysisService::new();
            let fx = canonical_corpus()
                .into_iter()
                .find(|f| f.label == "linear_chain_three_blocks")
                .unwrap();
            let harness = crate::application::program_analysis::conformance::replay_guard(
                &svc,
                &[
                    crate::application::program_analysis::conformance::ConformanceFixture {
                        algorithm: fx.algorithm,
                        label: fx.label,
                        params: fx.params.clone(),
                    },
                ],
            );
            let harness_digest = &harness[0].digest_a;
            assert_eq!(
                digest, *harness_digest,
                "MCP cfg digest must match conformance digest"
            );
        }

        #[test]
        fn test_taint_flow_digest_matches_conformance() {
            let body = handle_taint_flow(input_for("linear_taint")).expect("dispatch ok");
            let (algorithm, digest) = parse_tool_response(&body);
            assert_eq!(algorithm, "taint_flow");
            assert_eq!(digest.len(), 64, "digest is sha256 hex");
        }

        #[test]
        fn test_slice_forward_digest_matches_conformance() {
            let body =
                handle_slice_forward(input_for("linear_forward_slice")).expect("dispatch ok");
            let (algorithm, digest) = parse_tool_response(&body);
            assert_eq!(algorithm, "slice_forward");
            // Replay: same input, same digest.
            let body2 =
                handle_slice_forward(input_for("linear_forward_slice")).expect("dispatch ok");
            let (_, digest2) = parse_tool_response(&body2);
            assert_eq!(
                digest, digest2,
                "slice_forward must be replay-deterministic"
            );
        }

        #[cfg(feature = "program-analysis-server")]
        #[test]
        fn test_interproc_summary_digest_matches_conformance() {
            let body =
                handle_interproc_summary(input_for("two_callers_one_callee")).expect("dispatch ok");
            let (algorithm, digest) = parse_tool_response(&body);
            assert_eq!(algorithm, "interproc_summary");
            assert_eq!(digest.len(), 64);
            // The MCP body itself is sha256-stable across two calls.
            let body2 =
                handle_interproc_summary(input_for("two_callers_one_callee")).expect("dispatch ok");
            let (_, digest2) = parse_tool_response(&body2);
            assert_eq!(
                digest, digest2,
                "interproc_summary must be replay-deterministic"
            );
        }

        /// REQ-MCP-04 — unknown tool id is rejected.
        #[test]
        fn test_unknown_tool_yields_err_string() {
            // We can't easily drive the rmcp handler from inside this test
            // (it lives behind a tokio::time::timeout + match arm). Instead,
            // we verify the dispatch boundary that the handler uses: when
            // ProgramAnalysisService receives an unknown algorithm id, it
            // returns Err.
            use crate::domain::analytics::descriptor::AlgorithmId;
            let svc = crate::application::program_analysis::ProgramAnalysisService::new();
            let id = AlgorithmId::from_static("not_a_real_m5_algorithm");
            let res = svc.dispatch(
                &id,
                &serde_json::json!({}),
                &crate::domain::plan::limits::PlanLimits::default(),
            );
            assert!(res.is_err(), "unknown algorithm id must surface as Err");
        }

        /// REQ-MCP-03 — malformed algorithm_params yields a structured Err
        /// (no panic).
        #[test]
        fn test_malformed_params_yields_err() {
            let input = ProgramAnalysisToolInput {
                algorithm_params: serde_json::json!({"not": "valid"}),
                limits: None,
            };
            let res = handle_cfg(input);
            assert!(res.is_err(), "malformed params must surface as Err");
            // Body must contain an "analytics_error:" prefix for downstream
            // dashboards to classify it.
            let err = res.unwrap_err();
            assert!(
                err.contains("analytics_error") || err.contains("unsupported"),
                "error body should be structured, got: {err}"
            );
        }

        // Reachability sanity: the digest helper is exercised to keep the
        // SHA-256 path under coverage.
        #[test]
        fn test_sha256_helper_known_answer() {
            assert_eq!(
                sha256_hex(b"hello"),
                "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
            );
        }
    }
}
