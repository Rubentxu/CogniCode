//! `NodeKind` — value object describing the kind of a node in the
//! generic graph model.
//!
//! Layered on top of the existing code-only `SymbolKind` (22 variants).
//! The `Symbol(SymbolKind)` wrapper preserves exhaustive matching of
//! the legacy taxonomy; the unit variants `Decision`, `Doc`, `Issue`,
//! `Evidence` are new multimodal kinds added by the
//! `multimodal-docs-source` change, and `Component`, `Container`,
//! `System` are C4-model architectural node kinds added by the
//! `c4-architecture-nodes` change.
//!
//! Decision, Doc, and Evidence are available in the default (no-feature) build
//! to support the knowledge-layer read path. Issue, Component, Container, System,
//! and Route remain behind `multimodal` (write-extraction gates).
//!
//! ```text
//! NodeKind = Symbol(SymbolKind)
//!          | Decision     (default build — knowledge read)
//!          | Doc          (default build — knowledge read)
//!          | Evidence     (default build — knowledge read)
//!          | Issue        #[cfg(feature = "multimodal")]
//!          | Component    #[cfg(feature = "multimodal")]
//!          | Container    #[cfg(feature = "multimodal")]
//!          | System       #[cfg(feature = "multimodal")]
//!          | Route        #[cfg(feature = "multimodal")]  // e15.5
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::symbol_kind::SymbolKind;

/// Error type for [`NodeKind::from_str`] failures.
///
/// The parser is intentionally **total** — every stable
/// kebab-case `Display` form (including the `Symbol(SymbolKind)`
/// wrapper's inner kind) is accepted. The error variant is
/// reserved for the day a legacy row carries a kind string that
/// has been removed from the taxonomy; today's parser never
/// produces it, so it is unreachable in practice.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NodeKindParseError {
    /// The supplied string does not match any known kind. Always
    /// paired with the offending input for round-trip-safe error
    /// messages.
    #[error("unknown node kind: {0:?}")]
    Unknown(String),
}

/// The kind of a node in the generic (multimodal) graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    /// A code symbol — wraps the legacy 22-variant `SymbolKind`.
    Symbol(SymbolKind),
    /// A documented decision (ADR / RFC). Available in the default build.
    Decision,
    /// A documentation node (markdown, MDX, plain text). Available in the default build.
    Doc,
    /// An issue tracker artifact (Linear / GitHub issue). Multimodal.
    #[cfg(feature = "multimodal")]
    Issue,
    /// An evidence node (e.g. benchmark result, fuzzer finding). Available in the default build.
    Evidence,
    /// A C4-model component (grouping of related symbols). Multimodal.
    #[cfg(feature = "multimodal")]
    Component,
    /// A C4-model container (deployable unit). Multimodal.
    #[cfg(feature = "multimodal")]
    Container,
    /// A C4-model system (boundary of related containers). Multimodal.
    #[cfg(feature = "multimodal")]
    System,
    /// A runtime API route (HTTP, GraphQL, gRPC). Multimodal.
    /// Stable id form:
    /// - HTTP: `route:HTTP:{METHOD}:{path}` (e.g. `route:HTTP:POST:/api/users`)
    /// - GraphQL: `route:GraphQL:{type}.{field}` (e.g. `route:GraphQL:Query.users`)
    /// - gRPC: `route:gRPC:{service}.{rpc}` (e.g. `route:gRPC:UserService.GetUser`)
    #[cfg(feature = "multimodal")]
    Route,
}

impl FromStr for NodeKind {
    type Err = NodeKindParseError;

