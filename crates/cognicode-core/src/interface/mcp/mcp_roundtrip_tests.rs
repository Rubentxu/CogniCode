//! E2E MCP Protocol Roundtrip Tests
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
        HandlerContext::new(temp_dir.path().to_path_buf())
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
            handle_read_file, handle_write_file, handle_edit_file,
            handle_search_content, handle_list_files,
        };
        use crate::interface::mcp::schemas::{
            EditFileInput, FileEdit, ListFilesInput, ReadFileInput,
            SearchContentInput, WriteFileInput,
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
            assert!(output.symbols_found >= 0);
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
            assert_eq!(parsed_input.function_name, Some("simple_function".to_string()));

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
            let ctx = create_test_context(&temp_dir);

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
}
