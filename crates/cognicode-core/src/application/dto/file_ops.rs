//! File Operations DTOs - Transport-neutral types for file operations
//!
//! These DTOs decouple the application layer from the MCP protocol.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

// Import shared types from common.rs
use super::common::{ContentMatch, EditValidation, FileEdit, FileMetadata};

// ============================================================================
// Read File
// ============================================================================

/// Read mode for file content
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ReadMode {
    #[default]
    Raw,
    Outline,
    Symbols,
    Compressed,
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

impl FromStr for ReadMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "raw" => Ok(ReadMode::Raw),
            "outline" => Ok(ReadMode::Outline),
            "symbols" => Ok(ReadMode::Symbols),
            "compressed" => Ok(ReadMode::Compressed),
            _ => Err(format!("Unknown read mode: {}", s)),
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
    pub mode: Option<ReadMode>,
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

// ============================================================================
// Retrieve and Verify
// ============================================================================

/// Verification status for a matched file
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VerificationStatus {
    Verified,
    Rejected,
    Skipped,
}

/// Request for retrieve_and_verify operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveAndVerifyRequest {
    /// Search query string (required)
    pub query: String,
    /// Language filter (reserved, defaults to "rust")
    #[serde(default = "default_rv_language")]
    pub language: String,
    /// Maximum number of results (default: 20)
    #[serde(default = "default_rv_max_results")]
    pub max_results: u32,
    /// Whether to verify via rustc (default: true)
    #[serde(default = "default_rv_verify")]
    pub verify: bool,
}

fn default_rv_language() -> String {
    "rust".to_string()
}

fn default_rv_max_results() -> u32 {
    20
}

fn default_rv_verify() -> bool {
    true
}

/// A single verified match DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedMatchDto {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub matched_text: String,
    pub context: Vec<String>,
    pub status: VerificationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Result of retrieve_and_verify operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveAndVerifyResult {
    pub results: Vec<VerifiedMatchDto>,
    pub total: u32,
    pub verified_count: u32,
    pub rejected_count: u32,
    pub skipped_count: u32,
}
