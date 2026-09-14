//! Program Analysis Service — facade for running M5 algorithms (WU1).
//!
//! Per design D2, the registry of algorithms lives in
//! `domain::analytics::program_analysis::*_descriptor`. This service is the
//! application-layer entry point that resolves an algorithm id, validates
//! params, applies PlanLimits, and dispatches to the registered descriptor.
//!
//! The actual algorithm execution (CFG extraction, slicing, etc.) is added in
//! WU2–WU5; WU1 establishes the dispatch surface and testability scaffold.

use std::sync::Arc;

use crate::domain::analytics::descriptor::{
    AlgorithmDescriptor, AlgorithmId, AnalyticsError, RunOutput,
};
use crate::domain::analytics::program_analysis::{
    CFG_PER_FUNCTION, DOMINATORS_CFG, SLICE_BACKWARD, SLICE_FORWARD, TAINT_FLOW,
    cfg_descriptor::CfgDescriptor, dominators_cfg_descriptor::DominatorsCfgDescriptor,
    slicing_descriptor::SlicingDescriptor, taint_descriptor::TaintDescriptor,
};
#[cfg(feature = "program-analysis-server")]
use crate::domain::analytics::program_analysis::{DFG, INTERPROC_SUMMARY};
use crate::domain::plan::limits::PlanLimits;

/// Service facade for running M5 program-analysis algorithms.
///
/// Construction is cheap; descriptors are stateless. The service holds a
/// vector of descriptors and dispatches by id.
pub struct ProgramAnalysisService {
    descriptors: Vec<Arc<dyn AlgorithmDescriptor>>,
}

/// Adapter that exposes the canonical `SlicingDescriptor` under the
/// `slice_backward` algorithm id (forward + backward share the same descriptor).
struct SlicingBackwardAdapter(SlicingDescriptor);

impl AlgorithmDescriptor for SlicingBackwardAdapter {
    fn identity(&self) -> &crate::domain::analytics::descriptor::AlgorithmIdentity {
        // Returns a static identity referencing `slice_backward`. We leak the
        // boxed identity once at first call so the &'static lifetime is honest.
        use std::sync::OnceLock;
        static IDENTITY: OnceLock<Box<crate::domain::analytics::descriptor::AlgorithmIdentity>> =
            OnceLock::new();
        IDENTITY.get_or_init(|| {
            Box::new(crate::domain::analytics::descriptor::AlgorithmIdentity {
                id: SLICE_BACKWARD.clone(),
                version: crate::domain::analytics::descriptor::AlgorithmVersion::v1(),
                maturity: crate::domain::analytics::descriptor::Maturity::Experimental,
                cohort: 5,
            })
        })
    }

    fn params(&self) -> &dyn crate::domain::analytics::descriptor::AlgorithmParams {
        self.0.params()
    }

    fn output_schema(&self) -> &crate::domain::analytics::descriptor::OutputSchema {
        self.0.output_schema()
    }

    fn supported_modes(&self) -> &[crate::domain::analytics::descriptor::AnalyticsMode] {
        self.0.supported_modes()
    }

    fn complexity(&self) -> &crate::domain::analytics::descriptor::ComplexityClass {
        self.0.complexity()
    }

    fn limits(&self) -> &crate::domain::plan::limits::PlanLimits {
        self.0.limits()
    }

    fn conformance_fixtures(&self) -> &[crate::domain::analytics::descriptor::Fixture] {
        self.0.conformance_fixtures()
    }

    fn determinism(&self) -> crate::domain::analytics::descriptor::DeterminismKind {
        self.0.determinism()
    }

    fn directed(&self) -> bool {
        self.0.directed()
    }
    fn weighted(&self) -> bool {
        self.0.weighted()
    }
    fn heterogeneous(&self) -> bool {
        self.0.heterogeneous()
    }
    fn projection_assumption(&self) -> &crate::domain::analytics::descriptor::ProjectionAssumption {
        self.0.projection_assumption()
    }
}

impl Default for ProgramAnalysisService {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgramAnalysisService {
    /// Construct a service with the default M5 descriptor set.
    ///
    /// DFG / interprocedural summary descriptors are added under the
    /// `program-analysis-server` feature flag (per design D8).
    pub fn new() -> Self {
        #[allow(unused_mut)]
        let mut descriptors: Vec<Arc<dyn AlgorithmDescriptor>> = vec![
            Arc::new(CfgDescriptor),
            Arc::new(DominatorsCfgDescriptor),
            // Slicing descriptor registered under both algorithm ids (forward + backward).
            Arc::new(SlicingDescriptor),
            Arc::new(SlicingBackwardAdapter(SlicingDescriptor)),
            Arc::new(TaintDescriptor),
        ];

        #[cfg(feature = "program-analysis-server")]
        {
            use crate::domain::analytics::program_analysis::{
                dfg_descriptor::DfgDescriptor,
                interproc_summaries_descriptor::InterprocSummaryDescriptor,
            };
            descriptors.push(Arc::new(DfgDescriptor));
            descriptors.push(Arc::new(InterprocSummaryDescriptor));
        }

        Self { descriptors }
    }

