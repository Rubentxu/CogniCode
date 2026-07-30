//! `EdgeKind` — value object describing the kind of an edge in the
//! generic (multimodal) graph model.
//!
//! Layered on top of the existing code-only `DependencyType` (8
//! variants). The `Dependency(DependencyType)` wrapper preserves
//! exhaustive matching of the legacy taxonomy; the unit variants
//! `Cites`, `Justifies`, `Resolves`, `CorroboratedBy` are new
//! multimodal relationship kinds added by the `multimodal-docs-source`
//! change, and `PartOf`, `DeployedAs`, `InSystem` are C4-model
//! architectural relationship kinds added by the
//! `c4-architecture-nodes` change.
//!
//! Cites, Justifies, Resolves, and CorroboratedBy are available in the default
//! (no-feature) build to support the knowledge-layer read path. PartOf,
//! DeployedAs, InSystem, and the protocol call edges remain behind `multimodal`
//! (write-extraction gates).
//!
//! ```text
//! EdgeKind = Dependency(DependencyType)
//!          | Cites              (default build — knowledge read)
//!          | Justifies          (default build — knowledge read)
//!          | Resolves           (default build — knowledge read)
//!          | CorroboratedBy     (default build — knowledge read)
//!          | PartOf             #[cfg(feature = "multimodal")]
//!          | DeployedAs         #[cfg(feature = "multimodal")]
//!          | InSystem           #[cfg(feature = "multimodal")]
//!          | HttpCalls          #[cfg(feature = "multimodal")]  // e15.5
//!          | GraphqlCalls       #[cfg(feature = "multimodal")]  // e16.5
//!          | GrpcCalls          #[cfg(feature = "multimodal")]  // e17.5
//!          | TrpcCalls          #[cfg(feature = "multimodal")]  // e19
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::dependency_type::DependencyType;

/// Error type for [`EdgeKind::from_str`] failures.
///
/// The parser is intentionally **total** for the `Display` form of
/// every variant. The error variant exists for forward-compatibility
/// (e.g. a legacy row carrying a kind string that has been removed
/// from the taxonomy).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EdgeKindParseError {
    /// The supplied string does not match any known kind. Always
    /// paired with the offending input for round-trip-safe error
    /// messages.
    #[error("unknown edge kind: {0:?}")]
    Unknown(String),
}

/// The kind of an edge in the generic (multimodal) graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    /// A code-level dependency — wraps the legacy 8-variant
    /// `DependencyType` (Calls, Imports, …).
    Dependency(DependencyType),
    /// `source` cites `target` (e.g. a doc references a code symbol).
    /// Available in the default build.
    Cites,
    /// `source` justifies `target` (e.g. an ADR justifies an architectural choice).
    /// Available in the default build.
    Justifies,
    /// `source` resolves `target` (e.g. a PR resolves an issue).
    /// Available in the default build.
    Resolves,
    /// `source` is corroborated by `target` (e.g. a test result
    /// corroborates a claim in a design doc). Available in the default build.
    CorroboratedBy,
    /// `source` is part of `target` (e.g. a component is part of
    /// a container). Multimodal.
    #[cfg(feature = "multimodal")]
    PartOf,
    /// `source` is deployed as `target` (e.g. a container is
    /// deployed as a service). Multimodal.
    #[cfg(feature = "multimodal")]
    DeployedAs,
    /// `source` belongs to `target` system (e.g. a container
    /// belongs to a system). Multimodal.
    #[cfg(feature = "multimodal")]
    InSystem,
    /// `source` depends on `target` (e.g. a container depends on
    /// another container at the C4 level). Multimodal.
    #[cfg(feature = "multimodal")]
    DependsOn,
    /// `Route` invokes `Function` (HTTP). Multimodal.
    /// Direction: `Route -> Handler`. Phase 2 (e15.5).
    #[cfg(feature = "multimodal")]
    HttpCalls,
    /// `Route` invokes `Resolver` (GraphQL). Multimodal.
    /// Direction: `Route -> Resolver`. Phase 3 (e16.5).
    #[cfg(feature = "multimodal")]
    GraphqlCalls,
    /// `Route` invokes `ServiceImpl` (gRPC). Multimodal.
    /// Direction: `Route -> Implementation`. Phase 4 (e17.5).
    #[cfg(feature = "multimodal")]
    GrpcCalls,
    /// `Route` invokes `Procedure` (tRPC). Multimodal.
    /// Direction: `Route -> Procedure`. Phase 5 (e19, deferred).
    #[cfg(feature = "multimodal")]
    TrpcCalls,
}

