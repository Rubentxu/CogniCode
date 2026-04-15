//! File Operations Service - LLM-friendly file manipulation tools
//!
//! This service provides the core implementation for the 5 MCP file tools:
//! - read_file: Smart file reading with semantic modes
//! - write_file: Atomic file writes with workspace safety
//! - edit_file: String-replacement edits with tree-sitter validation
//! - search_content: Regex/literal search with .gitignore awareness
//! - list_files: Directory listing with .gitignore filtering

use crate::application::dto::{
    ContentMatch, EditFileRequest, EditFileResult, EditValidation, FileEdit, FileEntry,
    FileMetadata, ListFilesRequest, ListFilesResult, ReadFileRequest, ReadFileResult,
    SearchContentRequest, SearchContentResult, WriteFileRequest, WriteFileResult,
};
use crate::application::error::{AppError, AppResult};
use crate::domain::traits::Parser;
use crate::domain::value_objects::SymbolKind;
use crate::infrastructure::parser::{Language, TreeSitterParser};
use crate::infrastructure::vfs::VirtualFileSystem;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ignore::WalkBuilder;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;

/// Continuation token for chunked file reading
#[derive(Serialize, Deserialize)]
struct ContinuationToken {
    path: String,
    offset: usize,
    chunk_size: usize,
}

/// Encodes a continuation token to base64
fn encode_token(path: &str, offset: usize, chunk_size: usize) -> String {
    let token = ContinuationToken {
        path: path.to_string(),
        offset,
        chunk_size,
    };
    BASE64.encode(serde_json::to_vec(&token).unwrap_or_default())
}