    /// Returns the number of descriptors registered (varies by feature flags).
    pub fn registered_count(&self) -> usize {
        self.descriptors.len()
    }

    /// Returns true if the given algorithm id is registered.
    pub fn supports(&self, id: &AlgorithmId) -> bool {
        self.descriptors.iter().any(|d| d.identity().id == *id)
    }

    /// Returns all registered algorithm ids, in deterministic order.
    pub fn registered_ids(&self) -> Vec<AlgorithmId> {
        let mut ids: Vec<AlgorithmId> = self
            .descriptors
            .iter()
            .map(|d| d.identity().id.clone())
            .collect();
        ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        ids
    }

    /// Look up a descriptor by id.
    pub fn descriptor(&self, id: &AlgorithmId) -> Option<&dyn AlgorithmDescriptor> {
        self.descriptors
            .iter()
            .find(|d| d.identity().id == *id)
            .map(|d| d.as_ref())
    }

    /// Validate params against a descriptor. Returns `Ok(())` if valid.
    pub fn validate(&self, id: &AlgorithmId, params: &serde_json::Value) -> Result<(), String> {
        let desc = self
            .descriptor(id)
            .ok_or_else(|| format!("unsupported algorithm: {}", id))?;
        desc.params().validate(params)
    }

    /// Dispatch surface for algorithm execution.
    ///
    /// WU2 wires real execution paths for `cfg_per_function` and
    /// `dominators_cfg`. Both consume the same `adjacency` JSON parameter
    /// (a flat list of out-neighbor lists) plus a `root` block id; the
    /// `cfg_per_function` algorithm additionally accepts `exits`. Output
    /// is JSON-encoded for transport through `RunOutput::PageRank`.
    pub fn dispatch(
        &self,
        id: &AlgorithmId,
        params: &serde_json::Value,
        limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        if !self.supports(id) {
            return Err(AnalyticsError::Internal(format!(
                "unsupported algorithm: {}",
                id
            )));
        }
        self.validate(id, params)
            .map_err(AnalyticsError::InvalidParameter)?;

        // WU2: real CFG + dominators execution paths.
        if id == &*CFG_PER_FUNCTION {
            return self.run_cfg(params, limits);
        }
        if id == &*DOMINATORS_CFG {
            return self.run_dominators_cfg(params, limits);
        }

        // WU3: forward + backward slicing. Both ids share the same runtime
        // path; the descriptor layer is the only place that knows about the
        // forward/backward distinction at the param-validation level.
        if id == &*SLICE_FORWARD {
            return self.run_slice(params, limits, /*backward=*/ false);
        }
        if id == &*SLICE_BACKWARD {
            return self.run_slice(params, limits, /*backward=*/ true);
        }

        // WU5: taint placeholder (handled in WU5).
        if id == &*TAINT_FLOW {
            return self.run_taint(params, limits);
        }

        #[cfg(feature = "program-analysis-server")]
        {
            if id == &*DFG {
                return self.run_dfg(params, limits);
            }
            // WU4: interprocedural summaries.
            if id == &*INTERPROC_SUMMARY {
                return self.run_interproc_summary(params, limits);
            }
        }

        // Anything else (shouldn't reach here, but stay total).
        Ok(RunOutput::PageRank(serde_json::json!({
            "algorithm": id.as_str(),
            "status": "unhandled",
        })))
    }

    fn run_cfg(
        &self,
        params: &serde_json::Value,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        use cognicode_graph_algos::algorithms::{build_cfg, edge_count};
        let adjacency = parse_adjacency(params)?;
        let (root, exits) = parse_root_and_exits(params)?;
        let cfg = build_cfg(&adjacency, root, &exits);
        let json = serde_json::json!({
            "algorithm": "cfg_per_function",
            "entry": cfg.entry,
            "blocks": cfg.blocks,
            "edges": cfg.edges,
            "block_count": cfg.blocks.len(),
            "edge_count": edge_count(&cfg),
        });
        Ok(RunOutput::PageRank(json))
    }

