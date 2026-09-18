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
use super::grounding::GroundingRef;
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
    /// The canonical fact this node was projected from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grounding: Option<GroundingRef>,
}

/// A directed edge in the graph view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Source node id.
    pub from: u64,
    /// Target node id.
    pub to: u64,
    /// The canonical fact that witnesses this **relation**.
    ///
    /// Reachability is proved by traversing edges, so an edge grounding is what
    /// makes a graph witness checkable: nodes only show the endpoints exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grounding: Option<GroundingRef>,
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
                .map(|n| excluded.contains(&&n.subject))
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
                if let Some(limit) = max_hops
                    && d >= limit
                {
                    continue;
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

        // How each hop is witnessed. A hop with more than one parallel edge is
        // only usable when every parallel edge names the *same* fact: otherwise
        // no single fact can be said to ground the hop, and the hop fails
        // closed rather than picking one arbitrarily.
        let mut relations: BTreeMap<(u64, u64), Vec<&GraphEdge>> = BTreeMap::new();
        for edge in &graph.edges {
            relations
                .entry((edge.from, edge.to))
                .or_default()
                .push(edge);
        }

        let render = |id: u64| -> String {
            node_by_id
                .get(&id)
                .map(|n| format!("{} at {}:{}", n.subject, n.path, n.line))
                .unwrap_or_else(|| format!("node {id}"))
        };

        for path in paths {
            // Atoms are per *element of the witness*, not one blob per path:
            // the source node, one per traversed relation, and the sink node.
            // Reachability is proved by the relations, so the hop carrying an
            // ambiguous grounding is exactly the one that must not gate.
            let mut atoms: Vec<usize> = Vec::with_capacity(path.len() * 2);
            let mut causal: Vec<CausalObservation> = Vec::with_capacity(path.len() * 2);
            let mut ambiguous_hops: Vec<(u64, u64)> = Vec::new();

            let source_id = path[0];
            let source_node = node_by_id.get(&source_id);
            let source_evidence = outcome.produced_evidence.len();
            outcome.produced_evidence.push(ProducedEvidence {
                kind: EvidenceKind::GraphPath,
                detail: render(source_id),
                subject: source_node.and_then(|n| n.grounding).and_then(|g| g.entity),
                grounding: source_node.and_then(|n| n.grounding),
            });
            atoms.push(source_evidence);
            causal.push(CausalObservation {
                kind: CausalStepKind::Source,
                detail: render(source_id),
                subject: source_node.and_then(|n| n.grounding).and_then(|g| g.entity),
                evidence: Some(source_evidence),
            });

            for hop in path.windows(2) {
                let (from, to) = (hop[0], hop[1]);
                let edges = relations.get(&(from, to)).cloned().unwrap_or_default();
                let grounding = match edges.as_slice() {
                    [] => None,
                    [only] => only.grounding,
                    many => {
                        let mut facts = many.iter().map(|e| e.grounding);
                        let first = facts.next().flatten();
                        if many.iter().any(|e| e.grounding.is_some()) && facts.any(|g| g != first) {
                            ambiguous_hops.push((from, to));
                            None
                        } else {
                            first
                        }
                    }
                };

                let relation_evidence = outcome.produced_evidence.len();
                outcome.produced_evidence.push(ProducedEvidence {
                    kind: EvidenceKind::GraphPath,
                    detail: format!("{} -> {}", render(from), render(to)),
                    subject: grounding.and_then(|g| g.entity),
                    grounding,
                });
                atoms.push(relation_evidence);
                causal.push(CausalObservation {
                    kind: CausalStepKind::Flow,
                    detail: format!("{} -> {}", render(from), render(to)),
                    subject: grounding.and_then(|g| g.entity),
                    evidence: Some(relation_evidence),
                });
            }

            let sink_id = path[path.len() - 1];
            let sink_node = node_by_id.get(&sink_id);
            let sink_evidence = outcome.produced_evidence.len();
            outcome.produced_evidence.push(ProducedEvidence {
                kind: EvidenceKind::GraphPath,
                detail: render(sink_id),
                subject: sink_node.and_then(|n| n.grounding).and_then(|g| g.entity),
                grounding: sink_node.and_then(|n| n.grounding),
            });
            atoms.push(sink_evidence);
            causal.push(CausalObservation {
                kind: CausalStepKind::Sink,
                detail: render(sink_id),
                subject: sink_node.and_then(|n| n.grounding).and_then(|g| g.entity),
                evidence: Some(sink_evidence),
            });

            for (from, to) in ambiguous_hops {
                outcome.diagnostics.push(DetectorDiagnostic {
                    code: "ambiguous_relation_grounding".to_string(),
                    message: format!(
                        "relation {from} -> {to} is witnessed by parallel edges with different facts; the hop is left ungrounded rather than attributed arbitrarily"
                    ),
                });
            }

            outcome.matches.push(DetectorMatch {
                kind: produce.clone(),
                message: format!(
                    "{} reached {} ({} hop path)",
                    source,
                    sink,
                    path.len().saturating_sub(1)
                ),
                evidence: atoms,
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
            grounding: None,
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
            edges: vec![
                GraphEdge {
                    from: 1,
                    to: 2,
                    grounding: None,
                },
                GraphEdge {
                    from: 2,
                    to: 3,
                    grounding: None,
                },
            ],
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
            edges: vec![
                GraphEdge {
                    from: 1,
                    to: 4,
                    grounding: None,
                },
                GraphEdge {
                    from: 4,
                    to: 3,
                    grounding: None,
                },
            ],
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
        // Atomic witness: source node, one atom per traversed relation, sink
        // node. A single blob per path could not be grounded edge by edge.
        assert_eq!(outcome.produced_evidence.len(), 4);
        assert_eq!(outcome.produced_evidence[0].kind, EvidenceKind::GraphPath);
        let details: Vec<&str> = outcome
            .produced_evidence
            .iter()
            .map(|e| e.detail.as_str())
            .collect();
        assert!(details[0].contains("endpoint.http"), "{details:?}");
        assert!(details[1].contains("->"), "{details:?}");
        assert!(details[3].contains("persistence.write"), "{details:?}");
        assert_eq!(
            outcome.matches[0].evidence,
            vec![0, 1, 2, 3],
            "the match claims every atom of its witness"
        );
        assert_eq!(
            outcome.matches[0].kind.as_str(),
            "architecture.direct_db_access"
        );
        // Causal chain: Source -> Flow (one per traversed relation) -> Sink.
        let kinds: Vec<CausalStepKind> = outcome.matches[0].causal.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            vec![
                CausalStepKind::Source,
                CausalStepKind::Flow,
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
        graph.edges.push(GraphEdge {
            from: 9,
            to: 3,
            grounding: None,
        });
        let p = permit(admin_traversal_ir("endpoint.http", "persistence.write"));
        let outcome = GraphBackend.run(p.admitted(), &graph_input(graph)).unwrap();
        assert_eq!(outcome.matches.len(), 2);
        // Deterministic: the first path starts at node 1, the second at 9.
        // Each path contributes four atoms (source, two relations, sink).
        assert!(outcome.produced_evidence[0].detail.contains(":10"));
        assert!(outcome.produced_evidence[4].detail.contains(":90"));
    }
    /// Two parallel relations with *different* facts leave the hop ungrounded:
    /// no single fact can be said to witness it, and picking one arbitrarily
    /// would fabricate a causal claim.
    #[test]
    fn parallel_edges_with_different_facts_leave_the_hop_ungrounded() {
        let graph = GraphInput {
            nodes: vec![
                node(1, "endpoint.http", 10),
                node(2, "persistence.write", 30),
            ],
            edges: vec![
                GraphEdge {
                    from: 1,
                    to: 2,
                    grounding: Some(GroundingRef::fact(crate::domain::kernel_ids::FactId::new(
                        1,
                    ))),
                },
                GraphEdge {
                    from: 1,
                    to: 2,
                    grounding: Some(GroundingRef::fact(crate::domain::kernel_ids::FactId::new(
                        2,
                    ))),
                },
            ],
        };
        let p = permit(admin_traversal_ir("endpoint.http", "persistence.write"));
        let outcome = GraphBackend.run(p.admitted(), &graph_input(graph)).unwrap();
        assert_eq!(outcome.matches.len(), 1, "the relation is still found");
        assert_eq!(outcome.produced_evidence.len(), 3, "source, hop, sink");
        assert!(
            outcome.produced_evidence[1].grounding.is_none(),
            "the ambiguous hop must fail closed"
        );
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.code == "ambiguous_relation_grounding"),
            "the ambiguity must be reported, not hidden: {:?}",
            outcome.diagnostics
        );
    }

    /// Two parallel relations that name the *same* fact are not ambiguous.
    #[test]
    fn parallel_edges_with_the_same_fact_stay_grounded() {
        let fact = Some(GroundingRef::fact(crate::domain::kernel_ids::FactId::new(
            1,
        )));
        let graph = GraphInput {
            nodes: vec![
                node(1, "endpoint.http", 10),
                node(2, "persistence.write", 30),
            ],
            edges: vec![
                GraphEdge {
                    from: 1,
                    to: 2,
                    grounding: fact,
                },
                GraphEdge {
                    from: 1,
                    to: 2,
                    grounding: fact,
                },
            ],
        };
        let p = permit(admin_traversal_ir("endpoint.http", "persistence.write"));
        let outcome = GraphBackend.run(p.admitted(), &graph_input(graph)).unwrap();
        assert_eq!(
            outcome.produced_evidence[1].grounding, fact,
            "identical parallel edges agree on the fact"
        );
        assert!(outcome.diagnostics.is_empty());
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
                GraphEdge {
                    from: 1,
                    to: 2,
                    grounding: None,
                },
                GraphEdge {
                    from: 2,
                    to: 3,
                    grounding: None,
                },
                GraphEdge {
                    from: 1,
                    to: 4,
                    grounding: None,
                },
                GraphEdge {
                    from: 4,
                    to: 3,
                    grounding: None,
                },
            ],
        };
        let p = permit(admin_traversal_ir("endpoint.http", "persistence.write"));
        let outcome = GraphBackend.run(p.admitted(), &graph_input(graph)).unwrap();
        assert_eq!(outcome.matches.len(), 1, "the clean path must be found");
        let detail: String = outcome
            .produced_evidence
            .iter()
            .map(|e| e.detail.clone())
            .collect::<Vec<_>>()
            .join(" | ");
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
