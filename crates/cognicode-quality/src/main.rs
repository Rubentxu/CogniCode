//! CogniCode Quality Analysis MCP Server
//!
//! This binary exposes quality analysis tools from cognicode-axiom as MCP tools.

mod handler;
mod incremental;

use anyhow::Result;
use clap::Parser;
use handler::{
    QualityAnalysisHandler, AnalyzeFileParams, AnalyzeProjectParams,
    GetTechnicalDebtParams, GetRatingsParams, DetectDuplicationsParams,
    CheckCodeSmellParams, GetQualityGateParams, RunQualityGateParams,
    CheckLintParams, GetRemediationParams, TestRuleParams, ListSmellsParams,
    LoadAdrParams, FileAnalysisResult, IssueResult, FileMetricsResult,
    ProjectMetricsResult, RuleInfo, ComplexityResult, NamingIssue,
    RemediationSuggestion, TechnicalDebtReportResult, ProjectRatingsResult,
    DuplicationResult, DuplicationGroupResult, DuplicationLocationResult,
    GetQualityProfileParams, AnalyzeComplexityParams, CheckNamingParams, GetFileMetricsParams,
};
use cognicode_axiom::linters::{ClippyRunner, Linter};
use cognicode_axiom::rules::types::{Issue, RuleContext, RuleRegistry, Severity};
use cognicode_axiom::rules::{
    CompareOperator, DuplicationDetector, FileMetrics, GateCondition, MetricValue,
    ProjectMetrics as AxiomProjectMetrics, QualityGate,
};
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::parser::Language;
use rayon::ThreadPoolBuilder;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{CallToolRequestParams, CallToolResult, Content, ListToolsResult, ServerCapabilities, ServerInfo, Tool};
use rmcp::service::{RequestContext, RoleServer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use tracing::info;

/// CLI arguments for the quality server
#[derive(Parser)]
#[command(name = "cognicode-quality", version, about = "CogniCode Quality Analysis MCP Server")]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,
}

// ─────────────────────────────────────────────────────────────────────────────
// ServerHandler implementation
// ─────────────────────────────────────────────────────────────────────────────

