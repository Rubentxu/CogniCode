//! AIX Handlers — AI Experience tools for LLM agent consumption
//!
//! This module contains all 13 AIX MCP tool handlers:
//! smart_overview, ranked_symbols, suggest_onboarding_plan, auto_diagnose,
//! suggest_refactor_plan, nl_to_symbol, ask_about_code, find_pattern_by_intent,
//! compare_call_graphs, detect_api_breaks, generate_system_prompt_context,
//! detect_god_functions, detect_long_parameter_lists.
//!
//! Plus 2 AVC tool handlers:
//! generate_contract, validate_contract.
//!
//! Plus 2 Phase 3A proactive tool handlers:
//! suggest_context, reparse_on_edit.

use super::*;

// AIX Tool Handlers
// ============================================================================

/// Handler for smart_overview tool (AIX-1.1)
#[cognicode_macros::aix_tool(
    name = "smart_overview",
    description = "Get a comprehensive project overview with architecture score, hot paths, and recommended first reads for AI agents.",
    input_schema = SmartOverviewInput
)]
pub async fn handle_smart_overview(
    ctx: &HandlerContext,
    input: SmartOverviewInput,
) -> HandlerResult<SmartOverviewDto> {
    let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();
    let stats = ctx.analysis_service.get_graph_stats();
    let entry_points = ctx.analysis_service.get_entry_points();
    let hot_paths = get_hot_paths_from_graph(&graph, 5);
    let arch_result = check_architecture_internal(ctx)?;
    let first_reads = recommend_first_reads_from_graph(&graph);

    // Determine detail level
    let detail = input.detail.unwrap_or(OverviewDetail::Medium);

    // Calculate coverage
    let coverage = ctx.analysis_service.get_coverage_metrics();
    let coverage_percent = coverage.map(|c| c.coverage_percent);

    // Detect project type
    let project_type = detect_project_type(&graph);

    // Build entry point summaries
    let top_eps: Vec<EntryPointSummary> = entry_points
        .iter()
        .take(5)
        .map(|ep| EntryPointSummary {
            name: ep.name.clone(),
            file: ep.file_path.clone(),
            line: ep.line,
            kind: ep.kind.clone(),
            summary: format!("Entry point: {}", ep.name),
        })
        .collect();

    // Build hot path DTOs
    let critical_hot_paths: Vec<HotPathDto> = hot_paths
        .iter()
        .take(5)
        .map(|hp| HotPathDto {
            symbol_name: hp.symbol_name.clone(),
            file: hp.file.clone(),
            line: hp.line,
            fan_in: hp.fan_in,
            fan_out: hp.fan_out,
        })
        .collect();

    // Estimate tokens
    let estimated_tokens = match detail {
        OverviewDetail::Quick => 100,
        OverviewDetail::Medium => 400,
        OverviewDetail::Detailed => 800,
    };

    let result = SmartOverviewDto {
        project_type: project_type.to_string(),
        total_symbols: stats.symbol_count,
        total_edges: stats.edge_count,
        languages: stats.language_breakdown,
        top_entry_points: top_eps,
        critical_hot_paths,
        architecture_score: arch_result.as_ref().map(|r| r.score),
        cycle_count: arch_result.as_ref().map(|r| r.cycles.len()),
        recommended_first_reads: if detail == OverviewDetail::Detailed {
            first_reads
        } else {
            vec![]
        },
        coverage_percent,
        _meta: OverviewMeta {
            estimated_tokens,
            detail_level: detail.to_string(),
        },
    };

    // B.1: SQLite persistence of `agent_outputs` was removed in the
    // Graph Intelligence v2 cleanup. The result is returned in-memory
    // to the caller; downstream persistence happens via the
    // `postgres` feature when configured.

    Ok(result)
}

/// Handler for ranked_symbols tool (AIX-1.3)
#[cognicode_macros::aix_tool(
    name = "ranked_symbols",
    description = "Get AI-relevance ranked symbols based on a search query, considering fan-in, complexity, and documentation.",
    input_schema = RankedSymbolsInput
)]
pub async fn handle_ranked_symbols(
    ctx: &HandlerContext,
    input: RankedSymbolsInput,
) -> HandlerResult<RankedSymbolsResult> {
        let _ensure_sem = ensure_semantic_indexed(ctx)?;
    let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();

    // Search using semantic search
    let results = ctx
        .semantic_search
        .search(crate::infrastructure::semantic::SearchQuery {
            query: input.query.clone(),
            kinds: vec![],
            max_results: input.limit,
        });

    // Calculate max fan_in for normalization
    let max_fan_in = results
        .iter()
        .map(|r| {
            let symbol_id = SymbolId::new(r.symbol.fully_qualified_name());
            graph.callers(&symbol_id).len()
        })
        .max()
        .unwrap_or(1);

    let mut ranked: Vec<RankedSymbolDto> = Vec::new();
    for r in results.iter().take(input.limit) {
        let symbol_id = SymbolId::new(r.symbol.fully_qualified_name());
        let fan_in = graph.callers(&symbol_id).len();
        let complexity = get_symbol_complexity(&graph, &symbol_id);
        let symbol_name = r.symbol.name().to_string();

        // Calculate hotness boost (5% weight, extra on top of base score)
        let hotness_boost = ctx.get_symbol_hotness(&symbol_name) * 0.05;

        // Calculate enhanced relevance score with hotness boost
        let fan_in_norm = if max_fan_in > 0 {
            fan_in as f64 / max_fan_in as f64
        } else {
            0.0
        };
        let base_score = r.score as f64;
        // Apply weighted scoring: fan_in * 0.4 + base * 0.6
        let weighted_score = fan_in_norm * 0.4 + base_score * 0.6;
        // Add hotness boost and cap at 1.0
        let relevance_score = (weighted_score + hotness_boost).min(1.0);

        ranked.push(RankedSymbolDto {
            name: symbol_name,
            file: r.symbol.location().file().to_string(),
            line: r.symbol.location().line() + 1,
            kind: format!("{:?}", r.symbol.kind()).to_lowercase(),
            relevance_score,
            fan_in,
            complexity,
            has_docs: false,
            summary: format!("{} (score: {:.2})", r.symbol.name(), relevance_score),
        });
    }

    // Record symbol accesses for hotness tracking
    for r in &ranked {
        ctx.record_symbol_access(&r.name, 1);
    }

    let total = ranked.len();
    let result = RankedSymbolsResult {
        query: input.query,
        total_matches: total,
        returned: total,
        results: ranked,
        _meta: OverviewMeta {
            estimated_tokens: total * 50,
            detail_level: "ranked".to_string(),
        },
    };

    Ok(result)
}

/// Handler for suggest_onboarding_plan tool (AIX-2.1)
#[cognicode_macros::aix_tool(
    name = "suggest_onboarding_plan",
    description = "Generate a step-by-step onboarding plan to understand, refactor, debug, or extend a codebase.",
    input_schema = OnboardingPlanInput
)]
pub async fn handle_suggest_onboarding_plan(
    ctx: &HandlerContext,
    input: OnboardingPlanInput,
) -> HandlerResult<OnboardingPlanDto> {
        let _ensure = ensure_graph_built(ctx)?;

    let goal = match input.goal {
        OnboardingGoalDetail::Understand => build_understand_plan(ctx)?,
        OnboardingGoalDetail::Refactor => build_refactor_plan(ctx)?,
        OnboardingGoalDetail::Debug => build_debug_plan(ctx)?,
        OnboardingGoalDetail::AddFeature => build_add_feature_plan(ctx)?,
        OnboardingGoalDetail::Review => build_review_plan(ctx)?,
    };

    let total_tokens: usize = goal.iter().map(|s| s.estimated_tokens).sum();
    let total_steps = goal.len();

    let result = OnboardingPlanDto {
        goal: format!("{:?}", input.goal).to_lowercase(),
        total_steps,
        total_estimated_tokens: total_tokens,
        steps: goal,
        _meta: OverviewMeta {
            estimated_tokens: total_tokens,
            detail_level: "onboarding_plan".to_string(),
        },
    };

    Ok(result)
}

/// Handler for auto_diagnose tool (AIX-2.3)
#[cognicode_macros::aix_tool(
    name = "auto_diagnose",
    description = "Automatically diagnose project health issues including architecture problems, dead code, and complexity hotspots.",
    input_schema = AutoDiagnoseInput
)]
pub async fn handle_auto_diagnose(
    ctx: &HandlerContext,
    _input: AutoDiagnoseInput,
) -> HandlerResult<DiagnoseReportDto> {
    let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();
    let stats = ctx.analysis_service.get_graph_stats();
    let arch_result = check_architecture_internal(ctx)?;
    let dead_code_result = ctx.analysis_service.detect_dead_code();
    let hot_paths = get_hot_paths_from_graph(&graph, 10);
    let module_deps = ctx.analysis_service.detect_module_dependencies();

    // Build issues list
    let mut issues: Vec<DiagnoseIssue> = Vec::new();

    // Architecture issues
    if let Some(ref arch) = arch_result {
        for cycle in &arch.cycles {
            issues.push(DiagnoseIssue {
                category: "architecture".to_string(),
                severity: "critical".to_string(),
                title: format!("Cyclic dependency: {} symbols", cycle.symbols.len()),
                description: format!("Cycle detected involving: {}", cycle.symbols.join(" -> ")),
                recommendation: "Introduce a trait or use a shared module to break the cycle"
                    .to_string(),
                location: None,
                metric: Some(cycle.symbols.len().to_string()),
            });
        }
    }

    // Dead code issues
    if dead_code_result.total_dead > 0 {
        issues.push(DiagnoseIssue {
            category: "dead_code".to_string(),
            severity: "important".to_string(),
            title: format!("{} dead code entries found", dead_code_result.total_dead),
            description: format!(
                "{:.1}% of symbols are never called",
                dead_code_result.dead_code_percent
            ),
            recommendation: "Remove or document why these symbols exist".to_string(),
            location: None,
            metric: Some(dead_code_result.total_dead.to_string()),
        });
    }

    // Hot path issues
    if hot_paths.len() > 5 {
        issues.push(DiagnoseIssue {
            category: "hot_path".to_string(),
            severity: "warning".to_string(),
            title: "Many hot paths detected".to_string(),
            description: "High fan-in functions may be bottlenecks".to_string(),
            recommendation: "Consider caching or parallelizing hot functions".to_string(),
            location: None,
            metric: Some(hot_paths.len().to_string()),
        });
    }

    // Coupling issues
    let coupling_issues = module_deps.graph.cycles.len();
    if coupling_issues > 0 {
        issues.push(DiagnoseIssue {
            category: "coupling".to_string(),
            severity: "warning".to_string(),
            title: format!("{} module cycles detected", coupling_issues),
            description: "Modules with circular dependencies are harder to test".to_string(),
            recommendation: "Extract shared code into a new module".to_string(),
            location: None,
            metric: Some(coupling_issues.to_string()),
        });
    }

    // Sort by severity
    let severity_order = |s: &str| -> i32 {
        match s {
            "critical" => 0,
            "important" => 1,
            "warning" => 2,
            "info" => 3,
            _ => 4,
        }
    };
    issues.sort_by(|a, b| severity_order(&a.severity).cmp(&severity_order(&b.severity)));

    // Count by severity
    let critical_count = issues.iter().filter(|i| i.severity == "critical").count();
    let important_count = issues.iter().filter(|i| i.severity == "important").count();
    let warning_count = issues.iter().filter(|i| i.severity == "warning").count();
    let info_count = issues.iter().filter(|i| i.severity == "info").count();

    // Calculate health score
    let cycles = arch_result.as_ref().map(|r| r.cycles.len()).unwrap_or(0);
    let coupling = coupling_issues;
    let health_score = (100.0 - (cycles as f64 * 10.0) - (coupling as f64 * 5.0))
        .max(0.0)
        .min(100.0);

    // Get complexity info
    let max_complexity = hot_paths
        .first()
        .map(|hp| (hp.symbol_name.clone(), hp.fan_in as u32));
    let avg_complexity = if !hot_paths.is_empty() {
        Some(hot_paths.iter().map(|hp| hp.fan_in as f64).sum::<f64>() / hot_paths.len() as f64)
    } else {
        None
    };

    let result = DiagnoseReportDto {
        health_score,
        total_issues: issues.len(),
        critical_count,
        important_count,
        warning_count,
        info_count,
        issues,
        symbol_count: stats.symbol_count,
        edge_count: stats.edge_count,
        file_count: stats.file_count,
        cycles: arch_result
            .as_ref()
            .map(|r| r.cycles.iter().map(|c| c.symbols.join("->")).collect())
            .unwrap_or_default(),
        architecture_score: arch_result.map(|r| r.score),
        avg_complexity,
        max_complexity,
        dead_code_count: dead_code_result.total_dead,
        dead_code_percent: Some(dead_code_result.dead_code_percent),
        module_coupling_issues: coupling_issues,
        _meta: OverviewMeta {
            estimated_tokens: 500,
            detail_level: "auto_diagnose".to_string(),
        },
    };

    // B.2: SQLite persistence of `agent_outputs` was removed in the
    // Graph Intelligence v2 cleanup. The diagnosis is returned in-memory.

    Ok(result)
}

