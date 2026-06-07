//! MoldQL AST — pure data types, no parsing or execution logic.
//!
//! MoldQL is the query language for the explorer: a single call that
//! combines filter, scope, and lens into one expression. The AST is the
//! canonical, in-memory representation a parsed query settles into.

/// Top-level query variants.
#[derive(Debug, Clone, PartialEq)]
pub enum MoldQLQuery {
    /// `FIND <target> [IN SCOPE <path>] [WHERE ...] [APPLY <lens>]`
    Find(FindQuery),
    /// `EXPLORE <object_ref> THROUGH <direction> DEPTH <n>`
    Explore(ExploreQuery),
}

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
        self.parts
            .first()
            .map(String::as_str)
            .unwrap_or("")
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
