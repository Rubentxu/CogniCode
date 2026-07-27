//! Generic graph aggregates for the multimodal-docs-source change.
//!
//! This module hosts the two aggregate types that sit on top of the
//! `NodeKind` / `EdgeKind` value objects:
//!
//! - [`NodeId`] — a newtype `String` identifier for a node in the
//!   generic graph. The discriminator (`NodeKind`) is stored on the
//!   owning [`GraphNode`] rather than on the id, to keep `NodeId` a
//!   simple value-typed newtype.
//! - [`GraphNode`] — a heterogeneous graph node. Built via the
//!   [`GraphNodeBuilder`] fluent constructor.
//! - [`GraphEdge`] — a directed, typed edge between two nodes with a
//!   [`Provenance`] tag, a normalized confidence in `[0.0, 1.0]`, and
//!   an open `metadata` map for free-form extractor hints.
//!
//! The whole module is feature-gated behind the `multimodal` Cargo
//! feature. It pulls in types from `node_kind` and `edge_kind` whose
//! multimodal variants are also gated; without the feature this
//! module is empty.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::domain::value_objects::edge_kind::EdgeKind;
use crate::domain::value_objects::node_kind::NodeKind;
use crate::domain::value_objects::provenance::Provenance;

// ============================================================================
// NodeId
// ============================================================================

/// Stable identifier for a node in the generic (multimodal) graph.
///
/// `NodeId` is a thin newtype wrapper around `String`. The owning
/// [`GraphNode`] carries the [`NodeKind`] discriminator; the id is
/// opaque from the type system's point of view.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    /// Constructs a `NodeId` from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the underlying id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper and returns the inner `String`.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ============================================================================
// GraphEdge
// ============================================================================

/// Errors returned by [`GraphEdge::new`].
#[derive(Debug, Error, PartialEq)]
pub enum GraphEdgeError {
    /// `confidence` was outside the closed interval `[0.0, 1.0]`.
    #[error("confidence {0} is outside the closed interval [0.0, 1.0]")]
    ConfidenceOutOfRange(f64),
    /// `confidence` was `NaN`, `+∞` or `-∞`.
    #[error("confidence must be a finite number (no NaN or infinity)")]
    ConfidenceNotFinite,
    /// `source == target` — self-loops are not allowed.
    #[error("self-loops are not allowed: source and target are equal ({0})")]
    SelfLoop(String),
}

/// A directed, typed edge in the generic (multimodal) graph.
///
/// `GraphEdge` replaces the older code-only `EdgeMetadata` (which had
/// `caller_id`/`callee_id` fields) for the multimodal paths. The
/// source and target ids are generic [`NodeId`]s, the relationship is
/// a [`EdgeKind`], and the edge carries a [`Provenance`] tag and a
/// confidence score in `[0.0, 1.0]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    /// The originating node.
    pub source: NodeId,
    /// The destination node.
    pub target: NodeId,
    /// The relationship kind.
    pub kind: EdgeKind,
    /// How the edge was obtained (AST extractor, docs parser, …).
    pub provenance: Provenance,
    /// Normalized confidence in the closed interval `[0.0, 1.0]`.
    /// Must also be finite (no `NaN` or infinities).
    pub confidence: f64,
    /// Free-form metadata (key=value hints from the extractor).
    /// Defaults to JSON `Null`; never absent so callers can iterate
    /// without an extra `Option` layer. Use `to_map()` for backward-
    /// compatible `HashMap<String, String>` access.
    #[serde(default)]
    pub metadata: Value,
}

