//! DTOs del wire format `assurance-evidence/v1` que produce CogniCode.
//!
//! Ref autoridad:
//! - `pipelinek-assurance/07-integrations/COGNICODE_WORKSTREAM.md` (workstream)
//! - `pipelinek-assurance/assurance-providers/.../CogniCodeEvidenceExportDto.kt`
//!   (DTOs exactos del consumidor)
//!
//! Estos structs son el **productor** del envelope. Los nombres de campo,
//! tipos y restricciones deben encajar 1:1 con `CogniCodeEvidenceExportDto.kt`
//! y compañía. Si este modelo no produce los campos exactos que el codec del
//! consumidor espera, hay que ajustar el modelo, no el codec (que es
//! externo y está contractual).
//!
//! Visibilidad: este módulo es público dentro de `cognicode-core` porque el
//! CLI lo consume; desde fuera del crate, el boundary es el `cognicode
//! export assurance` CLI + el archivo CBOR/JSON.
//!
//! Convección digest (M-D01-style, fail-closed):
//! - Producer serializa el DTO con `digest = ""` placeholder.
//! - Calcula SHA-256 de esos bytes.
//! - Construye el DTO final con `digest = hex(sha256)`.
//! - Serializa el DTO final y lo emite.
//! - Consumer decodifica, copia con `digest = ""`, re-serializa, compara.
//!
//! Esto requiere que el encoding del producer sea byte-identical al del
//! consumer (kotlinx-serialization-cbor) sobre el mismo DTO. La verificación
//! de paridad vive en `tests/assurance_export_roundtrip.rs` y en el SHA-256
//! del golden export (ver `scripts/ci/test_assurance_export_golden.py`).

use serde::{Deserialize, Serialize};

/// Versión del schema del envelope.
///
/// El codec del consumidor (`CogniCodeEvidenceExportCodec.API_VERSION`)
/// falla cerrado si esto no coincide. Cambios incompatibles rompen el
/// contrato → bump a `assurance-evidence/v2`.
pub const API_VERSION: &str = "assurance-evidence/v1";

/// `kind` esperado por el codec (verifica en su `init {}`).
pub const KIND_EVIDENCE_EXPORT: &str = "EvidenceExport";

/// Media type para CBOR (`application/vnd.cognicode.assurance-evidence+cbor;version=1`).
pub const MEDIA_TYPE_CBOR: &str = "application/vnd.cognicode.assurance-evidence+cbor;version=1";

/// Media type para JSON debug (`application/vnd.cognicode.assurance-evidence+json;version=1`).
pub const MEDIA_TYPE_JSON: &str = "application/vnd.cognicode.assurance-evidence+json;version=1";

/// Cota de bytes máxima del envelope. El codec del consumidor falla cerrado
/// si excede. Aplicable a la entrada chained (read); en encode no se valida
/// (ver comentario en `CogniCodeEvidenceExportCodec.encodeToCbor`).
pub const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;

/// Cota de tamaño de colección (entities, facts, etc.).
pub const MAX_COLLECTION_SIZE: usize = 1024;

/// Cota de longitud de string (caracteres).
pub const MAX_STRING_LENGTH: usize = 65_536;

/// Cota de profundidad del árbol JSON (no aplica a CBOR; el codec del
/// consumidor aplica depth check solo en la pre-pasada JSON).
pub const MAX_NESTING_DEPTH: u8 = 8;

/// Envelope raíz `assurance-evidence/v1`.
///
/// El codec del consumidor verifica `apiVersion == "assurance-evidence/v1"`
/// y `kind == "EvidenceExport"` en su `init {}` (fail-closed).
///
/// `rename_all = "camelCase"` es OBLIGATORIO: el consumer serializa los
/// campos con sus nombres Kotlin (`apiVersion`, `sourceAnchors`,
/// `capabilityCompleteness`). Si el producer serializa con snake_case,
/// el digest del envelope difiere byte-a-byte del que el consumer
/// recomputa, y la verificación M-D01-style falla cerrado.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceExport {
    pub api_version: String,
    pub kind: String,
    pub producer: ProducerInfo,
    pub subject: SubjectRef,
    pub manifest: ManifestSection,
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
    pub relations: Vec<Relation>,
    pub signals: Vec<Signal>,
    pub source_anchors: Vec<SourceAnchor>,
    pub provenance: Provenance,
    pub capability_completeness: std::collections::BTreeMap<String, CapabilityCompleteness>,
    pub gaps: Vec<CapabilityGap>,
    pub digest: String,
}

/// Identidad del producer (CogniCode). Se replica en `manifest` por diseño:
/// producer y manifest viven en secciones distintas y el motor puede
/// necesitarlas por separado al normalizar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProducerInfo {
    pub id: String,
    pub version: String,
    pub schema_version: String,
}

/// Revisión y tipo del sujeto sobre el que CogniCode produjo la evidencia.
/// `revision` es lo que el consumer exige propagar para que la misma revision
/// produzca el mismo digest (UAT-024 en el lado consumer).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectRef {
    pub revision: String,
    pub kind: String,
}

/// Sección `manifest` del envelope.
///
/// `digest` aquí es el del MANIFEST considerado aisladamente (informativo);
/// el digest del envelope raíz firma el contenido COMPLETO. Misma forma,
/// contenido distinto. El codec del consumidor verifica el del envelope
/// raíz; el `manifest.digest` es declarativo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSection {
    pub requested_capabilities: Vec<String>,
    pub produced_capabilities: Vec<String>,
    pub completeness_by_capability: std::collections::BTreeMap<String, CapabilityCompleteness>,
    pub schema_version: String,
    pub digest: String,
}

