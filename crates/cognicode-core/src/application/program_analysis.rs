//! Program Analysis Service — facade for running M5 algorithms (WU1).
//!
//! Per design D2, the registry of algorithms lives in
//! `domain::analytics::program_analysis::*_descriptor`. This service is the
//! application-layer entry point that resolves an algorithm id, validates
//! params, applies PlanLimits, and dispatches to the registered descriptor.
//!
//! The actual algorithm execution (CFG extraction, slicing, etc.) is added in
//! WU2–WU5; WU1 establishes the dispatch surface and testability scaffold.
//!
//! WU6 adds the conformance harness as a sibling module under
//! `program_analysis/` (see `conformance`).

pub mod conformance;

#[cfg(feature = "program-analysis-server")]
pub mod ast_lift;

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

/// A statement in a typed taint-flow request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintFlowStatement {
    /// Stable statement id within the function.
    pub id: usize,
    /// Statement kind tag (free-form).
    pub kind: String,
    /// Variables defined.
    pub defs: Vec<String>,
    /// Variables used.
    pub uses: Vec<String>,
}

/// Typed request for the forward-taint algorithm.
///
/// This is the *application* contract: MCP/API present it as JSON, M6 passes
/// it as typed data. There is exactly one implementation behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintFlowRequest {
    /// Function identity.
    pub function_id: String,
    /// DFG digest of the function (pinned by the caller).
    pub dfg_digest: String,
    /// The function's statements.
    pub statements: Vec<TaintFlowStatement>,
    /// Statement ids that are source sites.
    pub sources: Vec<usize>,
    /// Statement ids that are sink sites.
    pub sinks: Vec<usize>,
    /// Statement ids whose defs are sanitised.
    pub untaints: Vec<usize>,
}

/// One typed taint path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintFlowPath {
    /// Source statement id.
    pub source: usize,
    /// Sink statement id.
    pub sink: usize,
    /// Intermediates strictly between source and sink.
    pub intermediates: Vec<usize>,
}

