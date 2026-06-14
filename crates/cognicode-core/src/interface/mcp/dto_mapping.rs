//! DTO Mapping - Conversion between MCP schemas and Application DTOs
//!
//! This module provides `From` implementations to convert between MCP protocol types
//! and transport-neutral application DTOs.
//!
//! NOTE: Types that are identical between MCP and DTO layers (FileMetadata, FileEdit,
//! FileEntry, ContentMatch, SyntaxIssue, EditValidation, ListFilesResult) are defined
//! once in dto/common.rs and re-exported by schemas.rs. Their conversions are identity
//! and do not require From impls here.

use crate::application::dto::{
    CallHierarchyEntry, EditFileRequest, EditFileResult,
    GetCallHierarchyResult,
    GetFileSymbolsResult, ListFilesRequest, ListFilesResult, ReadFileRequest, ReadFileResult,
    ReadMode, SearchContentRequest, SearchContentResult,
    SymbolKind, SymbolSummary,
    WriteFileRequest, WriteFileResult,
};
use crate::interface::mcp::schemas::{
    CallEntry, EditFileInput, EditFileOutput, GetCallHierarchyOutput, GetFileSymbolsOutput,
    ListFilesInput, ListFilesOutput, ReadFileInput, ReadFileOutput, SearchContentInput,
    SearchContentOutput, SymbolInfo,
    SymbolKind as McpSymbolKind, WriteFileInput, WriteFileOutput,
};

// ============================================================================
// Read File
// ============================================================================

impl From<ReadFileInput> for ReadFileRequest {
    fn from(input: ReadFileInput) -> Self {
        ReadFileRequest {
            path: input.path,
            start_line: input.start_line,
            end_line: input.end_line,
            mode: input.mode.map(|s| s.parse().unwrap_or(ReadMode::Raw)),
            chunk_size: input.chunk_size,
            continuation_token: input.continuation_token,
        }
    }
}

impl From<ReadFileResult> for ReadFileOutput {
    fn from(result: ReadFileResult) -> Self {
        ReadFileOutput {
            content: result.content,
            total_lines: result.total_lines,
            truncated: result.truncated,
            metadata: result.metadata.into(),
            mode: result.mode,
            start_line: result.start_line,
            end_line: result.end_line,
            has_more: result.has_more,
            next_token: result.next_token,
            suggested_chunk_size: result.suggested_chunk_size,
        }
    }
}

// ============================================================================
// Write File
// ============================================================================

impl From<WriteFileInput> for WriteFileRequest {
    fn from(input: WriteFileInput) -> Self {
        WriteFileRequest {
            path: input.path,
            content: input.content,
            create_dirs: input.create_dirs,
        }
    }
}

impl From<WriteFileResult> for WriteFileOutput {
    fn from(result: WriteFileResult) -> Self {
        WriteFileOutput {
            bytes_written: result.bytes_written,
            metadata: result.metadata.into(),
        }
    }
}

// ============================================================================
// Edit File
// ============================================================================

impl From<EditFileInput> for EditFileRequest {
    fn from(input: EditFileInput) -> Self {
        EditFileRequest {
            path: input.path,
            edits: input.edits,
        }
    }
}

impl From<EditFileResult> for EditFileOutput {
    fn from(result: EditFileResult) -> Self {
        EditFileOutput {
            applied: result.applied,
            validation: result.validation,
            preview: result.preview,
            bytes_changed: result.bytes_changed,
            reason: result.reason,
        }
    }
}

// ============================================================================
// Search Content
// ============================================================================

impl From<SearchContentInput> for SearchContentRequest {
    fn from(input: SearchContentInput) -> Self {
        SearchContentRequest {
            pattern: input.pattern,
            path: input.path,
            file_glob: input.file_glob,
            regex: input.regex,
            case_insensitive: input.case_insensitive,
            max_results: input.max_results,
            context_lines: input.context_lines,
        }
    }
}

impl From<SearchContentResult> for SearchContentOutput {
    fn from(result: SearchContentResult) -> Self {
        SearchContentOutput {
            matches: result.matches.into_iter().map(|m| m.into()).collect(),
            total: result.total,
            files_scanned: result.files_scanned,
        }
    }
}

// ============================================================================
// List Files
// ============================================================================

impl From<ListFilesInput> for ListFilesRequest {
    fn from(input: ListFilesInput) -> Self {
        ListFilesRequest {
            path: input.path,
            glob: input.glob,
            offset: input.offset,
            limit: input.limit,
            recursive: input.recursive,
            max_depth: input.max_depth,
        }
    }
}

impl From<ListFilesResult> for ListFilesOutput {
    fn from(result: ListFilesResult) -> Self {
        ListFilesOutput {
            files: result.files,
            total: result.total,
            depth_traversed: result.depth_traversed,
        }
    }
}

// ============================================================================
// Symbol Analysis
// ============================================================================

impl From<GetFileSymbolsOutput> for GetFileSymbolsResult {
    fn from(output: GetFileSymbolsOutput) -> Self {
        GetFileSymbolsResult {
            file_path: output.file_path,
            symbols: output.symbols.into_iter().map(|s| s.into()).collect(),
        }
    }
}

impl From<SymbolInfo> for SymbolSummary {
    fn from(info: SymbolInfo) -> Self {
        SymbolSummary {
            name: info.name,
            kind: info.kind.into(),
            location: info.location.into(),
            signature: info.signature,
        }
    }
}

impl From<McpSymbolKind> for SymbolKind {
    fn from(kind: McpSymbolKind) -> Self {
        match kind {
            McpSymbolKind::Module => SymbolKind::Module,
            McpSymbolKind::Class => SymbolKind::Class,
            McpSymbolKind::Struct => SymbolKind::Struct,
            McpSymbolKind::Enum => SymbolKind::Enum,
            McpSymbolKind::Trait => SymbolKind::Trait,
            McpSymbolKind::Function => SymbolKind::Function,
            McpSymbolKind::Method => SymbolKind::Method,
            McpSymbolKind::Field => SymbolKind::Field,
            McpSymbolKind::Variable => SymbolKind::Variable,
            McpSymbolKind::Constant => SymbolKind::Constant,
            McpSymbolKind::Constructor => SymbolKind::Constructor,
            McpSymbolKind::Interface => SymbolKind::Interface,
            McpSymbolKind::TypeAlias => SymbolKind::TypeAlias,
            McpSymbolKind::Parameter => SymbolKind::Parameter,
        }
    }
}

// ============================================================================
// Call Hierarchy
// ============================================================================

impl From<GetCallHierarchyOutput> for GetCallHierarchyResult {
    fn from(output: GetCallHierarchyOutput) -> Self {
        GetCallHierarchyResult {
            symbol: output.symbol,
            calls: output.calls.into_iter().map(|c| c.into()).collect(),
            metadata: output.metadata.into(),
        }
    }
}

impl From<CallEntry> for CallHierarchyEntry {
    fn from(entry: CallEntry) -> Self {
        CallHierarchyEntry {
            symbol: entry.symbol,
            file: entry.file,
            line: entry.line,
            column: entry.column,
            confidence: entry.confidence,
        }
    }
}

