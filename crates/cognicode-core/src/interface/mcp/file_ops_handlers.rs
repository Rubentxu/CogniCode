//! File Operations Handlers - MCP handlers for LLM-friendly file operations
//!
//! This module provides the 5 MCP tool handlers:
//! - read_file: Smart file reading with semantic modes
//! - write_file: Atomic file writes with workspace safety
//! - edit_file: String-replacement edits with tree-sitter validation
//! - search_content: Regex/literal search with .gitignore awareness
//! - list_files: Directory listing with .gitignore filtering
//!
//! Each handler is wrapped in instrument_tool for OTel metrics collection.

use crate::application::services::file_operations::FileOperationsService;
use crate::application::error::AppError;
use crate::infrastructure::telemetry::{get_global_metrics, instrument_tool, ToolError};
use crate::interface::mcp::handlers::{HandlerContext, HandlerError, HandlerResult};
use crate::interface::mcp::schemas::{
    EditFileInput, EditFileOutput, ListFilesInput, ListFilesOutput, ReadFileInput,
    ReadFileOutput, SearchContentInput, SearchContentOutput, WriteFileInput, WriteFileOutput,
};

/// Converts AppError to ToolError for use with instrument_tool
fn app_error_to_tool_error(e: AppError) -> ToolError {
    ToolError::new("AppError", e.to_string())
}