/// Typed result of a forward-taint run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintFlowResult {
    /// Detected paths, sorted by `(source, sink)`.
    pub paths: Vec<TaintFlowPath>,
    /// All statements reached by any source.
    pub tainted_statements: Vec<usize>,
    /// Statements treated as sanitise sites.
    pub untaint_statements: Vec<usize>,
}

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
    #[cfg(feature = "program-analysis-server")]
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

    /// WU5 taint analysis: forward propagation over the DFG from declared
    /// sources to sinks, with untaint sites clearing tainted state.
    ///
    /// Required params (validated by `TaintParams`):
    /// - `function_id`, `dfg_digest`
    ///
    /// Algorithm-specific params (algorithm-specific, validated by the
    /// descriptor contract — see `TaintPatterns::rust_v1()` for the
    /// declared Rust v1 pattern set):
    /// - `language`: e.g. `"rust"`. Currently the algorithm does not
    ///   match patterns itself; the source/sink site lists come in
    ///   pre-classified form.
    /// - `statements`: the DFG input (same shape as WU3)
    /// - `sources`: list of statement ids that are source sites
    /// - `sinks`: list of statement ids that are sink sites
    /// - `untaints`: list of statement ids that contain an untaint call
    ///
    /// Typed forward-taint entry point.
    ///
    /// `dispatch(TAINT_FLOW)` delegates here; M6 backends call it directly.
    /// One implementation, two presentations.
    ///
    /// # Enforced limits
    ///
    /// Deterministic, cheaply checkable bounds are enforced and **error**
    /// (never truncate silently):
    ///
    /// - `max_visited_nodes` — number of statements supplied;
    /// - `max_visited_edges` — DFG edge count;
    /// - `max_path_count` (or, failing that, `max_result_rows`) — number of
    ///   reported paths.
    ///
    /// # Not enforced (declared honestly)
    ///
    /// `time_ms`, `cancellation`, `max_memory_bytes`, `max_depth` and
    /// `max_hops` are **not** enforced by this v1 facade. Callers must not
    /// assume they are.
    pub fn taint_flow(
        &self,
        request: &TaintFlowRequest,
        limits: &PlanLimits,
    ) -> Result<TaintFlowResult, AnalyticsError> {
        use crate::domain::plan::limits::PlanLimitKind;
        use cognicode_graph_algos::algorithms::{Statement, dfg_edges, taint_forward};

        let statements: Vec<Statement> = request
            .statements
            .iter()
            .map(|s| Statement {
                id: s.id,
                kind: s.kind.clone(),
                defs: s.defs.clone(),
                uses: s.uses.clone(),
            })
            .collect();

        if let Some(max) = limits.max_visited_nodes
            && statements.len() as u64 > max
        {
            return Err(AnalyticsError::LimitExceeded(
                PlanLimitKind::MaxVisitedNodes,
            ));
        }

        let edges = dfg_edges(&statements);
        if let Some(max) = limits.max_visited_edges
            && edges.len() as u64 > max
        {
            return Err(AnalyticsError::LimitExceeded(
                PlanLimitKind::MaxVisitedEdges,
            ));
        }

        let result = taint_forward(&edges, &request.sources, &request.sinks, &request.untaints);

        // Path-count bound: a pathological analysis must fail, not truncate.
        if let Some(max) = limits.max_path_count {
            if result.paths.len() as u64 > max {
                return Err(AnalyticsError::LimitExceeded(PlanLimitKind::MaxPathCount));
            }
        } else if let Some(max) = limits.max_result_rows
            && result.paths.len() as u64 > max
        {
            return Err(AnalyticsError::LimitExceeded(PlanLimitKind::MaxResultRows));
        }

        Ok(TaintFlowResult {
            paths: result
                .paths
                .iter()
                .map(|p| TaintFlowPath {
                    source: p.source.stmt_id,
                    sink: p.sink.stmt_id,
                    intermediates: p.intermediates.iter().map(|s| s.stmt_id).collect(),
                })
                .collect(),
            tainted_statements: result.tainted_statements,
            untaint_statements: result.untaint_statements,
        })
    }

    fn run_taint(
        &self,
        params: &serde_json::Value,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        use cognicode_graph_algos::algorithms::Statement;

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

        let parse_id_list = |key: &str| -> Result<Vec<usize>, AnalyticsError> {
            Ok(params
                .get(key)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default())
        };
        let sources = parse_id_list("sources")?;
        let sinks = parse_id_list("sinks")?;
        let untaints = parse_id_list("untaints")?;

        // One implementation: delegate to the typed facade.
        let request = TaintFlowRequest {
            function_id: params
                .get("function_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            dfg_digest: params
                .get("dfg_digest")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            statements: statements
                .iter()
                .map(|s| TaintFlowStatement {
                    id: s.id,
                    kind: s.kind.clone(),
                    defs: s.defs.clone(),
                    uses: s.uses.clone(),
                })
                .collect(),
            sources,
            sinks,
            untaints,
        };
        let result = self.taint_flow(&request, _limits)?;

        // JSON presentation of the same typed result.
        let paths_json: Vec<serde_json::Value> = result
            .paths
            .iter()
            .map(|p| {
                serde_json::json!({
                    "source": p.source,
                    "sink": p.sink,
                    "intermediates": p.intermediates,
                })
            })
            .collect();

        Ok(RunOutput::PageRank(serde_json::json!({
            "algorithm": "taint_flow",
            "function_id": params.get("function_id"),
            "language": params.get("language"),
            "path_count": paths_json.len(),
            "paths": paths_json,
            "tainted_statements": result.tainted_statements,
            "untaint_statements": result.untaint_statements,
        })))
    }

    /// WU4 interprocedural summary: bottom-up accumulation of reads,
    /// writes, and calls per function, with fixed-point on recursive SCCs.
    ///
    /// Required params (validated by `InterprocSummaryParams`):
    /// - `function_id`: any non-empty string (used as the cache key prefix).
    ///
    /// Optional params:
    /// - `call_graph`: `Vec<Vec<usize>>` of callee indices per function
    /// - `functions`: list of `{function_id, statements, calls}` records
    ///   (`statements` follows the DFG shape from WU3)
    #[cfg(feature = "program-analysis-server")]
    fn run_interproc_summary(
        &self,
        params: &serde_json::Value,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        use cognicode_graph_algos::algorithms::{FunctionLocalView, compute_summaries, summary_id};

        // Default: empty inputs → empty summary list.
        let call_graph: Vec<Vec<usize>> = params
            .get("call_graph")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|row| {
                        row.as_array()
                            .map(|xs| {
                                xs.iter()
                                    .filter_map(|x| x.as_u64().map(|n| n as usize))
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let functions_json = params
            .get("functions")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let functions: Vec<FunctionLocalView> =
            serde_json::from_value(serde_json::Value::Array(functions_json))
                .map_err(|e| AnalyticsError::InvalidParameter(format!("bad functions: {e}")))?;

        let summaries = compute_summaries(&call_graph, &functions);

        // Build the JSON output, attaching a per-summary id.
        let summaries_json: Vec<serde_json::Value> = summaries
            .iter()
            .map(|s| {
                serde_json::json!({
                    "function_id": s.function_id,
                    "kind": s.kind,
                    "reads": s.reads,
                    "writes": s.writes,
                    "calls": s.calls,
                    "summary_id": summary_id(s),
                })
            })
            .collect();

        Ok(RunOutput::PageRank(serde_json::json!({
            "algorithm": "interproc_summary",
            "function_id": params.get("function_id"),
            "summary_count": summaries.len(),
            "summaries": summaries_json,
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

    #[test]
    #[cfg(feature = "program-analysis-server")]
    fn dispatch_interproc_summary_end_to_end() {
        let svc = ProgramAnalysisService::new();
        // foo (idx 0, id 1) calls bar (idx 1, id 2); bar is a leaf.
        let res = svc
            .dispatch(
                &INTERPROC_SUMMARY,
                &serde_json::json!({
                    "function_id": "module",
                    "call_graph": [[1], []],
                    "functions": [
                        {
                            "function_id": 1,
                            "statements": [{"id": 0, "defs": ["x"], "uses": []}],
                            "calls": [1],
                        },
                        {
                            "function_id": 2,
                            "statements": [{"id": 0, "defs": ["y"], "uses": ["z"]}],
                            "calls": [],
                        },
                    ],
                }),
                &PlanLimits::default(),
            )
            .expect("dispatch should succeed");
        match res {
            RunOutput::PageRank(v) => {
                assert_eq!(v.get("summary_count").and_then(|x| x.as_u64()), Some(2));
                let summaries = v
                    .get("summaries")
                    .and_then(|x| x.as_array())
                    .expect("summaries array");
                let foo = summaries
                    .iter()
                    .find(|s| s.get("function_id").and_then(|x| x.as_u64()) == Some(1))
                    .expect("foo summary");
                let bar = summaries
                    .iter()
                    .find(|s| s.get("function_id").and_then(|x| x.as_u64()) == Some(2))
                    .expect("bar summary");
                assert_eq!(foo.get("kind").and_then(|x| x.as_str()), Some("bottom_up"));
                assert_eq!(bar.get("kind").and_then(|x| x.as_str()), Some("leaf"));
                // foo inherits bar's writes (y) and reads (z).
                let foo_writes: Vec<&str> = foo
                    .get("writes")
                    .and_then(|x| x.as_array())
                    .expect("writes array")
                    .iter()
                    .filter_map(|x| x.as_str())
                    .collect();
                assert!(foo_writes.contains(&"x"));
                assert!(foo_writes.contains(&"y"));
                let foo_reads: Vec<&str> = foo
                    .get("reads")
                    .and_then(|x| x.as_array())
                    .expect("reads array")
                    .iter()
                    .filter_map(|x| x.as_str())
                    .collect();
                assert!(foo_reads.contains(&"z"));
                // summary_id is a 64-char hex string.
                assert_eq!(
                    foo.get("summary_id")
                        .and_then(|x| x.as_str())
                        .map(|s| s.len()),
                    Some(64)
                );
            }
            _ => panic!("expected RunOutput::PageRank for interproc_summary"),
        }
    }

    #[test]
    fn dispatch_taint_end_to_end() {
        let svc = ProgramAnalysisService::new();
        // x = src(); y = x; sink(y);
        let res = svc
            .dispatch(
                &TAINT_FLOW,
                &serde_json::json!({
                    "function_id": "rust_fn",
                    "dfg_digest": "sha256:linear",
                    "language": "rust",
                    "statements": [
                        {"id": 0, "defs": ["x"], "uses": ["src"]},
                        {"id": 1, "defs": ["y"], "uses": ["x"]},
                        {"id": 2, "defs": [], "uses": ["y"]},
                    ],
                    "sources": [0],
                    "sinks": [2],
                    "untaints": [],
                }),
                &PlanLimits::default(),
            )
            .expect("dispatch should succeed");
        match res {
            RunOutput::PageRank(v) => {
                assert_eq!(v.get("path_count").and_then(|x| x.as_u64()), Some(1));
                let paths = v
                    .get("paths")
                    .and_then(|x| x.as_array())
                    .expect("paths array");
                assert_eq!(paths.len(), 1);
                let path = &paths[0];
                assert_eq!(path.get("source").and_then(|x| x.as_u64()), Some(0));
                assert_eq!(path.get("sink").and_then(|x| x.as_u64()), Some(2));
                let intermediates: Vec<usize> = path
                    .get("intermediates")
                    .and_then(|x| x.as_array())
                    .expect("intermediates array")
                    .iter()
                    .filter_map(|x| x.as_u64().map(|n| n as usize))
                    .collect();
                assert_eq!(intermediates, vec![1]);
                let tainted: Vec<usize> = v
                    .get("tainted_statements")
                    .and_then(|x| x.as_array())
                    .expect("tainted array")
                    .iter()
                    .filter_map(|x| x.as_u64().map(|n| n as usize))
                    .collect();
                assert_eq!(tainted, vec![0, 1, 2]);
            }
            _ => panic!("expected RunOutput::PageRank for taint"),
        }
    }

    #[test]
    fn dispatch_taint_breaks_on_untaint() {
        let svc = ProgramAnalysisService::new();
        // x = src(); y = sanitize(x); sink(y);
        let res = svc
            .dispatch(
                &TAINT_FLOW,
                &serde_json::json!({
                    "function_id": "rust_fn",
                    "dfg_digest": "sha256:sanitized",
                    "language": "rust",
                    "statements": [
                        {"id": 0, "defs": ["x"], "uses": ["src"]},
                        {"id": 1, "defs": ["y"], "uses": ["x"]},
                        {"id": 2, "defs": [], "uses": ["y"]},
                    ],
                    "sources": [0],
                    "sinks": [2],
                    "untaints": [1],
                }),
                &PlanLimits::default(),
            )
            .expect("dispatch should succeed");
        match res {
            RunOutput::PageRank(v) => {
                // Untaint breaks the path → no taint paths detected.
                assert_eq!(v.get("path_count").and_then(|x| x.as_u64()), Some(0));
                let untaint_stmts: Vec<usize> = v
                    .get("untaint_statements")
                    .and_then(|x| x.as_array())
                    .expect("untaint array")
                    .iter()
                    .filter_map(|x| x.as_u64().map(|n| n as usize))
                    .collect();
                assert_eq!(untaint_stmts, vec![1]);
            }
            _ => panic!("expected RunOutput::PageRank for taint"),
        }
    }
}

/// Real acceptance evidence for M5: exercises the PUBLIC
/// `ProgramAnalysisService` dispatcher end-to-end with the canonical corpus.
/// This is the same path MCP/CLI callers will use, not a synthetic harness.
///
/// Run with:
///   cargo test -p cognicode-core --features program-analysis-server --lib \
///     --nocapture program_analysis::acceptance_evidence
#[cfg(test)]
mod acceptance_evidence {
    use super::conformance::{
        canonical_corpus, mcp_fixture_report, perf_envelope, replay_guard, run_corpus,
    };

    /// Public algorithm IDs whose canonical fixtures MUST dispatch Ok.
    /// Mirrors the design-D2 contract.
    const REQUIRED_PUBLIC_ALGORITHMS: &[&str] = &[
        "cfg_per_function",
        "dominators_cfg",
        "slice_forward",
        "slice_backward",
        "taint_flow",
        // DEBT-SDDK-006: the canonical fixture for this algorithm is
        // #[cfg(feature = "program-analysis-server")] in conformance.rs,
        // so the requirement must carry the same gate or the default
        // build asserts a fixture that cannot exist.
        #[cfg(feature = "program-analysis-server")]
        "interproc_summary",
    ];

    #[test]
    fn public_dispatcher_accepts_every_m5_algorithm_id() {
        let svc = super::ProgramAnalysisService::new();
        let corpus = canonical_corpus();
        let present: std::collections::BTreeSet<&str> =
            corpus.iter().map(|f| f.algorithm).collect();

        for required in REQUIRED_PUBLIC_ALGORITHMS {
            assert!(
                present.contains(required),
                "canonical_corpus missing public algorithm `{required}`"
            );
        }

        let (outcomes, digests) = run_corpus(&svc, &corpus);
        for o in &outcomes {
            assert!(o.ok, "dispatch returned Err for fixture `{}`", o.label);
        }
        for d in &digests {
            assert_eq!(d.len(), 64, "digest is not sha256 hex: {d}");
            assert!(d.chars().all(|c| c.is_ascii_hexdigit()));
        }
        // Negative invariant: an unknown algorithm id MUST be rejected.
        use crate::domain::analytics::descriptor::AlgorithmId;
        let bogus_id = AlgorithmId::from_static("not_a_real_algorithm");
        let bogus = serde_json::json!({});
        let res = svc.dispatch(
            &bogus_id,
            &bogus,
            &crate::domain::plan::limits::PlanLimits::default(),
        );
        assert!(res.is_err(), "unknown algorithm id should error");
    }

    #[test]
    fn replay_guard_is_byte_identical_for_entire_corpus() {
        let svc = super::ProgramAnalysisService::new();
        let corpus = canonical_corpus();
        let results = replay_guard(&svc, &corpus);
        for r in &results {
            assert!(
                r.identical,
                "replay mismatch on {}: {} vs {}",
                r.label, r.digest_a, r.digest_b
            );
        }
    }

    #[test]
    fn perf_envelope_publishes_median_p95_max() {
        let svc = super::ProgramAnalysisService::new();
        let corpus = canonical_corpus();
        let summary = perf_envelope(&svc, &corpus, u128::MAX);
        assert_eq!(summary.total_count, corpus.len());
        assert_eq!(summary.per_fixture.len(), corpus.len());
        assert!(summary.median_us <= summary.max_us);
        assert!(summary.p95_us <= summary.max_us);
        assert_eq!(summary.over_budget_count, 0);
    }

    #[test]
    fn mcp_fixture_report_serde_round_trips() {
        let svc = super::ProgramAnalysisService::new();
        let corpus = canonical_corpus();
        let report = mcp_fixture_report(&svc, &corpus);
        let json = serde_json::to_string(&report).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        let arr = parsed.as_array().expect("array");
        assert_eq!(arr.len(), corpus.len());
        for entry in arr {
            for key in ["algorithm", "label", "digest", "elapsed_us", "ok"] {
                assert!(entry.get(key).is_some(), "missing key `{key}` in {entry}");
            }
        }
    }
}
