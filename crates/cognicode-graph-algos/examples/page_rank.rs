//! Ranks a three-node graph with `page_rank`.
//!
//! Run with `cargo run --example page_rank -p cognicode-graph-algos`.

use cognicode_graph_algos::page_rank;

fn main() {
    // Edges: 0 -> 2, 1 -> 2, 2 -> 0. `in_neighbors[v]` lists the sources of
    // the edges that point at `v`; `out_degree[v]` counts the edges leaving it.
    let in_neighbors = vec![vec![2], vec![], vec![0, 1]];
    let out_degree = vec![1, 1, 1];

    let scores = page_rank(&in_neighbors, &out_degree, 3, 0.85, 100);

    let mut ranked: Vec<_> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    for (node, score) in ranked {
        println!("node {node}: {score:.4}");
    }
}
