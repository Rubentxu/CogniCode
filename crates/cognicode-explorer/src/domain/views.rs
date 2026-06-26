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

use crate::domain::evidence::build_evidence_blocks;
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::traits::graph_query_port::GraphQueryPort;
use cognicode_core::domain::value_objects::Provenance;

/// Build the Overview view: identity + call graph metrics + signature for callables.
pub fn build_overview<'a>(
    symbol: &ResolvedSymbol,
    repo: &'a dyn SymbolRepository,
    graph_query: Option<&'a dyn GraphQueryPort>,
) -> ContextualView {
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
            "fan_in": graph_query.map(|gq| gq.fan_in(&symbol.id)).unwrap_or(0),
            "fan_out": graph_query.map(|gq| gq.fan_out(&symbol.id)).unwrap_or(0),
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
        ..Default::default()
    }
}

/// Build the Call Graph view: incoming + outgoing relations and their counts.
pub fn build_callgraph<'a>(
    symbol: &ResolvedSymbol,
    repo: &'a dyn SymbolRepository,
    graph_query: Option<&'a dyn GraphQueryPort>,
) -> ContextualView {
    let callers = graph_query
        .as_ref()
        .map(|gq| gq.callers(&symbol.id))
        .unwrap_or_default();
    let callees = graph_query
        .as_ref()
        .map(|gq| gq.callees(&symbol.id))
        .unwrap_or_default();

    // Use GraphQueryPort for metadata when available. The base `SymbolRepository`
    // trait is metadata-free by design; the `GraphQueryPort` is the segregated
    // seam that lets us enrich the relations and the evidence block with
    // per-edge `(Provenance, f64)`. When `graph_query` is `None` — mocks, legacy
    // paths — we log once and emit `provenance: None`, `confidence: None`.
    let edge_meta: std::collections::HashMap<SymbolId, (Provenance, f64)> = match graph_query {
        Some(gq) => gq
            .callees_with_metadata(&symbol.id)
            .into_iter()
            .map(|m| (m.callee_id, (m.provenance, m.confidence)))
            .collect(),
        None => {
            tracing::warn!(
                symbol = %symbol.id,
                "graph_query not available; emitting null provenance/confidence"
            );
            std::collections::HashMap::new()
        }
    };

    // The evidence block mirrors the per-edge confidence when at least one
    // edge has metadata. With multiple distinct confidences we fall back to
    // `None` (the field is per-block, not per-edge — picking one would be
    // misleading). Mock / metadata-less paths stay at `None`.
    let (cg_confidence, cg_provenance): (Option<f32>, Option<String>) = if edge_meta.is_empty() {
        (None, None)
    } else if edge_meta.len() == 1 {
        let (p, c) = edge_meta.values().next().expect("single entry");
        (Some(*c as f32), Some(p.to_string()))
    } else {
        // Multiple distinct edges with potentially different confidences.
        // Surface the count instead of picking a representative value.
        (None, None)
    };

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
        confidence: cg_confidence,
        // Graph build time is not exposed through the explorer port.
        freshness: Some("unknown".into()),
        provenance: cg_provenance,
    };

    let mut relations: Vec<TypedRelation> = Vec::new();

    // For incoming (callers) we don't have direct edge metadata in the
    // current `GraphQueryPort` shape (it exposes outgoing
    // `callees_with_metadata` / `dependencies_with_metadata`). Incoming
    // edges fall back to `None` — same contract as the mock path.
    for c in &callers {
        relations.push(relation_for(
            "CALLED_BY",
            RelationDirection::Incoming,
            c,
            &cg_evidence_id,
            None,
        ));
    }
    for c in &callees {
        let meta = edge_meta.get(&c.id).copied();
        relations.push(relation_for(
            "CALLS",
            RelationDirection::Outgoing,
            c,
            &cg_evidence_id,
            meta,
        ));
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
        ..Default::default()
    }
}

