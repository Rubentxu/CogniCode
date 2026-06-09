//! Compile fixture graph — a 10-node / 15-edge call graph used by
//! the compile + parity tests. Edges carry provenance (lsp, tree_sitter,
//! postgres) and a confidence value in [0.0, 1.0] so the WHERE-filter
//! branch tests have real data to filter against.
//!
//! ## Layout
//!
//! ```text
//!   1 → 2 → 3 → 4
//!   1 → 5 → 6
//!   2 → 6
//!   7 → 8 → 9 (cycle: 9 → 8)
//!   10 (isolated)
//! ```
//!
//! ## Provenance
//!
//! Edges from `1→2`, `1→5`, `7→8`, `9→8` are `lsp`.
//! Edges from `2→3`, `2→6`, `8→9` are `tree_sitter`.
//! Edges from `3→4`, `5→6` are `postgres`.
//!
//! ## Confidence
//!
//! Each edge carries a confidence in `{0.0, 0.3, 0.5, 0.7, 1.0}`.

use crate::moldql::ast::{
    BooleanOp, BooleanQuery, ClusterMethod, ClusterQuery, Condition, ExplainQuery, Field,
    FindQuery, MoldQLQuery, NeighborsQuery, Op, PathQuery, SubgraphQuery, TargetType,
    TraversalDirection, Value,
};

/// Edge in the fixture graph. `(from, to, provenance, confidence)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixtureEdge {
    pub from: u32,
    pub to: u32,
    pub provenance: &'static str,
    pub confidence: f64,
}

/// 15 edges that cover the spec's provenance + confidence matrix.
pub const FIXTURE_EDGES: &[FixtureEdge] = &[
    FixtureEdge {
        from: 1,
        to: 2,
        provenance: "lsp",
        confidence: 1.0,
    },
    FixtureEdge {
        from: 1,
        to: 5,
        provenance: "lsp",
        confidence: 0.7,
    },
    FixtureEdge {
        from: 2,
        to: 3,
        provenance: "tree_sitter",
        confidence: 0.5,
    },
    FixtureEdge {
        from: 2,
        to: 6,
        provenance: "tree_sitter",
        confidence: 0.3,
    },
    FixtureEdge {
        from: 3,
        to: 4,
        provenance: "postgres",
        confidence: 0.0,
    },
    FixtureEdge {
        from: 5,
        to: 6,
        provenance: "postgres",
        confidence: 0.0,
    },
    FixtureEdge {
        from: 7,
        to: 8,
        provenance: "lsp",
        confidence: 1.0,
    },
    FixtureEdge {
        from: 8,
        to: 9,
        provenance: "tree_sitter",
        confidence: 0.5,
    },
    FixtureEdge {
        from: 9,
        to: 8,
        provenance: "lsp",
        confidence: 0.7,
    },
    // Extra chain to make 15 edges total.
    FixtureEdge {
        from: 1,
        to: 3,
        provenance: "postgres",
        confidence: 0.5,
    },
    FixtureEdge {
        from: 1,
        to: 4,
        provenance: "postgres",
        confidence: 0.3,
    },
    FixtureEdge {
        from: 1,
        to: 6,
        provenance: "tree_sitter",
        confidence: 0.5,
    },
    FixtureEdge {
        from: 2,
        to: 4,
        provenance: "postgres",
        confidence: 0.0,
    },
    FixtureEdge {
        from: 5,
        to: 3,
        provenance: "postgres",
        confidence: 0.0,
    },
    FixtureEdge {
        from: 6,
        to: 4,
        provenance: "postgres",
        confidence: 0.0,
    },
];

/// Node count.
pub const FIXTURE_NODES: u32 = 10;

/// SCC in the fixture: nodes 8 and 9 form a cycle, so `find_scc()`
/// should yield `[[1], [2], [3], [4], [5], [6], [7], [8, 9], [10]]`.
///
/// 10 is isolated.
pub fn expected_sccs() -> Vec<Vec<u32>> {
    vec![
        vec![1],
        vec![2],
        vec![3],
        vec![4],
        vec![5],
        vec![6],
        vec![7],
        vec![8, 9],
        vec![10],
    ]
}

