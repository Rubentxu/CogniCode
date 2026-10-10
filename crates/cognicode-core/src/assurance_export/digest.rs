//! Cálculo de digest M-D01-style, fail-closed.
//!
//! Convención (idéntica al codec del consumer en
//! `pipelinek-assurance/.../CogniCodeEvidenceExportCodec.kt::digestOf`):
//!
//! 1. `placeholder = dto.copy(digest = "")`
//! 2. `canonical = cbor.encodeToByteArray(serializer, placeholder)`
//! 3. `digest = hex(SHA-256(canonical))`
//!
//! El consumer verifica con la misma rutina sobre su re-codificación.
//! Para que las dos rutinas coincidan byte-a-byte, el encoding del
//! producer (Rust/ciborium) y del consumer (kotlinx-serialization-cbor)
//! deben producir los mismos bytes sobre el mismo DTO. Esa paridad se
//! verifica empíricamente con `tests/assurance_export_roundtrip.rs`:
//! ver el SHA-256 pinned en `tests/fixtures/assurance_export_golden.sha256`.
//!
//! El campo `digest` aparece en el envelope codificado en la posición que
//! le corresponda por orden de declaración (último campo del root). Cuando
//! el producer serializa el placeholder con `digest = ""`, el resto del
//! envelope es bit-identical al envelope final; el consumer re-codifica
//! con su propia libreria y compara contra el placeholder.
//!
//! Falla cerrado si difiere: un envelope con digest alterado en tránsito
//! se rechaza, no se admite con warning.

use sha2::{Digest as Sha2Digest, Sha256};

use super::envelope::EvidenceExport;

/// Calcula el digest esperado para un envelope: SHA-256 hex sobre los
/// bytes CBOR del envelope con `digest = ""` placeholder.
///
/// Esta rutina es la que el producer invoca una vez para declarar el
/// digest, y la que el consumer invoca sobre el envelope recibido para
/// verificarlo. Las dos deben producir el mismo SHA-256 sobre el mismo
/// DTO, lo que requiere paridad byte-a-byte entre ciborium (producer)
/// y kotlinx-serialization-cbor (consumer) sobre el mismo input.
pub fn digest_of(envelope: &EvidenceExport) -> String {
    let mut placeholder = envelope.clone();
    placeholder.digest = String::new();
    let canonical = super::serialize::encode_to_cbor(&placeholder).expect(
        "envelope placeholder debe ser encodable: la constraint de init ya \
         valida campos requeridos, encode falla solo si la libreria CBOR \
         rompe (bug nuestro, no del envelope)",
    );
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    let result = hasher.finalize();
    hex::encode(result)
}

/// SHA-256 hex de un buffer. Usado por golden tests y por la verificación
/// del golden en scripts/ci/test_assurance_export_golden.py.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Asigna el digest correcto al envelope y devuelve una copia lista para
/// serializar. Helper de `serialize_to_cbor_with_digest` para que la
/// callers no tengan que acordarse de la convención.
pub fn with_declared_digest(envelope: &EvidenceExport) -> EvidenceExport {
    let mut with_digest = envelope.clone();
    with_digest.digest = digest_of(envelope);
    with_digest
}
