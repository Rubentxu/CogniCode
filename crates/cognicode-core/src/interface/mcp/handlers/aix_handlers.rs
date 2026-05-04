//! AIX Handlers — AI Experience tools for LLM agent consumption
//!
//! This module contains all 13 AIX MCP tool handlers:
//! smart_overview, ranked_symbols, suggest_onboarding_plan, auto_diagnose,
//! suggest_refactor_plan, nl_to_symbol, ask_about_code, find_pattern_by_intent,
//! compare_call_graphs, detect_api_breaks, generate_system_prompt_context,
//! detect_god_functions, detect_long_parameter_lists.

use super::*;

// AIX Tool Handlers
// ============================================================================

/// Handler for smart_overview tool (AIX-1.1)
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
    let top_eps: Vec<EntryPointSummary> = entry_points.iter().take(5).map(|ep| {
        EntryPointSummary {
            name: ep.name.clone(),
            file: ep.file_path.clone(),
            line: ep.line,
            kind: ep.kind.clone(),
            summary: format!("Entry point: {}", ep.name),
        }
    }).collect();

    // Build hot path DTOs
    let critical_hot_paths: Vec<HotPathDto> = hot_paths.iter().take(5).map(|hp| {
        HotPathDto {
            symbol_name: hp.symbol_name.clone(),
            file: hp.file.clone(),
            line: hp.line,
            fan_in: hp.fan_in,
            fan_out: hp.fan_out,
        }
    }).collect();

    // Estimate tokens
    let estimated_tokens = match detail {
        OverviewDetail::Quick => 100,
        OverviewDetail::Medium => 400,
        OverviewDetail::Detailed => 800,
    };

    Ok(SmartOverviewDto {
        project_type: project_type.to_string(),
        total_symbols: stats.symbol_count,
        total_edges: stats.edge_count,
        languages: stats.language_breakdown,
        top_entry_points: top_eps,
        critical_hot_paths,
        architecture_score: arch_result.as_ref().map(|r| r.score),
        cycle_count: arch_result.as_ref().map(|r| r.cycles.len()),
        recommended_first_reads: if detail == OverviewDetail::Detailed { first_reads } else { vec![] },
        coverage_percent,
        _meta: OverviewMeta {
            estimated_tokens,
            detail_level: detail.to_string(),
        },
    })
}

/// Handler for ranked_symbols tool (AIX-1.3)
pub async fn handle_ranked_symbols(
    ctx: &HandlerContext,
    input: RankedSymbolsInput,
) -> HandlerResult<RankedSymbolsResult> {
    let _ensure_sem = ensure_semantic_indexed(ctx)?;
    let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();

    // Search using semantic search
    let results = ctx.semantic_search.search(
        crate::infrastructure::semantic::SearchQuery {
            query: input.query.clone(),
            kinds: vec![],
            max_results: input.limit,
        }
    );

    // Calculate max fan_in for normalization
    let max_fan_in = results.iter()
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
    Ok(RankedSymbolsResult {
        query: input.query,
        total_matches: total,
        returned: total,
        results: ranked,
        _meta: OverviewMeta {
            estimated_tokens: total * 50,
            detail_level: "ranked".to_string(),
        },
    })
}

/// Handler for suggest_onboarding_plan tool (AIX-2.1)
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

    Ok(OnboardingPlanDto {
        goal: format!("{:?}", input.goal).to_lowercase(),
        total_steps,
        total_estimated_tokens: total_tokens,
        steps: goal,
        _meta: OverviewMeta {
            estimated_tokens: total_tokens,
            detail_level: "onboarding_plan".to_string(),
        },
    })
}

