//! Pure view builders.
//!
//! Each function takes already-resolved data and shapes it into a
//! `ContextualView`. No I/O, no trait calls beyond the ports.

use serde_json::json;

use crate::dto::{
    ContextualView, EvidenceBlock, LineRange, RelationDirection, TypedRelation, ViewBlock,
};
use crate::ports::quality_repository::{QualityIssue, QualityRepository, RuleSummary};
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{RelationTarget, ResolvedSymbol, SymbolRepository};

/// Build the Overview view: identity + call graph metrics + signature for callables.
pub fn build_overview(symbol: &ResolvedSymbol, repo: &dyn SymbolRepository) -> ContextualView {
    let mut blocks: Vec<ViewBlock> = Vec::new();
    let evidence = vec![symbol_metadata_evidence(symbol)];

    blocks.push(ViewBlock {
        id: "identity".into(),
        title: "Identity".into(),
        body: json!({
            "name": symbol.name,
            "kind": symbol.kind.name(),
            "file": symbol.file,
            "line": symbol.line,
        }),
    });

    blocks.push(ViewBlock {
        id: "call_metrics".into(),
        title: "Call metrics".into(),
        body: json!({
            "fan_in": repo.fan_in(&symbol.id),
            "fan_out": repo.fan_out(&symbol.id),
        }),
    });

    if symbol.kind.is_callable() {
        blocks.push(ViewBlock {
            id: "signature".into(),
            title: "Signature".into(),
            body: json!({
                "signature": symbol.signature.clone().unwrap_or_default(),
            }),
        });
    }

    ContextualView {
        object_id: mvp_id(symbol),
        view_id: "overview".into(),
        title: "Overview".into(),
        blocks,
        relations: Vec::new(),
        evidence,
        findings: Vec::new(),
    }
}

/// Build the Call Graph view: incoming + outgoing relations and their counts.
pub fn build_callgraph(
    symbol: &ResolvedSymbol,
    repo: &dyn SymbolRepository,
) -> ContextualView {
    let callers = repo.callers(&symbol.id);
    let callees = repo.callees(&symbol.id);

    let cg_evidence_id = "evidence:call_graph".to_string();
    let cg_evidence = EvidenceBlock {
        id: cg_evidence_id.clone(),
        kind: "call_graph".into(),
        title: format!("Call graph of {}", symbol.name),
        file: Some(symbol.file.clone()),
        line_range: Some(LineRange {
            start: symbol.line,
            end: symbol.line,
        }),
        source_tool_or_query: "CallGraph::callers/callees".into(),
        confidence: Some(1.0),
        // Graph build time is not exposed through the explorer port.
        freshness: Some("unknown".into()),
    };

    let mut relations: Vec<TypedRelation> = Vec::new();

    for c in &callers {
        relations.push(relation_for("CALLED_BY", RelationDirection::Incoming, c, &cg_evidence_id));
    }
    for c in &callees {
        relations.push(relation_for("CALLS", RelationDirection::Outgoing, c, &cg_evidence_id));
    }

    let blocks = vec![
        ViewBlock {
            id: "callers".into(),
            title: format!("Callers ({})", callers.len()),
            body: json!({
                "count": callers.len(),
                "items": callers.iter().map(relation_summary).collect::<Vec<_>>(),
            }),
        },
        ViewBlock {
            id: "callees".into(),
            title: format!("Callees ({})", callees.len()),
            body: json!({
                "count": callees.len(),
                "items": callees.iter().map(relation_summary).collect::<Vec<_>>(),
            }),
        },
    ];

    ContextualView {
        object_id: mvp_id(symbol),
        view_id: "call-graph".into(),
        title: "Call Graph".into(),
        blocks,
        relations,
        evidence: vec![cg_evidence],
        findings: Vec::new(),
    }
}

/// Build the Source view: a numbered slice of the file around the symbol's line.
pub fn build_source(
    symbol: &ResolvedSymbol,
    reader: &dyn SourceReader,
) -> ContextualView {
    let start = symbol.line.saturating_sub(7).max(1);
    let end = symbol.line + 8;
    let slice = reader
        .read_lines(&symbol.file, start, end)
        .unwrap_or_default();

    let blocks = vec![ViewBlock {
        id: "source_slice".into(),
        title: format!("Source slice (lines {start}–{end})"),
        body: json!({
            "file": symbol.file,
            "line": symbol.line,
            "lines": slice
                .iter()
                .map(|(n, l)| json!({ "line": n, "text": l }))
                .collect::<Vec<_>>(),
        }),
    }];

    let evidence = vec![EvidenceBlock {
        id: "evidence:source_file".into(),
        kind: "source_file".into(),
        title: format!("Source file: {}", symbol.file),
        file: Some(symbol.file.clone()),
        line_range: Some(LineRange {
            start,
            end: end.min(symbol.line + 8),
        }),
        source_tool_or_query: "SourceReader::read_lines".into(),
        confidence: Some(1.0),
        // `read_lines` already proved the file is reachable; use the same
        // file-exists heuristic as `evidence::fs_index_evidence` so callers
        // see a consistent freshness signal.
        freshness: Some(if slice.is_empty() { "stale".into() } else { "fresh".into() }),
    }];

    ContextualView {
        object_id: mvp_id(symbol),
        view_id: "source".into(),
        title: "Source".into(),
        blocks,
        relations: Vec::new(),
        evidence,
        findings: Vec::new(),
    }
}

fn relation_summary(t: &RelationTarget) -> serde_json::Value {
    json!({
        "object_id": mvp_id_from_target(t),
        "name": t.name,
        "kind": t.kind.name(),
        "file": t.file,
        "line": t.line,
    })
}

fn mvp_id_from_target(t: &RelationTarget) -> String {
    format!("symbol:{}:{}:{}", t.file, t.name, t.line)
}

fn relation_for(
    relation_type: &str,
    direction: RelationDirection,
    target: &RelationTarget,
    evidence_id: &str,
) -> TypedRelation {
    TypedRelation {
        relation_type: relation_type.to_string(),
        direction,
        target_object_id: mvp_id_from_target(target),
        target_label: format!("{} ({})", target.name, target.kind.name()),
        evidence_ids: vec![evidence_id.to_string()],
    }
}

fn mvp_id(symbol: &ResolvedSymbol) -> String {
    format!("symbol:{}:{}:{}", symbol.file, symbol.name, symbol.line)
}

fn symbol_metadata_evidence(symbol: &ResolvedSymbol) -> EvidenceBlock {
    EvidenceBlock {
        id: "evidence:symbol_metadata".into(),
        kind: "symbol_metadata".into(),
        title: format!("Symbol metadata: {}", symbol.name),
        file: Some(symbol.file.clone()),
        line_range: Some(LineRange {
            start: symbol.line,
            end: symbol.line,
        }),
        source_tool_or_query: "CallGraphRepository::resolve".into(),
        confidence: Some(1.0),
        // Graph build time is not exposed through the explorer port yet.
        freshness: Some("unknown".into()),
    }
}

// ============================================================================
// Phase 3 — Quality lens view builders
// ============================================================================

