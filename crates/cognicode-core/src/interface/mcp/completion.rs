//! Completion-related handlers for MCP protocol
//!
//! This module implements the MCP completion capability, which provides
//! auto-completion suggestions based on context.

use ignore::WalkBuilder;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Handle completion/complete request
/// Returns completions based on referrer context
pub fn handle_completion(referrer: Option<Value>, argument: Value) -> Value {
    // Extract referrer information
    let (prompt_name, arg_name) = match referrer {
        Some(Value::Object(obj)) => {
            let prompt = obj.get("prompt").and_then(|v| v.as_str()).map(String::from);
            let arg = obj
                .get("argument")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .map(String::from);
            (prompt, arg)
        }
        _ => (None, None),
    };

    let arg_name_str = arg_name.as_deref().unwrap_or("");
    let arg_value = argument.get("value").and_then(|v| v.as_str()).unwrap_or("");

    let completions = if is_file_path_arg(arg_name_str) {
        get_file_path_completions(arg_value)
    } else if is_enum_arg(arg_name_str) {
        get_enum_completions(arg_name_str, arg_value)
    } else {
        // Default empty completion
        Vec::new()
    };

    serde_json::json!({
        "completion": {
            "values": completions
        }
    })
}

/// Check if argument is a file path
fn is_file_path_arg(arg_name: &str) -> bool {
    matches!(
        arg_name,
        "file_path"
            | "path"
            | "file"
            | "directory"
            | "source"
            | "target"
            | "symbol_name"
            | "refactor_target"
    )
}

/// Check if argument is an enum with known values
fn is_enum_arg(arg_name: &str) -> bool {
    matches!(
        arg_name,
        "focus"
            | "goal"
            | "detail_level"
            | "direction"
            | "action"
            | "format"
            | "mode"
            | "strategy"
            | "subgraph_direction"
            | "schema"
    )
}

/// Get file path completions for a prefix
fn get_file_path_completions(prefix: &str) -> Vec<String> {
    let base_path: PathBuf = if prefix.is_empty() || prefix == "." {
        PathBuf::from(".")
    } else if prefix.starts_with("./") || prefix.starts_with(".\\") {
        PathBuf::from(".").join(&prefix[2..])
    } else if prefix.starts_with('/') || prefix.starts_with('\\') {
        // Absolute path - just use it
        PathBuf::from(prefix)
    } else {
        // Relative path
        PathBuf::from(prefix)
    };

    let parent = base_path.parent().unwrap_or(Path::new("."));
    let file_name = base_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let mut completions = Vec::new();
    let search_pattern = if file_name.is_empty() {
        "*".to_string()
    } else {
        format!("{}*", file_name)
    };

    // Don't search too deep or with too many results
    for entry in WalkBuilder::new(parent)
        .hidden(false)
        .git_ignore(true)
        .max_depth(Some(3))
        .build()
        .take(50)
    {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            let entry_name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Check if matches prefix
            if entry_name.starts_with(file_name) || file_name.is_empty() {
                let relative_path = if parent.to_string_lossy() == "." {
                    entry_name.to_string()
                } else {
                    format!(
                        "{}/{}",
                        parent.to_string_lossy().replace("./", ""),
                        entry_name
                    )
                };

                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    completions.push(format!("{}/", relative_path));
                } else {
                    completions.push(relative_path);
                }
            }
        }
    }

    // Sort and dedupe
    completions.sort();
    completions.dedup();
    completions.truncate(20);
    completions
}

/// Get enum value completions
fn get_enum_completions(arg_name: &str, current: &str) -> Vec<String> {
    let values = match arg_name {
        "focus" => vec!["security", "performance", "readability"],
        "goal" => vec!["performance", "readability", "safety"],
        "detail_level" => vec!["brief", "detailed"],
        "direction" => vec!["incoming", "outgoing"],
        "action" => vec!["rename", "extract", "inline", "move", "change_signature"],
        "format" => vec!["code", "svg"],
        "mode" => vec!["raw", "outline", "symbols", "compressed"],
        "strategy" => vec!["lightweight", "on_demand", "per_file", "full"],
        "subgraph_direction" => vec!["in", "out", "both"],
        _ => vec![],
    };

    values
        .iter()
        .filter(|v| current.is_empty() || v.starts_with(current))
        .map(|v| v.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_returns_valid_structure() {
        let result = handle_completion(None, serde_json::json!({"value": ""}));
        assert!(result.get("completion").is_some());
        assert!(result.get("completion").unwrap().get("values").is_some());
    }

    #[test]
    fn test_enum_completions_for_focus() {
        let argument = serde_json::json!({"value": "sec"});
        let result = handle_completion(
            Some(serde_json::json!({
                "prompt": "code_review",
                "argument": {"name": "focus"}
            })),
            argument,
        );
        let values = result
            .get("completion")
            .unwrap()
            .get("values")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(values.iter().any(|v| v.as_str().unwrap() == "security"));
    }

    #[test]
    fn test_enum_completions_for_goal() {
        let argument = serde_json::json!({"value": "per"});
        let result = handle_completion(
            Some(serde_json::json!({
                "prompt": "refactor_suggest",
                "argument": {"name": "goal"}
            })),
            argument,
        );
        let values = result
            .get("completion")
            .unwrap()
            .get("values")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(values.iter().any(|v| v.as_str().unwrap() == "performance"));
    }

    #[test]
    fn test_enum_completions_filter_by_current() {
        let argument = serde_json::json!({"value": "in"});
        let result = handle_completion(
            Some(serde_json::json!({
                "argument": {"name": "direction"}
            })),
            argument,
        );
        let values = result
            .get("completion")
            .unwrap()
            .get("values")
            .unwrap()
            .as_array()
            .unwrap();
        // Should filter to only "incoming" and "in"
        assert!(values.iter().any(|v| v.as_str().unwrap() == "incoming"));
    }
}
