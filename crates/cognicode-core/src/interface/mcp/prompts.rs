//! Prompt-related types and handlers for MCP protocol
//!
//! This module implements the MCP prompts capability, which provides
//! access to predefined prompt templates.

use serde_json::Value;

/// Handle prompts/list request
/// Returns list of available prompts
pub fn handle_prompts_list(cursor: Option<&str>) -> Value {
    // Only one page of prompts, cursor ignored
    let _ = cursor;

    serde_json::json!({
        "prompts": [
            {
                "name": "code_review",
                "description": "Perform a comprehensive code review of a file, optionally focusing on specific aspects",
                "arguments": [
                    {
                        "name": "file_path",
                        "description": "Path to the file to review",
                        "required": true,
                        "schema": {
                            "type": "string"
                        }
                    },
                    {
                        "name": "focus",
                        "description": "Area to focus on: security, performance, or readability",
                        "required": false,
                        "schema": {
                            "type": "string",
                            "enum": ["security", "performance", "readability"]
                        }
                    }
                ]
            },
            {
                "name": "explain_code",
                "description": "Get a detailed explanation of code in a file",
                "arguments": [
                    {
                        "name": "file_path",
                        "description": "Path to the file to explain",
                        "required": true,
                        "schema": {
                            "type": "string"
                        }
                    },
                    {
                        "name": "detail_level",
                        "description": "Level of detail: brief or detailed",
                        "required": false,
                        "schema": {
                            "type": "string",
                            "enum": ["brief", "detailed"]
                        }
                    }
                ]
            },
            {
                "name": "refactor_suggest",
                "description": "Get suggestions for refactoring a file with a specific goal",
                "arguments": [
                    {
                        "name": "file_path",
                        "description": "Path to the file to refactor",
                        "required": true,
                        "schema": {
                            "type": "string"
                        }
                    },
                    {
                        "name": "goal",
                        "description": "Refactoring goal: performance, readability, or safety",
                        "required": false,
                        "schema": {
                            "type": "string",
                            "enum": ["performance", "readability", "safety"]
                        }
                    }
                ]
            }
        ]
    })
}

/// Handle prompts/get request
/// Returns a specific prompt with arguments substituted
pub fn handle_prompts_get(name: &str, arguments: Option<Value>) -> Result<Value, String> {
    let empty_map = serde_json::Map::new();
    let args_obj = match &arguments {
        Some(Value::Object(obj)) => obj,
        _ => &empty_map,
    };

    match name {
        "code_review" => get_code_review_prompt(args_obj),
        "explain_code" => get_explain_code_prompt(args_obj),
        "refactor_suggest" => get_refactor_suggest_prompt(args_obj),
        _ => Err(format!("Unknown prompt: {}", name)),
    }
}

