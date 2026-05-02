//! CogniCode Quality Analysis MCP Server
//!
//! This binary exposes quality analysis tools from cognicode-axiom as MCP tools.

use anyhow::Result;
use clap::Parser;
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

/// Quality Analysis Server Handler
struct QualityAnalysisHandler {
    cwd: PathBuf,
    rule_registry: RuleRegistry,
}

impl QualityAnalysisHandler {
    fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            rule_registry: RuleRegistry::discover(),
        }
    }

    fn count_functions_in_context(ctx: &RuleContext) -> usize {
        let query_str = format!("({}) @func", ctx.language.function_node_type());
        ctx.count_matches(&query_str)
    }

    fn aggregate_issues_by_severity(issues: &[Issue]) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for issue in issues {
            *counts.entry(format!("{:?}", issue.severity)).or_insert(0) += 1;
        }
        counts
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

    fn default_gate(&self) -> QualityGate {
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

    fn language_name(language: Language) -> String {
        language.name().to_lowercase()
    }

    fn analyze_file_impl(&self, params: AnalyzeFileParams) -> Result<FileAnalysisResult> {
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

    fn analyze_project_impl(&self, params: AnalyzeProjectParams) -> Result<ProjectAnalysisResult> {
        let root = if params.project_path.display().to_string() == "." {
            self.cwd.clone()
        } else {
            params.project_path
        };

        let mut all_issues = Vec::new();
        let mut file_metrics: HashMap<String, FileMetricsResult> = HashMap::new();

        let walker = ignore::WalkBuilder::new(&root)
            .hidden(false)
            .git_ignore(true)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

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

            file_metrics.insert(
                path.display().to_string(),
                FileMetricsResult {
                    lines_of_code: source.lines().count(),
                    function_count: Self::count_functions_in_context(&ctx),
                    issues_by_severity: Self::aggregate_issues_by_severity(&file_issues),
                },
            );
        }

        let total_loc: usize = file_metrics.values().map(|m| m.lines_of_code).sum();
        let total_functions: usize = file_metrics.values().map(|m| m.function_count).sum();

        let code_smells = all_issues.iter().filter(|i| matches!(i.category, cognicode_axiom::rules::Category::CodeSmell)).count();
        let bugs = all_issues.iter().filter(|i| matches!(i.category, cognicode_axiom::rules::Category::Bug)).count();
        let vulnerabilities = all_issues.iter().filter(|i| matches!(i.category, cognicode_axiom::rules::Category::Vulnerability)).count();
        let issues_by_severity = Self::aggregate_issues_by_severity(&all_issues);
        let issues_result: Vec<IssueResult> = all_issues.into_iter().map(IssueResult::from).collect();

        Ok(ProjectAnalysisResult {
            project_path: root.display().to_string(),
            total_files: file_metrics.len(),
            total_issues: issues_result.len(),
            issues: issues_result,
            file_metrics,
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
        })
    }
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

                // Get project metrics
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
                let issues: Vec<Issue> = project_result.issues.iter().cloned().map(IssueResult::into_issue).collect();
                let debt = calculator.calculate(&issues, project_result.project_metrics.ncloc);
                Ok(serde_json::to_string_pretty(&TechnicalDebtReportResult::from(debt))?)
            }
            "get_project_ratings" => {
                let params: GetRatingsParams = serde_json::from_value(args).unwrap_or_default();
                let project_result = self.analyze_project_impl(AnalyzeProjectParams { project_path: params.project_path })?;
                let issues: Vec<Issue> = project_result.issues.iter().cloned().map(IssueResult::into_issue).collect();
                let debt = cognicode_axiom::rules::TechnicalDebtCalculator::new().calculate(&issues, project_result.project_metrics.ncloc);
                let ratings = cognicode_axiom::rules::ProjectRatings::compute(&issues, project_result.project_metrics.ncloc, &debt);
                Ok(serde_json::to_string_pretty(&ProjectRatingsResult::from(ratings))?)
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
                    Ok(serde_json::to_string_pretty(&DuplicationResult::from_groups(groups))?)
                } else if let Some(file_path) = params.file_path {
                    let source = std::fs::read_to_string(&file_path)?;
                    let groups = detector.detect_duplications(&source);
                    Ok(serde_json::to_string_pretty(&DuplicationResult::from_groups(groups))?)
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

                // Get or load the gate
                let gate = self.default_gate();

                // Get project metrics
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
                        "severity": details, // placeholder
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
}

// ─────────────────────────────────────────────────────────────────────────────
// Type Definitions
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AnalyzeFileParams { file_path: PathBuf }

#[derive(Debug, Deserialize, Default)]
struct AnalyzeProjectParams { project_path: PathBuf }

#[derive(Debug, Deserialize, Default)]
struct GetTechnicalDebtParams { project_path: PathBuf }

#[derive(Debug, Deserialize, Default)]
struct GetRatingsParams { project_path: PathBuf }

#[derive(Debug, Deserialize, Default)]
struct DetectDuplicationsParams { file_path: Option<PathBuf>, project_path: Option<PathBuf> }

#[derive(Debug, Deserialize)]
struct CheckCodeSmellParams { rule_id: String, file_path: String }

#[derive(Debug, Deserialize)]
struct GetQualityProfileParams { profile_name: String }

#[derive(Debug, Deserialize)]
struct AnalyzeComplexityParams { file_path: String }