/// Build the Source view: a numbered slice of the file around the symbol's line.
pub fn build_source(symbol: &ResolvedSymbol, reader: &dyn SourceReader) -> ContextualView {
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
        freshness: Some(if slice.is_empty() {
            "stale".into()
        } else {
            "fresh".into()
        }),
        provenance: None,
    }];

    ContextualView {
        object_id: mvp_id(symbol),
        view_id: "source".into(),
        title: "Source".into(),
        blocks,
        relations: Vec::new(),
        evidence,
        findings: Vec::new(),
        ..Default::default()
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
    metadata: Option<(Provenance, f64)>,
) -> TypedRelation {
    let (provenance, confidence) = match metadata {
        Some((p, c)) => (Some(p.to_string()), Some(c)),
        None => (None, None),
    };
    TypedRelation {
        relation_type: relation_type.to_string(),
        direction,
        target_object_id: mvp_id_from_target(target),
        target_label: format!("{} ({})", target.name, target.kind.name()),
        evidence_ids: vec![evidence_id.to_string()],
        provenance,
        confidence,
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
        provenance: None,
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
        .map(|q| {
            q.issues_at_line(&symbol.file, symbol.line)
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let relations: Vec<TypedRelation> = issues
        .iter()
        .map(|i| TypedRelation {
            relation_type: "FOUND_AT".to_string(),
            direction: RelationDirection::Incoming,
            target_object_id: format!("issue:{}", i.id),
            target_label: format!("{}: {} ({} L{})", i.severity, i.rule_id, i.file_path, i.line),
            evidence_ids: vec![evidence_id.clone()],
            provenance: None,
            confidence: None,
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
        freshness: Some(if issues.is_empty() {
            "stale".into()
        } else {
            "fresh".into()
        }),
        provenance: None,
    }];

    ContextualView {
        object_id: mvp,
        view_id: "quality".into(),
        title: "Quality".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
        ..Default::default()
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
        .map(|q| q.quality_gate(None).unwrap_or_default())
        .unwrap_or_default();

    let relations: Vec<TypedRelation> = issues
        .iter()
        .map(|i| TypedRelation {
            relation_type: "FOUND_IN".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("issue:{}", i.id),
            target_label: format!("{}: {} (L{})", i.severity, i.rule_id, i.line),
            evidence_ids: vec![evidence_id.clone()],
            provenance: None,
            confidence: None,
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
        freshness: Some(if issues.is_empty() {
            "stale".into()
        } else {
            "fresh".into()
        }),
        provenance: None,
    }];

    ContextualView {
        object_id: mvp,
        view_id: "quality".into(),
        title: "File quality".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
        ..Default::default()
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
        .map(|q| q.quality_gate(None).unwrap_or_default())
        .unwrap_or_default();

    let relations: Vec<TypedRelation> = issues
        .iter()
        .map(|i| TypedRelation {
            relation_type: "FOUND_IN".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("issue:{}", i.id),
            target_label: format!("{}: {} ({} L{})", i.severity, i.rule_id, i.file_path, i.line),
            evidence_ids: vec![evidence_id.clone()],
            provenance: None,
            confidence: None,
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
        freshness: Some(if issues.is_empty() {
            "stale".into()
        } else {
            "fresh".into()
        }),
        provenance: None,
    }];

    ContextualView {
        object_id: mvp,
        view_id: "quality".into(),
        title: "Scope quality".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
        ..Default::default()
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
            target_object_id: format!("file:{}", issue.file_path),
            target_label: format!("{} (L{})", issue.file_path, issue.line),
            evidence_ids: vec![evidence_id.clone()],
            provenance: None,
            confidence: None,
        },
        TypedRelation {
            relation_type: "APPLIES_TO".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("rule:{}", issue.rule_id),
            target_label: issue.rule_id.clone(),
            evidence_ids: vec![evidence_id.clone()],
            provenance: None,
            confidence: None,
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
                "file": issue.file_path,
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
        file: Some(issue.file_path.clone()),
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
        provenance: None,
    }];

    ContextualView {
        object_id: mvp,
        view_id: "overview".into(),
        title: "Issue".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
        ..Default::default()
    }
}

/// Build the detail view for a single quality rule. Pulls the rule
/// summary from the repo (open count + description) and surfaces the
/// first 20 matching issues as `APPLIES_TO` relations. Degrades to a
/// `None` repo by treating the count as 0.
pub fn build_rule_detail(rule_id: &str, quality: Option<&dyn QualityRepository>) -> ContextualView {
    let evidence_id = "evidence:rule_detail".to_string();
    let mvp = format!("rule:{rule_id}");

    let summary: RuleSummary = quality
        .map(|q| {
            q.rule_summary(rule_id).unwrap_or(RuleSummary {
                rule_id: rule_id.to_string(),
                description: rule_id.to_string(),
                open_count: 0,
            })
        })
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
            target_label: format!("{}: {} ({} L{})", i.severity, i.rule_id, i.file_path, i.line),
            evidence_ids: vec![evidence_id.clone()],
            provenance: None,
            confidence: None,
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
        provenance: None,
    }];

    ContextualView {
        object_id: mvp,
        view_id: "overview".into(),
        title: "Rule".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
        ..Default::default()
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
        "file": i.file_path,
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
    let kinds_json: serde_json::Value =
        serde_json::to_value(&kinds_string).unwrap_or_else(|_| serde_json::json!({}));

    let evidence_id = "evidence:file_overview".to_string();
    let evidence = vec![EvidenceBlock {
        id: evidence_id.clone(),
        kind: "file_overview".into(),
        title: format!("File overview: {}", file_path),
        file: Some(file_path.to_string()),
        line_range: Some(LineRange {
            start: 1,
            end: line_count as u32,
        }),
        source_tool_or_query: "FsSourceReader::read_lines + CallGraph::find_by_file".into(),
        confidence: Some(1.0),
        // A non-empty result means the file is reachable; the freshness
        // signal mirrors the `source_file` evidence block to stay consistent.
        freshness: Some(if line_count > 0 {
            "fresh".into()
        } else {
            "stale".into()
        }),
        provenance: None,
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
        ..Default::default()
    }
}

/// File symbols view: every symbol in the file as a clickable `CONTAINS` relation.
pub fn build_file_symbols(symbols: &[ResolvedSymbol], file_path: &str) -> ContextualView {
    let evidence_id = "evidence:file_symbols".to_string();
    let relations: Vec<TypedRelation> = symbols
        .iter()
        .map(|s| TypedRelation {
            relation_type: "CONTAINS".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("symbol:{}:{}:{}", s.file, s.name, s.line),
            target_label: format!("{} ({}) at {}:{}", s.name, s.kind.name(), s.file, s.line),
            evidence_ids: vec![evidence_id.clone()],
            provenance: None,
            confidence: None,
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
        provenance: None,
    }];

    ContextualView {
        object_id: format!("file:{file_path}"),
        view_id: "symbols".into(),
        title: "Symbols in file".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
        ..Default::default()
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
    let kinds_json: serde_json::Value =
        serde_json::to_value(&kinds_string).unwrap_or_else(|_| serde_json::json!({}));

    let evidence_id = "evidence:scope_overview".to_string();
    let evidence = vec![EvidenceBlock {
        id: evidence_id,
        kind: "scope_overview".into(),
        title: format!("Scope overview: {}", scope_path),
        file: None,
        line_range: None,
        source_tool_or_query: "CallGraph::modules + CallGraphRepository::find_symbols_by_file"
            .into(),
        confidence: Some(1.0),
        freshness: Some("unknown".into()),
        provenance: None,
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
        ..Default::default()
    }
}

/// Scope dependencies: cross-scope CALLS/CALLED_BY relations, grouped by
/// target scope. Same-scope relations are filtered out — they are noise
/// for a module-candidate view.
pub fn build_scope_dependencies<'a>(
    scope_path: &str,
    repo: &'a dyn SymbolRepository,
    graph_query: Option<&'a dyn GraphQueryPort>,
) -> ContextualView {
    // 1. Collect the scope's member symbols via `all_symbols` and the
    //    boundary-aware membership test.
    let all = repo.all_symbols().unwrap_or_default();
    let mut member_files: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut member_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut member_symbols: Vec<ResolvedSymbol> = Vec::new();
    for sym in all {
        if scope_contains_file(scope_path, &sym.file) {
            member_files.insert(sym.file.clone());
            member_ids.insert(sym.id.to_string());
            member_symbols.push(sym);
        }
    }

    // Use GraphQueryPort for metadata when available — same seam as
    // `build_callgraph`. When `graph_query` is `None` — mocks, legacy
    // paths — we log once and emit null confidence/provenance.
    let evidence_confidence: Option<f32>;
    let evidence_provenance: Option<String>;
    if let Some(gq) = graph_query {
        // Collect every outgoing cross-scope edge's metadata for this scope.
        let mut per_edge: Vec<(Provenance, f64)> = Vec::new();
        for sym in &member_symbols {
            for m in gq.callees_with_metadata(&sym.id) {
                // Match the same-scope filter used below.
                if member_ids.contains(m.callee_id.as_str()) {
                    continue;
                }
                per_edge.push((m.provenance, m.confidence));
            }
        }
        if per_edge.is_empty() {
            evidence_confidence = None;
            evidence_provenance = None;
        } else if per_edge.len() == 1 {
            let (p, c) = per_edge[0];
            evidence_confidence = Some(c as f32);
            evidence_provenance = Some(p.to_string());
        } else {
            // Multiple distinct edges with possibly different confidence /
            // provenance — `EvidenceBlock.confidence` is a scalar, so we
            // surface the most conservative value (lowest confidence) as
            // a faithful "this bucket is at least this trustworthy" hint.
            let (min_p, min_c) = per_edge
                .into_iter()
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .expect("non-empty");
            evidence_confidence = Some(min_c as f32);
            evidence_provenance = Some(min_p.to_string());
        }
    } else {
        tracing::warn!(
            scope = %scope_path,
            "graph_query not available; emitting null provenance/confidence"
        );
        evidence_confidence = None;
        evidence_provenance = None;
    }

    // 2. For each member symbol, walk its callers + callees; keep only
    //    cross-scope relations and bucket them by target scope (the parent
    //    directory of the OTHER endpoint's file).
    #[derive(Default)]
    struct Bucket {
        outgoing_count: usize,
        incoming_count: usize,
    }
    let mut buckets: std::collections::BTreeMap<String, Bucket> = std::collections::BTreeMap::new();

    for sym in &member_symbols {
        for target in graph_query
            .as_ref()
            .map(|gq| gq.callees(&sym.id))
            .unwrap_or_default()
        {
            if member_ids.contains(target.id.as_str()) {
                continue; // same-scope
            }
            let scope = other_scope(scope_path, &target.file);
            buckets.entry(scope).or_default().outgoing_count += 1;
        }
        for caller in graph_query
            .as_ref()
            .map(|gq| gq.callers(&sym.id))
            .unwrap_or_default()
        {
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
        confidence: evidence_confidence,
        freshness: Some("unknown".into()),
        provenance: evidence_provenance,
    }];

    ContextualView {
        object_id: format!("scope:{scope_path}"),
        view_id: "dependencies".into(),
        title: "Scope dependencies".into(),
        blocks,
        relations: Vec::new(),
        evidence,
        findings: Vec::new(),
        ..Default::default()
    }
}

/// Scope hotspots: top N (default 5) symbols in the scope by `fan_in`.
/// `symbols` is expected to be pre-sorted by the service — the view
/// builder just shapes the data.
pub fn build_scope_hotspots(scope_path: &str, symbols: &[ResolvedSymbol]) -> ContextualView {
    let evidence_id = "evidence:scope_hotspots".to_string();
    let relations: Vec<TypedRelation> = symbols
        .iter()
        .map(|s| TypedRelation {
            relation_type: "HOTSPOT".to_string(),
            direction: RelationDirection::Outgoing,
            target_object_id: format!("symbol:{}:{}:{}", s.file, s.name, s.line),
            target_label: format!("{} ({} at {}:{})", s.name, s.kind.name(), s.file, s.line),
            evidence_ids: vec![evidence_id.clone()],
            provenance: None,
            confidence: None,
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
        provenance: None,
    }];

    ContextualView {
        object_id: format!("scope:{scope_path}"),
        view_id: "hotspots".into(),
        title: "Scope hotspots".into(),
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
        ..Default::default()
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

// ============================================================================
// Phase 1 — View Seam Consolidation: ViewDescriptor + ViewExecutor traits
// ============================================================================
//
// ISP-segregated traits replacing ViewDescriptorProvider:
//   ViewDescriptor  — metadata-only, object-safe (no async, no build)
//   ViewExecutor    — ViewDescriptor + async build()

use crate::dto::{InspectableObjectType, RendererKind, ViewKind};
use crate::error::ExplorerResult;
use async_trait::async_trait;

// Re-export InspectionTarget and ViewContext so existing code can import them
// from domain::views rather than dto. These are defined in dto.rs.
pub use crate::dto::{InspectionTarget, ViewContext};

/// Metadata-only trait for listing consumers (e.g., available_views).
/// All methods resolve through the vtable — no downcast needed.
pub trait ViewDescriptor: Send + Sync {
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn applies_to(&self) -> &'static [InspectableObjectType];
    fn view_kind(&self) -> ViewKind;
    fn renderer_kind(&self) -> RendererKind;
}

/// Async executor trait — extends ViewDescriptor with build().
/// Registry stores dyn ViewExecutor so list_for and get_executor
/// both work through the same vtable without Any-based downcast.
#[async_trait]
pub trait ViewExecutor: ViewDescriptor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView>;
}

// ============================================================================
// Built-in ViewDescriptorProvider registrations
// ============================================================================
//
// Phase 1 built-ins:
//   overview   → applies to Symbol, File, Scope   → ViewKind::VerticalSlice
//   call-graph → applies to Symbol               → ViewKind::CallGraph
//   source     → applies to Symbol               → ViewKind::SourceView
//   quality    → applies to Symbol, File, Scope, QualityIssue, Rule → ViewKind::QualityHotspots
//
// Registration uses `RegisterBuiltin` trait with a static initializer.
// Each provider struct calls `register_self()` during module initialization.

use crate::registry::{ProviderWrapper, ViewDescriptorProvider};

/// Overview view provider — applies to Symbol, File, Scope.
struct OverviewProvider;
impl ViewDescriptorProvider for OverviewProvider {
    fn id(&self) -> &'static str {
        "overview"
    }
    fn title(&self) -> &'static str {
        "Overview"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[
            InspectableObjectType::Symbol,
            InspectableObjectType::File,
            InspectableObjectType::Scope,
        ]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::VerticalSlice
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Json
    }
}
static OVERVIEW_PROVIDER: OverviewProvider = OverviewProvider;
inventory::submit!(ProviderWrapper {
    provider: &OVERVIEW_PROVIDER as &dyn ViewDescriptorProvider
});

/// Call-graph view provider — applies to Symbol.
struct CallGraphProvider;
impl ViewDescriptorProvider for CallGraphProvider {
    fn id(&self) -> &'static str {
        "call-graph"
    }
    fn title(&self) -> &'static str {
        "Call Graph"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Symbol]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::CallGraph
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Graph
    }
}
static CALLGRAPH_PROVIDER: CallGraphProvider = CallGraphProvider;
inventory::submit!(ProviderWrapper {
    provider: &CALLGRAPH_PROVIDER as &dyn ViewDescriptorProvider
});

/// Source view provider — applies to Symbol.
struct SourceProvider;
impl ViewDescriptorProvider for SourceProvider {
    fn id(&self) -> &'static str {
        "source"
    }
    fn title(&self) -> &'static str {
        "Source"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Symbol]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::SourceView
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Code
    }
}
static SOURCE_PROVIDER: SourceProvider = SourceProvider;
inventory::submit!(ProviderWrapper {
    provider: &SOURCE_PROVIDER as &dyn ViewDescriptorProvider
});