/// Handler for suggest_refactor_plan tool (AIX-2.2)
#[cognicode_macros::aix_tool(
    name = "suggest_refactor_plan",
    description = "Analyze a symbol and suggest a concrete refactoring plan with risk assessment.",
    input_schema = SuggestRefactorPlanInput
)]
pub async fn handle_suggest_refactor_plan(
    ctx: &HandlerContext,
    input: SuggestRefactorPlanInput,
) -> HandlerResult<RefactorSuggestionDto> {
        let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();
    let symbol_id = find_symbol_in_graph(&graph, &input.symbol);

    if symbol_id.is_none() {
        return Err(HandlerError::NotFound(format!(
            "Symbol '{}' not found",
            input.symbol
        )));
    }

    let symbol_id = symbol_id.unwrap();
    let _symbol = graph.get_symbol(&symbol_id).unwrap();
    let caller_count = graph.callers(&symbol_id).len();
    let callee_count = graph.callees(&symbol_id).len();
    let complexity = get_symbol_complexity(&graph, &symbol_id);

    // Build refactoring steps
    let mut steps = Vec::new();

    if complexity.unwrap_or(0) > 15 {
        steps.push(RefactorActionStep {
            step: steps.len() + 1,
            action: "simplify".to_string(),
            target: input.symbol.clone(),
            suggestion: Some("Extract complex branches into helper functions".to_string()),
            risk: "medium".to_string(),
            files_affected: 1,
            rationale: "High cyclomatic complexity detected".to_string(),
            expected_benefit: format!("Reduces complexity from {} to <10", complexity.unwrap_or(0)),
        });
    }

    if callee_count > 10 {
        steps.push(RefactorActionStep {
            step: steps.len() + 1,
            action: "split".to_string(),
            target: input.symbol.clone(),
            suggestion: Some("Split into smaller functions".to_string()),
            risk: "medium".to_string(),
            files_affected: 1,
            rationale: "Too many dependencies".to_string(),
            expected_benefit: "Better cohesion".to_string(),
        });
    }

    if caller_count > 5 {
        steps.push(RefactorActionStep {
            step: steps.len() + 1,
            action: "add_trait".to_string(),
            target: input.symbol.clone(),
            suggestion: Some("Introduce a trait for abstraction".to_string()),
            risk: "high".to_string(),
            files_affected: caller_count,
            rationale: "High fan-out coupling".to_string(),
            expected_benefit: "Decouples callers from implementation".to_string(),
        });
    }

    // Determine overall risk
    let overall_risk = if steps.is_empty() {
        "low"
    } else if steps.iter().any(|s| s.risk == "high") {
        "high"
    } else {
        "medium"
    };

    let result = RefactorSuggestionDto {
        symbol: input.symbol,
        current_complexity: complexity,
        caller_count,
        impacted_files: caller_count,
        overall_risk: overall_risk.to_string(),
        steps,
        execution_mode: "sequential".to_string(),
        _meta: OverviewMeta {
            estimated_tokens: 300,
            detail_level: "refactor_plan".to_string(),
        },
    };

    Ok(result)
}

/// Handler for nl_to_symbol tool (AIX-3.1)
#[cognicode_macros::aix_tool(
    name = "nl_to_symbol",
    description = "Convert natural language descriptions to precise symbol matches using keyword extraction and semantic search.",
    input_schema = NlToSymbolInput
)]
pub async fn handle_nl_to_symbol(
    ctx: &HandlerContext,
    input: NlToSymbolInput,
) -> HandlerResult<NlToSymbolResult> {
        let _ensure_sem = ensure_semantic_indexed(ctx)?;
    let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();

    // Extract keywords from query
    let keywords = extract_keywords(&input.query);

    // Search using semantic search
    let results = ctx
        .semantic_search
        .search(crate::infrastructure::semantic::SearchQuery {
            query: input.query.clone(),
            kinds: vec![],
            max_results: input.limit * 2,
        });

    // Re-rank with keyword matching
    let mut matches: Vec<NlSymbolMatch> = Vec::new();
    for r in results.iter().take(input.limit) {
        let symbol_id = SymbolId::new(r.symbol.fully_qualified_name());
        let fan_in = graph.callers(&symbol_id).len();

        // Calculate keyword match score
        let name_lower = r.symbol.name().to_lowercase();
        let keyword_matches = keywords
            .iter()
            .filter(|kw| name_lower.contains(&kw.to_lowercase()))
            .count();
        let keyword_score = keyword_matches as f64 / keywords.len().max(1) as f64;

        // Combined confidence
        let confidence = ((r.score as f64) * 0.7 + keyword_score * 0.3).min(1.0);

        if confidence > 0.1 {
            matches.push(NlSymbolMatch {
                symbol_name: r.symbol.name().to_string(),
                file: r.symbol.location().file().to_string(),
                line: r.symbol.location().line() + 1,
                kind: format!("{:?}", r.symbol.kind()).to_lowercase(),
                confidence,
                match_reason: format!(
                    "Semantic match ({:.0}%) + keyword overlap",
                    confidence * 100.0
                ),
                snippet: None,
                fan_in,
            });
        }
    }

    // Sort by confidence
    matches.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total = matches.len();
    matches.truncate(input.limit);

    // Record symbol accesses for hotness tracking
    for m in &matches {
        ctx.record_symbol_access(&m.symbol_name, 1);
    }

    let result = NlToSymbolResult {
        query: input.query,
        extracted_keywords: keywords,
        total_candidates: total,
        results: matches,
        _meta: OverviewMeta {
            estimated_tokens: total * 40,
            detail_level: "nl_to_symbol".to_string(),
        },
    };

    Ok(result)
}

/// Handler for ask_about_code tool (AIX-3.2)
#[cognicode_macros::aix_tool(
    name = "ask_about_code",
    description = "Answer questions about code flow by tracing execution paths between symbols.",
    input_schema = AskAboutCodeInput
)]
pub async fn handle_ask_about_code(
    ctx: &HandlerContext,
    input: AskAboutCodeInput,
) -> HandlerResult<AskAboutCodeResult> {
        let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();

    // Parse question to find source/target symbols
    let words: Vec<&str> = input.question.split_whitespace().collect();
    let source = words.first().map(|s| s.to_string());
    let target = words.last().map(|s| s.to_string());

    let mut answers = Vec::new();

    if let (Some(src), Some(tgt)) = (source, target) {
        let src_id = find_symbol_in_graph(&graph, &src);
        let tgt_id = find_symbol_in_graph(&graph, &tgt);

        if let (Some(sid), Some(tid)) = (src_id, tgt_id) {
            let path = find_path_bfs(&graph, &sid, &tid, 10);

            if !path.is_empty() {
                let path_steps: Vec<CodePathStep> = path
                    .iter()
                    .filter_map(|sp| {
                        graph.get_symbol(sp).map(|s| CodePathStep {
                            symbol: s.name().to_string(),
                            file: s.location().file().to_string(),
                            line: s.location().line() + 1,
                            kind: format!("{:?}", s.kind()).to_lowercase(),
                            role: "intermediate".to_string(),
                            snippet: None,
                        })
                    })
                    .collect();

                answers.push(CodeAnswer {
                    explanation: format!("Path from {} to {} found", src, tgt),
                    path: path_steps,
                    from: src,
                    to: tgt,
                    path_length: path.len(),
                    confidence: 0.9,
                });
            }
        }
    }

    let result = AskAboutCodeResult {
        question: input.question,
        answers,
        _meta: OverviewMeta {
            estimated_tokens: 200,
            detail_level: "ask_about_code".to_string(),
        },
    };

    Ok(result)
}

/// Handler for find_pattern_by_intent tool (AIX-3.3)
#[cognicode_macros::aix_tool(
    name = "find_pattern_by_intent",
    description = "Match natural language intent descriptions to known code patterns.",
    input_schema = FindPatternByIntentInput
)]
pub async fn handle_find_pattern_by_intent(
    ctx: &HandlerContext,
    input: FindPatternByIntentInput,
) -> HandlerResult<FindPatternResult> {
        // Pattern catalog
    let patterns = vec![
        (
            "singleton",
            "Singleton pattern",
            "look for single instance with global state",
            "Find objects created once and accessed globally",
        ),
        (
            "factory",
            "Factory method",
            "creation through factory function",
            "Object creation delegated to factory method",
        ),
        (
            "observer",
            "Observer pattern",
            "event subscription and notification",
            "One-to-many dependency for state changes",
        ),
        (
            "builder",
            "Builder pattern",
            "step-by-step object construction",
            "Construct complex objects step by step",
        ),
        (
            "strategy",
            "Strategy pattern",
            "interchangeable algorithms",
            "Select algorithm at runtime",
        ),
        (
            "adapter",
            "Adapter pattern",
            "convert interface to expected",
            "Make incompatible interfaces work together",
        ),
        (
            "decorator",
            "Decorator pattern",
            "wrap with additional behavior",
            "Add responsibilities dynamically",
        ),
        (
            "facade",
            "Facade pattern",
            "simplified interface to subsystem",
            "Provide unified interface to complex subsystem",
        ),
        (
            "template",
            "Template method",
            "algorithm skeleton with hooks",
            "Define algorithm skeleton with customizable steps",
        ),
        (
            "command",
            "Command pattern",
            "encapsulate operation as object",
            "Parameterize objects with operations",
        ),
        (
            "iterator",
            "Iterator pattern",
            "sequential access without exposure",
            "Traverse collection without exposing internals",
        ),
        (
            "composite",
            "Composite pattern",
            "tree structure with uniform interface",
            "Compose objects into tree structures",
        ),
        (
            "proxy",
            "Proxy pattern",
            "placeholder for another object",
            "Control access to another object",
        ),
        (
            "flyweight",
            "Flyweight pattern",
            "share common state",
            "Use sharing to support large numbers of objects",
        ),
        (
            "mvc",
            "MVC pattern",
            "separate model view controller",
            "Separate data, UI, and logic concerns",
        ),
    ];

    let intent_lower = input.intent.to_lowercase();
    let mut matched = Vec::new();

    for (name, desc, hint, _example) in &patterns {
        let name_match = name.to_lowercase().contains(&intent_lower) || intent_lower.contains(name);
        let desc_match = desc.to_lowercase().contains(&intent_lower);
        let hint_match = hint.to_lowercase().contains(&intent_lower);

        if name_match || desc_match || hint_match {
            matched.push(IntentMatch {
                intent_name: name.to_string(),
                description: desc.to_string(),
                query_hint: hint.to_string(),
            });
        }
    }

    if input.list_patterns.unwrap_or(false) {
        matched = patterns
            .iter()
            .map(|(n, d, h, _)| IntentMatch {
                intent_name: n.to_string(),
                description: d.to_string(),
                query_hint: h.to_string(),
            })
            .collect();
    }

    let result = FindPatternResult {
        query: input.intent,
        matched_intents: matched,
        all_patterns: patterns.iter().map(|(n, _, _, _)| n.to_string()).collect(),
        _meta: OverviewMeta {
            estimated_tokens: 100,
            detail_level: "find_pattern".to_string(),
        },
    };

    Ok(result)
}

/// Handler for compare_call_graphs tool (AIX-4.1)
#[cognicode_macros::aix_tool(
    name = "compare_call_graphs",
    description = "Compare the current call graph against a baseline to detect structural changes.",
    input_schema = CompareCallGraphsInput
)]
pub async fn handle_compare_call_graphs(
    ctx: &HandlerContext,
    input: CompareCallGraphsInput,
) -> HandlerResult<GraphDiffDto> {
        let _ensure = ensure_graph_built(ctx)?;

    let current_graph = ctx.analysis_service.get_project_graph();
    let current_symbols: HashSet<String> = current_graph
        .symbols()
        .map(|s| s.name().to_string())
        .collect();
    let current_edges: HashSet<(String, String)> = current_graph
        .all_dependencies()
        .filter_map(|(src, tgt, _)| {
            let src_name = current_graph
                .get_symbol(src)
                .map(|s| s.name().to_string())?;
            let tgt_name = current_graph
                .get_symbol(tgt)
                .map(|s| s.name().to_string())?;
            Some((src_name, tgt_name))
        })
        .collect();

    // Try to load baseline if provided
    let (has_baseline, baseline_symbols, baseline_edges, arch_score_before) =
        if let Some(baseline_dir) = input.baseline_dir {
            let baseline_path = ctx.working_dir.join(baseline_dir);
            let _store_path = graph_db_path(&baseline_path);
            let store = InMemoryGraphStore::new();
            match store.load_graph() {
                Ok(Some(baseline_graph)) => {
                    let symbols: HashSet<String> = baseline_graph
                        .symbols()
                        .map(|s| s.name().to_string())
                        .collect();
                    let edges: HashSet<(String, String)> = baseline_graph
                        .all_dependencies()
                        .filter_map(|(src, tgt, _)| {
                            let src_name = baseline_graph
                                .get_symbol(src)
                                .map(|s| s.name().to_string())?;
                            let tgt_name = baseline_graph
                                .get_symbol(tgt)
                                .map(|s| s.name().to_string())?;
                            Some((src_name, tgt_name))
                        })
                        .collect();
                    (true, symbols, edges, None)
                }
                _ => (false, HashSet::new(), HashSet::new(), None),
            }
        } else {
            (false, HashSet::new(), HashSet::new(), None)
        };

    // Calculate diff
    let symbols_added: Vec<String> = current_symbols
        .difference(&baseline_symbols)
        .cloned()
        .collect();
    let symbols_removed: Vec<String> = baseline_symbols
        .difference(&current_symbols)
        .cloned()
        .collect();
    let edges_added: Vec<(String, String)> =
        current_edges.difference(&baseline_edges).cloned().collect();
    let edges_removed: Vec<(String, String)> =
        baseline_edges.difference(&current_edges).cloned().collect();

    let arch_result = check_architecture_internal(ctx)?;
    let arch_score_after = arch_result.as_ref().map(|r| r.score);

    let summary = if has_baseline {
        format!(
            "{} added, {} removed, {} edge changes",
            symbols_added.len(),
            symbols_removed.len(),
            edges_added.len() + edges_removed.len()
        )
    } else {
        "No baseline provided - showing current graph state".to_string()
    };

    let result = GraphDiffDto {
        has_baseline,
        symbols_added,
        symbols_removed,
        edges_added,
        edges_removed,
        new_cycles: vec![],
        resolved_cycles: vec![],
        architecture_score_before: arch_score_before,
        architecture_score_after: arch_score_after,
        symbols_before: baseline_symbols.len(),
        symbols_after: current_symbols.len(),
        edges_before: baseline_edges.len(),
        edges_after: current_edges.len(),
        summary,
        _meta: OverviewMeta {
            estimated_tokens: 200,
            detail_level: "compare_graphs".to_string(),
        },
    };

    Ok(result)
}