/// Render a symbol's quality view: every issue at the symbol's
/// `file:line`. Returns an empty-state block when no issues match.
/// The function is total — a `None` quality repo degrades to the
/// same empty-state shape, so callers can wire it without knowing
/// whether a quality backend is connected.
pub fn build_symbol_quality_view(
    symbol: &ResolvedSymbol,
    quality: Option<&dyn QualityRepository>,
) -> ContextualView {
    let evidence_id = "evidence:symbol_quality".to_string();
    let mvp = mvp_id(symbol);
    let issues: Vec<QualityIssue> = quality
        .map(|q| q.issues_at_line(&symbol.file, symbol.line).unwrap_or_default())
        .unwrap_or_default();

    let relations: Vec<TypedRelation> = issues
        .iter()
        .map(|i| TypedRelation {
            relation_type: "FOUND_AT".to_string(),
            direction: RelationDirection::Incoming,
            target_object_id: format!("issue:{}", i.id),
            target_label: format!("{}: {} ({} L{})", i.severity, i.rule_id, i.file, i.line),
            evidence_ids: vec![evidence_id.clone()],
        })
        .collect();

    let blocks = vec![
        ViewBlock {
            id: "symbol_quality_identity".into(),
            title: "Quality".into(),
            body: json!({
                "file": symbol.file,
                "line": symbol.line,
                "issue_count": issues.len(),
            }),
        },
        ViewBlock {
            id: "symbol_quality_issues".into(),
            title: format!("Issues at this location ({})", issues.len()),
            body: json!({
                "count": issues.len(),
                "items": issues.iter().map(issue_summary).collect::<Vec<_>>(),
            }),
        },
    ];

    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "quality_finding".into(),
        title: format!("Quality findings at {}:{}", symbol.file, symbol.line),
        file: Some(symbol.file.clone()),
        line_range: Some(LineRange {
            start: symbol.line,
            end: symbol.line,
        }),
        source_tool_or_query: "QualityRepository::issues_at_line".into(),
        confidence: Some(1.0),
        // Quality data is point-in-time; freshness mirrors the file
        // heuristic used by source_file evidence so callers can compare.
        freshness: Some(if issues.is_empty() { "stale".into() } else { "fresh".into() }),
    }];

    ContextualView {
        object_id: mvp,
        view_id: "quality".into(),
        title: "Quality".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
    }
}

/// Render a file's quality view: every issue in the file, plus the
/// current quality gate. Degrades to an empty view when no quality
/// repo is wired.
pub fn build_file_quality_view(
    file_path: &str,
    quality: Option<&dyn QualityRepository>,
) -> ContextualView {
    let evidence_id = "evidence:file_quality".to_string();
    let mvp = format!("file:{file_path}");
    let issues: Vec<QualityIssue> = quality
        .map(|q| q.issues_for_file(file_path).unwrap_or_default())
        .unwrap_or_default();
    let gate = quality
        .map(|q| q.quality_gate().unwrap_or_default())
        .unwrap_or_default();

    let relations: Vec<TypedRelation> = issues
        .iter()
        .map(|i| TypedRelation {
            relation_type: "FOUND_IN".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("issue:{}", i.id),
            target_label: format!(
                "{}: {} (L{})",
                i.severity, i.rule_id, i.line
            ),
            evidence_ids: vec![evidence_id.clone()],
        })
        .collect();

    let blocks = vec![
        ViewBlock {
            id: "file_quality_identity".into(),
            title: "Quality".into(),
            body: json!({
                "path": file_path,
                "issue_count": issues.len(),
            }),
        },
        ViewBlock {
            id: "file_quality_issues".into(),
            title: format!("Issues in this file ({})", issues.len()),
            body: json!({
                "count": issues.len(),
                "items": issues.iter().map(issue_summary).collect::<Vec<_>>(),
            }),
        },
        ViewBlock {
            id: "file_quality_gate".into(),
            title: "Quality gate".into(),
            body: json!({
                "rating": gate.rating,
                "total_issues": gate.total_issues,
                "blockers": gate.blockers,
                "criticals": gate.criticals,
                "debt_minutes": gate.debt_minutes,
                "last_run": gate.last_run,
            }),
        },
    ];

    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "quality_finding".into(),
        title: format!("Quality findings in {}", file_path),
        file: Some(file_path.to_string()),
        line_range: None,
        source_tool_or_query: "QualityRepository::issues_for_file + quality_gate".into(),
        confidence: Some(1.0),
        freshness: Some(if issues.is_empty() { "stale".into() } else { "fresh".into() }),
    }];

    ContextualView {
        object_id: mvp,
        view_id: "quality".into(),
        title: "File quality".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
    }
}

/// Render a scope's quality view: every issue in the scope + the
/// current quality gate. Boundary-aware via `scope_contains_file` is
/// enforced by the adapter's SQL.
pub fn build_scope_quality_view(
    scope_path: &str,
    quality: Option<&dyn QualityRepository>,
) -> ContextualView {
    let evidence_id = "evidence:scope_quality".to_string();
    let mvp = format!("scope:{scope_path}");
    let issues: Vec<QualityIssue> = quality
        .map(|q| q.issues_for_scope(scope_path).unwrap_or_default())
        .unwrap_or_default();
    let gate = quality
        .map(|q| q.quality_gate().unwrap_or_default())
        .unwrap_or_default();

    let relations: Vec<TypedRelation> = issues
        .iter()
        .map(|i| TypedRelation {
            relation_type: "FOUND_IN".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("issue:{}", i.id),
            target_label: format!(
                "{}: {} ({} L{})",
                i.severity, i.rule_id, i.file, i.line
            ),
            evidence_ids: vec![evidence_id.clone()],
        })
        .collect();

    // Bucket issues by severity for the at-a-glance summary.
    let mut by_severity: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for i in &issues {
        *by_severity.entry(i.severity.clone()).or_insert(0) += 1;
    }
    let by_severity_json: serde_json::Value =
        serde_json::to_value(&by_severity).unwrap_or_else(|_| serde_json::json!({}));

    let blocks = vec![
        ViewBlock {
            id: "scope_quality_identity".into(),
            title: "Quality".into(),
            body: json!({
                "scope": scope_path,
                "issue_count": issues.len(),
                "by_severity": by_severity_json,
            }),
        },
        ViewBlock {
            id: "scope_quality_gate".into(),
            title: "Quality gate".into(),
            body: json!({
                "rating": gate.rating,
                "total_issues": gate.total_issues,
                "blockers": gate.blockers,
                "criticals": gate.criticals,
                "debt_minutes": gate.debt_minutes,
                "last_run": gate.last_run,
            }),
        },
        ViewBlock {
            id: "scope_quality_issues".into(),
            title: format!("Issues in this scope ({})", issues.len()),
            body: json!({
                "count": issues.len(),
                "items": issues.iter().map(issue_summary).collect::<Vec<_>>(),
            }),
        },
    ];

    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "quality_finding".into(),
        title: format!("Quality findings in scope {}", scope_path),
        file: None,
        line_range: None,
        source_tool_or_query: "QualityRepository::issues_for_scope + quality_gate".into(),
        confidence: Some(1.0),
        freshness: Some(if issues.is_empty() { "stale".into() } else { "fresh".into() }),
    }];

    ContextualView {
        object_id: mvp,
        view_id: "quality".into(),
        title: "Scope quality".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
    }
}

