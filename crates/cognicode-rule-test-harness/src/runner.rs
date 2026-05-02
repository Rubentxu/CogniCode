use anyhow::Result;
use cognicode_axiom::rules::types::*;
use cognicode_axiom::rules::RuleRegistry;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;

pub struct RuleRunner {
    registry: RuleRegistry,
}

impl RuleRunner {
    /// Create a new RuleRunner with discovered rules
    pub fn new() -> Self {
        Self {
            registry: RuleRegistry::discover(),
        }
    }

    /// Run a specific rule against a fixture file
    pub fn run_rule_on_file(
        &self,
        rule_id: &str,
        fixture_dir: &Path,
        file_path: &str,
    ) -> Result<Vec<Issue>> {
        let full_path = fixture_dir.join(file_path);
        let source = std::fs::read_to_string(&full_path)?;
        let ext = full_path.extension().and_then(|e| e.to_str());
        let language = Self::detect_language(ext, &full_path);

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language.to_ts_language())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| anyhow::anyhow!("Parse failed"))?;

        let graph = CallGraph::default();
        let metrics = FileMetrics::default();

        let ctx = RuleContext {
            tree: &tree,
            source: &source,
            file_path: &full_path,
            language: &language,
            graph: &graph,
            metrics: &metrics,
        };

        let mut issues = Vec::new();
        for rule in self.registry.all() {
            if rule.id() == rule_id {
                issues = rule.check(&ctx);
                break;
            }
        }

        Ok(issues)
    }

    /// Run all rules against a file
    pub fn run_all_rules_on_file(&self, file_path: &Path) -> Result<Vec<Issue>> {
        let source = std::fs::read_to_string(file_path)?;
        let ext = file_path.extension().and_then(|e| e.to_str());
        let language = Self::detect_language(ext, file_path);

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language.to_ts_language())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| anyhow::anyhow!("Parse failed"))?;

        let graph = CallGraph::default();
        let metrics = FileMetrics::default();

        let ctx = RuleContext {
            tree: &tree,
            source: &source,
            file_path,
            language: &language,
            graph: &graph,
            metrics: &metrics,
        };

        let lang_name = language.name();
        let applicable_rules = self.registry.for_language(lang_name);

        let issues: Vec<Issue> = applicable_rules
            .iter()
            .flat_map(|rule| rule.check(&ctx))
            .collect();

        Ok(issues)
    }

    /// Get all registered rule IDs
    pub fn get_rule_ids(&self) -> Vec<String> {
        self.registry.all().iter().map(|r| r.id().to_string()).collect()
    }

    /// Get the count of registered rules
    pub fn rule_count(&self) -> usize {
        self.registry.count()
    }

    fn detect_language(ext: Option<&str>, _path: &Path) -> Language {
        match ext {
            Some("rs") => Language::Rust,
            Some("js") => Language::JavaScript,
            Some("ts") | Some("tsx") => Language::TypeScript,
            Some("py") => Language::Python,
            Some("go") => Language::Go,
            Some("java") => Language::Java,
            _ => Language::Rust,
        }
    }
}

impl Default for RuleRunner {
    fn default() -> Self {
        Self::new()
    }
}
