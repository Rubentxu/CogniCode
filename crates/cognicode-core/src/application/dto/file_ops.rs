//! File Operations DTOs - Transport-neutral types for file operations
//!
//! These DTOs decouple the application layer from the MCP protocol.

use super::common::SourceLocation;
use serde::{Deserialize, Serialize};

// ============================================================================
// Read File
// ============================================================================

/// Read mode for file content
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReadMode {
    Raw,
    Outline,
    Symbols,
    Compressed,
}

impl Default for ReadMode {
    fn default() -> Self {
        ReadMode::Raw
    }
}

impl std::fmt::Display for ReadMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadMode::Raw => write!(f, "raw"),
            ReadMode::Outline => write!(f, "outline"),
            ReadMode::Symbols => write!(f, "symbols"),
            ReadMode::Compressed => write!(f, "compressed"),
        }
    }
}

/// Request for reading a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileRequest {
    pub path: String,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub end_line: Option<u32>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub chunk_size: Option<usize>,
    #[serde(default)]
    pub continuation_token: Option<String>,
}

impl ReadFileRequest {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            start_line: None,
            end_line: None,
            mode: None,
            chunk_size: None,
            continuation_token: None,
        }
    }
}

/// File metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Result of reading a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileResult {
    pub content: String,
    pub total_lines: u32,
    pub truncated: bool,
    pub metadata: FileMetadata,
    pub mode: String,
    pub start_line: u32,
    pub end_line: u32,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_chunk_size: Option<usize>,
}

// ============================================================================
// Write File
// ============================================================================

/// Request for writing a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub create_dirs: Option<bool>,
}

impl WriteFileRequest {
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            create_dirs: None,
        }
    }
}

/// Result of writing a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFileResult {
    pub bytes_written: u64,
    pub metadata: FileMetadata,
}

// ============================================================================
// Edit File
// ============================================================================

/// A single file edit operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    /// The exact text to replace
    pub old_string: String,
    /// The replacement text
    pub new_string: String,
}

/// Request for editing a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFileRequest {
    pub path: String,
    pub edits: Vec<FileEdit>,
}

impl EditFileRequest {
    pub fn new(path: impl Into<String>, edits: Vec<FileEdit>) -> Self {
        Self {
            path: path.into(),
            edits,
        }
    }
}

/// Syntax error in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxIssue {
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: String,
}

/// Validation result for edit operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditValidation {
    pub passed: bool,
    #[serde(default)]
    pub syntax_errors: Vec<SyntaxIssue>,
}

/// Result of editing a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFileResult {
    pub applied: bool,
    pub validation: EditValidation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub bytes_changed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ============================================================================
// Search Content
// ============================================================================

/// Request for searching file content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchContentRequest {
    pub pattern: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub file_glob: Option<String>,
    #[serde(default)]
    pub regex: Option<bool>,
    #[serde(default)]
    pub case_insensitive: Option<bool>,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub context_lines: Option<u32>,
}

impl SearchContentRequest {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            path: None,
            file_glob: None,
            regex: None,
            case_insensitive: None,
            max_results: None,
            context_lines: None,
        }
    }
}

/// A single content match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMatch {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub text: String,
    #[serde(default)]
    pub context: Vec<String>,
}

/// Result of searching content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchContentResult {
    pub matches: Vec<ContentMatch>,
    pub total: usize,
    pub files_scanned: usize,
}

// ============================================================================
// List Files
// ============================================================================

/// Request for listing files
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListFilesRequest {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub glob: Option<String>,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub recursive: Option<bool>,
    #[serde(default)]
    pub max_depth: Option<usize>,
}

/// A single file entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Result of listing files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFilesResult {
    pub files: Vec<FileEntry>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_traversed: Option<usize>,
}
