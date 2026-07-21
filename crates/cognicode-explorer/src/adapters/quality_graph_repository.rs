//! `QualityGraphRepository` — graph-shaped quality hotspot adapter.
//!
//! Provides ranked hotspot nodes and traversal edges for the `RiskMap` view
//! by combining call-graph fan-in data with weighted quality issue counts.
//!
//! The adapter is read-only and wraps an optional `QualityRepository` and
//! a `GraphQueryPort`. When quality data is not available, `rank_hotspots`
//! returns fan-in-only hotspots (still ranked, still useful).

use crate::domain::lenses::hotspots::compute_risk;
use crate::domain::lens::severity_weight;
use crate::dto::InspectionTarget;
use crate::error::{ExplorerError, ExplorerResult};
use crate::ports::quality_repository::QualityRepository;
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::traits::graph_query_port::GraphQueryPort;
use serde::{Deserialize, Serialize};

/// A single ranked hotspot node — a symbol augmented with risk metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotNode {
    /// Symbol identifier (format: `"symbol:{file}:{name}:{line}"`).
    pub object_id: String,
    /// Human-readable label for display.
    pub label: String,
    /// Source file path.
    pub file: String,
    /// Declaration line number.
    pub line: u32,
    /// Number of direct incoming edges (callers).
    pub fan_in: u32,
    /// Sum of `severity_weight` for all open issues at this symbol's location.
    pub weighted_issue_count: f32,
    /// Computed risk score: `fan_in * 0.4 + weighted_issue_count * 0.6`.
    pub risk: f32,
}

/// A graph edge enriched with provenance and confidence metadata.
///
/// Used by `QualityGraphRepository::traverse_from` to return edges that
/// preserve the provenance chain and confidence scores from the graph layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelEdge {
    /// Source symbol id.
    pub source_id: SymbolId,
    /// Resolved source symbol name.
    pub source_name: String,
    /// Target symbol id.
    pub target_id: SymbolId,
    /// Resolved target symbol name.
    pub target_name: String,
    /// How this relationship was established (e.g. `"extracted"`, `"inferred"`).
    pub provenance: String,
    /// Edge trust score in `[0.0, 1.0]`.
    pub confidence: f64,
}

/// Optional filter for `traverse_from`.
#[derive(Debug, Clone, Default)]
pub struct TraversalFilter {
    /// Maximum traversal depth. `None` means unlimited.
    pub max_depth: Option<u8>,
    /// Restrict to only incoming edges (callers).
    pub incoming_only: bool,
    /// Restrict to only outgoing edges (callees).
    pub outgoing_only: bool,
}

/// Read-only adapter combining quality data with graph topology for risk views.
pub struct QualityGraphRepository<'a> {
    quality: Option<&'a dyn QualityRepository>,
    graph: Option<&'a dyn GraphQueryPort>,
}

impl<'a> QualityGraphRepository<'a> {
    /// Construct a new repository.
    ///
    /// `quality` may be `None` — the adapter degrades gracefully by computing
    /// fan-in-only risk scores (quality signals are absent but the view still works).
    ///
    /// `graph` may also be `None` — in this case the adapter computes topology-only
    /// hotspot ranking (fan-in only, weighted_issue_count = 0.0).
    pub fn new(
        quality: Option<&'a dyn QualityRepository>,
        graph: Option<&'a dyn GraphQueryPort>,
    ) -> Self {
        Self { quality, graph }
    }

