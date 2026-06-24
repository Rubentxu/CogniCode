//! Anti-corruption layer for the ViewDescriptor/ViewSpec boundary.
//!
//! This module provides `From` implementations that translate between the
//! core crate's MCP schema types and the explorer crate's DTO types.
//!
//! ## Architecture
//!
//! - **Core** (`cognicode_core::interface::mcp::schemas`) owns `ViewDescriptor` and `ViewSpec`
//!   as the canonical wire/MCP types.
//! - **Explorer** (`cognicode_explorer::dto`) owns `ViewDescriptorDto` and `ViewSpec`
//!   as the internal DTOs with richer enum types.
//! - **Boundary** (`boundary`) sits between them, providing lossless, infallible conversions.
//!
//! ## Why this module exists
//!
//! The core crate cannot depend on the explorer crate (that would create a circular
//! dependency). Therefore, the explorer crate owns these `From` implementations —
//! the orphan rule is satisfied because explorer depends on core.
//!
//! See ADR-046 for the full boundary contract and design rationale.

// Note: serde is used indirectly through the serialize/deserialize trait bounds
// on the types being converted, but the actual Serialize/Deserialize imports
// are not explicitly needed here since we delegate to the types' own
// serialization methods.

use crate::dto::{ViewDescriptorDto, ViewSpec};
use cognicode_core::interface::mcp::schemas as core_schema;

impl From<core_schema::ViewDescriptor> for ViewDescriptorDto {
    fn from(s: core_schema::ViewDescriptor) -> Self {
        Self {
            id: s.id,
            title: s.title,
            is_builtin: s.is_builtin,
            source: s.source,
        }
    }
}

impl From<ViewDescriptorDto> for core_schema::ViewDescriptor {
    fn from(d: ViewDescriptorDto) -> Self {
        Self {
            id: d.id,
            title: d.title,
            is_builtin: d.is_builtin,
            source: d.source,
        }
    }
}

// ---------------------------------------------------------------------------
// ViewSpec conversions
// ---------------------------------------------------------------------------
//
// The explorer ViewSpec uses strong enums (ViewKind, RendererKind, DataSource,
// InspectableObjectType) while the core ViewSpec uses String fields for
// compatibility with JSON wire protocol. We use serde for lossless conversion.

impl From<ViewSpec> for core_schema::ViewSpec {
    fn from(spec: ViewSpec) -> Self {
        // Serialize the explorer ViewSpec to JSON, then deserialize as core ViewSpec.
        // This is lossless for all enum variants.
        let json = serde_json::to_value(&spec).expect("ViewSpec serialization must succeed");
        serde_json::from_value(json).expect("core ViewSpec deserialization must succeed")
    }
}

impl From<core_schema::ViewSpec> for ViewSpec {
    fn from(spec: core_schema::ViewSpec) -> Self {
        // Serialize the core ViewSpec to JSON, then deserialize as explorer ViewSpec.
        // For ViewKind and RendererKind, unknown tags map to Custom variants (fallback).
        // For InspectableObjectType, the value must be a known variant — unknown values
        // will cause deserialization to fail, which is the correct behavior (fail-fast
        // rather than silently accept malformed data from the wire).
        let json = serde_json::to_value(&spec).expect("core ViewSpec serialization must succeed");
        serde_json::from_value(json).expect("ViewSpec deserialization must succeed")
    }
}

// ---------------------------------------------------------------------------
// Round-trip tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn view_descriptor_dto_roundtrip() {
        let original = ViewDescriptorDto {
            id: "test-id".to_string(),
            title: "Test View".to_string(),
            is_builtin: true,
            source: None,
        };
        let core: core_schema::ViewDescriptor = original.clone().into();
        let roundtrip: ViewDescriptorDto = core.into();
        assert_eq!(original.id, roundtrip.id);
        assert_eq!(original.title, roundtrip.title);
        assert_eq!(original.is_builtin, roundtrip.is_builtin);
        assert_eq!(original.source, roundtrip.source);
    }

    #[test]
    fn view_descriptor_dto_with_source_roundtrip() {
        let original = ViewDescriptorDto {
            id: "runtime-id".to_string(),
            title: "My Custom View".to_string(),
            is_builtin: false,
            source: Some("runtime".to_string()),
        };
        let core: core_schema::ViewDescriptor = original.clone().into();
        let roundtrip: ViewDescriptorDto = core.into();
        assert_eq!(original.id, roundtrip.id);
        assert_eq!(original.title, roundtrip.title);
        assert_eq!(original.is_builtin, roundtrip.is_builtin);
        assert_eq!(original.source, roundtrip.source);
    }

    #[test]
    fn view_spec_roundtrip() {
        use crate::dto::{DataSource, InspectableObjectType, RendererKind, ViewKind};

        let original = ViewSpec {
            id: "12345678-1234-1234-1234-123456789012".to_string(),
            title: "Test Spec".to_string(),
            applies_to: InspectableObjectType::Symbol,
            view_kind: ViewKind::CallGraph,
            data_source: DataSource::Moldql {
                query: "symbols where fan_out > 5".to_string(),
            },
            transform: None,
            renderer_kind: RendererKind::Graph,
            props: serde_json::json!({"max_depth": 3}),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
            owner: "alice".to_string(),
        };

        let core: core_schema::ViewSpec = original.clone().into();
        let roundtrip: ViewSpec = core.into();

        assert_eq!(original.id, roundtrip.id);
        assert_eq!(original.title, roundtrip.title);
        assert_eq!(original.applies_to, roundtrip.applies_to);
        assert_eq!(original.view_kind, roundtrip.view_kind);
        assert_eq!(original.renderer_kind, roundtrip.renderer_kind);
        assert_eq!(original.owner, roundtrip.owner);
    }

    #[test]
    fn view_spec_infallibility_smoke() {
        // Every ViewKind variant should round-trip through both From impls.
        use crate::dto::{DataSource, InspectableObjectType, RendererKind, ViewKind};

        let vks = [
            ViewKind::VerticalSlice,
            ViewKind::CallGraph,
            ViewKind::SourceView,
            ViewKind::QualityHotspots,
        ];
        let rks = [
            RendererKind::Graph,
            RendererKind::Table,
            RendererKind::Tree,
            RendererKind::Code,
        ];

        for vk in vks.iter() {
            for rk in rks.iter() {
                let spec = ViewSpec {
                    id: "00000000-0000-0000-0000-000000000000".to_string(),
                    title: format!("vk={:?} rk={:?}", vk, rk),
                    applies_to: InspectableObjectType::Symbol,
                    view_kind: vk.clone(),
                    data_source: DataSource::Moldql {
                        query: "x".to_string(),
                    },
                    transform: None,
                    renderer_kind: rk.clone(),
                    props: serde_json::json!({}),
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                    owner: "tester".to_string(),
                };

                let core: core_schema::ViewSpec = spec.clone().into();
                let back: ViewSpec = core.into();
                assert_eq!(spec.view_kind, back.view_kind, "view_kind round-trip");
                assert_eq!(
                    spec.renderer_kind, back.renderer_kind,
                    "renderer_kind round-trip"
                );
            }
        }
    }
}