/// Build the detail view for a single quality issue. Includes a
/// `FOUND_IN` relation linking back to the file/scope that owns the
/// issue, plus an `APPLIES_TO` relation to the rule.
pub fn build_issue_detail(issue: &QualityIssue) -> ContextualView {
    let evidence_id = "evidence:issue_detail".to_string();
    let mvp = format!("issue:{}", issue.id);

    let relations = vec![
        TypedRelation {
            relation_type: "FOUND_IN".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("file:{}", issue.file),
            target_label: format!("{} (L{})", issue.file, issue.line),
            evidence_ids: vec![evidence_id.clone()],
        },
        TypedRelation {
            relation_type: "APPLIES_TO".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("rule:{}", issue.rule_id),
            target_label: issue.rule_id.clone(),
            evidence_ids: vec![evidence_id.clone()],
        },
    ];

    let blocks = vec![
        ViewBlock {
            id: "issue_identity".into(),
            title: "Issue".into(),
            body: json!({
                "id": issue.id,
                "rule_id": issue.rule_id,
                "severity": issue.severity,
                "category": issue.category,
                "status": issue.status,
            }),
        },
        ViewBlock {
            id: "issue_location".into(),
            title: "Location".into(),
            body: json!({
                "file": issue.file,
                "line": issue.line,
            }),
        },
        ViewBlock {
            id: "issue_message".into(),
            title: "Message".into(),
            body: json!({
                "message": issue.message,
            }),
        },
    ];

    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "quality_finding".into(),
        title: format!("Quality issue #{}", issue.id),
        file: Some(issue.file.clone()),
        line_range: Some(LineRange {
            start: issue.line,
            end: issue.line,
        }),
        source_tool_or_query: "QualityRepository::issue_by_id".into(),
        confidence: Some(1.0),
        freshness: Some(match issue.status.as_str() {
            "fixed" | "false_positive" => "stale".into(),
            _ => "fresh".into(),
        }),
    }];

    ContextualView {
        object_id: mvp,
        view_id: "overview".into(),
        title: "Issue".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
    }
}

/// Build the detail view for a single quality rule. Pulls the rule
/// summary from the repo (open count + description) and surfaces the
/// first 20 matching issues as `APPLIES_TO` relations. Degrades to a
/// `None` repo by treating the count as 0.
pub fn build_rule_detail(
    rule_id: &str,
    quality: Option<&dyn QualityRepository>,
) -> ContextualView {
    let evidence_id = "evidence:rule_detail".to_string();
    let mvp = format!("rule:{rule_id}");

    let summary: RuleSummary = quality
        .map(|q| q.rule_summary(rule_id).unwrap_or(RuleSummary {
            rule_id: rule_id.to_string(),
            description: rule_id.to_string(),
            open_count: 0,
        }))
        .unwrap_or_else(|| RuleSummary {
            rule_id: rule_id.to_string(),
            description: rule_id.to_string(),
            open_count: 0,
        });

    // Cap the relation count at 20 so a high-volume rule does not
    // bloat the view body. The full count is still in the block.
    const RELATION_CAP: usize = 20;
    // The repo does not expose "issues by rule" yet — the view
    // builder only has the rule id here. The open count lives in
    // `summary.open_count`; the related-issues list is empty until a
    // future `issues_for_rule` port method is added.
    let _ = quality;
    let related: Vec<QualityIssue> = Vec::new();

    let relations: Vec<TypedRelation> = related
        .iter()
        .take(RELATION_CAP)
        .map(|i| TypedRelation {
            relation_type: "APPLIES_TO".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("issue:{}", i.id),
            target_label: format!("{}: {} ({} L{})", i.severity, i.rule_id, i.file, i.line),
            evidence_ids: vec![evidence_id.clone()],
        })
        .collect();

    let blocks = vec![
        ViewBlock {
            id: "rule_identity".into(),
            title: "Rule".into(),
            body: json!({
                "rule_id": summary.rule_id,
                "description": summary.description,
                "open_count": summary.open_count,
            }),
        },
        ViewBlock {
            id: "rule_related".into(),
            title: format!("Related issues ({})", related.len()),
            body: json!({
                "count": related.len(),
                "items": related.iter().map(issue_summary).collect::<Vec<_>>(),
            }),
        },
    ];

    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "quality_finding".into(),
        title: format!("Quality rule {}", rule_id),
        file: None,
        line_range: None,
        source_tool_or_query: "QualityRepository::rule_summary".into(),
        confidence: Some(1.0),
        freshness: Some(if summary.open_count == 0 {
            "stale".into()
        } else {
            "fresh".into()
        }),
    }];

    ContextualView {
        object_id: mvp,
        view_id: "overview".into(),
        title: "Rule".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
    }
}

/// Shape one issue for inclusion in a view body. Used by every quality
/// view builder so the JSON shape stays consistent.
fn issue_summary(i: &QualityIssue) -> serde_json::Value {
    json!({
        "id": i.id,
        "rule_id": i.rule_id,
        "severity": i.severity,
        "category": i.category,
        "file": i.file,
        "line": i.line,
        "message": i.message,
        "status": i.status,
        "object_id": format!("issue:{}", i.id),
    })
}

// ============================================================================
// Phase 2 — File and Scope view builders
// ============================================================================

/// How a scope path is matched against a symbol's file. Boundary-aware:
/// `scope_contains("src", "src_extra.rs") == false` because we anchor on
/// the `/` separator, so prefixes do not bleed across module names.
pub fn scope_contains_file(scope: &str, file: &str) -> bool {
    file == scope || file.starts_with(&format!("{scope}/"))
}

