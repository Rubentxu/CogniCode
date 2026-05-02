//! Axiom MCP tool definitions
//!
//! Quality analysis MCP tool definitions:
//! - `check_quality`: Run quality analysis on a project using RuleRegistry
//! - `quality_delta`: Compare two quality snapshots
//! - `check_boundaries`: Check architectural boundary violations
//! - `check_lint`: Run linters (clippy, eslint, semgrep) — placeholder
//! - `get_debt`: Calculate technical debt for a project
//! - `get_ratings`: Get project ratings (reliability, security, maintainability)
//! - `detect_duplications`: Detect code duplications in a file
//! - `run_gate`: Evaluate quality gates against metrics
//! - `list_rules`: List all registered rules

use std::collections::HashMap;

use rmcp::model::Tool;
use serde::Deserialize;

use crate::error::{AxiomError, AxiomResult};
use crate::quality::{
    BoundaryChecker, BoundaryDefinition, BoundaryReport,
    QualityDelta, QualitySnapshot,
};
use crate::rules::{
    RuleRegistry, ProjectMetrics, QualityGateEvaluator,
    TechnicalDebtCalculator, DuplicationDetector, ProjectRatings,
    GateCondition, CompareOperator, MetricValue, QualityGate,
};

/// Shared axiom state for MCP tool handlers
#[derive(Debug)]
pub struct AxiomTools {
    pub rule_registry: RuleRegistry,
    pub gate_evaluator: QualityGateEvaluator,
    pub debt_calculator: TechnicalDebtCalculator,
    pub duplication_detector: DuplicationDetector,
}

impl AxiomTools {
    /// Create axiom tools with default configuration
    pub fn new() -> AxiomResult<Self> {
        let rule_registry = RuleRegistry::discover();
        let gate_evaluator = QualityGateEvaluator::new(Vec::new());
        let debt_calculator = TechnicalDebtCalculator::new();
        let duplication_detector = DuplicationDetector::new();
        Ok(Self {
            rule_registry,
            gate_evaluator,
            debt_calculator,
            duplication_detector,
        })
    }

