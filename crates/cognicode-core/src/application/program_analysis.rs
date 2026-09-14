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

        // WU3–WU5: forward + backward slicing, taint, DFG, summaries.
        Ok(RunOutput::PageRank(serde_json::json!({
            "algorithm": id.as_str(),
            "status": "stub",
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
}
