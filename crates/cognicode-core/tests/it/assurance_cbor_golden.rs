//! Fixtures contractuales versionados del digest CBOR (CGH-011 AC#2).
//!
//! El formato de Assurance depende del ORDEN de campos entre Rust y
//! Kotlin para reproducir el digest: una reordenación inocua en Rust
//! rompe el digest del consumer sin tocar ningún assert de valores.
//! Este ancla es la primera defensa: el bytes exactos del golden export
//! versionados en `tests/data/assurance_golden_v1.cbor`.
//!
//! Reglas:
//! - `cargo test` compara contra el golden versionado. Cualquier
//!   diferencia es un cambio de serialización y exige decisión explícita.
//! - `ASSURANCE_GOLDEN_UPDATE=1 cargo test` regenera el golden (workflow
//!   del desarrollador, nunca dentro de la puerta).
//! - El golden también fija el contrato especializado: `architecture`
//!   viaja como `Unsupported` con motivo y aparece en `gaps[]`; si
//!   alguien lo promueve a interfaz universal de Guard, este test lo ve
//!   (CGH-011 AC#1).

use std::path::PathBuf;

use cognicode_core::assurance_export::{
    API_VERSION, CapabilityCompleteness, decode_from_cbor, digest_of, encode_to_cbor_with_digest,
};

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/assurance_golden_v1.cbor")
}

fn golden_regeneration_requested() -> bool {
    std::env::var("ASSURANCE_GOLDEN_UPDATE").is_ok_and(|v| v == "1")
}

#[test]
fn assurance_cbor_golden_bytes_are_versioned_and_exact() {
    let envelope = crate::assurance_export_roundtrip::sample_envelope();
    let bytes = encode_to_cbor_with_digest(&envelope).expect("encode CBOR");

    if golden_regeneration_requested() {
        std::fs::write(golden_path(), &bytes).expect("escribir golden");
        return;
    }

    let golden = std::fs::read(golden_path()).unwrap_or_else(|e| {
        panic!(
            "falta el fixture contractual {}: {e}",
            golden_path().display()
        )
    });
    assert_eq!(
        golden, bytes,
        "el CBOR exportado ha cambiado de bytes: el digest que Kotlin \
         reproduce con el ORDEN de campos ya no coincide. Si el cambio \
         es intencional y coordinado con el consumer, regenerar con \
         ASSURANCE_GOLDEN_UPDATE=1 y declararlo en el PR."
    );
}

#[test]
fn assurance_golden_decodes_to_the_pinned_digest() {
    if golden_regeneration_requested() {
        return;
    }
    let golden = std::fs::read(golden_path()).expect("el golden debe estar versionado");
    let decoded = decode_from_cbor(&golden).expect("el golden debe decodificar");
    let recomputed = digest_of(&decoded);
    assert_eq!(recomputed.len(), 64, "SHA-256 hex");
    // El digest viaja en el propio envelope: el ancla lo verifica igual.
    assert_eq!(
        decoded.digest, recomputed,
        "el digest embebido no coincide con el recomputado sobre las \
         versiones decodificadas del golden"
    );
}

#[test]
fn assurance_golden_keeps_the_specialized_contract() {
    if golden_regeneration_requested() {
        return;
    }
    let golden = std::fs::read(golden_path()).expect("el golden debe estar versionado");
    let decoded = decode_from_cbor(&golden).expect("el golden debe decodificar");
    assert_eq!(decoded.api_version, API_VERSION);
    assert_eq!(
        decoded.capability_completeness.get("architecture"),
        Some(&CapabilityCompleteness::Unsupported {
            reason: "no se calcula en v1".into()
        }),
        "Assurance sigue declarando architecture Unsupported: no es la \
         interfaz universal de Guard (CGH-011 AC#1)"
    );
    assert!(
        decoded.gaps.iter().any(|g| g.capability == "architecture"),
        "el hueco de architecture debe seguir viajando en gaps[]"
    );
}