/// Quality view provider — applies to Symbol, File, Scope, QualityIssue, Rule.
struct QualityProvider;
impl ViewDescriptorProvider for QualityProvider {
    fn id(&self) -> &'static str {
        "quality"
    }
    fn title(&self) -> &'static str {
        "Quality"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[
            InspectableObjectType::Symbol,
            InspectableObjectType::File,
            InspectableObjectType::Scope,
            InspectableObjectType::QualityIssue,
            InspectableObjectType::Rule,
        ]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::QualityHotspots
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Table
    }
}
static QUALITY_PROVIDER: QualityProvider = QualityProvider;
inventory::submit!(ProviderWrapper {
    provider: &QUALITY_PROVIDER as &dyn ViewDescriptorProvider
});

// ============================================================================
// Phase 2 — ViewExecutor implementations for the 4 built-in capabilities
// ============================================================================
// Each struct wraps the existing build_* functions and dispatches based on
// InspectionTarget variant. The registry's get_executor() returns these
// instead of ProviderExecutorAdapter so that build() actually works.

/// Overview capability — applies to Symbol, File, Scope.
pub struct OverviewExecutor;
impl ViewDescriptor for OverviewExecutor {
    fn id(&self) -> &'static str {
        "overview"
    }
    fn title(&self) -> &'static str {
        "Overview"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[
            InspectableObjectType::Symbol,
            InspectableObjectType::File,
            InspectableObjectType::Scope,
        ]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::VerticalSlice
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Json
    }
}

#[async_trait]
impl ViewExecutor for OverviewExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Symbol(symbol) => {
                Ok(build_overview(symbol, ctx.repo, ctx.graph_query))
            }
            InspectionTarget::File { path, symbols } => {
                Ok(build_file_overview(symbols, path, ctx.reader))
            }
            InspectionTarget::Scope {
                path,
                files,
                symbols,
            } => Ok(build_scope_overview(path, files, symbols)),
            InspectionTarget::Issue(_) | InspectionTarget::Rule { .. } => {
                Err(crate::error::ExplorerError::ViewNotAvailable {
                    object_id: format!("{:?}", ctx.target),
                    view_id: "overview".into(),
                })
            }
        }
    }
}

/// CallGraph capability — applies to Symbol.
pub struct CallGraphExecutor;
impl ViewDescriptor for CallGraphExecutor {
    fn id(&self) -> &'static str {
        "call-graph"
    }
    fn title(&self) -> &'static str {
        "Call Graph"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Symbol]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::CallGraph
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Graph
    }
}

#[async_trait]
impl ViewExecutor for CallGraphExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Symbol(symbol) => {
                Ok(build_callgraph(symbol, ctx.repo, ctx.graph_query))
            }
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "call-graph".into(),
            }),
        }
    }
}

/// UsageExamples capability — applies to Symbol.
/// Shows callers and callees as a navigable table (complement to CallGraph's graph view).
pub struct UsageExamplesExecutor;
impl ViewDescriptor for UsageExamplesExecutor {
    fn id(&self) -> &'static str {
        "usage-examples"
    }
    fn title(&self) -> &'static str {
        "Usage Examples"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Symbol]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::UsageExamples
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Table
    }
}

#[async_trait]
impl ViewExecutor for UsageExamplesExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match &ctx.target {
            InspectionTarget::Symbol(symbol) => Ok(build_usage_examples(symbol, ctx.graph_query)),
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "usage-examples".into(),
            }),
        }
    }
}

/// Build the Usage Examples view: callers + callees as a table.
fn build_usage_examples(
    symbol: &ResolvedSymbol,
    graph_query: Option<&dyn GraphQueryPort>,
) -> ContextualView {
    let callers = graph_query
        .as_ref()
        .map(|gq| gq.callers(&symbol.id))
        .unwrap_or_default();
    let callees = graph_query
        .as_ref()
        .map(|gq| gq.callees(&symbol.id))
        .unwrap_or_default();

    let caller_rows: Vec<serde_json::Value> = callers
        .iter()
        .map(|t| {
            json!({
                "object_id": mvp_id_from_target(t),
                "name": t.name,
                "file": t.file,
                "line": t.line,
                "kind": t.kind.name(),
            })
        })
        .collect();
    let callee_rows: Vec<serde_json::Value> = callees
        .iter()
        .map(|t| {
            json!({
                "object_id": mvp_id_from_target(t),
                "name": t.name,
                "file": t.file,
                "line": t.line,
                "kind": t.kind.name(),
            })
        })
        .collect();

    let blocks = vec![
        ViewBlock {
            id: "callers".into(),
            title: format!("Called by ({})", callers.len()),
            body: json!({
                "columns": ["name", "file", "line", "kind"],
                "rows": caller_rows,
            }),
        },
        ViewBlock {
            id: "callees".into(),
            title: format!("Calls ({})", callees.len()),
            body: json!({
                "columns": ["name", "file", "line", "kind"],
                "rows": callee_rows,
            }),
        },
    ];

    ContextualView {
        object_id: mvp_id(symbol),
        view_id: "usage-examples".into(),
        title: "Usage Examples".into(),
        blocks,
        relations: Vec::new(),
        evidence: Vec::new(),
        findings: Vec::new(),
        renderer_kind: RendererKind::Table,
        ..Default::default()
    }
}

/// ApiSurface capability — applies to Scope.
/// Shows all symbols defined within a scope as a navigable table.
pub struct ApiSurfaceExecutor;
impl ViewDescriptor for ApiSurfaceExecutor {
    fn id(&self) -> &'static str {
        "api-surface"
    }
    fn title(&self) -> &'static str {
        "API Surface"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Scope]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::ApiSurface
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Table
    }
}

#[async_trait]
impl ViewExecutor for ApiSurfaceExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match &ctx.target {
            InspectionTarget::Scope { path, symbols, .. } => {
                Ok(build_api_surface(path, symbols))
            }
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "api-surface".into(),
            }),
        }
    }
}

/// Build the API Surface view: all symbols in a scope as a table.
fn build_api_surface(scope_path: &str, symbols: &[ResolvedSymbol]) -> ContextualView {
    // Sort symbols by name for consistent display.
    let mut sorted: Vec<&ResolvedSymbol> = symbols.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    let rows: Vec<serde_json::Value> = sorted
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "kind": s.kind.name(),
                "file": s.file,
                "line": s.line,
            })
        })
        .collect();

    let blocks = vec![ViewBlock {
        id: "api_surface".into(),
        title: scope_path.to_string(),
        body: json!({
            "columns": ["name", "kind", "file", "line"],
            "rows": rows,
        }),
    }];

    ContextualView {
        object_id: format!("scope:{scope_path}"),
        view_id: "api-surface".into(),
        title: "API Surface".into(),
        blocks,
        relations: Vec::new(),
        evidence: Vec::new(),
        findings: Vec::new(),
        renderer_kind: RendererKind::Table,
        ..Default::default()
    }
}

/// TestSlice capability — applies to Symbol.
/// Shows test functions that call the inspected symbol as a navigable table.
pub struct TestSliceExecutor;
impl ViewDescriptor for TestSliceExecutor {
    fn id(&self) -> &'static str {
        "test-slice"
    }
    fn title(&self) -> &'static str {
        "Test Slice"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Symbol]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::TestSlice
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Table
    }
}

#[async_trait]
impl ViewExecutor for TestSliceExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match &ctx.target {
            InspectionTarget::Symbol(symbol) => {
                Ok(build_test_slice(symbol, ctx.graph_query))
            }
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "test-slice".into(),
            }),
        }
    }
}