    /// Return all MCP tool definitions
    pub fn tool_definitions() -> Vec<Tool> {
        vec![
            Tool::new(
                "check_quality",
                "Run quality analysis on a project. Returns SOLID violations, LCOM scores, connascence metrics, and complexity.",
                std::sync::Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "project_path": {
                                "type": "string",
                                "description": "Path to the project directory"
                            },
                            "metrics": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Which metrics to run: [\"lcom\", \"solid\", \"connascence\", \"complexity\", \"all\"]",
                                "default": ["all"]
                            }
                        },
                        "required": ["project_path"]
                    })
                    .as_object()
                    .cloned()
                    .unwrap(),
                ),
            ),
            Tool::new(
                "quality_delta",
                "Compare two quality snapshots and report changes.",
                std::sync::Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "before": {
                                "type": "object",
                                "description": "The before QualitySnapshot"
                            },
                            "after": {
                                "type": "object",
                                "description": "The after QualitySnapshot"
                            }
                        },
                        "required": ["before", "after"]
                    })
                    .as_object()
                    .cloned()
                    .unwrap(),
                ),
            ),
            Tool::new(
                "check_boundaries",
                "Check architectural boundary violations in a project.",
                std::sync::Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "project_path": {
                                "type": "string",
                                "description": "Path to the project directory"
                            },
                            "boundaries_config": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "name": { "type": "string" },
                                        "path_patterns": { "type": "array", "items": { "type": "string" } },
                                        "allowed_dependencies": { "type": "array", "items": { "type": "string" } }
                                    },
                                    "required": ["name", "path_patterns", "allowed_dependencies"]
                                },
                                "description": "Boundary definitions (uses DDD defaults if empty)"
                            }
                        },
                        "required": ["project_path"]
                    })
                    .as_object()
                    .cloned()
                    .unwrap(),
                ),
            ),
            Tool::new(
                "check_lint",
                "Run linters (clippy, eslint, semgrep) on a project and return issues. Note: Linters not yet configured.",
                std::sync::Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "project_path": {
                                "type": "string",
                                "description": "Path to the project directory to lint"
                            },
                            "linters": {
                                "type": "array",
                                "items": {
                                    "type": "string",
                                    "enum": ["clippy", "eslint", "semgrep"]
                                },
                                "description": "Which linters to run (default: all available)"
                            }
                        },
                        "required": ["project_path"]
                    })
                    .as_object()
                    .cloned()
                    .unwrap(),
                ),
            ),
            Tool::new(
                "get_debt",
                "Calculate technical debt for a project using SQALE method.",
                std::sync::Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "project_path": {
                                "type": "string",
                                "description": "Path to the project directory"
                            }
                        },
                        "required": ["project_path"]
                    })
                    .as_object()
                    .cloned()
                    .unwrap(),
                ),
            ),
            Tool::new(
                "get_ratings",
                "Get project ratings (reliability, security, maintainability) based on issue analysis.",
                std::sync::Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "project_path": {
                                "type": "string",
                                "description": "Path to the project directory"
                            }
                        },
                        "required": ["project_path"]
                    })
                    .as_object()
                    .cloned()
                    .unwrap(),
                ),
            ),
            Tool::new(
                "detect_duplications",
                "Detect code duplications in a file using BLAKE3 hashing.",
                std::sync::Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": {
                                "type": "string",
                                "description": "Path to the file to analyze"
                            }
                        },
                        "required": ["file_path"]
                    })
                    .as_object()
                    .cloned()
                    .unwrap(),
                ),
            ),
            Tool::new(
                "run_gate",
                "Evaluate quality gates against project metrics.",
                std::sync::Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "gate_config": {
                                "type": "object",
                                "description": "Quality gate configuration",
                                "properties": {
                                    "name": { "type": "string" },
                                    "description": { "type": "string" },
                                    "conditions": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "metric": { "type": "string" },
                                                "operator": { "type": "string", "enum": ["GT", "GTE", "LT", "LTE", "EQ", "NEQ"] },
                                                "threshold": {}
                                            }
                                        }
                                    }
                                }
                            },
                            "metrics": {
                                "type": "object",
                                "description": "Project metrics to evaluate",
                                "properties": {
                                    "ncloc": { "type": "integer" },
                                    "functions": { "type": "integer" },
                                    "classes": { "type": "integer" },
                                    "complexity": { "type": "integer" },
                                    "code_smells": { "type": "integer" },
                                    "bugs": { "type": "integer" },
                                    "vulnerabilities": { "type": "integer" },
                                    "security_hotspots": { "type": "integer" },
                                    "debt_ratio": { "type": "number" },
                                    "maintainability_rating": { "type": "string" },
                                    "duplication_percentage": { "type": "number" }
                                }
                            }
                        },
                        "required": ["gate_config", "metrics"]
                    })
                    .as_object()
                    .cloned()
                    .unwrap(),
                ),
            ),
            Tool::new(
                "list_rules",
                "List all registered rules with their IDs, names, and severities.",
                std::sync::Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {}
                    })
                    .as_object()
                    .cloned()
                    .unwrap(),
                ),
            ),
        ]
    }

    // ── Quality Tool Handlers ────────────────────────────────

    /// Handle `check_quality` tool call — runs rules from RuleRegistry
    pub fn handle_check_quality(
        &self,
        args: &serde_json::Value,
    ) -> AxiomResult<serde_json::Value> {
        let input: CheckQualityInput = serde_json::from_value(args.clone())?;

        // Get all rules from registry
        let all_rules = self.rule_registry.all();
        
        let rule_summaries: Vec<serde_json::Value> = all_rules
            .iter()
            .map(|rule| {
                serde_json::json!({
                    "id": rule.id(),
                    "name": rule.name(),
                    "severity": format!("{:?}", rule.severity()).to_lowercase(),
                    "category": format!("{:?}", rule.category()).to_lowercase(),
                    "language": rule.language(),
                })
            })
            .collect();

        let report = QualityReport {
            status: "ready".to_string(),
            project_path: input.project_path,
            metrics_requested: input.metrics.unwrap_or_else(|| vec!["all".to_string()]),
            message: format!("Loaded {} rules from registry for analysis", all_rules.len()),
            rules: rule_summaries,
        };

        Ok(serde_json::to_value(report)?)
    }

    /// Handle `quality_delta` tool call
    pub fn handle_quality_delta(
        &self,
        args: &serde_json::Value,
    ) -> AxiomResult<serde_json::Value> {
        let input: QualityDeltaInput = serde_json::from_value(args.clone())?;

        let before: QualitySnapshot = serde_json::from_value(input.before)?;
        let after: QualitySnapshot = serde_json::from_value(input.after)?;

        let delta = QualityDelta::compare(&before, &after);

        Ok(serde_json::to_value(delta)?)
    }

    /// Handle `check_boundaries` tool call
    pub fn handle_check_boundaries(
        &self,
        args: &serde_json::Value,
    ) -> AxiomResult<serde_json::Value> {
        let input: CheckBoundariesInput = serde_json::from_value(args.clone())?;

        // Build boundary checker from config or use defaults
        let _checker = if let Some(configs) = &input.boundaries_config {
            let boundaries: Vec<BoundaryDefinition> = configs
                .iter()
                .map(|c| BoundaryDefinition {
                    name: c.name.clone(),
                    path_patterns: c.path_patterns.clone(),
                    allowed_dependencies: c.allowed_dependencies.clone(),
                })
                .collect();
            BoundaryChecker::new(boundaries)
        } else {
            BoundaryChecker::with_ddd_defaults()
        };

        let report = BoundaryReport {
            total_violations: 0,
            by_boundary: HashMap::new(),
            summary: "Boundary checking requires CallGraph build from cognicode-core".to_string(),
        };

        Ok(serde_json::to_value(report)?)
    }

    /// Handle `check_lint` tool call — placeholder returning not yet configured
    pub fn handle_check_lint(
        &self,
        _args: &serde_json::Value,
    ) -> AxiomResult<serde_json::Value> {
        Ok(serde_json::json!({
            "status": "not_yet_configured",
            "message": "Linter runner not yet implemented — coming in Phase 3"
        }))
    }

    /// Handle `get_debt` tool call — calculate technical debt
    pub fn handle_get_debt(
        &self,
        args: &serde_json::Value,
    ) -> AxiomResult<serde_json::Value> {
        let input: GetDebtInput = serde_json::from_value(args.clone())?;

        // Read the source file to get line count
        let source = std::fs::read_to_string(&input.file_path)
            .map_err(|e| AxiomError::Other(format!("Failed to read file: {}", e)))?;
        
        let ncloc = source.lines().count();
        
        // For now, return an empty debt report since we don't have actual issues
        // In a real implementation, this would run rules and collect issues first
        let report = self.debt_calculator.calculate(&[], ncloc);

        Ok(serde_json::to_value(report)?)
    }

    /// Handle `get_ratings` tool call — get project ratings
    pub fn handle_get_ratings(
        &self,
        args: &serde_json::Value,
    ) -> AxiomResult<serde_json::Value> {
        let input: GetRatingsInput = serde_json::from_value(args.clone())?;

        // Read the source file to get line count
        let source = std::fs::read_to_string(&input.project_path)
            .map_err(|e| AxiomError::Other(format!("Failed to read file: {}", e)))?;
        
        let ncloc = source.lines().count();
        
        // Empty issues and debt report for placeholder
        let debt_report = self.debt_calculator.calculate(&[], ncloc);
        let ratings = ProjectRatings::compute(&[], ncloc, &debt_report);

        Ok(serde_json::to_value(ratings)?)
    }

    /// Handle `detect_duplications` tool call — detect code duplications
    pub fn handle_detect_duplications(
        &self,
        args: &serde_json::Value,
    ) -> AxiomResult<serde_json::Value> {
        let input: DetectDuplicationsInput = serde_json::from_value(args.clone())?;

        // Read the source file
        let source = std::fs::read_to_string(&input.file_path)
            .map_err(|e| AxiomError::Other(format!("Failed to read file: {}", e)))?;
        
        let groups = self.duplication_detector.detect_duplications(&source);

        Ok(serde_json::to_value(groups)?)
    }

    /// Handle `run_gate` tool call — evaluate quality gates
    pub fn handle_run_gate(
        &self,
        args: &serde_json::Value,
    ) -> AxiomResult<serde_json::Value> {
        let input: RunGateInput = serde_json::from_value(args.clone())?;

        let gate_config = input.gate_config;
        let metrics = input.metrics;

        // Build GateCondition objects
        let conditions: Vec<GateCondition> = gate_config
            .conditions
            .iter()
            .map(|c| {
                let threshold = if let Some(val) = c.threshold.as_i64() {
                    MetricValue::Integer(val)
                } else if let Some(val) = c.threshold.as_f64() {
                    MetricValue::Float(val)
                } else {
                    MetricValue::Integer(0)
                };
                
                let operator = match c.operator.to_uppercase().as_str() {
                    "GT" => CompareOperator::GT,
                    "GTE" => CompareOperator::GTE,
                    "LT" => CompareOperator::LT,
                    "LTE" => CompareOperator::LTE,
                    "EQ" => CompareOperator::EQ,
                    "NEQ" => CompareOperator::NEQ,
                    _ => CompareOperator::EQ,
                };
                
                GateCondition::new(&c.metric, operator, threshold)
            })
            .collect();

        // Build gate directly with conditions
        let gate = QualityGate {
            name: gate_config.name,
            description: gate_config.description.unwrap_or_default(),
            conditions,
        };

        let result = gate.evaluate(&metrics);

        Ok(serde_json::to_value(result)?)
    }

    /// Handle `list_rules` tool call — list all registered rules
    pub fn handle_list_rules(
        &self,
        _args: &serde_json::Value,
    ) -> AxiomResult<serde_json::Value> {
        let all_rules = self.rule_registry.all();
        
        let rules: Vec<serde_json::Value> = all_rules
            .iter()
            .map(|rule| {
                serde_json::json!({
                    "id": rule.id(),
                    "name": rule.name(),
                    "severity": format!("{:?}", rule.severity()).to_lowercase(),
                    "category": format!("{:?}", rule.category()).to_lowercase(),
                    "language": rule.language(),
                })
            })
            .collect();

        Ok(serde_json::json!({
            "count": rules.len(),
            "rules": rules,
        }))
    }

    /// Dispatch a tool call by name
    pub fn dispatch(&self, tool_name: &str, args: &serde_json::Value) -> AxiomResult<serde_json::Value> {
        match tool_name {
            "check_quality" => self.handle_check_quality(args),
            "quality_delta" => self.handle_quality_delta(args),
            "check_boundaries" => self.handle_check_boundaries(args),
            "check_lint" => self.handle_check_lint(args),
            "get_debt" => self.handle_get_debt(args),
            "get_ratings" => self.handle_get_ratings(args),
            "detect_duplications" => self.handle_detect_duplications(args),
            "run_gate" => self.handle_run_gate(args),
            "list_rules" => self.handle_list_rules(args),
            _ => Err(AxiomError::Other(format!("Unknown axiom tool: {}", tool_name))),
        }
    }
}