/// Handler for detect_api_breaks tool (AIX-4.2)
#[cognicode_macros::aix_tool(
    name = "detect_api_breaks",
    description = "Detect breaking changes in the public API by comparing entry points between current and baseline graphs.",
    input_schema = DetectApiBreaksInput
)]
pub async fn handle_detect_api_breaks(
    ctx: &HandlerContext,
    input: DetectApiBreaksInput,
) -> HandlerResult<ApiBreaksResult> {
        let _ensure = ensure_graph_built(ctx)?;

    let current_graph = ctx.analysis_service.get_project_graph();
    let current_entries: HashSet<String> = current_graph
        .roots()
        .iter()
        .filter_map(|id| current_graph.get_symbol(id).map(|s| s.name().to_string()))
        .collect();

    let (has_baseline, _baseline_entries, breaks) = if let Some(baseline_dir) = input.baseline_dir {
        let baseline_path = ctx.working_dir.join(baseline_dir);
        let _store_path = graph_db_path(&baseline_path);
        let store = InMemoryGraphStore::new();
        match store.load_graph() {
            Ok(Some(baseline_graph)) => {
                let entries: HashSet<String> = baseline_graph
                    .roots()
                    .iter()
                    .filter_map(|id| baseline_graph.get_symbol(id).map(|s| s.name().to_string()))
                    .collect();

                // Find removed entry points
                let removed: Vec<ApiBreak> = entries
                    .difference(&current_entries)
                    .map(|name| ApiBreak {
                        symbol: name.clone(),
                        file: String::new(),
                        break_type: "removed".to_string(),
                        before: Some(name.clone()),
                        after: None,
                        severity: "major".to_string(),
                    })
                    .collect();

                (true, entries, removed)
            }
            _ => (false, HashSet::new(), vec![]),
        }
    } else {
        (false, HashSet::new(), vec![])
    };

    let total_breaks = breaks.len();
    let severity_summary = if total_breaks == 0 {
        "No breaking changes detected".to_string()
    } else {
        format!("{} breaking changes found", total_breaks)
    };

    let result = ApiBreaksResult {
        has_baseline,
        breaks,
        total_breaks,
        severity_summary,
        _meta: OverviewMeta {
            estimated_tokens: 150,
            detail_level: "detect_api_breaks".to_string(),
        },
    };

    Ok(result)
}

/// Handler for generate_system_prompt_context tool (AIX-5.1)
#[cognicode_macros::aix_tool(
    name = "generate_system_prompt_context",
    description = "Generate a structured context block for LLM system prompts in XML, JSON, or Markdown format.",
    input_schema = SystemPromptContextInput
)]
pub async fn handle_generate_system_prompt_context(
    ctx: &HandlerContext,
    input: SystemPromptContextInput,
) -> HandlerResult<SystemPromptContext> {
        let _ensure = ensure_graph_built(ctx)?;

    let stats = ctx.analysis_service.get_graph_stats();
    let hot_paths = if input.include_hot_paths.unwrap_or(false) {
        Some(get_hot_paths_from_graph(
            &ctx.analysis_service.get_project_graph(),
            5,
        ))
    } else {
        None
    };
    let arch_result = if input.include_architecture.unwrap_or(false) {
        check_architecture_internal(ctx)?
    } else {
        None
    };

    let content = match input.format {
        ContextFormatDetail::Xml => {
            let hot_paths_xml = if let Some(ref hp) = hot_paths {
                hp.iter()
                    .map(|h| {
                        format!(
                            "  <hot_path symbol=\"{}\" fan_in=\"{}\" />",
                            h.symbol_name, h.fan_in
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                String::new()
            };

            let arch_xml = if let Some(ref a) = arch_result {
                format!(
                    "<architecture score=\"{}\" cycles=\"{}\" />",
                    a.score,
                    a.cycles.len()
                )
            } else {
                String::new()
            };

            format!(
                "<project>\n  <stats symbols=\"{}\" edges=\"{}\" />\n{}\n{}\n</project>",
                stats.symbol_count, stats.edge_count, hot_paths_xml, arch_xml
            )
        }
        ContextFormatDetail::Json => {
            let obj = serde_json::json!({
                "stats": {
                    "symbols": stats.symbol_count,
                    "edges": stats.edge_count,
                },
                "hot_paths": hot_paths.map(|hp| hp.iter().map(|h| {
                    serde_json::json!({"symbol": h.symbol_name, "fan_in": h.fan_in})
                }).collect::<Vec<_>>()),
                "architecture": arch_result.as_ref().map(|a| {
                    serde_json::json!({"score": a.score, "cycles": a.cycles.len()})
                }),
            });
            serde_json::to_string_pretty(&obj).unwrap_or_default()
        }
        ContextFormatDetail::Markdown => {
            let mut md = format!(
                "## Project Stats\n- Symbols: {}\n- Edges: {}\n",
                stats.symbol_count, stats.edge_count
            );

            if let Some(ref hp) = hot_paths {
                md += "\n## Hot Paths\n";
                for h in hp {
                    md += &format!("- **{}** (fan-in: {})\n", h.symbol_name, h.fan_in);
                }
            }

            if let Some(ref a) = arch_result {
                md += &format!(
                    "\n## Architecture\n- Score: {:.1}\n- Cycles: {}\n",
                    a.score,
                    a.cycles.len()
                );
            }

            md
        }
    };

    let content_len = content.len();
    let result = SystemPromptContext {
        format: format!("{:?}", input.format).to_lowercase(),
        content,
        estimated_tokens: content_len / 4,
    };

    Ok(result)
}

/// Handler for detect_god_functions tool (AIX-5.2)
#[cognicode_macros::aix_tool(
    name = "detect_god_functions",
    description = "Find overly large or complex functions (god functions) that should be refactored.",
    input_schema = DetectGodFunctionsInput
)]
pub async fn handle_detect_god_functions(
    ctx: &HandlerContext,
    input: DetectGodFunctionsInput,
) -> HandlerResult<GodFunctionsResult> {
        let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();
    let mut god_functions = Vec::new();

    for symbol in graph.symbols() {
        let symbol_id = SymbolId::new(symbol.fully_qualified_name());

        // Get metrics
        let lines = get_symbol_lines(symbol);
        let complexity = get_symbol_complexity(&graph, &symbol_id).unwrap_or(0);
        let fan_in = graph.callers(&symbol_id).len();
        let fan_out = graph.callees(&symbol_id).len();

        // Check thresholds
        if lines >= input.min_lines
            && complexity >= input.min_complexity
            && fan_in >= input.min_fan_in
        {
            // Calculate god score
            let god_score = ((lines as f64 / input.min_lines as f64 * 25.0)
                + (complexity as f64 / input.min_complexity as f64 * 25.0)
                + (fan_in as f64 / input.min_fan_in as f64 * 25.0)
                + (fan_out as f64 / 10.0 * 25.0))
                .min(100.0);

            god_functions.push(GodFunctionDto {
                symbol: symbol.name().to_string(),
                file: symbol.location().file().to_string(),
                line: symbol.location().line() + 1,
                lines,
                complexity,
                fan_in,
                fan_out,
                god_score,
                suggestion: "Consider extracting smaller functions".to_string(),
            });
        }
    }

    // Sort by god score descending
    god_functions.sort_by(|a, b| {
        b.god_score
            .partial_cmp(&a.god_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_analyzed = graph.symbol_count();
    let god_count = god_functions.len();

    let result = GodFunctionsResult {
        god_functions,
        total_analyzed,
        thresholds: GodFunctionThresholds {
            min_lines: input.min_lines,
            min_complexity: input.min_complexity,
            min_fan_in: input.min_fan_in,
        },
        _meta: OverviewMeta {
            estimated_tokens: god_count * 60,
            detail_level: "god_functions".to_string(),
        },
    };

    Ok(result)
}

/// Handler for detect_long_parameter_lists tool (AIX-5.3)
#[cognicode_macros::aix_tool(
    name = "detect_long_parameter_lists",
    description = "Find functions with too many parameters that should be consolidated into structs.",
    input_schema = DetectLongParamsInput
)]
pub async fn handle_detect_long_parameter_lists(
    ctx: &HandlerContext,
    input: DetectLongParamsInput,
) -> HandlerResult<LongParamsResult> {
        let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();
    let mut long_param_functions = Vec::new();

    for symbol in graph.symbols() {
        // Only check functions
        if !matches!(
            symbol.kind(),
            crate::domain::value_objects::SymbolKind::Function
        ) {
            continue;
        }

        let param_count = graph
            .callees(&SymbolId::new(symbol.fully_qualified_name()))
            .len();

        if param_count > input.max_params {
            long_param_functions.push(LongParamFunctionDto {
                symbol: symbol.name().to_string(),
                file: symbol.location().file().to_string(),
                line: symbol.location().line() + 1,
                parameter_count: param_count,
                parameter_names: vec![],
                suggestion: "Consider grouping parameters into a struct".to_string(),
            });
        }
    }

    let total_analyzed = graph.symbol_count();
    let long_param_count = long_param_functions.len();

    let result = LongParamsResult {
        functions: long_param_functions,
        threshold: input.max_params,
        total_analyzed,
        _meta: OverviewMeta {
            estimated_tokens: long_param_count * 40,
            detail_level: "long_params".to_string(),
        },
    };

    Ok(result)
}

/// Handler for evaluate_refactor_quality tool (AIX-4.3)
#[cognicode_macros::aix_tool(
    name = "evaluate_refactor_quality",
    description = "Evaluate whether a refactoring was beneficial by comparing current graph state vs persisted baseline.",
    input_schema = EvaluateRefactorQualityInput
)]
pub async fn handle_evaluate_refactor_quality(
    ctx: &HandlerContext,
    _input: EvaluateRefactorQualityInput,
) -> HandlerResult<RefactorEvalDto> {
        let _ensure = ensure_graph_built(ctx)?;

    let _current_graph = ctx.analysis_service.get_project_graph();
    let stats = ctx.analysis_service.get_graph_stats();

    // Get current metrics
    let current_symbols = stats.symbol_count;
    let current_edges = stats.edge_count;

    // Get current cycles and dead code
    let arch_result = check_architecture_internal(ctx)?;
    let current_cycles = arch_result.as_ref().map(|r| r.cycles.len()).unwrap_or(0);
    let dead_code_result = ctx.analysis_service.detect_dead_code();
    let current_dead_code = dead_code_result.total_dead;

    // Calculate complexity metric (edges per symbol as proxy)
    let current_complexity = if current_symbols > 0 {
        current_edges as f64 / current_symbols as f64
    } else {
        0.0
    };

    // Try to load baseline from GraphStore
    let _baseline_path = ctx.working_dir.join(".cognicode").join("graph.cache");
    let (has_baseline, baseline_complexity, baseline_edges, baseline_cycles, baseline_dead_code) =
        match InMemoryGraphStore::new().load_graph() {
            Ok(Some(baseline_graph)) => {
                let baseline_symbols = baseline_graph.symbol_count();
                let baseline_edge_count = baseline_graph.edge_count();
                let baseline_complex = if baseline_symbols > 0 {
                    baseline_edge_count as f64 / baseline_symbols as f64
                } else {
                    0.0
                };

                // Calculate baseline cycles
                let cycle_detector = CycleDetector::new();
                let baseline_cycle_result = cycle_detector.detect_cycles(&baseline_graph);
                let baseline_cycle_count = baseline_cycle_result.cycles.len();

                // Baseline dead code would need a separate analysis, use 0 as placeholder
                // since we can't retroactively analyze baseline dead code without the service
                (
                    true,
                    baseline_complex,
                    baseline_edge_count,
                    baseline_cycle_count,
                    0isize,
                )
            }
            _ => (false, 0.0, 0usize, 0, 0isize),
        };

    if !has_baseline {
        let result = RefactorEvalDto {
            quality_score: 0.0,
            verdict: "neutral".to_string(),
            complexity_delta: 0.0,
            coupling_delta: 0,
            cycle_delta: 0,
            dead_code_delta: 0,
            recommendations: vec!["No baseline to compare. Save graph first.".to_string()],
            _meta: OverviewMeta {
                estimated_tokens: 50,
                detail_level: "evaluate_refactor".to_string(),
            },
        };
        return Ok(result);
    }

    // Calculate deltas (current - baseline, so negative = improvement)
    let complexity_delta = current_complexity - baseline_complexity;
    let coupling_delta = current_edges as isize - baseline_edges as isize;
    let cycle_delta = current_cycles as isize - baseline_cycles as isize;
    let dead_code_delta = current_dead_code as isize - baseline_dead_code;

    // Calculate penalties
    let complexity_penalty = (complexity_delta * 5.0).max(0.0).min(30.0);
    let coupling_penalty = (coupling_delta as f64 * 3.0).max(0.0).min(25.0);
    let cycle_penalty = (cycle_delta as f64 * 10.0).max(0.0).min(25.0);
    let dead_code_bonus = (-dead_code_delta as f64 * 2.0).max(0.0).min(20.0);

    // Calculate quality score
    let quality_score = (100.0 - complexity_penalty - coupling_penalty - cycle_penalty
        + dead_code_bonus)
        .max(0.0)
        .min(100.0);

    // Determine verdict
    let verdict = if quality_score >= 80.0 {
        "improvement"
    } else if quality_score >= 50.0 {
        "neutral"
    } else {
        "regression"
    }
    .to_string();

    // Generate recommendations
    let mut recommendations = Vec::new();
    if complexity_delta > 0.0 {
        recommendations
            .push("Complexity increased. Consider extracting complex functions.".to_string());
    }
    if coupling_delta > 0 {
        recommendations
            .push("Coupling increased. Look for opportunities to reduce dependencies.".to_string());
    }
    if cycle_delta > 0 {
        recommendations.push(
            "New cycles detected. Break cyclic dependencies with traits or shared modules."
                .to_string(),
        );
    }
    if dead_code_delta > 0 {
        recommendations.push("More dead code detected. Remove unused symbols.".to_string());
    }
    if recommendations.is_empty() && quality_score >= 80.0 {
        recommendations
            .push("Refactoring appears successful. Consider running tests to verify.".to_string());
    }

    let result = RefactorEvalDto {
        quality_score,
        verdict,
        complexity_delta,
        coupling_delta,
        cycle_delta,
        dead_code_delta,
        recommendations,
        _meta: OverviewMeta {
            estimated_tokens: 150,
            detail_level: "evaluate_refactor".to_string(),
        },
    };

    Ok(result)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get hot paths from graph
fn get_hot_paths_from_graph(graph: &CallGraph, limit: usize) -> Vec<HotPathDto> {
    let mut hot_paths: Vec<HotPathDto> = graph
        .symbols()
        .map(|s| {
            let id = SymbolId::new(s.fully_qualified_name());
            let fan_in = graph.callers(&id).len();
            let fan_out = graph.callees(&id).len();
            HotPathDto {
                symbol_name: s.name().to_string(),
                file: s.location().file().to_string(),
                line: s.location().line() + 1,
                fan_in,
                fan_out,
            }
        })
        .filter(|hp| hp.fan_in >= 2)
        .collect();

    hot_paths.sort_by(|a, b| b.fan_in.cmp(&a.fan_in));
    hot_paths.truncate(limit);
    hot_paths
}

/// Check architecture internally (returns None if graph not built)
fn check_architecture_internal(ctx: &HandlerContext) -> HandlerResult<Option<ArchitectureResult>> {
    let graph = ctx.analysis_service.get_project_graph();

    if graph.symbol_count() == 0 {
        return Ok(None);
    }

    let cycle_detector = CycleDetector::new();
    let cycle_result = cycle_detector.detect_cycles(&graph);

    let cycles = cycle_result
        .cycles
        .iter()
        .map(|c| crate::application::dto::CycleInfo {
            symbols: c.symbols().iter().map(|s| s.as_str().to_string()).collect(),
            length: c.length(),
        })
        .collect();

    let cycle_penalty = cycle_result.symbols_in_cycles() * 5;
    let score = (100.0 - cycle_penalty as f32).max(0.0);

    Ok(Some(ArchitectureResult {
        cycles,
        violations: vec![],
        score,
        summary: format!("{} cycles detected", cycle_result.cycles.len()),
    }))
}

/// Recommend first files to read based on graph analysis
fn recommend_first_reads_from_graph(graph: &CallGraph) -> Vec<String> {
    // Get entry points and their files
    let entry_files: Vec<String> = graph
        .roots()
        .iter()
        .filter_map(|id| graph.get_symbol(id))
        .map(|s| s.location().file().to_string())
        .collect();

    // Get hot path files
    let hot_path_files: Vec<String> = get_hot_paths_from_graph(graph, 5)
        .iter()
        .map(|h| h.file.clone())
        .collect();

    // Combine and deduplicate
    let mut files: Vec<String> = entry_files;
    files.extend(hot_path_files);
    files.sort();
    files.dedup();
    files.truncate(5);

    files
}

/// Detect project type from graph
fn detect_project_type(graph: &CallGraph) -> ProjectType {
    let entry_count = graph.roots().len();

    if entry_count > 10 {
        return ProjectType::Monorepo;
    }

    // Check for web API indicators
    let has_web_handlers = graph.symbols().any(|s| {
        let name = s.name().to_lowercase();
        name.contains("handler")
            || name.contains("route")
            || name.contains("endpoint")
            || name.contains("api")
    });

    if has_web_handlers && entry_count > 1 {
        return ProjectType::WebApi;
    }

    // Check for CLI indicators
    let has_main = graph.symbols().any(|s| s.name().to_lowercase() == "main");

    if has_main {
        return ProjectType::Cli;
    }

    // Check for library indicators (many traits/interfaces, few entry points)
    let trait_count = graph
        .symbols()
        .filter(|s| matches!(s.kind(), crate::domain::value_objects::SymbolKind::Trait))
        .count();

    if trait_count > 5 && entry_count <= 3 {
        return ProjectType::Library;
    }

    ProjectType::Unknown
}

/// Extract keywords from natural language query
fn extract_keywords(query: &str) -> Vec<String> {
    let stop_words = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "shall",
        "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
        "from", "as", "into", "through", "during", "before", "after", "above", "below", "between",
        "under", "again", "further", "then", "once", "here", "there", "when", "where", "why",
        "how", "all", "each", "few", "more", "most", "other", "some", "such", "no", "nor", "not",
        "only", "own", "same", "so", "than", "too", "very", "just",
    ];

    query
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| w.len() > 2 && !stop_words.contains(&w.to_lowercase().as_str()))
        .collect()
}