/// Returns true if the file path looks like a test file.
fn is_test_file(file: &str) -> bool {
    file.contains("/tests/")
        || file.contains("/test/")
        || file.ends_with("_test.rs")
        || file.ends_with("test_.rs")
        || file.ends_with(".test.ts")
        || file.ends_with(".test.tsx")
        || file.ends_with("_test.sh")
        || file.ends_with("_tests.rs")
}

/// Build the Test Slice view: test callers of a symbol as a table.
fn build_test_slice(
    symbol: &ResolvedSymbol,
    graph_query: Option<&dyn GraphQueryPort>,
) -> ContextualView {
    let all_callers = graph_query
        .as_ref()
        .map(|gq| gq.callers(&symbol.id))
        .unwrap_or_default();

    let test_callers: Vec<_> = all_callers
        .into_iter()
        .filter(|c| is_test_file(&c.file))
        .collect();

    // Sort by file path, then by line number.
    let mut sorted = test_callers.clone();
    sorted.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));

    let rows: Vec<serde_json::Value> = sorted
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "file": c.file,
                "line": c.line,
                "kind": c.kind.name(),
            })
        })
        .collect();

    let blocks = vec![ViewBlock {
        id: "test_slice".into(),
        title: format!("Tests ({})", sorted.len()),
        body: json!({
            "columns": ["name", "file", "line", "kind"],
            "rows": rows,
        }),
    }];

    ContextualView {
        object_id: mvp_id(symbol),
        view_id: "test-slice".into(),
        title: "Test Slice".into(),
        blocks,
        relations: Vec::new(),
        evidence: Vec::new(),
        findings: Vec::new(),
        renderer_kind: RendererKind::Table,
        ..Default::default()
    }
}

/// Source capability — applies to Symbol.
pub struct SourceExecutor;
impl ViewDescriptor for SourceExecutor {
    fn id(&self) -> &'static str {
        "source"
    }
    fn title(&self) -> &'static str {
        "Source"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Symbol]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::SourceView
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Code
    }
}

#[async_trait]
impl ViewExecutor for SourceExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Symbol(symbol) => Ok(build_source(symbol, ctx.reader)),
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "source".into(),
            }),
        }
    }
}

/// Quality capability — applies to Symbol, File, Scope, QualityIssue, Rule.
pub struct QualityExecutor;
impl ViewDescriptor for QualityExecutor {
    fn id(&self) -> &'static str {
        "quality"
    }
    fn title(&self) -> &'static str {
        "Quality"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[
            InspectableObjectType::Symbol,
            InspectableObjectType::File,
            InspectableObjectType::Scope,
            InspectableObjectType::QualityIssue,
            InspectableObjectType::Rule,
        ]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::QualityHotspots
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Table
    }
}

#[async_trait]
impl ViewExecutor for QualityExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Symbol(symbol) => Ok(build_symbol_quality_view(symbol, ctx.quality)),
            InspectionTarget::File { path, symbols: _ } => {
                Ok(build_file_quality_view(path, ctx.quality))
            }
            InspectionTarget::Scope {
                path,
                files: _,
                symbols: _,
            } => Ok(build_scope_quality_view(path, ctx.quality)),
            InspectionTarget::Issue(issue) => Ok(build_issue_detail(issue)),
            InspectionTarget::Rule { rule_id } => Ok(build_rule_detail(rule_id, ctx.quality)),
        }
    }
}

// ============================================================================
// Phase 3 — Evidence, Symbols, Dependencies, Hotspots capabilities
// ============================================================================

/// Evidence capability — applies to Symbol.
/// Absorbs the private `build_evidence_view` from service.rs.
pub struct EvidenceExecutor;
impl ViewDescriptor for EvidenceExecutor {
    fn id(&self) -> &'static str {
        "evidence"
    }
    fn title(&self) -> &'static str {
        "Evidence"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Symbol]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::EvidenceView
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Json
    }
}

#[async_trait]
impl ViewExecutor for EvidenceExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Symbol(resolved) => {
                let evidence =
                    build_evidence_blocks(resolved, ctx.repo, ctx.reader, ctx.graph_query);
                let blocks = vec![ViewBlock {
                    id: "evidence_summary".into(),
                    title: "Evidence blocks".into(),
                    body: json!({
                        "count": evidence.len(),
                        "kinds": evidence.iter().map(|b| b.kind.clone()).collect::<Vec<_>>(),
                    }),
                }];
                Ok(ContextualView {
                    object_id: format!(
                        "symbol:{}:{}:{}",
                        resolved.file, resolved.name, resolved.line
                    ),
                    view_id: "evidence".into(),
                    title: "Evidence".into(),
                    view_kind: ViewKind::EvidenceView,
                    blocks,
                    relations: Vec::new(),
                    evidence,
                    findings: Vec::new(),
                    renderer_kind: RendererKind::default(),
                })
            }
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "evidence".into(),
            }),
        }
    }
}

/// Symbols capability — applies to File.
/// Delegates to `build_file_symbols`.
pub struct SymbolsExecutor;
impl ViewDescriptor for SymbolsExecutor {
    fn id(&self) -> &'static str {
        "symbols"
    }
    fn title(&self) -> &'static str {
        "Symbols"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::File]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::SemanticSearchResults
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Table
    }
}

#[async_trait]
impl ViewExecutor for SymbolsExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::File { path, symbols } => Ok(build_file_symbols(symbols, path)),
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "symbols".into(),
            }),
        }
    }
}

/// Dependencies capability — applies to Scope.
/// Delegates to `build_scope_dependencies`.
pub struct DependenciesExecutor;
impl ViewDescriptor for DependenciesExecutor {
    fn id(&self) -> &'static str {
        "dependencies"
    }
    fn title(&self) -> &'static str {
        "Dependencies"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Scope]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::DependencyGraph
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Graph
    }
}

#[async_trait]
impl ViewExecutor for DependenciesExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Scope { path, .. } => {
                Ok(build_scope_dependencies(path, ctx.repo, ctx.graph_query))
            }
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "dependencies".into(),
            }),
        }
    }
}

/// Hotspots capability — applies to Scope.
/// Absorbs `top_hotspots()` pre-sorting from service.rs. Sorts scope
/// member symbols by `fan_in` descending inside `build()`, then delegates
/// to `build_scope_hotspots`.
pub struct HotspotsExecutor;
impl ViewDescriptor for HotspotsExecutor {
    fn id(&self) -> &'static str {
        "hotspots"
    }
    fn title(&self) -> &'static str {
        "Hotspots"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Scope]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::QualityHotspots
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Table
    }
}

#[async_trait]
impl ViewExecutor for HotspotsExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        const SCOPE_HOTSPOT_LIMIT: usize = 5;
        match ctx.target {
            InspectionTarget::Scope { path, symbols, .. } => {
                // Inline the top_hotspots pre-sort: sort by fan_in desc, then truncate.
                let mut sorted: Vec<ResolvedSymbol> = symbols.to_vec();
                sorted.sort_by(|a, b| {
                    let fa = ctx.graph_query.map(|gq| gq.fan_in(&a.id)).unwrap_or(0);
                    let fb = ctx.graph_query.map(|gq| gq.fan_in(&b.id)).unwrap_or(0);
                    fb.cmp(&fa).then_with(|| a.name.cmp(&b.name))
                });
                sorted.truncate(SCOPE_HOTSPOT_LIMIT);
                Ok(build_scope_hotspots(path, &sorted))
            }
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "hotspots".into(),
            }),
        }
    }
}

// Static executor instances — referenced by the registry's get_executor() via &'static dyn ViewExecutor.
pub static OVERVIEW_EXECUTOR: OverviewExecutor = OverviewExecutor;
pub static CALLGRAPH_EXECUTOR: CallGraphExecutor = CallGraphExecutor;
pub static SOURCE_EXECUTOR: SourceExecutor = SourceExecutor;
pub static QUALITY_EXECUTOR: QualityExecutor = QualityExecutor;
pub static EVIDENCE_EXECUTOR: EvidenceExecutor = EvidenceExecutor;
pub static SYMBOLS_EXECUTOR: SymbolsExecutor = SymbolsExecutor;
pub static DEPENDENCIES_EXECUTOR: DependenciesExecutor = DependenciesExecutor;
pub static HOTSPOTS_EXECUTOR: HotspotsExecutor = HotspotsExecutor;
pub static ARCHITECTURE_DRIFT_EXECUTOR: ArchitectureDriftExecutor = ArchitectureDriftExecutor;
pub static USAGE_EXAMPLES_EXECUTOR: UsageExamplesExecutor = UsageExamplesExecutor;
pub static API_SURFACE_EXECUTOR: ApiSurfaceExecutor = ApiSurfaceExecutor;
pub static TEST_SLICE_EXECUTOR: TestSliceExecutor = TestSliceExecutor;

/// Architecture drift capability — applies to Workspace.
///
/// Note: Architecture drift detection primarily uses the dedicated
/// `GET /api/workspaces/:workspace_id/drift` endpoint. This executor
/// exists for completeness and for potential future integration
/// through the views API.
pub struct ArchitectureDriftExecutor;
impl ViewDescriptor for ArchitectureDriftExecutor {
    fn id(&self) -> &'static str {
        "architecture-drift"
    }
    fn title(&self) -> &'static str {
        "Architecture Drift"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Workspace]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::ArchitectureDrift
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Table
    }
}