/// Handler for read_file tool
///
/// Smart file reader with semantic modes. Use INSTEAD of generic file reads:
/// - 'raw' mode: Returns file content as-is with line numbers
/// - 'outline' mode: Returns hierarchical structure (functions, classes)
/// - 'symbols' mode: Extracts function/class signatures only
/// - 'compressed' mode: Token-efficient summaries for large files
pub async fn handle_read_file(
    ctx: &HandlerContext,
    input: ReadFileInput,
) -> HandlerResult<ReadFileOutput> {
    let metrics = match get_global_metrics() {
        Some(m) => m,
        None => {
            // No metrics available, call service directly
            let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
            let dto_input: crate::application::dto::ReadFileRequest = input.into();
            let dto_result = service.read_file(dto_input).map_err(HandlerError::App)?;
            return Ok(dto_result.into());
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
    let mode = input.mode.clone();
    let dto_input: crate::application::dto::ReadFileRequest = input.into();

    let result = instrument_tool(&metrics, "read_file", async {
        match service.read_file(dto_input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            let mcp_output: ReadFileOutput = output.into();
            metrics.record_bytes_read(mcp_output.metadata.size as f64, mode.as_deref().unwrap_or("raw"));
            Ok(mcp_output)
        }
        Err(e) => Err(HandlerError::App(AppError::InternalError(e.message))),
    }
}

/// Handler for write_file tool
///
/// Create or overwrite files within the workspace. Validates paths and creates
/// parent directories. Returns metadata only — token-efficient.
pub async fn handle_write_file(
    ctx: &HandlerContext,
    input: WriteFileInput,
) -> HandlerResult<WriteFileOutput> {
    let metrics = match get_global_metrics() {
        Some(m) => m,
        None => {
            let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
            let dto_input: crate::application::dto::WriteFileRequest = input.into();
            let dto_result = service.write_file(dto_input).map_err(HandlerError::App)?;
            return Ok(dto_result.into());
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
    let dto_input: crate::application::dto::WriteFileRequest = input.into();

    let result = instrument_tool(&metrics, "write_file", async {
        match service.write_file(dto_input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            let mcp_output: WriteFileOutput = output.into();
            metrics.record_bytes_written(mcp_output.bytes_written as f64);
            Ok(mcp_output)
        }
        Err(e) => Err(HandlerError::App(AppError::InternalError(e.message))),
    }
}

/// Handler for edit_file tool
///
/// Edit files with syntax validation. Use INSTEAD of generic string replacement:
/// - Validates the result with tree-sitter to catch syntax errors before saving
/// - Supports multiple edits in one call
/// - Rejects edits that would cause syntax errors
pub async fn handle_edit_file(
    ctx: &HandlerContext,
    input: EditFileInput,
) -> HandlerResult<EditFileOutput> {
    let metrics = match get_global_metrics() {
        Some(m) => m,
        None => {
            let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
            let dto_input: crate::application::dto::EditFileRequest = input.into();
            let dto_result = service.edit_file(dto_input).map_err(HandlerError::App)?;
            return Ok(dto_result.into());
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
    let dto_input: crate::application::dto::EditFileRequest = input.into();

    let result = instrument_tool(&metrics, "edit_file", async {
        match service.edit_file(dto_input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            let mcp_output: EditFileOutput = output.into();
            if !mcp_output.validation.passed {
                metrics.record_edit_rejected("syntax_error");
            }
            if mcp_output.applied {
                metrics.record_bytes_written(mcp_output.bytes_changed as f64);
            }
            Ok(mcp_output)
        }
        Err(e) => Err(HandlerError::App(AppError::InternalError(e.message))),
    }
}

/// Handler for search_content tool
///
/// Search file contents with .gitignore awareness. Use INSTEAD of grep:
/// - Automatically respects .gitignore
/// - Supports regex and literal patterns
/// - Returns matches with context lines and capped results
pub async fn handle_search_content(
    ctx: &HandlerContext,
    input: SearchContentInput,
) -> HandlerResult<SearchContentOutput> {
    let metrics = match get_global_metrics() {
        Some(m) => m,
        None => {
            let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
            let dto_input: crate::application::dto::SearchContentRequest = input.into();
            let dto_result = service.search_content(dto_input).map_err(HandlerError::App)?;
            return Ok(dto_result.into());
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
    let dto_input: crate::application::dto::SearchContentRequest = input.into();

    let result = instrument_tool(&metrics, "search_content", async {
        match service.search_content(dto_input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            let mcp_output: SearchContentOutput = output.into();
            metrics.record_search_matches(mcp_output.total as f64, "mixed");
            metrics.record_files_scanned(mcp_output.files_scanned as f64);
            Ok(mcp_output)
        }
        Err(e) => Err(HandlerError::App(AppError::InternalError(e.message))),
    }
}

/// Handler for list_files tool
///
/// List project files with .gitignore awareness. Use INSTEAD of glob:
/// - Automatically respects .gitignore
/// - Returns metadata (size, modified time, language detection)
/// - Supports pagination
pub async fn handle_list_files(
    ctx: &HandlerContext,
    input: ListFilesInput,
) -> HandlerResult<ListFilesOutput> {
    let metrics = match get_global_metrics() {
        Some(m) => m,
        None => {
            let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
            let dto_input: crate::application::dto::ListFilesRequest = input.into();
            let dto_result = service.list_files(dto_input).map_err(HandlerError::App)?;
            return Ok(dto_result.into());
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
    let dto_input: crate::application::dto::ListFilesRequest = input.into();

    let result = instrument_tool(&metrics, "list_files", async {
        match service.list_files(dto_input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            let mcp_output: ListFilesOutput = output.into();
            metrics.record_files_scanned(mcp_output.total as f64);
            Ok(mcp_output)
        }
        Err(e) => Err(HandlerError::App(AppError::InternalError(e.message))),
    }
}

// ============================================================================
// Inline Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::mcp::handlers::HandlerContext;
    use crate::interface::mcp::schemas::{
        EditFileInput, FileEdit, ListFilesInput, ReadFileInput, SearchContentInput, WriteFileInput,
    };

    /// Helper to create a HandlerContext with workspace scoping
    fn create_test_context(temp_dir: &tempfile::TempDir) -> HandlerContext {
        HandlerContext::new(temp_dir.path().to_path_buf())
    }

    // ========================================================================
    // Security Boundary Enforcement Tests
    // ========================================================================

    mod security_boundary_tests {
        use super::*;

        #[tokio::test]
        async fn test_read_file_rejects_path_traversal() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Attempt path traversal to escape workspace
            let input = ReadFileInput {
                path: "../../etc/passwd".to_string(),
                start_line: None,
                end_line: None,
                mode: None,
                chunk_size: None,
                continuation_token: None,
            };

            let result = handle_read_file(&ctx, input).await;
            assert!(result.is_err(), "read_file should reject path traversal");
        }

        #[tokio::test]
        async fn test_read_file_rejects_directory_traversal() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Create a subdirectory and try to escape via ../
            std::fs::create_dir_all(temp_dir.path().join("subdir")).unwrap();

            let input = ReadFileInput {
                path: "../outside".to_string(),
                start_line: None,
                end_line: None,
                mode: None,
                chunk_size: None,
                continuation_token: None,
            };

            let result = handle_read_file(&ctx, input).await;
            assert!(result.is_err(), "read_file should reject directory traversal via ../");
        }

        #[tokio::test]
        async fn test_read_file_rejects_absolute_path_outside_workspace() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let input = ReadFileInput {
                path: "/etc/passwd".to_string(),
                start_line: None,
                end_line: None,
                mode: None,
                chunk_size: None,
                continuation_token: None,
            };

            let result = handle_read_file(&ctx, input).await;
            assert!(result.is_err(), "read_file should reject absolute paths outside workspace");
        }

        #[tokio::test]
        async fn test_read_file_rejects_symlink_to_outside() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Create a file outside workspace
            let outside_path = std::env::temp_dir().join("cognicode_test_outside.txt");
            std::fs::write(&outside_path, "outside content").unwrap();

            // Create a symlink inside workspace pointing to outside
            let symlink_path = temp_dir.path().join("link_to_outside");
            #[cfg(unix)]
            std::os::unix::fs::symlink(&outside_path, &symlink_path).unwrap();

            let input = ReadFileInput {
                path: symlink_path.to_string_lossy().to_string(),
                start_line: None,
                end_line: None,
                mode: None,
                chunk_size: None,
                continuation_token: None,
            };

            let result = handle_read_file(&ctx, input).await;
            // Symlink following should be blocked
            assert!(result.is_err(), "read_file should reject symlink following");

            // Cleanup
            let _ = std::fs::remove_file(outside_path);
        }

        #[tokio::test]
        async fn test_write_file_rejects_path_traversal() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let input = WriteFileInput {
                path: "../../evil.txt".to_string(),
                content: "malicious content".to_string(),
                create_dirs: Some(false),
            };

            let result = handle_write_file(&ctx, input).await;
            assert!(result.is_err(), "write_file should reject path traversal");
        }

        #[tokio::test]
        async fn test_write_file_rejects_absolute_path_outside_workspace() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let input = WriteFileInput {
                path: "/tmp/evil.txt".to_string(),
                content: "malicious content".to_string(),
                create_dirs: Some(false),
            };

            let result = handle_write_file(&ctx, input).await;
            assert!(result.is_err(), "write_file should reject absolute paths outside workspace");
        }

        #[tokio::test]
        async fn test_edit_file_rejects_path_traversal() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let input = EditFileInput {
                path: "../../etc/passwd".to_string(),
                edits: vec![FileEdit {
                    old_string: "something".to_string(),
                    new_string: "replacement".to_string(),
                }],
            };

            let result = handle_edit_file(&ctx, input).await;
            assert!(result.is_err(), "edit_file should reject path traversal");
        }

        #[tokio::test]
        async fn test_search_content_rejects_path_traversal() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let input = SearchContentInput {
                pattern: "test".to_string(),
                path: Some("../../secrets".to_string()),
                file_glob: None,
                regex: Some(true),
                case_insensitive: Some(false),
                max_results: Some(50),
                context_lines: Some(2),
            };

            let result = handle_search_content(&ctx, input).await;
            assert!(result.is_err(), "search_content should reject path traversal");
        }

        #[tokio::test]
        async fn test_list_files_rejects_path_traversal() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let input = ListFilesInput {
                path: Some("../../secrets".to_string()),
                glob: None,
                offset: None,
                limit: None,
                recursive: None,
                max_depth: None,
            };

            let result = handle_list_files(&ctx, input).await;
            assert!(result.is_err(), "list_files should reject path traversal");
        }
    }

    // ========================================================================
    // Read Modes Tests
    // ========================================================================

    mod read_modes_tests {
        use super::*;

        #[tokio::test]
        async fn test_read_file_raw_mode() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("test.txt");
            std::fs::write(&file_path, "line 1\nline 2\nline 3\n").unwrap();

            let input = ReadFileInput {
                path: file_path.to_str().unwrap().to_string(),
                start_line: None,
                end_line: None,
                mode: Some("raw".to_string()),
                chunk_size: None,
                continuation_token: None,
            };

            let result = handle_read_file(&ctx, input).await;
            assert!(result.is_ok(), "read_file raw mode should succeed");
            let output = result.unwrap();
            assert!(output.content.contains("line 1"));
            assert_eq!(output.mode, "raw");
        }

        #[tokio::test]
        async fn test_read_file_outline_mode() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("test.rs");
            std::fs::write(
                &file_path,
                "struct MyStruct { field: i32 }\nfn my_function() {}",
            )
            .unwrap();

            let input = ReadFileInput {
                path: file_path.to_str().unwrap().to_string(),
                start_line: None,
                end_line: None,
                mode: Some("outline".to_string()),
                chunk_size: None,
                continuation_token: None,
            };

            let result = handle_read_file(&ctx, input).await;
            assert!(result.is_ok(), "read_file outline mode should succeed");
            let output = result.unwrap();
            assert!(output.total_lines > 0);
        }

        #[tokio::test]
        async fn test_read_file_symbols_mode() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("test.rs");
            std::fs::write(
                &file_path,
                "struct MyStruct {}\nfn my_function() {}\nconst MY_CONST: i32 = 42;",
            )
            .unwrap();

            let input = ReadFileInput {
                path: file_path.to_str().unwrap().to_string(),
                start_line: None,
                end_line: None,
                mode: Some("symbols".to_string()),
                chunk_size: None,
                continuation_token: None,
            };

            let result = handle_read_file(&ctx, input).await;
            assert!(result.is_ok(), "read_file symbols mode should succeed");
            let output = result.unwrap();
            // Symbols mode should extract function/struct signatures
            assert!(
                output.content.contains("struct") || output.content.contains("fn"),
                "symbols mode should contain struct or fn"
            );
        }

        #[tokio::test]
        async fn test_read_file_compressed_mode() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("test.txt");
            // Create a file with many lines
            let mut content = String::from("// Header\n");
            for i in 0..100 {
                content.push_str(&format!("// Comment line {}\n", i));
                content.push_str(&format!("function_{}();\n", i));
                content.push_str("\n"); // blank line
            }
            std::fs::write(&file_path, &content).unwrap();

            let input = ReadFileInput {
                path: file_path.to_str().unwrap().to_string(),
                start_line: None,
                end_line: None,
                mode: Some("compressed".to_string()),
                chunk_size: None,
                continuation_token: None,
            };

            let result = handle_read_file(&ctx, input).await;
            assert!(result.is_ok(), "read_file compressed mode should succeed");
            let output = result.unwrap();
            // Compressed mode should significantly reduce content
            assert!(
                output.content.len() < content.len(),
                "compressed mode should reduce content size"
            );
        }
    }

    // ========================================================================
    // Write Operations Tests
    // ========================================================================

    mod write_operations_tests {
        use super::*;

        #[tokio::test]
        async fn test_write_file_atomic_write() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("new_file.txt");

            let input = WriteFileInput {
                path: file_path.to_str().unwrap().to_string(),
                content: "atomic content".to_string(),
                create_dirs: Some(false),
            };

            let result = handle_write_file(&ctx, input).await;
            assert!(result.is_ok(), "write_file should succeed for new file");

            let output = result.unwrap();
            assert_eq!(output.bytes_written, "atomic content".len() as u64);
            assert!(std::path::Path::new(&output.metadata.path).exists());
        }

        #[tokio::test]
        async fn test_write_file_creates_parent_dirs() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir
                .path()
                .join("deep")
                .join("nested")
                .join("dir")
                .join("file.txt");

            let input = WriteFileInput {
                path: file_path.to_str().unwrap().to_string(),
                content: "content with parent dirs".to_string(),
                create_dirs: Some(true),
            };

            let result = handle_write_file(&ctx, input).await;
            assert!(result.is_ok(), "write_file with create_dirs=true should succeed");
            assert!(file_path.exists());
        }

        #[tokio::test]
        async fn test_write_file_overwrites_existing() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("existing.txt");
            std::fs::write(&file_path, "original content").unwrap();

            let input = WriteFileInput {
                path: file_path.to_str().unwrap().to_string(),
                content: "new content".to_string(),
                create_dirs: Some(false),
            };

            let result = handle_write_file(&ctx, input).await;
            assert!(result.is_ok(), "write_file should succeed for existing file");

            let content = std::fs::read_to_string(&file_path).unwrap();
            assert_eq!(content, "new content");
        }
    }

    // ========================================================================
    // Edit Operations Tests
    // ========================================================================

    mod edit_operations_tests {
        use super::*;

        #[tokio::test]
        async fn test_edit_file_valid_tree_sitter_edit() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("test.rs");
            std::fs::write(&file_path, "fn old_name() {}").unwrap();

            let input = EditFileInput {
                path: file_path.to_str().unwrap().to_string(),
                edits: vec![FileEdit {
                    old_string: "old_name".to_string(),
                    new_string: "new_name".to_string(),
                }],
            };

            let result = handle_edit_file(&ctx, input).await;
            assert!(result.is_ok(), "edit_file should succeed for valid edit");

            let output = result.unwrap();
            assert!(output.applied || output.validation.passed);
        }

        #[tokio::test]
        async fn test_edit_file_validation() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("test.rs");
            std::fs::write(&file_path, "fn test() {}").unwrap();

            let input = EditFileInput {
                path: file_path.to_str().unwrap().to_string(),
                edits: vec![FileEdit {
                    old_string: "fn test() {}".to_string(),
                    new_string: "fn test() { ".to_string(), // Missing closing brace
                }],
            };

            let result = handle_edit_file(&ctx, input).await;
            assert!(result.is_ok(), "edit_file should return result even for invalid");

            let output = result.unwrap();
            // Validation should fail for syntax error
            assert!(
                !output.validation.passed || !output.applied,
                "invalid edit should fail validation"
            );
        }

        #[tokio::test]
        async fn test_edit_file_handles_no_match_gracefully() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("test.txt");
            std::fs::write(&file_path, "Hello World").unwrap();

            let input = EditFileInput {
                path: file_path.to_str().unwrap().to_string(),
                edits: vec![FileEdit {
                    old_string: "nonexistent_string".to_string(),
                    new_string: "replacement".to_string(),
                }],
            };

            let result = handle_edit_file(&ctx, input).await;
            assert!(result.is_ok(), "edit_file should handle no match gracefully");

            let output = result.unwrap();
            assert!(!output.applied, "edit should not be applied when no match");
            assert!(output.preview.is_some() || output.reason.is_some());
        }

        #[tokio::test]
        async fn test_edit_file_multiple_edits() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            let file_path = temp_dir.path().join("test.txt");
            std::fs::write(&file_path, "foo bar baz").unwrap();

            let input = EditFileInput {
                path: file_path.to_str().unwrap().to_string(),
                edits: vec![
                    FileEdit {
                        old_string: "foo".to_string(),
                        new_string: "qux".to_string(),
                    },
                    FileEdit {
                        old_string: "bar".to_string(),
                        new_string: "quux".to_string(),
                    },
                ],
            };

            let result = handle_edit_file(&ctx, input).await;
            assert!(result.is_ok(), "edit_file with multiple edits should succeed");
        }
    }

    // ========================================================================
    // Search Operations Tests
    // ========================================================================

    mod search_operations_tests {
        use super::*;

        #[tokio::test]
        async fn test_search_content_regex_search() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            std::fs::write(
                temp_dir.path().join("test.txt"),
                "item_one: value1\nitem_two: value2\nitem_ten: value10",
            )
            .unwrap();

            let input = SearchContentInput {
                pattern: r"item_\w+".to_string(),
                path: None,
                file_glob: Some("*.txt".to_string()),
                regex: Some(true),
                case_insensitive: Some(false),
                max_results: Some(50),
                context_lines: Some(0),
            };

            let result = handle_search_content(&ctx, input).await;
            assert!(result.is_ok(), "search_content regex should succeed");

            let output = result.unwrap();
            assert_eq!(output.total, 3, "should find all 3 items with regex");
        }

        #[tokio::test]
        async fn test_search_content_literal_search() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            std::fs::write(
                temp_dir.path().join("test.txt"),
                "item_one: value1\nitem_two: value2\nitem_ten: value10",
            )
            .unwrap();

            // Use literal search (regex = false)
            let input = SearchContentInput {
                pattern: "item_one".to_string(),
                path: None,
                file_glob: Some("*.txt".to_string()),
                regex: Some(false),
                case_insensitive: Some(false),
                max_results: Some(50),
                context_lines: Some(0),
            };

            let result = handle_search_content(&ctx, input).await;
            assert!(result.is_ok(), "search_content literal should succeed");

            let output = result.unwrap();
            assert_eq!(output.total, 1, "should find exactly 1 match for literal 'item_one'");
        }

        #[tokio::test]
        async fn test_search_content_respects_gitignore() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Initialize a git repo so .gitignore is respected
            let git_dir = temp_dir.path().join(".git");
            std::fs::create_dir_all(&git_dir).unwrap();
            std::fs::write(git_dir.join("config"), "[core]\n").unwrap();
            std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

            // Create .gitignore that ignores *.secret files
            std::fs::write(
                temp_dir.path().join(".gitignore"),
                "*.secret\n",
            )
            .unwrap();

            // Create files - one should be ignored
            std::fs::write(
                temp_dir.path().join("main.rs"),
                "let password = \"hello\";",
            )
            .unwrap();
            std::fs::write(
                temp_dir.path().join("secret.secret"),
                "API_KEY=supersecret",
            )
            .unwrap();

            let input = SearchContentInput {
                pattern: "password".to_string(),
                path: None,
                file_glob: None,
                regex: Some(false),
                case_insensitive: Some(false),
                max_results: Some(50),
                context_lines: Some(0),
            };

            let result = handle_search_content(&ctx, input).await;
            assert!(result.is_ok(), "search_content should succeed");

            let output = result.unwrap();
            // Should find password in main.rs but not in secret.secret
            assert!(
                output.matches.iter().any(|m| m.file.contains("main.rs")),
                "should find match in main.rs"
            );
            assert!(
                !output.matches.iter().any(|m| m.file.contains("secret.secret")),
                "should not find matches in gitignored files"
            );
        }
    }

    // ========================================================================
    // List Operations Tests
    // ========================================================================

    mod list_operations_tests {
        use super::*;

        #[tokio::test]
        async fn test_list_files_with_gitignore_filtering() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Initialize a git repo so .gitignore is respected
            let git_dir = temp_dir.path().join(".git");
            std::fs::create_dir_all(&git_dir).unwrap();
            std::fs::write(git_dir.join("config"), "[core]\n").unwrap();
            std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

            // Create .gitignore that ignores *.log files
            std::fs::write(temp_dir.path().join(".gitignore"), "*.log\n").unwrap();

            // Create files - one should be ignored
            std::fs::write(temp_dir.path().join("main.rs"), "fn main() {}").unwrap();
            std::fs::write(temp_dir.path().join("debug.log"), "DEBUG: starting").unwrap();

            let input = ListFilesInput {
                path: None,
                glob: Some("**/*".to_string()),
                offset: None,
                limit: None,
                recursive: None,
                max_depth: None,
            };

            let result = handle_list_files(&ctx, input).await;
            assert!(result.is_ok(), "list_files should succeed");

            let output = result.unwrap();
            let paths: Vec<&str> = output.files.iter().map(|f| f.path.as_str()).collect();

            assert!(
                paths.iter().any(|p| p.contains("main.rs")),
                "should find main.rs"
            );
            assert!(
                !paths.iter().any(|p| p.contains("debug.log")),
                "should not find debug.log (gitignored)"
            );
        }

        #[tokio::test]
        async fn test_list_files_without_gitignore_filtering() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            // Create a file with gitignored extension but no git repo initialized
            // (no .git directory means gitignore won't be applied in some implementations)
            std::fs::write(temp_dir.path().join("main.rs"), "fn main() {}").unwrap();
            std::fs::write(temp_dir.path().join("debug.log"), "DEBUG: starting").unwrap();

            let input = ListFilesInput {
                path: None,
                glob: Some("**/*".to_string()),
                offset: None,
                limit: None,
                recursive: None,
                max_depth: None,
            };

            let result = handle_list_files(&ctx, input).await;
            assert!(result.is_ok(), "list_files should succeed");

            let output = result.unwrap();
            let paths: Vec<&str> = output.files.iter().map(|f| f.path.as_str()).collect();

            // Without git repo, both files should be listed
            assert!(
                paths.iter().any(|p| p.contains("main.rs")),
                "should find main.rs"
            );
            assert!(
                paths.iter().any(|p| p.contains("debug.log")),
                "should find debug.log when no git repo"
            );
        }

        #[tokio::test]
        async fn test_list_files_returns_metadata() {
            let temp_dir = tempfile::tempdir().unwrap();
            let ctx = create_test_context(&temp_dir);

            std::fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();

            let input = ListFilesInput {
                path: None,
                glob: Some("*.txt".to_string()),
                offset: None,
                limit: None,
                recursive: None,
                max_depth: None,
            };

            let result = handle_list_files(&ctx, input).await;
            assert!(result.is_ok(), "list_files should succeed");

            let output = result.unwrap();
            assert!(output.total >= 1);

            if let Some(file_entry) = output.files.first() {
                assert!(file_entry.size > 0, "file should have size");
                assert!(file_entry.modified > 0, "file should have modified timestamp");
            }
        }
    }
}