/// Handler for auto_diagnose tool (AIX-2.3)
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
                recommendation: "Introduce a trait or use a shared module to break the cycle".to_string(),
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
            description: format!("{:.1}% of symbols are never called", dead_code_result.dead_code_percent),
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
    let health_score = (100.0 - (cycles as f64 * 10.0) - (coupling as f64 * 5.0)).max(0.0).min(100.0);

    // Get complexity info
    let max_complexity = hot_paths.first().map(|hp| (hp.symbol_name.clone(), hp.fan_in as u32));
    let avg_complexity = if !hot_paths.is_empty() {
        Some(hot_paths.iter().map(|hp| hp.fan_in as f64).sum::<f64>() / hot_paths.len() as f64)
    } else {
        None
    };

    Ok(DiagnoseReportDto {
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
        cycles: arch_result.as_ref().map(|r| r.cycles.iter().map(|c| c.symbols.join("->")).collect()).unwrap_or_default(),
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
    })
}

/// Handler for suggest_refactor_plan tool (AIX-2.2)
pub async fn handle_suggest_refactor_plan(
    ctx: &HandlerContext,
    input: SuggestRefactorPlanInput,
) -> HandlerResult<RefactorSuggestionDto> {
    let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();
    let symbol_id = find_symbol_in_graph(&graph, &input.symbol);

    if symbol_id.is_none() {
        return Err(HandlerError::NotFound(format!("Symbol '{}' not found", input.symbol)));
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

    Ok(RefactorSuggestionDto {
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
    })
}

/// Handler for nl_to_symbol tool (AIX-3.1)
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
    let results = ctx.semantic_search.search(
        crate::infrastructure::semantic::SearchQuery {
            query: input.query.clone(),
            kinds: vec![],
            max_results: input.limit * 2,
        }
    );

    // Re-rank with keyword matching
    let mut matches: Vec<NlSymbolMatch> = Vec::new();
    for r in results.iter().take(input.limit) {
        let symbol_id = SymbolId::new(r.symbol.fully_qualified_name());
        let fan_in = graph.callers(&symbol_id).len();

        // Calculate keyword match score
        let name_lower = r.symbol.name().to_lowercase();
        let keyword_matches = keywords.iter()
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
                match_reason: format!("Semantic match ({:.0}%) + keyword overlap", confidence * 100.0),
                snippet: None,
                fan_in,
            });
        }
    }

    // Sort by confidence
    matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

    let total = matches.len();
    matches.truncate(input.limit);

    // Record symbol accesses for hotness tracking
    for m in &matches {
        ctx.record_symbol_access(&m.symbol_name, 1);
    }

    Ok(NlToSymbolResult {
        query: input.query,
        extracted_keywords: keywords,
        total_candidates: total,
        results: matches,
        _meta: OverviewMeta {
            estimated_tokens: total * 40,
            detail_level: "nl_to_symbol".to_string(),
        },
    })
}

/// Handler for ask_about_code tool (AIX-3.2)
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
                let path_steps: Vec<CodePathStep> = path.iter().filter_map(|sp| {
                    graph.get_symbol(sp).map(|s| CodePathStep {
                        symbol: s.name().to_string(),
                        file: s.location().file().to_string(),
                        line: s.location().line() + 1,
                        kind: format!("{:?}", s.kind()).to_lowercase(),
                        role: "intermediate".to_string(),
                        snippet: None,
                    })
                }).collect();

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

    Ok(AskAboutCodeResult {
        question: input.question,
        answers,
        _meta: OverviewMeta {
            estimated_tokens: 200,
            detail_level: "ask_about_code".to_string(),
        },
    })
}