/// Generate code_review prompt
fn get_code_review_prompt(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<Value, String> {
    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let focus = args
        .get("focus")
        .and_then(|v| v.as_str())
        .unwrap_or("general");

    let user_message = match (file_path, focus) {
        (Some(path), "security") => format!(
            "Perform a security-focused code review of the file at `{}`. \
            Identify potential security vulnerabilities such as:\n\
            - Injection attacks\n\
            - Authentication/authorization issues\n\
            - Data exposure risks\n\
            - Cryptographic weaknesses\n\
            - Input validation problems\n\n\
            For each issue found, explain the risk and suggest a fix.",
            path
        ),
        (Some(path), "performance") => format!(
            "Perform a performance-focused code review of the file at `{}`. \
            Identify potential performance issues such as:\n\
            - Unnecessary allocations\n\
            - Inefficient algorithms or data structures\n\
            - Missing caching opportunities\n\
            - Blocking operations in async contexts\n\
            - Memory leaks or excessive memory usage\n\n\
            For each issue found, explain the impact and suggest an optimization.",
            path
        ),
        (Some(path), "readability") => format!(
            "Perform a readability-focused code review of the file at `{}`. \
            Identify areas that could be improved for better readability:\n\
            - Complex or deeply nested logic\n\
            - Poor naming choices\n\
            - Missing or unclear documentation\n\
            - Inconsistent formatting\n\
            - Missing error handling\n\n\
            For each issue found, suggest a more readable alternative.",
            path
        ),
        (Some(path), _) => format!(
            "Perform a comprehensive code review of the file at `{}`. \
            Analyze the code for:\n\
            - Correctness and bugs\n\
            - Security vulnerabilities\n\
            - Performance issues\n\
            - Code readability and maintainability\n\
            - Error handling\n\
            - Testing coverage\n\n\
            Provide specific suggestions for improvements with code examples where helpful.",
            path
        ),
        (None, "security") => "Perform a security-focused code review. Please provide a file_path argument to specify which file to review. I'll look for:\n- Injection attacks\n- Authentication/authorization issues\n- Data exposure risks\n- Cryptographic weaknesses\n- Input validation problems\n\nFor each issue found, explain the risk and suggest a fix.".to_string(),
        (None, "performance") => "Perform a performance-focused code review. Please provide a file_path argument to specify which file to review. I'll look for:\n- Unnecessary allocations\n- Inefficient algorithms or data structures\n- Missing caching opportunities\n- Blocking operations in async contexts\n- Memory leaks or excessive memory usage\n\nFor each issue found, explain the impact and suggest an optimization.".to_string(),
        (None, "readability") => "Perform a readability-focused code review. Please provide a file_path argument to specify which file to review. I'll look for:\n- Complex or deeply nested logic\n- Poor naming choices\n- Missing or unclear documentation\n- Inconsistent formatting\n- Missing error handling\n\nFor each issue found, suggest a more readable alternative.".to_string(),
        (None, _) => "Perform a comprehensive code review. Please provide a file_path argument to specify which file to review. I'll analyze the code for:\n- Correctness and bugs\n- Security vulnerabilities\n- Performance issues\n- Code readability and maintainability\n- Error handling\n- Testing coverage\n\nProvide specific suggestions for improvements with code examples where helpful.".to_string(),
    };

    let file_path_display = file_path.unwrap_or("<file_path>");
    Ok(serde_json::json!({
        "description": format!("Code review of {} focusing on {}", file_path_display, focus),
        "messages": [
            {
                "role": "user",
                "content": {
                    "type": "text",
                    "text": user_message
                }
            }
        ]
    }))
}

/// Generate explain_code prompt
fn get_explain_code_prompt(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<Value, String> {
    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let detail_level = args
        .get("detail_level")
        .and_then(|v| v.as_str())
        .unwrap_or("brief");

    let user_message = match (file_path, detail_level) {
        (Some(path), "detailed") => format!(
            "Provide a detailed explanation of the code in `{}`. \
            Include:\n\
            - Overall purpose and functionality\n\
            - Key data structures and their roles\n\
            - Main functions/methods and their interactions\n\
            - Control flow and edge cases handled\n\
            - External dependencies and their usage\n\
            - Any notable patterns or architectural decisions\n\n\
            Be thorough and include relevant code snippets.",
            path
        ),
        (Some(path), _) => format!(
            "Explain the code in `{}` in brief. \
            Focus on:\n\
            - What the file does (purpose)\n\
            - Key symbols (functions, structs, etc.)\n\
            - How the main parts work together\n\n\
            Keep it concise but informative.",
            path
        ),
        (None, "detailed") => "Provide a detailed explanation of the code. Please provide a file_path argument to specify which file to explain. I'll cover:\n- Overall purpose and functionality\n- Key data structures and their roles\n- Main functions/methods and their interactions\n- Control flow and edge cases handled\n- External dependencies and their usage\n- Any notable patterns or architectural decisions\n\nBe thorough and include relevant code snippets.".to_string(),
        (None, _) => "Explain the code in brief. Please provide a file_path argument to specify which file to explain. I'll focus on:\n- What the file does (purpose)\n- Key symbols (functions, structs, etc.)\n- How the main parts work together\n\nKeep it concise but informative.".to_string(),
    };

    let file_path_display = file_path.unwrap_or("<file_path>");
    Ok(serde_json::json!({
        "description": format!("Explanation of {} ({})", file_path_display, detail_level),
        "messages": [
            {
                "role": "user",
                "content": {
                    "type": "text",
                    "text": user_message
                }
            }
        ]
    }))
}

/// Generate refactor_suggest prompt
fn get_refactor_suggest_prompt(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<Value, String> {
    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let goal = args
        .get("goal")
        .and_then(|v| v.as_str())
        .unwrap_or("readability");

    let user_message = match (file_path, goal) {
        (Some(path), "performance") => format!(
            "Suggest refactoring improvements for `{}` with a focus on performance. \
            Consider:\n\
            - Reducing allocations and memory usage\n\
            - Using more efficient data structures or algorithms\n\
            - Leveraging compiler optimizations\n\
            - Parallelization opportunities\n\
            - Caching and memoization\n\n\
            Provide concrete code examples for each suggestion.",
            path
        ),
        (Some(path), "safety") => format!(
            "Suggest refactoring improvements for `{}` with a focus on code safety. \
            Consider:\n\
            - Error handling and fallibility\n\
            - Type safety and bounds checking\n\
            - Concurrency safety (if applicable)\n\
            - Memory safety (if applicable)\n\
            - Input validation\n\
            - Avoiding panics\n\n\
            Provide concrete code examples for each suggestion.",
            path
        ),
        (Some(path), _) => format!(
            "Suggest refactoring improvements for `{}` with a focus on readability and maintainability. \
            Consider:\n\
            - Simplifying complex logic\n\
            - Improving naming conventions\n\
            - Extracting reusable abstractions\n\
            - Adding documentation\n\
            - Reducing coupling\n\
            - Improving testability\n\n\
            Provide concrete code examples for each suggestion.",
            path
        ),
        (None, "performance") => "Suggest refactoring improvements with a focus on performance. Please provide a file_path argument to specify which file to refactor. I'll consider:\n- Reducing allocations and memory usage\n- Using more efficient data structures or algorithms\n- Leveraging compiler optimizations\n- Parallelization opportunities\n- Caching and memoization\n\nProvide concrete code examples for each suggestion.".to_string(),
        (None, "safety") => "Suggest refactoring improvements with a focus on code safety. Please provide a file_path argument to specify which file to refactor. I'll consider:\n- Error handling and fallibility\n- Type safety and bounds checking\n- Concurrency safety (if applicable)\n- Memory safety (if applicable)\n- Input validation\n- Avoiding panics\n\nProvide concrete code examples for each suggestion.".to_string(),
        (None, _) => "Suggest refactoring improvements with a focus on readability and maintainability. Please provide a file_path argument to specify which file to refactor. I'll consider:\n- Simplifying complex logic\n- Improving naming conventions\n- Extracting reusable abstractions\n- Adding documentation\n- Reducing coupling\n- Improving testability\n\nProvide concrete code examples for each suggestion.".to_string(),
    };

    let file_path_display = file_path.unwrap_or("<file_path>");
    Ok(serde_json::json!({
        "description": format!("Refactoring suggestions for {} (goal: {})", file_path_display, goal),
        "messages": [
            {
                "role": "user",
                "content": {
                    "type": "text",
                    "text": user_message
                }
            }
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompts_list_returns_prompts() {
        let result = handle_prompts_list(None);
        let prompts = result.get("prompts").unwrap().as_array().unwrap();
        assert_eq!(prompts.len(), 3);

        let names: Vec<&str> = prompts
            .iter()
            .filter_map(|p| p.get("name").and_then(|n| n.as_str()))
            .collect();
        assert!(names.contains(&"code_review"));
        assert!(names.contains(&"explain_code"));
        assert!(names.contains(&"refactor_suggest"));
    }

    #[test]
    fn test_prompts_get_code_review_without_args() {
        let result = handle_prompts_get("code_review", None);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.get("description").is_some());
        assert!(result.get("messages").is_some());
    }

    #[test]
    fn test_prompts_get_code_review_with_args() {
        let args = serde_json::json!({
            "file_path": "test.rs",
            "focus": "security"
        });
        let result = handle_prompts_get("code_review", Some(args));
        assert!(result.is_ok());
        let result = result.unwrap();
        let text = result.get("messages").unwrap().as_array().unwrap()[0]
            .get("content")
            .unwrap()
            .get("text")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(text.contains("test.rs"));
        assert!(text.contains("security"));
    }

    #[test]
    fn test_prompts_get_explain_code() {
        let args = serde_json::json!({
            "file_path": "main.rs",
            "detail_level": "detailed"
        });
        let result = handle_prompts_get("explain_code", Some(args));
        assert!(result.is_ok());
        let result = result.unwrap();
        let text = result.get("messages").unwrap().as_array().unwrap()[0]
            .get("content")
            .unwrap()
            .get("text")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(text.contains("main.rs"));
        assert!(text.contains("detailed"));
    }

    #[test]
    fn test_prompts_get_refactor_suggest() {
        let args = serde_json::json!({
            "file_path": "lib.rs",
            "goal": "performance"
        });
        let result = handle_prompts_get("refactor_suggest", Some(args));
        assert!(result.is_ok());
        let result = result.unwrap();
        let text = result.get("messages").unwrap().as_array().unwrap()[0]
            .get("content")
            .unwrap()
            .get("text")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(text.contains("lib.rs"));
        assert!(text.contains("performance"));
    }

    #[test]
    fn test_prompts_get_unknown_prompt() {
        let result = handle_prompts_get("unknown_prompt", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown prompt"));
    }
}
