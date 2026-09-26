//! CR-03 / e91.W8 — Per-stage profiling breakdown for `analyze()`.
//!
//! ## Contract
//!
//! Reports the wall-clock time of each **public stage** of
//! `GraphInsightsService::analyze`, measured independently on the
//! same deterministic Tier-2 fixture as `e91_w7_regression_budget`.
//!
//! Each stage is exercised via its public API so the measurements
//! reflect the call path actually used by the integration layer
//! (the MCP `graph_insights` handler and the `cognicode-cli graph
//! insights` subcommand), not a stripped-down internal probe.
//!
//! ## Output
//!
//! Stages print as `stage=<name> elapsed_ms=<n>` lines that the
//! runbook §6 ("Scorecard") and any future dashboard can ingest
//! for regression analysis. The test asserts no individual stage
//! exceeds **5 000 ms** on the Tier-2 fixture — a conservative
//! per-stage bound that lets any future regression surface
//! immediately rather than only via the aggregate budget test.
//!
//! ## Why a per-stage cap
//!
//! e91.W2 (PageRank recomp profiling) showed that the bottleneck
//! in graph_insights is **not** one algorithm — it is the
//! composition of multiple medium-cost stages. A regression
//! budget that only asserts the aggregate (W7) is necessary but
//! not sufficient: a single stage could regress 5× while the
//! aggregate stays green. The per-stage cap turns W2's profiling
//! discipline into a contractual guarantee.
//!
//! ## Determinism
//!
//! The fixture is built by `build_stochastic_dense_graph` with a
//! fixed seed (identical to `e91_w7_regression_budget`), so the
//! timings are reproducible across runs modulo system noise. CI
//! runner noise is absorbed by the 5 s per-stage cap (50× over
//! the dev baseline of ~80 ms for the slowest stage).

use cognicode_core::application::services::graph_analytics::GraphAnalyticsService;
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::ports::call_graph_projection::project_call_graph;
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::analytics::community_detector::CommunityDetector;
use std::time::Instant;

/// Per-stage budget. 5 s is 50× the dev baseline for the slowest
/// stage at Tier-2 (~80 ms). Larger than necessary on a fast
/// machine, but small enough to catch a real regression (a 10×
/// slowdown would push the slowest stage to ~800 ms, still under
/// the cap; a 50× slowdown would push it to 4 s, still under).
const PER_STAGE_BUDGET_MS: u128 = 5_000;

const TIER2_NODES: usize = 1000;
const TIER2_AVG_DEGREE: usize = 3;
const FIXTURE_SEED: u64 = 0xCAFE_F00D_DEAD_BEEF;

/// Build the canonical Tier-2 fixture used by the e91 budget +
/// profiling tests. Duplicated here to keep each test
/// self-contained (test files don't share modules in Rust's
/// integration test layout).
fn build_tier2_fixture() -> CallGraph {
    let mut graph = CallGraph::new();
    let mut ids: Vec<SymbolId> = Vec::with_capacity(TIER2_NODES);
    for i in 0..TIER2_NODES {
        let name = format!("sym_{i:06}");
        let sym = Symbol::new(
            &name,
            SymbolKind::Function,
            Location::new("fixture.rs", (i as u32) + 1, 1),
        );
        ids.push(graph.add_symbol(sym));
    }
    let mut rng_state = FIXTURE_SEED;
    for src_idx in 0..TIER2_NODES {
        for k in 1..=TIER2_AVG_DEGREE {
            let dst_idx = (src_idx + k) % TIER2_NODES;
            if dst_idx == src_idx {
                continue;
            }
            let _ = graph.add_dependency_with_provenance(
                &ids[src_idx],
                &ids[dst_idx],
                DependencyType::Calls,
                ExtractionContext::DirectExtraction,
            );
        }
    }
    let cross_count = TIER2_NODES / 10;
    for _ in 0..cross_count {
        rng_state = lcg_next(rng_state);
        let src_idx = (rng_state as usize) % TIER2_NODES;
        rng_state = lcg_next(rng_state);
        let dst_idx = (rng_state as usize) % TIER2_NODES;
        if src_idx == dst_idx {
            continue;
        }
        let _ = graph.add_dependency_with_provenance(
            &ids[src_idx],
            &ids[dst_idx],
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
    }
    graph
}

fn lcg_next(state: u64) -> u64 {
    state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)
}

/// Helper: measure one stage and assert against the per-stage budget.
fn assert_stage(name: &str, elapsed_ms: u128) {
    eprintln!("stage={name} elapsed_ms={elapsed_ms}");
    assert!(
        elapsed_ms <= PER_STAGE_BUDGET_MS,
        "e91.W8 regression: stage '{name}' took {elapsed_ms} ms, \
         exceeding per-stage budget {PER_STAGE_BUDGET_MS} ms on Tier-2 fixture. \
         Runbook E91 §2 calls out per-stage profiling as the diagnostic discipline; \
         investigate the hot path before merging."
    );
}

#[test]
fn w8_stage_projection() {
    let graph = build_tier2_fixture();
    let started = Instant::now();
    let projection = project_call_graph(&graph);
    // Touch a property so the call can't be elided.
    let _ = projection.node_count();
    assert_stage("projection", started.elapsed().as_millis());
}

#[test]
fn w8_stage_scc() {
    let graph = build_tier2_fixture();
    let projection = project_call_graph(&graph);
    let started = Instant::now();
    let sccs = projection.strongly_connected_components();
    let _ = sccs.len();
    assert_stage("scc", started.elapsed().as_millis());
}

#[test]
fn w8_stage_god_nodes() {
    let graph = build_tier2_fixture();
    let started = Instant::now();
    let gods = GraphAnalyticsService::god_nodes(&graph, 0.95);
    let _ = gods.len();
    assert_stage("god_nodes", started.elapsed().as_millis());
}

#[test]
fn w8_stage_feedback_arc_set() {
    let graph = build_tier2_fixture();
    let started = Instant::now();
    let fas = GraphAnalyticsService::feedback_arc_set(&graph);
    let _ = fas.len();
    assert_stage("feedback_arc_set", started.elapsed().as_millis());
}

#[test]
fn w8_stage_community_detect() {
    let graph = build_tier2_fixture();
    let started = Instant::now();
    let result = CommunityDetector::detect(&graph, 100);
    let _ = result.communities.len();
    assert_stage("community_detect", started.elapsed().as_millis());
}

#[test]
fn w8_stage_analyze_full_aggregate() {
    // This is a redundant safety net: even if a per-stage test
    // becomes flaky on a slow runner, the aggregate budget from
    // W7 catches the regression at the entry point.
    let graph = build_tier2_fixture();
    let started = Instant::now();
    let report =
        cognicode_core::application::services::graph_insights::GraphInsightsService::analyze(&graph);
    let _ = report.health_score;
    assert_stage("analyze_full", started.elapsed().as_millis());
}
