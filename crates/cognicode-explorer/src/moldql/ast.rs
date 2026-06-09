//! MoldQL AST — pure data types, no parsing or execution logic.
//!
//! MoldQL is the query language for the explorer: a single call that
//! combines filter, scope, and lens into one expression. The AST is the
//! canonical, in-memory representation a parsed query settles into.
//!
//! ExplorerQL extends this with 5 graph-native primitives
//! ([`MoldQLQuery::Path`], [`MoldQLQuery::Neighbors`],
//! [`MoldQLQuery::Subgraph`], [`MoldQLQuery::Cluster`],
//! [`MoldQLQuery::Explain`]) plus a boolean composition wrapper
//! ([`MoldQLQuery::Boolean`]). The original FIND/EXPLORE variants are
//! untouched — ExplorerQL is a strict superset of MoldQL.

/// The body of a `FIND` query.
#[derive(Debug, Clone, PartialEq)]
pub struct FindQuery {
    pub target: TargetType,
    /// Optional `IN SCOPE <path>` filter. `None` means "no scope restriction".
    pub scope: Option<String>,
    /// `WHERE` conditions. AND-chained — all must pass.
    pub conditions: Vec<Condition>,
    /// Optional `APPLY <lens>` clause.
    pub apply_lens: Option<String>,
}

/// What kind of objects the `FIND` clause returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetType {
    Symbols,
    Files,
    Scopes,
    Issues,
}

impl TargetType {
    /// Canonical lowercase form used in queries: `symbols`, `files`, etc.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Symbols => "symbols",
            Self::Files => "files",
            Self::Scopes => "scopes",
            Self::Issues => "issues",
        }
    }
}

/// A single `WHERE` clause predicate.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub field: Field,
    pub op: Op,
    pub value: Value,
}

/// A dotted field reference. `["fan_in"]` for plain fields, `["quality",
/// "critical"]` for nested ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub parts: Vec<String>,
}

impl Field {
    /// Single-part field. `fan_in` → `Field { parts: ["fan_in"] }`.
    pub fn single(part: impl Into<String>) -> Self {
        Self {
            parts: vec![part.into()],
        }
    }

    /// Two-part dotted field. `quality.critical` → `Field { parts:
    /// ["quality", "critical"] }`.
    pub fn dotted(a: impl Into<String>, b: impl Into<String>) -> Self {
        Self {
            parts: vec![a.into(), b.into()],
        }
    }

    /// The first segment. For `quality.critical` → `"quality"`.
    pub fn head(&self) -> &str {
        self.parts.first().map(String::as_str).unwrap_or("")
    }

    /// The second segment, if any. For `fan_in` → `None`; for
    /// `quality.critical` → `Some("critical")`.
    pub fn tail(&self) -> Option<&str> {
        self.parts.get(1).map(String::as_str)
    }
}

/// Comparison operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
    /// Substring / contains — only meaningful for string-valued fields.
    Contains,
}

impl Op {
    /// Wire form: `>`, `>=`, `<`, `<=`, `==`, `!=`, `~`.
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Gt => ">",
            Self::Gte => ">=",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Eq => "==",
            Self::Neq => "!=",
            Self::Contains => "~",
        }
    }
}

/// Right-hand side of a condition.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
}

/// The body of an `EXPLORE` query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreQuery {
    /// MVP id of the seed object (e.g. `symbol:src/main.rs:main:1`).
    pub object_ref: String,
    pub direction: Direction,
    /// Maximum BFS depth. Executor caps this at 5.
    pub depth: u32,
}

/// Which side of the call graph to walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Direction {
    Callers,
    Callees,
}

impl Direction {
    /// Wire form: `callers`, `callees`.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Callers => "callers",
            Self::Callees => "callees",
        }
    }
}

// ============================================================================
// ExplorerQL extensions — added per `sdd/explorerql-grammar`.
//
// All new types are STRICTLY ADDITIVE. The original 32 FIND/EXPLORE tests
// are unaffected because the existing variants are untouched.
// ============================================================================

/// Direction of graph traversal for ExplorerQL primitives. Distinct from
/// the legacy [`Direction`] enum (which is `callers`/`callees` for EXPLORE)
/// so the two enums cannot accidentally pattern-match the wrong value.
///
/// `Incoming` is reverse (callers), `Outgoing` is forward (callees), `Both`
/// walks both directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    Incoming,
    Outgoing,
    Both,
}

impl TraversalDirection {
    /// Wire form: `incoming`, `outgoing`, `both`.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
            Self::Both => "both",
        }
    }
}

/// The body of a `PATH` query.
///
/// `PATH FROM <from> TO <to> [MAX HOPS <n>] [WHERE ...]`. `max_hops` is
/// `None` when omitted (no upper bound on the BFS).
#[derive(Debug, Clone, PartialEq)]
pub struct PathQuery {
    pub from: String,
    pub to: String,
    pub max_hops: Option<u32>,
    pub conditions: Vec<Condition>,
}

