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
            return service
                .read_file(input)
                .map_err(HandlerError::App);
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());
    let mode = input.mode.clone();

    let result = instrument_tool(&metrics, "read_file", async {
        match service.read_file(input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            metrics.record_bytes_read(output.metadata.size as f64, mode.as_deref().unwrap_or("raw"));
            Ok(output)
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
            return service
                .write_file(input)
                .map_err(HandlerError::App);
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());

    let result = instrument_tool(&metrics, "write_file", async {
        match service.write_file(input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            metrics.record_bytes_written(output.bytes_written as f64);
            Ok(output)
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
            return service
                .edit_file(input)
                .map_err(HandlerError::App);
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());

    let result = instrument_tool(&metrics, "edit_file", async {
        match service.edit_file(input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            if !output.validation.passed {
                metrics.record_edit_rejected("syntax_error");
            }
            if output.applied {
                metrics.record_bytes_written(output.bytes_changed as f64);
            }
            Ok(output)
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
            return service
                .search_content(input)
                .map_err(HandlerError::App);
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());

    let result = instrument_tool(&metrics, "search_content", async {
        match service.search_content(input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            metrics.record_search_matches(output.total as f64, "mixed");
            metrics.record_files_scanned(output.files_scanned as f64);
            Ok(output)
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
            return service
                .list_files(input)
                .map_err(HandlerError::App);
        }
    };
    let service = FileOperationsService::new(ctx.working_dir.to_string_lossy().as_ref());

    let result = instrument_tool(&metrics, "list_files", async {
        match service.list_files(input) {
            Ok(output) => Ok(output),
            Err(e) => Err(app_error_to_tool_error(e)),
        }
    })
    .await;

    match result {
        Ok(output) => {
            metrics.record_files_scanned(output.total as f64);
            Ok(output)
        }
        Err(e) => Err(HandlerError::App(AppError::InternalError(e.message))),
    }
}
