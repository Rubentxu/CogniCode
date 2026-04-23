//! Scenario Manifest Schema and Parsing
//!
//! Manifests are YAML files that declare the scenario matrix:
//! repo, language, tool, action, workspace, expected outcome, and validation pipeline.
//!
//! Version 2.0 adds ground_truth and metrics sections for quality scoring.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Root of a scenario manifest YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Version of the manifest schema (e.g., "1.0" or "2.0")
    pub version: String,
    /// Language this manifest targets (rust, python, javascript, typescript, java, go)
    pub language: String,
    /// Tier A (functional) or Tier B (expected-fail probe)
    pub tier: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Repo name for real-repo scenarios (cloned to repos_dir/)
    /// Inherits to scenarios that don't specify their own repo.
    #[serde(default)]
    pub repo: Option<String>,
    /// Default validation pipeline stages
    #[serde(default)]
    pub validation: ValidationPipeline,
    /// List of scenario definitions
    pub scenarios: Vec<ScenarioDef>,
    /// Global defaults for retry policy
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,
    /// Default timeout in seconds (overridable per scenario)
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    120
}

/// A single scenario definition within a manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDef {
    /// Unique scenario name within this manifest
    pub name: String,
    /// Human-readable description
    pub description: Option<String>,
    /// MCP tool to call
    pub tool: String,
    /// Tool action (e.g., rename, extract, inline, read)
    pub action: String,
    /// Tool-specific arguments (JSON as YAML)
    #[serde(default)]
    pub arguments: HashMap<String, serde_json::Value>,
    /// Workspace path relative to repo root (where to run)
    pub workspace: String,
    /// Repo name (cloned to repos_dir/) — for Tier B real-repo scenarios
    /// If not set, falls back to language defaults (serde for Rust, click for Python, fixture for others)
    #[serde(default)]
    pub repo: Option<String>,
    /// Expected outcome
    #[serde(default = "default_expected_outcome")]
    pub expected_outcome: String,
    /// Override validation stages for this scenario (optional)
    #[serde(default)]
    pub validation: Option<ValidationPipeline>,
    /// Override timeout for this scenario (optional)
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// Scenario class (read_only, analysis, mutation)
    #[serde(default)]
    pub scenario_class: String,
    /// Whether this is a preview-only tool call (no disk mutation expected)
    #[serde(default)]
    pub preview_only: bool,
    /// Variant within the same tool/action (e.g., "concrete", "syntax_rejected", "regression")
    #[serde(default)]
    pub variant: Option<String>,
    /// Ground truth for correctness comparison (v2.0, backward compatible)
    /// Contains expected symbols, outline, code, complexity, usages, etc.
    #[serde(default)]
    pub ground_truth: Option<serde_json::Value>,
    /// Metrics definition for quality scoring (v2.0, backward compatible)
    /// Contains correctness type, latency targets, etc.
    #[serde(default)]
    pub metrics: Option<serde_json::Value>,
}

fn default_expected_outcome() -> String {
    "pass".to_string()
}

/// Validation pipeline definition — ordered stages executed after mutation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationPipeline {
    /// Ordered list of validation stages
    #[serde(default)]
    pub stages: Vec<StageDef>,
    /// Whether stage failure is considered a regression (true) or expected (false)
    #[serde(default = "default_true")]
    pub failure_is_regression: bool,
}

fn default_true() -> bool {
    true
}

/// A single validation stage (e.g., syntax, format, lint, build, test).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageDef {
    /// Stage name
    pub name: String,
    /// Commands to run in order; first non-zero exit stops the stage
    pub commands: Vec<String>,
    /// Expected exit code range [min, max] (default [0, 0])
    #[serde(default = "default_exit_range")]
    pub expected_exit_range: (i32, i32),
    /// Timeout for this stage in seconds (default 60)
    #[serde(default = "default_stage_timeout")]
    pub timeout_seconds: u64,
}

