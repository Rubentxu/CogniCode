//! Analysis Engine - generates root cause and recommendations from debug state

use std::collections::HashMap;

use crate::client::{StackFrame, Variable};

/// Root cause analysis result
#[derive(Debug, Clone)]
pub struct RootCause {
    pub summary: String,
    pub location: RootLocation,
    pub explanation: String,
}

/// Location of the root cause
#[derive(Debug, Clone)]
pub struct RootLocation {
    pub function: String,
    pub file: String,
    pub line: u32,
}

/// Recommendation for fixing the issue
#[derive(Debug, Clone)]
pub struct Recommendation {
    pub action: String,
    pub code_suggestion: Option<String>,
    pub alternative: Option<String>,
}

/// Analysis engine
pub struct AnalysisEngine {
    /// Common bug patterns
    patterns: Vec<BugPattern>,
}

impl AnalysisEngine {
    /// Create a new analysis engine
    pub fn new() -> Self {
        Self {
            patterns: vec![
                BugPattern {
                    name: "index_out_of_bounds".to_string(),
                    detect: vec!["index out of bounds".to_string(), "out of bounds".to_string()],
                    explanation: "The code tried to access an index that doesn't exist in the collection"
                        .to_string(),
                    fix_template: Some("Check array bounds before access: if (index >= array.len()) {{ ... }}".to_string()),
                },
                BugPattern {
                    name: "null_pointer".to_string(),
                    detect: vec!["null".to_string(), "None".to_string(), "nil".to_string(), "undefined".to_string()],
                    explanation: "The code tried to use a value that doesn't exist".to_string(),
                    fix_template: Some("Add null check: if (value != null) {{ ... }}".to_string()),
                },
                BugPattern {
                    name: "empty_collection".to_string(),
                    detect: vec!["empty".to_string(), "items is empty".to_string()],
                    explanation: "The code assumed a collection had elements but it was empty"
                        .to_string(),
                    fix_template: Some("Add empty check: if (!collection.isEmpty()) {{ ... }}".to_string()),
                },
                BugPattern {
                    name: "type_mismatch".to_string(),
                    detect: vec!["type mismatch".to_string(), "expected String".to_string()],
                    explanation: "The code used a value of the wrong type".to_string(),
                    fix_template: Some("Convert or cast to the expected type".to_string()),
                },
            ],
        }
    }

    /// Analyze a crash and determine root cause
    #[allow(unused_variables)]
    pub fn analyze_crash(
        &self,
        stopped: &crate::client::StoppedEvent,
        stack: &[StackFrame],
        vars: &[Variable], // Reserved for future enhanced analysis
    ) -> RootCause {
        // Find the crash location (usually the innermost frame)
        let crash_frame = stack.first();

        let (function, file, line) = if let Some(frame) = crash_frame {
            (
                frame.name.clone(),
                frame.source.as_ref().and_then(|s| s.path.clone()).unwrap_or_default(),
                frame.line,
            )
        } else {
            ("unknown".to_string(), "unknown".to_string(), 0)
        };

        // Detect the error type from the stopped event reason
        let error_message = match stopped.reason.as_str() {
            "exception" => stopped.text.clone().unwrap_or_default(),
            "breakpoint" => stopped.description.clone().unwrap_or_default(),
            "step" => "stepped".to_string(),
            _ => stopped.reason.clone(),
        };

        // Try to match a pattern
        let pattern = self.detect_pattern(&error_message);

        let explanation = if let Some(p) = pattern {
            p.explanation.clone()
        } else {
            format!("An error occurred: {}", error_message)
        };

        let summary = if let Some(p) = pattern {
            format!("{} at {}:{}", p.name, function, line)
        } else {
            format!("Error at {}:{}: {}", function, line, error_message)
        };

        RootCause {
            summary,
            location: RootLocation {
                function,
                file,
                line,
            },
            explanation,
        }
    }

    /// Detect a bug pattern from error message
    fn detect_pattern(&self, message: &str) -> Option<&BugPattern> {
        let msg_lower = message.to_lowercase();
        for pattern in &self.patterns {
            for keyword in &pattern.detect {
                if msg_lower.contains(&keyword.to_lowercase()) {
                    return Some(pattern);
                }
            }
        }
        None
    }

    /// Suggest a fix based on the root cause
    pub fn suggest_fix(&self, root_cause: &RootCause) -> Recommendation {
        // Try to find a pattern match
        let pattern = self.detect_pattern(&root_cause.explanation);

        if let Some(p) = pattern {
            if let Some(ref template) = p.fix_template {
                return Recommendation {
                    action: format!("Fix {} by adding a guard", p.name),
                    code_suggestion: Some(template.replace("{{", "{").replace("}}", "}")),
                    alternative: Some(format!(
                        "Alternatively, make the function handle {} gracefully",
                        p.name
                    )),
                };
            }
        }

        // Generic recommendation
        Recommendation {
            action: "Investigate the error location".to_string(),
            code_suggestion: Some(format!(
                "// In {} at line {}\n// Add appropriate error handling",
                root_cause.location.file, root_cause.location.line
            )),
            alternative: Some("Consider adding a guard clause or validation before this point".to_string()),
        }
    }

    /// Analyze variables to find suspicious values
    pub fn analyze_variables(&self, vars: &[Variable]) -> HashMap<String, String> {
        let mut suspicious = HashMap::new();

        for var in vars {
            // Check for empty collections
            if var.value == "[]" || var.value == "{}" || var.value.is_empty() {
                suspicious.insert(var.name.clone(), format!("(empty) {}", var.value));
            }
            // Check for None/Null values
            else if var.value == "null" || var.value == "None" || var.value == "nil" {
                suspicious.insert(var.name.clone(), format!("(null) {}", var.value));
            }
            // Check for error indicators in value
            else if var.value.contains("Err(") || var.value.contains("Error") {
                suspicious.insert(var.name.clone(), format!("(error) {}", var.value));
            }
        }

        suspicious
    }
}

impl Default for AnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A known bug pattern
struct BugPattern {
    name: String,
    detect: Vec<String>,
    explanation: String,
    fix_template: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_detection() {
        let engine = AnalysisEngine::new();

        let result = engine.detect_pattern("index out of bounds");
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "index_out_of_bounds");

        let result = engine.detect_pattern("Unexpected null value");
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "null_pointer");
    }

    #[test]
    fn test_variable_analysis() {
        let engine = AnalysisEngine::new();

        let vars = vec![
            Variable {
                name: "items".to_string(),
                value: "[]".to_string(),
                type_: Some("Vec".to_string()),
                variables_reference: None,
                named_variables: None,
                indexed_variables: None,
                presentation_hint: None,
            },
            Variable {
                name: "count".to_string(),
                value: "42".to_string(),
                type_: Some("i32".to_string()),
                variables_reference: None,
                named_variables: None,
                indexed_variables: None,
                presentation_hint: None,
            },
        ];

        let suspicious = engine.analyze_variables(&vars);
        assert!(suspicious.contains_key("items"));
        assert!(!suspicious.contains_key("count"));
    }
}