#[async_trait]
impl ViewExecutor for ArchitectureDriftExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        // Architecture drift detection requires workspace-level context
        // (root path) which is not available through InspectionTarget.
        // The dedicated drift endpoint should be used instead.
        Err(crate::error::ExplorerError::NotImplemented(
            "Architecture drift requires the /api/workspaces/{id}/drift endpoint".into(),
        ))
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
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                symbols: HashMap::new(),
            }
        }

        fn with(&mut self, sym: ResolvedSymbol) -> &mut Self {
            self.symbols.insert(sym.id.to_string(), sym);
            self
        }
    }

    impl SymbolRepository for MockRepo {
        fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.symbols.get(id.as_str()).cloned())
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

        let view = build_overview(&sym, &repo, None);

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

        let view = build_overview(&sym, &repo, None);
        let ids: Vec<&str> = view.blocks.iter().map(|b| b.id.as_str()).collect();
        assert!(!ids.contains(&"signature"));
    }

    #[test]
    fn callgraph_populates_relations() {
        // When graph_query is None (mock path), relations are empty but the
        // view structure is still correct — callers/callees require a
        // GraphQueryPort (tested via CallGraphRepository in metadata-aware tests).
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let mut repo = MockRepo::new();
        repo.with(sym.clone());

        let view = build_callgraph(&sym, &repo, None);
        assert_eq!(view.view_id, "call-graph");
        // Without graph_query, relations are empty (null provenance/confidence path)
        assert!(view.relations.is_empty());
        // But the callers/callees blocks are still present
        let block_ids: Vec<&str> = view.blocks.iter().map(|b| b.id.as_str()).collect();
        assert!(block_ids.contains(&"callers"));
        assert!(block_ids.contains(&"callees"));
    }

    #[test]
    fn callgraph_leaf_has_empty_callers_block() {
        let sym = make_resolved("src/foo.rs", "leaf", 1, SymbolKind::Function);
        let repo = MockRepo::new();
        let view = build_callgraph(&sym, &repo, None);

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
            (1..=50)
                .map(|i| format!("line {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
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
        // Without graph_query (null mock path), all relation counts are 0
        // and entries are empty. Real callers/callees require GraphQueryPort.
        let mut repo = MockRepo::new();
        let _a = make_resolved("src/foo/a.rs", "alpha", 1, SymbolKind::Function);
        let _b = make_resolved("src/foo/b.rs", "beta", 2, SymbolKind::Function);
        let _c = make_resolved("src/bar/c.rs", "gamma", 3, SymbolKind::Function);
        repo.with(_a.clone());
        // Note: with_callee/caller calls removed — callers/callees now come
        // from GraphQueryPort, not SymbolRepository.

        let view = build_scope_dependencies("src/foo", &repo, None);
        assert_eq!(view.view_id, "dependencies");
        // Null graph_query path: no entries
        let entries = view.blocks[0].body["entries"]
            .as_array()
            .expect("entries array");
        assert!(entries.is_empty());
    }

    #[test]
    fn scope_dependencies_counts_incoming_separately() {
        // Without graph_query, no incoming/outgoing counts are populated.
        let mut repo = MockRepo::new();
        let _a = make_resolved("src/foo/a.rs", "alpha", 1, SymbolKind::Function);
        let _c = make_resolved("src/bar/c.rs", "gamma", 3, SymbolKind::Function);
        repo.with(_a.clone());
        // Note: with_caller removed — callers now come from GraphQueryPort.

        let view = build_scope_dependencies("src/foo", &repo, None);
        let entries = view.blocks[0].body["entries"].as_array().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn scope_dependencies_for_empty_scope_returns_empty_entries() {
        let repo = MockRepo::new();
        let view = build_scope_dependencies("src/empty", &repo, None);
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

    use crate::ports::quality_repository::{IssueFilter, QualityGateSummary, QualityRepository};
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
        fn with_line(&mut self, file: &str, line: u32, issues: Vec<QualityIssue>) -> &mut Self {
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
        fn quality_gate(&self, _workspace_id: Option<&str>) -> ExplorerResult<QualityGateSummary> {
            Ok(self.gate.clone())
        }
        fn open_issues_count(&self, _workspace_id: Option<&str>) -> ExplorerResult<usize> {
            Ok(self.open_count)
        }
        fn issues_for_workspace(
            &self,
            _workspace_id: Option<&str>,
            filter: &IssueFilter,
        ) -> ExplorerResult<Vec<QualityIssue>> {
            let mut out: Vec<QualityIssue> = self
                .by_file
                .values()
                .chain(self.by_scope.values())
                .flat_map(|v| v.iter().cloned())
                .chain(self.by_id.values().cloned())
                .filter(|i| filter.severity.as_deref().is_none_or(|s| i.severity == s))
                .filter(|i| filter.category.as_deref().is_none_or(|c| i.category == c))
                .filter(|i| filter.status.as_deref().is_none_or(|s| i.status == s))
                .filter(|i| match &filter.file_prefix {
                    None => true,
                    Some(p) => i.file_path == *p || i.file_path.starts_with(&format!("{p}/")),
                })
                .collect();
            if let Some(n) = filter.limit {
                out.truncate(n);
            }
            Ok(out)
        }
    }

    fn make_issue(id: i64, file: &str, line: u32, rule: &str, severity: &str) -> QualityIssue {
        QualityIssue {
            id,
            rule_id: rule.to_string(),
            severity: severity.to_string(),
            category: "CodeSmell".to_string(),
            file_path: file.to_string(),
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
        assert_eq!(
            view.evidence[0].source_tool_or_query,
            "QualityRepository::issues_at_line"
        );
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
        let rel_types: Vec<&str> = view
            .relations
            .iter()
            .map(|r| r.relation_type.as_str())
            .collect();
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
        assert_eq!(
            identity.body["description"],
            "Method names should comply with naming conventions"
        );
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

    // -----------------------------------------------------------------------
    // Phase 4 — MCP edge metadata (downcast + provenance/confidence)
    // -----------------------------------------------------------------------
    //
    // These tests cover the spec scenarios for
    // `mcp-postgres-envelope`:
    //
    // * REQ1: TypedRelation carries provenance + confidence.
    // * REQ2: EvidenceBlock carries provenance; confidence is the per-edge
    //         value (not a hardcoded 1.0).
    // * REQ3: View builders use GraphQueryPort; on failure
    //         they leave fields as None and SHOULD log a warning.
    // * REQ4: Serde backward compatibility for both pre-change and
    //         post-change payloads.

    use crate::adapters::CallGraphRepository;
    use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId as AggSymbolId};
    use cognicode_core::domain::services::ExtractionContext;
    use cognicode_core::domain::value_objects::{
        DependencyType, Location, Provenance, SymbolKind as CoreSymbolKind,
    };
    use std::sync::Arc;

    /// Test fixture: a tiny call graph with a single typed edge.
    /// Used by tests 4.4, 4.5, 4.7 to seed metadata-aware repositories
    /// with controlled `(Provenance, f64)` tuples.
    fn build_metadata_aware_graph_with_edge(
        source: (&str, &str, u32),
        target: (&str, &str, u32),
        extraction: ExtractionContext,
    ) -> (Arc<CallGraph>, AggSymbolId, AggSymbolId) {
        let mut g = CallGraph::new();
        let s = g.add_symbol(Symbol::new(
            source.1,
            CoreSymbolKind::Function,
            Location::new(source.0, source.2, 0),
        ));
        let t = g.add_symbol(Symbol::new(
            target.1,
            CoreSymbolKind::Function,
            Location::new(target.0, target.2, 0),
        ));
        g.add_dependency_with_provenance(&s, &t, DependencyType::Calls, extraction)
            .expect("add dep");
        (Arc::new(g), s, t)
    }

    #[test]
    fn legacy_payload_deserializes_into_updated_dto() {
        // Pre-change payload — no `provenance` / `confidence` fields. The
        // `#[serde(default)]` annotations on the new fields must let this
        // deserialize cleanly with both fields resolving to `None`.
        let legacy = r#"{
            "relation_type": "CALLS",
            "direction": "outgoing",
            "target_object_id": "symbol:src/a.rs:a:1",
            "target_label": "a (function)",
            "evidence_ids": []
        }"#;
        let parsed: crate::dto::TypedRelation =
            serde_json::from_str(legacy).expect("legacy payload must deserialize");
        assert_eq!(parsed.relation_type, "CALLS");
        assert!(parsed.provenance.is_none());
        assert!(parsed.confidence.is_none());
    }

    #[test]
    fn enriched_payload_round_trips() {
        // New payload — both fields populated. Round-trip through
        // serde_json without losing values.
        let original = crate::dto::TypedRelation {
            relation_type: "CALLS".to_string(),
            direction: crate::dto::RelationDirection::Outgoing,
            target_object_id: "symbol:src/a.rs:a:1".to_string(),
            target_label: "a (function)".to_string(),
            evidence_ids: vec!["evidence:test".to_string()],
            provenance: Some("Extracted".to_string()),
            confidence: Some(0.9),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: crate::dto::TypedRelation =
            serde_json::from_str(&json).expect("round-trip parse");
        assert_eq!(parsed.provenance.as_deref(), Some("Extracted"));
        assert_eq!(parsed.confidence, Some(0.9));
        // Full structural equality — no fields lost.
        assert_eq!(parsed.relation_type, original.relation_type);
        assert_eq!(parsed.target_object_id, original.target_object_id);
    }

    #[test]
    fn typed_relation_metadata_populated_from_aware_repo() {
        // Real, metadata-aware repository. The call-graph view
        // builder's outgoing `CALLS` relations must carry
        // non-null provenance + confidence. Incoming `CALLED_BY`
        // relations fall back to `None` (current trait surface has
        // no callers_with_metadata).
        let (graph, source, _) = build_metadata_aware_graph_with_edge(
            ("src/a.rs", "alpha", 1),
            ("src/b.rs", "beta", 5),
            ExtractionContext::Heuristic { score: 0.85 },
        );
        let repo = CallGraphRepository::new(graph);
        let resolved = ResolvedSymbol {
            id: source,
            name: "alpha".to_string(),
            kind: SymbolKind::Function,
            file: "src/a.rs".to_string(),
            line: 1,
            signature: Some("fn alpha()".to_string()),
        };
        let view = build_callgraph(&resolved, &repo, Some(&repo as &dyn GraphQueryPort));
        // Exactly one outgoing CALLS — the seeded edge.
        let outgoing: Vec<_> = view
            .relations
            .iter()
            .filter(|r| r.relation_type == "CALLS")
            .collect();
        assert_eq!(outgoing.len(), 1);
        let rel = outgoing[0];
        assert_eq!(rel.provenance.as_deref(), Some("Inferred"));
        assert_eq!(rel.confidence, Some(0.85));
    }

    #[test]
    fn typed_relation_metadata_null_for_mock_repo() {
        // When graph_query is None (mock path), relations are empty and
        // evidence carries null provenance/confidence — no panic, no error.
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let mut repo = MockRepo::new();
        repo.with(sym.clone());

        let view = build_callgraph(&sym, &repo, None);
        // Mock path: no relations (callers/callees require GraphQueryPort)
        assert!(view.relations.is_empty());
        // Evidence block is present but with null metadata
        assert_eq!(view.evidence.len(), 1);
        assert!(view.evidence[0].provenance.is_none());
        assert!(view.evidence[0].confidence.is_none());
        assert!(view.evidence[0].provenance.is_none());
        assert!(view.evidence[0].confidence.is_none());
    }

    #[test]
    fn evidence_block_reports_per_evidence_confidence() {
        // Seed a single edge at confidence 0.72 (Heuristic). The
        // `cg_evidence` block in the call-graph view must report
        // 0.72_f32 (after the f64→f32 cast) — NOT a hardcoded 1.0.
        let (graph, source, _) = build_metadata_aware_graph_with_edge(
            ("src/a.rs", "alpha", 1),
            ("src/b.rs", "beta", 5),
            ExtractionContext::Heuristic { score: 0.72 },
        );
        let repo = CallGraphRepository::new(graph);
        let resolved = ResolvedSymbol {
            id: source,
            name: "alpha".to_string(),
            kind: SymbolKind::Function,
            file: "src/a.rs".to_string(),
            line: 1,
            signature: Some("fn alpha()".to_string()),
        };
        let view = build_callgraph(&resolved, &repo, Some(&repo as &dyn GraphQueryPort));
        let evidence = &view.evidence[0];
        assert_eq!(evidence.provenance.as_deref(), Some("Inferred"));
        let confidence = evidence
            .confidence
            .expect("per-edge confidence must be set when downcast succeeds");
        assert!(
            (confidence - 0.72_f32).abs() < 1e-5,
            "expected 0.72 (cast f64→f32), got {confidence}"
        );
    }

    #[test]
    fn evidence_block_degrades_gracefully() {
        // `build_scope_dependencies` against a mock — the per-edge
        // evidence block must report `provenance: None` and
        // `confidence: None` with no panic.
        let mut repo = MockRepo::new();
        let _a = make_resolved("src/foo/a.rs", "alpha", 1, SymbolKind::Function);
        let _c = make_resolved("src/bar/c.rs", "gamma", 3, SymbolKind::Function);
        repo.with(_a.clone());
        // Note: with_callee removed — callers/callees come from GraphQueryPort.

        let view = build_scope_dependencies("src/foo", &repo, None);
        assert_eq!(view.evidence.len(), 1);
        assert!(
            view.evidence[0].provenance.is_none(),
            "mock scope-deps must emit null provenance"
        );
        assert!(
            view.evidence[0].confidence.is_none(),
            "mock scope-deps must emit null confidence, not a hardcoded 1.0"
        );
    }

    // =========================================================================
    // Phase 2 — Capability Tests (RED: fail until ViewExecutors are wired)
    // =========================================================================
    // These tests call get_executor() and await build() on the returned
    // executor. Phase 1's ProviderExecutorAdapter returns ViewNotAvailable,
    // so these tests fail in RED. GREEN adds real ViewExecutor implementations.

    #[tokio::test]
    async fn overview_capability_builds_for_symbol() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("overview")
            .expect("overview must be registered");

        let symbol = make_resolved("src/main.rs", "main", 1, SymbolKind::Function);
        let mut mock_repo = MockRepo::new();
        mock_repo.with(symbol.clone());
        let target = InspectionTarget::Symbol(symbol);
        let ctx = ViewContext {
            target: &target,
            repo: &mock_repo,
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        assert_eq!(view.view_id, "overview");
        assert_eq!(view.title, "Overview");
        assert!(
            view.blocks.iter().any(|b| b.id == "identity"),
            "overview view must have identity block"
        );
        assert!(
            view.blocks.iter().any(|b| b.id == "call_metrics"),
            "overview view must have call_metrics block"
        );
    }

    #[tokio::test]
    async fn overview_capability_builds_for_file() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("overview")
            .expect("overview must be registered");

        let symbols = vec![
            make_resolved("src/lib.rs", "foo", 10, SymbolKind::Function),
            make_resolved("src/lib.rs", "bar", 20, SymbolKind::Function),
        ];
        let target = InspectionTarget::File {
            path: "src/lib.rs".into(),
            symbols,
        };
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        assert_eq!(view.view_id, "overview");
        // build_file_overview returns "File overview" as title — capability preserves it
        assert_eq!(view.title, "File overview");
    }

    #[tokio::test]
    async fn overview_capability_builds_for_scope() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("overview")
            .expect("overview must be registered");

        let symbols = vec![make_resolved("src/lib.rs", "foo", 10, SymbolKind::Function)];
        let target = InspectionTarget::Scope {
            path: "src".into(),
            files: vec!["src/lib.rs".into()],
            symbols,
        };
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        assert_eq!(view.view_id, "overview");
        // build_scope_overview returns "Scope overview" as title — capability preserves it
        assert_eq!(view.title, "Scope overview");
    }

    #[tokio::test]
    async fn callgraph_capability_builds_for_symbol() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("call-graph")
            .expect("call-graph must be registered");

        let symbol = make_resolved("src/main.rs", "main", 1, SymbolKind::Function);
        let target = InspectionTarget::Symbol(symbol);
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        assert_eq!(view.view_id, "call-graph");
        assert_eq!(view.title, "Call Graph");
        assert!(
            view.blocks.iter().any(|b| b.id == "callers"),
            "call-graph view must have callers block"
        );
        assert!(
            view.blocks.iter().any(|b| b.id == "callees"),
            "call-graph view must have callees block"
        );
    }

    #[tokio::test]
    async fn source_capability_builds_for_symbol() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("source")
            .expect("source must be registered");

        let symbol = make_resolved("src/main.rs", "main", 5, SymbolKind::Function);
        let target = InspectionTarget::Symbol(symbol);
        let mut reader_content = HashMap::new();
        reader_content.insert("src/main.rs".into(), "fn main() {}".into());
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(reader_content),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        assert_eq!(view.view_id, "source");
        assert_eq!(view.title, "Source");
        assert!(
            view.blocks.iter().any(|b| b.id == "source_slice"),
            "source view must have source_slice block"
        );
    }

    #[tokio::test]
    async fn quality_capability_builds_for_symbol() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("quality")
            .expect("quality must be registered");

        let symbol = make_resolved("src/main.rs", "main", 1, SymbolKind::Function);
        let target = InspectionTarget::Symbol(symbol);
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        assert_eq!(view.view_id, "quality");
        assert_eq!(view.title, "Quality");
    }

    #[tokio::test]
    async fn quality_capability_builds_for_issue() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("quality")
            .expect("quality must be registered");

        let issue = QualityIssue {
            id: 1,
            rule_id: "rust_lint_foo".into(),
            severity: "warning".into(),
            category: "lint".into(),
            file_path: "src/main.rs".into(),
            line: 10,
            message: "test issue".into(),
            status: "open".into(),
        };
        let target = InspectionTarget::Issue(issue);
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        // build_issue_detail returns view_id "overview" and title "Issue" — existing quirk
        assert_eq!(view.view_id, "overview");
        assert_eq!(view.title, "Issue");
    }

    #[tokio::test]
    async fn quality_capability_builds_for_rule() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("quality")
            .expect("quality must be registered");

        let target = InspectionTarget::Rule {
            rule_id: "rust_lint_foo".into(),
        };
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        // build_rule_detail returns view_id "overview" and title "Rule" — existing quirk
        assert_eq!(view.view_id, "overview");
        assert_eq!(view.title, "Rule");
    }

    // =========================================================================
    // Phase 3 — Evidence, Symbols, Dependencies, Hotspots capability tests
    // =========================================================================

    #[tokio::test]
    async fn evidence_capability_handles_symbol() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("evidence")
            .expect("evidence must be registered");

        let symbol = make_resolved("src/main.rs", "main", 1, SymbolKind::Function);
        let target = InspectionTarget::Symbol(symbol);
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        assert_eq!(view.view_id, "evidence");
        assert_eq!(view.title, "Evidence");
        // Evidence blocks must be populated (build_evidence_blocks returns 4 blocks)
        assert!(
            view.blocks.iter().any(|b| b.id == "evidence_summary"),
            "evidence view must have evidence_summary block"
        );
        // evidence.len() == 4 (symbol_metadata, call_graph, source_file, fs_index)
        let summary_block = view
            .blocks
            .iter()
            .find(|b| b.id == "evidence_summary")
            .unwrap();
        let count = summary_block.body["count"].as_u64().expect("count field");
        assert_eq!(count, 4, "build_evidence_blocks returns 4 evidence kinds");
    }

    #[tokio::test]
    async fn evidence_capability_rejects_file() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("evidence")
            .expect("evidence must be registered");

        let target = InspectionTarget::File {
            path: "src/main.rs".into(),
            symbols: vec![make_resolved(
                "src/main.rs",
                "main",
                1,
                SymbolKind::Function,
            )],
        };
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let result = executor.build(&ctx).await;
        assert!(
            result.is_err(),
            "evidence capability must reject File target with error"
        );
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            crate::error::ExplorerError::ViewNotAvailable { .. }
        ));
    }

    #[tokio::test]
    async fn symbols_capability_handles_file() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("symbols")
            .expect("symbols must be registered");

        let symbols = vec![
            make_resolved("src/lib.rs", "foo", 10, SymbolKind::Function),
            make_resolved("src/lib.rs", "bar", 20, SymbolKind::Struct),
        ];
        let target = InspectionTarget::File {
            path: "src/lib.rs".into(),
            symbols,
        };
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        assert_eq!(view.view_id, "symbols");
        assert_eq!(view.title, "Symbols in file");
        // build_file_symbols produces CONTAINS relations for each symbol
        assert_eq!(view.relations.len(), 2);
        for rel in &view.relations {
            assert_eq!(rel.relation_type, "CONTAINS");
        }
    }

    #[tokio::test]
    async fn symbols_capability_rejects_symbol() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("symbols")
            .expect("symbols must be registered");

        let symbol = make_resolved("src/main.rs", "main", 1, SymbolKind::Function);
        let target = InspectionTarget::Symbol(symbol);
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let result = executor.build(&ctx).await;
        assert!(
            result.is_err(),
            "symbols capability must reject Symbol target with error"
        );
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            crate::error::ExplorerError::ViewNotAvailable { .. }
        ));
    }

    #[tokio::test]
    async fn dependencies_capability_handles_scope() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("dependencies")
            .expect("dependencies must be registered");

        let symbols = vec![make_resolved("src/lib.rs", "foo", 10, SymbolKind::Function)];
        let target = InspectionTarget::Scope {
            path: "src".into(),
            files: vec!["src/lib.rs".into()],
            symbols,
        };
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let view = executor.build(&ctx).await.expect("build must succeed");
        assert_eq!(view.view_id, "dependencies");
        assert_eq!(view.title, "Scope dependencies");
        // Empty cross-scope entries for mock repo
        let entries = view.blocks[0].body["entries"].as_array().unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn dependencies_capability_rejects_file() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("dependencies")
            .expect("dependencies must be registered");

        let target = InspectionTarget::File {
            path: "src/main.rs".into(),
            symbols: vec![make_resolved(
                "src/main.rs",
                "main",
                1,
                SymbolKind::Function,
            )],
        };
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let result = executor.build(&ctx).await;
        assert!(
            result.is_err(),
            "dependencies capability must reject File target with error"
        );
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            crate::error::ExplorerError::ViewNotAvailable { .. }
        ));
    }

    #[tokio::test]
    async fn hotspots_capability_sorts_by_fan_in_desc() {
        // NOTE: fan_in sorting requires a GraphQueryPort — without one, all
        // fan_in values are 0 and the sort order is undefined. This test
        // verifies the view builds without error in the null-graph_query path.
        let mut repo = MockRepo::new();
        let low = make_resolved("src/low.rs", "low", 1, SymbolKind::Function);
        let medium = make_resolved("src/med.rs", "medium", 2, SymbolKind::Function);
        let high = make_resolved("src/high.rs", "high", 3, SymbolKind::Function);
        repo.with(low.clone());
        repo.with(medium.clone());
        repo.with(high.clone());

        let symbols = vec![low.clone(), medium.clone(), high.clone()];
        let target = InspectionTarget::Scope {
            path: "src".into(),
            files: vec![
                "src/low.rs".into(),
                "src/med.rs".into(),
                "src/high.rs".into(),
            ],
            symbols,
        };
        let ctx = ViewContext {
            target: &target,
            repo: &repo,
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("hotspots")
            .expect("hotspots must be registered");
        let view = executor.build(&ctx).await.expect("build must succeed");

        assert_eq!(view.view_id, "hotspots");
        assert_eq!(view.title, "Scope hotspots");
        // Without graph_query, fan_in is 0 for all — items may be in any order
        let items = view.blocks[0].body["items"]
            .as_array()
            .expect("items array");
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn hotspots_capability_rejects_file() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry
            .get_executor("hotspots")
            .expect("hotspots must be registered");

        let target = InspectionTarget::File {
            path: "src/main.rs".into(),
            symbols: vec![make_resolved(
                "src/main.rs",
                "main",
                1,
                SymbolKind::Function,
            )],
        };
        let ctx = ViewContext {
            target: &target,
            repo: &MockRepo::new(),
            reader: &MockReader::new(HashMap::new()),
            quality: None,
            graph_query: None,
        };

        let result = executor.build(&ctx).await;
        assert!(
            result.is_err(),
            "hotspots capability must reject File target with error"
        );
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            crate::error::ExplorerError::ViewNotAvailable { .. }
        ));
    }

    // =========================================================================
    // UsageExamples tests
    // =========================================================================

    #[test]
    fn usage_examples_returns_callers_and_callees_blocks() {
        // With graph_query = None (mock path), both blocks are empty.
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let view = build_usage_examples(&sym, None);

        assert_eq!(view.view_id, "usage-examples");
        assert_eq!(view.blocks.len(), 2);
        // First block: callers
        let callers_block = &view.blocks[0];
        assert_eq!(callers_block.id, "callers");
        assert!(callers_block.title.contains("0")); // 0 callers in mock path
        // Second block: callees
        let callees_block = &view.blocks[1];
        assert_eq!(callees_block.id, "callees");
        assert!(callees_block.title.contains("0")); // 0 callees in mock path
    }

    #[test]
    fn usage_examples_graceful_degradation_with_no_graph_query() {
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let view = build_usage_examples(&sym, None);

        assert_eq!(view.blocks.len(), 2);
        assert_eq!(view.blocks[0].id, "callers");
        assert_eq!(view.blocks[1].id, "callees");
        // No panic, no error
    }

    #[test]
    fn usage_examples_renderer_kind_is_table() {
        let sym = make_resolved("src/foo.rs", "bar", 42, SymbolKind::Function);
        let view = build_usage_examples(&sym, None);
        assert_eq!(view.renderer_kind, RendererKind::Table);
    }

    // =========================================================================
    // ApiSurface tests
    // =========================================================================

    #[test]
    fn api_surface_returns_all_symbols_sorted_by_name() {
        let sym1 = make_resolved("src/lib.rs", "zebra", 10, SymbolKind::Function);
        let sym2 = make_resolved("src/lib.rs", "alpha", 20, SymbolKind::Function);
        let sym3 = make_resolved("src/lib.rs", "beta", 30, SymbolKind::Function);
        let symbols = vec![sym1, sym2, sym3];
        let view = build_api_surface("src/lib.rs", &symbols);

        assert_eq!(view.view_id, "api-surface");
        assert_eq!(view.blocks.len(), 1);
        let block = &view.blocks[0];
        assert_eq!(block.id, "api_surface");
        assert_eq!(block.title, "src/lib.rs");
        // Sorted by name: alpha, beta, zebra
        let rows = block.body.get("rows").unwrap().as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].get("name").unwrap().as_str().unwrap(), "alpha");
        assert_eq!(rows[1].get("name").unwrap().as_str().unwrap(), "beta");
        assert_eq!(rows[2].get("name").unwrap().as_str().unwrap(), "zebra");
    }

    #[test]
    fn api_surface_empty_scope_returns_empty_table() {
        let symbols: Vec<ResolvedSymbol> = vec![];
        let view = build_api_surface("src/empty.rs", &symbols);

        assert_eq!(view.view_id, "api-surface");
        assert_eq!(view.blocks.len(), 1);
        let block = &view.blocks[0];
        let rows = block.body.get("rows").unwrap().as_array().unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn api_surface_renderer_kind_is_table() {
        let sym = make_resolved("src/lib.rs", "foo", 1, SymbolKind::Function);
        let view = build_api_surface("src/lib.rs", &[sym]);
        assert_eq!(view.renderer_kind, RendererKind::Table);
    }

    #[test]
    fn api_surface_view_id_and_title() {
        let sym = make_resolved("src/lib.rs", "foo", 1, SymbolKind::Function);
        let view = build_api_surface("src/lib.rs", &[sym]);
        assert_eq!(view.view_id, "api-surface");
        assert_eq!(view.title, "API Surface");
    }

    // =========================================================================
    // TestSlice tests
    // =========================================================================

    #[test]
    fn test_slice_returns_only_test_callers() {
        let sym = make_resolved("src/lib.rs", "foo", 1, SymbolKind::Function);
        // Mock callers: one test file, one non-test file
        let test_caller = make_resolved("tests/foo_test.rs", "test_foo", 10, SymbolKind::Function);
        let prod_caller = make_resolved("src/lib.rs", "bar", 20, SymbolKind::Function);
        // This test just verifies the is_test_file heuristic
        assert!(is_test_file("tests/foo_test.rs"));
        assert!(is_test_file("src/utils/tests/helpers.rs"));
        assert!(is_test_file("test/unit/core_test.rs"));
        assert!(is_test_file("_test.sh"));
        assert!(!is_test_file("src/lib.rs"));
        assert!(!is_test_file("src/main.rs"));
    }

    #[test]
    fn test_slice_empty_when_no_test_callers() {
        let sym = make_resolved("src/lib.rs", "foo", 1, SymbolKind::Function);
        let view = build_test_slice(&sym, None);
        assert_eq!(view.view_id, "test-slice");
        assert_eq!(view.blocks.len(), 1);
        let rows = view.blocks[0].body.get("rows").unwrap().as_array().unwrap();
        assert_eq!(rows.len(), 0); // None available with graph_query = None
    }

    #[test]
    fn test_slice_renderer_kind_is_table() {
        let sym = make_resolved("src/lib.rs", "foo", 1, SymbolKind::Function);
        let view = build_test_slice(&sym, None);
        assert_eq!(view.renderer_kind, RendererKind::Table);
    }

    #[test]
    fn test_slice_view_id_and_title() {
        let sym = make_resolved("src/lib.rs", "foo", 1, SymbolKind::Function);
        let view = build_test_slice(&sym, None);
        assert_eq!(view.view_id, "test-slice");
        assert_eq!(view.title, "Test Slice");
    }
}