impl ServerHandler for QualityAnalysisHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder().enable_tools().build(),
        )
        .with_server_info(rmcp::model::Implementation::new("cognicode-quality", env!("CARGO_PKG_VERSION")))
        .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_03_26)
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_ {
        async move {
            Ok(ListToolsResult {
                tools: self.list_tools(),
                meta: None,
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_ {
        let handler = self;
        async move {
            let result = handler.handle_tool_call(&request.name, request.arguments.unwrap_or_default()).await;
            match result {
                Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
            }
        }
    }
}

impl QualityAnalysisHandler {
    fn list_tools(&self) -> Vec<Tool> {
        vec![
            Tool::new("analyze_file", "Run all quality rules on a single file", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"file_path": {"type": "string"}}, "required": ["file_path"]}).as_object().cloned().unwrap())),
            Tool::new("analyze_project", "Run all quality rules on an entire project", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"project_path": {"type": "string"}}}).as_object().cloned().unwrap())),
            Tool::new("get_rule_registry", "List all available code quality rules", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {}}).as_object().cloned().unwrap())),
            Tool::new("get_quality_gate", "Evaluate quality gate conditions", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"gate_name": {"type": "string"}}}).as_object().cloned().unwrap())),
            Tool::new("get_technical_debt", "Calculate SQALE technical debt", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"project_path": {"type": "string"}}}).as_object().cloned().unwrap())),
            Tool::new("get_project_ratings", "Get A-E ratings", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"project_path": {"type": "string"}}}).as_object().cloned().unwrap())),
            Tool::new("detect_duplications", "Find duplicate code blocks", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"file_path": {"type": "string"}, "project_path": {"type": "string"}}}).as_object().cloned().unwrap())),
            Tool::new("check_code_smell", "Check specific code smell", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"rule_id": {"type": "string"}, "file_path": {"type": "string"}}, "required": ["rule_id", "file_path"]}).as_object().cloned().unwrap())),
            Tool::new("get_quality_profile", "Get quality profile", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"profile_name": {"type": "string"}}}).as_object().cloned().unwrap())),
            Tool::new("list_quality_profiles", "List quality profiles", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {}}).as_object().cloned().unwrap())),
            Tool::new("analyze_complexity", "Get complexity metrics", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"file_path": {"type": "string"}}, "required": ["file_path"]}).as_object().cloned().unwrap())),
            Tool::new("check_naming_convention", "Check naming conventions", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"file_path": {"type": "string"}, "convention": {"type": "string"}}}).as_object().cloned().unwrap())),
            Tool::new("get_file_metrics", "Get file metrics", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"file_path": {"type": "string"}}, "required": ["file_path"]}).as_object().cloned().unwrap())),
            Tool::new("run_quality_gate", "Run quality gate", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"gate_name": {"type": "string"}, "project_path": {"type": "string"}}}).as_object().cloned().unwrap())),
            Tool::new("check_lint", "Run linters on project", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"project_path": {"type": "string"}, "linters": {"type": "array", "items": {"type": "string"}}}}).as_object().cloned().unwrap())),
            Tool::new("get_remediation_suggestions", "Get remediation suggestions", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"project_path": {"type": "string"}, "max_issues": {"type": "number"}}}).as_object().cloned().unwrap())),
            Tool::new("test_rule", "Test a rule against source code fixture", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"rule_id": {"type": "string"}, "source": {"type": "string"}, "language": {"type": "string", "default": "rust"}}}).as_object().cloned().unwrap())),
            Tool::new("list_smells", "List all code smells found in a project with counts", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"project_path": {"type": "string"}}}).as_object().cloned().unwrap())),
            Tool::new("load_adrs", "Parse ADR files and convert to quality rules", std::sync::Arc::new(serde_json::json!({"type": "object", "properties": {"adr_path": {"type": "string"}, "adr_directory": {"type": "string"}}}).as_object().cloned().unwrap())),
        ]
    }

    async fn handle_tool_call(&self, name: &str, arguments: serde_json::Map<String, serde_json::Value>) -> anyhow::Result<String> {
        let args = serde_json::Value::Object(arguments);
        match name {
            "analyze_file" => {
                let params: AnalyzeFileParams = serde_json::from_value(args)?;
                let result = self.analyze_file_impl(params)?;
                Ok(serde_json::to_string_pretty(&result)?)
            }
            "analyze_project" => {
                let params: AnalyzeProjectParams = serde_json::from_value(args).unwrap_or_default();
                let result = self.analyze_project_impl(params)?;
                Ok(serde_json::to_string_pretty(&result)?)
            }
            "get_rule_registry" => {
                let rules: Vec<RuleInfo> = self.rule_registry.all().iter().map(|r| RuleInfo {
                    id: r.id().to_string(),
                    name: r.name().to_string(),
                    severity: format!("{:?}", r.severity()),
                    category: format!("{:?}", r.category()),
                    language: r.language().to_string(),
                }).collect();
                Ok(serde_json::to_string_pretty(&rules)?)
            }
            "get_quality_gate" => {
                let params: GetQualityGateParams = serde_json::from_value(args)?;
                let gate = self.default_gate();

                let project_path = params.project_path.unwrap_or_else(|| self.cwd.clone());
                let project_result = self.analyze_project_impl(AnalyzeProjectParams { project_path })?;

                let mut metrics = AxiomProjectMetrics::new();
                metrics.code_smells = project_result.project_metrics.code_smells;
                metrics.bugs = project_result.project_metrics.bugs;
                metrics.vulnerabilities = project_result.project_metrics.vulnerabilities;
                metrics.ncloc = project_result.project_metrics.ncloc;

                let result = gate.evaluate(&metrics);
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "gate_name": result.gate_name,
                    "passed": result.passed,
                    "blocked": result.blocked,
                    "conditions": result.condition_results.iter().map(|c| serde_json::json!({
                        "metric": c.metric,
                        "passed": c.passed,
                        "expected": c.threshold,
                        "actual": c.actual_value,
                        "message": c.message,
                    })).collect::<Vec<_>>(),
                }))?)
            }
            "get_technical_debt" => {
                let params: GetTechnicalDebtParams = serde_json::from_value(args).unwrap_or_default();
                let project_result = self.analyze_project_impl(AnalyzeProjectParams { project_path: params.project_path })?;
                let calculator = cognicode_axiom::rules::TechnicalDebtCalculator::new();
                let issues: Vec<Issue> = project_result.issues.iter().cloned().map(|i| Issue::new(&i.rule_id, &i.message, Severity::Minor, cognicode_axiom::rules::Category::CodeSmell, std::path::PathBuf::from(&i.file), i.line)).collect();
                let debt = calculator.calculate(&issues, project_result.project_metrics.ncloc);
                let debt_result = TechnicalDebtReportResult {
                    total_debt_minutes: debt.total_debt_minutes,
                    debt_ratio: debt.debt_ratio,
                    rating: format!("{:?}", debt.rating),
                    total_issues: debt.total_issues,
                    ncloc: debt.ncloc,
                };
                Ok(serde_json::to_string_pretty(&debt_result)?)
            }
            "get_project_ratings" => {
                let params: GetRatingsParams = serde_json::from_value(args).unwrap_or_default();
                let project_result = self.analyze_project_impl(AnalyzeProjectParams { project_path: params.project_path })?;
                let issues: Vec<Issue> = project_result.issues.iter().cloned().map(|i| Issue::new(&i.rule_id, &i.message, Severity::Minor, cognicode_axiom::rules::Category::CodeSmell, std::path::PathBuf::from(&i.file), i.line)).collect();
                let debt = cognicode_axiom::rules::TechnicalDebtCalculator::new().calculate(&issues, project_result.project_metrics.ncloc);
                let ratings = cognicode_axiom::rules::ProjectRatings::compute(&issues, project_result.project_metrics.ncloc, &debt);
                let ratings_result = ProjectRatingsResult {
                    reliability: format!("{:?}", ratings.reliability),
                    security: format!("{:?}", ratings.security),
                    maintainability: format!("{:?}", ratings.maintainability),
                    overall: ratings.overall(),
                };
                Ok(serde_json::to_string_pretty(&ratings_result)?)
            }
            "detect_duplications" => {
                let params: DetectDuplicationsParams = serde_json::from_value(args).unwrap_or_default();
                let detector = DuplicationDetector::new();
                if let Some(project_path) = params.project_path {
                    let mut files: Vec<(String, String)> = Vec::new();
                    let walker = ignore::WalkBuilder::new(&project_path).hidden(false).git_ignore(true).build();
                    for entry in walker.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(ext) = path.extension() {
                                if let Some(lang) = Language::from_extension(Some(ext)) {
                                    if !lang.name().is_empty() {
                                        if let Ok(content) = std::fs::read_to_string(path) {
                                            files.push((path.display().to_string(), content));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let groups = detector.detect_multi_file_duplications(&files);
                    let dup_result = DuplicationResult {
                        groups: groups.iter().map(|g| DuplicationGroupResult {
                            lines: g.lines,
                            hash: g.hash,
                            locations: g.locations.iter().map(|l| DuplicationLocationResult {
                                file: l.file.clone(),
                                start_line: l.start_line,
                                end_line: l.end_line,
                            }).collect(),
                        }).collect(),
                        total_duplicates: groups.iter().map(|g| g.locations.len()).sum(),
                    };
                    Ok(serde_json::to_string_pretty(&dup_result)?)
                } else if let Some(file_path) = params.file_path {
                    let source = std::fs::read_to_string(&file_path)?;
                    let groups = detector.detect_duplications(&source);
                    let dup_result = DuplicationResult {
                        groups: groups.iter().map(|g| DuplicationGroupResult {
                            lines: g.lines,
                            hash: g.hash,
                            locations: g.locations.iter().map(|l| DuplicationLocationResult {
                                file: l.file.clone(),
                                start_line: l.start_line,
                                end_line: l.end_line,
                            }).collect(),
                        }).collect(),
                        total_duplicates: groups.iter().map(|g| g.locations.len()).sum(),
                    };
                    Ok(serde_json::to_string_pretty(&dup_result)?)
                } else {
                    Err(anyhow::anyhow!("Either file_path or project_path required"))
                }
            }
            "check_code_smell" => {
                let params: CheckCodeSmellParams = serde_json::from_value(args)?;
                let file_path = PathBuf::from(&params.file_path);
                let source = std::fs::read_to_string(&file_path)?;
                let ext = file_path.extension();
                let language = Language::from_extension(ext.map(OsStr::new)).unwrap_or(Language::Rust);

                let mut parser = tree_sitter::Parser::new();
                let ts_language = language.to_ts_language();
                if parser.set_language(&ts_language).is_err() {
                    return Err(anyhow::anyhow!("Failed to set language"));
                }
                let tree = match parser.parse(&source, None) {
                    Some(t) => t,
                    None => return Err(anyhow::anyhow!("Failed to parse file")),
                };
                let graph = CallGraph::default();
                let metrics = FileMetrics::default();

                let ctx = RuleContext {
                    tree: &tree,
                    source: &source,
                    file_path: &file_path,
                    language: &language,
                    graph: &graph,
                    metrics: &metrics,
                };

                let mut found_issues = Vec::new();
                for rule in self.rule_registry.all() {
                    if rule.id() == params.rule_id {
                        let issues = rule.check(&ctx);
                        found_issues.extend(issues);
                        break;
                    }
                }
                let results: Vec<IssueResult> = found_issues.into_iter().map(IssueResult::from).collect();
                Ok(serde_json::to_string_pretty(&results)?)
            }
            "get_quality_profile" => {
                let params: GetQualityProfileParams = serde_json::from_value(args)?;

                let profile_yaml = r#"
profiles:
  - name: "cognicode-default"
    description: "Default CogniCode quality profile"
    language: "rust"
    is_default: true
    rules:
      - rule_id: "S138"
        enabled: true
        parameters:
          threshold: 50
      - rule_id: "S3776"
        enabled: true
        parameters:
          threshold: 15
      - rule_id: "S134"
        enabled: true
        parameters:
          threshold: 4
      - rule_id: "S107"
        enabled: true
        parameters:
          threshold: 7
"#;

                let engine = cognicode_axiom::rules::QualityProfileEngine::from_yaml(profile_yaml)
                    .unwrap_or_else(|_| cognicode_axiom::rules::QualityProfileEngine::default());

                let profile = engine.resolve_profile(&params.profile_name);
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "name": profile.name,
                    "description": profile.language,
                    "language": "rust",
                    "rules": profile.rules.iter().map(|(id, config)| serde_json::json!({
                        "rule_id": id,
                        "enabled": config.enabled,
                        "severity": format!("{:?}", config.severity),
                        "parameters": config.parameters,
                    })).collect::<Vec<_>>(),
                }))?)
            }
            "list_quality_profiles" => {
                Ok(serde_json::to_string_pretty(&["cognicode-default"])?)
            }
            "analyze_complexity" => {
                let params: AnalyzeComplexityParams = serde_json::from_value(args)?;
                let file_path = PathBuf::from(&params.file_path);
                let source = std::fs::read_to_string(&file_path)?;
                let ext = file_path.extension();
                let language = Language::from_extension(ext.map(OsStr::new)).unwrap_or(Language::Rust);

                let mut parser = tree_sitter::Parser::new();
                let ts_language = language.to_ts_language();
                if parser.set_language(&ts_language).is_err() {
                    return Err(anyhow::anyhow!("Failed to set language"));
                }
                let tree = match parser.parse(&source, None) {
                    Some(t) => t,
                    None => return Err(anyhow::anyhow!("Failed to parse file")),
                };
                let graph = CallGraph::default();
                let metrics = FileMetrics::default();

                let ctx = RuleContext {
                    tree: &tree,
                    source: &source,
                    file_path: &file_path,
                    language: &language,
                    graph: &graph,
                    metrics: &metrics,
                };

                let complexity_result = ComplexityResult {
                    file_path: params.file_path,
                    total_complexity: ctx.cognitive_complexity(ctx.tree.root_node()),
                };
                Ok(serde_json::to_string_pretty(&complexity_result)?)
            }
            "check_naming_convention" => {
                let params: CheckNamingParams = serde_json::from_value(args)?;
                let convention = params.convention.unwrap_or_else(|| "snake_case".to_string());
                let source = std::fs::read_to_string(&params.file_path)?;
                let issues = self.check_naming_impl(&source, &convention);
                Ok(serde_json::to_string_pretty(&issues)?)
            }
            "get_file_metrics" => {
                let params: GetFileMetricsParams = serde_json::from_value(args)?;
                let file_path = PathBuf::from(&params.file_path);
                let source = std::fs::read_to_string(&file_path)?;
                let ext = file_path.extension();
                let language = Language::from_extension(ext.map(OsStr::new)).unwrap_or(Language::Rust);

                let mut parser = tree_sitter::Parser::new();
                let ts_language = language.to_ts_language();
                if parser.set_language(&ts_language).is_err() {
                    return Err(anyhow::anyhow!("Failed to set language"));
                }
                let tree = match parser.parse(&source, None) {
                    Some(t) => t,
                    None => return Err(anyhow::anyhow!("Failed to parse file")),
                };
                let graph = CallGraph::default();
                let metrics = FileMetrics::default();

                let ctx = RuleContext {
                    tree: &tree,
                    source: &source,
                    file_path: &file_path,
                    language: &language,
                    graph: &graph,
                    metrics: &metrics,
                };

                let result = FileMetricsResult {
                    lines_of_code: source.lines().count(),
                    function_count: Self::count_functions_in_context(&ctx),
                    issues_by_severity: HashMap::new(),
                };
                Ok(serde_json::to_string_pretty(&result)?)
            }
            "run_quality_gate" => {
                let params: RunQualityGateParams = serde_json::from_value(args)?;

                let gate = self.default_gate();

                let project_path = params.project_path.unwrap_or_else(|| self.cwd.clone());
                let project_result = self.analyze_project_impl(AnalyzeProjectParams { project_path })?;

                let mut metrics = AxiomProjectMetrics::new();
                metrics.code_smells = project_result.project_metrics.code_smells;
                metrics.bugs = project_result.project_metrics.bugs;
                metrics.vulnerabilities = project_result.project_metrics.vulnerabilities;
                metrics.ncloc = project_result.project_metrics.ncloc;

                let result = gate.evaluate(&metrics);
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "gate_name": result.gate_name,
                    "passed": result.passed,
                    "blocked": result.blocked,
                    "condition_count": result.condition_results.len(),
                    "failed_count": result.condition_results.iter().filter(|c| !c.passed).count(),
                    "summary": if result.passed { "PASSED" } else if result.blocked { "BLOCKED" } else { "FAILED" },
                }))?)
            }
            "check_lint" => {
                let params: CheckLintParams = serde_json::from_value(args)?;
                let project_path = params.project_path.unwrap_or_else(|| self.cwd.clone());
                let linters = params.linters.unwrap_or_else(|| vec!["clippy".to_string()]);

                let mut results = Vec::new();
                for linter in &linters {
                    match linter.as_str() {
                        "clippy" => {
                            let runner = ClippyRunner::new();
                            match runner.run(&project_path) {
                                Ok(report) => {
                                    for issue in report.issues {
                                        results.push(serde_json::json!({
                                            "linter": "clippy",
                                            "message": issue.message,
                                            "file": issue.file.display().to_string(),
                                            "line": issue.line,
                                            "column": issue.column,
                                            "severity": format!("{:?}", issue.severity),
                                            "code": issue.code,
                                        }));
                                    }
                                }
                                Err(e) => {
                                    results.push(serde_json::json!({
                                        "linter": "clippy",
                                        "error": e.to_string(),
                                    }));
                                }
                            }
                        }
                        _ => {
                            results.push(serde_json::json!({
                                "linter": linter,
                                "status": "not_available",
                                "message": format!("Linter '{}' not yet supported", linter),
                            }));
                        }
                    }
                }
                Ok(serde_json::to_string_pretty(&results)?)
            }
            "get_remediation_suggestions" => {
                let params: GetRemediationParams = serde_json::from_value(args).unwrap_or_default();
                let max_issues = params.max_issues.unwrap_or(10) as usize;
                let project_result = self.analyze_project_impl(AnalyzeProjectParams { project_path: params.project_path })?;
                let suggestions: Vec<_> = project_result.issues.into_iter().take(max_issues).map(|issue| RemediationSuggestion {
                    rule_id: issue.rule_id,
                    message: issue.message,
                    effort_minutes: 15,
                    description: "Consider refactoring".to_string(),
                }).collect();
                Ok(serde_json::to_string_pretty(&suggestions)?)
            }
            "test_rule" => {
                let params: TestRuleParams = serde_json::from_value(args)?;
                let source = params.source;
                let lang_str = params.language.unwrap_or_else(|| "rust".to_string());
                let language = Language::from_extension(Some(std::ffi::OsStr::new(match lang_str.as_str() {
                    "rust" => "rs",
                    "python" => "py",
                    "javascript" => "js",
                    "typescript" => "ts",
                    "go" => "go",
                    "java" => "java",
                    _ => "rs",
                }))).unwrap_or(Language::Rust);

                let mut parser = tree_sitter::Parser::new();
                parser.set_language(&language.to_ts_language())
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                let tree = parser.parse(&source, None)
                    .ok_or_else(|| anyhow::anyhow!("Failed to parse"))?;

                let file_path = PathBuf::from("<fixture>");
                let graph = CallGraph::default();
                let metrics = FileMetrics::default();
                let ctx = RuleContext {
                    tree: &tree, source: &source, file_path: &file_path,
                    language: &language, graph: &graph, metrics: &metrics,
                };

                let mut results = Vec::new();
                for rule in self.rule_registry.all() {
                    if rule.id() == params.rule_id {
                        let issues = rule.check(&ctx);
                        results = issues.into_iter().map(IssueResult::from).collect();
                        break;
                    }
                }

                if results.is_empty() {
                    Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "rule_id": params.rule_id,
                        "issues_found": 0,
                        "status": "passed",
                        "issues": []
                    }))?)
                } else {
                    Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "rule_id": params.rule_id,
                        "issues_found": results.len(),
                        "status": "issues_found",
                        "issues": results,
                    }))?)
                }
            }
            "list_smells" => {
                let params: ListSmellsParams = serde_json::from_value(args).unwrap_or_default();
                let project_result = self.analyze_project_impl(AnalyzeProjectParams { project_path: params.project_path })?;

                let mut smell_counts: HashMap<String, usize> = HashMap::new();
                let mut smell_details: HashMap<String, Vec<IssueResult>> = HashMap::new();

                for issue in project_result.issues {
                    *smell_counts.entry(issue.rule_id.clone()).or_insert(0) += 1;
                    smell_details.entry(issue.rule_id.clone()).or_default().push(issue);
                }

                let smells: Vec<_> = smell_counts.into_iter().map(|(rule_id, count)| {
                    let details = smell_details.get(&rule_id).map(|v| v.len()).unwrap_or(0);
                    serde_json::json!({
                        "rule_id": rule_id,
                        "count": count,
                        "severity": details,
                    })
                }).collect();

                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "total_smells": project_result.total_issues,
                    "smells": smells,
                }))?)
            }
            "load_adrs" => {
                let params: LoadAdrParams = serde_json::from_value(args)?;
                let mut results = Vec::new();

                if let Some(path) = &params.adr_path {
                    let content = std::fs::read_to_string(path)?;
                    match cognicode_axiom::rules::AdrParser::parse(&content) {
                        Ok(adr) => {
                            results.push(serde_json::json!({
                                "file": path,
                                "title": adr.title,
                                "status": adr.status,
                                "decision": adr.decision,
                            }));
                        }
                        Err(e) => {
                            results.push(serde_json::json!({
                                "file": path,
                                "error": e.to_string(),
                            }));
                        }
                    }
                }

                if let Some(dir) = &params.adr_directory {
                    let dir_path = PathBuf::from(dir);
                    if dir_path.is_dir() {
                        for entry in std::fs::read_dir(&dir_path)? {
                            let entry = entry?;
                            let path = entry.path();
                            if path.extension().map(|e| e == "md").unwrap_or(false) {
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    if let Ok(adr) = cognicode_axiom::rules::AdrParser::parse(&content) {
                                        results.push(serde_json::json!({
                                            "file": path.display().to_string(),
                                            "title": adr.title,
                                            "status": adr.status,
                                            "decision": adr.decision,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "adrs_loaded": results.len(),
                    "adrs": results,
                }))?)
            }
            _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
        }
    }

    fn check_naming_impl(&self, source: &str, convention: &str) -> Vec<NamingIssue> {
        let mut issues = Vec::new();
        let re = match convention {
            "snake_case" => regex::Regex::new(r"\b(let|const)\s+(?:mut\s+)?([A-Z][a-zA-Z0-9_]*)").unwrap(),
            "camelCase" => regex::Regex::new(r"\b(let|const)\s+(?:mut\s+)?([a-z]+_[a-z])").unwrap(),
            _ => regex::Regex::new(r"\b(let|const)\s+(?:mut\s+)?([A-Z][a-zA-Z0-9_]*)").unwrap(),
        };

        for (line_num, line) in source.lines().enumerate() {
            for cap in re.captures_iter(line) {
                if let Some(name) = cap.get(2) {
                    issues.push(NamingIssue {
                        line: line_num + 1,
                        column: 1,
                        identifier: name.as_str().to_string(),
                        expected_convention: convention.to_string(),
                        message: format!("Use {} naming convention", convention),
                    });
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Entry Point
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if !args.cwd.exists() {
        eprintln!("Error: Directory '{}' does not exist", args.cwd.display());
        std::process::exit(1);
    }

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .compact()
        .with_writer(std::io::stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    match ThreadPoolBuilder::new()
        .stack_size(8 * 1024 * 1024)
        .build_global()
    {
        Ok(_) => info!("Rayon global thread pool initialized with 8 MB stack size"),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already been initialized") {
                tracing::warn!("Rayon global thread pool already initialized; using existing configuration");
            } else {
                panic!("Failed to initialize Rayon global thread pool: {}", e);
            }
        }
    }

    info!("Starting CogniCode Quality MCP Server v{} on port 8001", env!("CARGO_PKG_VERSION"));

    let handler = QualityAnalysisHandler::new(args.cwd);
    let transport = rmcp::transport::io::stdio();
    let server = rmcp::serve_server(handler, transport).await?;

    server.waiting().await?;

    Ok(())
}