impl GraphEdge {
    /// Constructs a `GraphEdge` after validating the invariants.
    ///
    /// Invariants:
    /// 1. `confidence.is_finite()` (rejects `NaN`, `+∞`, `-∞`).
    /// 2. `confidence ∈ [0.0, 1.0]`.
    /// 3. `source != target` (no self-loops).
    pub fn new(
        source: NodeId,
        target: NodeId,
        kind: EdgeKind,
        provenance: Provenance,
        confidence: f64,
    ) -> Result<Self, GraphEdgeError> {
        if !confidence.is_finite() {
            return Err(GraphEdgeError::ConfidenceNotFinite);
        }
        if !(0.0..=1.0).contains(&confidence) {
            return Err(GraphEdgeError::ConfidenceOutOfRange(confidence));
        }
        if source == target {
            return Err(GraphEdgeError::SelfLoop(source.0));
        }
        Ok(Self {
            source,
            target,
            kind,
            provenance,
            confidence,
            metadata: Value::Null,
        })
    }

    /// Returns `true` if this edge has any metadata key set.
    pub fn has_metadata(&self) -> bool {
        match &self.metadata {
            Value::Object(map) => !map.is_empty(),
            Value::Array(arr) => !arr.is_empty(),
            Value::Null => false,
            _ => true,
        }
    }

    /// Inserts a string metadata key, returning `self` for chaining.
    /// This is the flat-string shim for backward compatibility.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if self.metadata.is_null() {
            self.metadata = Value::Object(serde_json::Map::new());
        }
        if let Value::Object(ref mut map) = self.metadata {
            map.insert(key.into(), Value::String(value.into()));
        }
        self
    }

    /// Converts `metadata` to a `HashMap<String, String>` for backward
    /// compatibility. Non-string values are skipped.
    pub fn metadata_to_map(&self) -> HashMap<String, String> {
        match &self.metadata {
            Value::Object(map) => map
                .iter()
                .filter_map(|(k, v)| {
                    v.as_str().map(|s| (k.clone(), s.to_string()))
                })
                .collect(),
            _ => HashMap::new(),
        }
    }
}

// ============================================================================
// GraphNode
// ============================================================================

/// A heterogeneous graph node. Constructed via [`GraphNodeBuilder`]
/// to keep call sites readable and to provide sensible defaults for
/// timestamps and the open `properties` map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    /// Stable identifier.
    pub id: NodeId,
    /// The kind discriminator.
    pub kind: NodeKind,
    /// Human-readable label (e.g. function name, ADR title, issue title).
    pub label: String,
    /// Source file / URL the node was extracted from, if applicable.
    pub source_path: Option<PathBuf>,
    /// Open key=value map for kind-specific attributes.
    /// Stored as JSONB (`serde_json::Value`) to support structured data
    /// (nested objects, arrays) beyond flat strings. Default is
    /// `serde_json::Value::Object(Default::default())` (empty object `{}`).
    /// Use `to_map()` for backward-compatible `HashMap<String, String>` access.
    #[serde(default)]
    pub properties: Value,
    /// UTC timestamp of creation.
    pub created_at: DateTime<Utc>,
    /// UTC timestamp of the last update.
    pub updated_at: DateTime<Utc>,
}

impl GraphNode {
    /// Returns a fresh builder seeded with `id` and `kind` and the
    /// current UTC timestamp for both `created_at` and `updated_at`.
    pub fn builder(id: impl Into<NodeId>, kind: NodeKind) -> GraphNodeBuilder {
        GraphNodeBuilder::new(id, kind)
    }

    /// Inserts a property (string value), returning `self` for chaining.
    /// Updates `updated_at` to the current UTC time.
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if self.properties.is_null() {
            self.properties = Value::Object(serde_json::Map::new());
        }
        if let Value::Object(ref mut map) = self.properties {
            map.insert(key.into(), Value::String(value.into()));
        }
        self.updated_at = Utc::now();
        self
    }

    /// Inserts a property with any JSON value, returning `self` for chaining.
    /// Updates `updated_at` to the current UTC time.
    pub fn with_property_json(mut self, key: impl Into<String>, value: Value) -> Self {
        if self.properties.is_null() {
            self.properties = Value::Object(serde_json::Map::new());
        }
        if let Value::Object(ref mut map) = self.properties {
            map.insert(key.into(), value);
        }
        self.updated_at = Utc::now();
        self
    }

    /// Converts `properties` to a `HashMap<String, String>` for backward
    /// compatibility. Non-string values are skipped.
    pub fn properties_to_map(&self) -> HashMap<String, String> {
        match &self.properties {
            Value::Object(map) => map
                .iter()
                .filter_map(|(k, v)| {
                    v.as_str().map(|s| (k.clone(), s.to_string()))
                })
                .collect(),
            _ => HashMap::new(),
        }
    }
}