impl FromStr for EdgeKind {
    type Err = EdgeKindParseError;

    /// Parse an `EdgeKind` from its stable dotted or kebab-case
    /// `Display` form. The `Dependency(...)` wrapper is matched on
    /// the `dependency.` prefix and the inner kind is delegated to
    /// `DependencyType::from_str`. Without the `multimodal`
    /// feature, only the `dependency.*` strings are accepted.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // The rationale-edge variants (Cites, Justifies, Resolves, CorroboratedBy)
        // take precedence on the bare string so we can never accidentally re-interpret
        // them as DependencyType. The `dependency.` prefix disambiguates the wrapper.
        match s {
            "cites" => return Ok(EdgeKind::Cites),
            "justifies" => return Ok(EdgeKind::Justifies),
            "resolves" => return Ok(EdgeKind::Resolves),
            "corroborated_by" => return Ok(EdgeKind::CorroboratedBy),
            // C4-model and protocol edges: still require the `multimodal` feature.
            #[cfg(feature = "multimodal")]
            "part_of" => return Ok(EdgeKind::PartOf),
            #[cfg(feature = "multimodal")]
            "deployed_as" => return Ok(EdgeKind::DeployedAs),
            #[cfg(feature = "multimodal")]
            "in_system" => return Ok(EdgeKind::InSystem),
            #[cfg(feature = "multimodal")]
            "depends_on" => return Ok(EdgeKind::DependsOn),
            // Cross-service protocol edges (Phase 2 — e15.5)
            #[cfg(feature = "multimodal")]
            "http_calls" => return Ok(EdgeKind::HttpCalls),
            #[cfg(feature = "multimodal")]
            "graphql_calls" => return Ok(EdgeKind::GraphqlCalls),
            #[cfg(feature = "multimodal")]
            "grpc_calls" => return Ok(EdgeKind::GrpcCalls),
            #[cfg(feature = "multimodal")]
            "trpc_calls" => return Ok(EdgeKind::TrpcCalls),
            _ => {}
        }
        if let Some(rest) = s.strip_prefix("dependency.") {
            return DependencyType::from_str(rest)
                .map(EdgeKind::Dependency)
                .map_err(|_| EdgeKindParseError::Unknown(s.to_string()));
        }
        // Belt-and-braces: also accept the bare DependencyType
        // strings (e.g. `"calls"`) so a row that was persisted
        // before the `dependency.` prefix was added still parses.
        // We try `DependencyType::from_str` first; if that
        // succeeds we wrap, otherwise the input is unknown.
        if let Ok(dt) = DependencyType::from_str(s) {
            return Ok(EdgeKind::Dependency(dt));
        }
        Err(EdgeKindParseError::Unknown(s.to_string()))
    }
}

impl EdgeKind {
    /// Returns a stable kebab-case identifier for this kind.
    /// For `Dependency`, the inner `DependencyType` is included in
    /// dotted form (e.g. `dependency.calls`).
    pub fn as_str(&self) -> String {
        match self {
            EdgeKind::Dependency(d) => format!("dependency.{}", d),
            EdgeKind::Cites => "cites".to_string(),
            EdgeKind::Justifies => "justifies".to_string(),
            EdgeKind::Resolves => "resolves".to_string(),
            EdgeKind::CorroboratedBy => "corroborated_by".to_string(),
            #[cfg(feature = "multimodal")]
            EdgeKind::PartOf => "part_of".to_string(),
            #[cfg(feature = "multimodal")]
            EdgeKind::DeployedAs => "deployed_as".to_string(),
            #[cfg(feature = "multimodal")]
            EdgeKind::InSystem => "in_system".to_string(),
            #[cfg(feature = "multimodal")]
            EdgeKind::DependsOn => "depends_on".to_string(),
            // Cross-service protocol edges (Phase 2 — e15.5)
            #[cfg(feature = "multimodal")]
            EdgeKind::HttpCalls => "http_calls".to_string(),
            #[cfg(feature = "multimodal")]
            EdgeKind::GraphqlCalls => "graphql_calls".to_string(),
            #[cfg(feature = "multimodal")]
            EdgeKind::GrpcCalls => "grpc_calls".to_string(),
            #[cfg(feature = "multimodal")]
            EdgeKind::TrpcCalls => "trpc_calls".to_string(),
        }
    }