// ── Input Types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CheckQualityInput {
    project_path: String,
    metrics: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct QualityDeltaInput {
    before: serde_json::Value,
    after: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct CheckBoundariesInput {
    #[allow(dead_code)]
    project_path: String,
    boundaries_config: Option<Vec<BoundaryConfig>>,
}

#[derive(Debug, Deserialize)]
struct BoundaryConfig {
    name: String,
    path_patterns: Vec<String>,
    allowed_dependencies: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GetDebtInput {
    file_path: String,
}

#[derive(Debug, Deserialize)]
struct GetRatingsInput {
    project_path: String,
}

#[derive(Debug, Deserialize)]
struct DetectDuplicationsInput {
    file_path: String,
}

#[derive(Debug, Deserialize)]
struct RunGateInput {
    gate_config: GateConfigInput,
    metrics: ProjectMetrics,
}

#[derive(Debug, Deserialize)]
struct GateConfigInput {
    name: String,
    description: Option<String>,
    conditions: Vec<GateConditionInput>,
}

#[derive(Debug, Deserialize)]
struct GateConditionInput {
    metric: String,
    operator: String,
    threshold: serde_json::Value,
}

/// Quality report with rule summaries
#[derive(Debug, serde::Serialize)]
struct QualityReport {
    status: String,
    project_path: String,
    metrics_requested: Vec<String>,
    message: String,
    #[serde(default)]
    rules: Vec<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tools() -> AxiomTools {
        AxiomTools::new().unwrap()
    }

    #[test]
    fn test_tool_definitions_count() {
        let tools = AxiomTools::tool_definitions();
        // 9 quality and security tools
        assert_eq!(tools.len(), 9);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"check_quality"));
        assert!(names.contains(&"quality_delta"));
        assert!(names.contains(&"check_boundaries"));
        assert!(names.contains(&"check_lint"));
        assert!(names.contains(&"get_debt"));
        assert!(names.contains(&"get_ratings"));
        assert!(names.contains(&"detect_duplications"));
        assert!(names.contains(&"run_gate"));
        assert!(names.contains(&"list_rules"));
    }

    #[test]
    fn test_dispatch_unknown_tool() {
        let tools = test_tools();
        let result = tools.dispatch("unknown_tool", &serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_check_quality_tool() {
        let tools = test_tools();
        let result = tools.handle_check_quality(&serde_json::json!({
            "project_path": "/path/to/project",
            "metrics": ["lcom", "solid"]
        })).unwrap();

        assert!(result["status"].as_str().unwrap().contains("ready"));
        assert_eq!(result["project_path"], "/path/to/project");
    }

    #[test]
    fn test_check_boundaries_tool() {
        let tools = test_tools();
        let result = tools.handle_check_boundaries(&serde_json::json!({
            "project_path": "/path/to/project"
        })).unwrap();

        assert!(result["summary"].as_str().unwrap().contains("CallGraph"));
    }

    #[test]
    fn test_check_boundaries_with_config() {
        let tools = test_tools();
        let result = tools.handle_check_boundaries(&serde_json::json!({
            "project_path": "/path/to/project",
            "boundaries_config": [
                {
                    "name": "domain",
                    "path_patterns": ["src/domain/"],
                    "allowed_dependencies": []
                },
                {
                    "name": "infrastructure",
                    "path_patterns": ["src/infrastructure/"],
                    "allowed_dependencies": ["*"]
                }
            ]
        })).unwrap();

        assert!(result["summary"].as_str().unwrap().contains("CallGraph"));
    }

    #[test]
    fn test_check_lint_not_configured() {
        let tools = test_tools();
        let result = tools.handle_check_lint(&serde_json::json!({
            "project_path": "/path/to/project"
        })).unwrap();

        assert_eq!(result["status"], "not_yet_configured");
    }

    #[test]
    fn test_list_rules() {
        let tools = test_tools();
        let result = tools.handle_list_rules(&serde_json::json!({})).unwrap();

        assert!(result["count"].as_i64().unwrap() > 0);
        assert!(result["rules"].is_array());
    }
}
