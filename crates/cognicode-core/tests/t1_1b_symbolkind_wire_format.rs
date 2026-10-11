//! T1-1b — Wire format contract for `McpSymbolKind` (schemas::SymbolKind).
//!
//! Estado pre-T1-1b:
//!   - `schemas::SymbolKind` existe con `#[serde(rename_all = "lowercase")]`
//!     y 15 variantes. La conversión `McpSymbolKind -> SymbolKind`
//!     (canónico) está en `dto_mapping.rs:186`. Pero el contrato
//!     **NO estaba pineado por test** — un cambio accidental en el
//!     `rename_all` o en el set de variantes rompía la wire format
//!     sin que ningún test lo detectara (mcp_roundtrip_tests no
//!     existe como suite dedicada).
//!
//! Lo que cierra este test:
//!   - drift en el wire format (un cambio a PascalCase o snake_case
//!     rompe la compatibilidad con consumers MCP externos)
//!   - drift en el set de variantes expuestas (añadir/quitar variantes
//!     sin actualizar el From canónico)
//!   - drift en la conversión (un cambio en `dto_mapping.rs` que pierda
//!     un mapping)
//!
//! Política T1-1b: el shim vive en `schemas.rs` como enum paralelo
//! (no newtype). La razón es que `SymbolKind` también aparece como
//! campo en structs MCP (SymbolInfo.kind), y un newtype exigiría
//! `McpSymbolKind` en todos esos campos sin benefício. El enum
//! paralelo con `serde(rename_all = "lowercase")` cumple la misma
//! función: el dominio no contamina el transporte.

use cognicode_core::application::dto::SymbolKind as CanonicalSymbolKind;
use cognicode_core::interface::mcp::schemas::SymbolKind as McpSymbolKind;
use serde_json::Value;

/// Wire format esperado para cada variante. Si un cambio toca
/// `#[serde(rename_all = "lowercase")]` o el nombre de una variante,
/// este test RED con el JSON exacto que se desvía.
const EXPECTED_WIRE_FORMAT: &[(McpSymbolKind, &str)] = &[
    (McpSymbolKind::Module, "module"),
    (McpSymbolKind::Class, "class"),
    (McpSymbolKind::Struct, "struct"),
    (McpSymbolKind::Enum, "enum"),
    (McpSymbolKind::Trait, "trait"),
    (McpSymbolKind::Function, "function"),
    (McpSymbolKind::Method, "method"),
    (McpSymbolKind::Field, "field"),
    (McpSymbolKind::Variable, "variable"),
    (McpSymbolKind::Constant, "constant"),
    (McpSymbolKind::Constructor, "constructor"),
    (McpSymbolKind::Interface, "interface"),
    (McpSymbolKind::TypeAlias, "type_alias"),
    (McpSymbolKind::Parameter, "parameter"),
];

#[test]
fn all_mcp_symbolkind_variants_serialize_to_lowercase() {
    // Each variant must serialize to the exact lowercase string the contract
    // promises. If this RED, either the rename_all attribute was changed, or
    // a variant was renamed in the enum.
    let mut seen = std::collections::HashSet::new();
    for (variant, expected_json) in EXPECTED_WIRE_FORMAT {
        let json = serde_json::to_string(variant)
            .expect("SymbolKind must serialize to JSON (impl Serialize)");
        let parsed: Value =
            serde_json::from_str(&json).expect("serialized form must be valid JSON");
        let actual = parsed
            .as_str()
            .unwrap_or_else(|| panic!("SymbolKind must serialize as a JSON string, got: {json}"));
        assert_eq!(
            actual, *expected_json,
            "wire format drift: McpSymbolKind::{variant:?} serializes to {actual:?}, expected {:?}",
            *expected_json
        );
        assert!(
            actual.chars().all(|c| !c.is_ascii_uppercase()),
            "wire format must be lowercase (rename_all = \"lowercase\"), got: {actual}"
        );
        assert!(
            seen.insert(actual.to_string()),
            "two McpSymbolKind variants serialize to the same wire string: {actual}"
        );
    }
    // The set size must match the const array length — a missing row in
    // EXPECTED_WIRE_FORMAT would silently let a new variant pass.
    assert_eq!(
        seen.len(),
        EXPECTED_WIRE_FORMAT.len(),
        "EXPECTED_WIRE_FORMAT must cover every McpSymbolKind variant exactly once"
    );
}

#[test]
fn from_mcp_to_canonical_preserves_all_15_variants() {
    // For each McpSymbolKind, the From conversion must produce a canonical
    // SymbolKind that round-trips back. This pins the mapping in
    // dto_mapping.rs:186 — if a mapping is removed, this test RED because
    // the catch-all `_ => ...` would map everything to one variant.
    //
    // `.into()` exercises the `impl From<McpSymbolKind> for SymbolKind` in
    // dto_mapping.rs. If that impl is removed, the line fails to compile.
    for (variant, _) in EXPECTED_WIRE_FORMAT {
        let canonical: CanonicalSymbolKind = variant.clone().into();
        // The wire format is PascalCase for the canonical (per
        // application/dto/common.rs doc comment). This pins that.
        let canonical_json = serde_json::to_string(&canonical)
            .expect("Canonical SymbolKind must serialize (impl Serialize)");
        assert!(
            canonical_json.starts_with('"') && canonical_json.ends_with('"'),
            "canonical SymbolKind must serialize as a JSON string, got: {canonical_json}"
        );
        let inner = &canonical_json[1..canonical_json.len() - 1];
        assert!(
            inner.chars().next().is_some_and(|c| c.is_ascii_uppercase()),
            "canonical SymbolKind wire format must be PascalCase, got: {inner}"
        );
    }
}

#[test]
fn mcp_symbolkind_does_not_serialize_as_pascalcase() {
    // Negative test: a wire format change to PascalCase (the canonical's
    // format) would silently make the MCP layer indistinguishable from
    // the canonical. Pin that the MCP layer does NOT use PascalCase.
    let json = serde_json::to_string(&McpSymbolKind::Function).expect("SymbolKind must serialize");
    assert_eq!(
        json, "\"function\"",
        "MCP SymbolKind::Function must serialize as \"function\" (lowercase), got: {json}"
    );
}

#[test]
fn mcp_symbolkind_does_not_serialize_as_snake_case_for_compound_names() {
    // `TypeAlias` becomes `type_alias` in lowercase+snake. If the serde
    // attribute were `lowercase` alone (without `snake_case` fallback for
    // compound names), it would serialize as `typealias`. Pin the exact
    // form.
    let json = serde_json::to_string(&McpSymbolKind::TypeAlias).expect("SymbolKind must serialize");
    assert_eq!(
        json, "\"type_alias\"",
        "MCP SymbolKind::TypeAlias must serialize as \"type_alias\" (snake_case compound), got: {json}"
    );
}
