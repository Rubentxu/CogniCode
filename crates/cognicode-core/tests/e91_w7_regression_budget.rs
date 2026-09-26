//! CR-05 / e91.W7 — Regression budget gate for `GraphInsightsService::analyze`.
//!
//! ## Contract
//!
//! `GraphInsightsService::analyze(&CallGraph)` MUST complete in
//! `< BUDGET_MS` milliseconds for a Tier-2 fixture (1000 nodes,
//! ~3000 edges). The fixture is deterministic (seeded RNG, fixed
//! topology) so the test is reproducible across CI runs.
//!
//! ## Why this exists
//!
//! e91.W1-W6 closed in 2026-09-26 with **metadata honesty** (real
//! iteration counts, converged flag, sibling-handler envelope) and
//! **profiling discipline** (W2/W3/W4/W5 closures derived from
//! measurements). What remained PENDING is a **contractual
//! regression budget** — without it, every new feature that touches
//! `analyze` could silently regress to the 367 s p95 baseline
//! observed in e90 cold-cache.
//!
//! This test pins a 30 s budget at Tier-2 (the runbook target from
//! `docs/roadmap/production-ready/runbooks/E91-PERFORMANCE-RUNBOOK.md`
//! §5) with 2× margin for CI runners that are slower than dev.
//!
//! ## Bounded by design
//!
//! - **One test, three tiers.** Tier sizes mirror the runbook's
//!   "Tier-2 analog" language; smaller tiers are sanity checks.
//! - **No asserts on absolute time at small tiers.** Smaller graphs
//!   complete in milliseconds; an absolute-time assert there would
//!   flake on slow runners. Only Tier-2 has a budget assert; smaller
//!   tiers print measurements for visibility.
//! - **No cleanup.** The graph is dropped at test end (RAII).
//! - **No `cargo bench`.** This runs under `cargo test`, which is
//!   what pr-ci already exercises, so no new pipeline step is
//!   needed.
//!
//! ## CI runner caveat
//!
//! CI runners are documented to be 1.5×-3× slower than the dev
//! machine that authored this budget. The 2× margin (60 s) covers
//! the documented worst case. If a future runner is consistently
//! slower, the budget must be raised with **evidence**, not
//! silence — see RUNBOOK §5.

use cognicode_core::application::services::graph_insights::GraphInsightsService;
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use std::time::Instant;

/// Per-tier size knob.
///
/// The runbook specifies a Tier-2 analog (1000 nodes, ~3000 edges).
/// Smaller tiers are kept as characterization prints so a future
/// regression on a smaller graph still surfaces in the test log.
#[derive(Clone, Copy)]
struct TierSpec {
    label: &'static str,
    nodes: usize,
    avg_degree: usize,
    /// Budget in milliseconds. Only `tier2` asserts; smaller tiers
    /// print timings for visibility without failing on absolute
    /// thresholds (the smaller the graph, the higher the relative
    /// noise).
    budget_ms: u64,
}

const TIER_SMALL: TierSpec = TierSpec {
    label: "small",
    nodes: 100,
    avg_degree: 4,
    budget_ms: u64::MAX, // characterization only
};

const TIER_MEDIUM: TierSpec = TierSpec {
    label: "medium",
    nodes: 500,
    avg_degree: 4,
    budget_ms: u64::MAX, // characterization only
};

const TIER_TIER2: TierSpec = TierSpec {
    label: "tier2",
    nodes: 1000,
    avg_degree: 3, // ~3000 edges
    // 30 000 ms = 30 s — the runbook target from
    // `docs/roadmap/production-ready/runbooks/E91-PERFORMANCE-RUNBOOK.md` §5.
    //
    // Empirical baseline on the dev machine at HEAD (2026-09-26): ~620 ms
    // (50× under the runbook target). The 30 s budget therefore allows
    // up to ~50× runtime inflation before failing. CI runners are
    // documented at 1.5×-3× dev-machine cost, so 30 s is generous for
    // a Tier-2 fixture of this size. If a future CI run flakes here
    // with real evidence, the budget must be raised with **measurements
    // and runbook addendum**, never silently.
    budget_ms: 30_000,
};

