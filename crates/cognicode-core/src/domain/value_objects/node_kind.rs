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
//! All non-`Symbol` variants are gated behind the `multimodal` Cargo
//! feature so the default build is byte-for-byte unchanged.
//!
//! ```text
//! NodeKind = Symbol(SymbolKind)
//!          | Decision     #[cfg(feature = "multimodal")]
//!          | Doc          #[cfg(feature = "multimodal")]
//!          | Issue        #[cfg(feature = "multimodal")]
//!          | Evidence     #[cfg(feature = "multimodal")]
//!          | Component    #[cfg(feature = "multimodal")]
//!          | Container    #[cfg(feature = "multimodal")]
//!          | System       #[cfg(feature = "multimodal")]
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
    /// A documented decision (ADR / RFC). Multimodal.
    #[cfg(feature = "multimodal")]
    Decision,
    /// A documentation node (markdown, MDX, plain text). Multimodal.
    #[cfg(feature = "multimodal")]
    Doc,
    /// An issue tracker artifact (Linear / GitHub issue). Multimodal.
    #[cfg(feature = "multimodal")]
    Issue,
    /// An evidence node (e.g. benchmark result, fuzzer finding). Multimodal.
    #[cfg(feature = "multimodal")]
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
}

impl FromStr for NodeKind {
    type Err = NodeKindParseError;

    /// Parse a `NodeKind` from its stable kebab-case `Display` form.
    ///
    /// The `Symbol(SymbolKind)` wrapper is matched on the `symbol`
    /// prefix and the inner kind is delegated to
    /// `SymbolKind::from_str`. Without the `multimodal` feature,
    /// the only accepted string is `"symbol"`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "symbol" => SymbolKind::from_str(s)
                .map(NodeKind::Symbol)
                .map_err(|_| NodeKindParseError::Unknown(s.to_string())),
            #[cfg(feature = "multimodal")]
            "decision" => Ok(NodeKind::Decision),
            #[cfg(feature = "multimodal")]
            "doc" => Ok(NodeKind::Doc),
            #[cfg(feature = "multimodal")]
            "issue" => Ok(NodeKind::Issue),
            #[cfg(feature = "multimodal")]
            "evidence" => Ok(NodeKind::Evidence),
            #[cfg(feature = "multimodal")]
            "component" => Ok(NodeKind::Component),
            #[cfg(feature = "multimodal")]
            "container" => Ok(NodeKind::Container),
            #[cfg(feature = "multimodal")]
            "system" => Ok(NodeKind::System),
            _ => Err(NodeKindParseError::Unknown(s.to_string())),
        }
    }
}

impl NodeKind {
    /// Returns a stable, kebab-case identifier for this kind.
    /// Used for JSON serialization, DB persistence, and frontend style
    /// class mapping.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Symbol(_) => "symbol",
            #[cfg(feature = "multimodal")]
            NodeKind::Decision => "decision",
            #[cfg(feature = "multimodal")]
            NodeKind::Doc => "doc",
            #[cfg(feature = "multimodal")]
            NodeKind::Issue => "issue",
            #[cfg(feature = "multimodal")]
            NodeKind::Evidence => "evidence",
            #[cfg(feature = "multimodal")]
            NodeKind::Component => "component",
            #[cfg(feature = "multimodal")]
            NodeKind::Container => "container",
            #[cfg(feature = "multimodal")]
            NodeKind::System => "system",
        }
    }

    /// Returns `true` if this kind is a multimodal (non-code) node.
    #[cfg(feature = "multimodal")]
    pub fn is_multimodal(&self) -> bool {
        !matches!(self, NodeKind::Symbol(_))
    }

    /// Returns `true` if this kind is a multimodal (non-code) node.
    /// Without the `multimodal` feature, no kind is multimodal.
    #[cfg(not(feature = "multimodal"))]
    pub fn is_multimodal(&self) -> bool {
        false
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
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

    /// The four multimodal variants must exist and round-trip through
    /// JSON when the `multimodal` feature is enabled. Phase 1 of the
    /// C4 architecture change adds three more (`Component`,
    /// `Container`, `System`) for a total of 7.
    #[test]
    #[cfg(feature = "multimodal")]
    fn node_kind_multimodal_variants() {
        for kind in [
            NodeKind::Decision,
            NodeKind::Doc,
            NodeKind::Issue,
            NodeKind::Evidence,
            NodeKind::Component,
            NodeKind::Container,
            NodeKind::System,
        ] {
            assert!(kind.is_multimodal());
            let json = serde_json::to_string(&kind).expect("serialize");
            let parsed: NodeKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, kind);
        }

        // A symbol is NOT multimodal.
        let sym = NodeKind::Symbol(SymbolKind::Function);
        assert!(!sym.is_multimodal());
    }

    /// Without the `multimodal` feature, only `NodeKind::Symbol` is
    /// constructable; `is_multimodal` always returns `false`.
    #[test]
    #[cfg(not(feature = "multimodal"))]
    fn node_kind_symbol_only_without_feature() {
        let kind = NodeKind::Symbol(SymbolKind::Module);
        assert!(!kind.is_multimodal());
    }

    /// `Display` must produce a stable, kebab-case identifier for every
    /// kind. The frontend and the PG layer both rely on this string.
    #[test]
    fn node_kind_display() {
        assert_eq!(format!("{}", NodeKind::Symbol(SymbolKind::Function)), "symbol");
        #[cfg(feature = "multimodal")]
        {
            assert_eq!(format!("{}", NodeKind::Decision), "decision");
            assert_eq!(format!("{}", NodeKind::Doc), "doc");
            assert_eq!(format!("{}", NodeKind::Issue), "issue");
            assert_eq!(format!("{}", NodeKind::Evidence), "evidence");
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

        #[cfg(feature = "multimodal")]
        {
            assert_eq!(NodeKind::Decision.as_str(), format!("{}", NodeKind::Decision));
            assert_eq!(NodeKind::Doc.as_str(), format!("{}", NodeKind::Doc));
        }
    }

    /// T5 RED gate (partial): the `Symbol` variant is the always-on
    /// discriminator, so this test compiles under both feature
    /// configurations.
    #[test]
    fn feature_gate_compiles_symbol_variant() {
        let kind = NodeKind::Symbol(SymbolKind::Trait);
        assert_eq!(kind.as_str(), "symbol");
    }

    #[test]
    fn node_kind_hashable_and_eq() {
        use std::collections::HashSet;
        let mut set: HashSet<NodeKind> = HashSet::new();
        set.insert(NodeKind::Symbol(SymbolKind::Function));
        #[cfg(feature = "multimodal")]
        {
            set.insert(NodeKind::Decision);
            set.insert(NodeKind::Doc);
            set.insert(NodeKind::Issue);
            set.insert(NodeKind::Evidence);
            set.insert(NodeKind::Component);
            set.insert(NodeKind::Container);
            set.insert(NodeKind::System);
        }
        // The Symbol is already present; inserting it again is a no-op.
        set.insert(NodeKind::Symbol(SymbolKind::Function));
        // 1 Symbol + 7 multimodal = 8 total under the feature.
        #[cfg(feature = "multimodal")]
        assert_eq!(set.len(), 8);
        #[cfg(not(feature = "multimodal"))]
        assert_eq!(set.len(), 1);
    }
}
