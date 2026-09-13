//! SymbolKindDetail — the single `kind=<SerdeName>` codec (E38.1 CP-2/OE-9).
//!
//! The symbol kind of a `core:defines` fact rides the fact's
//! `provenance.detail` as `kind=<SerdeName>` (e37 design D3), where
//! `<SerdeName>` is the serde name of a [`SymbolKind`] variant (the variant
//! identifier). This module is the ONE codec for that convention:
//!
//! - [`encode`](SymbolKindDetail::encode) — forward (producer side); the
//!   serde-name table has no wildcard arm, so a new `SymbolKind` variant
//!   fails compilation here instead of silently degrading.
//! - [`decode`](SymbolKindDetail::decode) — inverse (consumer side);
//!   returns `None` for an absent prefix or an unknown serde name. There is
//!   NO silent fallback to `SymbolKind::Unknown`: consumers handle the
//!   `Option` explicitly at their call sites (E38.1 loud-fallback rule).

use crate::domain::value_objects::SymbolKind;

/// The `kind=<SerdeName>` provenance-detail convention over [`SymbolKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolKindDetail;

impl SymbolKindDetail {
    /// The detail prefix asserted by every encoded kind detail.
    pub const PREFIX: &str = "kind=";

    /// Encodes `kind` as the `kind=<SerdeName>` provenance detail.
    pub fn encode(kind: SymbolKind) -> String {
        format!("{}{}", Self::PREFIX, Self::serde_name(kind))
    }

    /// Decodes a provenance detail onto a [`SymbolKind`].
    ///
    /// Returns `None` when the detail does not start with [`PREFIX`] or the
    /// remaining name is not a known serde name — callers decide loudly how
    /// to treat an undecodable detail (no silent `Unknown` here).
    pub fn decode(detail: &str) -> Option<SymbolKind> {
        detail
            .strip_prefix(Self::PREFIX)
            .and_then(Self::from_serde_name)
    }

    /// The serde name of a [`SymbolKind`] variant. Exhaustive with no
    /// wildcard arm so a new variant fails compilation here.
    pub fn serde_name(kind: SymbolKind) -> &'static str {
        match kind {
            SymbolKind::Function => "Function",
            SymbolKind::Class => "Class",
            SymbolKind::Module => "Module",
            SymbolKind::Variable => "Variable",
            SymbolKind::Parameter => "Parameter",
            SymbolKind::Type => "Type",
            SymbolKind::Method => "Method",
            SymbolKind::Property => "Property",
            SymbolKind::Field => "Field",
            SymbolKind::Import => "Import",
            SymbolKind::EnumVariant => "EnumVariant",
            SymbolKind::Trait => "Trait",
            SymbolKind::Generic => "Generic",
            SymbolKind::Constant => "Constant",
            SymbolKind::Constructor => "Constructor",
            SymbolKind::Struct => "Struct",
            SymbolKind::Enum => "Enum",
            SymbolKind::Interface => "Interface",
            SymbolKind::File => "File",
            SymbolKind::Namespace => "Namespace",
            SymbolKind::Package => "Package",
            SymbolKind::Unknown => "Unknown",
        }
    }

    /// Inverse of [`serde_name`](Self::serde_name): every serde variant name
    /// maps back onto its [`SymbolKind`]; anything else is `None`.
    pub fn from_serde_name(name: &str) -> Option<SymbolKind> {
        match name {
            "Function" => Some(SymbolKind::Function),
            "Class" => Some(SymbolKind::Class),
            "Module" => Some(SymbolKind::Module),
            "Variable" => Some(SymbolKind::Variable),
            "Parameter" => Some(SymbolKind::Parameter),
            "Type" => Some(SymbolKind::Type),
            "Method" => Some(SymbolKind::Method),
            "Property" => Some(SymbolKind::Property),
            "Field" => Some(SymbolKind::Field),
            "Import" => Some(SymbolKind::Import),
            "EnumVariant" => Some(SymbolKind::EnumVariant),
            "Trait" => Some(SymbolKind::Trait),
            "Generic" => Some(SymbolKind::Generic),
            "Constant" => Some(SymbolKind::Constant),
            "Constructor" => Some(SymbolKind::Constructor),
            "Struct" => Some(SymbolKind::Struct),
            "Enum" => Some(SymbolKind::Enum),
            "Interface" => Some(SymbolKind::Interface),
            "File" => Some(SymbolKind::File),
            "Namespace" => Some(SymbolKind::Namespace),
            "Package" => Some(SymbolKind::Package),
            "Unknown" => Some(SymbolKind::Unknown),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Task 1.2 — exhaustive round-trip over ALL `SymbolKind` variants:
    /// encode → decode recovers the original kind, and the serde names match
    /// the variant identifiers the serde derive emits.
    #[test]
    fn encode_decode_round_trips_every_variant() {
        let all = [
            SymbolKind::Function,
            SymbolKind::Class,
            SymbolKind::Module,
            SymbolKind::Variable,
            SymbolKind::Parameter,
            SymbolKind::Type,
            SymbolKind::Method,
            SymbolKind::Property,
            SymbolKind::Field,
            SymbolKind::Import,
            SymbolKind::EnumVariant,
            SymbolKind::Trait,
            SymbolKind::Generic,
            SymbolKind::Constant,
            SymbolKind::Constructor,
            SymbolKind::Struct,
            SymbolKind::Enum,
            SymbolKind::Interface,
            SymbolKind::File,
            SymbolKind::Namespace,
            SymbolKind::Package,
            SymbolKind::Unknown,
        ];
        for kind in all {
            let detail = SymbolKindDetail::encode(kind);
            assert!(
                detail.starts_with(SymbolKindDetail::PREFIX),
                "{detail} must carry the prefix"
            );
            let decoded = SymbolKindDetail::decode(&detail).expect("encoded kinds decode");
            assert_eq!(decoded, kind, "round trip failed for {detail}");
            assert_eq!(
                SymbolKindDetail::serde_name(kind),
                detail
                    .strip_prefix(SymbolKindDetail::PREFIX)
                    .expect("prefix"),
                "serde name must be the detail body for {kind:?}"
            );
        }
    }

    /// Undecodable details are `None` — the codec never degrades silently.
    #[test]
    fn decode_rejects_malformed_details() {
        for case in ["", "Function", "kind=", "kind=Bogus", "kind=Function "] {
            assert!(
                SymbolKindDetail::decode(case).is_none(),
                "{case:?} must not decode"
            );
        }
    }
}
