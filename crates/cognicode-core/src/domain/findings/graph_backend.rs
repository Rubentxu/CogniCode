//! Graph detector backend (M6, cycle e59).
//!
//! Evaluates the IR's `FLOW`/`EXCLUDE` steps over a supplied graph view and
//! emits a `GraphPath` evidence item per accepted path. It does **not** build
//! findings: the executor persists the evidence and the
//! [`FindingAssembler`](super::FindingAssembler) owns the rest, exactly as for
//! [`AstBackend`](super::AstBackend). No special case is added to the seam.
//!
//! ## Semantics
//!
//! - `MATCH` declares the subjects the detector observes (informational here).
//! - `FLOW source ->* sink` matches a path from a node whose subject is
//!   `source` to a node whose subject is `sink`, bounded by `max_hops`.
//! - `EXCLUDE path_contains X` drops any path whose nodes include a subject
//!   `X` (e.g. a sanitizer on the path).
//!
//! Traversal is deterministic: source nodes are visited in id order, BFS
//! expands neighbours in id order, and sinks are reported in id order.
//!
//! Ceiling: `B` (a graph path is strong static evidence, but not runtime).
//!
//! Pure domain: no I/O.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::admission::AdmittedDetector;
use super::detector_ir::{AnalysisCapability, DetectorStep, SubjectPattern};
use super::execution::{AnalysisInput, BackendError, DetectorBackend};
use super::finding::{CausalStepKind, EvidenceClass};
use super::outcome::{
    CausalObservation, DetectorDiagnostic, DetectorMatch, DetectorOutcome, EvidenceKind,
    ProducedEvidence,
};

/// A node in the graph view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    /// Stable node id.
    pub id: u64,
    /// The namespaced subject this node constitutes.
    pub subject: SubjectPattern,
    /// File path (for messages).
    pub path: String,
    /// 1-based line (for messages).
    pub line: u32,
}

/// A directed edge in the graph view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Source node id.
    pub from: u64,
    /// Target node id.
    pub to: u64,
}

/// The graph view handed to [`GraphBackend`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphInput {
    /// Nodes.
    pub nodes: Vec<GraphNode>,
    /// Directed edges.
    pub edges: Vec<GraphEdge>,
}

/// The graph-query backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct GraphBackend;

impl GraphBackend {
    /// One accepted path: the ordered node ids from source to sink.
    /// Shortest valid witnesses from a `source`-subject node to a
    /// `sink`-subject node.
    ///
    /// Exclusion is enforced **during** traversal: a node whose subject is in
    /// `excluded` is never entered, so the search finds the shortest witness
    /// that satisfies the constraints instead of finding an arbitrary path and
    /// discarding it afterwards (which could hide a valid alternate path).
    fn find_paths(
        &self,
        graph: &GraphInput,
        source_subject: &SubjectPattern,
        sink_subject: &SubjectPattern,
        max_hops: Option<u32>,
        excluded: &[&SubjectPattern],
    ) -> Vec<Vec<u64>> {
        // Deterministic index.
        let mut nodes: BTreeMap<u64, &GraphNode> = BTreeMap::new();
        for n in &graph.nodes {
            nodes.insert(n.id, n);
        }
        let mut adjacency: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for e in &graph.edges {
            adjacency.entry(e.from).or_default().push(e.to);
        }
        for neighbours in adjacency.values_mut() {
            neighbours.sort_unstable();
            neighbours.dedup();
        }

        let is_excluded = |id: u64| -> bool {
            nodes
                .get(&id)
                .map(|n| excluded.iter().any(|e| *e == &n.subject))
                .unwrap_or(false)
        };

        let mut paths = Vec::new();
        for (&source_id, source_node) in &nodes {
            if &source_node.subject != source_subject || is_excluded(source_id) {
                continue;
            }

            // BFS with predecessor tracking.
            let mut predecessor: BTreeMap<u64, u64> = BTreeMap::new();
            let mut distance: BTreeMap<u64, u32> = BTreeMap::new();
            let mut queue = std::collections::VecDeque::new();
            distance.insert(source_id, 0);
            queue.push_back(source_id);
            while let Some(current) = queue.pop_front() {
                let d = distance[&current];
                if let Some(limit) = max_hops {
                    if d >= limit {
                        continue;
                    }
                }
                if let Some(neighbours) = adjacency.get(&current) {
                    for &next in neighbours {
                        if distance.contains_key(&next) || is_excluded(next) {
                            continue;
                        }
                        distance.insert(next, d + 1);
                        predecessor.insert(next, current);
                        queue.push_back(next);
                    }
                }
            }

            let sinks: Vec<u64> = nodes
                .iter()
                .filter(|(id, n)| **id != source_id && &n.subject == sink_subject)
                .map(|(id, _)| *id)
                .collect();
            for sink_id in sinks {
                if !distance.contains_key(&sink_id) {
                    continue;
                }
                let mut path = vec![sink_id];
                let mut cursor = sink_id;
                while let Some(&prev) = predecessor.get(&cursor) {
                    path.push(prev);
                    cursor = prev;
                    if cursor == source_id {
                        break;
                    }
                }
                path.reverse();
                paths.push(path);
            }
        }
        paths
    }
}