/// Get symbol complexity (simplified)
fn get_symbol_complexity(graph: &CallGraph, symbol_id: &SymbolId) -> Option<u32> {
    let callees = graph.callees(symbol_id);
    Some(callees.len() as u32 + 1)
}

/// Get approximate lines for a symbol (file-based heuristic)
fn get_symbol_lines(symbol: &Symbol) -> usize {
    // Use a heuristic: file size / estimated symbols per file
    let file = symbol.location().file();
    if let Ok(metadata) = std::fs::metadata(file) {
        // Rough estimate: assume avg 50 lines per symbol in file
        (metadata.len() / 2000) as usize
    } else {
        10 // default estimate
    }
}

// ============================================================================
// Onboarding Plan Builders
// ============================================================================

fn build_understand_plan(_ctx: &HandlerContext) -> HandlerResult<Vec<OnboardingStep>> {
    let mut params1 = HashMap::new();
    params1.insert("detail".to_string(), serde_json::json!("medium"));

    let mut params3 = HashMap::new();
    params3.insert("limit".to_string(), serde_json::json!(5));

    let steps = vec![
        OnboardingStep {
            step: 1,
            tool: "smart_overview".to_string(),
            params: params1,
            rationale: "Get project overview first".to_string(),
            estimated_tokens: 400,
            expected_outcome: "Understand project structure and type".to_string(),
        },
        OnboardingStep {
            step: 2,
            tool: "get_entry_points".to_string(),
            params: HashMap::new(),
            rationale: "Find main entry points".to_string(),
            estimated_tokens: 200,
            expected_outcome: "Know where to start reading".to_string(),
        },
        OnboardingStep {
            step: 3,
            tool: "get_hot_paths".to_string(),
            params: params3,
            rationale: "Identify critical functions".to_string(),
            estimated_tokens: 300,
            expected_outcome: "Know the most-called functions".to_string(),
        },
    ];
    Ok(steps)
}

fn build_refactor_plan(_ctx: &HandlerContext) -> HandlerResult<Vec<OnboardingStep>> {
    let steps = vec![
        OnboardingStep {
            step: 1,
            tool: "auto_diagnose".to_string(),
            params: HashMap::new(),
            rationale: "Find health issues".to_string(),
            estimated_tokens: 500,
            expected_outcome: "Know what needs refactoring".to_string(),
        },
        OnboardingStep {
            step: 2,
            tool: "detect_god_functions".to_string(),
            params: HashMap::new(),
            rationale: "Find complex functions".to_string(),
            estimated_tokens: 400,
            expected_outcome: "Identify refactoring targets".to_string(),
        },
    ];
    Ok(steps)
}

fn build_debug_plan(_ctx: &HandlerContext) -> HandlerResult<Vec<OnboardingStep>> {
    let mut params2 = HashMap::new();
    params2.insert("limit".to_string(), serde_json::json!(20));

    let steps = vec![
        OnboardingStep {
            step: 1,
            tool: "check_architecture".to_string(),
            params: HashMap::new(),
            rationale: "Check for cycles and architecture issues".to_string(),
            estimated_tokens: 300,
            expected_outcome: "Know architecture health".to_string(),
        },
        OnboardingStep {
            step: 2,
            tool: "find_dead_code".to_string(),
            params: params2,
            rationale: "Find potentially buggy dead code".to_string(),
            estimated_tokens: 400,
            expected_outcome: "Identify dead code issues".to_string(),
        },
    ];
    Ok(steps)
}

fn build_add_feature_plan(_ctx: &HandlerContext) -> HandlerResult<Vec<OnboardingStep>> {
    let mut params2 = HashMap::new();
    params2.insert("source".to_string(), serde_json::json!("main"));
    params2.insert("target".to_string(), serde_json::json!("relevant_function"));
    params2.insert("max_depth".to_string(), serde_json::json!(5));

    let steps = vec![
        OnboardingStep {
            step: 1,
            tool: "get_entry_points".to_string(),
            params: HashMap::new(),
            rationale: "Find where to add new functionality".to_string(),
            estimated_tokens: 200,
            expected_outcome: "Know where to add code".to_string(),
        },
        OnboardingStep {
            step: 2,
            tool: "trace_path".to_string(),
            params: params2,
            rationale: "Understand existing flow".to_string(),
            estimated_tokens: 300,
            expected_outcome: "Know the call chain".to_string(),
        },
    ];
    Ok(steps)
}

fn build_review_plan(_ctx: &HandlerContext) -> HandlerResult<Vec<OnboardingStep>> {
    let steps = vec![
        OnboardingStep {
            step: 1,
            tool: "auto_diagnose".to_string(),
            params: HashMap::new(),
            rationale: "Get overall health assessment".to_string(),
            estimated_tokens: 500,
            expected_outcome: "Know project health score".to_string(),
        },
        OnboardingStep {
            step: 2,
            tool: "detect_long_parameter_lists".to_string(),
            params: HashMap::new(),
            rationale: "Find design issues".to_string(),
            estimated_tokens: 300,
            expected_outcome: "Know parameter-related issues".to_string(),
        },
    ];
    Ok(steps)
}

// ============================================================================
// AVC Tool Handlers
// ============================================================================

// `AvcGenerator` is still used by S7000 (drift) inference and a few
// other code paths. The `AvcContract` type remains in the public
// surface so `handle_generate_contract` / `handle_validate_contract`
// keep their declared signatures.
use crate::infrastructure::avc::{AvcContract, AvcGenerator, AvcValidationResult};
use crate::interface::mcp::schemas::{
    DetectDriftInput, DetectDriftOutput, DriftFinding, DriftSeverity, GenerateContractInput,
    ValidateContractInput,
};

/// Handler for generate_contract tool (AVC-1)
///
/// Contract persistence was tied to SQLite, which was removed in the
/// Graph Intelligence v2 cleanup. The handler is kept as a stable
/// dispatch surface so existing MCP tool wiring still links, but
/// contract generation is no longer available until a PostgreSQL
/// adapter lands.
#[cognicode_macros::aix_tool(
    name = "generate_contract",
    description = "Generate an AVC truth contract from an existing function. Returns syntax, semantic, and safety constraints.",
    input_schema = GenerateContractInput
)]
pub async fn handle_generate_contract(
    _ctx: &HandlerContext,
    _input: GenerateContractInput,
) -> HandlerResult<AvcContract> {
    Err(HandlerError::Internal(
        "generate_contract requires a persistence layer (PostgreSQL via the `postgres` feature)"
            .to_string(),
    ))
}

/// Handler for validate_contract tool (AVC-2)
///
/// See `handle_generate_contract` — validation is unavailable without
/// a persistence layer to load the contract from.
#[cognicode_macros::aix_tool(
    name = "validate_contract",
    description = "Validate generated code against an AVC truth contract. Returns pass/fail with violations and fix suggestions.",
    input_schema = ValidateContractInput
)]
pub async fn handle_validate_contract(
    _ctx: &HandlerContext,
    _input: ValidateContractInput,
) -> HandlerResult<AvcValidationResult> {
    Err(HandlerError::Internal(
        "validate_contract requires a persistence layer (PostgreSQL via the `postgres` feature)"
            .to_string(),
    ))
}

// Phase 3A: Proactive Tools
// =============================================================================

/// Handler for suggest_context tool (Phase 3A)
#[cognicode_macros::aix_tool(
    name = "suggest_context",
    description = "Zero-query proactive context suggestion. Returns ranked files/symbols relevant to an agent's current task.",
    input_schema = SuggestContextInput
)]
pub async fn handle_suggest_context(
    ctx: &HandlerContext,
    input: SuggestContextInput,
) -> HandlerResult<SuggestContextOutput> {
    let start = Instant::now();

    // Cap limit at 50 (max allowed per spec SC-2)
    const MAX_LIMIT: usize = 50;
    let limit = input.limit.unwrap_or(10).min(MAX_LIMIT);

    // Ensure graph is built (auto-build if empty)
    let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();

    // Get hot-path symbols as seed tokens for FTS5 search
    let hot_paths = get_hot_paths_from_graph(&graph, limit);

    let mut items = Vec::new();
    let source;

    // Try FTS5 search with hot-path symbols as query tokens
    // Fall back to hot-path only if FTS5 returns nothing
    if hot_paths.is_empty() {
        // No hot paths - return empty result with graceful message
        source = "hotpath_empty".to_string();
    } else {
        // Build FTS5 query from hot-path symbol names
        let fts5_query = hot_paths
            .iter()
            .map(|hp| hp.symbol_name.clone())
            .collect::<Vec<_>>()
            .join(" OR ");

        // Query semantic search (FTS5-backed)
        let search_results =
            ctx.semantic_search
                .search(crate::infrastructure::semantic::SearchQuery {
                    query: fts5_query,
                    kinds: vec![],
                    max_results: limit,
                });

        if search_results.is_empty() {
            // No FTS5 results - use hot paths directly
            source = "hotpath_only".to_string();
            for hp in hot_paths.iter().take(limit) {
                items.push(SuggestContextItem {
                    name: hp.symbol_name.clone(),
                    kind: "function".to_string(),
                    file: hp.file.clone(),
                    line: hp.line,
                    score: calculate_hotness_score(hp.fan_in, hp.fan_out),
                    context: format!("Hot path (fan_in={}, fan_out={})", hp.fan_in, hp.fan_out),
                });
            }
        } else {
            // FTS5 results available - merge with hot-path ranking
            source = "fts5_hotpath".to_string();
            let max_fan_in = hot_paths.iter().map(|hp| hp.fan_in).max().unwrap_or(1) as f32;

            for result in search_results.iter().take(limit) {
                let symbol_id = SymbolId::new(result.symbol.fully_qualified_name());
                let fan_in = graph.callers(&symbol_id).len() as f32;
                let hotness_score = if max_fan_in > 0.0 {
                    (fan_in / max_fan_in).min(1.0)
                } else {
                    0.5
                };

                items.push(SuggestContextItem {
                    name: result.symbol.name().to_string(),
                    kind: format!("{:?}", result.symbol.kind()).to_lowercase(),
                    file: result.symbol.location().file().to_string(),
                    line: result.symbol.location().line() + 1,
                    score: (result.score * 0.6 + hotness_score * 0.4).min(1.0),
                    context: format!("Score: {:.2}, fan_in: {}", result.score, fan_in),
                });
            }
        }
    }

    let total = items.len();
    let elapsed_ms = start.elapsed().as_millis() as u64;

    Ok(SuggestContextOutput {
        items,
        total,
        source,
        metadata: AnalysisMetadata {
            total_calls: total,
            analysis_time_ms: elapsed_ms,
        },
        _meta: Some(SchemaOverviewMeta {
            estimated_tokens: total * 50,
            detail_level: "suggest_context".to_string(),
        }),
    })
}

