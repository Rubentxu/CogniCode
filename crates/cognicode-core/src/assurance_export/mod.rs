//! Export `assurance-evidence/v1` que produce CogniCode para pipelinek-assurance.
//!
//! Ref autoridad:
//! - `pipelinek-assurance/07-integrations/COGNICODE_WORKSTREAM.md`
//! - `pipelinek-assurance/.../CogniCodeEvidenceExportDto.kt` (shape contractual)
//! - `pipelinek-assurance/.../CogniCodeEvidenceExportCodec.kt` (codec)
//!
//! Este módulo es el **productor** del envelope. El **consumidor**
//! (CogniCodeArtifactProvider en el lado assurance) ya existe y es
//! contractual: si este producer no encaja exactamente con su codec,
//! R1 de pipelinek-assurance queda bloqueado.
//!
//! Tres cosas pinea este módulo:
//!
//! 1. **Forma 1:1 con el DTO del consumer** (`envelope.rs`): camelCase,
//!    campos opcionales con `skip_serializing_if = "Option::is_none"`,
//!    sealed interface sobre `CapabilityCompleteness` con `tag = "kind"`.
//! 2. **Digest M-D01-style, fail-closed** (`digest.rs`): SHA-256 hex sobre
//!    los bytes CBOR del envelope con `digest = ""` placeholder.
//! 3. **Bounded decoding** (`serialize.rs`): `MAX_INPUT_BYTES`,
//!    `MAX_COLLECTION_SIZE`, `MAX_STRING_LENGTH`, `MAX_NESTING_DEPTH`.
//!    Aplica en decode (chained export + tests roundtrip); en encode el
//!    caller es de confianza porque construyó el DTO.
//!
//! Lo que NO hace este módulo:
//!
//! - Reimplementar análisis (Tarjan, símbolos, etc.). El extractor de
//!   workspace (`workspace_extractor.rs`) consume lo que CogniCode ya
//!   calcula; no añade análisis.
//! - Firmar el envelope con claves externas (release de pipelinek-assurance).
//! - Servir el envelope sobre transporte nuevo (es un formato de archivo).

pub mod digest;
pub mod envelope;
pub mod serialize;
pub mod workspace_extractor;

pub use digest::{digest_of, sha256_hex, with_declared_digest};
pub use envelope::{
    API_VERSION, CapabilityCompleteness, CapabilityGap, Entity, EvidenceExport, Fact,
    KIND_EVIDENCE_EXPORT, MAX_COLLECTION_SIZE, MAX_INPUT_BYTES, MAX_NESTING_DEPTH,
    MAX_STRING_LENGTH, MEDIA_TYPE_CBOR, MEDIA_TYPE_JSON, ManifestSection, ProducerInfo, Provenance,
    Relation, Signal, SourceAnchor, SubjectRef,
};
pub use serialize::{
    SerializeError, decode_from_cbor, decode_from_json, encode_to_cbor, encode_to_cbor_with_digest,
    encode_to_json, encode_to_json_with_digest,
};
pub use workspace_extractor::{ExtractionCapabilities, WorkspaceInfo, extract_from_workspace};
