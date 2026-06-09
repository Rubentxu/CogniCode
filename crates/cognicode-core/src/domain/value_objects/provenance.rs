//! `Provenance` — value object classifying how a `CallGraph` edge was obtained.
//!
//! Every edge in the call graph now carries a `Provenance` and a
//! `confidence: f64` (validated to the closed interval `[0.0, 1.0]`).
//! This enum is the closed set of supported values:
//!
//! * [`Provenance::Extracted`] — the edge was observed directly in the
//!   source by an AST extractor (e.g. tree-sitter call/invoke nodes).
//!   These edges get `confidence = 1.0`.
//! * [`Provenance::Inferred`] — the edge was produced by a heuristic
//!   resolver (e.g. fuzzy import resolution). `confidence` lives in
//!   `[0.5, 0.9]`.
//! * [`Provenance::Ambiguous`] — the edge could not be resolved to a
//!   single target; multiple candidates exist. `confidence <= 0.5`.
//!
//! Adding a new variant is a **breaking change** for the bincode blob
//! format and must be paired with bumping the blob version in
//! `cognicode_db::graph::VersionedBlob`.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Closed enumeration describing how a `CallGraph` edge was obtained.
///
/// The default variant is [`Provenance::Extracted`] (the safe, common case
/// for AST-derived edges). Persistence code that loads legacy blobs must
/// also default to `Extracted` (see `CallGraphV1::into_v2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Provenance {
    /// Edge observed directly in the source (AST extractor).
    #[default]
    Extracted,
    /// Edge produced by a heuristic resolver with a confidence score in
    /// `[0.5, 0.9]`.
    Inferred,
    /// Edge could not be resolved to a single target; `confidence <= 0.5`.
    Ambiguous,
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Provenance::Extracted => "Extracted",
            Provenance::Inferred => "Inferred",
            Provenance::Ambiguous => "Ambiguous",
        };
        f.write_str(s)
    }
}

/// Parses the canonical `Display` strings (`"Extracted"`, `"Inferred"`,
/// `"Ambiguous"`). Returns `Err(())` on any other input — callers
/// that want a default fallback (e.g. the PostgreSQL row mapper)
/// compose `from_str(...).unwrap_or(Provenance::Extracted)`.
///
/// This is the inverse of the `Display` impl above. Persistence
/// layers that write `Provenance` via `Display` can therefore
/// round-trip through this `FromStr` without loss.
impl FromStr for Provenance {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Extracted" => Ok(Provenance::Extracted),
            "Inferred" => Ok(Provenance::Inferred),
            "Ambiguous" => Ok(Provenance::Ambiguous),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_extracted() {
        assert_eq!(Provenance::default(), Provenance::Extracted);
    }

    #[test]
    fn display_matches_variant_name() {
        assert_eq!(format!("{}", Provenance::Extracted), "Extracted");
        assert_eq!(format!("{}", Provenance::Inferred), "Inferred");
        assert_eq!(format!("{}", Provenance::Ambiguous), "Ambiguous");
    }

    #[test]
    fn json_roundtrip_preserves_variant() {
        for variant in [
            Provenance::Extracted,
            Provenance::Inferred,
            Provenance::Ambiguous,
        ] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let parsed: Provenance = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn bincode_roundtrip_preserves_variant() {
        for variant in [
            Provenance::Extracted,
            Provenance::Inferred,
            Provenance::Ambiguous,
        ] {
            let bytes = bincode::serde::encode_to_vec(&variant, bincode::config::standard())
                .expect("bincode encode");
            let (decoded, _): (Provenance, usize) =
                bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                    .expect("bincode decode");
            assert_eq!(decoded, variant);
        }
    }

    #[test]
    fn is_hashable() {
        // Compile-time proof that Provenance can live in HashSet/HashMap keys.
        use std::collections::HashSet;
        let mut set: HashSet<Provenance> = HashSet::new();
        set.insert(Provenance::Extracted);
        set.insert(Provenance::Inferred);
        set.insert(Provenance::Ambiguous);
        assert_eq!(set.len(), 3);
        assert!(set.contains(&Provenance::Extracted));
    }

    #[test]
    fn from_str_round_trips_through_display() {
        for variant in [
            Provenance::Extracted,
            Provenance::Inferred,
            Provenance::Ambiguous,
        ] {
            let s = variant.to_string();
            let parsed: Provenance = s.parse().expect("from_str must accept Display form");
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn from_str_rejects_unknown_input() {
        let result: Result<Provenance, ()> = "garbage".parse();
        assert!(result.is_err(), "unknown input must error");

        // Composition pattern: caller picks the fallback.
        let fallback: Provenance = "garbage".parse().unwrap_or(Provenance::Extracted);
        assert_eq!(fallback, Provenance::Extracted);
    }
}
