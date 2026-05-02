//! Quality Analysis Handler Module
//!
//! Contains the QualityAnalysisHandler and all related types.

use anyhow::Result;
use cognicode_axiom::linters::{ClippyRunner, Linter};
use cognicode_axiom::rules::types::{Issue, RuleContext, RuleRegistry, Severity};
use cognicode_axiom::rules::{
    CompareOperator, DuplicationDetector, FileMetrics, GateCondition, MetricValue,
    ProjectMetrics as AxiomProjectMetrics, QualityGate,
};
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::parser::Language;
use rayon::ThreadPoolBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use tracing::info;

use crate::incremental::{AnalysisState, BaselineDiff};



/// Quality Analysis Handler - exposes analysis functionality
pub struct QualityAnalysisHandler {
    pub cwd: PathBuf,
    pub rule_registry: RuleRegistry,
}

impl QualityAnalysisHandler {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            rule_registry: RuleRegistry::discover(),
        }
    }

    pub fn count_functions_in_context(ctx: &RuleContext) -> usize {
        let query_str = format!("({}) @func", ctx.language.function_node_type());
        ctx.count_matches(&query_str)
    }

    pub fn aggregate_issues_by_severity(issues: &[Issue]) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for issue in issues {
            *counts.entry(format!("{:?}", issue.severity)).or_insert(0) += 1;
        }
        counts
    }

    pub fn language_name(language: Language) -> String {
        language.name().to_lowercase()
    }

    pub fn analyze_file_impl(&self, params: AnalyzeFileParams) -> Result<FileAnalysisResult> {
        let source = std::fs::read_to_string(&params.file_path)?;
        let ext = params.file_path.extension();
        let language = Language::from_extension(ext.map(OsStr::new)).unwrap_or(Language::Rust);

        let mut parser = tree_sitter::Parser::new();
        let ts_language = language.to_ts_language();
        if parser.set_language(&ts_language).is_err() {
            return Ok(FileAnalysisResult {
                file_path: params.file_path.display().to_string(),
                issues: vec![],
                metrics: FileMetricsResult::default(),
                success: false,
                error: Some("Failed to set language".to_string()),
            });
        }
        let tree = match parser.parse(&source, None) {
            Some(t) => t,
            None => {
                return Ok(FileAnalysisResult {
                    file_path: params.file_path.display().to_string(),
                    issues: vec![],
                    metrics: FileMetricsResult::default(),
                    success: false,
                    error: Some("Failed to parse file".to_string()),
                });
            }
        };
        let graph = CallGraph::default();
        let metrics = FileMetrics::default();

        let ctx = RuleContext {
            tree: &tree,
            source: &source,
            file_path: &params.file_path,
            language: &language,
            graph: &graph,
            metrics: &metrics,
        };

        let mut all_issues = Vec::new();
        let lang_name = Self::language_name(language);
        let rules = self.rule_registry.for_language(&lang_name);
        for rule in rules {
            let issues = rule.check(&ctx);
            all_issues.extend(issues);
        }

        let file_metrics = FileMetricsResult {
            lines_of_code: source.lines().count(),
            function_count: Self::count_functions_in_context(&ctx),
            issues_by_severity: Self::aggregate_issues_by_severity(&all_issues),
        };

        Ok(FileAnalysisResult {
            file_path: params.file_path.display().to_string(),
            issues: all_issues.into_iter().map(IssueResult::from).collect(),
            metrics: file_metrics,
            success: true,
            error: None,
        })
    }

    pub fn analyze_project_impl(&self, params: AnalyzeProjectParams) -> Result<ProjectAnalysisResult> {
        let root = params.project_path;

        // === INCREMENTAL: Load state ===
        let mut state = AnalysisState::load(&root);

        // Collect all files
        let mut all_files = Vec::new();
        let walker = ignore::WalkBuilder::new(&root)
            .hidden(false)
            .git_ignore(true)
            .build();
        for entry in walker.flatten() {
            if entry.path().is_file() {
                all_files.push(entry.path().to_path_buf());
            }
        }

        // === INCREMENTAL: Find changed files ===
        let changed_files = state.find_changed_files(&all_files);
        let total_files = all_files.len();
        let changed_count = changed_files.len();
        let reused_count = total_files - changed_count;

        info!("Incremental: {}/{} files changed, {} reused from cache",
              changed_count, total_files, reused_count);

        // Only analyze changed files
        let mut all_issues = Vec::new();
        let mut file_metrics_map: HashMap<String, FileMetricsResult> = HashMap::new();

        for path in &changed_files {
            let ext = path.extension();
            let language = match Language::from_extension(ext.map(OsStr::new)) {
                Some(lang) => lang,
                None => continue,
            };

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let mut parser = tree_sitter::Parser::new();
            let ts_language = language.to_ts_language();
            if parser.set_language(&ts_language).is_err() {
                continue;
            }
            let tree = match parser.parse(&source, None) {
                Some(t) => t,
                None => continue,
            };
            let graph = CallGraph::default();
            let metrics = FileMetrics::default();

            let ctx = RuleContext {
                tree: &tree,
                source: &source,
                file_path: path,
                language: &language,
                graph: &graph,
                metrics: &metrics,
            };

            let mut file_issues = Vec::new();
            let lang_name = Self::language_name(language);
            let rules = self.rule_registry.for_language(&lang_name);
            for rule in rules {
                let issues = rule.check(&ctx);
                file_issues.extend(issues);
            }

            all_issues.extend(file_issues.clone());

            let issues_count = file_issues.len();
            state.update_file_state(path, issues_count);

            file_metrics_map.insert(
                path.display().to_string(),
                FileMetricsResult {
                    lines_of_code: source.lines().count(),
                    function_count: Self::count_functions_in_context(&ctx),
                    issues_by_severity: Self::aggregate_issues_by_severity(&file_issues),
                },
            );
        }

        // Recover cached issues for unchanged files
        for path in &all_files {
            if !changed_files.contains(path) {
                let key = path.to_string_lossy().to_string();
                if let Some(file_state) = state.file_states.get(&key) {
                    // Carry forward metrics from cached state
                    if let Ok(source) = std::fs::read_to_string(path) {
                        file_metrics_map.insert(
                            key.clone(),
                            FileMetricsResult {
                                lines_of_code: source.lines().count(),
                                function_count: 0, // Not re-computed for cached files
                                issues_by_severity: HashMap::new(),
                            },
                        );
                    }
                }
            }
        }

        let total_loc: usize = file_metrics_map.values().map(|m| m.lines_of_code).sum();
        let total_functions: usize = file_metrics_map.values().map(|m| m.function_count).sum();

        let code_smells = all_issues.iter().filter(|i| matches!(i.category, cognicode_axiom::rules::Category::CodeSmell)).count();
        let bugs = all_issues.iter().filter(|i| matches!(i.category, cognicode_axiom::rules::Category::Bug)).count();
        let vulnerabilities = all_issues.iter().filter(|i| matches!(i.category, cognicode_axiom::rules::Category::Vulnerability)).count();
        let blockers = all_issues.iter().filter(|i| matches!(i.severity, Severity::Blocker)).count();
        let criticals = all_issues.iter().filter(|i| matches!(i.severity, Severity::Critical)).count();
        let issues_by_severity = Self::aggregate_issues_by_severity(&all_issues);

        // === New code issues (computed before consuming all_issues) ===
        let new_code_files = state.get_new_code_files();
        let new_code_issues = all_issues.iter()
            .filter(|issue| {
                let file_str = issue.file.to_string_lossy().to_string();
                new_code_files.contains(&file_str)
            })
            .count();

        let issues_result: Vec<IssueResult> = all_issues.into_iter().map(IssueResult::from).collect();

        // === Compute metrics ===
        let total_issues = issues_result.len();
        // Debt estimation: simplified - 5 min per code smell, 15 min per bug, 30 min per vulnerability
        let debt = (code_smells as u64 * 5) + (bugs as u64 * 15) + (vulnerabilities as u64 * 30);
        let rating = if blockers > 0 || bugs > 10 {
            "F"
        } else if code_smells > 50 || vulnerabilities > 5 {
            "C"
        } else if code_smells > 25 {
            "B"
        } else {
            "A"
        };

        // === Persist state ===
        // Auto-set baseline on first analysis if not already set
        if state.baseline.is_none() {
            state.set_baseline(total_issues, debt, rating, blockers, criticals);
        }
        state.add_snapshot(total_issues, debt, rating, changed_count, 0, 0);
        state.save();

        // === Diff vs baseline ===
        let baseline_diff = state.diff_vs_baseline(total_issues, debt, rating, blockers);

        Ok(ProjectAnalysisResult {
            project_path: root.display().to_string(),
            total_files: file_metrics_map.len(),
            total_issues: issues_result.len(),
            issues: issues_result,
            file_metrics: file_metrics_map,
            project_metrics: ProjectMetricsResult {
                ncloc: total_loc,
                functions: total_functions,
                classes: 0,
                code_smells,
                bugs,
                vulnerabilities,
                issues_by_severity,
            },
            success: true,
            error: None,
            incremental: IncrementalInfo {
                files_total: total_files,
                files_changed: changed_count,
                files_reused: reused_count,
                baseline_diff,
                new_code_issues,
            },
        })
    }

    pub fn default_gate(&self) -> QualityGate {
        QualityGate::new("cognicode-default", "Default CogniCode quality gate")
            .add_condition(GateCondition::new(
                "code_smells",
                CompareOperator::LT,
                MetricValue::Integer(50),
            ))
            .add_condition(GateCondition::new(
                "bugs",
                CompareOperator::LT,
                MetricValue::Integer(10),
            ))
            .add_condition(GateCondition::new(
                "vulnerabilities",
                CompareOperator::LT,
                MetricValue::Integer(5),
            ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AnalyzeFileParams {
    pub file_path: PathBuf,
}

#[derive(Debug, Deserialize, Default)]
pub struct AnalyzeProjectParams {
    pub project_path: PathBuf,
}

#[derive(Debug, Deserialize, Default)]
pub struct GetTechnicalDebtParams {
    pub project_path: PathBuf,
}

#[derive(Debug, Deserialize, Default)]
pub struct GetRatingsParams {
    pub project_path: PathBuf,
}

#[derive(Debug, Deserialize, Default)]
pub struct DetectDuplicationsParams {
    pub file_path: Option<PathBuf>,
    pub project_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct CheckCodeSmellParams {
    pub rule_id: String,
    pub file_path: String,
}

#[derive(Debug, Deserialize)]
pub struct GetQualityProfileParams {
    pub profile_name: String,
}

#[derive(Debug, Deserialize)]
pub struct AnalyzeComplexityParams {
    pub file_path: String,
}

#[derive(Debug, Deserialize)]
pub struct CheckNamingParams {
    pub file_path: String,
    pub convention: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetFileMetricsParams {
    pub file_path: String,
}

#[derive(Debug, Deserialize)]
pub struct GetQualityGateParams {
    pub gate_name: String,
    pub project_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct RunQualityGateParams {
    pub gate_name: String,
    pub project_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct CheckLintParams {
    pub project_path: Option<PathBuf>,
    pub linters: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GetRemediationParams {
    pub project_path: PathBuf,
    pub max_issues: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct TestRuleParams {
    pub rule_id: String,
    pub source: String,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListSmellsParams {
    pub project_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct LoadAdrParams {
    pub adr_path: Option<String>,
    pub adr_directory: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SetBaselineParams {
    pub project_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GetDiffParams {
    pub project_path: Option<PathBuf>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Result Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct FileAnalysisResult {
    pub file_path: String,
    pub issues: Vec<IssueResult>,
    pub metrics: FileMetricsResult,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectAnalysisResult {
    pub project_path: String,
    pub total_files: usize,
    pub total_issues: usize,
    pub issues: Vec<IssueResult>,
    pub file_metrics: HashMap<String, FileMetricsResult>,
    pub project_metrics: ProjectMetricsResult,
    pub success: bool,
    pub error: Option<String>,
    pub incremental: IncrementalInfo,
}

#[derive(Debug, Serialize)]
pub struct IncrementalInfo {
    pub files_total: usize,
    pub files_changed: usize,
    pub files_reused: usize,
    pub baseline_diff: Option<BaselineDiff>,
    pub new_code_issues: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssueResult {
    pub rule_id: String,
    pub message: String,
    pub severity: String,
    pub category: String,
    pub file: String,
    pub line: usize,
    pub column: Option<usize>,
}

impl From<Issue> for IssueResult {
    fn from(issue: Issue) -> Self {
        Self {
            rule_id: issue.rule_id,
            message: issue.message,
            severity: format!("{:?}", issue.severity),
            category: format!("{:?}", issue.category),
            file: issue.file.display().to_string(),
            line: issue.line,
            column: issue.column,
        }
    }
}

#[derive(Debug, Serialize, Default)]
pub struct FileMetricsResult {
    pub lines_of_code: usize,
    pub function_count: usize,
    pub issues_by_severity: HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct ProjectMetricsResult {
    pub ncloc: usize,
    pub functions: usize,
    pub classes: usize,
    pub code_smells: usize,
    pub bugs: usize,
    pub vulnerabilities: usize,
    pub issues_by_severity: HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct RuleInfo {
    pub id: String,
    pub name: String,
    pub severity: String,
    pub category: String,
    pub language: String,
}

#[derive(Debug, Serialize)]
pub struct ComplexityResult {
    pub file_path: String,
    pub total_complexity: i32,
}

#[derive(Debug, Serialize)]
pub struct NamingIssue {
    pub line: usize,
    pub column: usize,
    pub identifier: String,
    pub expected_convention: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RemediationSuggestion {
    pub rule_id: String,
    pub message: String,
    pub effort_minutes: u32,
    pub description: String,
}

// Technical Debt Result
#[derive(Debug, Serialize)]
pub struct TechnicalDebtReportResult {
    pub total_debt_minutes: u64,
    pub debt_ratio: f64,
    pub rating: String,
    pub total_issues: usize,
    pub ncloc: usize,
}

// Project Ratings Result
#[derive(Debug, Serialize)]
pub struct ProjectRatingsResult {
    pub reliability: String,
    pub security: String,
    pub maintainability: String,
    pub overall: char,
}

// Duplication Result
#[derive(Debug, Serialize)]
pub struct DuplicationResult {
    pub groups: Vec<DuplicationGroupResult>,
    pub total_duplicates: usize,
}

#[derive(Debug, Serialize)]
pub struct DuplicationGroupResult {
    pub lines: usize,
    pub hash: u32,
    pub locations: Vec<DuplicationLocationResult>,
}

#[derive(Debug, Serialize)]
pub struct DuplicationLocationResult {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
}