/// All nodes reachable from a root via outgoing edges, BFS-capped at
/// `depth`. Used by the petgraph branch to compute the expected result
/// without a real `petgraph::Graph` instance.
pub fn bfs_reachable(root: u32, max_hops: u32) -> Vec<u32> {
    use std::collections::BTreeSet;
    let mut visited: BTreeSet<u32> = BTreeSet::new();
    visited.insert(root);
    let mut frontier: Vec<u32> = vec![root];
    for _ in 0..max_hops {
        let mut next: Vec<u32> = Vec::new();
        for &n in &frontier {
            for e in FIXTURE_EDGES {
                if e.from == n && visited.insert(e.to) {
                    next.push(e.to);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    visited.into_iter().collect()
}

/// Reverse BFS: all nodes that can reach `root`.
pub fn bfs_predecessors(root: u32, max_hops: u32) -> Vec<u32> {
    use std::collections::BTreeSet;
    let mut visited: BTreeSet<u32> = BTreeSet::new();
    visited.insert(root);
    let mut frontier: Vec<u32> = vec![root];
    for _ in 0..max_hops {
        let mut next: Vec<u32> = Vec::new();
        for &n in &frontier {
            for e in FIXTURE_EDGES {
                if e.to == n && visited.insert(e.from) {
                    next.push(e.from);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    visited.into_iter().collect()
}

/// BFS in both directions from `root`.
pub fn bfs_dual(root: u32, max_hops: u32) -> Vec<u32> {
    use std::collections::BTreeSet;
    let mut visited: BTreeSet<u32> = BTreeSet::new();
    visited.insert(root);
    let mut frontier: Vec<u32> = vec![root];
    for _ in 0..max_hops {
        let mut next: Vec<u32> = Vec::new();
        for &n in &frontier {
            for e in FIXTURE_EDGES {
                if e.from == n && visited.insert(e.to) {
                    next.push(e.to);
                }
                if e.to == n && visited.insert(e.from) {
                    next.push(e.from);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    visited.into_iter().collect()
}

// ---- Convenience AST builders used by the compile tests ----

pub fn path(from: &str, to: &str) -> MoldQLQuery {
    MoldQLQuery::Path(PathQuery {
        from: from.into(),
        to: to.into(),
        max_hops: None,
        conditions: Vec::new(),
    })
}

pub fn path_with_max_hops(from: &str, to: &str, n: u32) -> MoldQLQuery {
    MoldQLQuery::Path(PathQuery {
        from: from.into(),
        to: to.into(),
        max_hops: Some(n),
        conditions: Vec::new(),
    })
}

pub fn neighbors(root: &str, depth: u32, direction: TraversalDirection) -> MoldQLQuery {
    MoldQLQuery::Neighbors(NeighborsQuery {
        root: root.into(),
        depth,
        direction,
        conditions: Vec::new(),
    })
}

pub fn subgraph(root: &str, depth: u32) -> MoldQLQuery {
    MoldQLQuery::Subgraph(SubgraphQuery {
        root: root.into(),
        depth,
        direction: TraversalDirection::Both,
        conditions: Vec::new(),
    })
}

pub fn cluster(method: ClusterMethod) -> MoldQLQuery {
    MoldQLQuery::Cluster(ClusterQuery {
        method,
        conditions: Vec::new(),
    })
}

pub fn explain(from: &str, to: &str) -> MoldQLQuery {
    MoldQLQuery::Explain(ExplainQuery {
        from: from.into(),
        to: to.into(),
        conditions: Vec::new(),
    })
}

pub fn find_symbols() -> MoldQLQuery {
    MoldQLQuery::Find(FindQuery {
        target: TargetType::Symbols,
        scope: None,
        conditions: Vec::new(),
        apply_lens: None,
    })
}

pub fn and(left: MoldQLQuery, right: MoldQLQuery) -> MoldQLQuery {
    MoldQLQuery::Boolean(BooleanQuery {
        op: BooleanOp::And,
        operands: vec![left, right],
    })
}

pub fn or(left: MoldQLQuery, right: MoldQLQuery) -> MoldQLQuery {
    MoldQLQuery::Boolean(BooleanQuery {
        op: BooleanOp::Or,
        operands: vec![left, right],
    })
}

pub fn not(inner: MoldQLQuery) -> MoldQLQuery {
    MoldQLQuery::Boolean(BooleanQuery {
        op: BooleanOp::Not,
        operands: vec![inner],
    })
}

pub fn cond_provenance(source: &str, value: &str) -> Condition {
    Condition {
        field: Field::dotted("provenance", source),
        op: Op::Eq,
        value: Value::String(value.into()),
    }
}

pub fn cond_confidence_gte(n: f64) -> Condition {
    Condition {
        field: Field::single("confidence"),
        op: Op::Gte,
        value: Value::Number(n),
    }
}