/// File overview view: identity, line count, symbol count, kinds breakdown.
pub fn build_file_overview(
    symbols: &[ResolvedSymbol],
    file_path: &str,
    reader: &dyn SourceReader,
) -> ContextualView {
    let line_count = reader
        .read_lines(file_path, 1, u32::MAX)
        .map(|lines| {
            // `read_lines` already clamps end-of-file; the highest line
            // number returned is the line count (assuming the file is
            // densely numbered, which is what `read_lines` returns).
            lines.iter().map(|(n, _)| *n).max().unwrap_or(0) as usize
        })
        .unwrap_or(0);

    // Tally symbols per kind for the breakdown block. Convert to
    // `String` keys before serialising because JSON requires string keys
    // and `serde_json` will emit `null` for a BTreeMap with non-string
    // key types.
    let mut kinds: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for s in symbols {
        *kinds.entry(s.kind.name()).or_insert(0) += 1;
    }
    let kinds_string: std::collections::BTreeMap<String, usize> =
        kinds.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    let kinds_json: serde_json::Value = serde_json::to_value(&kinds_string)
        .unwrap_or_else(|_| serde_json::json!({}));

    let evidence_id = "evidence:file_overview".to_string();
    let evidence = vec![EvidenceBlock {
        id: evidence_id.clone(),
        kind: "file_overview".into(),
        title: format!("File overview: {}", file_path),
        file: Some(file_path.to_string()),
        line_range: Some(LineRange { start: 1, end: line_count as u32 }),
        source_tool_or_query: "FsSourceReader::read_lines + CallGraph::find_by_file".into(),
        confidence: Some(1.0),
        // A non-empty result means the file is reachable; the freshness
        // signal mirrors the `source_file` evidence block to stay consistent.
        freshness: Some(if line_count > 0 { "fresh".into() } else { "stale".into() }),
    }];

    let blocks = vec![
        ViewBlock {
            id: "file_identity".into(),
            title: "File".into(),
            body: json!({
                "path": file_path,
                "line_count": line_count,
                "symbol_count": symbols.len(),
            }),
        },
        ViewBlock {
            id: "kinds".into(),
            title: "Symbol kinds".into(),
            body: json!({
                "breakdown": kinds_json,
            }),
        },
    ];

    ContextualView {
        object_id: format!("file:{file_path}"),
        view_id: "overview".into(),
        title: "File overview".into(),
        blocks,
        relations: Vec::new(),
        evidence,
        findings: Vec::new(),
    }
}

/// File symbols view: every symbol in the file as a clickable `CONTAINS` relation.
pub fn build_file_symbols(
    symbols: &[ResolvedSymbol],
    file_path: &str,
) -> ContextualView {
    let evidence_id = "evidence:file_symbols".to_string();
    let relations: Vec<TypedRelation> = symbols
        .iter()
        .map(|s| TypedRelation {
            relation_type: "CONTAINS".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("symbol:{}:{}:{}", s.file, s.name, s.line),
            target_label: format!("{} ({}) at {}:{}", s.name, s.kind.name(), s.file, s.line),
            evidence_ids: vec![evidence_id.clone()],
        })
        .collect();

    let blocks = vec![ViewBlock {
        id: "symbols".into(),
        title: format!("Symbols in {} ({})", file_path, symbols.len()),
        body: json!({
            "count": symbols.len(),
            "items": symbols.iter().map(|s| json!({
                "name": s.name,
                "kind": s.kind.name(),
                "line": s.line,
                "object_id": format!("symbol:{}:{}:{}", s.file, s.name, s.line),
            })).collect::<Vec<_>>(),
        }),
    }];

    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "file_symbols".into(),
        title: format!("File symbols: {}", file_path),
        file: Some(file_path.to_string()),
        line_range: None,
        source_tool_or_query: "CallGraphRepository::find_symbols_by_file".into(),
        confidence: Some(1.0),
        // Graph build time is not exposed through the explorer port yet.
        freshness: Some("unknown".into()),
    }];

    ContextualView {
        object_id: format!("file:{file_path}"),
        view_id: "symbols".into(),
        title: "Symbols in file".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
    }
}

/// Scope overview: member counts, kinds breakdown, and a candidate flag.
pub fn build_scope_overview(
    scope_path: &str,
    files: &[String],
    symbols: &[ResolvedSymbol],
) -> ContextualView {
    let mut kinds: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for s in symbols {
        *kinds.entry(s.kind.name()).or_insert(0) += 1;
    }
    let kinds_string: std::collections::BTreeMap<String, usize> =
        kinds.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    let kinds_json: serde_json::Value = serde_json::to_value(&kinds_string)
        .unwrap_or_else(|_| serde_json::json!({}));

    let evidence_id = "evidence:scope_overview".to_string();
    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "scope_overview".into(),
        title: format!("Scope overview: {}", scope_path),
        file: None,
        line_range: None,
        source_tool_or_query: "CallGraph::modules + CallGraphRepository::find_symbols_by_file".into(),
        confidence: Some(1.0),
        freshness: Some("unknown".into()),
    }];

    let blocks = vec![
        ViewBlock {
            id: "scope_identity".into(),
            title: "Scope".into(),
            body: json!({
                "path": scope_path,
                "file_count": files.len(),
                "symbol_count": symbols.len(),
                "promotion_ready": false,
            }),
        },
        ViewBlock {
            id: "scope_kinds".into(),
            title: "Symbol kinds".into(),
            body: json!({
                "breakdown": kinds_json,
            }),
        },
        ViewBlock {
            id: "scope_files".into(),
            title: "Member files".into(),
            body: json!({
                "files": files,
            }),
        },
    ];

    ContextualView {
        object_id: format!("scope:{scope_path}"),
        view_id: "overview".into(),
        title: "Scope overview".into(),
        blocks,
        relations: Vec::new(),
        evidence,
        findings: Vec::new(),
    }
}

/// Scope dependencies: cross-scope CALLS/CALLED_BY relations, grouped by
/// target scope. Same-scope relations are filtered out — they are noise
/// for a module-candidate view.
pub fn build_scope_dependencies(
    scope_path: &str,
    repo: &dyn SymbolRepository,
) -> ContextualView {
    // 1. Collect the scope's member symbols via `all_symbols` and the
    //    boundary-aware membership test.
    let all = repo.all_symbols().unwrap_or_default();
    let mut member_files: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    let mut member_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let mut member_symbols: Vec<ResolvedSymbol> = Vec::new();
    for sym in all {
        if scope_contains_file(scope_path, &sym.file) {
            member_files.insert(sym.file.clone());
            member_ids.insert(sym.id.to_string());
            member_symbols.push(sym);
        }
    }

    // 2. For each member symbol, walk its callers + callees; keep only
    //    cross-scope relations and bucket them by target scope (the parent
    //    directory of the OTHER endpoint's file).
    #[derive(Default)]
    struct Bucket {
        outgoing_count: usize,
        incoming_count: usize,
    }
    let mut buckets: std::collections::BTreeMap<String, Bucket> =
        std::collections::BTreeMap::new();

    for sym in &member_symbols {
        for target in repo.callees(&sym.id) {
            if member_ids.contains(target.id.as_str()) {
                continue; // same-scope
            }
            let scope = other_scope(scope_path, &target.file);
            buckets.entry(scope).or_default().outgoing_count += 1;
        }
        for caller in repo.callers(&sym.id) {
            if member_ids.contains(caller.id.as_str()) {
                continue; // same-scope
            }
            let scope = other_scope(scope_path, &caller.file);
            buckets.entry(scope).or_default().incoming_count += 1;
        }
    }

    // 3. Shape the result.
    let evidence_id = "evidence:scope_dependencies".to_string();
    let entries: Vec<serde_json::Value> = buckets
        .into_iter()
        .map(|(other, b)| {
            json!({
                "scope": other,
                "outgoing_count": b.outgoing_count,
                "incoming_count": b.incoming_count,
            })
        })
        .collect();

    let blocks = vec![ViewBlock {
        id: "cross_scope".into(),
        title: format!("Cross-scope relations ({})", entries.len()),
        body: json!({
            "scope": scope_path,
            "file_count": member_files.len(),
            "symbol_count": member_symbols.len(),
            "entries": entries,
        }),
    }];

    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "scope_dependencies".into(),
        title: format!("Scope dependencies: {}", scope_path),
        file: None,
        line_range: None,
        source_tool_or_query: "CallGraph::callers + CallGraph::callees (cross-scope filter)".into(),
        confidence: Some(1.0),
        freshness: Some("unknown".into()),
    }];

    ContextualView {
        object_id: format!("scope:{scope_path}"),
        view_id: "dependencies".into(),
        title: "Scope dependencies".into(),
        blocks,
        relations: Vec::new(),
        evidence,
        findings: Vec::new(),
    }
}