impl DetectorBackend for GraphBackend {
    fn name(&self) -> &'static str {
        "graph"
    }

    fn capabilities(&self) -> BTreeSet<AnalysisCapability> {
        // FLOW/EXCLUDE require GraphQuery (see `DetectorStep::required_capabilities`).
        [AnalysisCapability::GraphQuery].into_iter().collect()
    }

    fn evidence_ceiling(&self) -> EvidenceClass {
        // A graph path is strong static evidence (B): better than a bare AST
        // match, weaker than a runtime trace.
        EvidenceClass::B
    }

    fn run(
        &self,
        admitted: &AdmittedDetector,
        input: &AnalysisInput,
    ) -> Result<DetectorOutcome, BackendError> {
        let graph = input
            .graph
            .as_ref()
            .ok_or(BackendError::MissingInput("graph"))?;

        let mut flows: Vec<(&SubjectPattern, &SubjectPattern, Option<u32>)> = Vec::new();
        let mut excludes: Vec<&SubjectPattern> = Vec::new();
        let mut produce = None;
        for step in &admitted.definition.steps {
            match step {
                DetectorStep::Flow {
                    source,
                    sink,
                    max_hops,
                } => flows.push((source, sink, *max_hops)),
                DetectorStep::Exclude { path_contains } => excludes.push(path_contains),
                DetectorStep::Produce { kind } => produce = Some(kind),
                DetectorStep::Match { .. } | DetectorStep::Verify { .. } => {}
            }
        }

        let produce =
            produce.ok_or_else(|| BackendError::Internal("detector has no PRODUCE step".into()))?;

        // Multiple FLOW steps have no defined semantics yet: fail loud rather
        // than silently running only the last one.
        let (source, sink, max_hops) = match flows.as_slice() {
            [] => {
                return Ok(DetectorOutcome {
                    produced_evidence: vec![],
                    matches: vec![],
                    diagnostics: vec![DetectorDiagnostic {
                        code: "no_flow_steps".to_string(),
                        message: "detector declares no FLOW step; nothing to traverse".to_string(),
                    }],
                });
            }
            [single] => *single,
            many => {
                return Err(BackendError::UnsupportedIr(format!(
                    "detector declares {} FLOW steps; multi-FLOW semantics are not defined yet",
                    many.len()
                )));
            }
        };

        let node_by_id: BTreeMap<u64, &GraphNode> = graph.nodes.iter().map(|n| (n.id, n)).collect();
        let mut outcome = DetectorOutcome::empty();

        let paths = self.find_paths(graph, source, sink, max_hops, &excludes);
        if paths.is_empty() {
            // Distinguish "no path at all" from "every path is sanitized".
            let unconstrained = self.find_paths(graph, source, sink, max_hops, &[]);
            if !unconstrained.is_empty() {
                outcome.diagnostics.push(DetectorDiagnostic {
                    code: "path_excluded".to_string(),
                    message: format!(
                        "path(s) from {} to {} exist but every witness passes through an excluded node",
                        source, sink
                    ),
                });
            }
        }

        for path in paths {
            let evidence_index = outcome.produced_evidence.len();
            let render = |id: u64| -> String {
                node_by_id
                    .get(&id)
                    .map(|n| format!("{} at {}:{}", n.subject, n.path, n.line))
                    .unwrap_or_else(|| format!("node {id}"))
            };
            outcome.produced_evidence.push(ProducedEvidence {
                kind: EvidenceKind::GraphPath,
                detail: path
                    .iter()
                    .map(|id| render(*id))
                    .collect::<Vec<_>>()
                    .join(" -> "),
                subject: None,
                fact: None,
            });

            let last = path.len().saturating_sub(1);
            let causal: Vec<CausalObservation> = path
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let kind = if i == 0 {
                        CausalStepKind::Source
                    } else if i == last {
                        CausalStepKind::Sink
                    } else {
                        CausalStepKind::Flow
                    };
                    CausalObservation {
                        kind,
                        detail: render(*id),
                        subject: None,
                        fact: None,
                        evidence: Some(evidence_index),
                    }
                })
                .collect();

            outcome.matches.push(DetectorMatch {
                kind: produce.clone(),
                message: format!(
                    "{} reached {} ({} hop path)",
                    source,
                    sink,
                    path.len().saturating_sub(1)
                ),
                evidence: vec![evidence_index],
                causal,
            });
        }

        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::DetectorAuthority;
    use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission, ExecutionPermit};
    use crate::domain::findings::detector_ir::{
        DetectorFindingPolicy, DetectorId, DetectorIr, FindingKind,
    };

    fn node(id: u64, subject: &str, line: u32) -> GraphNode {
        GraphNode {
            id,
            subject: SubjectPattern::new(subject).unwrap(),
            path: "src/app.rs".to_string(),
            line,
        }
    }

    fn admin_traversal_ir(flow_source: &str, flow_sink: &str) -> DetectorIr {
        DetectorIr {
            id: DetectorId::new("architecture.direct_db_access").unwrap(),
            name: "direct db access".to_string(),
            policy: DetectorFindingPolicy::default(),
            requires: [AnalysisCapability::GraphQuery].into_iter().collect(),
            authority: DetectorAuthority::Candidate,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new(flow_source).unwrap(),
                },
                DetectorStep::Flow {
                    source: SubjectPattern::new(flow_source).unwrap(),
                    sink: SubjectPattern::new(flow_sink).unwrap(),
                    max_hops: None,
                },
                DetectorStep::Exclude {
                    path_contains: SubjectPattern::new("security.sanitizer").unwrap(),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new("architecture.direct_db_access").unwrap(),
                },
            ],
        }
    }

    fn permit(ir: DetectorIr) -> ExecutionPermit {
        DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::HumanCurated).unwrap()
    }

    /// endpoint(1) -> sanitizer(2) -> persistence(3)
    fn graph_with_sanitizer() -> GraphInput {
        GraphInput {
            nodes: vec![
                node(1, "endpoint.http", 10),
                node(2, "security.sanitizer", 20),
                node(3, "persistence.write", 30),
            ],
            edges: vec![GraphEdge { from: 1, to: 2 }, GraphEdge { from: 2, to: 3 }],
        }
    }

    /// endpoint(1) -> service(4) -> persistence(3)
    fn graph_clean() -> GraphInput {
        GraphInput {
            nodes: vec![
                node(1, "endpoint.http", 10),
                node(4, "service.handler", 15),
                node(3, "persistence.write", 30),
            ],
            edges: vec![GraphEdge { from: 1, to: 4 }, GraphEdge { from: 4, to: 3 }],
        }
    }

    fn graph_input(graph: GraphInput) -> AnalysisInput {
        AnalysisInput {
            scope: None,
            ast: None,
            dataflow: None,
            graph: Some(graph),
        }
    }

    #[test]
    fn finds_a_flow_path() {
        let p = permit(admin_traversal_ir("endpoint.http", "persistence.write"));
        let outcome = GraphBackend
            .run(p.admitted(), &graph_input(graph_clean()))
            .unwrap();
        assert_eq!(outcome.matches.len(), 1);
        assert_eq!(outcome.produced_evidence.len(), 1);
        assert_eq!(outcome.produced_evidence[0].kind, EvidenceKind::GraphPath);
        assert_eq!(
            outcome.matches[0].kind.as_str(),
            "architecture.direct_db_access"
        );
        // Causal chain: Source -> Flow -> Sink.
        let kinds: Vec<CausalStepKind> = outcome.matches[0].causal.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            vec![
                CausalStepKind::Source,
                CausalStepKind::Flow,
                CausalStepKind::Sink
            ]
        );
        assert_eq!(
            outcome.matches[0].causal[0].evidence,
            Some(0),
            "causal evidence is attributable"
        );
    }

    #[test]
    fn excludes_a_path_through_a_sanitizer() {
        let p = permit(admin_traversal_ir("endpoint.http", "persistence.write"));
        let outcome = GraphBackend
            .run(p.admitted(), &graph_input(graph_with_sanitizer()))
            .unwrap();
        assert!(
            outcome.matches.is_empty(),
            "sanitized path must be excluded"
        );
        assert_eq!(outcome.diagnostics[0].code, "path_excluded");
    }

    #[test]
    fn missing_graph_input_fails_loud() {
        let p = permit(admin_traversal_ir("endpoint.http", "persistence.write"));
        let err = GraphBackend
            .run(p.admitted(), &AnalysisInput::default())
            .unwrap_err();
        assert_eq!(err, BackendError::MissingInput("graph"));
    }

    #[test]
    fn no_flow_steps_is_a_diagnostic() {
        let ir = DetectorIr {
            id: DetectorId::new("architecture.no_flow").unwrap(),
            name: "no flow".to_string(),
            policy: DetectorFindingPolicy::default(),
            requires: [AnalysisCapability::GraphQuery].into_iter().collect(),
            authority: DetectorAuthority::Candidate,
            steps: vec![DetectorStep::Produce {
                kind: FindingKind::new("architecture.no_flow").unwrap(),
            }],
        };
        let p = permit(ir);
        let outcome = GraphBackend
            .run(p.admitted(), &graph_input(graph_clean()))
            .unwrap();
        assert!(outcome.is_empty());
        assert_eq!(outcome.diagnostics[0].code, "no_flow_steps");
    }

    #[test]
    fn source_nodes_are_visited_in_deterministic_order() {
        let mut graph = graph_clean();
        graph.nodes.push(node(9, "endpoint.http", 90));
        graph.edges.push(GraphEdge { from: 9, to: 3 });
        let p = permit(admin_traversal_ir("endpoint.http", "persistence.write"));
        let outcome = GraphBackend.run(p.admitted(), &graph_input(graph)).unwrap();
        assert_eq!(outcome.matches.len(), 2);
        // Deterministic: the first path starts at node 1, the second at 9.
        assert!(outcome.produced_evidence[0].detail.contains(":10"));
        assert!(outcome.produced_evidence[1].detail.contains(":90"));
    }
    #[test]
    fn finds_an_alternate_clean_path_when_the_shortest_witness_is_sanitized() {
        //   endpoint(1) ── sanitizer(2) ── persistence(3)
        //         └────── service(4) ────┘
        // A valid witness exists through node 4; exclusion must be enforced
        // during traversal, not by discarding one arbitrary path afterwards.
        let graph = GraphInput {
            nodes: vec![
                node(1, "endpoint.http", 10),
                node(2, "security.sanitizer", 20),
                node(4, "service.handler", 40),
                node(3, "persistence.write", 30),
            ],
            edges: vec![
                GraphEdge { from: 1, to: 2 },
                GraphEdge { from: 2, to: 3 },
                GraphEdge { from: 1, to: 4 },
                GraphEdge { from: 4, to: 3 },
            ],
        };
        let p = permit(admin_traversal_ir("endpoint.http", "persistence.write"));
        let outcome = GraphBackend.run(p.admitted(), &graph_input(graph)).unwrap();
        assert_eq!(outcome.matches.len(), 1, "the clean path must be found");
        let detail = &outcome.produced_evidence[0].detail;
        assert!(
            detail.contains("service.handler"),
            "witness must go through the service, got: {detail}"
        );
        assert!(
            !detail.contains("security.sanitizer"),
            "witness must not mention the sanitizer, got: {detail}"
        );
    }

    #[test]
    fn multiple_flow_steps_fail_loud() {
        let mut ir = admin_traversal_ir("endpoint.http", "persistence.write");
        ir.steps.insert(
            2,
            DetectorStep::Flow {
                source: SubjectPattern::new("endpoint.http").unwrap(),
                sink: SubjectPattern::new("cache.read").unwrap(),
                max_hops: None,
            },
        );
        let p = permit(ir);
        let err = GraphBackend
            .run(p.admitted(), &graph_input(graph_clean()))
            .unwrap_err();
        assert!(
            matches!(err, BackendError::UnsupportedIr(_)),
            "multi-FLOW must fail loud, got {err:?}"
        );
    }
}