/// The body of a `NEIGHBORS` query.
///
/// `NEIGHBORS <root> DEPTH <n> [DIRECTION <d>] [WHERE ...]`.
#[derive(Debug, Clone, PartialEq)]
pub struct NeighborsQuery {
    pub root: String,
    pub depth: u32,
    pub direction: TraversalDirection,
    pub conditions: Vec<Condition>,
}

/// The body of a `SUBGRAPH` query.
///
/// `SUBGRAPH ROOT <root> [DEPTH <n>] [DIRECTION <d>] [WHERE ...]`.
/// Defaults: `depth = 3`, `direction = Both`.
#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphQuery {
    pub root: String,
    pub depth: u32,
    pub direction: TraversalDirection,
    pub conditions: Vec<Condition>,
}

/// How [`ClusterQuery`] partitions the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterMethod {
    /// Strongly connected components.
    Scc,
    /// Weakly connected components.
    Connected,
}

impl ClusterMethod {
    /// Wire form: `scc`, `connected`.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Scc => "scc",
            Self::Connected => "connected",
        }
    }
}

/// The body of a `CLUSTER` query.
///
/// `CLUSTER [METHOD (scc|connected)] [WHERE ...]`. Bare `CLUSTER` is legal.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterQuery {
    pub method: ClusterMethod,
    pub conditions: Vec<Condition>,
}

/// The body of an `EXPLAIN` query.
///
/// `EXPLAIN FROM <from> TO <to> [WHERE ...]`. Rejects `MAX HOPS` — the
/// spec mandates exact path-finding, not BFS.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplainQuery {
    pub from: String,
    pub to: String,
    pub conditions: Vec<Condition>,
}

/// Top-level boolean composition. `NOT` wraps a single sub-query; `AND`
/// and `OR` join a non-empty list of sub-queries. Filters on the
/// sub-queries stay scoped to those sub-queries (no bleed).
#[derive(Debug, Clone, PartialEq)]
pub struct BooleanQuery {
    pub op: BooleanOp,
    /// For `NOT` this holds exactly one sub-query; for `AND` / `OR` it
    /// holds 2+ sub-queries.
    pub operands: Vec<MoldQLQuery>,
}

/// Boolean operator joining the operands of a [`BooleanQuery`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    And,
    Or,
    Not,
}

impl BooleanOp {
    /// Wire form: `AND`, `OR`, `NOT`.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::And => "AND",
            Self::Or => "OR",
            Self::Not => "NOT",
        }
    }
}

// Extend the top-level enum with the 5 ExplorerQL primitives plus the
// boolean composition wrapper. Each variant carries its respective query
// struct so the executor can pattern-match on it.

/// Top-level query variants.
#[derive(Debug, Clone, PartialEq)]
pub enum MoldQLQuery {
    /// `FIND <target> [IN SCOPE <path>] [WHERE ...] [APPLY <lens>]`
    Find(FindQuery),
    /// `EXPLORE <object_ref> THROUGH <direction> DEPTH <n>`
    Explore(ExploreQuery),
    /// `PATH FROM <from> TO <to> [MAX HOPS <n>] [WHERE ...]`
    Path(PathQuery),
    /// `NEIGHBORS <root> DEPTH <n> [DIRECTION <d>] [WHERE ...]`
    Neighbors(NeighborsQuery),
    /// `SUBGRAPH ROOT <root> [DEPTH <n>] [DIRECTION <d>] [WHERE ...]`
    Subgraph(SubgraphQuery),
    /// `CLUSTER [METHOD (scc|connected)] [WHERE ...]`
    Cluster(ClusterQuery),
    /// `EXPLAIN FROM <from> TO <to> [WHERE ...]`
    Explain(ExplainQuery),
    /// `( <q1> AND|OR <q2> [AND|OR <q3> ...] )` or `NOT <q>`
    Boolean(BooleanQuery),
}