#[derive(Debug, Deserialize)]
struct CheckNamingParams { file_path: String, convention: Option<String> }

#[derive(Debug, Deserialize)]
struct GetFileMetricsParams { file_path: String }

#[derive(Debug, Deserialize)]
struct GetQualityGateParams { gate_name: String, project_path: Option<PathBuf> }

#[derive(Debug, Deserialize)]
struct RunQualityGateParams { gate_name: String, project_path: Option<PathBuf> }

#[derive(Debug, Deserialize)]
struct CheckLintParams { project_path: Option<PathBuf>, linters: Option<Vec<String>> }

#[derive(Debug, Deserialize, Default)]
struct GetRemediationParams { project_path: PathBuf, max_issues: Option<u32> }

#[derive(Debug, Deserialize)]
struct TestRuleParams { rule_id: String, source: String, language: Option<String> }

#[derive(Debug, Deserialize, Default)]
struct ListSmellsParams { project_path: PathBuf }

#[derive(Debug, Deserialize)]
struct LoadAdrParams { adr_path: Option<String>, adr_directory: Option<String> }

// Result types
#[derive(Debug, Serialize)]
struct FileAnalysisResult {
    file_path: String,
    issues: Vec<IssueResult>,
    metrics: FileMetricsResult,
    success: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProjectAnalysisResult {
    project_path: String,
    total_files: usize,
    total_issues: usize,
    issues: Vec<IssueResult>,
    file_metrics: HashMap<String, FileMetricsResult>,
    project_metrics: ProjectMetricsResult,
    success: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct IssueResult {
    rule_id: String,
    message: String,
    severity: String,
    category: String,
    file: String,
    line: usize,
    column: Option<usize>,
}

impl IssueResult {
    fn into_issue(self) -> Issue {
        Issue::new(&self.rule_id, &self.message,
            match self.severity.as_str() {
                "Info" => Severity::Info,
                "Minor" => Severity::Minor,
                "Major" => Severity::Major,
                "Critical" => Severity::Critical,
                _ => Severity::Minor,
            },
            cognicode_axiom::rules::Category::CodeSmell,
            std::path::PathBuf::from(&self.file),
            self.line,
        )
    }
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
struct FileMetricsResult {
    lines_of_code: usize,
    function_count: usize,
    issues_by_severity: HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct ProjectMetricsResult {
    ncloc: usize,
    functions: usize,
    classes: usize,
    code_smells: usize,
    bugs: usize,
    vulnerabilities: usize,
    issues_by_severity: HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct RuleInfo {
    id: String,
    name: String,
    severity: String,
    category: String,
    language: String,
}

#[derive(Debug, Serialize)]
struct ComplexityResult {
    file_path: String,
    total_complexity: i32,
}

#[derive(Debug, Serialize)]
struct NamingIssue {
    line: usize,
    column: usize,
    identifier: String,
    expected_convention: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct RemediationSuggestion {
    rule_id: String,
    message: String,
    effort_minutes: u32,
    description: String,
}

// Technical Debt Result (local type with Serialize)
#[derive(Debug, Serialize)]
struct TechnicalDebtReportResult {
    total_debt_minutes: u64,
    debt_ratio: f64,
    rating: String,
    total_issues: usize,
    ncloc: usize,
}

impl From<cognicode_axiom::rules::TechnicalDebtReport> for TechnicalDebtReportResult {
    fn from(r: cognicode_axiom::rules::TechnicalDebtReport) -> Self {
        Self {
            total_debt_minutes: r.total_debt_minutes,
            debt_ratio: r.debt_ratio,
            rating: format!("{:?}", r.rating),
            total_issues: r.total_issues,
            ncloc: r.ncloc,
        }
    }
}

// Project Ratings Result (local type with Serialize)
#[derive(Debug, Serialize)]
struct ProjectRatingsResult {
    reliability: String,
    security: String,
    maintainability: String,
    overall: char,
}

impl From<cognicode_axiom::rules::ProjectRatings> for ProjectRatingsResult {
    fn from(r: cognicode_axiom::rules::ProjectRatings) -> Self {
        Self {
            reliability: format!("{:?}", r.reliability),
            security: format!("{:?}", r.security),
            maintainability: format!("{:?}", r.maintainability),
            overall: r.overall(),
        }
    }
}

// Duplication Result
#[derive(Debug, Serialize)]
struct DuplicationResult {
    groups: Vec<DuplicationGroupResult>,
    total_duplicates: usize,
}

#[derive(Debug, Serialize)]
struct DuplicationGroupResult {
    lines: usize,
    hash: u32,
    locations: Vec<DuplicationLocationResult>,
}

#[derive(Debug, Serialize)]
struct DuplicationLocationResult {
    file: String,
    start_line: usize,
    end_line: usize,
}

impl DuplicationResult {
    fn from_groups(groups: Vec<cognicode_axiom::rules::DuplicationGroup>) -> Self {
        let total = groups.iter().map(|g| g.locations.len()).sum();
        Self {
            groups: groups.into_iter().map(|g| DuplicationGroupResult {
                lines: g.lines,
                hash: g.hash,
                locations: g.locations.into_iter().map(|l| DuplicationLocationResult {
                    file: l.file,
                    start_line: l.start_line,
                    end_line: l.end_line,
                }).collect(),
            }).collect(),
            total_duplicates: total,
        }
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