    /// Rank all symbols in `target` by risk score descending, returning up to `limit` hotspots.
    ///
    /// Returns `ExplorerError::QualityUnavailable` when quality data is required
    /// but the quality repository is not wired.
    pub fn rank_hotspots(
        &self,
        target: &InspectionTarget,
        limit: usize,
    ) -> ExplorerResult<Vec<HotspotNode>> {
        let symbols = match target {
            InspectionTarget::Symbol(s) => vec![s.clone()],
            InspectionTarget::File { symbols, .. } => symbols.clone(),
            InspectionTarget::Scope { symbols, .. } => symbols.clone(),
            InspectionTarget::Issue(_) | InspectionTarget::Rule { .. } => {
                return Err(ExplorerError::ViewNotAvailable {
                    object_id: format!("{:?}", target),
                    view_id: "risk_map".into(),
                });
            }
            InspectionTarget::SavedExploration(_) | InspectionTarget::Investigation(_) => {
                return Err(ExplorerError::ViewNotAvailable {
                    object_id: format!("{:?}", target),
                    view_id: "risk_map".into(),
                });
            }
        };

        // Collect hotspots with risk scores.
        let mut hotspots: Vec<HotspotNode> = Vec::new();
        for symbol in symbols {
            let fan_in = self.graph.map(|g| g.fan_in(&symbol.id) as u32).unwrap_or(0);
            let weighted_issues = self
                .quality
                .ok_or_else(|| {
                    ExplorerError::QualityUnavailable(
                        "quality repository not wired".to_string(),
                    )
                })?
                .issues_at_line(&symbol.file, symbol.line)
                .unwrap_or_default()
                .iter()
                .map(|i| severity_weight(&i.severity))
                .sum();

            let risk = compute_risk(fan_in, weighted_issues);
            let object_id = format!("symbol:{}:{}:{}", symbol.file, symbol.name, symbol.line);
            let label = format!("{} at {}:{}", symbol.name, symbol.file, symbol.line);
            hotspots.push(HotspotNode {
                object_id,
                label,
                file: symbol.file.clone(),
                line: symbol.line,
                fan_in,
                weighted_issue_count: weighted_issues,
                risk,
            });
        }

        // Sort by risk descending, then by name ascending for stable ordering.
        hotspots.sort_by(|a, b| {
            b.risk
                .partial_cmp(&a.risk)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.label.cmp(&b.label))
        });