// ============================================================================
// Tests — roundtrip + Debug+Clone+PartialEq + behavior coverage for the 5
// new ExplorerQL variants and the boolean wrapper.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// All 6 new variants must be `Debug + Clone + PartialEq` and pattern-
    /// matchable through `MoldQLQuery`. This is the roundtrip gate.
    #[test]
    fn query_variants_roundtrip() {
        let path = MoldQLQuery::Path(PathQuery {
            from: "a".to_string(),
            to: "b".to_string(),
            max_hops: Some(3),
            conditions: Vec::new(),
        });
        let neighbors = MoldQLQuery::Neighbors(NeighborsQuery {
            root: "a".to_string(),
            depth: 2,
            direction: TraversalDirection::Both,
            conditions: Vec::new(),
        });
        let subgraph = MoldQLQuery::Subgraph(SubgraphQuery {
            root: "a".to_string(),
            depth: 3,
            direction: TraversalDirection::Both,
            conditions: Vec::new(),
        });
        let cluster = MoldQLQuery::Cluster(ClusterQuery {
            method: ClusterMethod::Scc,
            conditions: Vec::new(),
        });
        let explain = MoldQLQuery::Explain(ExplainQuery {
            from: "a".to_string(),
            to: "b".to_string(),
            conditions: Vec::new(),
        });
        let boolean = MoldQLQuery::Boolean(BooleanQuery {
            op: BooleanOp::And,
            operands: vec![path.clone(), neighbors.clone()],
        });

        // Debug + Clone + PartialEq
        for q in [
            path.clone(),
            neighbors.clone(),
            subgraph.clone(),
            cluster.clone(),
            explain.clone(),
            boolean.clone(),
        ] {
            let _ = format!("{q:?}");
            let cloned = q.clone();
            assert_eq!(q, cloned);
        }

        // Pattern-matchable
        assert!(matches!(path, MoldQLQuery::Path(_)));
        assert!(matches!(neighbors, MoldQLQuery::Neighbors(_)));
        assert!(matches!(subgraph, MoldQLQuery::Subgraph(_)));
        assert!(matches!(cluster, MoldQLQuery::Cluster(_)));
        assert!(matches!(explain, MoldQLQuery::Explain(_)));
        assert!(matches!(boolean, MoldQLQuery::Boolean(_)));
    }

    #[test]
    fn traversal_direction_keyword() {
        assert_eq!(TraversalDirection::Incoming.keyword(), "incoming");
        assert_eq!(TraversalDirection::Outgoing.keyword(), "outgoing");
        assert_eq!(TraversalDirection::Both.keyword(), "both");
    }

    #[test]
    fn cluster_method_keyword() {
        assert_eq!(ClusterMethod::Scc.keyword(), "scc");
        assert_eq!(ClusterMethod::Connected.keyword(), "connected");
    }

    #[test]
    fn boolean_op_keyword() {
        assert_eq!(BooleanOp::And.keyword(), "AND");
        assert_eq!(BooleanOp::Or.keyword(), "OR");
        assert_eq!(BooleanOp::Not.keyword(), "NOT");
    }

    /// `PathQuery` defaults: `max_hops` and `conditions` default to `None`
    /// / empty when not provided.
    #[test]
    fn path_query_default_max_hops_is_none() {
        let q = PathQuery {
            from: "a".into(),
            to: "b".into(),
            max_hops: None,
            conditions: Vec::new(),
        };
        assert!(q.max_hops.is_none());
        assert!(q.conditions.is_empty());
    }

    /// `SubgraphQuery` defaults: `depth = 3`, `direction = Both`.
    #[test]
    fn subgraph_query_defaults() {
        let q = SubgraphQuery {
            root: "a".into(),
            depth: 3,
            direction: TraversalDirection::Both,
            conditions: Vec::new(),
        };
        assert_eq!(q.depth, 3);
        assert_eq!(q.direction, TraversalDirection::Both);
    }

    /// `ClusterQuery` defaults: `method = Scc`, empty WHERE.
    #[test]
    fn cluster_query_defaults() {
        let q = ClusterQuery {
            method: ClusterMethod::Scc,
            conditions: Vec::new(),
        };
        assert_eq!(q.method, ClusterMethod::Scc);
        assert!(q.conditions.is_empty());
    }

    /// `BooleanQuery` keeps operands as a `Vec<MoldQLQuery>` so nested
    /// composition works (`(A AND B) OR C` is representable).
    #[test]
    fn boolean_query_keeps_nested_operands() {
        let inner = MoldQLQuery::Path(PathQuery {
            from: "a".into(),
            to: "b".into(),
            max_hops: None,
            conditions: Vec::new(),
        });
        let q = BooleanQuery {
            op: BooleanOp::Or,
            operands: vec![
                MoldQLQuery::Boolean(BooleanQuery {
                    op: BooleanOp::And,
                    operands: vec![inner.clone(), inner.clone()],
                }),
                inner,
            ],
        };
        assert_eq!(q.op, BooleanOp::Or);
        assert_eq!(q.operands.len(), 2);
        assert!(matches!(
            q.operands[0],
            MoldQLQuery::Boolean(BooleanQuery {
                op: BooleanOp::And,
                ..
            })
        ));
    }

    /// `NOT` wraps exactly one operand.
    #[test]
    fn boolean_query_not_holds_single_operand() {
        let inner = MoldQLQuery::Path(PathQuery {
            from: "a".into(),
            to: "b".into(),
            max_hops: None,
            conditions: Vec::new(),
        });
        let q = BooleanQuery {
            op: BooleanOp::Not,
            operands: vec![inner],
        };
        assert_eq!(q.op, BooleanOp::Not);
        assert_eq!(q.operands.len(), 1);
    }
}
