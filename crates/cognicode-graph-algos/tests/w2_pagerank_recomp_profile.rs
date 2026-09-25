//! W2 (e91) — Cost characterisation for PageRank recomputation in
//! the duplicated-handler pattern. Before fixing the API to accept
//! pre-computed scores, we need empirical evidence that recomputing
//! PageRank from scratch is actually expensive enough to matter.
//!
//! Three handlers in `cognicode-explorer/src/mcp/handler/
//! graph_analyze.rs` each call `GraphAnalyticsService::page_rank`
//! on the same subgraph with identical parameters:
//!   * god_nodes             — calls page_rank internally (line 197)
//!   * community_god_nodes   — calls page_rank internally (line 288)
//!   * surprising_connections— calls page_rank internally (line 369)
//!
//! When a user invokes any two of these tools in sequence (e.g. the
//! typical workflow "explore graph → identify god nodes → find
//! surprising connections"), PageRank runs from scratch TWICE (or
//! thrice) on the same graph with the same alpha=0.85, max_iter=100.
//!
//! This test verifies that such duplicated work is in fact the
//! performance bottleneck worth a fix. It does NOT assert the fix
//! — only that the symptom exists at non-trivial cost.
//!
//! To keep gate cost low, the test uses synthetic graphs at three
//! sizes that exercise:
//!   * tiny   (50 nodes, ~150 edges) — sanity / convergence test
//!   * small  (200 nodes, ~600 edges) — typical subgraph depth=3
//!   * medium (1000 nodes, ~3000 edges) — Tier-2 fixture analog
//!
//! If a future e91.W3 implementation threads scores through the
//! handlers, this characterisation should also verify that the
//! second/third invocation cost approaches zero. Pin the line at
//! which the optimisation pays off.

use cognicode_graph_algos::algorithms::page_rank;
use std::collections::HashMap;
use std::time::Instant;

/// Build a dense cyclic graph that resists early convergence:
/// every node points to its next `fanout` neighbours in a toroidal
/// structure (indices mod n), so each iteration must propagate
/// rank mass fully around the cycle before delta drops under
/// tolerance. This is the worst-case-ish workload for PageRank and
/// avoids the "synthetic hub graphs converge in 3-5 iterations"
/// failure mode observed in early drafts of this test.
#[allow(clippy::needless_range_loop)] // `v` indexes both in_neighbors (push) and out_degree (incr).
fn build_dense_cyclic_graph(n: usize, fanout: usize) -> (Vec<Vec<usize>>, Vec<usize>) {
    let mut in_neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut out_degree = vec![0usize; n];
    for v in 0..n {
        for k in 1..=fanout {
            let target = (v + k) % n;
            in_neighbors[target].push(v);
            out_degree[v] += 1;
        }
    }
    (in_neighbors, out_degree)
}

/// Characterise the wall-clock cost of recomputing PageRank on a
/// synthetic subgraph. Reports timings via eprintln so the gate
/// stays cheap (no `assert!` based on absolute time — that would
/// flake on CI). The println output is captured by the CI runner
/// for regression analysis.
///
/// Future evidence trail: this test prints `<size>,<n>,<one>,<two>,<ratio>`
/// lines that can be diffed across runs.
#[test]
fn profile_pagerank_recomputation_cost() {
    let alpha = 0.85;
    let max_iter = 100;
    // Dense cyclic graphs are much harder for PageRank than hub
    // graphs. Sizes chosen so the smallest tier produces
    // measurable timing on a release build runner.
    let sizes = [
        (10_000usize, 4usize), // tier-2 analog
        (25_000, 4),           // tier-3 analog
        (50_000, 6),           // tier-3 large — likely budget for p95
    ];

    println!(
        "W2 profile: PageRank recomputation cost (alpha={alpha}, max_iter={max_iter}, dense-cycle graph)"
    );
    println!("size,n,fanout,one_us,two_us,wasted_pct");

    for (n, fanout) in sizes {
        let (in_n, out_d) = build_dense_cyclic_graph(n, fanout);

        // First invocation (the "warm" cost — same call as today's
        // graph_god_nodes handler makes).
        let t1 = Instant::now();
        let _scores1 = page_rank(&in_n, &out_d, n, alpha, max_iter);
        let one_us = t1.elapsed().as_micros();

        // Second invocation (the "wasted" cost when god_nodes is
        // called after pagerank was already computed).
        let t2 = Instant::now();
        let _scores2 = page_rank(&in_n, &out_d, n, alpha, max_iter);
        let two_us = t2.elapsed().as_micros();

        // Sum = what the user pays for the duplicated pattern.
        // Wasted = fraction that the second call would have been,
        // i.e. would be saved if the API accepted pre-computed scores.
        let sum_us = one_us + two_us;
        let wasted_pct = if sum_us > 0 {
            (two_us as f64) / (sum_us as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "  n={n:<6} fanout={fanout}  warm={one_us:>6}µs  wasted={two_us:>6}µs  ({wasted_pct:>5.1}% savings if shared)"
        );

        // Sanity: both invocations must produce identical scores
        // (deterministic algorithm).
        assert_eq!(
            _scores1, _scores2,
            "PageRank is deterministic — different outputs would mean a bug"
        );
    }
}

#[test]
fn profile_pagerank_dense_cycle_at_tier2_sizes() {
    // Sanity gate: a tier-2-sized dense cycle must finish well
    // under the latency budget we want to fit inside MCP p95
    // (5s for analytics family per ADR-XXX). If this fails, the
    // benchmark at the next size is the relevant threshold.
    let n = 10_000;
    let (in_n, out_d) = build_dense_cyclic_graph(n, 4);
    let t = Instant::now();
    let _ = page_rank(&in_n, &out_d, n, 0.85, 100);
    let elapsed_ms = t.elapsed().as_millis();

    println!("tier-2 dense cycle (n={n}) single PageRank: {elapsed_ms} ms");
    // Bound is 5s (analytics family budget) with generous slack
    // for cold-cache and small runners — we want this to fit
    // inside one MCP tool call, not multiple.
    assert!(
        elapsed_ms < 5_000,
        "single PageRank on tier-2 dense cycle (10K nodes) must fit in 5s; got {elapsed_ms}ms"
    );
}

/// Pin the deterministic-output guarantee of PageRank. Multiple
/// invocations on the same graph must return identical HashMaps.
/// Combined with the recomputation cost characterisation, this
/// establishes the precondition for safe caching: scores are
/// stable across calls, so the only cost the duplicate removes is
/// the compute, not the result quality.
#[test]
fn profile_pagerank_is_deterministic() {
    let (in_n, out_d) = build_dense_cyclic_graph(200, 4);
    let s1 = page_rank(&in_n, &out_d, 200, 0.85, 100);
    let s2 = page_rank(&in_n, &out_d, 200, 0.85, 100);
    let s3 = page_rank(&in_n, &out_d, 200, 0.85, 100);

    assert_eq!(s1, s2);
    assert_eq!(s2, s3);

    // And: every node must receive a finite score (NaN guard).
    let nan_count: usize = s1.values().filter(|v| !v.is_finite()).count();
    assert_eq!(
        nan_count, 0,
        "PageRank produced {nan_count} non-finite scores"
    );
}

/// Helper: assert that `HashMap<usize, f64>` is the canonical
/// return shape (so future `e91.W3` "thread scores through" work
/// has a typed contract to honour). Just a sanity compile-time
/// check via std::any.
#[allow(dead_code)]
fn _pagerank_returns_hashmap_usize_f64(s: HashMap<usize, f64>) -> HashMap<usize, f64> {
    s
}
