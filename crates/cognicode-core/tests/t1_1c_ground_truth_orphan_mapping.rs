//! T1-1c — Ground-truth orphan → CanonicalSymbolKind::Unknown contract.
//!
//! Estado pre-T1-1c:
//!   - T1-1c parcial (commit `21fe4e5d`) añadió
//!     `impl From<SymbolKind> for CanonicalSymbolKind` en
//!     `sandbox_core/ground_truth.rs:58`.
//!   - El From mapea 11 variantes directo (Function, Struct, ...)
//!     y 4 orphans (`Impl`, `Static`, `Macro`, `Other(_)`) a
//!     `CanonicalSymbolKind::Unknown` — la decisión "consolidación
//!     con pérdida acotada" del goal doc.
//!   - PERO el contrato NO estaba pineado por test. Un cambio
//!     futuro que accidentalmente mapeara `Impl` a
//!     `CanonicalSymbolKind::Function` (por ejemplo) cambiaría
//!     los scores de scoring sin que ningún test lo detectara.
//!
//! Lo que cierra este test:
//!   - drift en el mapping de los 4 orphans (que sigan yendo a
//!     `Unknown`, no a una variante específica del canónico)
//!   - drift en el mapping de los 11 directos (que sigan
//!     yendo a su variante correspondiente, no a `Unknown`)
//!
//! Política T1-1c: la pérdida de información es ACOTADA — los 4
//! orphans se vuelven `Unknown` en el canónico, pero el string
//! original se preserva en `ground_truth`'s propio
//! `SymbolKind::Other(s)`. Esto permite que el ground truth
//! YAML siga conteniendo la información que el scoring podría
//! usar, mientras el canónico recibe solo "esto no encaja en
//! ninguna variante conocida".

use cognicode_core::domain::value_objects::symbol_kind::SymbolKind as CanonicalSymbolKind;
use cognicode_core::sandbox_core::ground_truth::SymbolKind as GroundTruthSymbolKind;

#[test]
fn four_orphans_map_to_unknown() {
    // The "consolidación con pérdida acotada" decision: Impl,
    // Static, Macro, and Other(_) collapse to Unknown in the
    // canonical space. If this RED, the consolidation policy
    // has drifted; the scoring downstream depends on this
    // collapse to make all four weigh the same.
    let orphans = [
        GroundTruthSymbolKind::Impl,
        GroundTruthSymbolKind::Static,
        GroundTruthSymbolKind::Macro,
        GroundTruthSymbolKind::Other("anything".to_string()),
    ];
    for orphan in &orphans {
        let canonical: CanonicalSymbolKind = orphan.clone().into();
        assert_eq!(
            canonical,
            CanonicalSymbolKind::Unknown,
            "ground-truth orphan {orphan:?} must collapse to CanonicalSymbolKind::Unknown, \
             got {canonical:?}. The 'consolidación con pérdida acotada' decision from \
             odd/tasks/v1015-consolidation-and-promote.md §T1-1c requires all four \
             orphans to map to the same sink so scoring weights stay comparable."
        );
    }
}

#[test]
fn eleven_direct_variants_map_to_their_canonical_counterpart() {
    // The 11 direct mappings are the ones the domain says ARE
    // representable in both spaces. If a mapping changes (e.g.
    // a future refactor decides Method → Function in canonical),
    // this test RED and forces a deliberate decision.
    let pairs: &[(GroundTruthSymbolKind, CanonicalSymbolKind)] = &[
        (GroundTruthSymbolKind::Function, CanonicalSymbolKind::Function),
        (GroundTruthSymbolKind::Struct, CanonicalSymbolKind::Struct),
        (GroundTruthSymbolKind::Module, CanonicalSymbolKind::Module),
        (GroundTruthSymbolKind::Enum, CanonicalSymbolKind::Enum),
        (GroundTruthSymbolKind::Trait, CanonicalSymbolKind::Trait),
        (GroundTruthSymbolKind::Method, CanonicalSymbolKind::Method),
        (GroundTruthSymbolKind::Field, CanonicalSymbolKind::Field),
        (GroundTruthSymbolKind::Const, CanonicalSymbolKind::Constant),
        (GroundTruthSymbolKind::TypeAlias, CanonicalSymbolKind::TypeAlias),
        (GroundTruthSymbolKind::Class, CanonicalSymbolKind::Class),
        (GroundTruthSymbolKind::Interface, CanonicalSymbolKind::Interface),
    ];
    assert_eq!(
        pairs.len(),
        11,
        "the direct-mapping table must stay 11; if a variant is \
         added or removed, this test should be updated deliberately, \
         not by accident"
    );
    for (gt, expected) in pairs {
        let canonical: CanonicalSymbolKind = gt.clone().into();
        assert_eq!(
            canonical, *expected,
            "ground-truth {gt:?} must map to canonical {expected:?}, got {canonical:?}"
        );
    }
}

#[test]
fn three_extra_variants_map_to_their_specific_canonical() {
    // These three are not orphans (they have a canonical
    // counterpart) but they also aren't in the "11 direct"
    // list above. They go to specific canonical variants:
    //
    //   Variant  → EnumVariant   (ground truth "variant" can be
    //                            either an enum variant or a
    //                            union variant; the canonical
    //                            model picks EnumVariant)
    //   Property → Property      (1:1, no collapse)
    //   Parameter → Parameter    (1:1, no collapse)
    //   Variable  → Variable     (1:1, no collapse)
    //
    // If any of these change, the scoring weights for these
    // categories will change silently. Pin the mapping.
    let pairs: &[(GroundTruthSymbolKind, CanonicalSymbolKind)] = &[
        (GroundTruthSymbolKind::Variant, CanonicalSymbolKind::EnumVariant),
        (GroundTruthSymbolKind::Property, CanonicalSymbolKind::Property),
        (GroundTruthSymbolKind::Parameter, CanonicalSymbolKind::Parameter),
        (GroundTruthSymbolKind::Variable, CanonicalSymbolKind::Variable),
    ];
    for (gt, expected) in pairs {
        let canonical: CanonicalSymbolKind = gt.clone().into();
        assert_eq!(
            canonical, *expected,
            "ground-truth {gt:?} must map to canonical {expected:?}, got {canonical:?}"
        );
    }
}

#[test]
fn other_string_preserves_content_in_ground_truth_space() {
    // The "pérdida acotada" half of the decision: the canonical
    // collapses to Unknown, but the ground-truth SymbolKind
    // itself keeps the original string in Other(s). This
    // allows YAML round-trip without information loss in the
    // ground-truth space, even though the canonical can't
    // carry the detail.
    let original = "very_specific_dynamic_kind";
    let gt = GroundTruthSymbolKind::from_str(original);
    match &gt {
        GroundTruthSymbolKind::Other(s) => assert_eq!(s, original),
        _ => panic!("from_str should produce Other(\"{original}\") for unknown kinds"),
    }
    // And as_str round-trips it back:
    assert_eq!(gt.as_str(), original);
}