fn default_exit_range() -> (i32, i32) {
    (0, 0)
}

fn default_stage_timeout() -> u64 {
    60
}

/// Retry policy for infra-level failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of attempts
    pub max_attempts: u32,
    /// Failure classes that trigger retry
    pub on: Vec<String>,
}

impl Manifest {
    /// Parse a YAML manifest file.
    pub fn from_path(path: &PathBuf) -> Result<Self, ManifestError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ManifestError::IoError(path.clone(), e))?;
        Self::from_str(&content)
    }

    /// Parse a YAML manifest from a string.
    pub fn from_str(content: &str) -> Result<Self, ManifestError> {
        serde_yaml::from_str(content).map_err(ManifestError::ParseError)
    }

    /// Expand the manifest into a flat list of concrete scenario instances.
    /// Each scenario inherits global defaults and gets a unique ID.
    pub fn expand(&self) -> Vec<ExpandedScenario> {
        let mut scenarios = Vec::new();
        for def in &self.scenarios {
            let validation = def
                .validation
                .clone()
                .unwrap_or_else(|| self.validation.clone());
            let timeout = def.timeout_seconds.unwrap_or(self.timeout_seconds);
            // Propagate top-level repo to scenario if scenario doesn't have its own
            // Empty string "" means "explicitly no repo" (prevents inheritance)
            // None means "inherit from manifest level"
            let repo = match &def.repo {
                Some(r) if r.is_empty() => None,
                Some(r) => Some(r.clone()),
                None => self.repo.clone(),
            };
            let scenario = ExpandedScenario {
                id: format!(
                    "{}_{}_{}",
                    self.language,
                    def.name,
                    def.variant.as_deref().unwrap_or("default")
                ),
                language: self.language.clone(),
                tier: self.tier.clone(),
                tool: def.tool.clone(),
                action: def.action.clone(),
                arguments: def.arguments.clone(),
                workspace: def.workspace.clone(),
                expected_outcome: def.expected_outcome.clone(),
                validation,
                timeout_seconds: timeout,
                scenario_class: def.scenario_class.clone(),
                preview_only: def.preview_only,
                variant: def.variant.clone(),
                ground_truth: def.ground_truth.clone(),
                metrics: def.metrics.clone(),
                repo,         // Inherits from manifest top-level if not set in scenario
                commit: None, // Set by orchestrator
                container_image: None,
            };
            scenarios.push(scenario);
        }
        scenarios
    }
}

