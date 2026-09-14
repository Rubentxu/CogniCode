//! Data-flow graph (DFG) — per-function, pure functions over flat slices.
//!
//! ## Representation
//!
//! The DFG records **intra-procedural** data dependencies: every
//! statement that **defines** a variable flows into every later statement
//! in the same function that **uses** the same variable. The hot loop
//! never re-parses the source: it consumes a flat list of statements
//! already lifted out of the CFG by a higher-level extractor (today: a
//! synthetic input shape; tomorrow: a tree-sitter pass — see M5.1).
//!
//! ## Input shape
//!
//! ```text
//! Statement { id: usize, kind: "assign"|"call"|"branch", defs: [String], uses: [String] }
//! ```
//!
//! - `id` is a stable identifier within the function (typically the
//!   statement index inside its enclosing basic block).
//! - `defs` is the set of variables the statement introduces (may be empty).
//! - `uses` is the set of variables the statement reads (may be empty).
//!
//! ## Determinism
//!
//! `dfg_edges()` emits edges sorted by `(from, to, variable)`. Variables
//! inside each edge are sorted lexicographically. This gives a stable
//! output across runs and platforms, which is required by the conformance
//! fixtures and the digests pinned in `cfg_digest`.

use serde::{Deserialize, Serialize};

/// A single statement lifted out of the CFG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statement {
    /// Stable id (e.g. global statement index in the function).
    pub id: usize,
    /// Statement kind tag. Free-form for now; the DFG algo does not branch on it.
    #[serde(default)]
    pub kind: String,
    /// Variables this statement defines.
    #[serde(default)]
    pub defs: Vec<String>,
    /// Variables this statement uses.
    #[serde(default)]
    pub uses: Vec<String>,
}

/// One edge in the DFG: statement `from` defines `variable` which is
/// then used by statement `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefUseEdge {
    /// Statement that defines the variable.
    pub from: usize,
    /// Statement that uses the variable.
    pub to: usize,
    /// The variable that flows from `from` to `to`.
    pub variable: String,
}

/// Compute the intra-procedural DFG.
///
/// Returns edges sorted by `(from, to, variable)`. Edges within the same
/// `(from, to)` pair are emitted in variable-sorted order so the output
/// is bit-stable across runs.
pub fn dfg_edges(statements: &[Statement]) -> Vec<DefUseEdge> {
    let mut edges: Vec<DefUseEdge> = Vec::new();

    // For each variable, the latest statement that defined it (single-pass
    // forward sweep). We only need the most recent definition because we
    // don't model control-flow-aware reaching-defs here — that's a richer
    // "dataflow analysis" pass, not the flat-slice primitive this slice
    // exposes (see design D3).
    let mut last_def: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();

    for stmt in statements {
        // Emit def→use edges for every variable this statement uses.
        for var in &stmt.uses {
            if let Some(&def_id) = last_def.get(var) {
                edges.push(DefUseEdge {
                    from: def_id,
                    to: stmt.id,
                    variable: var.clone(),
                });
            }
        }
        // Now update the def map with this statement's defs.
        for var in &stmt.defs {
            last_def.insert(var.clone(), stmt.id);
        }
    }

    // Deterministic order: (from, to, variable).
    edges.sort_by(|a, b| {
        a.from
            .cmp(&b.from)
            .then_with(|| a.to.cmp(&b.to))
            .then_with(|| a.variable.cmp(&b.variable))
    });
    edges
}