    /// Returns `true` if this kind is a multimodal (non-code) edge.
    /// Cites, Justifies, Resolves, and CorroboratedBy are available in the
    /// default build but are still semantically multimodal kinds.
    pub fn is_multimodal(&self) -> bool {
        !matches!(self, EdgeKind::Dependency(_))
    }
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- T2 RED gate tests ----

    /// `EdgeKind::Dependency(DependencyType)` must accept the existing
    /// 8-variant `DependencyType` payload without losing the inner kind.
    #[test]
    fn edge_kind_dependency_wraps_existing() {
        let kind = EdgeKind::Dependency(DependencyType::Calls);
        assert!(matches!(kind, EdgeKind::Dependency(DependencyType::Calls)));

        // JSON roundtrip preserves the inner DependencyType.
        let json = serde_json::to_string(&kind).expect("serialize");
        let parsed: EdgeKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, kind);

        // Different inner kinds remain distinguishable.
        let imports = EdgeKind::Dependency(DependencyType::Imports);
        let json = serde_json::to_string(&imports).unwrap();
        let parsed: EdgeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, imports);
        assert_ne!(parsed, kind);
    }

    /// The four rationale edge variants (Cites, Justifies, Resolves, CorroboratedBy)
    /// are always available. The C4-model and protocol edges still require `multimodal`.
    #[test]
    fn edge_kind_rationale_variants() {
        // Rationale edges: always available, always multimodal.
        for kind in [
            EdgeKind::Cites,
            EdgeKind::Justifies,
            EdgeKind::Resolves,
            EdgeKind::CorroboratedBy,
        ] {
            assert!(kind.is_multimodal());
            let json = serde_json::to_string(&kind).expect("serialize");
            let parsed: EdgeKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, kind);
        }

        // A Dependency edge is NOT multimodal.
        let dep = EdgeKind::Dependency(DependencyType::Calls);
        assert!(!dep.is_multimodal());
    }

    /// C4-model and protocol edges require the `multimodal` feature.
    #[test]
    #[cfg(feature = "multimodal")]
    fn edge_kind_multimodal_only_variants() {
        for kind in [
            EdgeKind::PartOf,
            EdgeKind::DeployedAs,
            EdgeKind::InSystem,
            EdgeKind::HttpCalls,
            EdgeKind::GraphqlCalls,
            EdgeKind::GrpcCalls,
            EdgeKind::TrpcCalls,
        ] {
            assert!(kind.is_multimodal());
            let json = serde_json::to_string(&kind).expect("serialize");
            let parsed: EdgeKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, kind);
        }
    }

    /// `Display` must produce a stable dotted identifier for every kind.
    /// The frontend style-class mapping and the PG layer both rely on
    /// this string.
    #[test]
    fn edge_kind_display() {
        assert_eq!(
            format!("{}", EdgeKind::Dependency(DependencyType::Calls)),
            "dependency.calls"
        );
        assert_eq!(
            format!("{}", EdgeKind::Dependency(DependencyType::UsesGeneric)),
            "dependency.uses_generic"
        );
        // Rationale edges: always available.
        assert_eq!(format!("{}", EdgeKind::Cites), "cites");
        assert_eq!(format!("{}", EdgeKind::Justifies), "justifies");
        assert_eq!(format!("{}", EdgeKind::Resolves), "resolves");
        assert_eq!(format!("{}", EdgeKind::CorroboratedBy), "corroborated_by");
        #[cfg(feature = "multimodal")]
        {
            assert_eq!(format!("{}", EdgeKind::PartOf), "part_of");
            assert_eq!(format!("{}", EdgeKind::DeployedAs), "deployed_as");
            assert_eq!(format!("{}", EdgeKind::InSystem), "in_system");
            // Cross-service protocol edges (Phase 2 — e15.5)
            assert_eq!(format!("{}", EdgeKind::HttpCalls), "http_calls");
            assert_eq!(format!("{}", EdgeKind::GraphqlCalls), "graphql_calls");
            assert_eq!(format!("{}", EdgeKind::GrpcCalls), "grpc_calls");
            assert_eq!(format!("{}", EdgeKind::TrpcCalls), "trpc_calls");
        }
    }

    // ---- Additional TDD coverage ----

    #[test]
    fn edge_kind_as_str_matches_display() {
        let dep = EdgeKind::Dependency(DependencyType::Inherits);
        assert_eq!(dep.as_str(), format!("{}", dep));

        // Rationale edges: always available.
        assert_eq!(EdgeKind::Cites.as_str(), format!("{}", EdgeKind::Cites));
        assert_eq!(
            EdgeKind::CorroboratedBy.as_str(),
            format!("{}", EdgeKind::CorroboratedBy)
        );

        #[cfg(feature = "multimodal")]
        {
            assert_eq!(EdgeKind::PartOf.as_str(), format!("{}", EdgeKind::PartOf));
            assert_eq!(
                EdgeKind::DeployedAs.as_str(),
                format!("{}", EdgeKind::DeployedAs)
            );
        }
    }

    /// T5 RED gate (partial): the `Dependency` variant is the always-on
    /// discriminator, so this test compiles under both feature
    /// configurations.
    #[test]
    fn feature_gate_compiles_dependency_variant() {
        let kind = EdgeKind::Dependency(DependencyType::Defines);
        assert_eq!(kind.as_str(), "dependency.defines");
    }

    #[test]
    fn edge_kind_hashable_and_eq() {
        use std::collections::HashSet;
        let mut set: HashSet<EdgeKind> = HashSet::new();
        set.insert(EdgeKind::Dependency(DependencyType::Calls));
        // Rationale edges: always available.
        set.insert(EdgeKind::Cites);
        set.insert(EdgeKind::Justifies);
        set.insert(EdgeKind::Resolves);
        set.insert(EdgeKind::CorroboratedBy);
        #[cfg(feature = "multimodal")]
        {
            set.insert(EdgeKind::PartOf);
            set.insert(EdgeKind::DeployedAs);
            set.insert(EdgeKind::InSystem);
            // Cross-service protocol edges (Phase 2 — e15.5)
            set.insert(EdgeKind::HttpCalls);
            set.insert(EdgeKind::GraphqlCalls);
            set.insert(EdgeKind::GrpcCalls);
            set.insert(EdgeKind::TrpcCalls);
        }
        set.insert(EdgeKind::Dependency(DependencyType::Calls));
        // 1 Dependency + 4 rationale = 5 always-on;
        // + 7 multimodal = 12 total under the feature,
        // 1 Dependency + 4 rationale = 5 total without.
        #[cfg(feature = "multimodal")]
        assert_eq!(set.len(), 12);
        #[cfg(not(feature = "multimodal"))]
        assert_eq!(set.len(), 5);
    }

    /// Cross-service protocol edges (Phase 2 — e15.5) parse from their
    /// stable kebab-case form and round-trip through JSON.
    #[test]
    #[cfg(feature = "multimodal")]
    fn edge_kind_protocol_calls_roundtrip() {
        for (kind, str_form) in [
            (EdgeKind::HttpCalls, "http_calls"),
            (EdgeKind::GraphqlCalls, "graphql_calls"),
            (EdgeKind::GrpcCalls, "grpc_calls"),
            (EdgeKind::TrpcCalls, "trpc_calls"),
        ] {
            assert!(kind.is_multimodal());
            assert_eq!(kind.as_str(), str_form);
            assert_eq!(format!("{}", kind), str_form);

            // JSON roundtrip
            let json = serde_json::to_string(&kind).expect("serialize");
            let parsed: EdgeKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, kind);

            // FromStr
            let parsed_from_str = EdgeKind::from_str(str_form).expect("parse");
            assert_eq!(parsed_from_str, kind);
        }
    }
}