/// A fully-expanded scenario ready for execution.
#[derive(Debug, Clone, Serialize)]
pub struct ExpandedScenario {
    /// Unique scenario ID
    pub id: String,
    pub language: String,
    pub tier: String,
    pub tool: String,
    pub action: String,
    pub arguments: HashMap<String, serde_json::Value>,
    pub workspace: String,
    pub expected_outcome: String,
    pub validation: ValidationPipeline,
    pub timeout_seconds: u64,
    pub scenario_class: String,
    pub preview_only: bool,
    pub variant: Option<String>,
    /// Ground truth for correctness comparison (v2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ground_truth: Option<serde_json::Value>,
    /// Metrics definition for quality scoring (v2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<serde_json::Value>,
    /// Set by orchestrator from manifest metadata
    pub repo: Option<String>,
    pub commit: Option<String>,
    pub container_image: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("failed to read manifest file {0}: {1}")]
    IoError(PathBuf, std::io::Error),
    #[error("failed to parse YAML: {0}")]
    ParseError(serde_yaml::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MANIFEST: &str = r#"
version: "1.0"
language: rust
tier: A
description: "Rust Tier A scenarios"

timeout_seconds: 120

validation:
  failure_is_regression: true
  stages:
    - name: syntax
      commands: ["rustfmt --check", "cargo check"]
      timeout_seconds: 60
    - name: format
      commands: ["cargo fmt --check"]
      timeout_seconds: 30

scenarios:
  - name: rename_function
    tool: safe_refactor
    action: rename
    workspace: src/ser.rs
    scenario_class: mutation
    preview_only: false
    description: "Rename a function and validate build/test"
    arguments:
      target: Serialized
      new_name: SerializedNew
      file_path: src/ser.rs
"#;

    #[test]
    fn test_manifest_parse() {
        let manifest = Manifest::from_str(TEST_MANIFEST).unwrap();
        assert_eq!(manifest.language, "rust");
        assert_eq!(manifest.tier, "A");
        assert_eq!(manifest.scenarios.len(), 1);
        assert_eq!(manifest.scenarios[0].name, "rename_function");
        assert_eq!(manifest.validation.stages.len(), 2);
    }

    #[test]
    fn test_manifest_expand() {
        let manifest = Manifest::from_str(TEST_MANIFEST).unwrap();
        let expanded = manifest.expand();
        assert_eq!(expanded.len(), 1);
        let s = &expanded[0];
        assert_eq!(s.language, "rust");
        assert_eq!(s.tool, "safe_refactor");
        assert!(!s.preview_only);
    }

    #[test]
    fn test_manifest_expand_with_variants() {
        let manifest: Manifest = serde_yaml::from_str(TEST_MANIFEST).unwrap();
        let scenarios: Vec<ExpandedScenario> = manifest.expand();
        assert_eq!(scenarios[0].id, "rust_rename_function_default");
    }

    #[test]
    fn test_stage_default_timeout() {
        let stage: StageDef = serde_yaml::from_str("name: test\ncommands: [echo ok]").unwrap();
        assert_eq!(stage.timeout_seconds, 60);
    }

    #[test]
    fn test_scenario_default_expected_outcome() {
        let scenario: ScenarioDef =
            serde_yaml::from_str("name: test\ntool: read_file\naction: read\nworkspace: .")
                .unwrap();
        assert_eq!(scenario.expected_outcome, "pass");
    }

    #[test]
    fn test_expand_repo_inheritance_from_manifest() {
        let manifest_yaml = r#"
version: "1.0"
language: rust
tier: B
repo: my_rust_repo
timeout_seconds: 60
validation:
  failure_is_regression: true
  stages:
    - name: build
      commands: [cargo build]
scenarios:
  - name: test_scenario
    tool: safe_refactor
    action: rename
    workspace: src/lib.rs
"#;
        let manifest: Manifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let scenarios = manifest.expand();
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].repo, Some("my_rust_repo".to_string()));
    }

    #[test]
    fn test_expand_repo_empty_string_overrides_manifest() {
        let manifest_yaml = r#"
version: "1.0"
language: rust
tier: B
repo: manifest_repo
timeout_seconds: 60
validation:
  failure_is_regression: true
  stages:
    - name: build
      commands: [cargo build]
scenarios:
  - name: test_scenario
    tool: safe_refactor
    action: rename
    workspace: src/lib.rs
    repo: ""
"#;
        let manifest: Manifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let scenarios = manifest.expand();
        // Empty string "" means "explicitly no repo" — should NOT inherit
        assert_eq!(scenarios[0].repo, None);
    }

    #[test]
    fn test_expand_scenario_repo_overrides_manifest() {
        let manifest_yaml = r#"
version: "1.0"
language: rust
tier: B
repo: manifest_repo
timeout_seconds: 60
validation:
  failure_is_regression: true
  stages:
    - name: build
      commands: [cargo build]
scenarios:
  - name: test_scenario
    tool: safe_refactor
    action: rename
    workspace: src/lib.rs
    repo: scenario_repo
"#;
        let manifest: Manifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let scenarios = manifest.expand();
        assert_eq!(scenarios[0].repo, Some("scenario_repo".to_string()));
    }

    #[test]
    fn test_expand_validation_inheritance_from_manifest() {
        let manifest_yaml = r#"
version: "1.0"
language: rust
tier: A
timeout_seconds: 60
validation:
  failure_is_regression: true
  stages:
    - name: build
      commands: [cargo build]
    - name: test
      commands: [cargo test]
scenarios:
  - name: test_scenario
    tool: read_file
    action: read
    workspace: src/lib.rs
"#;
        let manifest: Manifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let scenarios = manifest.expand();
        assert_eq!(scenarios[0].validation.stages.len(), 2);
        assert_eq!(scenarios[0].validation.stages[0].name, "build");
        assert_eq!(scenarios[0].validation.stages[1].name, "test");
    }

    #[test]
    fn test_expand_validation_override_per_scenario() {
        let manifest_yaml = r#"
version: "1.0"
language: rust
tier: A
timeout_seconds: 60
validation:
  failure_is_regression: true
  stages:
    - name: build
      commands: [cargo build]
scenarios:
  - name: test_scenario
    tool: read_file
    action: read
    workspace: src/lib.rs
    validation:
      failure_is_regression: false
      stages:
        - name: syntax
          commands: [rustfmt --check]
"#;
        let manifest: Manifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let scenarios = manifest.expand();
        // Scenario should have its own validation, not manifest's
        assert_eq!(scenarios[0].validation.stages.len(), 1);
        assert_eq!(scenarios[0].validation.stages[0].name, "syntax");
        assert!(!scenarios[0].validation.failure_is_regression);
    }

    #[test]
    fn test_expand_timeout_inheritance() {
        let manifest_yaml = r#"
version: "1.0"
language: rust
tier: A
timeout_seconds: 120
validation:
  failure_is_regression: true
  stages:
    - name: build
      commands: [cargo build]
scenarios:
  - name: test_scenario
    tool: read_file
    action: read
    workspace: src/lib.rs
"#;
        let manifest: Manifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let scenarios = manifest.expand();
        assert_eq!(scenarios[0].timeout_seconds, 120);
    }

    #[test]
    fn test_expand_timeout_override_per_scenario() {
        let manifest_yaml = r#"
version: "1.0"
language: rust
tier: A
timeout_seconds: 120
validation:
  failure_is_regression: true
  stages:
    - name: build
      commands: [cargo build]
scenarios:
  - name: test_scenario
    tool: read_file
    action: read
    workspace: src/lib.rs
    timeout_seconds: 30
"#;
        let manifest: Manifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let scenarios = manifest.expand();
        assert_eq!(scenarios[0].timeout_seconds, 30);
    }

    #[test]
    fn test_expand_id_with_variant() {
        let manifest_yaml = r#"
version: "1.0"
language: python
tier: A
timeout_seconds: 60
validation:
  failure_is_regression: true
  stages:
    - name: build
      commands: [python -m py_compile]
scenarios:
  - name: refactor_func
    tool: safe_refactor
    action: rename
    workspace: main.py
    variant: regression
"#;
        let manifest: Manifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let scenarios = manifest.expand();
        assert_eq!(scenarios[0].id, "python_refactor_func_regression");
        assert_eq!(scenarios[0].variant, Some("regression".to_string()));
    }

    #[test]
    fn test_expand_multiple_scenarios_each_gets_id() {
        let manifest_yaml = r#"
version: "1.0"
language: rust
tier: A
timeout_seconds: 60
validation:
  failure_is_regression: true
  stages:
    - name: build
      commands: [cargo build]
scenarios:
  - name: scenario_a
    tool: read_file
    action: read
    workspace: src/a.rs
  - name: scenario_b
    tool: read_file
    action: read
    workspace: src/b.rs
"#;
        let manifest: Manifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let scenarios = manifest.expand();
        assert_eq!(scenarios.len(), 2);
        assert_eq!(scenarios[0].id, "rust_scenario_a_default");
        assert_eq!(scenarios[1].id, "rust_scenario_b_default");
    }
}
