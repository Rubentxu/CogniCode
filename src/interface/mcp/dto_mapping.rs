//! DTO Mapping - Conversion between MCP schemas and Application DTOs
//!
//! This module provides `From` implementations to convert between MCP protocol types
//! and transport-neutral application DTOs.

use crate::application::dto::{
    AnalysisMetadata, CallHierarchyEntry, ContentMatch, EditFileRequest, EditFileResult,
    EditValidation, FileEdit, FileEntry, FileMetadata, GetCallHierarchyResult,
    GetFileSymbolsResult, ListFilesRequest, ListFilesResult, ReadFileRequest, ReadFileResult,
    RiskLevel, SearchContentRequest, SearchContentResult, SourceLocation as DtoSourceLocation,
    SymbolKind, SymbolSummary, SyntaxIssue, WriteFileRequest, WriteFileResult,
};
use crate::interface::mcp::schemas::{
    CallEntry, ContentMatch as McpContentMatch, EditFileInput, EditFileOutput,
    EditValidation as McpEditValidation, FileEdit as McpFileEdit, FileEntry as McpFileEntry,
    FileMetadata as McpFileMetadata, GetCallHierarchyOutput, GetFileSymbolsOutput, ListFilesInput,
    ListFilesOutput, ReadFileInput, ReadFileOutput, RiskLevel as McpRiskLevel, SearchContentInput,
    SearchContentOutput, SourceLocation as McpSourceLocation, SymbolInfo,
    SymbolKind as McpSymbolKind, SyntaxError, WriteFileInput, WriteFileOutput,
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
            mode: input.mode,
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

impl From<FileMetadata> for McpFileMetadata {
    fn from(metadata: FileMetadata) -> Self {
        McpFileMetadata {
            path: metadata.path,
            size: metadata.size,
            modified: metadata.modified,
            language: metadata.language,
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
            edits: input.edits.into_iter().map(|e| e.into()).collect(),
        }
    }
}

impl From<McpFileEdit> for FileEdit {
    fn from(edit: McpFileEdit) -> Self {
        FileEdit {
            old_string: edit.old_string,
            new_string: edit.new_string,
        }
    }
}

impl From<EditFileResult> for EditFileOutput {
    fn from(result: EditFileResult) -> Self {
        EditFileOutput {
            applied: result.applied,
            validation: result.validation.into(),
            preview: result.preview,
            bytes_changed: result.bytes_changed,
            reason: result.reason,
        }
    }
}

impl From<McpEditValidation> for EditValidation {
    fn from(validation: McpEditValidation) -> Self {
        EditValidation {
            passed: validation.passed,
            syntax_errors: validation
                .syntax_errors
                .into_iter()
                .map(|e| e.into())
                .collect(),
        }
    }
}

impl From<EditValidation> for McpEditValidation {
    fn from(validation: EditValidation) -> Self {
        McpEditValidation {
            passed: validation.passed,
            syntax_errors: validation
                .syntax_errors
                .into_iter()
                .map(|e| e.into())
                .collect(),
        }
    }
}

impl From<SyntaxError> for SyntaxIssue {
    fn from(error: SyntaxError) -> Self {
        SyntaxIssue {
            line: error.line,
            column: error.column,
            message: error.message,
            severity: error.severity,
        }
    }
}

impl From<SyntaxIssue> for SyntaxError {
    fn from(error: SyntaxIssue) -> Self {
        SyntaxError {
            line: error.line,
            column: error.column,
            message: error.message,
            severity: error.severity,
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

impl From<ContentMatch> for McpContentMatch {
    fn from(m: ContentMatch) -> Self {
        McpContentMatch {
            file: m.file,
            line: m.line,
            col: m.col,
            text: m.text,
            context: m.context,
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
            files: result.files.into_iter().map(|f| f.into()).collect(),
            total: result.total,
            depth_traversed: result.depth_traversed,
        }
    }
}

impl From<FileEntry> for McpFileEntry {
    fn from(entry: FileEntry) -> Self {
        McpFileEntry {
            path: entry.path,
            size: entry.size,
            modified: entry.modified,
            is_dir: entry.is_dir,
            language: entry.language,
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

impl From<McpSourceLocation> for DtoSourceLocation {
    fn from(loc: McpSourceLocation) -> Self {
        DtoSourceLocation {
            file: loc.file,
            line: loc.line,
            column: loc.column,
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

impl From<crate::interface::mcp::schemas::AnalysisMetadata> for AnalysisMetadata {
    fn from(meta: crate::interface::mcp::schemas::AnalysisMetadata) -> Self {
        AnalysisMetadata {
            total_calls: meta.total_calls,
            analysis_time_ms: meta.analysis_time_ms,
        }
    }
}

// ============================================================================
// Impact Analysis
// ============================================================================

impl From<McpRiskLevel> for RiskLevel {
    fn from(level: McpRiskLevel) -> Self {
        match level {
            McpRiskLevel::Low => RiskLevel::Low,
            McpRiskLevel::Medium => RiskLevel::Medium,
            McpRiskLevel::High => RiskLevel::High,
            McpRiskLevel::Critical => RiskLevel::Critical,
        }
    }
}