    fn run_dominators_cfg(
        &self,
        params: &serde_json::Value,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        use cognicode_graph_algos::algorithms::dominators_cfg;
        let adjacency = parse_adjacency(params)?;
        let (root, _exits) = parse_root_and_exits(params)?;
        let dom = dominators_cfg(&adjacency, root);
        let json = serde_json::json!({
            "algorithm": "dominators_cfg",
            "entry": root,
            "dominators": dom,
        });
        Ok(RunOutput::PageRank(json))
    }

    /// WU3 forward/backward slice over the supplied CFG.
    ///
    /// Required params (validated by `SlicingParams`):
    /// - `variable`: string name of interest
    /// - `definition_site`: block id where the definition lives
    /// - `adjacency`: flat out-neighbor list
    /// - `use_sites` (optional, backward only): list of block ids that
    ///   contain a use of `variable`; defaults to `[definition_site]`
    ///   for forward and `[]` for backward.
    fn run_slice(
        &self,
        params: &serde_json::Value,
        _limits: &PlanLimits,
        backward: bool,
    ) -> Result<RunOutput, AnalyticsError> {
        use cognicode_graph_algos::algorithms::{backward_slice, forward_slice};
        let variable = params
            .get("variable")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AnalyticsError::InvalidParameter("missing 'variable'".into()))?
            .to_string();
        let definition_site = params
            .get("definition_site")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| AnalyticsError::InvalidParameter("missing 'definition_site'".into()))?
            as usize;
        let adjacency = parse_adjacency(params)?;

        let nodes: Vec<usize> = if backward {
            let use_sites: Vec<usize> = params
                .get("use_sites")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default();
            backward_slice(&adjacency, &use_sites)
        } else {
            forward_slice(&adjacency, definition_site)
        };

        let json = serde_json::json!({
            "algorithm": if backward { "slice_backward" } else { "slice_forward" },
            "variable": variable,
            "definition_site": definition_site,
            "direction": if backward { "backward" } else { "forward" },
            "nodes": nodes,
        });
        Ok(RunOutput::PageRank(json))
    }

    /// WU3 DFG: intra-procedural def→use edges from a flat statement list.
    fn run_dfg(
        &self,
        params: &serde_json::Value,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        use cognicode_graph_algos::algorithms::{DefUseEdge, Statement, dfg_edges};
        let stmts_json = params
            .get("statements")
            .ok_or_else(|| AnalyticsError::InvalidParameter("missing 'statements'".into()))?
            .as_array()
            .ok_or_else(|| {
                AnalyticsError::InvalidParameter("'statements' must be a JSON array".into())
            })?;
        let statements: Vec<Statement> =
            serde_json::from_value(serde_json::Value::Array(stmts_json.clone()))
                .map_err(|e| AnalyticsError::InvalidParameter(format!("bad statement: {e}")))?;
        let edges: Vec<DefUseEdge> = dfg_edges(&statements);
        let json = serde_json::json!({
            "algorithm": "dfg",
            "function_id": params.get("function_id"),
            "edge_count": edges.len(),
            "edges": edges,
        });
        Ok(RunOutput::PageRank(json))
    }

    /// WU5 taint placeholder. Real implementation lands in WU5.
    fn run_taint(
        &self,
        params: &serde_json::Value,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        // Until WU5, the descriptor's params are validated and we return a
        // structured "stub" response so callers can already wire their
        // requests through the dispatcher.
        let sources: Vec<serde_json::Value> = params
            .get("sources")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let sinks: Vec<serde_json::Value> = params
            .get("sinks")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(RunOutput::PageRank(serde_json::json!({
            "algorithm": "taint_flow",
            "status": "stub",
            "note": "WU5 wires the declared-pattern matching; descriptor already validates params.",
            "source_count": sources.len(),
            "sink_count": sinks.len(),
        })))
    }

    /// WU4 interprocedural summary placeholder.
    #[cfg(feature = "program-analysis-server")]
    fn run_interproc_summary(
        &self,
        _params: &serde_json::Value,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        Ok(RunOutput::PageRank(serde_json::json!({
            "algorithm": "interproc_summary",
            "status": "stub",
            "note": "WU4 wires the call-graph + summary cache.",
        })))
    }

    /// Convenience: returns the full canonical list of M5 algorithm ids.
    pub fn m5_ids(&self) -> Vec<AlgorithmId> {
        let mut ids = vec![
            CFG_PER_FUNCTION.clone(),
            DOMINATORS_CFG.clone(),
            SLICE_FORWARD.clone(),
            SLICE_BACKWARD.clone(),
            TAINT_FLOW.clone(),
        ];
        #[cfg(feature = "program-analysis-server")]
        {
            ids.push(DFG.clone());
            ids.push(INTERPROC_SUMMARY.clone());
        }
        ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        ids
    }
}