    /// Parse a `NodeKind` from its stable `Display` form.
    ///
    /// The `Symbol(SymbolKind)` wrapper is matched on the `"symbol."`
    /// prefix and the inner kind is delegated to
    /// `SymbolKind::from_str`. Bare `"symbol"` is rejected as legacy.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Symbol sub-kind: "symbol.{inner}"
        if let Some(inner) = s.strip_prefix("symbol.") {
            if inner.is_empty() {
                return Err(NodeKindParseError::Unknown(s.to_string()));
            }
            return SymbolKind::from_str(inner)
                .map(NodeKind::Symbol)
                .map_err(|_| NodeKindParseError::Unknown(s.to_string()));
        }
        // Unit variants
        match s {
            "decision" => Ok(NodeKind::Decision),
            "doc" => Ok(NodeKind::Doc),
            "evidence" => Ok(NodeKind::Evidence),
            #[cfg(feature = "multimodal")]
            "issue" => Ok(NodeKind::Issue),
            #[cfg(feature = "multimodal")]
            "component" => Ok(NodeKind::Component),
            #[cfg(feature = "multimodal")]
            "container" => Ok(NodeKind::Container),
            #[cfg(feature = "multimodal")]
            "system" => Ok(NodeKind::System),
            #[cfg(feature = "multimodal")]
            "route" => Ok(NodeKind::Route),
            // Legacy bare "symbol" is rejected — use "symbol.{inner}" instead.
            _ => Err(NodeKindParseError::Unknown(s.to_string())),
        }
    }
}

impl NodeKind {
    /// Returns a stable identifier for this kind.
    /// Used for JSON serialization, DB persistence, and frontend style
    /// class mapping.
    ///
    /// For `Symbol(SymbolKind)` variants, returns `"symbol.{inner}"` where
    /// `{inner}` is the kebab-case name of the inner `SymbolKind`. This makes
    /// sub-kinds distinguishable in the serialized form.
    pub fn as_str(&self) -> String {
        match self {
            NodeKind::Symbol(inner) => format!("symbol.{}", inner),
            NodeKind::Decision => "decision".to_string(),
            NodeKind::Doc => "doc".to_string(),
            #[cfg(feature = "multimodal")]
            NodeKind::Issue => "issue".to_string(),
            NodeKind::Evidence => "evidence".to_string(),
            #[cfg(feature = "multimodal")]
            NodeKind::Component => "component".to_string(),
            #[cfg(feature = "multimodal")]
            NodeKind::Container => "container".to_string(),
            #[cfg(feature = "multimodal")]
            NodeKind::System => "system".to_string(),
            #[cfg(feature = "multimodal")]
            NodeKind::Route => "route".to_string(),
        }
    }