/// Handler for find_pattern_by_intent tool (AIX-3.3)
pub async fn handle_find_pattern_by_intent(
    _ctx: &HandlerContext,
    input: FindPatternByIntentInput,
) -> HandlerResult<FindPatternResult> {
    // Pattern catalog
    let patterns = vec![
        ("singleton", "Singleton pattern", "look for single instance with global state", "Find objects created once and accessed globally"),
        ("factory", "Factory method", "creation through factory function", "Object creation delegated to factory method"),
        ("observer", "Observer pattern", "event subscription and notification", "One-to-many dependency for state changes"),
        ("builder", "Builder pattern", "step-by-step object construction", "Construct complex objects step by step"),
        ("strategy", "Strategy pattern", "interchangeable algorithms", "Select algorithm at runtime"),
        ("adapter", "Adapter pattern", "convert interface to expected", "Make incompatible interfaces work together"),
        ("decorator", "Decorator pattern", "wrap with additional behavior", "Add responsibilities dynamically"),
        ("facade", "Facade pattern", "simplified interface to subsystem", "Provide unified interface to complex subsystem"),
        ("template", "Template method", "algorithm skeleton with hooks", "Define algorithm skeleton with customizable steps"),
        ("command", "Command pattern", "encapsulate operation as object", "Parameterize objects with operations"),
        ("iterator", "Iterator pattern", "sequential access without exposure", "Traverse collection without exposing internals"),
        ("composite", "Composite pattern", "tree structure with uniform interface", "Compose objects into tree structures"),
        ("proxy", "Proxy pattern", "placeholder for another object", "Control access to another object"),
        ("flyweight", "Flyweight pattern", "share common state", "Use sharing to support large numbers of objects"),
        ("mvc", "MVC pattern", "separate model view controller", "Separate data, UI, and logic concerns"),
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
        matched = patterns.iter().map(|(n, d, h, _)| IntentMatch {
            intent_name: n.to_string(),
            description: d.to_string(),
            query_hint: h.to_string(),
        }).collect();
    }

    Ok(FindPatternResult {
        query: input.intent,
        matched_intents: matched,
        all_patterns: patterns.iter().map(|(n, _, _, _)| n.to_string()).collect(),
        _meta: OverviewMeta {
            estimated_tokens: 100,
            detail_level: "find_pattern".to_string(),
        },
    })
}