        hotspots.truncate(limit);
        Ok(hotspots)
    }

    /// Traverse edges from `symbol_id` and return enriched relation edges.
    ///
    /// Preserves `provenance` and `confidence` from the graph layer.
    /// Returns an empty vector when the graph is not wired.
    pub fn traverse_from(
        &self,
        symbol_id: &SymbolId,
        filter: TraversalFilter,
    ) -> ExplorerResult<Vec<RelEdge>> {
        let mut edges = Vec::new();

        let graph = match self.graph {
            Some(g) => g,
            None => return Ok(edges), // No graph wired — topology-only path
        };

        // Gather incoming edges (callers).
        if !filter.outgoing_only {
            let caller_meta = graph.callers_with_metadata(symbol_id);
            let caller_targets = graph.callers(symbol_id);
            for (meta, target) in caller_meta.iter().zip(caller_targets.iter()) {
                edges.push(RelEdge {
                    source_id: meta.caller_id.clone(),
                    source_name: target.name.clone(),
                    target_id: symbol_id.clone(),
                    target_name: String::new(), // Caller doesn't know target name here
                    provenance: meta.provenance.to_string(),
                    confidence: meta.confidence,
                });
            }
        }

        // Gather outgoing edges (callees).
        if !filter.incoming_only {
            let callee_meta = graph.callees_with_metadata(symbol_id);
            let callee_targets = graph.callees(symbol_id);
            for (meta, target) in callee_meta.iter().zip(callee_targets.iter()) {
                edges.push(RelEdge {
                    source_id: symbol_id.clone(),
                    source_name: String::new(), // Source doesn't know target name here
                    target_id: meta.callee_id.clone(),
                    target_name: target.name.clone(),
                    provenance: meta.provenance.to_string(),
                    confidence: meta.confidence,
                });
            }
        }

        Ok(edges)
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::quality_repository::QualityIssue;
    use crate::ports::symbol_repository::ResolvedSymbol;
    use cognicode_core::domain::value_objects::Provenance;

    /// Mock QualityRepository that returns configurable issues.
    struct MockQualityRepository {
        issues: Vec<QualityIssue>,
    }

    impl MockQualityRepository {
        fn new(issues: Vec<QualityIssue>) -> Self {
            Self { issues }
        }
    }

    impl QualityRepository for MockQualityRepository {
        fn issues_for_file(&self, _file: &str) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self.issues.clone())
        }
        fn issues_for_scope(&self, _scope_prefix: &str) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self.issues.clone())
        }
        fn issues_at_line(&self, file: &str, line: u32) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self
                .issues
                .iter()
                .filter(|i| i.file_path == file && i.line == line)
                .cloned()
                .collect())
        }
        fn issue_by_id(&self, _id: i64) -> ExplorerResult<Option<QualityIssue>> {
            Ok(None)
        }
        fn rule_summary(&self, _rule_id: &str) -> ExplorerResult<crate::ports::quality_repository::RuleSummary> {
            Ok(crate::ports::quality_repository::RuleSummary {
                rule_id: String::new(),
                description: String::new(),
                open_count: 0,
            })
        }
        fn quality_gate(
            &self,
            _workspace_id: Option<&str>,
        ) -> ExplorerResult<crate::ports::quality_repository::QualityGateSummary> {
            Ok(Default::default())
        }
        fn open_issues_count(&self, _workspace_id: Option<&str>) -> ExplorerResult<usize> {
            Ok(self.issues.len())
        }
        fn issues_for_workspace(
            &self,
            _workspace_id: Option<&str>,
            _filter: &crate::ports::quality_repository::IssueFilter,
        ) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self.issues.clone())
        }
    }

    /// Mock GraphQueryPort that returns configurable fan-in/fan-out, callers/callees, and metadata.
    struct MockGraphQueryPort {
        fan_in_map: std::collections::HashMap<SymbolId, usize>,
        callers_map: std::collections::HashMap<SymbolId, Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget>>,
        callees_map: std::collections::HashMap<SymbolId, Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget>>,
        callers_metadata_map: std::collections::HashMap<SymbolId, Vec<cognicode_core::domain::traits::graph_query_port::CallerWithMetadata>>,
        callees_metadata_map: std::collections::HashMap<SymbolId, Vec<cognicode_core::domain::traits::graph_query_port::CalleeWithMetadata>>,
    }

    impl MockGraphQueryPort {
        fn new() -> Self {
            Self {
                fan_in_map: std::collections::HashMap::new(),
                callers_map: std::collections::HashMap::new(),
                callees_map: std::collections::HashMap::new(),
                callers_metadata_map: std::collections::HashMap::new(),
                callees_metadata_map: std::collections::HashMap::new(),
            }
        }
        fn with_fan_in(mut self, id: SymbolId, fan_in: usize) -> Self {
            self.fan_in_map.insert(id, fan_in);
            self
        }
        fn with_callers(mut self, id: SymbolId, callers: Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget>) -> Self {
            self.callers_map.insert(id, callers);
            self
        }
        fn with_callees(mut self, id: SymbolId, callees: Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget>) -> Self {
            self.callees_map.insert(id, callees);
            self
        }
        fn with_callers_metadata(mut self, id: SymbolId, metadata: Vec<cognicode_core::domain::traits::graph_query_port::CallerWithMetadata>) -> Self {
            self.callers_metadata_map.insert(id, metadata);
            self
        }
        fn with_callees_metadata(mut self, id: SymbolId, metadata: Vec<cognicode_core::domain::traits::graph_query_port::CalleeWithMetadata>) -> Self {
            self.callees_metadata_map.insert(id, metadata);
            self
        }
    }

    impl GraphQueryPort for MockGraphQueryPort {
        fn callers(&self, id: &SymbolId) -> Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget> {
            self.callers_map.get(id).cloned().unwrap_or_default()
        }
        fn callees(&self, id: &SymbolId) -> Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget> {
            self.callees_map.get(id).cloned().unwrap_or_default()
        }
        fn fan_in(&self, id: &SymbolId) -> usize {
            self.fan_in_map.get(id).copied().unwrap_or(0)
        }
        fn fan_out(&self, _id: &SymbolId) -> usize {
            0
        }
        fn callers_with_metadata(
            &self,
            id: &SymbolId,
        ) -> Vec<cognicode_core::domain::traits::graph_query_port::CallerWithMetadata> {
            self.callers_metadata_map.get(id).cloned().unwrap_or_default()
        }
        fn callees_with_metadata(
            &self,
            id: &SymbolId,
        ) -> Vec<cognicode_core::domain::traits::graph_query_port::CalleeWithMetadata> {
            self.callees_metadata_map.get(id).cloned().unwrap_or_default()
        }
        fn dependencies_with_metadata(
            &self,
            _id: &SymbolId,
        ) -> Vec<cognicode_core::domain::traits::graph_query_port::RelationTargetWithMetadata> {
            Vec::new()
        }
        fn traverse_callees(
            &self,
            _id: &SymbolId,
            _max_depth: u8,
        ) -> Vec<cognicode_core::domain::aggregates::CallEntry> {
            Vec::new()
        }
        fn traverse_callers(
            &self,
            _id: &SymbolId,
            _max_depth: u8,
        ) -> Vec<cognicode_core::domain::aggregates::CallEntry> {
            Vec::new()
        }
    }

    fn make_symbol(id: &str, name: &str, file: &str, line: u32) -> ResolvedSymbol {
        ResolvedSymbol {
            id: SymbolId::new(id),
            name: name.to_string(),
            file: file.to_string(),
            line,
            signature: None,
            kind: cognicode_core::domain::value_objects::SymbolKind::Function,
        }
    }

    // -------------------------------------------------------------------------
    // Tests for rank_hotspots
    // -------------------------------------------------------------------------

    #[test]
    fn rank_hotspots_returns_ranked_by_risk_desc() {
        let sym_a = make_symbol("a", "alpha", "src/lib.rs", 10);
        let sym_b = make_symbol("b", "beta", "src/lib.rs", 20);
        let sym_c = make_symbol("c", "gamma", "src/lib.rs", 30);

        let issues = vec![
            QualityIssue {
                id: 1,
                rule_id: "rule1".into(),
                severity: "critical".into(),
                category: "complexity".into(),
                file_path: "src/lib.rs".into(),
                line: 10,
                message: "issue".into(),
                status: "open".into(),
            },
            QualityIssue {
                id: 2,
                rule_id: "rule1".into(),
                severity: "major".into(),
                category: "complexity".into(),
                file_path: "src/lib.rs".into(),
                line: 20,
                message: "issue".into(),
                status: "open".into(),
            },
        ];

        let quality = MockQualityRepository::new(issues);
        let graph = MockGraphQueryPort::new()
            .with_fan_in(sym_a.id.clone(), 10) // 10 * 0.4 = 4.0
            .with_fan_in(sym_b.id.clone(), 5); // 5 * 0.4 = 2.0

        let repo = QualityGraphRepository::new(Some(&quality), Some(&graph));
        let target = InspectionTarget::Scope {
            path: "src".into(),
            files: vec!["src/lib.rs".into()],
            symbols: vec![sym_a.clone(), sym_b.clone(), sym_c],
        };

        let hotspots = repo.rank_hotspots(&target, 5).unwrap();

        assert_eq!(hotspots.len(), 3);
        // Alpha: fan_in=10, issues=[critical=2.0] → risk = 4.0 + 1.2 = 5.2
        // Beta: fan_in=5, issues=[major=1.5] → risk = 2.0 + 0.9 = 2.9
        // Gamma: fan_in=0, issues=[] → risk = 0.0
        assert_eq!(hotspots[0].object_id, "symbol:src/lib.rs:alpha:10");
        assert_eq!(hotspots[1].object_id, "symbol:src/lib.rs:beta:20");
        assert_eq!(hotspots[2].object_id, "symbol:src/lib.rs:gamma:30");
    }

    #[test]
    fn rank_hotspots_respects_limit() {
        let sym_a = make_symbol("a", "alpha", "src/lib.rs", 10);
        let sym_b = make_symbol("b", "beta", "src/lib.rs", 20);
        let sym_c = make_symbol("c", "gamma", "src/lib.rs", 30);

        let quality = MockQualityRepository::new(vec![]);
        let graph = MockGraphQueryPort::new()
            .with_fan_in(sym_a.id.clone(), 3)
            .with_fan_in(sym_b.id.clone(), 2)
            .with_fan_in(sym_c.id.clone(), 1);

        let repo = QualityGraphRepository::new(Some(&quality), Some(&graph));
        let target = InspectionTarget::Scope {
            path: "src".into(),
            files: vec!["src/lib.rs".into()],
            symbols: vec![sym_a, sym_b, sym_c],
        };

        let hotspots = repo.rank_hotspots(&target, 2).unwrap();

        assert_eq!(hotspots.len(), 2);
        assert_eq!(hotspots[0].object_id, "symbol:src/lib.rs:alpha:10");
        assert_eq!(hotspots[1].object_id, "symbol:src/lib.rs:beta:20");
    }

    #[test]
    fn rank_hotspots_fan_in_only_when_no_issues() {
        let sym_a = make_symbol("a", "alpha", "src/lib.rs", 10);
        let sym_b = make_symbol("b", "beta", "src/lib.rs", 20);

        // No quality issues
        let quality = MockQualityRepository::new(vec![]);
        let graph = MockGraphQueryPort::new()
            .with_fan_in(sym_a.id.clone(), 10)
            .with_fan_in(sym_b.id.clone(), 5);

        let repo = QualityGraphRepository::new(Some(&quality), Some(&graph));
        let target = InspectionTarget::Scope {
            path: "src".into(),
            files: vec!["src/lib.rs".into()],
            symbols: vec![sym_a.clone(), sym_b.clone()],
        };

        let hotspots = repo.rank_hotspots(&target, 5).unwrap();

        assert_eq!(hotspots.len(), 2);
        // Risk should still be non-zero (fan_in contributes 0.4 per caller)
        assert!(hotspots[0].risk > 0.0);
        assert_eq!(hotspots[0].weighted_issue_count, 0.0);
        assert_eq!(hotspots[0].fan_in, 10);
    }

    #[test]
    fn rank_hotspots_missing_quality_record_excluded() {
        let sym_a = make_symbol("a", "alpha", "src/lib.rs", 10);
        // Beta is at a different line with no issues
        let sym_b = make_symbol("b", "beta", "src/lib.rs", 99);

        let issues = vec![QualityIssue {
            id: 1,
            rule_id: "rule1".into(),
            severity: "critical".into(),
            category: "complexity".into(),
            file_path: "src/lib.rs".into(),
            line: 10, // Only alpha's line has issues
            message: "issue".into(),
            status: "open".into(),
        }];

        let quality = MockQualityRepository::new(issues);
        let graph = MockGraphQueryPort::new()
            .with_fan_in(sym_a.id.clone(), 10)
            .with_fan_in(sym_b.id.clone(), 5);

        let repo = QualityGraphRepository::new(Some(&quality), Some(&graph));
        let target = InspectionTarget::Scope {
            path: "src".into(),
            files: vec!["src/lib.rs".into()],
            symbols: vec![sym_a.clone(), sym_b.clone()],
        };

        let hotspots = repo.rank_hotspots(&target, 5).unwrap();

        // Beta has no issues at its line, so weighted_issue_count = 0
        // But it's NOT excluded — it just has a lower risk score
        assert_eq!(hotspots.len(), 2);
        assert_eq!(hotspots[1].object_id, "symbol:src/lib.rs:beta:99");
        assert_eq!(hotspots[1].weighted_issue_count, 0.0);
    }

    #[test]
    fn rank_hotspots_quality_unavailable_error() {
        let sym_a = make_symbol("a", "alpha", "src/lib.rs", 10);

        // No quality repository wired
        let graph = MockGraphQueryPort::new().with_fan_in(sym_a.id.clone(), 10);
        let repo = QualityGraphRepository::new(None, Some(&graph));
        let target = InspectionTarget::Symbol(sym_a);

        let err = repo.rank_hotspots(&target, 5).unwrap_err();
        assert!(matches!(err, ExplorerError::QualityUnavailable(_)));
    }

    #[test]
    fn rank_hotspots_symbol_target() {
        let sym = make_symbol("s", "test_fn", "src/lib.rs", 42);

        let issues = vec![QualityIssue {
            id: 1,
            rule_id: "rule1".into(),
            severity: "critical".into(),
            category: "complexity".into(),
            file_path: "src/lib.rs".into(),
            line: 42,
            message: "issue".into(),
            status: "open".into(),
        }];

        let quality = MockQualityRepository::new(issues);
        let graph = MockGraphQueryPort::new().with_fan_in(sym.id.clone(), 7);
        let repo = QualityGraphRepository::new(Some(&quality), Some(&graph));
        let target = InspectionTarget::Symbol(sym.clone());

        let hotspots = repo.rank_hotspots(&target, 5).unwrap();

        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].fan_in, 7);
        assert_eq!(hotspots[0].weighted_issue_count, 2.0); // critical = 2.0
        let expected_risk = 7.0 * 0.4 + 2.0 * 0.6; // 2.8 + 1.2 = 4.0
        assert_eq!(hotspots[0].risk, expected_risk);
    }

    // -------------------------------------------------------------------------
    // Tests for traverse_from
    // -------------------------------------------------------------------------

    #[test]
    fn traverse_from_preserves_provenance_and_confidence() {
        let target_id = SymbolId::new("target");
        let caller_id = SymbolId::new("caller1");
        let callee_id = SymbolId::new("callee1");

        let callers = vec![
            cognicode_core::domain::traits::graph_query_port::RelationTarget {
                id: caller_id.clone(),
                name: "caller1".into(),
                kind: cognicode_core::domain::value_objects::SymbolKind::Function,
                file: "src/lib.rs".into(),
                line: 10,
                signature: None,
            },
        ];
        let callees = vec![
            cognicode_core::domain::traits::graph_query_port::RelationTarget {
                id: callee_id.clone(),
                name: "callee1".into(),
                kind: cognicode_core::domain::value_objects::SymbolKind::Function,
                file: "src/lib.rs".into(),
                line: 50,
                signature: None,
            },
        ];
        let callers_meta = vec![
            cognicode_core::domain::traits::graph_query_port::CallerWithMetadata {
                caller_id: caller_id.clone(),
                provenance: Provenance::Extracted,
                confidence: 0.95,
            },
        ];
        let callees_meta = vec![
            cognicode_core::domain::traits::graph_query_port::CalleeWithMetadata {
                callee_id: callee_id.clone(),
                dependency_type: cognicode_core::domain::value_objects::DependencyType::Calls,
                provenance: Provenance::Inferred,
                confidence: 0.87,
            },
        ];

        let graph = MockGraphQueryPort::new()
            .with_callers(target_id.clone(), callers)
            .with_callees(target_id.clone(), callees)
            .with_callers_metadata(target_id.clone(), callers_meta)
            .with_callees_metadata(target_id.clone(), callees_meta);
        let repo = QualityGraphRepository::new(None, Some(&graph));
        let filter = TraversalFilter::default();

        let edges = repo.traverse_from(&target_id, filter).unwrap();

        assert_eq!(edges.len(), 2);

        // Check incoming edge (caller)
        let incoming = edges.iter().find(|e| e.target_id == target_id).unwrap();
        assert_eq!(incoming.source_id, caller_id);
        assert_eq!(incoming.source_name, "caller1");
        assert_eq!(incoming.provenance, "Extracted");
        assert_eq!(incoming.confidence, 0.95);

        // Check outgoing edge (callee)
        let outgoing = edges.iter().find(|e| e.source_id == target_id).unwrap();
        assert_eq!(outgoing.target_id, callee_id);
        assert_eq!(outgoing.target_name, "callee1");
        assert_eq!(outgoing.provenance, "Inferred");
        assert_eq!(outgoing.confidence, 0.87);
    }

    #[test]
    fn traverse_from_incoming_only() {
        let target_id = SymbolId::new("target");
        let caller_id = SymbolId::new("caller1");

        let callers = vec![
            cognicode_core::domain::traits::graph_query_port::RelationTarget {
                id: caller_id.clone(),
                name: "caller1".into(),
                kind: cognicode_core::domain::value_objects::SymbolKind::Function,
                file: "src/lib.rs".into(),
                line: 10,
                signature: None,
            },
        ];
        let callees = vec![
            cognicode_core::domain::traits::graph_query_port::RelationTarget {
                id: SymbolId::new("other"),
                name: "other".into(),
                kind: cognicode_core::domain::value_objects::SymbolKind::Function,
                file: "src/lib.rs".into(),
                line: 50,
                signature: None,
            },
        ];
        let callers_meta = vec![
            cognicode_core::domain::traits::graph_query_port::CallerWithMetadata {
                caller_id: caller_id.clone(),
                provenance: Provenance::Extracted,
                confidence: 0.9,
            },
        ];
        let callees_meta = vec![
            cognicode_core::domain::traits::graph_query_port::CalleeWithMetadata {
                callee_id: SymbolId::new("other"),
                dependency_type: cognicode_core::domain::value_objects::DependencyType::Calls,
                provenance: Provenance::Inferred,
                confidence: 0.8,
            },
        ];

        let graph = MockGraphQueryPort::new()
            .with_callers(target_id.clone(), callers)
            .with_callees(target_id.clone(), callees)
            .with_callers_metadata(target_id.clone(), callers_meta)
            .with_callees_metadata(target_id.clone(), callees_meta);
        let repo = QualityGraphRepository::new(None, Some(&graph));
        let filter = TraversalFilter {
            incoming_only: true,
            ..Default::default()
        };

        let edges = repo.traverse_from(&target_id, filter).unwrap();

        assert_eq!(edges.len(), 1);
        let incoming = &edges[0];
        assert_eq!(incoming.source_id, caller_id);
        assert_eq!(incoming.target_id, target_id);
    }

    // -------------------------------------------------------------------------
    // Tests for compute_risk helper
    // -------------------------------------------------------------------------

    #[test]
    fn compute_risk_formula() {
        // risk = fan_in * 0.4 + weighted_issue_count * 0.6
        assert_eq!(compute_risk(10, 2.0), 10.0 * 0.4 + 2.0 * 0.6);
        assert_eq!(compute_risk(0, 0.0), 0.0);
        assert_eq!(compute_risk(5, 1.5), 5.0 * 0.4 + 1.5 * 0.6); // 2.0 + 0.9 = 2.9
    }
}