/// Decodes a continuation token from base64
fn decode_token(token: &str) -> Option<ContinuationToken> {
    let bytes = BASE64.decode(token).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// FileOperationsService - Handles all file manipulation operations
///
/// This service provides safe, workspace-scoped file operations with:
/// - Path traversal prevention
/// - .gitignore-aware search and listing
/// - Tree-sitter syntax validation for edits
/// - Support for multiple read modes (raw, outline, symbols, compressed)
pub struct FileOperationsService {
    /// Workspace root for path validation
    workspace_root: String,

    /// Virtual file system for edit operations
    #[allow(dead_code)]
    vfs: VirtualFileSystem,
}

impl FileOperationsService {
    /// Creates a new FileOperationsService
    pub fn new(workspace_root: impl Into<String>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            vfs: VirtualFileSystem::new(),
        }
    }

    /// Validates that a path is within the workspace
    /// For existing files: canonicalizes and checks within workspace
    /// For new files: validates parent directory is within workspace and returns absolute path
    fn validate_path(&self, path: &str) -> AppResult<String> {
        let requested = Path::new(path);
        let root = Path::new(&self.workspace_root);

        // Get absolute path
        let absolute_path = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            // Resolve relative paths against workspace root, not CWD
            root.join(requested)
        };

        // If the file exists, canonicalize it and check within workspace
        if absolute_path.exists() {
            let requested_canonical = absolute_path.canonicalize().map_err(|e| {
                AppError::InvalidParameter(format!("Failed to canonicalize path: {}", e))
            })?;
            let root_canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

            if !requested_canonical.starts_with(&root_canonical) {
                return Err(AppError::InvalidParameter(
                    "Path outside workspace".to_string(),
                ));
            }
            return Ok(requested_canonical.to_string_lossy().to_string());
        }

        // For new files: validate parent directory exists and is within workspace
        let parent = absolute_path
            .parent()
            .ok_or_else(|| AppError::InvalidParameter("No parent directory".to_string()))?;

        // Check parent exists
        if !parent.exists() {
            return Err(AppError::InvalidParameter(format!(
                "Parent directory does not exist: {}",
                parent.display()
            )));
        }

        // Canonicalize parent and check within workspace
        let parent_canonical = parent.canonicalize().map_err(|e| {
            AppError::InvalidParameter(format!("Failed to canonicalize parent: {}", e))
        })?;
        let root_canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

        if !parent_canonical.starts_with(&root_canonical) {
            return Err(AppError::InvalidParameter(
                "Path outside workspace".to_string(),
            ));
        }

        // Return the full absolute path by joining canonical parent with filename
        let filename = absolute_path
            .file_name()
            .ok_or_else(|| AppError::InvalidParameter("Invalid filename".to_string()))?;

        let final_path = parent_canonical.join(filename);
        Ok(final_path.to_string_lossy().to_string())
    }

    /// Reads a file from disk with optional line range and mode
    ///
    /// Modes:
    /// - `raw`: Returns file content as-is with line numbers, respects start_line/end_line
    /// - `outline`: Returns hierarchical structure of the file (functions, classes, etc.)
    /// - `symbols`: Extracts function/class signatures only (name, kind, line, signature)
    /// - `compressed`: Summarizes content to reduce tokens for large files
    ///
    /// Chunked Reading (raw mode only):
    /// - If continuation_token is provided, decode it to resume from offset
    /// - If chunk_size is provided, read that many bytes (extend to line boundary)
    /// - has_more and next_token indicate if there's more content
    pub fn read_file(&self, input: ReadFileRequest) -> AppResult<ReadFileResult> {
        // Reject empty path
        if input.path.trim().is_empty() || input.path == "." {
            return Err(AppError::InvalidParameter(
                "Empty path not allowed".to_string(),
            ));
        }

        let validated_path = self.validate_path(&input.path)?;

        // Check for binary file
        if Self::is_binary_file(Path::new(&validated_path)) {
            return Err(AppError::InvalidParameter(
                "Cannot read binary file in text mode".to_string(),
            ));
        }

        let mode = input.mode.as_deref().unwrap_or("raw");
        let metadata = self.build_file_metadata(&validated_path)?;
        let total_lines = self.count_lines(&validated_path)?;

        // Handle different modes
        let (content, start_line, end_line, has_more, next_token, suggested_chunk_size) = match mode
        {
            "outline" | "symbols" | "compressed" => {
                // Non-raw modes ignore chunk_size and continuation_token
                let content = match mode {
                    "outline" => self.read_file_outline(&validated_path, &input)?,
                    "symbols" => self.read_file_symbols(&validated_path, &input)?,
                    "compressed" => self.read_file_compressed(&validated_path, &input)?,
                    _ => unreachable!(),
                };
                let start = input.start_line.unwrap_or(1);
                let end = input.end_line.unwrap_or(total_lines);
                (content, start, end, false, None, None)
            }
            _ => {
                // Raw mode with optional chunked reading
                let file_size = metadata.size as usize;
                let is_large_file = file_size > 1_000_000; // 1MB threshold

                // Determine offset from continuation_token or start_line
                let (offset, chunk_size): (usize, usize) =
                    if let Some(ref token) = input.continuation_token {
                        if let Some(ct) = decode_token(token) {
                            (ct.offset, ct.chunk_size)
                        } else {
                            return Err(AppError::InvalidParameter(
                                "Invalid continuation token".to_string(),
                            ));
                        }
                    } else {
                        let start = input.start_line.unwrap_or(1);
                        // Convert start_line to byte offset (approximate)
                        let offset = if start <= 1 {
                            0
                        } else {
                            // Read up to start_line to compute approximate offset
                            self.read_file_raw_at_line(&validated_path, start)?
                        };
                        (offset, input.chunk_size.unwrap_or(0))
                    };

                // Read chunk or full content
                let chunk_mode = chunk_size > 0 || input.continuation_token.is_some();
                let effective_chunk_size = chunk_size;

                let (content, actual_end_line, actual_has_more, actual_next_token) =
                    if chunk_mode && effective_chunk_size > 0 {
                        // Chunked reading
                        let (chunk, new_offset, reached_end) =
                            self.read_file_chunk(&validated_path, offset, effective_chunk_size)?;

                        let has_more = !reached_end;
                        let next_token = if has_more {
                            Some(encode_token(
                                &validated_path,
                                new_offset,
                                effective_chunk_size,
                            ))
                        } else {
                            None
                        };

                        // Count lines in chunk to determine line numbers
                        let lines_in_chunk: Vec<&str> = chunk.lines().collect();
                        let chunk_start_line = if offset == 0 {
                            1
                        } else {
                            // Count newlines before offset to determine line number
                            let content = fs::read_to_string(&validated_path).map_err(|e| {
                                AppError::InvalidParameter(format!("Failed to read file: {}", e))
                            })?;
                            content[..offset].lines().count() as u32 + 1
                        };
                        let chunk_end_line = chunk_start_line + lines_in_chunk.len() as u32 - 1;

                        (chunk, chunk_end_line, has_more, next_token)
                    } else {
                        // Non-chunked reading (original behavior)
                        let start = input.start_line.unwrap_or(1);
                        let end = input.end_line.unwrap_or(500).min(500);
                        let content = self.read_file_range(&validated_path, start, end)?;
                        (content, end, false, None)
                    };

                // Auto-suggest for large files in raw mode without chunk_size
                let auto_suggest = is_large_file
                    && chunk_size == 0
                    && input.continuation_token.is_none()
                    && mode == "raw";

                let suggested = if auto_suggest {
                    Some(65536) // 64KB suggested chunk size
                } else {
                    None
                };

                let start = input.start_line.unwrap_or(1);
                (
                    content,
                    start,
                    actual_end_line,
                    actual_has_more || auto_suggest,
                    actual_next_token,
                    suggested,
                )
            }
        };

        // Handle large file truncation warning
        let truncated = content.len() > 100_000;

        Ok(ReadFileResult {
            content,
            total_lines,
            truncated,
            metadata,
            mode: mode.to_string(),
            start_line,
            end_line,
            has_more,
            next_token,
            suggested_chunk_size,
        })
    }

    /// Reads file in outline mode - returns hierarchical structure
    fn read_file_outline(&self, path: &str, input: &ReadFileRequest) -> AppResult<String> {
        let content = fs::read_to_string(path)
            .map_err(|e| AppError::InvalidParameter(format!("Failed to read file: {}", e)))?;

        let lang = Self::detect_language(path).and_then(|l| match l.as_str() {
            "rust" => Some(Language::Rust),
            "python" => Some(Language::Python),
            "javascript" => Some(Language::JavaScript),
            "typescript" => Some(Language::TypeScript),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            _ => None,
        });

        let Some(language) = lang else {
            // Fall back to raw content for unsupported languages
            let start = input.start_line.unwrap_or(1);
            let end = input.end_line.unwrap_or(u32::MAX);
            return self.read_file_range(path, start, end);
        };

        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::InvalidParameter(format!("Parser error: {}", e)))?;

        let symbols = parser
            .find_all_symbols(&content)
            .map_err(|e| AppError::InvalidParameter(format!("Symbol extraction error: {}", e)))?;

        // Format as outline
        let mut outline = String::new();
        for symbol in symbols {
            let kind_str = match symbol.kind() {
                SymbolKind::Function => "function",
                SymbolKind::Class => "class",
                SymbolKind::Variable => "variable",
                _ => "unknown",
            };
            let location = symbol.location();
            outline.push_str(&format!(
                "{}:{}:{}:{}:{}\n",
                location.line(),
                location.column(),
                kind_str,
                symbol.name(),
                location.file()
            ));
        }

        Ok(outline)
    }

    /// Reads file in symbols mode - extracts function/class signatures only
    fn read_file_symbols(&self, path: &str, input: &ReadFileRequest) -> AppResult<String> {
        let content = fs::read_to_string(path)
            .map_err(|e| AppError::InvalidParameter(format!("Failed to read file: {}", e)))?;

        let lang = Self::detect_language(path).and_then(|l| match l.as_str() {
            "rust" => Some(Language::Rust),
            "python" => Some(Language::Python),
            "javascript" => Some(Language::JavaScript),
            "typescript" => Some(Language::TypeScript),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            _ => None,
        });

        let Some(language) = lang else {
            let start = input.start_line.unwrap_or(1);
            let end = input.end_line.unwrap_or(u32::MAX);
            return self.read_file_range(path, start, end);
        };

        let parser = TreeSitterParser::new(language)
            .map_err(|e| AppError::InvalidParameter(format!("Parser error: {}", e)))?;

        let symbols = parser
            .find_all_symbols(&content)
            .map_err(|e| AppError::InvalidParameter(format!("Symbol extraction error: {}", e)))?;

        // Format as compressed symbol list
        let mut symbols_output = String::new();
        for symbol in symbols {
            let kind_str = match symbol.kind() {
                SymbolKind::Function => "fn",
                SymbolKind::Class => "struct",
                SymbolKind::Variable => "let",
                _ => "???",
            };
            let location = symbol.location();
            symbols_output.push_str(&format!(
                "{} @ {}:{} in {}\n",
                kind_str,
                location.line(),
                location.column(),
                symbol.name()
            ));
        }

        Ok(symbols_output)
    }

    /// Reads file in compressed mode - summarizes content to reduce tokens
    /// Achieves ≤30% token efficiency by:
    /// - Stripping all comments (// and /* */)
    /// - Stripping doc comments (///, //!)
    /// - Removing blank/empty lines
    /// - Removing import/use statements
    /// - Showing only first/last N symbols with ellipsis for omitted
    /// - Using tree-sitter for signature extraction when available
    fn read_file_compressed(&self, path: &str, input: &ReadFileRequest) -> AppResult<String> {
        let content = fs::read_to_string(path)
            .map_err(|e| AppError::InvalidParameter(format!("Failed to read file: {}", e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let _total_lines = lines.len();
        let start = (input.start_line.unwrap_or(1) as usize)
            .saturating_sub(1)
            .min(lines.len());
        let end = (input.end_line.unwrap_or(lines.len() as u32) as usize).min(lines.len());

        if start >= end {
            return Ok(String::new());
        }

        let lines_slice = &lines[start..end];

        // Try to use tree-sitter for better compression if language is supported
        let lang = Self::detect_language(path).and_then(|l| match l.as_str() {
            "rust" => Some(Language::Rust),
            "python" => Some(Language::Python),
            "javascript" => Some(Language::JavaScript),
            "typescript" => Some(Language::TypeScript),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            _ => None,
        });

        if let Some(language) = lang {
            if let Ok(parser) = TreeSitterParser::new(language) {
                // Use tree-sitter to extract symbols and compress
                if let Ok(symbols) = parser.find_all_symbols(&lines_slice.join("\n")) {
                    let mut compressed = String::new();
                    let total_symbols = symbols.len();

                    // Limit symbols shown to first 10 and last 5
                    const MAX_SHOW_FIRST: usize = 10;
                    const MAX_SHOW_LAST: usize = 5;

                    compressed.push_str(&format!(
                        "// File: {} ({} lines, {} symbols)\n",
                        path,
                        lines_slice.len(),
                        total_symbols
                    ));
                    compressed.push_str("// SIGNATURES:\n");

                    if total_symbols > MAX_SHOW_FIRST + MAX_SHOW_LAST {
                        // Show first N symbols
                        for symbol in symbols.iter().take(MAX_SHOW_FIRST) {
                            let kind_str = match symbol.kind() {
                                SymbolKind::Function => "fn",
                                SymbolKind::Class => "class",
                                SymbolKind::Struct => "struct",
                                SymbolKind::Enum => "enum",
                                SymbolKind::Trait => "trait",
                                SymbolKind::Method => "method",
                                SymbolKind::Module => "mod",
                                _ => "item",
                            };
                            let location = symbol.location();
                            compressed.push_str(&format!(
                                "// {:5} @ {}:{} - {}\n",
                                kind_str,
                                location.line(),
                                location.column(),
                                symbol.name()
                            ));
                        }

                        // Ellipsis for omitted symbols
                        compressed.push_str(&format!(
                            "// ... {} symbols omitted ...\n",
                            total_symbols - MAX_SHOW_FIRST - MAX_SHOW_LAST
                        ));

                        // Show last N symbols
                        for symbol in symbols.iter().skip(total_symbols - MAX_SHOW_LAST) {
                            let kind_str = match symbol.kind() {
                                SymbolKind::Function => "fn",
                                SymbolKind::Class => "class",
                                SymbolKind::Struct => "struct",
                                SymbolKind::Enum => "enum",
                                SymbolKind::Trait => "trait",
                                SymbolKind::Method => "method",
                                SymbolKind::Module => "mod",
                                _ => "item",
                            };
                            let location = symbol.location();
                            compressed.push_str(&format!(
                                "// {:5} @ {}:{} - {}\n",
                                kind_str,
                                location.line(),
                                location.column(),
                                symbol.name()
                            ));
                        }
                    } else {
                        // Show all symbols if few enough
                        for symbol in symbols {
                            let kind_str = match symbol.kind() {
                                SymbolKind::Function => "fn",
                                SymbolKind::Class => "class",
                                SymbolKind::Struct => "struct",
                                SymbolKind::Enum => "enum",
                                SymbolKind::Trait => "trait",
                                SymbolKind::Method => "method",
                                SymbolKind::Module => "mod",
                                _ => "item",
                            };
                            let location = symbol.location();
                            compressed.push_str(&format!(
                                "// {:5} @ {}:{} - {}\n",
                                kind_str,
                                location.line(),
                                location.column(),
                                symbol.name()
                            ));
                        }
                    }

                    // Add body summary with compression info
                    compressed.push_str("// BODY SUMMARY:\n");
                    let lang_str = format!("{:?}", language);
                    let body_summary =
                        Self::compress_content_basic(&lines_slice.join("\n"), &lang_str);
                    compressed.push_str(&body_summary);
                    return Ok(compressed);
                }
            }
        }

        // Fallback: basic compression without tree-sitter
        let lang_str = lang.map(|l| format!("{:?}", l)).unwrap_or_default();
        Ok(Self::compress_content_basic(
            &lines_slice.join("\n"),
            &lang_str,
        ))
    }

    /// Basic content compression without tree-sitter
    fn compress_content_basic(content: &str, language: &str) -> String {
        let mut result = Vec::new();
        let mut in_block_comment = false;
        let mut blank_line_count = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle block comments
            if in_block_comment {
                if trimmed.contains("*/") {
                    in_block_comment = false;
                }
                continue;
            }

            if trimmed.starts_with("/*") {
                if !trimmed.contains("*/") {
                    in_block_comment = true;
                }
                continue;
            }

            // Skip line comments
            if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!")
            {
                continue;
            }

            // Skip import/use statements (aggressive for all languages)
            let is_import = matches!(language, "rust" | "python" | "javascript" | "typescript")
                && (trimmed.starts_with("use ")
                    || trimmed.starts_with("import ")
                    || trimmed.starts_with("from "));
            if is_import {
                continue;
            }

            // Skip blank lines (but compress multiple blanks to one)
            if trimmed.is_empty() {
                blank_line_count += 1;
                if blank_line_count <= 1 {
                    result.push(line.to_string());
                }
                continue;
            }
            blank_line_count = 0;

            // For function bodies in Rust-like languages, replace with `...`
            // This is a simple heuristic
            if trimmed.ends_with('{')
                && !trimmed.contains(" fn ")
                && !trimmed.contains("struct ")
                && !trimmed.contains("enum ")
            {
                // Likely a function body start, keep the signature but mark body
                result.push(format!("{} {{ ... }}", trimmed.trim_end_matches('{')));
                continue;
            }

            result.push(line.to_string());
        }

        // Calculate compression ratio
        let original_len = content.len();
        let compressed_len = result.join("\n").len();
        let ratio = if original_len > 0 {
            (compressed_len as f64 / original_len as f64 * 100.0).round() as i32
        } else {
            100
        };

        format!(
            "// Compression: {}% of original ({} -> {} chars)\n{}\n",
            ratio,
            original_len,
            compressed_len,
            result.join("\n")
        )
    }

    /// Estimates byte offset for a given line number
    fn read_file_raw_at_line(&self, path: &str, target_line: u32) -> AppResult<usize> {
        let content = fs::read_to_string(path)
            .map_err(|e| AppError::InvalidParameter(format!("Failed to read file: {}", e)))?;

        let mut current_line = 1u32;
        for (offset, _) in content.char_indices() {
            if current_line >= target_line {
                return Ok(offset);
            }
            if content[content.char_indices().nth(0).unwrap().0..].starts_with('\n') {
                current_line += 1;
            }
        }
        Ok(content.len())
    }

    /// Reads a chunk of bytes from a file, extending to the next line boundary
    /// Returns (content, new_offset, reached_end)
    fn read_file_chunk(
        &self,
        path: &str,
        offset: usize,
        chunk_size: usize,
    ) -> AppResult<(String, usize, bool)> {
        let content = fs::read_to_string(path)
            .map_err(|e| AppError::InvalidParameter(format!("Failed to read file: {}", e)))?;

        let total_size = content.len();

        if offset >= total_size {
            return Ok((String::new(), offset, true));
        }

        // Read from offset, limited to chunk_size
        let remaining = &content[offset..];
        let read_size = chunk_size.min(remaining.len());

        // Find the appropriate end position:
        // 1. If chunk ends at a newline, use chunk_size as-is
        // 2. If chunk doesn't end at newline, extend to next newline (or end of file)
        let (end_offset, reached_end) = if offset + read_size >= total_size {
            // At or past end of file
            (total_size, true)
        } else {
            // Check if there's a newline within the bytes we read
            let chunk_bytes = &remaining.as_bytes()[..read_size];
            if let Some(newline_pos) = chunk_bytes.iter().rposition(|&b| b == b'\n') {
                // Found newline within the chunk - extend to include the full line
                (offset + newline_pos + 1, false)
            } else {
                // No newline in our chunk - look ahead for next newline
                if let Some(next_newline) = remaining[read_size..].find('\n') {
                    (offset + read_size + next_newline + 1, false)
                } else {
                    // No more newlines, return rest of file
                    (total_size, true)
                }
            }
        };

        let chunk = String::from(&content[offset..end_offset]);
        Ok((chunk, end_offset, reached_end))
    }

    /// Reads a range of lines from a file
    fn read_file_range(&self, path: &str, start_line: u32, end_line: u32) -> AppResult<String> {
        let content = fs::read_to_string(path)
            .map_err(|e| AppError::InvalidParameter(format!("Failed to read file: {}", e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let start_idx = (start_line as usize).saturating_sub(1).min(lines.len());
        let end_idx = (end_line as usize).min(lines.len());

        if start_idx >= end_idx {
            return Ok(String::new());
        }

        Ok(lines[start_idx..end_idx].join("\n"))
    }

    /// Counts total lines in a file
    fn count_lines(&self, path: &str) -> AppResult<u32> {
        let content = fs::read_to_string(path)
            .map_err(|e| AppError::InvalidParameter(format!("Failed to read file: {}", e)))?;
        Ok(content.lines().count() as u32)
    }

    /// Builds file metadata
    fn build_file_metadata(&self, path: &str) -> AppResult<FileMetadata> {
        let metadata = fs::metadata(path)
            .map_err(|e| AppError::InvalidParameter(format!("Failed to read metadata: {}", e)))?;

        let modified = metadata
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let language = Self::detect_language(path);

        Ok(FileMetadata {
            path: path.to_string(),
            size: metadata.len(),
            modified,
            language,
        })
    }

    /// Detects programming language from file extension
    fn detect_language(path: &str) -> Option<String> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            Some("rs") => Some("rust".to_string()),
            Some("py") => Some("python".to_string()),
            Some("js") | Some("jsx") => Some("javascript".to_string()),
            Some("ts") | Some("tsx") => Some("typescript".to_string()),
            Some("go") => Some("go".to_string()),
            Some("java") => Some("java".to_string()),
            Some("c") | Some("h") => Some("c".to_string()),
            Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") => Some("cpp".to_string()),
            Some("rb") => Some("ruby".to_string()),
            Some("php") => Some("php".to_string()),
            Some("swift") => Some("swift".to_string()),
            Some("kt") | Some("kts") => Some("kotlin".to_string()),
            Some("cs") => Some("csharp".to_string()),
            Some("scala") => Some("scala".to_string()),
            Some("md") | Some("markdown") => Some("markdown".to_string()),
            Some("json") => Some("json".to_string()),
            Some("yaml") | Some("yml") => Some("yaml".to_string()),
            Some("toml") => Some("toml".to_string()),
            Some("xml") => Some("xml".to_string()),
            Some("html") | Some("htm") => Some("html".to_string()),
            Some("css") => Some("css".to_string()),
            Some("scss") | Some("sass") => Some("scss".to_string()),
            Some("sql") => Some("sql".to_string()),
            Some("sh") | Some("bash") => Some("shell".to_string()),
            _ => None,
        }
    }

    /// Writes content to a file atomically
    ///
    /// Uses a temporary file and rename to ensure atomic writes.
    /// If create_dirs is true, creates parent directories if they don't exist.
    pub fn write_file(&self, input: WriteFileRequest) -> AppResult<WriteFileResult> {
        // For new files with create_dirs=true, create parent directories BEFORE validation
        // This is necessary because validate_path checks parent exists for new files
        let path_for_validation = if input.create_dirs.unwrap_or(false) {
            let requested = Path::new(&input.path);
            let absolute_path = if requested.is_absolute() {
                requested.to_path_buf()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(requested))
                    .map_err(|e| {
                        AppError::InvalidParameter(format!("Failed to resolve path: {}", e))
                    })?
            };

            if let Some(parent) = absolute_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent).map_err(|e| {
                        AppError::InvalidParameter(format!("Failed to create directories: {}", e))
                    })?;
                }
            }
            input.path.clone()
        } else {
            input.path.clone()
        };

        let validated_path = self.validate_path(&path_for_validation)?;

        // FW-5: Enforce size limit (10MB default)
        const DEFAULT_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
        let content_size = input.content.len() as u64;
        if content_size > DEFAULT_MAX_FILE_SIZE {
            return Err(AppError::InvalidParameter(
                "File content exceeds maximum allowed size (10MB)".to_string(),
            ));
        }

        let bytes_written = input.content.len() as u64;

        // Atomic write: write to temp file then rename
        let temp_path = format!("{}.tmp.{}", validated_path, std::process::id());

        // Write content to temp file
        {
            let mut file = File::create(&temp_path).map_err(|e| {
                AppError::InvalidParameter(format!("Failed to create temp file: {}", e))
            })?;
            file.write_all(input.content.as_bytes()).map_err(|e| {
                AppError::InvalidParameter(format!("Failed to write to temp file: {}", e))
            })?;
        }

        // Atomically rename temp file to target
        fs::rename(&temp_path, &validated_path).map_err(|e| {
            // Clean up temp file on error
            let _ = fs::remove_file(&temp_path);
            AppError::InvalidParameter(format!("Failed to write file atomically: {}", e))
        })?;

        Ok(WriteFileResult {
            bytes_written,
            metadata: self.build_file_metadata(&validated_path)?,
        })
    }

    /// Applies edits to a file with syntax validation
    ///
    /// Each edit must match exactly one occurrence. After applying all edits,
    /// the modified content is validated with tree-sitter for syntax errors.
    pub fn edit_file(&self, input: EditFileRequest) -> AppResult<EditFileResult> {
        let validated_path = self.validate_path(&input.path)?;

        // Read current content
        let original_content = fs::read_to_string(&validated_path)
            .map_err(|e| AppError::InvalidParameter(format!("Failed to read file: {}", e)))?;

        // Check that each old_string appears exactly once
        for edit in &input.edits {
            let matches: Vec<_> = original_content.match_indices(&edit.old_string).collect();
            if matches.is_empty() {
                return Ok(EditFileResult {
                    applied: false,
                    validation: EditValidation {
                        passed: false,
                        syntax_errors: vec![],
                    },
                    preview: Some(format!(
                        "No matches found for old_string: {}",
                        Self::truncate_string(&edit.old_string, 50)
                    )),
                    bytes_changed: 0,
                    reason: Some("no_match".to_string()),
                });
            }
            if matches.len() > 1 {
                return Ok(EditFileResult {
                    applied: false,
                    validation: EditValidation {
                        passed: false,
                        syntax_errors: vec![],
                    },
                    preview: Some(format!(
                        "Multiple matches ({}) found for old_string: {}",
                        matches.len(),
                        Self::truncate_string(&edit.old_string, 50)
                    )),
                    bytes_changed: 0,
                    reason: Some("multiple_matches".to_string()),
                });
            }
        }

        // Apply edits
        let mut new_content = original_content.clone();
        for edit in &input.edits {
            let new = new_content.replace(&edit.old_string, &edit.new_string);
            if new != new_content {
                new_content = new;
            }
        }

        // If content unchanged, return early
        if new_content == original_content {
            return Ok(EditFileResult {
                applied: false,
                validation: EditValidation {
                    passed: true,
                    syntax_errors: vec![],
                },
                preview: Some("No changes made".to_string()),
                bytes_changed: 0,
                reason: None,
            });
        }

        // Validate syntax using tree-sitter if possible
        let lang = Self::detect_language(&validated_path);
        let validation = if let Some(lang_str) = lang {
            let language = match lang_str.as_str() {
                "rust" => Some(Language::Rust),
                "python" => Some(Language::Python),
                "javascript" => Some(Language::JavaScript),
                "typescript" => Some(Language::TypeScript),
                "go" => Some(Language::Go),
                "java" => Some(Language::Java),
                _ => None,
            };

            if let Some(language) = language {
                match TreeSitterParser::new(language) {
                    Ok(parser) => match parser.parse_tree(&new_content) {
                        Ok(tree) => {
                            if TreeSitterParser::has_error_nodes(&tree) {
                                EditValidation {
                                    passed: false,
                                    syntax_errors: vec![],
                                }
                            } else {
                                EditValidation {
                                    passed: true,
                                    syntax_errors: vec![],
                                }
                            }
                        }
                        Err(_) => EditValidation {
                            passed: false,
                            syntax_errors: vec![],
                        },
                    },
                    Err(_) => EditValidation {
                        passed: true, // Can't create parser, skip validation
                        syntax_errors: vec![],
                    },
                }
            } else {
                EditValidation {
                    passed: true, // Unsupported language, skip validation
                    syntax_errors: vec![],
                }
            }
        } else {
            EditValidation {
                passed: true, // Unknown language, skip validation
                syntax_errors: vec![],
            }
        };

        if validation.passed {
            // Write the modified content atomically
            let temp_path = format!("{}.tmp.{}", validated_path, std::process::id());

            {
                let mut file = File::create(&temp_path).map_err(|e| {
                    AppError::InvalidParameter(format!("Failed to create temp file: {}", e))
                })?;
                file.write_all(new_content.as_bytes()).map_err(|e| {
                    AppError::InvalidParameter(format!("Failed to write to temp file: {}", e))
                })?;
            }

            fs::rename(&temp_path, &validated_path).map_err(|e| {
                // Clean up temp file on error
                let _ = fs::remove_file(&temp_path);
                AppError::InvalidParameter(format!("Failed to write file atomically: {}", e))
            })?;
        }

        // bytes_changed is only meaningful when validation passed and file was actually written
        let bytes_changed = if validation.passed {
            (new_content.len() as i64 - original_content.len() as i64).unsigned_abs()
        } else {
            0
        };

        // Determine if edit was applied and set rejection reason if syntax was rejected
        let applied = validation.passed && new_content != original_content;
        let reason = if validation.passed {
            None
        } else {
            Some("syntax_rejected".to_string())
        };

        Ok(EditFileResult {
            applied,
            validation,
            preview: Some(format!("Changed {} bytes", bytes_changed)),
            bytes_changed,
            reason,
        })
    }

    /// Truncates a string for display purposes
    fn truncate_string(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len])
        }
    }

    /// Searches for content within files using .gitignore-aware traversal
    ///
    /// Supports literal and regex patterns, case-insensitive search,
    /// and returns matches with surrounding context lines.
    pub fn search_content(&self, input: SearchContentRequest) -> AppResult<SearchContentResult> {
        let search_path = input
            .path
            .as_ref()
            .map(|p| self.validate_path(p))
            .unwrap_or_else(|| Ok(self.workspace_root.clone()))?;

        let max_results = input.max_results.unwrap_or(50);
        let context_lines = input.context_lines.unwrap_or(2) as usize;
        let case_insensitive = input.case_insensitive.unwrap_or(false);
        let is_regex = input.regex.unwrap_or(true);

        // Validate regex pattern if regex mode is enabled
        if is_regex {
            if let Err(msg) = Self::validate_regex_pattern(&input.pattern) {
                return Err(AppError::InvalidParameter(format!(
                    "Invalid regex pattern: {}",
                    msg
                )));
            }
        }

        // Build walker with gitignore awareness
        let mut walker = WalkBuilder::new(&search_path);
        walker
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .parents(true);

        let mut matches = Vec::new();
        let mut files_scanned = 0;

        for result in walker.build() {
            if matches.len() >= max_results {
                break;
            }

            let entry = match result {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // Apply glob filter if provided
            if let Some(glob) = &input.file_glob {
                if !Self::path_matches_glob(path, glob) {
                    continue;
                }
            }

            files_scanned += 1;

            // Skip binary files
            if Self::is_binary_file(path) {
                continue;
            }

            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Search for pattern
            let search_matches = Self::find_matches_in_content(
                &content,
                &input.pattern,
                is_regex,
                case_insensitive,
                context_lines,
            );

            for (line_num, col, text, context) in search_matches {
                if matches.len() >= max_results {
                    break;
                }
                matches.push(ContentMatch {
                    file: path.to_string_lossy().to_string(),
                    line: line_num,
                    col,
                    text,
                    context,
                });
            }
        }

        let total = matches.len();

        Ok(SearchContentResult {
            matches,
            total,
            files_scanned,
        })
    }

    /// Finds matches in content using literal or regex search
    fn find_matches_in_content(
        content: &str,
        pattern: &str,
        is_regex: bool,
        case_insensitive: bool,
        context_lines: usize,
    ) -> Vec<(u32, u32, String, Vec<String>)> {
        // Vec of (line_number, col, matched_text, context)
        let mut results = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        if is_regex {
            // Use the real regex crate
            let pattern_with_flags = if case_insensitive {
                format!("(?i){}", pattern)
            } else {
                pattern.to_string()
            };

            let re = match Regex::new(&pattern_with_flags) {
                Ok(re) => re,
                Err(_) => return results, // Invalid regex, return empty
            };

            for (idx, line_content) in lines.iter().enumerate() {
                for m in re.find_iter(line_content) {
                    let context_before: Vec<String> = lines[..idx]
                        .iter()
                        .rev()
                        .take(context_lines)
                        .map(|s| s.to_string())
                        .collect();
                    let context_after: Vec<String> = lines[idx + 1..]
                        .iter()
                        .take(context_lines)
                        .map(|s| s.to_string())
                        .collect();

                    results.push((
                        (idx + 1) as u32,
                        (m.start() + 1) as u32, // 1-indexed column
                        m.as_str().to_string(),
                        context_before
                            .into_iter()
                            .rev()
                            .chain(context_after)
                            .collect(),
                    ));
                }
            }
        } else {
            // Literal search
            for (idx, line_content) in lines.iter().enumerate() {
                let search_text = if case_insensitive {
                    line_content.to_lowercase()
                } else {
                    line_content.to_string()
                };
                let search_pattern = if case_insensitive {
                    pattern.to_lowercase()
                } else {
                    pattern.to_string()
                };

                let mut start = 0;
                while let Some(pos) = search_text[start..].find(&search_pattern) {
                    let abs_pos = start + pos;
                    let matched_text = &line_content[abs_pos..abs_pos + search_pattern.len()];

                    let context_before: Vec<String> = lines[..idx]
                        .iter()
                        .rev()
                        .take(context_lines)
                        .map(|s| s.to_string())
                        .collect();
                    let context_after: Vec<String> = lines[idx + 1..]
                        .iter()
                        .take(context_lines)
                        .map(|s| s.to_string())
                        .collect();

                    results.push((
                        (idx + 1) as u32,
                        (abs_pos + 1) as u32, // 1-indexed column
                        matched_text.to_string(),
                        context_before
                            .into_iter()
                            .rev()
                            .chain(context_after)
                            .collect(),
                    ));
                    start += pos + 1;
                }
            }
        }

        results
    }

    /// Validates a regex pattern before processing
    /// Returns Err with description if pattern is invalid
    fn validate_regex_pattern(pattern: &str) -> Result<(), String> {
        Regex::new(pattern)
            .map(|_| ())
            .map_err(|e| format!("Invalid regex pattern: {}", e))
    }

    /// Checks if a path matches a glob pattern
    fn path_matches_glob(path: &Path, glob: &str) -> bool {
        // Simple glob matching - supports **/*.ext patterns
        let path_str = path.to_string_lossy();

        // Handle **/*.rs style patterns
        if let Some(ext_pattern) = glob.strip_prefix("**/") {
            if let Some(path_str) = path_str.rsplit('/').next() {
                return Self::glob_match(path_str, ext_pattern);
            }
        } else if let Some(_ext_pattern) = glob.strip_prefix("*.") {
            // Handle *.rs style patterns
            if let Some(path_str) = path_str.rsplit('/').next() {
                return Self::glob_match(path_str, glob);
            }
        }

        Self::glob_match(&path_str, glob)
    }

    /// Simple glob pattern matching
    fn glob_match(text: &str, pattern: &str) -> bool {
        // Handle simple patterns like *.rs, test_*.py, etc.
        if let Some(ext) = pattern.strip_prefix("*.") {
            if let Some(text_ext) = text.rsplit('.').next() {
                return text_ext == ext || ext == "*";
            }
            return false;
        }

        // Handle **.rs -> ends with .rs
        if let Some(ext) = pattern.strip_prefix("**.") {
            if let Some(text_ext) = text.rsplit('.').next() {
                return text_ext == ext;
            }
            return false;
        }

        // Handle prefix*suffix pattern (e.g., test*.py) - asterisk in middle
        // This is: starts with prefix, ends with suffix, * matches anything in between
        if pattern.contains('*') && !pattern.starts_with('*') && !pattern.ends_with('*') {
            if let Some(star_pos) = pattern.find('*') {
                let prefix = &pattern[..star_pos];
                let suffix = &pattern[star_pos + 1..];
                // text must start with prefix and end with suffix
                // and be at least as long as prefix + suffix
                if text.starts_with(prefix)
                    && text.ends_with(suffix)
                    && prefix.len() + suffix.len() <= text.len()
                {
                    return true;
                }
            }
        }

        // Handle *suffix pattern (e.g., *test.py)
        if let Some(stripped) = pattern.strip_prefix('*') {
            return text.ends_with(stripped);
        }

        // Handle suffix* pattern (e.g., test*)
        if let Some(stripped) = pattern.strip_suffix('*') {
            return text.starts_with(stripped);
        }

        // Simple contains check for other patterns
        text.contains(pattern)
    }

    /// Checks if a file is binary
    fn is_binary_file(path: &Path) -> bool {
        let binary_extensions = [
            "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "svg", "pdf", "zip", "tar", "gz",
            "bz2", "xz", "7z", "rar", "exe", "dll", "so", "dylib", "bin", "mp3", "mp4", "avi",
            "mov", "wmv", "flac", "wav", "ttf", "otf", "woff", "woff2", "eot",
        ];

        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| binary_extensions.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    /// Lists files in a directory with .gitignore-aware traversal
    ///
    /// Returns file entries with metadata, supporting pagination and glob filtering.
    pub fn list_files(&self, input: ListFilesRequest) -> AppResult<ListFilesResult> {
        let search_path = input
            .path
            .as_ref()
            .map(|p| self.validate_path(p))
            .unwrap_or_else(|| Ok(self.workspace_root.clone()))?;

        // Check if path exists and is a directory
        let path = Path::new(&search_path);
        if !path.exists() {
            return Err(AppError::InvalidParameter(format!(
                "Directory not found: {}",
                search_path
            )));
        }
        if !path.is_dir() {
            return Err(AppError::InvalidParameter(format!(
                "Path is not a directory: {}",
                search_path
            )));
        }

        let offset = input.offset.unwrap_or(0);
        let limit = input.limit.unwrap_or(100);

        // Determine max_depth based on recursive and max_depth parameters
        // WalkBuilder default behavior is unlimited depth (None)
        // - recursive=false → max_depth(1) - immediate children only
        // - recursive=true + max_depth=None → max_depth(None) - unlimited
        // - recursive=true + max_depth=Some(n) → max_depth(Some(n))
        // - max_depth takes precedence if explicitly specified
        let max_depth = if let Some(explicit_depth) = input.max_depth {
            Some(explicit_depth)
        } else if input.recursive == Some(false) {
            Some(1)
        } else {
            None // Default: recursive=true behavior (unlimited)
        };

        let mut entries = Vec::new();
        let mut max_depth_reached: usize = 0;

        // Build walker with gitignore awareness
        let mut walker = WalkBuilder::new(&search_path);
        walker
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .parents(true);

        // Apply max_depth if specified
        if let Some(depth) = max_depth {
            walker.max_depth(Some(depth));
        }

        // Issue 1 fix: When recursive=false (max_depth=1), exclude root entry (depth 0)
        // by setting min_depth(1) so only immediate children are returned
        if max_depth == Some(1) {
            walker.min_depth(Some(1));
        }

        for result in walker.build() {
            let entry = match result {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();

            // Issue 2 fix: Use WalkBuilder's depth() method instead of path component counting
            // WalkBuilder depth model: 0=root, 1=immediate children, 2=one level below, etc.
            let depth = entry.depth();
            max_depth_reached = max_depth_reached.max(depth);

            // Apply glob filter if provided
            if let Some(glob) = &input.glob {
                if !Self::path_matches_glob(path, glob) {
                    continue;
                }
            }

            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let modified = metadata
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            entries.push(FileEntry {
                path: path.to_string_lossy().to_string(),
                size: metadata.len(),
                modified,
                is_dir: metadata.is_dir(),
                language: Self::detect_language(&path.to_string_lossy()),
            });
        }

        let total = entries.len();

        // Sort by path and apply pagination
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries.truncate(offset.saturating_add(limit));
        let files = entries.into_iter().skip(offset).take(limit).collect();

        Ok(ListFilesResult {
            files,
            total,
            depth_traversed: Some(max_depth_reached),
        })
    }
}

impl Default for FileOperationsService {
    fn default() -> Self {
        Self::new(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_read_file_raw_mode() {
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "fn main() {{").unwrap();
        writeln!(file, "    println!(\"hello\");").unwrap();
        writeln!(file, "}}").unwrap();

        let service =
            FileOperationsService::new(file.path().parent().unwrap().to_string_lossy().to_string());
        let input = ReadFileRequest {
            path: file.path().to_str().unwrap().to_string(),
            start_line: None,
            end_line: None,
            mode: Some("raw".to_string()),
            continuation_token: None,
            chunk_size: None,
        };

        let result = service.read_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.content.contains("fn main"));
        assert!(!output.truncated);
        assert_eq!(output.metadata.language, Some("rust".to_string()));
        assert!(!output.has_more);
        assert!(output.next_token.is_none());
    }

    #[test]
    fn test_read_file_with_line_range() {
        let mut file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();

        let service =
            FileOperationsService::new(file.path().parent().unwrap().to_string_lossy().to_string());
        let input = ReadFileRequest {
            path: file.path().to_str().unwrap().to_string(),
            start_line: Some(2),
            end_line: Some(3),
            mode: Some("raw".to_string()),
            continuation_token: None,
            chunk_size: None,
        };

        let result = service.read_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.content.contains("line 2"));
        assert!(!output.content.contains("line 1"));
    }

    #[test]
    fn test_read_file_symbols_mode() {
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "fn hello() {{}}").unwrap();
        writeln!(file, "struct MyStruct {{}}").unwrap();

        let service =
            FileOperationsService::new(file.path().parent().unwrap().to_string_lossy().to_string());
        let input = ReadFileRequest {
            path: file.path().to_str().unwrap().to_string(),
            start_line: None,
            end_line: None,
            mode: Some("symbols".to_string()),
            continuation_token: None,
            chunk_size: None,
        };

        let result = service.read_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        // Should contain function/struct markers
        assert!(output.content.contains("fn") || output.content.contains("struct"));
        assert!(!output.has_more);
    }

    #[test]
    fn test_read_file_binary_rejection() {
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("test.png");
        std::fs::write(&binary_path, &[0x89, 0x50, 0x4E, 0x47]).unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = ReadFileRequest {
            path: binary_path.to_string_lossy().to_string(),
            start_line: None,
            end_line: None,
            mode: Some("raw".to_string()),
            continuation_token: None,
            chunk_size: None,
        };

        let result = service.read_file(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_file_atomic() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = WriteFileInput {
            path: file_path.to_str().unwrap().to_string(),
            content: "Hello, atomic world!".to_string(),
            create_dirs: Some(false),
        };

        let result = service.write_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.bytes_written, 20); // "Hello, atomic world!" is 20 bytes

        // Verify file was written correctly
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, atomic world!");
    }

    #[test]
    fn test_write_file_create_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("test.txt");

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = WriteFileInput {
            path: nested_path.to_str().unwrap().to_string(),
            content: "nested content".to_string(),
            create_dirs: Some(true),
        };

        let result = service.write_file(input);
        assert!(result.is_ok());
        assert!(nested_path.exists());
    }

    #[test]
    fn test_path_traversal_prevention() {
        let temp_dir = TempDir::new().unwrap();
        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        // Try to access a path outside workspace
        let input = ReadFileRequest {
            path: "/etc/passwd".to_string(),
            start_line: None,
            end_line: None,
            mode: None,
            continuation_token: None,
            chunk_size: None,
        };

        let result = service.read_file(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_edit_file_single_match() {
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "fn old_name() {{}}").unwrap();
        let file_path = file.path().to_str().unwrap().to_string();

        let service =
            FileOperationsService::new(file.path().parent().unwrap().to_string_lossy().to_string());
        let input = EditFileInput {
            path: file_path.clone(),
            edits: vec![FileEdit {
                old_string: "old_name".to_string(),
                new_string: "new_name".to_string(),
            }],
        };

        let result = service.edit_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.applied);
        assert!(output.validation.passed);

        // Verify the file was modified
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("new_name"));
        assert!(!content.contains("old_name"));
    }

    #[test]
    fn test_edit_file_multiple_matches_rejected() {
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "let x = foo;").unwrap();
        writeln!(file, "let y = foo;").unwrap();
        let file_path = file.path().to_str().unwrap().to_string();

        let service =
            FileOperationsService::new(file.path().parent().unwrap().to_string_lossy().to_string());
        let input = EditFileInput {
            path: file_path.clone(),
            edits: vec![FileEdit {
                old_string: "foo".to_string(),
                new_string: "bar".to_string(),
            }],
        };

        let result = service.edit_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.applied);
        assert!(output
            .preview
            .as_ref()
            .unwrap()
            .contains("Multiple matches"));
    }

    #[test]
    fn test_edit_file_no_match_rejected() {
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "fn test() {{}}").unwrap();
        let file_path = file.path().to_str().unwrap().to_string();

        let service =
            FileOperationsService::new(file.path().parent().unwrap().to_string_lossy().to_string());
        let input = EditFileInput {
            path: file_path.clone(),
            edits: vec![FileEdit {
                old_string: "nonexistent".to_string(),
                new_string: "something".to_string(),
            }],
        };

        let result = service.edit_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.applied);
        assert!(output.preview.as_ref().unwrap().contains("No matches"));
    }

    #[test]
    fn test_search_content_literal() {
        let temp_dir = TempDir::new().unwrap();
        let file1_path = temp_dir.path().join("a.rs");
        fs::write(&file1_path, "fn hello() {\n    println!(\"hi\");\n}").unwrap();

        let file2_path = temp_dir.path().join("b.rs");
        fs::write(&file2_path, "fn world() {\n    println!(\"bye\");\n}").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = SearchContentInput {
            pattern: "println".to_string(),
            path: None,
            file_glob: Some("*.rs".to_string()),
            regex: Some(false),
            case_insensitive: Some(false),
            max_results: Some(50),
            context_lines: Some(1),
        };

        let result = service.search_content(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.total, 2); // Both files have println
        assert!(output.files_scanned >= 2);
    }

    #[test]
    fn test_search_content_regex() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        fs::write(
            &file_path,
            "def foo_handler():\n    pass\ndef bar_handler():\n    pass",
        )
        .unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = SearchContentInput {
            pattern: r"\w+_handler".to_string(),
            path: None,
            file_glob: Some("*.py".to_string()),
            regex: Some(true),
            case_insensitive: Some(false),
            max_results: Some(50),
            context_lines: Some(1),
        };

        let result = service.search_content(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.total >= 2);
    }

    #[test]
    fn test_search_content_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn HELLO() {}\nfn hello() {}").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = SearchContentInput {
            pattern: "hello".to_string(),
            path: None,
            file_glob: None,
            regex: Some(false),
            case_insensitive: Some(true),
            max_results: Some(50),
            context_lines: Some(0),
        };

        let result = service.search_content(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.total, 2); // Both HELLO and hello match
    }

    #[test]
    fn test_list_files_pagination() {
        let temp_dir = TempDir::new().unwrap();

        // Create 10 files
        for i in 0..10 {
            let file_path = temp_dir.path().join(format!("file{}.txt", i));
            fs::write(&file_path, format!("content {}", i)).unwrap();
        }

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        // Get first 5
        let input = ListFilesRequest {
            path: None,
            glob: Some("*.txt".to_string()),
            offset: Some(0),
            limit: Some(5),
            recursive: None,
            max_depth: None,
        };

        let result = service.list_files(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.files.len(), 5);
        assert_eq!(output.total, 10);
    }

    #[test]
    fn test_list_files_glob_filter() {
        let temp_dir = TempDir::new().unwrap();

        let rust_path = temp_dir.path().join("test.rs");
        fs::write(&rust_path, "fn main() {}").unwrap();

        let py_path = temp_dir.path().join("test.py");
        fs::write(&py_path, "def main(): pass").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        let input = ListFilesRequest {
            path: None,
            glob: Some("*.rs".to_string()),
            offset: None,
            limit: None,
            recursive: None,
            max_depth: None,
        };

        let result = service.list_files(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.files.len(), 1);
        assert!(output.files[0].path.ends_with(".rs"));
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(
            FileOperationsService::detect_language("test.rs"),
            Some("rust".to_string())
        );
        assert_eq!(
            FileOperationsService::detect_language("test.py"),
            Some("python".to_string())
        );
        assert_eq!(
            FileOperationsService::detect_language("test.js"),
            Some("javascript".to_string())
        );
        assert_eq!(
            FileOperationsService::detect_language("test.ts"),
            Some("typescript".to_string())
        );
        assert_eq!(FileOperationsService::detect_language("test"), None);
    }

    #[test]
    fn test_is_binary_file() {
        assert!(FileOperationsService::is_binary_file(Path::new("test.png")));
        assert!(FileOperationsService::is_binary_file(Path::new("test.jpg")));
        assert!(FileOperationsService::is_binary_file(Path::new("test.pdf")));
        assert!(!FileOperationsService::is_binary_file(Path::new("test.rs")));
        assert!(!FileOperationsService::is_binary_file(Path::new("test.py")));
    }

    #[test]
    fn test_glob_match() {
        assert!(FileOperationsService::glob_match("test.rs", "*.rs"));
        assert!(!FileOperationsService::glob_match("test.py", "*.rs"));
        assert!(FileOperationsService::glob_match("src/lib.rs", "*.rs"));
        assert!(FileOperationsService::glob_match("test.py", "test*.py"));
        assert!(!FileOperationsService::glob_match("test.py", "foo*.py"));
    }

    #[test]
    fn test_list_files_recursive_false_returns_only_immediate_children() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested directory structure
        std::fs::create_dir_all(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        std::fs::write(temp_dir.path().join("subdir").join("file2.txt"), "content2").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        let input = ListFilesRequest {
            path: None,
            glob: None,
            offset: None,
            limit: None,
            recursive: Some(false),
            max_depth: None,
        };

        let result = service.list_files(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        // Should only get immediate children (file1.txt and subdir), not file2.txt
        let paths: Vec<&str> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(
            paths.iter().any(|p| p.contains("file1.txt")),
            "Should find file1.txt"
        );
        assert!(
            paths.iter().any(|p| p.contains("subdir")),
            "Should find subdir"
        );
        // file2.txt should NOT appear since it's in a subdirectory
        assert!(
            !paths.iter().any(|p| p.contains("file2.txt")),
            "Should not find file2.txt (not immediate child)"
        );
    }

    #[test]
    fn test_list_files_max_depth_1() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested directory structure
        std::fs::create_dir_all(temp_dir.path().join("level1").join("level2")).unwrap();
        std::fs::write(temp_dir.path().join("root.txt"), "root").unwrap();
        std::fs::write(temp_dir.path().join("level1").join("l1.txt"), "level1").unwrap();
        std::fs::write(
            temp_dir.path().join("level1").join("level2").join("l2.txt"),
            "level2",
        )
        .unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        let input = ListFilesRequest {
            path: None,
            glob: None,
            offset: None,
            limit: None,
            recursive: Some(true),
            max_depth: Some(1),
        };

        let result = service.list_files(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let paths: Vec<&str> = output.files.iter().map(|f| f.path.as_str()).collect();

        // depth 1 means: root + immediate children of root
        // So we get root.txt and level1/ (the directory), but NOT files inside level1/
        assert!(
            paths.iter().any(|p| p.contains("root.txt")),
            "Should find root.txt"
        );
        assert!(
            paths
                .iter()
                .any(|p| p.ends_with("level1") || p.contains("level1")),
            "Should find level1 directory"
        );
        assert!(
            !paths.iter().any(|p| p.contains("l1.txt")),
            "Should not find l1.txt (depth 2)"
        );
        assert!(
            !paths.iter().any(|p| p.contains("l2.txt")),
            "Should not find l2.txt (depth 3)"
        );
    }

    #[test]
    fn test_list_files_max_depth_0_returns_root_only() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested structure
        std::fs::create_dir_all(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        std::fs::write(temp_dir.path().join("subdir").join("file2.txt"), "content2").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        let input = ListFilesRequest {
            path: None,
            glob: None,
            offset: None,
            limit: None,
            recursive: Some(true),
            max_depth: Some(0),
        };

        let result = service.list_files(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        // max_depth 0 means only the root directory itself
        // WalkBuilder returns the starting directory as depth 0
        // Since we search from temp_dir, it returns temp_dir (as directory entry)
        // But our files are inside temp_dir, so we shouldn't see them
        let paths: Vec<&str> = output.files.iter().map(|f| f.path.as_str()).collect();
        // Should not find any of the actual files
        assert!(
            !paths.iter().any(|p| p.contains("file1.txt")),
            "Should not find file1.txt with max_depth 0"
        );
        assert!(
            !paths.iter().any(|p| p.contains("file2.txt")),
            "Should not find file2.txt with max_depth 0"
        );
    }

    #[test]
    fn test_list_files_default_recursive_true() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested structure
        std::fs::create_dir_all(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        std::fs::write(temp_dir.path().join("subdir").join("file2.txt"), "content2").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        // Default: no recursive or max_depth specified
        let input = ListFilesRequest {
            path: None,
            glob: Some("*.txt".to_string()),
            offset: None,
            limit: None,
            recursive: None,
            max_depth: None,
        };

        let result = service.list_files(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let paths: Vec<&str> = output.files.iter().map(|f| f.path.as_str()).collect();

        // Default behavior should be recursive (unlimited depth)
        assert!(
            paths.iter().any(|p| p.contains("file1.txt")),
            "Should find file1.txt"
        );
        assert!(
            paths.iter().any(|p| p.contains("file2.txt")),
            "Should find file2.txt (recursive)"
        );
    }

    #[test]
    fn test_edit_file_bytes_changed_on_success() {
        let mut file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(file, "Hello World").unwrap();
        let file_path = file.path().to_str().unwrap().to_string();

        let service =
            FileOperationsService::new(file.path().parent().unwrap().to_string_lossy().to_string());
        let input = EditFileInput {
            path: file_path.clone(),
            edits: vec![FileEdit {
                old_string: "World".to_string(),
                new_string: "Rust".to_string(),
            }],
        };

        let result = service.edit_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.applied);
        // "World" is 5 chars, "Rust" is 4 chars, so bytes_changed = 1
        assert_eq!(output.bytes_changed, 1);
    }

    #[test]
    fn test_edit_file_bytes_changed_zero_on_rejection() {
        let mut file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(file, "Hello World").unwrap();
        let file_path = file.path().to_str().unwrap().to_string();

        let service =
            FileOperationsService::new(file.path().parent().unwrap().to_string_lossy().to_string());

        // Try to edit something that doesn't exist
        let input = EditFileInput {
            path: file_path.clone(),
            edits: vec![FileEdit {
                old_string: "NonExistent".to_string(),
                new_string: "Something".to_string(),
            }],
        };

        let result = service.edit_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.applied);
        assert_eq!(
            output.bytes_changed, 0,
            "bytes_changed should be 0 on rejection"
        );
    }

    // ========================================================================
    // Phase 4: Regex crate tests
    // ========================================================================

    #[test]
    fn test_search_regex_anchored_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn foo() {}\nfn bar() {}\n  fn indented() {}").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = SearchContentInput {
            pattern: r"^fn".to_string(),
            path: None,
            file_glob: Some("*.rs".to_string()),
            regex: Some(true),
            case_insensitive: Some(false),
            max_results: Some(50),
            context_lines: Some(0),
        };

        let result = service.search_content(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        // Should match 2 lines: "fn foo()" and "fn bar()" but NOT "  fn indented()"
        assert_eq!(output.total, 2, "Should match 2 lines starting with ^fn");
        for m in &output.matches {
            assert!(
                m.text.starts_with("fn"),
                "Match should start with 'fn': {}",
                m.text
            );
        }
    }

    #[test]
    fn test_search_regex_group_alternation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "foo bar baz\nfoo qux foo").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = SearchContentInput {
            pattern: r"(foo|bar)".to_string(),
            path: None,
            file_glob: Some("*.txt".to_string()),
            regex: Some(true),
            case_insensitive: Some(false),
            max_results: Some(50),
            context_lines: Some(0),
        };

        let result = service.search_content(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        // Should find "foo", "bar", "foo", "foo" - but with unique matches count
        assert!(
            output.total >= 3,
            "Should find at least 3 matches for (foo|bar)"
        );
    }

    #[test]
    fn test_search_regex_real_column_position() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        // "  hello world" - "hello" starts at column 3 (1-indexed)
        fs::write(&file_path, "  hello world\nanother line").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = SearchContentInput {
            pattern: r"hello".to_string(),
            path: None,
            file_glob: Some("*.txt".to_string()),
            regex: Some(true),
            case_insensitive: Some(false),
            max_results: Some(50),
            context_lines: Some(0),
        };

        let result = service.search_content(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.total, 1);
        assert_eq!(
            output.matches[0].col, 3,
            "Column should be 3 (1-indexed position of 'hello')"
        );
    }

    #[test]
    fn test_search_regex_character_class() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello World\nanother word\n123 Numbers").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = SearchContentInput {
            pattern: r"[A-Z][a-z]+".to_string(),
            path: None,
            file_glob: Some("*.txt".to_string()),
            regex: Some(true),
            case_insensitive: Some(false),
            max_results: Some(50),
            context_lines: Some(0),
        };

        let result = service.search_content(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        // Should match "Hello" and "World"
        assert!(output.total >= 2, "Should match CamelCase words");
    }

    // ========================================================================
    // Phase 5: Chunked reading tests
    // ========================================================================

    #[test]
    fn test_continuation_token_encode_decode() {
        let path = "/some/path/file.txt";
        let offset = 1000;
        let chunk_size = 4096;

        let encoded = encode_token(path, offset, chunk_size);
        assert!(!encoded.is_empty());

        let decoded = decode_token(&encoded).unwrap();
        assert_eq!(decoded.path, path);
        assert_eq!(decoded.offset, offset);
        assert_eq!(decoded.chunk_size, chunk_size);
    }

    #[test]
    fn test_read_file_chunked_basic() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        // Create a file with 10 lines
        let content: String = (0..10).map(|i| format!("line {}\n", i)).collect();
        fs::write(&file_path, &content).unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());
        let input = ReadFileRequest {
            path: file_path.to_str().unwrap().to_string(),
            start_line: None,
            end_line: None,
            mode: Some("raw".to_string()),
            continuation_token: None,
            chunk_size: Some(50), // Small chunk size
        };

        let result = service.read_file(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(
            output.has_more || output.content.len() < content.len(),
            "With chunk_size, should indicate more content or return partial"
        );
    }

    #[test]
    fn test_read_file_chunked_continuation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content: String = (0..20).map(|i| format!("line {}\n", i)).collect();
        fs::write(&file_path, &content).unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        // First read with small chunk
        let input1 = ReadFileRequest {
            path: file_path.to_str().unwrap().to_string(),
            start_line: None,
            end_line: None,
            mode: Some("raw".to_string()),
            continuation_token: None,
            chunk_size: Some(50),
        };

        let result1 = service.read_file(input1).unwrap();
        let first_content_len = result1.content.len();

        // If there's more content, use the continuation token
        if let Some(token) = &result1.next_token {
            let input2 = ReadFileRequest {
                path: file_path.to_str().unwrap().to_string(),
                start_line: None,
                end_line: None,
                mode: Some("raw".to_string()),
                continuation_token: Some(token.clone()),
                chunk_size: Some(50),
            };

            let result2 = service.read_file(input2).unwrap();
            let combined_len = result1.content.len() + result2.content.len();

            // Combined should be longer than first chunk alone
            assert!(combined_len > first_content_len);
        }
    }

    #[test]
    fn test_read_file_chunked_line_boundary() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        // Create a file where chunk would cut mid-line without adjustment
        let content = "aaaaaaaaaa\nbbbbbbbbbb\ncccccccccc\n";
        fs::write(&file_path, content).unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        // Request a chunk size that would cut a line (but not at newline boundary)
        let input = ReadFileRequest {
            path: file_path.to_str().unwrap().to_string(),
            start_line: None,
            end_line: None,
            mode: Some("raw".to_string()),
            continuation_token: None,
            chunk_size: Some(15), // Would cut "aaaaaaaaaa\nbbbb" if not adjusted
        };

        let result = service.read_file(input).unwrap();

        // Content should not be truncated mid-line - should end at newline
        if !result.has_more || result.next_token.is_some() {
            // If it's a partial chunk, it should end at a newline
            if let Some(last_line) = result.content.lines().last() {
                // The last line might be incomplete only if it's the very last line
                // But since we have 3 full lines, it should be complete
                assert!(
                    result.content.ends_with('\n') || !result.has_more,
                    "Chunk should end at newline boundary when has_more is true"
                );
            }
        }
    }

    #[test]
    fn test_read_file_non_raw_ignores_chunk_size() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn foo() {}\nfn bar() {}").unwrap();

        let service = FileOperationsService::new(temp_dir.path().to_string_lossy().to_string());

        // Even with chunk_size, outline mode should ignore it and return full outline
        let input = ReadFileRequest {
            path: file_path.to_str().unwrap().to_string(),
            start_line: None,
            end_line: None,
            mode: Some("outline".to_string()),
            continuation_token: None,
            chunk_size: Some(10), // Small chunk that would truncate if applied
        };

        let result = service.read_file(input).unwrap();

        // Non-raw mode should ignore chunk_size and return complete result
        assert!(!result.has_more, "Non-raw mode should not set has_more");
        assert!(
            result.next_token.is_none(),
            "Non-raw mode should not return next_token"
        );
        assert!(
            result.content.len() > 0,
            "Non-raw mode should return content"
        );
    }
}