/// Scope hotspots: top N (default 5) symbols in the scope by `fan_in`.
/// `symbols` is expected to be pre-sorted by the service — the view
/// builder just shapes the data.
pub fn build_scope_hotspots(
    scope_path: &str,
    symbols: &[ResolvedSymbol],
) -> ContextualView {
    let evidence_id = "evidence:scope_hotspots".to_string();
    let relations: Vec<TypedRelation> = symbols
        .iter()
        .map(|s| TypedRelation {
            relation_type: "HOTSPOT".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("symbol:{}:{}:{}", s.file, s.name, s.line),
            target_label: format!("{} ({} at {}:{})", s.name, s.kind.name(), s.file, s.line),
            evidence_ids: vec![evidence_id.clone()],
        })
        .collect();

    let blocks = vec![ViewBlock {
        id: "hotspots".into(),
        title: format!("Top hotspots ({})", symbols.len()),
        body: json!({
            "scope": scope_path,
            "count": symbols.len(),
            "items": symbols.iter().map(|s| json!({
                "name": s.name,
                "kind": s.kind.name(),
                "file": s.file,
                "line": s.line,
                "object_id": format!("symbol:{}:{}:{}", s.file, s.name, s.line),
            })).collect::<Vec<_>>(),
        }),
    }];

    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "scope_hotspots".into(),
        title: format!("Scope hotspots: {}", scope_path),
        file: None,
        line_range: None,
        source_tool_or_query: "CallGraph::fan_in (top-N filter)".into(),
        confidence: Some(1.0),
        freshness: Some("unknown".into()),
    }];

    ContextualView {
        object_id: format!("scope:{scope_path}"),
        view_id: "hotspots".into(),
        title: "Scope hotspots".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
    }
}