    /// Returns `true` if this kind is a multimodal (non-code) node.
    /// Decision, Doc, and Evidence are available in the default build but
    /// are still semantically "multimodal" kinds (not code symbols).
    pub fn is_multimodal(&self) -> bool {
        !matches!(self, NodeKind::Symbol(_))
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- T1 RED gate tests ----

    /// `NodeKind::Symbol(SymbolKind)` must accept the existing 22-variant
    /// `SymbolKind` payload without losing the inner kind on round-trip.
    #[test]
    fn node_kind_symbol_wraps_existing() {
        let kind = NodeKind::Symbol(SymbolKind::Function);
        assert!(matches!(kind, NodeKind::Symbol(SymbolKind::Function)));

        // JSON roundtrip preserves the inner SymbolKind.
        let json = serde_json::to_string(&kind).expect("serialize");
        let parsed: NodeKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, kind);

        // Different inner kinds remain distinguishable.
        let class = NodeKind::Symbol(SymbolKind::Class);
        let json = serde_json::to_string(&class).unwrap();
        let parsed: NodeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, class);
        assert_ne!(parsed, kind);
    }

    /// Decision, Doc, and Evidence are always available (knowledge-layer read path).
    /// Issue, Component, Container, System, Route require the `multimodal` feature.
    #[test]
    fn node_kind_knowledge_variants() {
        // Decision, Doc, Evidence are always available.
        for kind in [NodeKind::Decision, NodeKind::Doc, NodeKind::Evidence] {
            assert!(kind.is_multimodal(), "{kind:?} should be multimodal");
            let json = serde_json::to_string(&kind).expect("serialize");
            let parsed: NodeKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, kind);
        }

        // A symbol is NOT multimodal.
        let sym = NodeKind::Symbol(SymbolKind::Function);
        assert!(!sym.is_multimodal());
    }

    /// The remaining multimodal-only variants (Issue, Component, Container, System, Route)
    /// are only tested when the `multimodal` feature is enabled.
    #[test]
    #[cfg(feature = "multimodal")]
    fn node_kind_multimodal_only_variants() {
        for kind in [
            NodeKind::Issue,
            NodeKind::Component,
            NodeKind::Container,
            NodeKind::System,
            NodeKind::Route,
        ] {
            assert!(kind.is_multimodal());
            let json = serde_json::to_string(&kind).expect("serialize");
            let parsed: NodeKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, kind);
        }
    }

    /// `Display` must produce a stable, kebab-case identifier for every
    /// kind. The frontend and the PG layer both rely on this string.
    #[test]
    fn node_kind_display() {
        // After Task 1.3: Symbol variants emit "symbol.{inner}" not bare "symbol"
        assert_eq!(
            format!("{}", NodeKind::Symbol(SymbolKind::Function)),
            "symbol.function"
        );
        // Knowledge-layer variants: always available.
        assert_eq!(format!("{}", NodeKind::Decision), "decision");
        assert_eq!(format!("{}", NodeKind::Doc), "doc");
        assert_eq!(format!("{}", NodeKind::Evidence), "evidence");
        #[cfg(feature = "multimodal")]
        {
            assert_eq!(format!("{}", NodeKind::Issue), "issue");
            assert_eq!(format!("{}", NodeKind::Component), "component");
            assert_eq!(format!("{}", NodeKind::Container), "container");
            assert_eq!(format!("{}", NodeKind::System), "system");
        }
    }

    // ---- Additional TDD coverage ----

    #[test]
    fn node_kind_as_str_matches_display() {
        // as_str and Display must agree (Display is just a thin wrapper).
        let sym = NodeKind::Symbol(SymbolKind::Class);
        assert_eq!(sym.as_str(), format!("{}", sym));

        // Knowledge-layer variants: always available.
        assert_eq!(
            NodeKind::Decision.as_str(),
            format!("{}", NodeKind::Decision)
        );
        assert_eq!(NodeKind::Doc.as_str(), format!("{}", NodeKind::Doc));
        assert_eq!(
            NodeKind::Evidence.as_str(),
            format!("{}", NodeKind::Evidence)
        );

        #[cfg(feature = "multimodal")]
        {
            assert_eq!(NodeKind::Issue.as_str(), format!("{}", NodeKind::Issue));
            assert_eq!(
                NodeKind::Component.as_str(),
                format!("{}", NodeKind::Component)
            );
            assert_eq!(
                NodeKind::Container.as_str(),
                format!("{}", NodeKind::Container)
            );
            assert_eq!(NodeKind::System.as_str(), format!("{}", NodeKind::System));
            assert_eq!(NodeKind::Route.as_str(), format!("{}", NodeKind::Route));
        }
    }

    /// T5 RED gate (partial): the `Symbol` variant is the always-on
    /// discriminator, so this test compiles under both feature
    /// configurations.
    #[test]
    fn feature_gate_compiles_symbol_variant() {
        let kind = NodeKind::Symbol(SymbolKind::Trait);
        // After Task 1.3: Symbol variant emits "symbol.trait" not bare "symbol"
        assert_eq!(kind.as_str(), "symbol.trait");
        assert_eq!(format!("{}", kind), "symbol.trait");
    }

    #[test]
    fn node_kind_hashable_and_eq() {
        use std::collections::HashSet;
        let mut set: HashSet<NodeKind> = HashSet::new();
        set.insert(NodeKind::Symbol(SymbolKind::Function));
        // Knowledge-layer variants: always available.
        set.insert(NodeKind::Decision);
        set.insert(NodeKind::Doc);
        set.insert(NodeKind::Evidence);
        #[cfg(feature = "multimodal")]
        {
            set.insert(NodeKind::Issue);
            set.insert(NodeKind::Component);
            set.insert(NodeKind::Container);
            set.insert(NodeKind::System);
            set.insert(NodeKind::Route);
        }
        // The Symbol is already present; inserting it again is a no-op.
        set.insert(NodeKind::Symbol(SymbolKind::Function));
        // 1 Symbol + 3 always-on + 5 multimodal = 9 total under the feature,
        // 1 Symbol + 3 always-on = 4 total without.
        #[cfg(feature = "multimodal")]
        assert_eq!(set.len(), 9);
        #[cfg(not(feature = "multimodal"))]
        assert_eq!(set.len(), 4);
    }

    /// `NodeKind::Route` is multimodal, parses from "route", and emits "route"
    /// via `Display`. Phase 2 of e15.5 — API route ingestion.
    #[test]
    #[cfg(feature = "multimodal")]
    fn node_kind_route_roundtrip() {
        let route = NodeKind::Route;
        assert!(route.is_multimodal());
        assert_eq!(route.as_str(), "route");
        assert_eq!(format!("{}", route), "route");

        // JSON roundtrip
        let json = serde_json::to_string(&route).expect("serialize");
        let parsed: NodeKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, route);

        // FromStr
        let parsed_from_str = NodeKind::from_str("route").expect("parse route");
        assert_eq!(parsed_from_str, route);
    }

    // -------------------------------------------------------------------------
    // Task 1.3a RED — Symbol sub-kinds produce distinct strings
    // Scenario: `generic-graph-model::Symbol sub-kinds produce distinct strings`
    // Assert: `Symbol(Function).to_string()=="symbol.function"` round-trips
    // -------------------------------------------------------------------------

    /// `NodeKind::Symbol(SymbolKind::Function).to_string()` must be `"symbol.function"`.
    #[test]
    fn node_kind_symbol_function_display() {
        assert_eq!(
            NodeKind::Symbol(SymbolKind::Function).to_string(),
            "symbol.function"
        );
    }

    /// `NodeKind::Symbol(SymbolKind::Class).to_string()` must be `"symbol.class"`.
    #[test]
    fn node_kind_symbol_class_display() {
        assert_eq!(
            NodeKind::Symbol(SymbolKind::Class).to_string(),
            "symbol.class"
        );
    }

    /// `NodeKind::Symbol(SymbolKind::Method).to_string()` must be `"symbol.method"`.
    #[test]
    fn node_kind_symbol_method_display() {
        assert_eq!(
            NodeKind::Symbol(SymbolKind::Method).to_string(),
            "symbol.method"
        );
    }

    /// `from_str("symbol.function")` must parse to `NodeKind::Symbol(SymbolKind::Function)`.
    #[test]
    fn node_kind_from_str_symbol_function() {
        let parsed: NodeKind = "symbol.function".parse().expect("parse symbol.function");
        assert_eq!(parsed, NodeKind::Symbol(SymbolKind::Function));
    }

    /// `from_str("symbol.class")` must parse to `NodeKind::Symbol(SymbolKind::Class)`.
    #[test]
    fn node_kind_from_str_symbol_class() {
        let parsed: NodeKind = "symbol.class".parse().expect("parse symbol.class");
        assert_eq!(parsed, NodeKind::Symbol(SymbolKind::Class));
    }

    // -------------------------------------------------------------------------
    // Task 1.3a RED — Legacy rejection
    // Scenario: `generic-graph-model::Unit variants and legacy rejection`
    // Assert: `from_str("symbol")`→`Err(Unknown)`
    // -------------------------------------------------------------------------

    /// Bare `"symbol"` must be rejected as a legacy format.
    #[test]
    fn node_kind_from_str_bare_symbol_rejected() {
        let result: Result<NodeKind, _> = "symbol".parse();
        assert!(
            result.is_err(),
            "bare 'symbol' must be rejected as legacy format"
        );
    }

    // -------------------------------------------------------------------------
    // Task 1.3a — as_str matches Display for Symbol variants
    // -------------------------------------------------------------------------

    /// `as_str()` for `Symbol(Function)` must return `"symbol.function"`.
    #[test]
    fn node_kind_as_str_symbol_function() {
        assert_eq!(
            NodeKind::Symbol(SymbolKind::Function).as_str(),
            "symbol.function"
        );
    }
}