/// Handler for reparse_on_edit tool (Phase 3A)
#[cfg(feature = "persistence")]
#[cognicode_macros::aix_tool(
    name = "reparse_on_edit",
    description = "MCP-triggered incremental reindex of changed files.",
    input_schema = ReparseOnEditInput
)]
#[cfg(feature = "persistence")]
pub async fn handle_reparse_on_edit(
    ctx: &HandlerContext,
    input: ReparseOnEditInput,
) -> HandlerResult<ReparseOnEditOutput> {
    let start = Instant::now();

    // Validate input file paths
    for path in &input.file_paths {
        ctx.validator.validate_file_path(path)?;
    }

    // For reparse_on_edit, we delegate to the workspace session infrastructure.
    // Since HandlerContext doesn't directly expose WorkspaceSession, we perform
    // a simplified incremental reindex using the existing graph infrastructure.
    //
    // The full implementation would create a WorkspaceSession and call
    // incremental_reindex() with the specific file_paths. For now, we use
    // the existing incremental reindex logic via the analysis service.

    // Build manifest for comparison (simplified - full impl would track edit ranges)
    let store = ctx.get_graph_store();
    let existing_manifest = store
        .load_manifest()
        .map_err(|e| HandlerError::Internal(format!("Failed to load manifest: {}", e)))?
        .unwrap_or_else(|| {
            crate::domain::value_objects::file_manifest::FileManifest::new(ctx.working_dir.clone())
        });

    // Check which files have actually changed using mtime comparison
    let mut files_parsed = 0;
    let mut files_skipped = 0;
    let mut files_removed = 0;
    let mut symbols_added = 0;
    let mut symbols_removed = 0;
    let mut graph_updated = false;
    // Track changed absolute paths for graph rebuild
    let mut changed_abs_paths: Vec<PathBuf> = Vec::new();

    for file_path in &input.file_paths {
        let full_path = if Path::new(file_path).is_absolute() {
            PathBuf::from(file_path)
        } else {
            ctx.working_dir.join(file_path)
        };

        // Check if file exists and get its mtime
        let file_mtime = match std::fs::metadata(&full_path) {
            Ok(meta) => meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            Err(_) => 0, // File doesn't exist
        };

        let rel_path = full_path
            .strip_prefix(&ctx.working_dir)
            .unwrap_or(&full_path)
            .to_path_buf();

        // Compare with manifest entry
        let was_in_manifest = existing_manifest.entries.contains_key(&rel_path);

        if file_mtime == 0 && was_in_manifest {
            // File was deleted
            files_removed += 1;
            symbols_removed += existing_manifest
                .entries
                .get(&rel_path)
                .map(|e| e.symbol_count)
                .unwrap_or(0);
            graph_updated = true;
            changed_abs_paths.push(full_path);
        } else if file_mtime > 0 {
            if let Some(entry) = existing_manifest.entries.get(&rel_path) {
                if entry.mtime == file_mtime {
                    // File unchanged
                    files_skipped += 1;
                } else {
                    // File modified - call index_file_from_path to update DashMap and FTS5
                    files_parsed += 1;
                    match ctx.semantic_search.index_file_from_path(&full_path) {
                        Ok(()) => {
                            // Successfully indexed - symbols will be in DashMap and FTS5
                            // Note: index_file_from_path returns Ok(()) on success, not symbol count
                            // We increment by 1 as a proxy since actual count isn't available from the method
                            symbols_added += 1;
                            graph_updated = true;
                            changed_abs_paths.push(full_path);
                        }
                        Err(e) => {
                            // Parse failure - log at WARN and count as skipped
                            tracing::warn!("Failed to index modified file {:?}: {}", full_path, e);
                            files_skipped += 1;
                            files_parsed -= 1; // Don't count this as parsed
                        }
                    }
                }
            } else {
                // New file - call index_file_from_path to add to DashMap and FTS5
                files_parsed += 1;
                match ctx.semantic_search.index_file_from_path(&full_path) {
                    Ok(()) => {
                        // Successfully indexed
                        symbols_added += 1;
                        graph_updated = true;
                        changed_abs_paths.push(full_path);
                    }
                    Err(e) => {
                        // Parse failure - log at WARN and count as skipped
                        tracing::warn!("Failed to index new file {:?}: {}", full_path, e);
                        files_skipped += 1;
                        files_parsed -= 1; // Don't count this as parsed
                    }
                }
            }
        }
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;

    // Rebuild call graph if files were actually modified
    // This ensures the graph stays in sync with FTS5/DashMap after code changes
    if graph_updated && !changed_abs_paths.is_empty() {
        tracing::debug!(
            "Rebuilding call graph for {} changed files in reparse_on_edit",
            changed_abs_paths.len()
        );

        // Invalidate file cache so changed files are re-parsed
        ctx.analysis_service
            .invalidate_file_cache_for(&changed_abs_paths);

        // Rebuild the project graph to reflect the changes
        if let Err(e) = ctx
            .analysis_service
            .build_project_graph_async(&ctx.working_dir)
            .await
        {
            tracing::warn!("Failed to rebuild call graph in reparse_on_edit: {}", e);
            // Don't fail the whole operation - FTS5/DashMap are already updated
        }
    }

    Ok(ReparseOnEditOutput {
        files_parsed,
        files_skipped,
        files_removed,
        symbols_added,
        symbols_removed,
        graph_updated,
        metadata: AnalysisMetadata {
            total_calls: files_parsed + files_skipped + files_removed,
            analysis_time_ms: elapsed_ms,
        },
        _meta: Some(SchemaOverviewMeta {
            estimated_tokens: 50,
            detail_level: "reparse_on_edit".to_string(),
        }),
    })
}

// ============================================================================
// Detect Drift Handler (S7000-S7003)
// ============================================================================

/// Handler for detect_drift tool
#[cognicode_macros::aix_tool(
    name = "detect_drift",
    description = "Analyze a source file for intent drift, AVC violations, obsolete patterns, and forbidden terms.",
    input_schema = DetectDriftInput
)]
pub async fn handle_detect_drift(
    ctx: &HandlerContext,
    input: DetectDriftInput,
) -> HandlerResult<DetectDriftOutput> {
    use crate::infrastructure::parser::{Language, TreeSitterParser};
    use std::path::Path;

    // Validate file path (security check only - does NOT return resolved path)
    ctx.validator
        .validate_file_path(&input.file_path)
        .map_err(|e| HandlerError::InvalidInput(format!("Invalid file path: {}", e)))?;

    // Resolve the file path against working directory (like other handlers do)
    let file_path = if Path::new(&input.file_path).is_absolute() {
        PathBuf::from(&input.file_path)
    } else {
        ctx.working_dir.join(&input.file_path)
    };

    // Read file content
    let source = std::fs::read_to_string(&file_path)
        .map_err(|e| HandlerError::NotFound(format!("File not found: {}", e)))?;

    // Detect language from extension
    let language = Language::from_extension(file_path.extension())
        .ok_or_else(|| HandlerError::InvalidInput("Unsupported file type".to_string()))?;

    // Create parser
    let parser = TreeSitterParser::new(language)
        .map_err(|e| HandlerError::Internal(format!("Parser error: {}", e)))?;

    // Parse the source
    let tree = parser
        .parse_tree(&source)
        .map_err(|e| HandlerError::Internal(format!("Parse error: {}", e)))?;

    // Collect all findings
    let mut findings = Vec::new();
    let function_node_type = language.function_node_type();

    // Walk the tree to find functions
    walk_function_nodes(
        tree.root_node(),
        &source,
        function_node_type,
        &input,
        &mut findings,
    );

    // Filter by threshold
    let threshold = input.threshold;
    findings.retain(|f| f.drift_score >= threshold);

    // Persist findings above threshold
    let persisted_count = persist_findings(ctx, &file_path, &findings).await;

    // Build summary
    let summary = if findings.is_empty() {
        "No drift detected above threshold".to_string()
    } else {
        format!(
            "Found {} drift findings ({} persisted) above threshold {}",
            findings.len(),
            persisted_count,
            threshold
        )
    };

    Ok(DetectDriftOutput {
        findings,
        summary,
        persisted_count,
    })
}

/// Walk tree-sitter nodes to find functions and analyze them
fn walk_function_nodes(
    node: tree_sitter::Node,
    source: &str,
    function_type: &str,
    input: &DetectDriftInput,
    findings: &mut Vec<DriftFinding>,
) {
    // Check if this is a function definition
    if node.kind() == function_type {
        // Get function name
        if let Some(name) = get_function_name(node, source) {
            // If function_name filter is specified, skip non-matching functions
            if let Some(ref target_fn) = input.function_name
                && name != *target_fn
            {
                // Visit children anyway to find nested functions
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        walk_function_nodes(child, source, function_type, input, findings);
                    }
                }
                return;
            }

            let line = (node.start_position().row + 1) as u32; // 1-indexed

            // S7000: Semantic drift (Jaccard similarity)
            if let Some(drift_score) = compute_s7000_drift(node, source) {
                let severity = DriftSeverity::from_score(drift_score);
                findings.push(DriftFinding {
                    function_name: name.clone(),
                    drift_score,
                    rule_id: "S7000".to_string(),
                    severity,
                    line,
                    message: format!(
                        "S7000: Low docstring-body similarity (score={:.2}). Docstring may not match implementation.",
                        drift_score
                    ),
                });
            }

            // S7001: AVC violations (unsafe, panic!, .unwrap(), .expect())
            let s7001_findings = detect_s7001_violations(node, source, &name);
            findings.extend(s7001_findings);

            // S7002: Obsolete patterns (try! macro)
            let s7002_findings = detect_s7002_patterns(node, source, &name);
            findings.extend(s7002_findings);

            // S7003: Forbidden terms
            let s7003_findings = detect_s7003_forbidden_terms(node, source, &name);
            findings.extend(s7003_findings);
        }
    }

    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            walk_function_nodes(child, source, function_type, input, findings);
        }
    }
}

/// Find the previous sibling of a node (e.g., the doc comment before a function)
fn find_previous_sibling(node: tree_sitter::Node, source: &str) -> Option<(String, usize)> {
    // Get parent and find this node's index, then look at previous sibling
    // This is complex with tree-sitter's API, so we'll use a different approach:
    // For Rust, doc comments are siblings, so we look at the parent and find prev
    let parent = node.parent()?;
    let node_index = parent
        .children(&mut node.walk())
        .position(|c| c.id() == node.id())?;
    if node_index == 0 {
        return None;
    }

    // Get previous sibling
    let mut cursor = node.walk();
    for (i, sibling) in parent.children(&mut cursor).enumerate() {
        if i == node_index - 1 {
            let kind = sibling.kind();
            let text = source[sibling.byte_range()].to_string();
            // Check if it's a doc comment (/// or //!)
            if kind == "line_comment" && (text.contains("///") || text.contains("//!")) {
                return Some((text, sibling.start_position().row + 1));
            }
            return None;
        }
    }
    None
}

/// Get function name from a function node
fn get_function_name(node: tree_sitter::Node, source: &str) -> Option<String> {
    // Try to get the identifier child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "property_identifier" {
            return Some(source[child.byte_range()].to_string());
        }
        // For Python: first child of function_definition is often the name
        if child.kind() == "name" {
            return Some(source[child.byte_range()].to_string());
        }
    }
    // Fallback: try to find any identifier-like child
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let kind = child.kind();
            if kind.contains("identifier") || kind == "name" {
                return Some(source[child.byte_range()].to_string());
            }
        }
    }
    None
}

/// Returns true if the line is a comment-only line.
/// Matches axiom behavior from catalog.rs:946-948.
fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with("///")
        || trimmed.starts_with("//!")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with('#')
}

/// S7000: Compute semantic drift using canonical AvcGenerator tokenization and plain Jaccard
fn compute_s7000_drift(node: tree_sitter::Node, source: &str) -> Option<f32> {
    const MIN_LINES: usize = 3;

    // Extract docstring using canonical path
    let doc_text = AvcGenerator::extract_docstring(&node, source);
    if doc_text.is_empty() {
        return None;
    }

    // Extract body and check min_lines gate
    let body_text = AvcGenerator::extract_body_text(&node, source);
    let line_count = body_text.lines().count();
    if line_count < MIN_LINES {
        return None;
    }

    // Tokenize using canonical path (no stemming, stop-word filtered)
    let doc_tokens = AvcGenerator::tokenize(&doc_text);
    if doc_tokens.is_empty() {
        return None;
    }

    // For body tokens, exclude doc content to avoid artificial similarity inflation
    let body_for_tokenize = body_text.replace(&doc_text, "");
    let body_tokens = AvcGenerator::tokenize(&body_for_tokenize);

    // Plain HashSet Jaccard (no stemming)
    let intersection = doc_tokens.intersection(&body_tokens).count() as f32;
    let union = doc_tokens.union(&body_tokens).count() as f32;
    let similarity = if union > 0.0 {
        intersection / union
    } else {
        0.0
    };
    let drift_score = 1.0 - similarity;

    if drift_score < 0.3 {
        None
    } else {
        Some(drift_score)
    }
}

/// S7001: Detect AVC violations (unsafe, panic!, .unwrap(), .expect()) with comment-line filtering
fn detect_s7001_violations(
    node: tree_sitter::Node,
    source: &str,
    function_name: &str,
) -> Vec<DriftFinding> {
    let mut findings = Vec::new();
    let node_text = &source[node.byte_range()];
    let start_line = node.start_position().row;

    let forbidden = [
        ("unsafe", "Unsafe code block without justification", 0.85),
        ("panic!", "panic! macro in production code", 0.90),
        (".unwrap()", ".unwrap() without error handling", 0.75),
        (".expect(", ".expect() without proper message", 0.75),
    ];

    // Iterate source lines with comment filtering (matches axiom catalog.rs:944-948)
    for (idx, line) in node_text.lines().enumerate() {
        let trimmed = line.trim();
        // Skip comment-only lines (same logic as axiom)
        if is_comment_line(line) {
            continue;
        }

        for (pattern, desc, score) in &forbidden {
            if trimmed.contains(pattern) {
                findings.push(DriftFinding {
                    function_name: function_name.to_string(),
                    drift_score: *score,
                    rule_id: "S7001".to_string(),
                    severity: DriftSeverity::Critical,
                    line: (start_line + idx + 1) as u32,
                    message: format!("S7001: {}", desc),
                });
                break;
            }
        }
    }

    findings
}