/// Parse the flat adjacency list from the dispatch params.
///
/// Expected shape:
/// ```json
/// { "adjacency": [[1, 2], [3], [], [4]], "root": 0, "exits": [4] }
/// ```
///
/// `exits` is optional; defaults to an empty list. `root` defaults to 0.
fn parse_adjacency(params: &serde_json::Value) -> Result<Vec<Vec<usize>>, AnalyticsError> {
    let adjacency = params
        .get("adjacency")
        .ok_or_else(|| AnalyticsError::InvalidParameter("missing 'adjacency'".into()))?;
    let arr = adjacency.as_array().ok_or_else(|| {
        AnalyticsError::InvalidParameter("'adjacency' must be a JSON array".into())
    })?;
    let mut out: Vec<Vec<usize>> = Vec::with_capacity(arr.len());
    for (i, row) in arr.iter().enumerate() {
        let row_arr = row.as_array().ok_or_else(|| {
            AnalyticsError::InvalidParameter(format!("adjacency[{i}] must be a JSON array"))
        })?;
        let mut nbrs: Vec<usize> = Vec::with_capacity(row_arr.len());
        for v in row_arr {
            let n = v.as_u64().ok_or_else(|| {
                AnalyticsError::InvalidParameter(format!(
                    "adjacency[{i}] elements must be non-negative integers"
                ))
            })?;
            nbrs.push(n as usize);
        }
        out.push(nbrs);
    }
    Ok(out)
}