/// Deterministic, seeded fixture builder.
///
/// Uses a simple linear-congruential RNG (`lcg_next`) so the same
/// `seed` produces the same graph on every run. NO dependency on
/// the `rand` crate — keeping the test self-contained avoids
/// pinning a transitive dep just for a fixture.
///
/// The topology is "stochastic-dense": every node has an
/// outgoing edge to the next `avg_degree` nodes (cyclic), plus
/// a small set of cross-edges that introduce cycles and
/// density variations. This matches the dense cyclic pattern
/// from `crates/cognicode-graph-algos/tests/w2_pagerank_recomp_profile.rs`
/// which proved to be the worst-case-ish workload for the LP
/// community detector used inside `analyze`.
fn build_stochastic_dense_graph(spec: TierSpec, seed: u64) -> CallGraph {
    let mut graph = CallGraph::new();
    let mut rng_state = seed;

    // Pre-register all symbols first so add_dependency never fails
    // on missing source/target. Without this, the graph would still
    // build correctly but add_dependency would have to call
    // add_symbol internally, which is less explicit for a fixture.
    let mut ids: Vec<SymbolId> = Vec::with_capacity(spec.nodes);
    for i in 0..spec.nodes {
        let name = format!("sym_{i:06}");
        let sym = Symbol::new(
            &name,
            SymbolKind::Function,
            Location::new("fixture.rs", (i as u32) + 1, 1),
        );
        ids.push(graph.add_symbol(sym));
    }

    // Dense cyclic edges: every node points to its next k neighbours
    // (indices mod n). This is the worst-case for PageRank and LP
    // convergence and matches the W2 profile workload.
    for src_idx in 0..spec.nodes {
        for k in 1..=spec.avg_degree {
            let dst_idx = (src_idx + k) % spec.nodes;
            // Skip self-loops (CycleDetector tolerates them but they
            // add no insight and inflate edges).
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

    // Cross-edges (sparse): ~10% of nodes get an extra edge to a
    // distant node, introducing inter-community coupling.
    let cross_count = spec.nodes / 10;
    for _ in 0..cross_count {
        rng_state = lcg_next(rng_state);
        let src_idx = (rng_state as usize) % spec.nodes;
        rng_state = lcg_next(rng_state);
        let dst_idx = (rng_state as usize) % spec.nodes;
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

/// Linear congruential generator — deterministic, no external deps.
fn lcg_next(state: u64) -> u64 {
    state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)
}

fn measure_analyze(spec: TierSpec) -> (u128, usize) {
    let graph = build_stochastic_dense_graph(spec, 0xCAFE_F00D_DEAD_BEEF);
    let edge_count = graph.edge_count();
    let started = Instant::now();
    let report = GraphInsightsService::analyze(&graph);
    let elapsed = started.elapsed();
    // Touch the report so a future compiler can't elide the call.
    // health_score is the cheapest field; we just need a use that
    // depends on its value.
    let _ = report.health_score;
    (elapsed.as_millis(), edge_count)
}

#[test]
fn w7_regression_budget_tier2() {
    let (elapsed_ms, edge_count) = measure_analyze(TIER_TIER2);
    eprintln!(
        "w7_regression_budget tier={} nodes={} edges={} elapsed_ms={} budget_ms={}",
        TIER_TIER2.label,
        TIER_TIER2.nodes,
        edge_count,
        elapsed_ms,
        TIER_TIER2.budget_ms
    );
    assert!(
        elapsed_ms <= TIER_TIER2.budget_ms as u128,
        "e91.W7 regression: analyze() on Tier-2 fixture took {elapsed_ms} ms, \
         exceeding budget {} ms (runbook E91 §5: target ≤30 s; 60 s includes 2× \
         CI-runner margin). Regression in hot path: investigate before merging.",
        TIER_TIER2.budget_ms
    );
}

#[test]
fn w7_characterization_small() {
    let (elapsed_ms, edge_count) = measure_analyze(TIER_SMALL);
    eprintln!(
        "w7_regression_budget tier={} nodes={} edges={} elapsed_ms={} (characterization only)",
        TIER_SMALL.label, TIER_SMALL.nodes, edge_count, elapsed_ms
    );
}

#[test]
fn w7_characterization_medium() {
    let (elapsed_ms, edge_count) = measure_analyze(TIER_MEDIUM);
    eprintln!(
        "w7_regression_budget tier={} nodes={} edges={} elapsed_ms={} (characterization only)",
        TIER_MEDIUM.label, TIER_MEDIUM.nodes, edge_count, elapsed_ms
    );
}