/// Convenience: which statements does `variable`'s definition at
/// `definition_site` reach in the DFG?
///
/// Returns a sorted `Vec<usize>` of statement ids whose `uses` chain
/// back (transitively, in the DFG) to `definition_site` for `variable`.
///
/// This is a thin forward walk over the DFG edges; it is **not** a
/// control-flow-aware slice (that's `slicing::forward_slice`). It exists
/// so conformance fixtures can pin the DFG-only portion of the analysis.
pub fn reaching_uses(edges: &[DefUseEdge], definition_site: usize, variable: &str) -> Vec<usize> {
    // Build a successor adjacency over `to` for any edge matching `variable`.
    let mut succ: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for e in edges {
        if e.variable == variable && e.from == definition_site {
            succ.entry(e.from).or_default().push(e.to);
        }
    }
    let mut visited: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    let mut stack = vec![definition_site];
    while let Some(u) = stack.pop() {
        if !visited.insert(u) {
            continue;
        }
        if let Some(nexts) = succ.get(&u) {
            for &v in nexts {
                if !visited.contains(&v) {
                    stack.push(v);
                }
            }
        }
    }
    visited.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: usize, defs: &[&str], uses: &[&str]) -> Statement {
        Statement {
            id,
            kind: "assign".to_string(),
            defs: defs.iter().map(|s| s.to_string()).collect(),
            uses: uses.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn linear_chain_yields_one_edge_per_use() {
        // x = 1; y = x; z = y;
        let stmts = vec![
            s(0, &["x"], &[]),
            s(1, &["y"], &["x"]),
            s(2, &["z"], &["y"]),
        ];
        let edges = dfg_edges(&stmts);
        assert_eq!(
            edges,
            vec![
                DefUseEdge {
                    from: 0,
                    to: 1,
                    variable: "x".into()
                },
                DefUseEdge {
                    from: 1,
                    to: 2,
                    variable: "y".into()
                },
            ]
        );
    }

    #[test]
    fn later_definition_overrides_earlier_one() {
        // x = 1; y = x; x = 2; z = x;
        let stmts = vec![
            s(0, &["x"], &[]),
            s(1, &["y"], &["x"]),
            s(2, &["x"], &[]),
            s(3, &["z"], &["x"]),
        ];
        let edges = dfg_edges(&stmts);
        // stmt 1 uses x — defined at stmt 0 (last_def == 0)
        // stmt 3 uses x — defined at stmt 2 (last_def == 2)
        assert_eq!(
            edges,
            vec![
                DefUseEdge {
                    from: 0,
                    to: 1,
                    variable: "x".into()
                },
                DefUseEdge {
                    from: 2,
                    to: 3,
                    variable: "x".into()
                },
            ]
        );
    }

    #[test]
    fn diamond_uses_reach_via_most_recent_def() {
        // a = 1; if c { b = a; } else { d = a; }
        // Modeled flat: stmt 0 def a; stmt 1 use a def b; stmt 2 use a def d
        let stmts = vec![
            s(0, &["a"], &[]),
            s(1, &["b"], &["a"]),
            s(2, &["d"], &["a"]),
        ];
        let edges = dfg_edges(&stmts);
        assert_eq!(edges.len(), 2);
        assert!(edges.contains(&DefUseEdge {
            from: 0,
            to: 1,
            variable: "a".into()
        }));
        assert!(edges.contains(&DefUseEdge {
            from: 0,
            to: 2,
            variable: "a".into()
        }));
    }

    #[test]
    fn use_before_def_yields_no_edge() {
        // y = x; x = 1;
        let stmts = vec![s(0, &["y"], &["x"]), s(1, &["x"], &[])];
        let edges = dfg_edges(&stmts);
        assert!(edges.is_empty());
    }

    #[test]
    fn reaching_uses_is_deterministic_and_sorted() {
        let stmts = vec![
            s(0, &["x"], &[]),
            s(1, &["y"], &["x"]),
            s(2, &["z"], &["x"]),
            s(3, &["w"], &["y"]),
        ];
        let edges = dfg_edges(&stmts);
        let reaches = reaching_uses(&edges, 0, "x");
        assert_eq!(reaches, vec![0, 1, 2], "x flows from 0 to 1 and 2 only");
    }

    #[test]
    fn empty_input_yields_no_edges() {
        assert!(dfg_edges(&[]).is_empty());
    }

    #[test]
    fn pure_use_only_statement_has_no_outgoing_def() {
        // No statement before defines `x` → no edges.
        let stmts = vec![s(0, &[], &["x"])];
        let edges = dfg_edges(&stmts);
        assert!(edges.is_empty());
    }
}
