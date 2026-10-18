//! Resource-related types and handlers for MCP protocol
//!
//! This module implements the MCP resources capability, which provides
//! access to workspace files as resources.

use base64::Engine;
use ignore::WalkBuilder;
use serde_json::Value;
use std::path::Path;

/// Handle resources/list request
/// Lists workspace files as resources using ignore::WalkBuilder.
///
/// H2.3: the cursor is now an anchored envelope. A cursor minted
/// in a different workspace or before the workspace state
/// changed is a typed error (kind + reason), not a silent reset
/// to page 0. The previous bare-numeric cursor is a typed
/// `Garbled` error.
pub fn handle_resources_list(workspace: &str, cursor: Option<&str>) -> Result<Value, String> {
    use crate::interface::mcp::cursor::{CursorContext, encode_cursor, verify_cursor};

    let workspace_path = Path::new(workspace);
    // The snapshot is the workspace path. A more precise anchor
    // (mtime, git head) can be added later without breaking the
    // format — the same context shape, a sharper snapshot.
    let cursor_ctx = CursorContext {
        workspace: workspace.to_string(),
        snapshot: workspace.to_string(),
    };

    let cursor_offset: usize = match cursor {
        Some(c) => match verify_cursor(c, &cursor_ctx) {
            Ok(off) => off,
            Err(e) => return Err(format!("{e}")),
        },
        None => 0,
    };

    let mut resources = Vec::new();
    let mut count = 0;
    let mut next_cursor: Option<String> = None;
    const PAGE_SIZE: usize = 20;

    for entry in WalkBuilder::new(workspace_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_depth(Some(10))
        .build()
    {
        if count < cursor_offset {
            count += 1;
            continue;
        }

        if resources.len() >= PAGE_SIZE {
            // Encode next cursor anchored to the same context.
            let next_offset = cursor_offset + PAGE_SIZE;
            next_cursor = Some(encode_cursor(&cursor_ctx, next_offset));
            break;
        }

        if let Ok(entry) = entry
            && entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
            && let Ok(path) = entry.path().strip_prefix(workspace_path)
        {
            let path_str = path.to_string_lossy();
            let uri = format!("file:///{}", path_str.replace('\\', "/"));

            let mime_type = detect_mime_type(&path_str);

            resources.push(serde_json::json!({
                "uri": uri,
                "name": path_str,
                "mimeType": mime_type,
                "size": entry.metadata().map(|m| m.len()).unwrap_or(0)
            }));
        }
        count += 1;
    }

    let mut result = serde_json::json!({
        "resources": resources
    });

    if let Some(cursor) = next_cursor {
        result["nextCursor"] = Value::String(cursor);
    }

    Ok(result)
}

/// Handle resources/read request
/// Reads file at URI, detects binary vs text
pub fn handle_resources_read(workspace: &str, uri: &str) -> Result<Value, String> {
    // Parse URI - expected format: file:///path/to/file
    let path = if let Some(path_str) = uri.strip_prefix("file:///") {
        // Remove "file:///" prefix
        Path::new(workspace).join(path_str)
    } else {
        return Err("Invalid URI format: must start with file:///".to_string());
    };

    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    let _metadata =
        std::fs::metadata(&path).map_err(|e| format!("Cannot read file metadata: {}", e))?;

    let is_binary = is_binary_file(&path)?;
    let mime_type = detect_mime_type(&path.to_string_lossy());

    let contents = if is_binary {
        // Read as base64 encoded blob
        let data = std::fs::read(&path).map_err(|e| format!("Cannot read file: {}", e))?;
        let blob = base64::engine::general_purpose::STANDARD.encode(&data);
        serde_json::json!([{
            "uri": uri,
            "mimeType": mime_type,
            "blob": blob
        }])
    } else {
        // Read as text
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("Cannot read file: {}", e))?;
        serde_json::json!([{
            "uri": uri,
            "mimeType": mime_type,
            "text": text
        }])
    };

    Ok(serde_json::json!({
        "contents": contents
    }))
}

/// Handle resources/templates/list request
/// Returns template for file resources
pub fn handle_resource_templates_list() -> Value {
    serde_json::json!({
        "resourceTemplates": [
            {
                "uriTemplate": "file:///{path}",
                "name": "Project Files",
                "description": "Read files from the project workspace",
                "mimeType": "application/octet-stream"
            }
        ]
    })
}

/// Detect MIME type based on file extension
fn detect_mime_type(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "rs" => "text/x-rust",
        "js" => "text/javascript",
        "ts" => "text/typescript",
        "jsx" => "text/javascript-jsx",
        "tsx" => "text/typescript-jsx",
        "py" => "text/x-python",
        "go" => "text/x-go",
        "java" => "text/x-java",
        "c" => "text/x-c",
        "cpp" | "cc" | "cxx" => "text/x-c++",
        "h" | "hpp" => "text/x-c-header",
        "css" => "text/css",
        "scss" | "sass" => "text/x-scss",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "xml" => "application/xml",
        "yaml" | "yml" => "text/yaml",
        "toml" => "application/toml",
        "md" | "markdown" => "text/markdown",
        "txt" => "text/plain",
        "sh" | "bash" => "text/x-shellscript",
        "zsh" => "text/x-zsh",
        "fish" => "text/x-fish",
        "ps1" => "text/x-powershell",
        "sql" => "text/x-sql",
        "proto" => "text/x-protobuf",
        _ => "application/octet-stream",
    }
}

/// Simple binary detection - check first 8000 bytes for null bytes
fn is_binary_file(path: &Path) -> Result<bool, String> {
    use std::io::Read;

    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open file: {}", e))?;

    let mut buffer = vec![0u8; 8000];
    let bytes_read = std::io::BufReader::new(file)
        .read(&mut buffer)
        .map_err(|e| format!("Cannot read file: {}", e))?;

    // Check for null bytes in first 8000 bytes
    Ok(buffer[..bytes_read].contains(&0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_mime_type() {
        assert_eq!(detect_mime_type("test.rs"), "text/x-rust");
        assert_eq!(detect_mime_type("test.py"), "text/x-python");
        assert_eq!(detect_mime_type("test.json"), "application/json");
        assert_eq!(detect_mime_type("test.unknown"), "application/octet-stream");
    }

    #[test]
    fn test_resource_templates_list() {
        let result = handle_resource_templates_list();
        let templates = result.get("resourceTemplates").unwrap().as_array().unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(
            templates[0].get("name").unwrap().as_str().unwrap(),
            "Project Files"
        );
    }

    #[test]
    fn test_resources_list_returns_valid_structure() {
        let result = handle_resources_list(".", None).expect("ok");
        assert!(result.get("resources").is_some());
        assert!(result.get("resources").unwrap().is_array());
    }
}