/// Fluent builder for [`GraphNode`].
///
/// Defaults:
/// - `label`: empty string
/// - `source_path`: `None`
/// - `properties`: empty JSON object `{}`
/// - `created_at` / `updated_at`: the time at which `id`/`kind` were
///   passed to [`GraphNodeBuilder::new`].
pub struct GraphNodeBuilder {
    id: NodeId,
    kind: NodeKind,
    label: String,
    source_path: Option<PathBuf>,
    properties: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl GraphNodeBuilder {
    /// Creates a new builder with empty defaults and the current UTC
    /// time for both timestamps.
    pub fn new(id: impl Into<NodeId>, kind: NodeKind) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            kind,
            label: String::new(),
            source_path: None,
            properties: Value::Object(serde_json::Map::new()),
            created_at: now,
            updated_at: now,
        }
    }

    /// Sets the human-readable label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Sets the source path / URL.
    pub fn source_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Inserts a single property with a string value.
    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let Value::Object(ref mut map) = self.properties {
            map.insert(key.into(), Value::String(value.into()));
        }
        self
    }

    /// Inserts a single property with any JSON value.
    pub fn property_json(mut self, key: impl Into<String>, value: Value) -> Self {
        if self.properties.is_null() {
            self.properties = Value::Object(serde_json::Map::new());
        }
        if let Value::Object(ref mut map) = self.properties {
            map.insert(key.into(), value);
        }
        self
    }

    /// Inserts many string properties in one shot.
    pub fn properties(
        mut self,
        props: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        if self.properties.is_null() {
            self.properties = Value::Object(serde_json::Map::new());
        }
        if let Value::Object(ref mut map) = self.properties {
            for (k, v) in props {
                map.insert(k.into(), Value::String(v.into()));
            }
        }
        self
    }

    /// Imports properties from a `HashMap<String, String>` (backward compat adapter).
    pub fn properties_from_map(mut self, map: &HashMap<String, String>) -> Self {
        self.properties = Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect(),
        );
        self
    }

    /// Overrides the `created_at` timestamp.
    pub fn created_at(mut self, ts: DateTime<Utc>) -> Self {
        self.created_at = ts;
        self
    }

    /// Overrides the `updated_at` timestamp.
    pub fn updated_at(mut self, ts: DateTime<Utc>) -> Self {
        self.updated_at = ts;
        self
    }

    /// Finalizes the builder, returning a fully-formed `GraphNode`.
    pub fn build(self) -> GraphNode {
        GraphNode {
            id: self.id,
            kind: self.kind,
            label: self.label,
            source_path: self.source_path,
            properties: self.properties,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::dependency_type::DependencyType;
    use crate::domain::value_objects::symbol_kind::SymbolKind;
    use chrono::TimeZone;

    // ---- helpers ----

    fn symbol_id() -> NodeId {
        NodeId::new("src/main.rs:main:1")
    }

    fn symbol_id_2() -> NodeId {
        NodeId::new("src/lib.rs:helper:42")
    }

    fn doc_id() -> NodeId {
        NodeId::new("doc:docs/adr/0001.md#context")
    }

    // ---- T3 RED gate tests ----

    /// A valid edge (in-range confidence, distinct source/target) must
    /// be constructable.
    #[test]
    fn graph_edge_creation_valid() {
        let edge = GraphEdge::new(
            symbol_id(),
            symbol_id_2(),
            EdgeKind::Dependency(DependencyType::Calls),
            Provenance::Extracted,
            0.95,
        )
        .expect("valid edge must construct");
        assert_eq!(edge.source, symbol_id());
        assert_eq!(edge.target, symbol_id_2());
        assert_eq!(edge.confidence, 0.95);
        assert_eq!(edge.provenance, Provenance::Extracted);
        assert!(!edge.has_metadata());

        // Boundary values 0.0 and 1.0 are accepted.
        GraphEdge::new(
            symbol_id(),
            symbol_id_2(),
            EdgeKind::Dependency(DependencyType::Imports),
            Provenance::Extracted,
            0.0,
        )
        .expect("0.0 is in-range");
        GraphEdge::new(
            symbol_id(),
            symbol_id_2(),
            EdgeKind::Dependency(DependencyType::Imports),
            Provenance::Extracted,
            1.0,
        )
        .expect("1.0 is in-range");
    }

    /// Multimodal variant on a different source/target pair.
    #[cfg(feature = "multimodal")]
    #[test]
    fn graph_edge_creation_multimodal() {
        let m_edge = GraphEdge::new(
            doc_id(),
            symbol_id(),
            EdgeKind::Cites,
            Provenance::Inferred,
            0.9,
        )
        .expect("multimodal edge must construct");
        assert_eq!(m_edge.kind, EdgeKind::Cites);
        assert_eq!(m_edge.provenance, Provenance::Inferred);
    }

    /// `confidence` outside `[0.0, 1.0]` must be rejected.
    #[test]
    fn graph_edge_confidence_out_of_range() {
        let err = GraphEdge::new(
            symbol_id(),
            symbol_id_2(),
            EdgeKind::Dependency(DependencyType::Calls),
            Provenance::Extracted,
            1.5,
        )
        .unwrap_err();
        assert_eq!(err, GraphEdgeError::ConfidenceOutOfRange(1.5));

        let err = GraphEdge::new(
            symbol_id(),
            symbol_id_2(),
            EdgeKind::Dependency(DependencyType::Calls),
            Provenance::Extracted,
            -0.1,
        )
        .unwrap_err();
        assert_eq!(err, GraphEdgeError::ConfidenceOutOfRange(-0.1));

        // NaN is reported as ConfidenceNotFinite (NOT OutOfRange).
        let err = GraphEdge::new(
            symbol_id(),
            symbol_id_2(),
            EdgeKind::Dependency(DependencyType::Calls),
            Provenance::Extracted,
            f64::NAN,
        )
        .unwrap_err();
        assert_eq!(err, GraphEdgeError::ConfidenceNotFinite);
    }

    /// `source == target` (self-loop) must be rejected.
    #[test]
    fn graph_edge_self_loop_rejected() {
        let id = symbol_id();
        let err = GraphEdge::new(
            id.clone(),
            id,
            EdgeKind::Dependency(DependencyType::Calls),
            Provenance::Extracted,
            1.0,
        )
        .unwrap_err();
        assert_eq!(
            err,
            GraphEdgeError::SelfLoop("src/main.rs:main:1".to_string())
        );
    }

    // ---- Additional T3 coverage ----

    #[test]
    fn graph_edge_with_metadata_chains() {
        let edge = GraphEdge::new(
            symbol_id(),
            symbol_id_2(),
            EdgeKind::Dependency(DependencyType::Calls),
            Provenance::Inferred,
            0.7,
        )
        .unwrap()
        .with_metadata("section", "intro")
        .with_metadata("line", "12");
        assert!(edge.has_metadata());

        // Backward-compatible access via to_map adapter
        let map = edge.metadata_to_map();
        assert_eq!(map.get("section").map(String::as_str), Some("intro"));
        assert_eq!(map.get("line").map(String::as_str), Some("12"));
    }

    #[test]
    fn graph_edge_json_roundtrip_preserves_fields() {
        let edge = GraphEdge::new(
            symbol_id(),
            symbol_id_2(),
            EdgeKind::Dependency(DependencyType::Calls),
            Provenance::Inferred,
            0.7,
        )
        .unwrap()
        .with_metadata("k", "v");
        let json = serde_json::to_string(&edge).expect("serialize");
        let parsed: GraphEdge = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, edge);
    }

    // ---- T4 RED gate tests ----

    /// The default builder produces a valid `GraphNode` with sensible
    /// defaults.
    #[test]
    fn graph_node_creation() {
        let now = Utc::now();
        let node = GraphNode::builder(symbol_id(), NodeKind::Symbol(SymbolKind::Function))
            .label("main")
            .source_path("/repo/src/main.rs")
            .property("visibility", "pub")
            .created_at(now)
            .updated_at(now)
            .build();

        assert_eq!(node.id, symbol_id());
        assert_eq!(node.kind, NodeKind::Symbol(SymbolKind::Function));
        assert_eq!(node.label, "main");
        assert_eq!(
            node.source_path.as_deref(),
            Some(std::path::Path::new("/repo/src/main.rs"))
        );
        // Backward-compatible access via to_map adapter
        let map = node.properties_to_map();
        assert_eq!(map.get("visibility").map(String::as_str), Some("pub"));
        assert_eq!(node.created_at, now);
        assert_eq!(node.updated_at, now);

        // JSON roundtrip preserves every field.
        let json = serde_json::to_string(&node).expect("serialize");
        let parsed: GraphNode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, node);
    }

    /// The builder must be a fluent, optional-each-step pattern that
    /// produces a `GraphNode` matching the inputs.
    #[cfg(feature = "multimodal")]
    #[test]
    fn graph_node_builder_pattern() {
        // 1) minimal: only required fields, defaults for the rest.
        let minimal = GraphNode::builder(symbol_id(), NodeKind::Decision).build();
        assert_eq!(minimal.id, symbol_id());
        assert_eq!(minimal.kind, NodeKind::Decision);
        assert_eq!(minimal.label, "");
        assert!(minimal.source_path.is_none());
        assert!(minimal.properties.is_empty());
        // Both timestamps set to "now" (they may differ by a few ns;
        // we only assert they are not the default epoch).
        assert!(minimal.created_at.timestamp() > 0);
        assert!(minimal.updated_at.timestamp() > 0);

        // 2) full: every optional field set.
        let fixed = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let full = GraphNode::builder(doc_id(), NodeKind::Doc)
            .label("ADR-0001: Context")
            .source_path("/repo/docs/adr/0001.md")
            .property("status", "accepted")
            .property("date", "2026-01-02")
            .properties([("section", "Context"), ("author", "team")])
            .created_at(fixed)
            .updated_at(fixed)
            .build();
        assert_eq!(full.label, "ADR-0001: Context");
        assert_eq!(
            full.source_path.as_deref(),
            Some(std::path::Path::new("/repo/docs/adr/0001.md"))
        );
        assert_eq!(full.properties.len(), 4);
        assert_eq!(
            full.properties.get("status").map(String::as_str),
            Some("accepted")
        );
        assert_eq!(
            full.properties.get("section").map(String::as_str),
            Some("Context")
        );
        assert_eq!(full.created_at, fixed);
        assert_eq!(full.updated_at, fixed);

        // 3) `with_property` post-build also updates updated_at.
        let before = Utc::now();
        let node = GraphNode::builder(symbol_id(), NodeKind::Evidence)
            .created_at(before)
            .updated_at(before)
            .build();
        let original_updated = node.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(2));
        let updated = node.with_property("sha256", "deadbeef");
        assert!(updated.updated_at >= original_updated);
    }

    // ---- Feature-gate cross check ----

    #[test]
    fn graph_node_builder_symbol_only() {
        let now = Utc::now();
        let node = GraphNode::builder(symbol_id(), NodeKind::Symbol(SymbolKind::Function))
            .label("main")
            .source_path("/repo/src/main.rs")
            .property("visibility", "pub")
            .created_at(now)
            .updated_at(now)
            .build();
        assert_eq!(node.label, "main");

        // Backward-compatible access via to_map adapter
        let map = node.properties_to_map();
        assert_eq!(map.get("visibility").map(String::as_str), Some("pub"));
        assert_eq!(node.created_at, now);
    }

    #[test]
    fn node_id_string_newtype_basics() {
        let id = NodeId::new("abc");
        assert_eq!(id.as_str(), "abc");
        assert_eq!(id.clone(), NodeId::from("abc"));
        assert_eq!(id.clone(), NodeId::from("abc".to_string()));
        assert_eq!(format!("{}", id), "abc");
    }

    // -------------------------------------------------------------------------
    // Task 1.6a RED — Typed JSONB properties round-trip
    // Scenario: `generic-graph-model::Structured properties round-trip via PG JSONB`
    // Assert: `Value::Object({…nested…})` round-trips; adapter still maps
    //         `String→String` flat keys.
    // -------------------------------------------------------------------------

    /// `GraphNode.properties` must be `serde_json::Value` defaulting to an
    /// empty JSON object `{}`.
    #[test]
    fn graph_node_properties_default_is_empty_object() {
        let node = GraphNode::builder(symbol_id(), NodeKind::Symbol(SymbolKind::Function))
            .build();
        assert_eq!(
            node.properties,
            serde_json::Value::Object(serde_json::Map::new()),
            "default properties must be empty JSON object"
        );
    }

    /// `GraphNode.properties` must accept nested JSON values (not just flat strings).
    #[test]
    fn graph_node_properties_nested_json() {
        let nested = serde_json::json!({
            "complexity": 12,
            "tags": ["auth", "security"],
            "nested": { "k": "v" }
        });
        let node = GraphNode::builder(symbol_id(), NodeKind::Symbol(SymbolKind::Function))
            .build()
            .with_property_json("meta", nested.clone());

        let retrieved = node.properties.get("meta").expect("meta key");
        assert_eq!(retrieved, &nested);
    }

    /// `properties` round-trips through JSON serialization.
    #[test]
    fn graph_node_properties_json_roundtrip() {
        let node = GraphNode::builder(symbol_id(), NodeKind::Symbol(SymbolKind::Function))
            .property("visibility", "pub")
            .property_json("complexity", serde_json::json!(5))
            .build();

        let json = serde_json::to_string(&node).expect("serialize");
        let parsed: GraphNode = serde_json::from_str(&json).expect("deserialize");

        let map = parsed.properties_to_map();
        assert_eq!(
            map.get("visibility").map(String::as_str),
            Some("pub"),
            "flat string property preserved"
        );
        assert_eq!(
            parsed.properties.get("complexity"),
            Some(&serde_json::json!(5)),
            "nested numeric property preserved"
        );
    }

    /// `to_map()` adapter must convert JSON properties back to `HashMap<String, String>`
    /// for backward compatibility with code that expects flat string properties.
    #[test]
    fn graph_node_properties_to_map_adapter() {
        let node = GraphNode::builder(symbol_id(), NodeKind::Symbol(SymbolKind::Function))
            .property("visibility", "pub")
            .property("line", "42")
            .build();

        let map = node.properties_to_map();
        assert_eq!(map.get("visibility").map(String::as_str), Some("pub"));
        assert_eq!(map.get("line").map(String::as_str), Some("42"));
    }

    /// `from_map()` adapter must convert `HashMap<String, String>` to JSON properties.
    #[test]
    fn graph_node_properties_from_map_adapter() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert("visibility".to_string(), "pub".to_string());
        map.insert("line".to_string(), "42".to_string());

        let node = GraphNode::builder(symbol_id(), NodeKind::Symbol(SymbolKind::Function))
            .properties_from_map(&map)
            .build();

        // Verify round-trip: properties_from_map creates Value which to_map decodes back
        let decoded = node.properties_to_map();
        assert_eq!(decoded.get("visibility").map(String::as_str), Some("pub"));
        assert_eq!(decoded.get("line").map(String::as_str), Some("42"));
    }

    // -------------------------------------------------------------------------
    // Task 1.6b GREEN — Change `properties: HashMap<String, String>` to JSON Value
    // Tests above verify the contract; implementation below makes them pass.
    // -------------------------------------------------------------------------
}