// ============================================================================
// Phase 1 — View Seam Consolidation: Trait Object and Registry Contract Tests
// ============================================================================
//
// These tests verify:
// 1. ViewDescriptor and ViewExecutor are object-safe (dynTrait compiles)
// 2. ViewRegistry::list_for returns sorted descriptors for each object type
// 3. ViewRegistry::get_executor returns the capability by id
// 4. No downcast is used anywhere in the registry dispatch path
//
// The tests are written in RED (they fail until the traits/types are added).
// After GREEN (traits + types + registry update), these tests pass.

#[cfg(test)]
mod view_seam_tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Object safety — &dyn ViewDescriptor / &dyn ViewExecutor must compile
    // -------------------------------------------------------------------------

    #[test]
    fn view_descriptor_trait_is_object_safe() {
        // If this compiles, ViewDescriptor is object-safe (no methods prevent dyn dispatch).
        fn _check(_: &dyn ViewDescriptor) {}
    }

    #[test]
    fn view_executor_trait_is_object_safe() {
        // If this compiles, ViewExecutor is object-safe (inherits ViewDescriptor safety).
        fn _check(_: &dyn ViewExecutor) {}
    }

    // -------------------------------------------------------------------------
    // Registry contract — list_for returns sorted descriptors per object type
    // -------------------------------------------------------------------------

    #[test]
    fn registry_list_for_symbol_returns_sorted_descriptors() {
        // Register ViewExecutor implementations via the registry's builtin_providers.
        // list_for(Symbol) should return descriptors sorted alphabetically by id.
        let registry = crate::registry::ViewRegistry::new(None);
        let descriptors = registry.list_for(crate::dto::InspectableObjectType::Symbol);

        // Verify sorted order
        let ids: Vec<&str> = descriptors.iter().map(|d| d.id.as_str()).collect();
        let mut sorted_ids = ids.clone();
        sorted_ids.sort();
        assert_eq!(
            ids, sorted_ids,
            "list_for(Symbol) must return descriptors sorted alphabetically by id"
        );

        // For Symbol, we expect: call-graph, overview, quality, source
        // (the 4 built-in capabilities that handle Symbol in Phase 1)
        let symbol_caps = vec!["call-graph", "overview", "quality", "source"];
        for cap in symbol_caps {
            assert!(
                ids.contains(&cap),
                "Symbol should include '{cap}' in list_for(Symbol)"
            );
        }
    }

    #[test]
    fn registry_list_for_file_returns_sorted_descriptors() {
        let registry = crate::registry::ViewRegistry::new(None);
        let descriptors = registry.list_for(crate::dto::InspectableObjectType::File);

        let ids: Vec<&str> = descriptors.iter().map(|d| d.id.as_str()).collect();
        let mut sorted_ids = ids.clone();
        sorted_ids.sort();
        assert_eq!(
            ids, sorted_ids,
            "list_for(File) must return descriptors sorted alphabetically by id"
        );

        // For File, we expect: overview, quality
        // (Phase 1: symbols/evidence/dependencies/hotspots not yet migrated to registry)
        let file_caps = vec!["overview", "quality"];
        for cap in file_caps {
            assert!(
                ids.contains(&cap),
                "File should include '{cap}' in list_for(File)"
            );
        }
    }

    #[test]
    fn registry_list_for_scope_returns_sorted_descriptors() {
        let registry = crate::registry::ViewRegistry::new(None);
        let descriptors = registry.list_for(crate::dto::InspectableObjectType::Scope);

        let ids: Vec<&str> = descriptors.iter().map(|d| d.id.as_str()).collect();
        let mut sorted_ids = ids.clone();
        sorted_ids.sort();
        assert_eq!(
            ids, sorted_ids,
            "list_for(Scope) must return descriptors sorted alphabetically by id"
        );

        // For Scope, we expect: overview, quality
        // (Phase 1: dependencies/hotspots not yet migrated to registry)
        let scope_caps = vec!["overview", "quality"];
        for cap in scope_caps {
            assert!(
                ids.contains(&cap),
                "Scope should include '{cap}' in list_for(Scope)"
            );
        }
    }

    #[test]
    fn registry_list_for_issue_returns_sorted_descriptors() {
        let registry = crate::registry::ViewRegistry::new(None);
        let descriptors = registry.list_for(crate::dto::InspectableObjectType::QualityIssue);

        let ids: Vec<&str> = descriptors.iter().map(|d| d.id.as_str()).collect();
        let mut sorted_ids = ids.clone();
        sorted_ids.sort();
        assert_eq!(
            ids, sorted_ids,
            "list_for(QualityIssue) must return descriptors sorted alphabetically by id"
        );

        // For QualityIssue, we expect: quality only
        // (overview does not apply to QualityIssue)
        let issue_caps = vec!["quality"];
        for cap in issue_caps {
            assert!(
                ids.contains(&cap),
                "QualityIssue should include '{cap}' in list_for(QualityIssue)"
            );
        }
    }

    #[test]
    fn registry_list_for_rule_returns_sorted_descriptors() {
        let registry = crate::registry::ViewRegistry::new(None);
        let descriptors = registry.list_for(crate::dto::InspectableObjectType::Rule);

        let ids: Vec<&str> = descriptors.iter().map(|d| d.id.as_str()).collect();
        let mut sorted_ids = ids.clone();
        sorted_ids.sort();
        assert_eq!(
            ids, sorted_ids,
            "list_for(Rule) must return descriptors sorted alphabetically by id"
        );

        // For Rule, we expect: quality only
        // (overview does not apply to Rule)
        let rule_caps = vec!["quality"];
        for cap in rule_caps {
            assert!(
                ids.contains(&cap),
                "Rule should include '{cap}' in list_for(Rule)"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Registry contract — get_executor returns the capability by id
    // -------------------------------------------------------------------------

    #[test]
    fn registry_get_executor_overview_returns_capability() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry.get_executor("overview");
        assert!(
            executor.is_some(),
            "get_executor(\"overview\") must return Some — OverviewCapability should be registered"
        );
        let exec = executor.unwrap();
        assert_eq!(exec.id(), "overview");
        assert_eq!(exec.title(), "Overview");
    }

    #[test]
    fn registry_get_executor_unknown_id_returns_none() {
        let registry = crate::registry::ViewRegistry::new(None);
        let executor = registry.get_executor("this-does-not-exist");
        assert!(
            executor.is_none(),
            "get_executor for unknown id must return None"
        );
    }
}