/// Bucket a foreign file under its scope (parent directory), excluding
/// the current scope.
fn other_scope(current_scope: &str, other_file: &str) -> String {
    let other = std::path::Path::new(other_file)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| other_file.to_string());
    if other == current_scope {
        // Should not happen — scope_contains_file prevents it. Fall
        // back to the file's literal path so the caller still sees
        // the relation.
        other_file.to_string()
    } else {
        other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ExplorerResult;
    use crate::ports::source_reader::SourceReader;
    use crate::ports::symbol_repository::{RelationTarget, ResolvedSymbol, SymbolRepository};
    use cognicode_core::domain::aggregates::SymbolId;
    use cognicode_core::domain::value_objects::SymbolKind;
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn make_resolved(file: &str, name: &str, line: u32, kind: SymbolKind) -> ResolvedSymbol {
        ResolvedSymbol {
            id: SymbolId::new(format!("{file}:{name}:{line}")),
            name: name.to_string(),
            kind,
            file: file.to_string(),
            line,
            signature: Some(format!("fn {name}() -> ()")),
        }
    }

    /// Hand-rolled mock repository — no mockall to keep the crate's
    /// dev-dependencies slim. Returns pre-baked answers keyed by SymbolId.
    struct MockRepo {
        symbols: HashMap<String, ResolvedSymbol>,
        callers: HashMap<String, Vec<RelationTarget>>,
        callees: HashMap<String, Vec<RelationTarget>>,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                symbols: HashMap::new(),
                callers: HashMap::new(),
                callees: HashMap::new(),
            }
        }

        fn with(&mut self, sym: ResolvedSymbol) -> &mut Self {
            self.symbols.insert(sym.id.to_string(), sym);
            self
        }

        fn with_caller(&mut self, owner: &str, target: ResolvedSymbol) -> &mut Self {
            self.callers
                .entry(owner.to_string())
                .or_default()
                .push(RelationTarget::from(&target));
            self
        }

        fn with_callee(&mut self, owner: &str, target: ResolvedSymbol) -> &mut Self {
            self.callees
                .entry(owner.to_string())
                .or_default()
                .push(RelationTarget::from(&target));
            self
        }
    }

    impl SymbolRepository for MockRepo {
        fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.symbols.get(id.as_str()).cloned())
        }
        fn callers(&self, id: &SymbolId) -> Vec<RelationTarget> {
            self.callers.get(id.as_str()).cloned().unwrap_or_default()
        }
        fn callees(&self, id: &SymbolId) -> Vec<RelationTarget> {
            self.callees.get(id.as_str()).cloned().unwrap_or_default()
        }
        fn fan_in(&self, id: &SymbolId) -> usize {
            self.callers(id).len()
        }
        fn fan_out(&self, id: &SymbolId) -> usize {
            self.callees(id).len()
        }
        // The mock is used by view-builder tests, not by spotter/search tests,
        // so find/aggregate are intentionally no-ops here.
        fn find_symbols_by_name(&self, _name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn find_symbols_by_file(&self, file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self
                .symbols
                .values()
                .filter(|s| s.file == file)
                .cloned()
                .collect())
        }
        fn module_list(&self) -> Vec<String> {
            let mut modules: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for s in self.symbols.values() {
                if let Some(parent) = std::path::Path::new(&s.file).parent() {
                    let p = parent.to_string_lossy().to_string();
                    if !p.is_empty() {
                        modules.insert(p);
                    }
                }
            }
            modules.into_iter().collect()
        }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.symbols.values().cloned().collect())
        }
        fn graph_stats(&self) -> crate::ports::symbol_repository::GraphStats {
            crate::ports::symbol_repository::GraphStats::default()
        }
    }

    struct MockReader {
        content: Mutex<HashMap<String, String>>,
    }

    impl MockReader {
        fn new(content: HashMap<String, String>) -> Self {
            Self {
                content: Mutex::new(content),
            }
        }
    }

    impl SourceReader for MockReader {
        fn read_source(&self, file: &str) -> ExplorerResult<String> {
            self.content
                .lock()
                .unwrap()
                .get(file)
                .cloned()
                .ok_or_else(|| crate::error::ExplorerError::SourceUnavailable {
                    file: file.to_string(),
                    object_id: file.to_string(),
                })
        }

        fn read_lines(
            &self,
            file: &str,
            start: u32,
            end: u32,
        ) -> ExplorerResult<Vec<(u32, String)>> {
            let content = self.read_source(file)?;
            Ok(content
                .lines()
                .enumerate()
                .map(|(i, l)| ((i + 1) as u32, l.to_string()))
                .filter(|(n, _)| *n >= start && *n <= end)
                .collect())
        }
    }

    #[test]
    fn overview_includes_signature_for_callable() {
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let mut repo = MockRepo::new();
        repo.with(sym.clone());

        let view = build_overview(&sym, &repo);

        assert_eq!(view.view_id, "overview");
        let ids: Vec<&str> = view.blocks.iter().map(|b| b.id.as_str()).collect();
        assert!(ids.contains(&"identity"));
        assert!(ids.contains(&"call_metrics"));
        assert!(ids.contains(&"signature"));
        assert_eq!(view.evidence.len(), 1);
        assert_eq!(view.evidence[0].kind, "symbol_metadata");
    }

    #[test]
    fn overview_omits_signature_for_type() {
        let sym = make_resolved("src/foo.rs", "Foo", 5, SymbolKind::Struct);
        let mut repo = MockRepo::new();
        repo.with(sym.clone());

        let view = build_overview(&sym, &repo);
        let ids: Vec<&str> = view.blocks.iter().map(|b| b.id.as_str()).collect();
        assert!(!ids.contains(&"signature"));
    }

    #[test]
    fn callgraph_populates_relations() {
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let caller = make_resolved("src/main.rs", "main", 1, SymbolKind::Function);
        let callee_a = make_resolved("src/baz.rs", "baz", 10, SymbolKind::Function);
        let callee_b = make_resolved("src/qux.rs", "qux", 20, SymbolKind::Function);

        let mut repo = MockRepo::new();
        repo.with(sym.clone())
            .with_caller(&sym.id.to_string(), caller.clone())
            .with_callee(&sym.id.to_string(), callee_a.clone())
            .with_callee(&sym.id.to_string(), callee_b.clone());

        let view = build_callgraph(&sym, &repo);
        assert_eq!(view.view_id, "call-graph");

        let incoming: Vec<_> = view
            .relations
            .iter()
            .filter(|r| r.direction == RelationDirection::Incoming)
            .collect();
        let outgoing: Vec<_> = view
            .relations
            .iter()
            .filter(|r| r.direction == RelationDirection::Outgoing)
            .collect();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].relation_type, "CALLED_BY");
        assert_eq!(incoming[0].target_object_id, "symbol:src/main.rs:main:1");
        assert_eq!(outgoing.len(), 2);
        assert!(outgoing.iter().all(|r| r.relation_type == "CALLS"));
    }

    #[test]
    fn callgraph_leaf_has_empty_callers_block() {
        let sym = make_resolved("src/foo.rs", "leaf", 1, SymbolKind::Function);
        let repo = MockRepo::new();
        let view = build_callgraph(&sym, &repo);

        let callers_block = view
            .blocks
            .iter()
            .find(|b| b.id == "callers")
            .expect("callers block");
        let body = &callers_block.body;
        assert_eq!(body["count"], 0);
        assert!(view.relations.is_empty());
    }

    #[test]
    fn source_view_reads_numbered_lines() {
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let mut content = HashMap::new();
        content.insert(
            "src/foo.rs".to_string(),
            (1..=50).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n"),
        );
        let reader = MockReader::new(content);

        let view = build_source(&sym, &reader);
        assert_eq!(view.view_id, "source");

        let block = &view.blocks[0];
        let lines = block.body["lines"].as_array().expect("lines array");
        // Window is [42-7, 42+8] = [35, 50], clamped at 50 → 16 lines.
        assert_eq!(lines.len(), 16);
        assert_eq!(lines[0]["line"], 35);
        assert_eq!(lines[0]["text"], "line 35");
    }

    // -----------------------------------------------------------------------
    // Phase 2 — File and Scope view builders
    // -----------------------------------------------------------------------

    fn make_reader_with_lines(file: &str, line_count: u32) -> MockReader {
        let mut content = HashMap::new();
        let body: String = (1..=line_count)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        content.insert(file.to_string(), body);
        MockReader::new(content)
    }

    #[test]
    fn file_overview_reports_symbol_count_and_kinds() {
        let syms = vec![
            make_resolved("src/main.rs", "alpha", 1, SymbolKind::Function),
            make_resolved("src/main.rs", "beta", 10, SymbolKind::Struct),
            make_resolved("src/main.rs", "gamma", 20, SymbolKind::Function),
        ];
        let reader = make_reader_with_lines("src/main.rs", 30);
        let view = build_file_overview(&syms, "src/main.rs", &reader);
        assert_eq!(view.view_id, "overview");
        let identity = view
            .blocks
            .iter()
            .find(|b| b.id == "file_identity")
            .expect("file_identity block");
        assert_eq!(identity.body["path"], "src/main.rs");
        assert_eq!(identity.body["line_count"], 30);
        assert_eq!(identity.body["symbol_count"], 3);
        let kinds = view
            .blocks
            .iter()
            .find(|b| b.id == "kinds")
            .expect("kinds block");
        // SymbolKind::name() returns lowercase: "function", "struct".
        assert_eq!(kinds.body["breakdown"]["function"], 2);
        assert_eq!(kinds.body["breakdown"]["struct"], 1);
        assert_eq!(view.evidence.len(), 1);
        assert_eq!(view.evidence[0].kind, "file_overview");
        assert_eq!(view.evidence[0].freshness.as_deref(), Some("fresh"));
    }

    #[test]
    fn file_overview_for_missing_file_marks_stale() {
        let syms = vec![];
        let reader = MockReader::new(HashMap::new());
        let view = build_file_overview(&syms, "src/missing.rs", &reader);
        assert_eq!(view.evidence[0].freshness.as_deref(), Some("stale"));
        assert_eq!(view.blocks[0].body["line_count"], 0);
    }

    #[test]
    fn file_symbols_emits_contains_relation_per_symbol() {
        let syms = vec![
            make_resolved("src/main.rs", "alpha", 1, SymbolKind::Function),
            make_resolved("src/main.rs", "beta", 10, SymbolKind::Struct),
        ];
        let view = build_file_symbols(&syms, "src/main.rs");
        assert_eq!(view.view_id, "symbols");
        assert_eq!(view.relations.len(), 2);
        for rel in &view.relations {
            assert_eq!(rel.relation_type, "CONTAINS");
            assert!(rel.target_object_id.starts_with("symbol:"));
        }
        let ids: Vec<&str> = view
            .relations
            .iter()
            .map(|r| r.target_object_id.as_str())
            .collect();
        assert!(ids.contains(&"symbol:src/main.rs:alpha:1"));
        assert!(ids.contains(&"symbol:src/main.rs:beta:10"));
    }

    #[test]
    fn scope_contains_file_anchors_on_separator() {
        assert!(scope_contains_file("src", "src/a.rs"));
        assert!(scope_contains_file("src", "src/foo/bar.rs"));
        assert!(scope_contains_file("src/foo", "src/foo/x.rs"));
        // No bleed across module-name boundary.
        assert!(!scope_contains_file("src", "src_extra.rs"));
        assert!(!scope_contains_file("src", "srcx.rs"));
        // Exact path is a member.
        assert!(scope_contains_file("src", "src"));
    }

    #[test]
    fn scope_overview_lists_files_and_kinds() {
        let syms = vec![
            make_resolved("src/foo/a.rs", "alpha", 1, SymbolKind::Function),
            make_resolved("src/foo/b.rs", "beta", 2, SymbolKind::Struct),
            make_resolved("src/foo/a.rs", "gamma", 3, SymbolKind::Function),
        ];
        let files = vec!["src/foo/a.rs".to_string(), "src/foo/b.rs".to_string()];
        let view = build_scope_overview("src/foo", &files, &syms);
        assert_eq!(view.view_id, "overview");
        let identity = view
            .blocks
            .iter()
            .find(|b| b.id == "scope_identity")
            .expect("scope_identity block");
        assert_eq!(identity.body["path"], "src/foo");
        assert_eq!(identity.body["file_count"], 2);
        assert_eq!(identity.body["symbol_count"], 3);
        assert_eq!(identity.body["promotion_ready"], false);
        let kinds = view
            .blocks
            .iter()
            .find(|b| b.id == "scope_kinds")
            .expect("kinds block");
        // SymbolKind::name() is lowercase.
        assert_eq!(kinds.body["breakdown"]["function"], 2);
        assert_eq!(kinds.body["breakdown"]["struct"], 1);
        let members = view
            .blocks
            .iter()
            .find(|b| b.id == "scope_files")
            .expect("scope_files block");
        let listed: Vec<&str> = members.body["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(listed, vec!["src/foo/a.rs", "src/foo/b.rs"]);
    }

    #[test]
    fn scope_dependencies_filters_same_scope_calls() {
        // A member of `src/foo` calls another member of `src/foo` (same-scope,
        // should be filtered out) AND a member of `src/bar` (cross-scope, kept).
        let mut repo = MockRepo::new();
        let a = make_resolved("src/foo/a.rs", "alpha", 1, SymbolKind::Function);
        let b = make_resolved("src/foo/b.rs", "beta", 2, SymbolKind::Function);
        let c = make_resolved("src/bar/c.rs", "gamma", 3, SymbolKind::Function);
        repo.with(a.clone())
            .with(b.clone())
            .with(c.clone())
            .with_callee(&a.id.to_string(), b.clone())  // same-scope, ignored
            .with_callee(&a.id.to_string(), c.clone()); // cross-scope, kept

        let view = build_scope_dependencies("src/foo", &repo);
        assert_eq!(view.view_id, "dependencies");
        let cross = &view.blocks[0].body["entries"];
        let entries = cross.as_array().expect("entries array");
        assert_eq!(entries.len(), 1, "only the cross-scope call should appear");
        assert_eq!(entries[0]["scope"], "src/bar");
        assert_eq!(entries[0]["outgoing_count"], 1);
        assert_eq!(entries[0]["incoming_count"], 0);
    }

    #[test]
    fn scope_dependencies_counts_incoming_separately() {
        // A `src/bar` symbol calls a `src/foo` member — this is an incoming
        // relation for the `src/foo` scope.
        let mut repo = MockRepo::new();
        let a = make_resolved("src/foo/a.rs", "alpha", 1, SymbolKind::Function);
        let c = make_resolved("src/bar/c.rs", "gamma", 3, SymbolKind::Function);
        repo.with(a.clone())
            .with(c.clone())
            .with_caller(&a.id.to_string(), c.clone());

        let view = build_scope_dependencies("src/foo", &repo);
        let entries = view.blocks[0].body["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["scope"], "src/bar");
        assert_eq!(entries[0]["incoming_count"], 1);
        assert_eq!(entries[0]["outgoing_count"], 0);
    }

    #[test]
    fn scope_dependencies_for_empty_scope_returns_empty_entries() {
        let repo = MockRepo::new();
        let view = build_scope_dependencies("src/empty", &repo);
        let entries = view.blocks[0].body["entries"].as_array().unwrap();
        assert!(entries.is_empty());
        assert_eq!(view.blocks[0].body["file_count"], 0);
        assert_eq!(view.blocks[0].body["symbol_count"], 0);
    }

    #[test]
    fn scope_hotspots_renders_top_n_as_relations() {
        let syms = vec![
            make_resolved("src/foo/a.rs", "alpha", 1, SymbolKind::Function),
            make_resolved("src/foo/b.rs", "beta", 2, SymbolKind::Function),
        ];
        let view = build_scope_hotspots("src/foo", &syms);
        assert_eq!(view.view_id, "hotspots");
        assert_eq!(view.relations.len(), 2);
        for rel in &view.relations {
            assert_eq!(rel.relation_type, "HOTSPOT");
        }
    }

    // -----------------------------------------------------------------------
    // Phase 3 — Quality view builders
    // -----------------------------------------------------------------------

    use crate::ports::quality_repository::{QualityGateSummary, QualityRepository};
    use std::collections::HashMap as StdHashMap;

    /// Hand-rolled mock quality repository. Returns pre-baked answers
    /// keyed by file / scope / id, so view-builder tests are fully
    /// deterministic. Counts on the empty paths return zero.
    struct MockQuality {
        by_file: StdHashMap<String, Vec<QualityIssue>>,
        by_scope: StdHashMap<String, Vec<QualityIssue>>,
        by_line: StdHashMap<(String, u32), Vec<QualityIssue>>,
        by_id: StdHashMap<i64, QualityIssue>,
        rules: StdHashMap<String, RuleSummary>,
        gate: QualityGateSummary,
        open_count: usize,
    }

    impl MockQuality {
        fn new() -> Self {
            Self {
                by_file: StdHashMap::new(),
                by_scope: StdHashMap::new(),
                by_line: StdHashMap::new(),
                by_id: StdHashMap::new(),
                rules: StdHashMap::new(),
                gate: QualityGateSummary::default(),
                open_count: 0,
            }
        }
        fn with_file(&mut self, file: &str, issues: Vec<QualityIssue>) -> &mut Self {
            self.by_file.insert(file.to_string(), issues);
            self
        }
        fn with_line(
            &mut self,
            file: &str,
            line: u32,
            issues: Vec<QualityIssue>,
        ) -> &mut Self {
            self.by_line.insert((file.to_string(), line), issues);
            self
        }
        fn with_scope(&mut self, scope: &str, issues: Vec<QualityIssue>) -> &mut Self {
            self.by_scope.insert(scope.to_string(), issues);
            self
        }
        fn with_rule(&mut self, summary: RuleSummary) -> &mut Self {
            self.rules.insert(summary.rule_id.clone(), summary);
            self
        }
        fn with_gate(&mut self, gate: QualityGateSummary) -> &mut Self {
            self.gate = gate;
            self
        }
    }

    impl QualityRepository for MockQuality {
        fn issues_for_file(&self, file: &str) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self.by_file.get(file).cloned().unwrap_or_default())
        }
        fn issues_for_scope(&self, scope: &str) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self.by_scope.get(scope).cloned().unwrap_or_default())
        }
        fn issues_at_line(&self, file: &str, line: u32) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self
                .by_line
                .get(&(file.to_string(), line))
                .cloned()
                .unwrap_or_default())
        }
        fn issue_by_id(&self, id: i64) -> ExplorerResult<Option<QualityIssue>> {
            Ok(self.by_id.get(&id).cloned())
        }
        fn rule_summary(&self, rule_id: &str) -> ExplorerResult<RuleSummary> {
            Ok(self
                .rules
                .get(rule_id)
                .cloned()
                .unwrap_or_else(|| RuleSummary {
                    rule_id: rule_id.to_string(),
                    description: rule_id.to_string(),
                    open_count: 0,
                }))
        }
        fn quality_gate(&self) -> ExplorerResult<QualityGateSummary> {
            Ok(self.gate.clone())
        }
        fn open_issues_count(&self) -> ExplorerResult<usize> {
            Ok(self.open_count)
        }
    }

    fn make_issue(id: i64, file: &str, line: u32, rule: &str, severity: &str) -> QualityIssue {
        QualityIssue {
            id,
            rule_id: rule.to_string(),
            severity: severity.to_string(),
            category: "CodeSmell".to_string(),
            file: file.to_string(),
            line,
            message: format!("test message for issue {id}"),
            status: "open".to_string(),
        }
    }

    #[test]
    fn symbol_quality_view_emits_finding_relations_for_matching_issues() {
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let mut q = MockQuality::new();
        q.with_line(
            "src/foo.rs",
            42,
            vec![
                make_issue(1, "src/foo.rs", 42, "rust:S100", "Blocker"),
                make_issue(2, "src/foo.rs", 42, "rust:S101", "Critical"),
            ],
        );
        let view = build_symbol_quality_view(&sym, Some(&q));
        assert_eq!(view.view_id, "quality");
        assert_eq!(view.relations.len(), 2);
        for rel in &view.relations {
            assert_eq!(rel.relation_type, "FOUND_AT");
        }
        // Issue ids appear in the relations.
        let ids: Vec<&str> = view
            .relations
            .iter()
            .map(|r| r.target_object_id.as_str())
            .collect();
        assert!(ids.contains(&"issue:1"));
        assert!(ids.contains(&"issue:2"));
        assert_eq!(view.evidence.len(), 1);
        assert_eq!(view.evidence[0].kind, "quality_finding");
        assert_eq!(view.evidence[0].source_tool_or_query, "QualityRepository::issues_at_line");
    }

    #[test]
    fn symbol_quality_view_with_none_repo_returns_empty_state() {
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let view = build_symbol_quality_view(&sym, None);
        assert_eq!(view.view_id, "quality");
        assert!(view.relations.is_empty(), "no relations when repo is None");
        assert!(!view.evidence.is_empty(), "evidence block still emitted");
        // Block at "symbol_quality_issues" must report 0.
        let block = view
            .blocks
            .iter()
            .find(|b| b.id == "symbol_quality_issues")
            .expect("issues block");
        assert_eq!(block.body["count"], 0);
    }

    #[test]
    fn file_quality_view_groups_issues_and_surfaces_gate() {
        let mut q = MockQuality::new();
        q.with_file(
            "src/main.rs",
            vec![
                make_issue(1, "src/main.rs", 5, "rust:S100", "Blocker"),
                make_issue(2, "src/main.rs", 12, "rust:S101", "Minor"),
            ],
        );
        q.with_gate(QualityGateSummary {
            rating: Some("B".into()),
            total_issues: 2,
            blockers: 1,
            criticals: 0,
            debt_minutes: 60,
            last_run: Some("2026-06-06T10:00:00Z".into()),
        });
        let view = build_file_quality_view("src/main.rs", Some(&q));
        assert_eq!(view.view_id, "quality");
        assert_eq!(view.relations.len(), 2);
        for rel in &view.relations {
            assert_eq!(rel.relation_type, "FOUND_IN");
        }
        let gate = view
            .blocks
            .iter()
            .find(|b| b.id == "file_quality_gate")
            .expect("gate block");
        assert_eq!(gate.body["rating"], "B");
        assert_eq!(gate.body["blockers"], 1);
    }

    #[test]
    fn scope_quality_view_buckets_issues_by_severity() {
        let mut q = MockQuality::new();
        q.with_scope(
            "src",
            vec![
                make_issue(1, "src/a.rs", 1, "rust:S100", "Blocker"),
                make_issue(2, "src/b.rs", 2, "rust:S101", "Blocker"),
                make_issue(3, "src/a.rs", 3, "rust:S102", "Minor"),
            ],
        );
        let view = build_scope_quality_view("src", Some(&q));
        assert_eq!(view.view_id, "quality");
        let identity = view
            .blocks
            .iter()
            .find(|b| b.id == "scope_quality_identity")
            .expect("identity block");
        assert_eq!(identity.body["by_severity"]["Blocker"], 2);
        assert_eq!(identity.body["by_severity"]["Minor"], 1);
        assert_eq!(view.relations.len(), 3);
    }

    #[test]
    fn issue_detail_emits_found_in_and_applies_to_relations() {
        let issue = make_issue(7, "src/foo.rs", 42, "rust:S100", "Blocker");
        let view = build_issue_detail(&issue);
        assert_eq!(view.view_id, "overview");
        assert_eq!(view.relations.len(), 2);
        let rel_types: Vec<&str> =
            view.relations.iter().map(|r| r.relation_type.as_str()).collect();
        assert!(rel_types.contains(&"FOUND_IN"));
        assert!(rel_types.contains(&"APPLIES_TO"));
        let found_in = view
            .relations
            .iter()
            .find(|r| r.relation_type == "FOUND_IN")
            .unwrap();
        assert_eq!(found_in.target_object_id, "file:src/foo.rs");
        let applies = view
            .relations
            .iter()
            .find(|r| r.relation_type == "APPLIES_TO")
            .unwrap();
        assert_eq!(applies.target_object_id, "rule:rust:S100");
    }

    #[test]
    fn issue_detail_marks_fixed_as_stale() {
        let mut issue = make_issue(7, "src/foo.rs", 42, "rust:S100", "Blocker");
        issue.status = "fixed".to_string();
        let view = build_issue_detail(&issue);
        assert_eq!(view.evidence[0].freshness.as_deref(), Some("stale"));
    }

    #[test]
    fn rule_detail_pulls_open_count_from_repo() {
        let mut q = MockQuality::new();
        q.with_rule(RuleSummary {
            rule_id: "rust:S100".to_string(),
            description: "Method names should comply with naming conventions".to_string(),
            open_count: 7,
        });
        let view = build_rule_detail("rust:S100", Some(&q));
        assert_eq!(view.view_id, "overview");
        let identity = view
            .blocks
            .iter()
            .find(|b| b.id == "rule_identity")
            .expect("identity block");
        assert_eq!(identity.body["open_count"], 7);
        assert_eq!(identity.body["description"], "Method names should comply with naming conventions");
        assert_eq!(view.evidence[0].freshness.as_deref(), Some("fresh"));
    }

    #[test]
    fn rule_detail_with_none_repo_returns_zero_state() {
        let view = build_rule_detail("rust:S100", None);
        let identity = view
            .blocks
            .iter()
            .find(|b| b.id == "rule_identity")
            .expect("identity block");
        assert_eq!(identity.body["open_count"], 0);
        assert_eq!(view.evidence[0].freshness.as_deref(), Some("stale"));
    }
}