/// Entidad estática de CogniCode (módulo, símbolo, archivo, etc.).
///
/// `layer` opcional: no toda entidad tiene capa arquitectónica. Cuando
/// falta, se omite (serde `skip_serializing_if = "Option::is_none"`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
}

/// Hecho determinista `subject-predicate-object` que CogniCode ya tenía.
///
/// `authority` viaja como string para aceptar autoridades nuevas sin
/// breaking change. El consumer mapea a `EvidenceAuthority` por nombre; lo
/// desconocido cae a `DeterministicAnalyzer` y se declara en la siguiente
/// auditoría (no en el decoder).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    pub id: String,
    pub entity_ref: String,
    pub predicate: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_value: Option<String>,
    pub authority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_anchor_ref: Option<String>,
}

/// Relación tipada entre dos entidades. `evidence` es el id del `Fact` que
/// la sostiene (UAT-001 en el lado consumer). Sin `evidence`, la relación
/// es opaca: no se sabe qué la motivó.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub evidence: String,
}

/// Señal heurística exportada por CogniCode.
///
/// `score` es `String` (no `Double`) porque el score puede ser numérico,
/// categórico o textual según la heurística: SOLID = "yes"/"partial"/"no",
/// god-function = "0.42". El tipo de dominio es String en ambos lados.
///
/// `thresholds` es `Map<String,String>` para conservar la misma forma que
/// el consumer espera.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Signal {
    pub id: String,
    pub entity_ref: String,
    pub kind: String,
    pub score: String,
    pub algorithm_id: String,
    pub algorithm_version: String,
    pub thresholds: std::collections::BTreeMap<String, String>,
}

/// Localización fuente de un `Fact` o `Signal`. `column` y `symbol_ref`
/// opcionales: lenguajes sin noción de columna (algunos formateadores) o
/// sin símbolo atómico los omiten.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceAnchor {
    pub file: String,
    pub line: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_ref: Option<String>,
}

/// Procedencia de un item exportado por CogniCode.
///
/// AAT-13 (autoridades separadas): `provenance.producerId` no es lo mismo
/// que `producer.id` aunque suele coincidir; `provenance.artifactRef`/
/// `artifactDigest` referencian el artefacto del que se leyó, no el envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub producer_id: String,
    pub producer_version: String,
    pub subject_revision: String,
    pub capability: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
}

/// Completitud declarada por CogniCode para una capability.
///
/// Mapea 1:1 a `CapabilityCompletenessDto` del consumer:
/// - `Complete` (sin payload)
/// - `Partial { gaps }` (gaps no vacío, requerido por el `init {}` del consumer)
/// - `Unknown` (sin payload)
/// - `Unsupported { reason }` (reason no vacío)
///
/// `tag = "kind"` + `rename_all = "snake_case"`: el consumer serializa el
/// discriminador como `{"kind": "complete"}` o `{"kind": "partial", "gaps": [...]}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityCompleteness {
    Complete,
    Partial { gaps: Vec<CapabilityGap> },
    Unknown,
    Unsupported { reason: String },
}

impl CapabilityCompleteness {
    /// Constructor seguro de `Partial`.
    ///
    /// El consumer (`pipelinek-assurance/.../CogniCodeEvidenceExportDto.kt`)
    /// exige en su `init {}` que `Partial.gaps.isNotEmpty()`. Sin este guard
    /// aquí, un `Partial { gaps: vec![] }` se serializa sin error en el
    /// producer y el consumer falla cerrado en runtime. El assert lo atrapa
    /// en el lado que introduce el error, no en el lado que lo detecta.
    pub fn partial(gaps: Vec<CapabilityGap>) -> Self {
        assert!(
            !gaps.is_empty(),
            "Partial expone al menos un gap, o deja de ser Partial (consumer DTO contract)"
        );
        Self::Partial { gaps }
    }

    /// Constructor seguro de `Unsupported`.
    ///
    /// El consumer exige `reason.isNotBlank()` (regla C4 del workstream:
    /// "Capabilities no soportadas = gap honesto con motivo, nunca vacío").
    pub fn unsupported(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        assert!(
            !reason.trim().is_empty(),
            "Unsupported requiere reason no vacío (consumer DTO contract)"
        );
        Self::Unsupported { reason }
    }

    /// Nombre estable de la capability, útil para mensajes de error y
    /// para mapear a la versión string del consumer (`"complete"`,
    /// `"partial"`, `"unknown"`, `"unsupported"`).
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial { .. } => "partial",
            Self::Unknown => "unknown",
            Self::Unsupported { .. } => "unsupported",
        }
    }

    /// `true` si la capability aporta items utilizables (Complete o Partial
    /// con gaps que el consumer sabe manejar).
    pub fn is_producing(&self) -> bool {
        matches!(self, Self::Complete | Self::Partial { .. })
    }
}

/// Gap de capability. El consumer parsea `reason` contra
/// `EvidenceGap.GapReason`; lo desconocido cae a `PartialProduced("<raw>")`,
/// que es lo más honesto: no afirma lo que no entiende, lo expone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityGap {
    pub capability: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