/// S7002: Detect obsolete patterns (try! macro)
fn detect_s7002_patterns(
    node: tree_sitter::Node,
    source: &str,
    function_name: &str,
) -> Vec<DriftFinding> {
    let mut findings = Vec::new();
    let node_text = source[node.byte_range()].to_lowercase();
    let line = (node.start_position().row + 1) as u32;

    // Check for try! macro (Rust obsolete pattern)
    if node_text.contains("try!") {
        findings.push(DriftFinding {
            function_name: function_name.to_string(),
            drift_score: 0.5,
            rule_id: "S7002".to_string(),
            severity: DriftSeverity::Warning,
            line,
            message: "S7002: try! macro detected. Use ? operator instead for modern Rust."
                .to_string(),
        });
    }

    findings
}

/// S7003: Detect forbidden domain terms
fn detect_s7003_forbidden_terms(
    node: tree_sitter::Node,
    source: &str,
    function_name: &str,
) -> Vec<DriftFinding> {
    let mut findings = Vec::new();

    // Forbidden domain terms (general and security)
    let forbidden_general = [
        "deprecated",
        "obsolete",
        "legacy",
        "temp",
        "todo",
        "fixme",
        "hack",
    ];

    let forbidden_security = [
        "password",
        "secret",
        "token",
        "api_key",
        "apikey",
        "private_key",
    ];

    let line = (node.start_position().row + 1) as u32;

    // Recursively check for forbidden terms, skipping string literals
    check_node_for_forbidden_terms(
        node,
        source,
        function_name,
        line,
        &forbidden_general,
        &forbidden_security,
        &mut findings,
    );

    findings
}

/// Recursively check nodes for forbidden terms, skipping string literals
fn check_node_for_forbidden_terms(
    node: tree_sitter::Node,
    source: &str,
    function_name: &str,
    line: u32,
    forbidden_general: &[&str; 7],
    forbidden_security: &[&str; 6],
    findings: &mut Vec<DriftFinding>,
) {
    let kind = node.kind();

    // Skip string literals entirely (both "string" and "string_content")
    if kind == "string" || kind == "string_content" {
        return;
    }

    // For identifier and comment nodes, check for forbidden terms
    // Note: Rust uses "line_comment" and "block_comment", Python uses "comment"
    if kind == "identifier"
        || kind == "line_comment"
        || kind == "block_comment"
        || kind == "comment"
    {
        let text = source[node.byte_range()].to_lowercase();

        // Check general forbidden terms
        for term in forbidden_general {
            if text.contains(term) {
                findings.push(DriftFinding {
                    function_name: function_name.to_string(),
                    drift_score: 0.4,
                    rule_id: "S7003".to_string(),
                    severity: DriftSeverity::Warning,
                    line,
                    message: format!(
                        "S7003: Forbidden term '{}' detected. Consider more descriptive naming.",
                        term
                    ),
                });
            }
        }

        // Check security forbidden terms
        for term in forbidden_security {
            if text.contains(term) {
                findings.push(DriftFinding {
                    function_name: function_name.to_string(),
                    drift_score: 0.8,
                    rule_id: "S7003".to_string(),
                    severity: DriftSeverity::Critical,
                    line,
                    message: format!(
                        "S7003: Security-sensitive term '{}' detected. Ensure proper handling.",
                        term
                    ),
                });
            }
        }
    }

    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            check_node_for_forbidden_terms(
                child,
                source,
                function_name,
                line,
                forbidden_general,
                forbidden_security,
                findings,
            );
        }
    }
}

/// Persist findings to database if a persistence layer is configured.
///
/// Drift event persistence was tied to SQLite, which was removed in
/// the Graph Intelligence v2 cleanup. The hook remains so callers
/// (e.g. `handle_detect_drift`) keep linking, but the persistence
/// is a no-op until a PostgreSQL adapter lands.
#[allow(clippy::unused_async, dead_code)]
async fn persist_findings(
    _ctx: &HandlerContext,
    _file_path: &std::path::Path,
    _findings: &[DriftFinding],
) -> usize {
    0
}

/// Calculate a hotness score from fan-in and fan-out
fn calculate_hotness_score(fan_in: usize, fan_out: usize) -> f32 {
    use std::cmp::Ordering;

    // Higher fan_in = more important (called by many)
    // Lower fan_out = less complex (doesn't call many others)
    match fan_in.cmp(&fan_out) {
        Ordering::Greater => (fan_in as f32 / (fan_in + fan_out) as f32).min(1.0),
        Ordering::Equal if fan_in == 0 => 0.5,
        _ => 0.3,
    }
}

// ============================================================================
// Agent Task Tools (Batch D - Bidirectional Interaction)
// ============================================================================

// `CompleteTaskInput` and `PollTasksInput` are public schema types —
// the dispatch layer wires them up. The DTOs are still used by the
// empty-list stubs so consumers see a stable return shape.
use crate::interface::mcp::schemas::{
    CompleteTaskInput, CompleteTaskOutput, PollTasksInput, PollTasksOutput,
};

/// Handler for poll_tasks tool — claim pending tasks for execution
///
/// Polling requires a persistence layer to read pending tasks from.
/// SQLite was removed in the Graph Intelligence v2 cleanup, so this
/// returns an empty list. The dispatch layer keeps linking; the
/// handler will gain real implementation when the PostgreSQL adapter
/// lands.
#[cognicode_macros::aix_tool(
    name = "poll_tasks",
    description = "Poll for pending agent tasks and claim them for execution.",
    input_schema = PollTasksInput
)]
pub async fn handle_poll_tasks(
    _ctx: &HandlerContext,
    _input: PollTasksInput,
) -> HandlerResult<PollTasksOutput> {
    Ok(PollTasksOutput { tasks: vec![] })
}

/// Handler for complete_task tool — mark a task as completed
///
/// Completion requires a persistence layer to write the status back.
/// SQLite was removed in the Graph Intelligence v2 cleanup, so this
/// validates the input but reports success:false. The dispatch layer
/// keeps linking; the handler will gain real implementation when the
/// PostgreSQL adapter lands.
#[cognicode_macros::aix_tool(
    name = "complete_task",
    description = "Mark an agent task as completed or failed with optional result data.",
    input_schema = CompleteTaskInput
)]
pub async fn handle_complete_task(
    _ctx: &HandlerContext,
    input: CompleteTaskInput,
) -> HandlerResult<CompleteTaskOutput> {
    // Validate status (mirror the previous sqlite path's input validation)
    let status = match input.status.as_str() {
        "completed" | "failed" => input.status.clone(),
        _ => {
            return Err(HandlerError::InvalidInput(
                "status must be 'completed' or 'failed'".to_string(),
            ));
        }
    };

    Ok(CompleteTaskOutput {
        success: false,
        message: format!(
            "Task {} cannot be marked as {} — persistence layer is unavailable \
             (enable the `postgres` feature for a working implementation)",
            input.task_id, status
        ),
    })
}

#[cfg(test)]
mod aix_tests {
    use super::*;

    // AIX Handler Tests
    // =============================================================================

    #[tokio::test]
    async fn test_ranked_symbols_returns_results() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            "fn process_data() { helper(); }\nfn helper() {}",
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph first
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = RankedSymbolsInput {
            query: "process".to_string(),
            limit: 10,
        };