fn parse_root_and_exits(params: &serde_json::Value) -> Result<(usize, Vec<usize>), AnalyticsError> {
    let root = params
        .get("root")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AnalyticsError::InvalidParameter("missing or invalid 'root'".into()))?
        as usize;
    let exits: Vec<usize> = params
        .get("exits")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();
    Ok((root, exits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_starts_with_baseline_descriptors() {
        let svc = ProgramAnalysisService::new();
        assert!(
            svc.registered_count() >= 4,
            "expected ≥4 baseline descriptors (cfg/dominators/slicing/taint)"
        );
        assert!(svc.supports(&CFG_PER_FUNCTION));
        assert!(svc.supports(&DOMINATORS_CFG));
        // The slicing descriptor represents both forward and backward slicing
        // (single canonical algorithm id "slice_forward"; backward is a flag
        // in params). Both algorithm ids must report supported.
        assert!(svc.supports(&SLICE_FORWARD));
        assert!(svc.supports(&SLICE_BACKWARD));
        assert!(svc.supports(&TAINT_FLOW));
    }

    #[test]
    fn service_validates_params() {
        let svc = ProgramAnalysisService::new();
        let good = serde_json::json!({"function_id": "fn_42"});
        assert!(svc.validate(&CFG_PER_FUNCTION, &good).is_ok());
        let bad = serde_json::json!({});
        assert!(svc.validate(&CFG_PER_FUNCTION, &bad).is_err());
    }

    #[test]
    fn service_rejects_unsupported_algorithm() {
        let svc = ProgramAnalysisService::new();
        let unknown = AlgorithmId::from_static("does_not_exist");
        assert!(!svc.supports(&unknown));
        let res = svc.dispatch(&unknown, &serde_json::json!({}), &PlanLimits::default());
        assert!(res.is_err());
    }

    #[test]
    fn service_m5_ids_are_sorted_and_unique() {
        let svc = ProgramAnalysisService::new();
        let ids = svc.m5_ids();
        let mut sorted = ids.clone();
        sorted.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        assert_eq!(ids, sorted);
        let mut unique = ids.clone();
        unique.dedup();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn dispatch_succeeds_for_supported_algorithm() {
        let svc = ProgramAnalysisService::new();
        // WU2 dispatch: CFG runs on adjacency + root (function_id is a
        // contractual no-op here; the descriptor still requires it for
        // forward compatibility with tree-sitter wiring in M5.1).
        let res = svc.dispatch(
            &CFG_PER_FUNCTION,
            &serde_json::json!({
                "function_id": "f1",
                "adjacency": [[1], [2], []],
                "root": 0,
                "exits": [2],
            }),
            &PlanLimits::default(),
        );
        assert!(res.is_ok(), "dispatch should succeed: {res:?}");
    }

    #[test]
    fn dispatch_dominators_cfg_end_to_end() {
        let svc = ProgramAnalysisService::new();
        let res = svc
            .dispatch(
                &DOMINATORS_CFG,
                &serde_json::json!({
                    "function_id": "diamond",
                    "cfg_digest": "sha256:test",
                    "adjacency": [[1], [2, 3], [4], [4], []],
                    "root": 0,
                }),
                &PlanLimits::default(),
            )
            .expect("dispatch should succeed");
        // RunOutput::PageRank wraps a JSON value; extract the dominator info.
        match res {
            RunOutput::PageRank(v) => {
                let dom = v.get("dominators").expect("dominators key present");
                let arr = dom.as_array().expect("dominators is an array");
                assert_eq!(arr.len(), 5, "diamond has 5 blocks");
                // entry self-dominates
                let entry_dom = arr
                    .iter()
                    .find(|x| x.get("block_id").and_then(|b| b.as_u64()) == Some(0))
                    .expect("entry present");
                assert_eq!(
                    entry_dom
                        .get("immediate_dominator")
                        .and_then(|v| v.as_u64()),
                    Some(0)
                );
            }
            _ => panic!("expected RunOutput::PageRank for dominators"),
        }
    }

    #[test]
    fn dispatch_forward_slice_end_to_end() {
        let svc = ProgramAnalysisService::new();
        // Linear chain 0 -> 1 -> 2; forward slice from 0 should return all.
        let res = svc
            .dispatch(
                &SLICE_FORWARD,
                &serde_json::json!({
                    "function_id": "linear",
                    "variable": "x",
                    "definition_site": 0,
                    "adjacency": [[1], [2], []],
                }),
                &PlanLimits::default(),
            )
            .expect("dispatch should succeed");
        match res {
            RunOutput::PageRank(v) => {
                let nodes: Vec<usize> = v
                    .get("nodes")
                    .and_then(|x| x.as_array())
                    .expect("nodes array")
                    .iter()
                    .filter_map(|n| n.as_u64().map(|u| u as usize))
                    .collect();
                assert_eq!(nodes, vec![0, 1, 2]);
                assert_eq!(v.get("direction").and_then(|x| x.as_str()), Some("forward"));
            }
            _ => panic!("expected RunOutput::PageRank for slicing"),
        }
    }

    #[test]
    fn dispatch_backward_slice_end_to_end() {
        let svc = ProgramAnalysisService::new();
        // Diamond: 0 -> 1; 1 -> 2, 3; 2 -> 4; 3 -> 4; use_sites = [4].
        let res = svc
            .dispatch(
                &SLICE_BACKWARD,
                &serde_json::json!({
                    "function_id": "diamond",
                    "variable": "x",
                    "definition_site": 0,
                    "adjacency": [[1], [2, 3], [4], [4], []],
                    "use_sites": [4],
                }),
                &PlanLimits::default(),
            )
            .expect("dispatch should succeed");
        match res {
            RunOutput::PageRank(v) => {
                let nodes: Vec<usize> = v
                    .get("nodes")
                    .and_then(|x| x.as_array())
                    .expect("nodes array")
                    .iter()
                    .filter_map(|n| n.as_u64().map(|u| u as usize))
                    .collect();
                assert_eq!(nodes, vec![0, 1, 2, 3, 4]);
                assert_eq!(
                    v.get("direction").and_then(|x| x.as_str()),
                    Some("backward")
                );
            }
            _ => panic!("expected RunOutput::PageRank for slicing"),
        }
    }

    #[test]
    #[cfg(feature = "program-analysis-server")]
    fn dispatch_dfg_end_to_end() {
        let svc = ProgramAnalysisService::new();
        // x = 1; y = x; z = y
        let res = svc
            .dispatch(
                &DFG,
                &serde_json::json!({
                    "function_id": "linear_dfg",
                    "cfg_digest": "sha256:linear",
                    "statements": [
                        {"id": 0, "defs": ["x"], "uses": []},
                        {"id": 1, "defs": ["y"], "uses": ["x"]},
                        {"id": 2, "defs": ["z"], "uses": ["y"]},
                    ],
                }),
                &PlanLimits::default(),
            )
            .expect("dispatch should succeed");
        match res {
            RunOutput::PageRank(v) => {
                assert_eq!(v.get("edge_count").and_then(|x| x.as_u64()), Some(2));
                let edges = v
                    .get("edges")
                    .and_then(|x| x.as_array())
                    .expect("edges array");
                assert_eq!(edges.len(), 2);
                // Sorted by (from, to, variable) — (0, 1, x) then (1, 2, y).
                let first = &edges[0];
                assert_eq!(first.get("from").and_then(|x| x.as_u64()), Some(0));
                assert_eq!(first.get("to").and_then(|x| x.as_u64()), Some(1));
                assert_eq!(first.get("variable").and_then(|x| x.as_str()), Some("x"));
            }
            _ => panic!("expected RunOutput::PageRank for DFG"),
        }
    }
}