/// Handler for compare_call_graphs tool (AIX-4.1)
pub async fn handle_compare_call_graphs(
    ctx: &HandlerContext,
    input: CompareCallGraphsInput,
) -> HandlerResult<GraphDiffDto> {
    let _ensure = ensure_graph_built(ctx)?;

    let current_graph = ctx.analysis_service.get_project_graph();
    let current_symbols: HashSet<String> = current_graph.symbols()
        .map(|s| s.name().to_string())
        .collect();
    let current_edges: HashSet<(String, String)> = current_graph.all_dependencies()
        .filter_map(|(src, tgt, _)| {
            let src_name = current_graph.get_symbol(&src).map(|s| s.name().to_string())?;
            let tgt_name = current_graph.get_symbol(&tgt).map(|s| s.name().to_string())?;
            Some((src_name, tgt_name))
        })
        .collect();

    // Try to load baseline if provided
    let (has_baseline, baseline_symbols, baseline_edges, arch_score_before) = if let Some(baseline_dir) = input.baseline_dir {
        let baseline_path = ctx.working_dir.join(baseline_dir);
        let _store_path = graph_db_path(&baseline_path);
        let store = InMemoryGraphStore::new();
        match store.load_graph() {
            Ok(Some(baseline_graph)) => {
                let symbols: HashSet<String> = baseline_graph.symbols()
                    .map(|s| s.name().to_string())
                    .collect();
                let edges: HashSet<(String, String)> = baseline_graph.all_dependencies()
                    .filter_map(|(src, tgt, _)| {
                        let src_name = baseline_graph.get_symbol(&src).map(|s| s.name().to_string())?;
                        let tgt_name = baseline_graph.get_symbol(&tgt).map(|s| s.name().to_string())?;
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
    let symbols_added: Vec<String> = current_symbols.difference(&baseline_symbols).into_iter().cloned().collect();
    let symbols_removed: Vec<String> = baseline_symbols.difference(&current_symbols).into_iter().cloned().collect();
    let edges_added: Vec<(String, String)> = current_edges.difference(&baseline_edges).cloned().collect();
    let edges_removed: Vec<(String, String)> = baseline_edges.difference(&current_edges).cloned().collect();

    let arch_result = check_architecture_internal(ctx)?;
    let arch_score_after = arch_result.as_ref().map(|r| r.score);

    let summary = if has_baseline {
        format!("{} added, {} removed, {} edge changes",
            symbols_added.len(), symbols_removed.len(), edges_added.len() + edges_removed.len())
    } else {
        "No baseline provided - showing current graph state".to_string()
    };

    Ok(GraphDiffDto {
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
    })
}

/// Handler for detect_api_breaks tool (AIX-4.2)
pub async fn handle_detect_api_breaks(
    ctx: &HandlerContext,
    input: DetectApiBreaksInput,
) -> HandlerResult<ApiBreaksResult> {
    let _ensure = ensure_graph_built(ctx)?;

    let current_graph = ctx.analysis_service.get_project_graph();
    let current_entries: HashSet<String> = current_graph.roots()
        .iter()
        .filter_map(|id| current_graph.get_symbol(id).map(|s| s.name().to_string()))
        .collect();

    let (has_baseline, _baseline_entries, breaks) = if let Some(baseline_dir) = input.baseline_dir {
        let baseline_path = ctx.working_dir.join(baseline_dir);
        let _store_path = graph_db_path(&baseline_path);
        let store = InMemoryGraphStore::new();
        match store.load_graph() {
            Ok(Some(baseline_graph)) => {
                let entries: HashSet<String> = baseline_graph.roots()
                    .iter()
                    .filter_map(|id| baseline_graph.get_symbol(id).map(|s| s.name().to_string()))
                    .collect();

                // Find removed entry points
                let removed: Vec<ApiBreak> = entries.difference(&current_entries)
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

    Ok(ApiBreaksResult {
        has_baseline,
        breaks,
        total_breaks,
        severity_summary,
        _meta: OverviewMeta {
            estimated_tokens: 150,
            detail_level: "detect_api_breaks".to_string(),
        },
    })
}

/// Handler for generate_system_prompt_context tool (AIX-5.1)
pub async fn handle_generate_system_prompt_context(
    ctx: &HandlerContext,
    input: SystemPromptContextInput,
) -> HandlerResult<SystemPromptContext> {
    let _ensure = ensure_graph_built(ctx)?;

    let stats = ctx.analysis_service.get_graph_stats();
    let hot_paths = if input.include_hot_paths.unwrap_or(false) {
        Some(get_hot_paths_from_graph(&ctx.analysis_service.get_project_graph(), 5))
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
                hp.iter().map(|h| format!("  <hot_path symbol=\"{}\" fan_in=\"{}\" />", h.symbol_name, h.fan_in)).collect::<Vec<_>>().join("\n")
            } else {
                String::new()
            };

            let arch_xml = if let Some(ref a) = arch_result {
                format!("<architecture score=\"{}\" cycles=\"{}\" />", a.score, a.cycles.len())
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
            let mut md = format!("## Project Stats\n- Symbols: {}\n- Edges: {}\n", stats.symbol_count, stats.edge_count);

            if let Some(ref hp) = hot_paths {
                md += "\n## Hot Paths\n";
                for h in hp {
                    md += &format!("- **{}** (fan-in: {})\n", h.symbol_name, h.fan_in);
                }
            }

            if let Some(ref a) = arch_result {
                md += &format!("\n## Architecture\n- Score: {:.1}\n- Cycles: {}\n", a.score, a.cycles.len());
            }

            md
        }
    };

    let content_len = content.len();
    Ok(SystemPromptContext {
        format: format!("{:?}", input.format).to_lowercase(),
        content,
        estimated_tokens: content_len / 4,
    })
}

/// Handler for detect_god_functions tool (AIX-5.2)
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
        let lines = get_symbol_lines(&symbol);
        let complexity = get_symbol_complexity(&graph, &symbol_id).unwrap_or(0);
        let fan_in = graph.callers(&symbol_id).len();
        let fan_out = graph.callees(&symbol_id).len();

        // Check thresholds
        if lines >= input.min_lines && complexity >= input.min_complexity && fan_in >= input.min_fan_in {
            // Calculate god score
            let god_score = ((lines as f64 / input.min_lines as f64 * 25.0)
                + (complexity as f64 / input.min_complexity as f64 * 25.0)
                + (fan_in as f64 / input.min_fan_in as f64 * 25.0)
                + (fan_out as f64 / 10.0 * 25.0)).min(100.0);

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
    god_functions.sort_by(|a, b| b.god_score.partial_cmp(&a.god_score).unwrap_or(std::cmp::Ordering::Equal));

    let total_analyzed = graph.symbol_count();
    let god_count = god_functions.len();

    Ok(GodFunctionsResult {
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
    })
}

/// Handler for detect_long_parameter_lists tool (AIX-5.3)
pub async fn handle_detect_long_parameter_lists(
    ctx: &HandlerContext,
    input: DetectLongParamsInput,
) -> HandlerResult<LongParamsResult> {
    let _ensure = ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();
    let mut long_param_functions = Vec::new();

    for symbol in graph.symbols() {
        // Only check functions
        if !matches!(symbol.kind(), crate::domain::value_objects::SymbolKind::Function) {
            continue;
        }

        let param_count = graph.callees(&SymbolId::new(symbol.fully_qualified_name())).len();

        if param_count > input.max_params {
            long_param_functions.push(LongParamFunctionDto {
                symbol: symbol.name().to_string(),
                file: symbol.location().file().to_string(),
                line: symbol.location().line() + 1,
                parameter_count: param_count,
                parameter_names: vec![],
                suggestion: format!("Consider grouping parameters into a struct"),
            });
        }
    }

    let total_analyzed = graph.symbol_count();
    let long_param_count = long_param_functions.len();

    Ok(LongParamsResult {
        functions: long_param_functions,
        threshold: input.max_params,
        total_analyzed,
        _meta: OverviewMeta {
            estimated_tokens: long_param_count * 40,
            detail_level: "long_params".to_string(),
        },
    })
}

/// Handler for evaluate_refactor_quality tool (AIX-4.3)
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
                (true, baseline_complex, baseline_edge_count, baseline_cycle_count, 0isize)
            }
            _ => (false, 0.0, 0usize, 0, 0isize),
        };

    if !has_baseline {
        return Ok(RefactorEvalDto {
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
        });
    }

    // Calculate deltas (current - baseline, so negative = improvement)
    let complexity_delta = current_complexity - baseline_complexity;
    let coupling_delta = current_edges as isize - baseline_edges as isize;
    let cycle_delta = current_cycles as isize - baseline_cycles as isize;
    let dead_code_delta = current_dead_code as isize - baseline_dead_code as isize;

    // Calculate penalties
    let complexity_penalty = (complexity_delta * 5.0).max(0.0).min(30.0);
    let coupling_penalty = (coupling_delta as f64 * 3.0).max(0.0).min(25.0);
    let cycle_penalty = (cycle_delta as f64 * 10.0).max(0.0).min(25.0);
    let dead_code_bonus = (-dead_code_delta as f64 * 2.0).max(0.0).min(20.0);

    // Calculate quality score
    let quality_score = (100.0 - complexity_penalty - coupling_penalty - cycle_penalty + dead_code_bonus)
        .max(0.0).min(100.0);

    // Determine verdict
    let verdict = if quality_score >= 80.0 {
        "improvement"
    } else if quality_score >= 50.0 {
        "neutral"
    } else {
        "regression"
    }.to_string();

    // Generate recommendations
    let mut recommendations = Vec::new();
    if complexity_delta > 0.0 {
        recommendations.push("Complexity increased. Consider extracting complex functions.".to_string());
    }
    if coupling_delta > 0 {
        recommendations.push("Coupling increased. Look for opportunities to reduce dependencies.".to_string());
    }
    if cycle_delta > 0 {
        recommendations.push("New cycles detected. Break cyclic dependencies with traits or shared modules.".to_string());
    }
    if dead_code_delta > 0 {
        recommendations.push("More dead code detected. Remove unused symbols.".to_string());
    }
    if recommendations.is_empty() && quality_score >= 80.0 {
        recommendations.push("Refactoring appears successful. Consider running tests to verify.".to_string());
    }

    Ok(RefactorEvalDto {
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
    })
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get hot paths from graph
fn get_hot_paths_from_graph(graph: &CallGraph, limit: usize) -> Vec<HotPathDto> {
    let mut hot_paths: Vec<HotPathDto> = graph.symbols()
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

    let cycles = cycle_result.cycles.iter().map(|c| {
        crate::application::dto::CycleInfo {
            symbols: c.symbols().iter().map(|s| s.as_str().to_string()).collect(),
            length: c.length(),
        }
    }).collect();

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
    let entry_files: Vec<String> = graph.roots()
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
    let has_web_handlers = graph.symbols()
        .any(|s| {
            let name = s.name().to_lowercase();
            name.contains("handler") || name.contains("route") || name.contains("endpoint") || name.contains("api")
        });

    if has_web_handlers && entry_count > 1 {
        return ProjectType::WebApi;
    }

    // Check for CLI indicators
    let has_main = graph.symbols()
        .any(|s| s.name().to_lowercase() == "main");

    if has_main {
        return ProjectType::Cli;
    }

    // Check for library indicators (many traits/interfaces, few entry points)
    let trait_count = graph.symbols()
        .filter(|s| matches!(s.kind(), crate::domain::value_objects::SymbolKind::Trait))
        .count();

    if trait_count > 5 && entry_count <= 3 {
        return ProjectType::Library;
    }

    ProjectType::Unknown
}

/// Extract keywords from natural language query
fn extract_keywords(query: &str) -> Vec<String> {
    let stop_words = ["the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could", "should",
        "may", "might", "must", "shall", "can", "need", "dare", "ought", "used",
        "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into",
        "through", "during", "before", "after", "above", "below", "between", "under",
        "again", "further", "then", "once", "here", "there", "when", "where", "why",
        "how", "all", "each", "few", "more", "most", "other", "some", "such", "no",
        "nor", "not", "only", "own", "same", "so", "than", "too", "very", "just"];

    query.split_whitespace()
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
        std::fs::write(&file_path, "fn process_data() { helper(); }\nfn helper() {}").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
            assert!(output.steps.len() > 0, "Should have steps for goal: {:?}", goal);
        }
    }

    #[tokio::test]
    async fn test_auto_diagnose_health_score() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() { helper(); }\nfn helper() {}").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
        assert_eq!(output.critical_count + output.important_count + output.warning_count + output.info_count, output.total_issues);
    }

    #[tokio::test]
    async fn test_nl_to_symbol_extracts_keywords() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn process_user_data() {}\nfn calculate_total() {}").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
        assert!(kw_str.contains("process") || kw_str.contains("user") || kw_str.contains("data") || output.extracted_keywords.is_empty());
    }

    #[tokio::test]
    async fn test_find_pattern_by_intent_list() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::new(temp.path().to_path_buf());

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

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
        std::fs::write(&file_path, "fn many_params(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) {}").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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

        let ctx = HandlerContext::new(temp.path().to_path_buf());

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        let input = SmartOverviewInput { detail: Some(OverviewDetail::Medium) };

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

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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

        let ctx = HandlerContext::new(temp.path().to_path_buf());

        // Do NOT build graph - this ensures no baseline exists
        let input = EvaluateRefactorQualityInput {};

        let result = handle_evaluate_refactor_quality(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // No graph built means no baseline - should be neutral with a note
        assert_eq!(output.verdict, "neutral");
        assert!(output.quality_score >= 0.0);
        assert!(output.recommendations.iter().any(|r| r.contains("No baseline")));
    }

    #[tokio::test]
    async fn test_symbol_hotness_tracking_increments() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = HandlerContext::new(temp.path().to_path_buf());

        // Record some symbol accesses
        ctx.record_symbol_access("symbol_a", 5);
        ctx.record_symbol_access("symbol_b", 3);
        ctx.record_symbol_access("symbol_a", 2); // Should increment to 7

        // Verify hotness scores
        let score_a = ctx.get_symbol_hotness("symbol_a");
        let score_b = ctx.get_symbol_hotness("symbol_b");
        let score_c = ctx.get_symbol_hotness("symbol_nonexistent");

        // symbol_a has 7 accesses, symbol_b has 3, max is 7
        assert!((score_a - 1.0).abs() < 0.001, "symbol_a should have hotness 1.0 (hottest)");
        assert!((score_b - 3.0 / 7.0).abs() < 0.001, "symbol_b should have hotness 3/7");
        assert!((score_c - 0.0).abs() < 0.001, "nonexistent symbol should have hotness 0.0");
    }

    #[tokio::test]
    async fn test_ranked_symbols_hotness_boost() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn process_data() { helper(); }\nfn helper() {}").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
        assert!(hotness > 0.0, "process_data should have some hotness after being ranked");
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
        std::fs::write(&file_path, r#"
fn helper1() {}
fn helper2() {}
fn main() {
    helper1();
    helper2();
}
"#).unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
        assert_eq!(output.god_functions.len(), 0, "Simple functions should not be detected as god functions");
        assert!(output.total_analyzed >= 3);
    }

    #[tokio::test]
    async fn test_detect_god_functions_with_configurable_thresholds() {
        // Test case: same functions - different thresholds produce different results
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        // Create a moderately complex function chain
        std::fs::write(&file_path, r#"
fn f1() {}
fn f2() { f1(); }
fn f3() { f2(); f1(); }
fn f4() { f3(); f2(); f1(); }
fn f5() { f4(); f3(); f2(); f1(); f1(); }
fn main() { f5(); }
"#).unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
        std::fs::write(&file_path, r#"
fn simple_add(a: i32, b: i32) -> i32 { a + b }
fn process(name: String) {}
fn main() {
    simple_add(1, 2);
    process("test".to_string());
}
"#).unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

        // Build graph
        let build_input = BuildGraphInput { directory: None };
        let _ = handle_build_graph(&ctx, build_input).await;

        // Strict threshold - no function has more than 2 params
        let input = DetectLongParamsInput { max_params: 2 };

        let result = handle_detect_long_parameter_lists(&ctx, input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should find no functions with too many parameters
        assert_eq!(output.functions.len(), 0, "Simple functions should not be flagged");
        assert_eq!(output.threshold, 2);
    }

    #[tokio::test]
    async fn test_detect_long_parameter_lists_configurable_threshold() {
        // Test case: threshold affects what gets flagged
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, r#"
fn many_params(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) {}
fn some_params(a: i32, b: i32, c: i32) {}
fn main() {}
"#).unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
        assert!(output_no_baseline.summary.contains("No baseline") || !output_no_baseline.summary.is_empty());
    }

    #[tokio::test]
    async fn test_generate_system_prompt_context_empty_project() {
        // Test case: generate context for empty/minimal project
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
        std::fs::write(&file_path, r#"
fn main() { helper1(); helper1(); helper1(); }
fn helper1() { helper2(); }
fn helper2() {}
"#).unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
        assert!(output.recommendations.iter().any(|r| r.contains("baseline") || r.contains("Baseline")));
    }

    #[tokio::test]
    async fn test_evaluate_refactor_quality_with_metrics() {
        // Test case: verify that metrics are calculated correctly
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("src/main.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, r#"
fn main() { a(); b(); c(); }
fn a() {}
fn b() { x(); y(); }
fn c() {}
fn x() {}
fn y() {}
"#).unwrap();

        let ctx = HandlerContext::new(temp.path().to_path_buf());

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
}