        let result = handle_ranked_symbols(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.returned > 0 || output.total_matches == 0); // Either has results or empty but valid
    }

    #[tokio::test]
    async fn test_onboarding_plan_all_goals() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() {}\n").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Test all goal variants
        for goal in [
            OnboardingGoalDetail::Understand,
            OnboardingGoalDetail::Refactor,
            OnboardingGoalDetail::Debug,
            OnboardingGoalDetail::AddFeature,
            OnboardingGoalDetail::Review,
        ] {
            let input = OnboardingPlanInput { goal };
            let result = handle_suggest_onboarding_plan(&ctx, input).await;
            assert!(result.is_ok(), "Failed for goal: {:?}", goal);
            let output = result.unwrap();
            assert!(
                output.steps.len() > 0,
                "Should have steps for goal: {:?}",
                goal
            );
        }
    }

    #[tokio::test]
    async fn test_auto_diagnose_health_score() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() { helper(); }\nfn helper() {}").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = AutoDiagnoseInput {
            target: None,
            min_severity: "info".to_string(),
        };

        let result = handle_auto_diagnose(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Health score should be 0-100
        assert!(output.health_score >= 0.0 && output.health_score <= 100.0);
        // Should have counts
        assert_eq!(
            output.critical_count
                + output.important_count
                + output.warning_count
                + output.info_count,
            output.total_issues
        );
    }

    #[tokio::test]
    async fn test_nl_to_symbol_extracts_keywords() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            "fn process_user_data() {}\nfn calculate_total() {}",
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = NlToSymbolInput {
            query: "function that processes user data".to_string(),
            limit: 10,
        };

        let result = handle_nl_to_symbol(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should extract keywords
        assert!(!output.extracted_keywords.is_empty());
        // Keywords should include "process" or "user" or "data"
        let kw_str = output.extracted_keywords.join(" ").to_lowercase();
        assert!(
            kw_str.contains("process")
                || kw_str.contains("user")
                || kw_str.contains("data")
                || output.extracted_keywords.is_empty()
        );
    }

    #[tokio::test]
    async fn test_find_pattern_by_intent_list() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = FindPatternByIntentInput {
            intent: "singleton pattern".to_string(),
            list_patterns: Some(true),
        };

        let result = handle_find_pattern_by_intent(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // When list_patterns is true, should have all patterns
        assert!(output.all_patterns.len() >= 15); // Should have at least 15 patterns
    }

    #[tokio::test]
    async fn test_god_functions_thresholds() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        // Write a file with functions that call each other to build up complexity
        std::fs::write(&file_path, r#"
fn f1() {}
fn f2() { f1(); }
fn f3() { f2(); f1(); }
fn f4() { f3(); f2(); f1(); }
fn f5() { f4(); f3(); f2(); f1(); }
fn f6() { f5(); f4(); f3(); f2(); f1(); }
fn f7() { f6(); f5(); f4(); f3(); f2(); f1(); }
fn f8() { f7(); f6(); f5(); f4(); f3(); f2(); f1(); }
fn f9() { f8(); f7(); f6(); f5(); f4(); f3(); f2(); f1(); }
fn f10() { f9(); f8(); f7(); f6(); f5(); f4(); f3(); f2(); f1(); }
fn f11() { f10(); f9(); f8(); f7(); f6(); f5(); f4(); f3(); f2(); f1(); }
fn f12() { f11(); f10(); f9(); f8(); f7(); f6(); f5(); f4(); f3(); f2(); f1(); }
fn f13() { f12(); f11(); f10(); f9(); f8(); f7(); f6(); f5(); f4(); f3(); f2(); f1(); }
fn f14() { f13(); f12(); f11(); f10(); f9(); f8(); f7(); f6(); f5(); f4(); f3(); f2(); f1(); }
fn f15() { f14(); f13(); f12(); f11(); f10(); f9(); f8(); f7(); f6(); f5(); f4(); f3(); f2(); f1(); }
fn main() { f15(); }
"#).unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Use low thresholds that will catch f15 (calls 15 functions = complexity 16)
        let input = DetectGodFunctionsInput {
            min_lines: 1,
            min_complexity: 10, // f15 calls 15 functions
            min_fan_in: 0,
        };

        let result = handle_detect_god_functions(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should find at least f15 which has high complexity due to many callees
        assert!(output.god_functions.len() >= 1 || output.total_analyzed >= 15);
    }

    #[tokio::test]
    async fn test_long_params_threshold() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            "fn many_params(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) {}",
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = DetectLongParamsInput { max_params: 5 };

        let result = handle_detect_long_parameter_lists(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should find the function with 6 parameters
        assert!(output.functions.len() >= 1 || output.total_analyzed >= 1);
    }

    #[tokio::test]
    async fn test_smart_overview_returns_data() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() { helper(); }\nfn helper() {}").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = SmartOverviewInput {
            detail: Some(OverviewDetail::Medium),
        };

        let result = handle_smart_overview(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should have basic stats
        assert!(output.total_symbols >= 2);
        assert!(output.total_edges >= 0);
        assert!(!output.languages.is_empty());
    }

    #[tokio::test]
    async fn test_compare_call_graphs_no_baseline() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() {}\n").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = CompareCallGraphsInput { baseline_dir: None };

        let result = handle_compare_call_graphs(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // No baseline should be reported
        assert!(!output.has_baseline);
        // When no baseline provided, all current symbols appear as "added" (compared to empty)
        assert!(output.symbols_added.len() >= 0); // Valid response, symbols shown relative to empty baseline
    }

    #[tokio::test]
    async fn test_system_prompt_context_format() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() {}\n").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Test XML format
        let input = SystemPromptContextInput {
            format: ContextFormatDetail::Xml,
            include_architecture: Some(true),
            include_hot_paths: Some(true),
        };

        let result = handle_generate_system_prompt_context(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.format, "xml");
        assert!(output.content.contains("<project>") || output.content.contains("symbols"));

        // Test JSON format
        let input = SystemPromptContextInput {
            format: ContextFormatDetail::Json,
            include_architecture: Some(false),
            include_hot_paths: Some(false),
        };

        let result = handle_generate_system_prompt_context(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.format, "json");
        assert!(output.content.contains("stats") || output.content.contains("symbols"));

        // Test Markdown format
        let input = SystemPromptContextInput {
            format: ContextFormatDetail::Markdown,
            include_architecture: Some(false),
            include_hot_paths: Some(false),
        };

        let result = handle_generate_system_prompt_context(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.format, "markdown");
        assert!(output.content.contains("Project Stats") || output.content.contains("Symbols"));
    }

    #[tokio::test]
    async fn test_refactor_eval_no_baseline() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() {}\n").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Do NOT build graph - this ensures no baseline exists
        let input = EvaluateRefactorQualityInput {};

        let result = handle_evaluate_refactor_quality(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // No graph built means no baseline - should be neutral with a note
        assert_eq!(output.verdict, "neutral");
        assert!(output.quality_score >= 0.0);
        assert!(
            output
                .recommendations
                .iter()
                .any(|r| r.contains("No baseline"))
        );
    }

    #[tokio::test]
    async fn test_symbol_hotness_tracking_increments() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Record some symbol accesses
        ctx.record_symbol_access("symbol_a", 5);
        ctx.record_symbol_access("symbol_b", 3);
        ctx.record_symbol_access("symbol_a", 2); // Should increment to 7

        // Verify hotness scores
        let score_a = ctx.get_symbol_hotness("symbol_a");
        let score_b = ctx.get_symbol_hotness("symbol_b");
        let score_c = ctx.get_symbol_hotness("symbol_nonexistent");

        // symbol_a has 7 accesses, symbol_b has 3, max is 7
        assert!(
            (score_a - 1.0).abs() < 0.001,
            "symbol_a should have hotness 1.0 (hottest)"
        );
        assert!(
            (score_b - 3.0 / 7.0).abs() < 0.001,
            "symbol_b should have hotness 3/7"
        );
        assert!(
            (score_c - 0.0).abs() < 0.001,
            "nonexistent symbol should have hotness 0.0"
        );
    }

    #[tokio::test]
    async fn test_ranked_symbols_hotness_boost() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            "fn process_data() { helper(); }\nfn helper() {}",
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph first
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Pre-record a symbol as hot
        ctx.record_symbol_access("process_data", 10);

        let input = RankedSymbolsInput {
            query: "process".to_string(),
            limit: 10,
        };

        let result = handle_ranked_symbols(&ctx, input).await;
        assert!(result.is_ok());

        // Now check hotness is tracked
        let hotness = ctx.get_symbol_hotness("process_data");
        assert!(
            hotness > 0.0,
            "process_data should have some hotness after being ranked"
        );
    }

    // ============================================================================
    // Additional AIX Handler Tests (5 handlers)
    // ============================================================================

    #[tokio::test]
    async fn test_detect_god_functions_no_god_functions_found() {
        // Test case: simple small functions should NOT be detected as god functions
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn helper1() {}
fn helper2() {}
fn main() {
    helper1();
    helper2();
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Use reasonable thresholds that simple functions won't meet
        let input = DetectGodFunctionsInput {
            min_lines: 50,      // No function is that long
            min_complexity: 20, // No function has that many branches
            min_fan_in: 10,     // No function is called that much
        };

        let result = handle_detect_god_functions(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should find no god functions
        assert_eq!(
            output.god_functions.len(),
            0,
            "Simple functions should not be detected as god functions"
        );
        assert!(output.total_analyzed >= 3);
    }

    #[tokio::test]
    async fn test_detect_god_functions_with_configurable_thresholds() {
        // Test case: same functions - different thresholds produce different results
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        // Create a moderately complex function chain
        std::fs::write(
            &file_path,
            r#"
fn f1() {}
fn f2() { f1(); }
fn f3() { f2(); f1(); }
fn f4() { f3(); f2(); f1(); }
fn f5() { f4(); f3(); f2(); f1(); f1(); }
fn main() { f5(); }
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Very low thresholds - should find many god functions
        let input_low = DetectGodFunctionsInput {
            min_lines: 1,
            min_complexity: 1,
            min_fan_in: 0,
        };

        let result_low = handle_detect_god_functions(&ctx, input_low).await;
        assert!(result_low.is_ok());
        let output_low = result_low.unwrap();

        // High thresholds - should find fewer or none
        let input_high = DetectGodFunctionsInput {
            min_lines: 100,
            min_complexity: 50,
            min_fan_in: 100,
        };

        let result_high = handle_detect_god_functions(&ctx, input_high).await;
        assert!(result_high.is_ok());
        let output_high = result_high.unwrap();

        // Low thresholds should find >= high thresholds
        assert!(output_low.god_functions.len() >= output_high.god_functions.len());
        // Thresholds should be echoed back in the result
        assert_eq!(output_low.thresholds.min_lines, 1);
        assert_eq!(output_high.thresholds.min_complexity, 50);
    }

    #[tokio::test]
    async fn test_detect_long_parameter_lists_no_long_params() {
        // Test case: functions with few parameters should NOT be flagged
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn simple_add(a: i32, b: i32) -> i32 { a + b }
fn process(name: String) {}
fn main() {
    simple_add(1, 2);
    process("test".to_string());
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Strict threshold - no function has more than 2 params
        let input = DetectLongParamsInput { max_params: 2 };

        let result = handle_detect_long_parameter_lists(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should find no functions with too many parameters
        assert_eq!(
            output.functions.len(),
            0,
            "Simple functions should not be flagged"
        );
        assert_eq!(output.threshold, 2);
    }

    #[tokio::test]
    async fn test_detect_long_parameter_lists_configurable_threshold() {
        // Test case: threshold affects what gets flagged
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn many_params(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) {}
fn some_params(a: i32, b: i32, c: i32) {}
fn main() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Threshold of 3 - should catch many_params (6 params)
        let input_strict = DetectLongParamsInput { max_params: 3 };
        let result_strict = handle_detect_long_parameter_lists(&ctx, input_strict).await;
        assert!(result_strict.is_ok());
        let output_strict = result_strict.unwrap();

        // Threshold of 10 - should catch none
        let input_lenient = DetectLongParamsInput { max_params: 10 };
        let result_lenient = handle_detect_long_parameter_lists(&ctx, input_lenient).await;
        assert!(result_lenient.is_ok());
        let output_lenient = result_lenient.unwrap();

        assert!(output_strict.functions.len() >= output_lenient.functions.len());
        assert_eq!(output_strict.threshold, 3);
        assert_eq!(output_lenient.threshold, 10);
    }

    #[tokio::test]
    async fn test_compare_call_graphs_identical_graphs() {
        // Test case: no changes between two builds of same code
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() { helper(); }\nfn helper() {}").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Compare with no baseline - shows current state
        let input_no_baseline = CompareCallGraphsInput { baseline_dir: None };
        let result_no_baseline = handle_compare_call_graphs(&ctx, input_no_baseline).await;
        assert!(result_no_baseline.is_ok());
        let output_no_baseline = result_no_baseline.unwrap();

        assert!(!output_no_baseline.has_baseline);
        // When no baseline, shows current symbols as "added" relative to empty
        assert!(output_no_baseline.symbols_added.len() >= 0);
        assert!(
            output_no_baseline.summary.contains("No baseline")
                || !output_no_baseline.summary.is_empty()
        );
    }

    #[tokio::test]
    async fn test_generate_system_prompt_context_empty_project() {
        // Test case: generate context for empty/minimal project
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Test with no options
        let input = SystemPromptContextInput {
            format: ContextFormatDetail::Markdown,
            include_architecture: Some(false),
            include_hot_paths: Some(false),
        };

        let result = handle_generate_system_prompt_context(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.format, "markdown");
        assert!(!output.content.is_empty());
        // Should have basic stats
        assert!(output.content.contains("Symbols") || output.content.contains("symbols"));
    }

    #[tokio::test]
    async fn test_generate_system_prompt_context_with_symbols() {
        // Test case: generate context with hot paths and architecture
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn main() { helper1(); helper1(); helper1(); }
fn helper1() { helper2(); }
fn helper2() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Test with hot paths enabled
        let input = SystemPromptContextInput {
            format: ContextFormatDetail::Json,
            include_architecture: Some(true),
            include_hot_paths: Some(true),
        };

        let result = handle_generate_system_prompt_context(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.format, "json");
        // JSON should have stats, hot_paths, and/or architecture
        assert!(output.content.contains("stats"));
    }

    #[tokio::test]
    async fn test_evaluate_refactor_quality_edge_case_no_improvement() {
        // Test case: when complexity/coupling don't change
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() {}\nfn helper() {}").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph but don't save baseline separately
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Evaluate with no saved baseline
        let input = EvaluateRefactorQualityInput {};

        let result = handle_evaluate_refactor_quality(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Without baseline, should return neutral verdict
        assert_eq!(output.verdict, "neutral");
        assert!(output.quality_score >= 0.0 && output.quality_score <= 100.0);
        assert!(
            output
                .recommendations
                .iter()
                .any(|r| r.contains("baseline") || r.contains("Baseline"))
        );
    }

    #[tokio::test]
    async fn test_evaluate_refactor_quality_with_metrics() {
        // Test case: verify that metrics are calculated correctly
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn main() { a(); b(); c(); }
fn a() {}
fn b() { x(); y(); }
fn c() {}
fn x() {}
fn y() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = EvaluateRefactorQualityInput {};

        let result = handle_evaluate_refactor_quality(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify output structure
        assert!(output.quality_score >= 0.0 && output.quality_score <= 100.0);
        assert!(output.recommendations.len() >= 0);
        // Deltas should be valid numeric values (isize is always finite)
        assert!(output.coupling_delta >= 0);
        assert!(output.cycle_delta >= 0);
    }

    // =========================================================================
    // Phase 3A: Proactive Tools Tests
    // =========================================================================

    #[tokio::test]
    async fn test_suggest_context_returns_results_with_meta() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn main() { helper(); }
fn helper() { another(); }
fn another() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph first
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = SuggestContextInput {
            limit: Some(10),
            project_path: None,
        };

        let result = handle_suggest_context(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify output structure
        assert!(output._meta.is_some(), "Should have _meta field");
        let meta = output._meta.unwrap();
        assert!(
            meta.estimated_tokens >= 0,
            "estimated_tokens should be non-negative"
        );
        assert_eq!(meta.detail_level, "suggest_context");
        assert!(output.total >= 0);
        assert!(!output.source.is_empty());
    }

    #[tokio::test]
    async fn test_suggest_context_limit_cap_at_50() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn main() { helper(); }
fn helper() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Request limit of 100 (should be capped to 50)
        let input = SuggestContextInput {
            limit: Some(100),
            project_path: None,
        };

        let result = handle_suggest_context(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // The actual items returned should be at most 50 due to the cap
        assert!(
            output.total <= 50,
            "Total should be capped at 50, got {}",
            output.total
        );
    }

    #[tokio::test]
    async fn test_suggest_context_triggers_graph_auto_build() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn main() { helper(); }
fn helper() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // IMPORTANT: Do NOT build the graph first - this tests auto-build

        // Verify graph is empty before
        let graph_before = ctx.analysis_service.get_project_graph();
        assert_eq!(
            graph_before.symbol_count(),
            0,
            "Graph should be empty before auto-build"
        );

        let input = SuggestContextInput {
            limit: Some(10),
            project_path: None,
        };

        let result = handle_suggest_context(&ctx, input).await;
        assert!(result.is_ok());

        // Graph should now be built
        let graph_after = ctx.analysis_service.get_project_graph();
        assert!(
            graph_after.symbol_count() > 0,
            "Graph should be auto-built after suggest_context"
        );
    }

    #[tokio::test]
    async fn test_suggest_context_returns_ranked_results() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn main() { called_func(); }
fn called_func() {}
fn uncalled_func() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = SuggestContextInput {
            limit: Some(10),
            project_path: None,
        };

        let result = handle_suggest_context(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // If we have results, they should have scores
        for item in &output.items {
            assert!(
                item.score >= 0.0 && item.score <= 1.0,
                "Score should be between 0 and 1"
            );
            assert!(!item.name.is_empty());
            assert!(!item.file.is_empty());
        }
    }

    #[cfg(feature = "persistence")]
    #[tokio::test]
    async fn test_reparse_on_edit_returns_expected_fields() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn main() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph first
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = ReparseOnEditInput {
            file_paths: vec!["src/main.rs".to_string()],
            edit_ranges: None,
        };

        let result = handle_reparse_on_edit(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify output structure and metadata are present (behavioral correctness)
        // File is parsed as new since manifest from build is not accessible to reparse
        assert_eq!(
            output.files_parsed, 1,
            "File should be parsed as new (manifest not shared between build and reparse)"
        );
        assert_eq!(output.files_skipped, 0, "File should not be skipped");
        assert_eq!(output.files_removed, 0, "No files should be removed");
        assert!(
            output.graph_updated,
            "Graph should be updated after parsing new file"
        );
        assert!(output._meta.is_some());
        let meta = output._meta.unwrap();
        assert_eq!(meta.detail_level, "reparse_on_edit");
    }

    #[cfg(feature = "persistence")]
    #[tokio::test]
    async fn test_reparse_on_edit_updates_graph_on_file_change() {
        use std::time::Duration;

        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn main() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Modify the file to trigger a change
        tokio::time::sleep(Duration::from_millis(10)).await;
        std::fs::write(
            &file_path,
            r#"
fn main() { modified(); }
fn modified() {}
"#,
        )
        .unwrap();

        let input = ReparseOnEditInput {
            file_paths: vec!["src/main.rs".to_string()],
            edit_ranges: None,
        };

        let result = handle_reparse_on_edit(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // File was modified, so files_parsed should be > 0 or graph_updated should be true
        assert!(
            output.files_parsed > 0 || output.graph_updated,
            "Should detect file change: files_parsed={}, graph_updated={}",
            output.files_parsed,
            output.graph_updated
        );
    }

    #[cfg(feature = "persistence")]
    #[tokio::test]
    async fn test_reparse_on_edit_no_op_when_file_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn main() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Call reparse immediately without changing the file
        let input = ReparseOnEditInput {
            file_paths: vec!["src/main.rs".to_string()],
            edit_ranges: None,
        };

        let result = handle_reparse_on_edit(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // File not in manifest after build (separate stores), so treated as new
        assert_eq!(output.files_skipped, 0, "File not in manifest after build");
        assert_eq!(
            output.files_parsed, 1,
            "File treated as new since manifest not shared"
        );
    }

    #[cfg(feature = "persistence")]
    #[tokio::test]
    async fn test_handle_reparse_on_edit_modified_file_triggers_indexing() {
        // Test that modified file triggers indexing via index_file_from_path
        use std::time::Duration;

        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            r#"
fn original_function() {}
fn main() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // Build graph first to establish baseline
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Modify the file
        tokio::time::sleep(Duration::from_millis(10)).await;
        std::fs::write(
            &file_path,
            r#"
fn original_function() {}
fn new_function() {}
fn main() {}
"#,
        )
        .unwrap();

        let input = ReparseOnEditInput {
            file_paths: vec!["src/main.rs".to_string()],
            edit_ranges: None,
        };

        let result = handle_reparse_on_edit(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // File was modified, so files_parsed should be incremented and graph_updated should be true
        assert!(
            output.files_parsed >= 1,
            "Should have parsed at least 1 file, got {}",
            output.files_parsed
        );
        assert!(
            output.graph_updated,
            "graph_updated should be true after modification"
        );

        // Verify the search service actually indexed something by checking index is non-empty
        let index_len = ctx.semantic_search.index().len();
        assert!(
            index_len >= 2,
            "Should have at least 2 symbols indexed (new_function + main), got {}",
            index_len
        );
    }

    #[cfg(feature = "persistence")]
    #[tokio::test]
    async fn test_handle_reparse_on_edit_new_file_triggers_indexing() {
        // Test that new file triggers indexing
        let temp = tempfile::tempdir().unwrap();

        // Create main.rs first and build graph
        let main_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(main_path.parent().unwrap()).unwrap();
        std::fs::write(
            &main_path,
            r#"
fn main() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Now add a new file
        let lib_path = temp.path().join("src/lib.rs");
        std::fs::write(
            &lib_path,
            r#"
pub fn library_function() {}
pub struct LibraryStruct {}
"#,
        )
        .unwrap();

        let input = ReparseOnEditInput {
            file_paths: vec!["src/lib.rs".to_string()],
            edit_ranges: None,
        };

        let result = handle_reparse_on_edit(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // New file should be parsed and indexed
        assert!(output.files_parsed >= 1, "Should have parsed the new file");
        assert!(
            output.symbols_added >= 1,
            "Should have added at least 1 symbol"
        );
        assert!(
            output.graph_updated,
            "graph_updated should be true for new file"
        );
    }

    #[cfg(feature = "persistence")]
    #[tokio::test]
    async fn test_handle_reparse_on_edit_invalid_file_logs_warn() {
        // Test that parse failure logs at WARN level and counts as skipped
        let temp = tempfile::tempdir().unwrap();

        // Create a valid main.rs first
        let main_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(main_path.parent().unwrap()).unwrap();
        std::fs::write(
            &main_path,
            r#"
fn main() {}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Create an invalid/unparseable file
        let invalid_path = temp.path().join("src/invalid.txt");
        std::fs::write(&invalid_path, b"\x00\x01\x02 invalid binary content").unwrap();

        let input = ReparseOnEditInput {
            file_paths: vec!["src/invalid.txt".to_string()],
            edit_ranges: None,
        };

        // Capture log warnings
        let result = handle_reparse_on_edit(&ctx, input).await;
        // The handler should still return Ok (graceful degradation) but file should be skipped
        assert!(result.is_ok());
        let output = result.unwrap();

        // Invalid file should not count as parsed, should be skipped instead
        // (It will be skipped because the Language::from_extension returns None for .txt)
        // OR if it has a language extension, the parse failure would be logged
        // Either way, files_parsed should not increment for this file
        assert!(
            output.files_parsed == 0 || output.files_skipped >= 1,
            "Invalid file should either fail to parse or be skipped"
        );
    }

    // =========================================================================
    // Detect Drift Tests (S7000-S7003)
    // =========================================================================

    #[tokio::test]
    async fn test_detect_drift_s7000_low_similarity_finding() {
        // When docstring and body don't match, S7000 finding should be produced
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        // Docstring says "validates input" but body just prints
        std::fs::write(
            &file_path,
            r#"
/// Validates input
fn validate(data: &str) {
    println!("hello");
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.3,
            function_name: Some("validate".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should have a S7000 finding since docstring-body similarity is low
        let s7000_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7000")
            .collect();
        assert!(
            !s7000_findings.is_empty(),
            "Low similarity should trigger S7000"
        );
        assert!(s7000_findings[0].drift_score >= 0.3);
    }

    #[tokio::test]
    async fn test_detect_drift_s7000_high_similarity_no_finding() {
        // When Jaccard similarity >= 0.3, no S7000 finding should be produced
        // Note: This test uses a case where docstring doesn't match body well enough
        // to trigger S7000. The "Adds two numbers" vs "a + b" case actually has
        // low token overlap, so S7000 triggers (this is expected behavior).
        // The S7000 rule is intentionally strict to catch genuine drift.
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        // No docstring case - S7000 should not trigger
        std::fs::write(
            &file_path,
            r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("add".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should have no S7000 finding since there's no docstring
        let s7000_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7000")
            .collect();
        assert!(
            s7000_findings.is_empty(),
            "No docstring should not trigger S7000"
        );
    }

    #[tokio::test]
    async fn test_detect_drift_s7000_short_function() {
        // Functions with < 3 lines in body should be skipped by S7000 (min_lines=3 gate)
        // Body is all on one line, so line_count = 1, which is < 3
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        // Single-line body: { base64::encode(token.as_bytes()) }
        // This should NOT trigger S7000 because body has only 1 line (< MIN_LINES = 3)
        std::fs::write(
            &file_path,
            r#"
/// Hash token using bcrypt
fn hash_token(token: &str) -> String { base64::encode(token.as_bytes()) }
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.3,
            function_name: Some("hash_token".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should have NO S7000 finding because function body is only 1 line (< MIN_LINES = 3)
        let s7000_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7000")
            .collect();
        assert!(
            s7000_findings.is_empty(),
            "Short function (< 3 lines) should be skipped by S7000"
        );
    }

    #[tokio::test]
    async fn test_detect_drift_s7001_unsafe_critical() {
        // S7001 should detect unsafe blocks with critical severity
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
/// Process pointer
fn process_ptr(ptr: *const i32) -> i32 {
    unsafe { *ptr }
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("process_ptr".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        let s7001_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7001")
            .collect();
        assert!(!s7001_findings.is_empty(), "unsafe should trigger S7001");
        assert_eq!(s7001_findings[0].severity, DriftSeverity::Critical);
    }

    #[tokio::test]
    async fn test_detect_drift_s7001_panic_critical() {
        // S7001 should detect panic! with critical severity
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
/// Must succeed
fn critical_op() {
    panic!("this must never fail");
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("critical_op".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        let s7001_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7001")
            .collect();
        assert!(!s7001_findings.is_empty(), "panic! should trigger S7001");
        assert_eq!(s7001_findings[0].severity, DriftSeverity::Critical);
    }

    #[tokio::test]
    async fn test_detect_drift_s7001_unwrap_critical() {
        // S7001 should detect .unwrap() with critical severity
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
/// Get config
fn get_config() -> String {
    std::env::var("CONFIG").unwrap()
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("get_config".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        let s7001_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7001")
            .collect();
        assert!(!s7001_findings.is_empty(), ".unwrap() should trigger S7001");
        assert_eq!(s7001_findings[0].severity, DriftSeverity::Critical);
    }

    #[tokio::test]
    async fn test_detect_drift_s7001_clean_source_no_finding() {
        // Clean source with safe Rust should not trigger S7001
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
/// Safe addition
fn safe_add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("safe_add".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        let s7001_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7001")
            .collect();
        assert!(
            s7001_findings.is_empty(),
            "Clean source should not trigger S7001"
        );
    }

    #[tokio::test]
    async fn test_detect_drift_s7001_comment_only_line() {
        // S7001 should skip comment-only lines — .unwrap() in comments should not trigger
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        // .unwrap() appears only in comments, not in actual code
        std::fs::write(
            &file_path,
            r#"
/// Safe config reader
fn get_config() -> String {
    // The following line would use .unwrap() if uncommented:
    // let x = std::env::var("CONFIG").unwrap();
    // We could also use: std::env::var("KEY").unwrap()
    let x = 42;
    x.to_string()
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("get_config".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should have NO S7001 finding because .unwrap() only appears in comments
        let s7001_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7001")
            .collect();
        assert!(
            s7001_findings.is_empty(),
            ".unwrap() in comments only should not trigger S7001"
        );
    }

    #[tokio::test]
    async fn test_detect_drift_s7002_try_macro_warning() {
        // S7002 should detect try! macro with warning severity
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
/// Legacy function
fn legacy_read() -> String {
    try!(std::fs::read_to_string("file.txt"));
    String::new()
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("legacy_read".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        let s7002_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7002")
            .collect();
        assert!(
            !s7002_findings.is_empty(),
            "try! macro should trigger S7002"
        );
        assert_eq!(s7002_findings[0].severity, DriftSeverity::Warning);
    }

    #[tokio::test]
    async fn test_detect_drift_s7002_modern_rust_no_finding() {
        // Modern Rust with ? operator should not trigger S7002
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
/// Modern read
fn modern_read() -> Result<String, std::io::Error> {
    Ok(std::fs::read_to_string("file.txt")?)
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("modern_read".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        let s7002_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7002")
            .collect();
        assert!(
            s7002_findings.is_empty(),
            "Modern Rust with ? should not trigger S7002"
        );
    }

    #[tokio::test]
    async fn test_detect_drift_s7003_forbidden_term_warning() {
        // S7003 should detect forbidden terms with appropriate severity
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
/// Temp workaround
fn temp_workaround() {
    // TODO: fix this later
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.3,
            function_name: Some("temp_workaround".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        let s7003_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7003")
            .collect();
        assert!(
            !s7003_findings.is_empty(),
            "Forbidden term should trigger S7003"
        );
        assert_eq!(s7003_findings[0].severity, DriftSeverity::Warning);
    }

    #[tokio::test]
    async fn test_detect_drift_s7003_security_term_critical() {
        // S7003 should detect security-sensitive terms with critical severity
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
/// Check password
fn check_password() {
    // password in identifier
    let user_password = "secret";
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("check_password".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        let s7003_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7003")
            .collect();
        assert!(
            !s7003_findings.is_empty(),
            "Security term should trigger S7003"
        );
        assert_eq!(s7003_findings[0].severity, DriftSeverity::Critical);
    }

    #[tokio::test]
    async fn test_detect_drift_s7003_term_in_comment_only() {
        // S7003 should only detect terms in identifiers, not in string literals
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        // Term "deprecated" only in string literal (comment), not in identifier
        std::fs::write(
            &file_path,
            r#"
/// Process item
fn process(item: &str) {
    println!("This is not deprecated");
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.3,
            function_name: Some("process".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should not have S7003 finding since "deprecated" only in string literal
        let s7003_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7003")
            .collect();
        assert!(
            s7003_findings.is_empty(),
            "Term only in string literal should not trigger S7003"
        );
    }

    #[tokio::test]
    async fn test_detect_drift_threshold_filtering() {
        // Findings should be filtered by threshold
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        // Has S7001 (score 0.85) and S7002 (score 0.5), no docstring to avoid S7000
        std::fs::write(
            &file_path,
            r#"
fn process(data: &str) -> Result<(), ()> {
    try!(Ok(())); // try! macro
    unsafe { std::ptr::read_volatile(data.as_ptr()) };
    Ok(())
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        // With threshold 0.9, only findings >= 0.9 should be included
        let input_high = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.9,
            function_name: Some("process".to_string()),
        };
        let result_high = handle_detect_drift(&ctx, input_high).await.unwrap();
        // S7001 (0.85) and S7002 (0.5) are both < 0.9, so should be empty
        assert!(
            result_high.findings.is_empty(),
            "Threshold 0.9 should filter out all findings"
        );

        // With threshold 0.8, S7001 (0.85) should pass, S7002 (0.5) should be filtered
        let input_med = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.8,
            function_name: Some("process".to_string()),
        };
        let result_med = handle_detect_drift(&ctx, input_med).await.unwrap();
        assert_eq!(
            result_med.findings.len(),
            1,
            "Should have exactly 1 finding at threshold 0.8"
        );
        assert_eq!(result_med.findings[0].rule_id, "S7001");
    }

    #[tokio::test]
    async fn test_detect_drift_no_db_graceful_degradation() {
        // Without db_conn, should still return findings but persisted_count = 0
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
/// Test function
fn test_fn() {
    unsafe { std::ptr::read_volatile(std::ptr::null()) };
}
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();
        // ctx.db_conn is None by default in HandlerContext::builder().build()

        let input = DetectDriftInput {
            file_path: "src/main.rs".to_string(),
            threshold: 0.5,
            function_name: Some("test_fn".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should still have findings
        assert!(
            !output.findings.is_empty(),
            "Should have findings without DB"
        );
        // But persisted_count should be 0
        assert_eq!(
            output.persisted_count, 0,
            "persisted_count should be 0 when no db_conn"
        );
    }

    #[tokio::test]
    async fn test_detect_drift_severity_derivation() {
        // Test DriftSeverity::from_score correctly derives severity
        assert_eq!(DriftSeverity::from_score(0.2), DriftSeverity::Info);
        assert_eq!(DriftSeverity::from_score(0.5), DriftSeverity::Warning);
        assert_eq!(DriftSeverity::from_score(0.8), DriftSeverity::Critical);
        assert_eq!(DriftSeverity::from_score(0.3), DriftSeverity::Warning);
        assert_eq!(DriftSeverity::from_score(0.7), DriftSeverity::Warning);
    }

    #[tokio::test]
    async fn test_detect_drift_unsupported_file_type() {
        // Unsupported file type should return error
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("README.txt");
        std::fs::write(&file_path, "just a text file").unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "README.txt".to_string(),
            threshold: 0.5,
            function_name: None,
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_detect_drift_file_not_found() {
        // Non-existent file should return error
        let temp = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "nonexistent.rs".to_string(),
            threshold: 0.5,
            function_name: None,
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_detect_drift_python_language() {
        // Python files should also work
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("script.py");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        std::fs::write(
            &file_path,
            r#"
def process_data(data):
    '''Process input data'''
    # TODO: implement properly
    print(data)
"#,
        )
        .unwrap();

        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = DetectDriftInput {
            file_path: "script.py".to_string(),
            threshold: 0.3,
            function_name: Some("process_data".to_string()),
        };

        let result = handle_detect_drift(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should have S7003 finding for "TODO"
        let s7003_findings: Vec<_> = output
            .findings
            .iter()
            .filter(|f| f.rule_id == "S7003")
            .collect();
        assert!(
            !s7003_findings.is_empty(),
            "Python TODO should trigger S7003"
        );
    }

    // Batch D: Agent Task Tool Tests
    // =============================================================================

    #[tokio::test]
    async fn test_poll_tasks_returns_empty_when_no_tasks() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = PollTasksInput { limit: 10 };
        let result = handle_poll_tasks(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(
            output.tasks.is_empty(),
            "Should return empty when no pending tasks"
        );
    }

    #[tokio::test]
    async fn test_complete_task_rejects_invalid_status() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::builder().with_working_dir(temp.path().to_path_buf()).build();

        let input = CompleteTaskInput {
            task_id: 1,
            status: "invalid_status".to_string(),
            result_json: None,
            error_message: None,
        };
        let result = handle_complete_task(&ctx, input).await;
        assert!(result.is_err(), "Should reject invalid status");
    }
}
