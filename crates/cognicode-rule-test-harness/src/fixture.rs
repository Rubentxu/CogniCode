use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Expected issue for test case validation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpectedIssue {
    pub rule_id: String,
    pub severity: String,
    pub line: Option<usize>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Fixture {
    pub rule_id: String,
    pub language: String,
    pub test_cases: Vec<TestCase>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TestCase {
    pub name: String,
    pub file: String,
    pub description: String,
    pub expected_min_issues: usize,
    pub expected_max_issues: usize,
    #[serde(default)]
    pub expected_rule_ids: Vec<String>,
    #[serde(default)]
    pub expected_severities: Vec<String>,
}

impl Fixture {
    /// Load a fixture from a directory path
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let expected_path = path.join("expected.json");
        if !expected_path.exists() {
            anyhow::bail!("expected.json not found in {}", path.display());
        }
        let content = std::fs::read_to_string(&expected_path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Get all fixture directories in a root directory
    pub fn find_fixtures(root: &std::path::Path) -> anyhow::Result<Vec<PathBuf>> {
        let mut fixtures = Vec::new();
        if !root.exists() {
            return Ok(fixtures);
        }

        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let expected_json = path.join("expected.json");
                if expected_json.exists() {
                    fixtures.push(path);
                }
            }
        }

        Ok(fixtures)
    }
}
