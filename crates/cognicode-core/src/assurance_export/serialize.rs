//! Serialización CBOR + JSON del envelope `assurance-evidence/v1`.
//!
//! Compatibilidad con el codec del consumer (kotlinx-serialization-cbor):
//!
//! El codec del consumer decodifica el envelope con kotlinx-serialization-cbor
//! y re-calcula el digest sobre la versión re-codificada por kotlinx. Para
//! que esa verificación pase byte-a-byte, el encoding del producer
//! (Rust/ciborium) debe producir los mismos bytes que kotlinx-serialization-cbor
//! sobre el mismo DTO. Las reglas que se aplican:
//!
//! 1. **Mapas con definite-length** (no indefinite-length). Tanto kotlinx
//!    como ciborium por defecto.
//! 2. **Field order = declaration order** del `@Serializable`/struct. El
//!    DTO del consumer tiene el orden: apiVersion, kind, producer, subject,
//!    manifest, entities, facts, relations, signals, sourceAnchors,
//!    provenance, capabilityCompleteness, gaps, digest. El struct
//!    `EvidenceExport` en Rust respeta ese orden: `api_version`, `kind`,
//!    `producer`, `subject`, `manifest`, `entities`, `facts`, `relations`,
//!    `signals`, `source_anchors`, `provenance`, `capability_completeness`,
//!    `gaps`, `digest`. Las keys serde usan los nombres de campo Rust
//!    (`api_version` no `apiVersion`); kotlinx serializa los nombres del
//!    @SerialName o los nombres Kotlin de la propiedad. **Aquí está la
//!    divergencia**: el consumer usa `apiVersion` (camelCase), el struct
//!    Rust usa `api_version`. Hay que serializar con `rename_all = "camelCase"`.
//! 3. **Encoding de strings** = UTF-8 con length prefix. Idéntico en ambos.
//! 4. **Skip null** = el campo no se serializa si es None. Configurar con
//!    `skip_serializing_if = "Option::is_none"` en cada Option (hecho en
//!    `envelope.rs`).
//!
//! Si la paridad byte-a-byte falla (test `roundtrip_matches_consumer_codec`),
//! el diagnóstico vive en `tests/assurance_export_roundtrip.rs` y se
//! resuelve ajustando el encoding aquí, no cambiando el codec del consumer.

use ciborium::ser::into_writer;
use ciborium::value::Value as CborValue;

use super::digest::with_declared_digest;
use super::envelope::EvidenceExport;

/// `ciborium::Value` no expone `.kind()`; este helper devuelve el nombre
/// del variante para mensajes de error.
fn cbor_kind(v: &CborValue) -> &'static str {
    match v {
        CborValue::Null => "null",
        CborValue::Bool(_) => "bool",
        CborValue::Integer(_) => "integer",
        CborValue::Float(_) => "float",
        CborValue::Bytes(_) => "bytes",
        CborValue::Text(_) => "text",
        CborValue::Array(_) => "array",
        CborValue::Map(_) => "map",
        CborValue::Tag(_, _) => "tag",
        _ => "other",
    }
}

/// Error de serialización. Distinto de las excepciones de dominio: significa
/// "el envelope no se pudo serializar", no "el envelope viola invariantes".
#[derive(Debug, thiserror::Error)]
pub enum SerializeError {
    #[error("CBOR encode falló: {0}")]
    Cbor(String),
    #[error("JSON encode falló: {0}")]
    Json(String),
    #[error("campo no conforme a bounded decoding: {0}")]
    Bounded(String),
}

/// Serializa el envelope a CBOR con el digest declarado (M-D01-style).
///
/// Orden de operaciones (idéntico al producer del workstream del consumer):
/// 1. Computar `digest_of(envelope)` (placeholder con `digest = ""`).
/// 2. Construir envelope final con `digest = digest_of(...)`.
/// 3. Serializar el envelope final a CBOR.
/// 4. Devolver bytes.
///
/// Si el producer serializa con `digest = ""` (paso 1) y el consumer
/// re-serializa con `digest = ""` (paso 1 de `verifyDigest`), los bytes
/// son idénticos SI el encoding de ambos lados es determinista y respeta
/// el orden de declaración del DTO. Esa paridad se valida en tests.
pub fn encode_to_cbor_with_digest(envelope: &EvidenceExport) -> Result<Vec<u8>, SerializeError> {
    let with_digest = with_declared_digest(envelope);
    encode_to_cbor(&with_digest)
}

/// Serializa un envelope YA con `digest` declarado a CBOR. La convención
/// del digest es responsabilidad del caller; este helper solo codifica.
pub fn encode_to_cbor(envelope: &EvidenceExport) -> Result<Vec<u8>, SerializeError> {
    let mut buf = Vec::with_capacity(4096);
    into_writer(envelope, &mut buf).map_err(|e| SerializeError::Cbor(e.to_string()))?;
    Ok(buf)
}

/// Serializa el envelope a JSON debug con el digest declarado. Útil para
/// inspección humana y para golden tests que comparan texto en vez de
/// binario. El formato canónico para el consumer es CBOR; JSON es solo
/// debug.
pub fn encode_to_json_with_digest(envelope: &EvidenceExport) -> Result<String, SerializeError> {
    let with_digest = with_declared_digest(envelope);
    encode_to_json(&with_digest)
}

pub fn encode_to_json(envelope: &EvidenceExport) -> Result<String, SerializeError> {
    serde_json::to_string(envelope).map_err(|e| SerializeError::Json(e.to_string()))
}

/// Decodifica un envelope desde CBOR. **No usado por el producer normal**;
/// vive aquí para:
/// - Tests roundtrip (write + read).
/// - Chained export (producer que lee envelopes CogniCode previos para
///   derivar uno nuevo, con bounded decoding aplicado).
///
/// Aplica las mismas cotas que el codec del consumer (búsqueda del mismo
/// fail-closed). Cualquier violación falla con `SerializeError::Bounded`,
/// no con panic, para que el caller pueda reportar el motivo.
pub fn decode_from_cbor(bytes: &[u8]) -> Result<EvidenceExport, SerializeError> {
    if bytes.len() as u64 > super::envelope::MAX_INPUT_BYTES {
        return Err(SerializeError::Bounded(format!(
            "entrada CBOR de {} bytes excede MAX_INPUT_BYTES={}",
            bytes.len(),
            super::envelope::MAX_INPUT_BYTES
        )));
    }

    let value: CborValue = ciborium::de::from_reader(bytes)
        .map_err(|e| SerializeError::Cbor(format!("CBOR no decodificable: {e}")))?;

    // Bounded checks ANTES de construir el modelo. Misma política que el
    // codec del consumer sobre JSON: la profundidad/colección se verifica
    // sobre el árbol, no sobre bytes.
    check_bounded_cbor(&value, 0)?;

    let envelope = cbor_value_to_envelope(value)?;
    check_bounded_envelope(&envelope)?;
    Ok(envelope)
}

/// Decodifica un envelope desde JSON. Aplicar bounded decoding con
/// `MAX_NESTING_DEPTH = 8` antes de materializar el DTO (mismo flujo que
/// el codec del consumer sobre JSON).
pub fn decode_from_json(text: &str) -> Result<EvidenceExport, SerializeError> {
    use super::envelope::*;

    let byte_len = text.len() as u64;
    if byte_len > super::envelope::MAX_INPUT_BYTES {
        return Err(SerializeError::Bounded(format!(
            "entrada JSON de {byte_len} bytes excede MAX_INPUT_BYTES={}",
            super::envelope::MAX_INPUT_BYTES
        )));
    }

    let element: serde_json::Value =
        serde_json::from_str(text).map_err(|e| SerializeError::Json(e.to_string()))?;
    check_bounded_json(&element, 0)?;

    let envelope: EvidenceExport =
        serde_json::from_value(element).map_err(|e| SerializeError::Json(e.to_string()))?;
    check_bounded_envelope(&envelope)?;
    Ok(envelope)
}

// ----- bounded decoding helpers -------------------------------------------

/// Recorre el árbol CBOR y verifica profundidad. La profundidad semántica
/// se calcula como `max depth de cualquier child + 1`; las hojas (text,
/// int, bytes, etc.) tienen profundidad 1.
fn check_bounded_cbor(value: &CborValue, depth: u8) -> Result<(), SerializeError> {
    if depth >= super::envelope::MAX_NESTING_DEPTH {
        return Err(SerializeError::Bounded(format!(
            "anidamiento real = {depth} excede MAX_NESTING_DEPTH={}",
            super::envelope::MAX_NESTING_DEPTH
        )));
    }
    match value {
        CborValue::Map(entries) => {
            for (k, v) in entries {
                check_bounded_cbor(k, depth + 1)?;
                check_bounded_cbor(v, depth + 1)?;
            }
        }
        CborValue::Array(items) => {
            for item in items {
                check_bounded_cbor(item, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_bounded_json(value: &serde_json::Value, depth: u8) -> Result<(), SerializeError> {
    if depth >= super::envelope::MAX_NESTING_DEPTH {
        return Err(SerializeError::Bounded(format!(
            "anidamiento real = {depth} excede MAX_NESTING_DEPTH={}",
            super::envelope::MAX_NESTING_DEPTH
        )));
    }
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                check_bounded_string(k)?;
                check_bounded_json(v, depth + 1)?;
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                check_bounded_json(item, depth + 1)?;
            }
        }
        serde_json::Value::String(s) => check_bounded_string(s)?,
        _ => {}
    }
    Ok(())
}

fn check_bounded_string(s: &str) -> Result<(), SerializeError> {
    if s.len() > super::envelope::MAX_STRING_LENGTH {
        return Err(SerializeError::Bounded(format!(
            "cadena de {} caracteres excede MAX_STRING_LENGTH={}",
            s.len(),
            super::envelope::MAX_STRING_LENGTH
        )));
    }
    Ok(())
}

/// Verifica que la envelope materializada cumple con las cotas de tamaño
/// de colección y longitud de string. Aplicado en decode (no en encode:
/// el producer es de confianza porque construyó el DTO).
fn check_bounded_envelope(env: &EvidenceExport) -> Result<(), SerializeError> {
    use super::envelope::*;
    if env.entities.len() > MAX_COLLECTION_SIZE {
        return Err(bounded("entities", env.entities.len()));
    }
    if env.facts.len() > MAX_COLLECTION_SIZE {
        return Err(bounded("facts", env.facts.len()));
    }
    if env.relations.len() > MAX_COLLECTION_SIZE {
        return Err(bounded("relations", env.relations.len()));
    }
    if env.signals.len() > MAX_COLLECTION_SIZE {
        return Err(bounded("signals", env.signals.len()));
    }
    if env.source_anchors.len() > MAX_COLLECTION_SIZE {
        return Err(bounded("sourceAnchors", env.source_anchors.len()));
    }
    if env.gaps.len() > MAX_COLLECTION_SIZE {
        return Err(bounded("gaps", env.gaps.len()));
    }
    if env.capability_completeness.len() > MAX_COLLECTION_SIZE {
        return Err(bounded(
            "capabilityCompleteness",
            env.capability_completeness.len(),
        ));
    }
    for s in [
        &env.api_version,
        &env.kind,
        &env.digest,
        &env.producer.id,
        &env.producer.version,
        &env.producer.schema_version,
        &env.subject.revision,
        &env.subject.kind,
        &env.manifest.schema_version,
        &env.manifest.digest,
        &env.provenance.producer_id,
        &env.provenance.producer_version,
        &env.provenance.subject_revision,
        &env.provenance.capability,
    ] {
        check_bounded_string(s)?;
    }
    for e in &env.entities {
        check_bounded_string(&e.id)?;
        check_bounded_string(&e.kind)?;
        check_bounded_string(&e.name)?;
        if let Some(layer) = &e.layer {
            check_bounded_string(layer)?;
        }
    }
    for f in &env.facts {
        check_bounded_string(&f.id)?;
        check_bounded_string(&f.entity_ref)?;
        check_bounded_string(&f.predicate)?;
        check_bounded_string(&f.authority)?;
        if let Some(ov) = &f.object_value {
            check_bounded_string(ov)?;
        }
        if let Some(sa) = &f.source_anchor_ref {
            check_bounded_string(sa)?;
        }
    }
    for r in &env.relations {
        check_bounded_string(&r.from)?;
        check_bounded_string(&r.to)?;
        check_bounded_string(&r.kind)?;
        check_bounded_string(&r.evidence)?;
    }
    for s in &env.signals {
        check_bounded_string(&s.id)?;
        check_bounded_string(&s.entity_ref)?;
        check_bounded_string(&s.kind)?;
        check_bounded_string(&s.score)?;
        check_bounded_string(&s.algorithm_id)?;
        check_bounded_string(&s.algorithm_version)?;
        for (k, v) in &s.thresholds {
            check_bounded_string(k)?;
            check_bounded_string(v)?;
        }
    }
    for sa in &env.source_anchors {
        check_bounded_string(&sa.file)?;
        if let Some(sym) = &sa.symbol_ref {
            check_bounded_string(sym)?;
        }
    }
    for (cap, comp) in &env.capability_completeness {
        check_bounded_string(cap)?;
        match comp {
            CapabilityCompleteness::Unsupported { reason } => check_bounded_string(reason)?,
            CapabilityCompleteness::Partial { gaps } if gaps.len() > MAX_COLLECTION_SIZE => {
                return Err(bounded("Partial.gaps", gaps.len()));
            }
            _ => {}
        }
    }
    for g in &env.gaps {
        check_bounded_string(&g.capability)?;
        check_bounded_string(&g.reason)?;
        if let Some(d) = &g.detail {
            check_bounded_string(d)?;
        }
    }
    Ok(())
}

fn bounded(name: &str, size: usize) -> SerializeError {
    SerializeError::Bounded(format!(
        "{name}: {size} excede MAX_COLLECTION_SIZE={MAX_COLLECTION_SIZE}",
        MAX_COLLECTION_SIZE = super::envelope::MAX_COLLECTION_SIZE
    ))
}

// ----- CBOR -> envelope conversion ----------------------------------------

/// Convierte un árbol CBOR genérico en un `EvidenceExport`. Aplica la
/// conversión campo a campo; un campo desconocido falla cerrado con
/// `SerializeError::Cbor`.
fn cbor_value_to_envelope(value: CborValue) -> Result<EvidenceExport, SerializeError> {
    use super::envelope::*;
    use std::collections::BTreeMap;

    let map = match value {
        CborValue::Map(m) => m,
        other => {
            return Err(SerializeError::Cbor(format!(
                "envelope raiz debe ser map, encontrado: {:?}",
                cbor_kind(&other)
            )));
        }
    };

    let mut api_version: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut producer: Option<ProducerInfo> = None;
    let mut subject: Option<SubjectRef> = None;
    let mut manifest: Option<ManifestSection> = None;
    let mut entities: Vec<Entity> = Vec::new();
    let mut facts: Vec<Fact> = Vec::new();
    let mut relations: Vec<Relation> = Vec::new();
    let mut signals: Vec<Signal> = Vec::new();
    let mut source_anchors: Vec<SourceAnchor> = Vec::new();
    let mut provenance: Option<Provenance> = None;
    let mut capability_completeness: BTreeMap<String, CapabilityCompleteness> = BTreeMap::new();
    let mut gaps: Vec<CapabilityGap> = Vec::new();
    let mut digest: Option<String> = None;

    for (k, v) in map {
        let key = match k {
            CborValue::Text(t) => t,
            other => {
                return Err(SerializeError::Cbor(format!(
                    "key debe ser string, encontrado: {:?}",
                    cbor_kind(&other)
                )));
            }
        };
        match key.as_str() {
            "apiVersion" => api_version = Some(require_string(key, v)?),
            "kind" => kind = Some(require_string(key, v)?),
            "producer" => producer = Some(cbor_to_producer(v)?),
            "subject" => subject = Some(cbor_to_subject(v)?),
            "manifest" => manifest = Some(cbor_to_manifest(v)?),
            "entities" => entities = cbor_to_vec(key, v, cbor_to_entity)?,
            "facts" => facts = cbor_to_vec(key, v, cbor_to_fact)?,
            "relations" => relations = cbor_to_vec(key, v, cbor_to_relation)?,
            "signals" => signals = cbor_to_vec(key, v, cbor_to_signal)?,
            "sourceAnchors" => source_anchors = cbor_to_vec(key, v, cbor_to_source_anchor)?,
            "provenance" => provenance = Some(cbor_to_provenance(v)?),
            "capabilityCompleteness" => capability_completeness = cbor_to_capability_map(key, v)?,
            "gaps" => gaps = cbor_to_vec(key, v, cbor_to_capability_gap)?,
            "digest" => digest = Some(require_string(key, v)?),
            other => {
                return Err(SerializeError::Cbor(format!(
                    "campo desconocido en envelope: {other:?}"
                )));
            }
        }
    }

    let envelope = EvidenceExport {
        api_version: api_version.ok_or_else(|| SerializeError::Cbor("falta apiVersion".into()))?,
        kind: kind.ok_or_else(|| SerializeError::Cbor("falta kind".into()))?,
        producer: producer.ok_or_else(|| SerializeError::Cbor("falta producer".into()))?,
        subject: subject.ok_or_else(|| SerializeError::Cbor("falta subject".into()))?,
        manifest: manifest.ok_or_else(|| SerializeError::Cbor("falta manifest".into()))?,
        entities,
        facts,
        relations,
        signals,
        source_anchors,
        provenance: provenance.ok_or_else(|| SerializeError::Cbor("falta provenance".into()))?,
        capability_completeness,
        gaps,
        digest: digest.ok_or_else(|| SerializeError::Cbor("falta digest".into()))?,
    };

    if envelope.api_version != API_VERSION {
        return Err(SerializeError::Cbor(format!(
            "apiVersion desconocida: {} (esperada {})",
            envelope.api_version, API_VERSION
        )));
    }
    if envelope.kind != KIND_EVIDENCE_EXPORT {
        return Err(SerializeError::Cbor(format!(
            "kind inesperado: {} (esperado {})",
            envelope.kind, KIND_EVIDENCE_EXPORT
        )));
    }

    Ok(envelope)
}

fn require_string(key: String, v: CborValue) -> Result<String, SerializeError> {
    match v {
        CborValue::Text(s) => Ok(s),
        other => Err(SerializeError::Cbor(format!(
            "{key}: esperaba string, encontrado {:?}",
            cbor_kind(&other)
        ))),
    }
}

fn cbor_to_vec<T, F>(key: String, v: CborValue, f: F) -> Result<Vec<T>, SerializeError>
where
    F: FnMut(CborValue) -> Result<T, SerializeError>,
{
    match v {
        CborValue::Array(items) => items.into_iter().map(f).collect(),
        other => Err(SerializeError::Cbor(format!(
            "{key}: esperaba array, encontrado {:?}",
            cbor_kind(&other)
        ))),
    }
}

fn cbor_to_producer(v: CborValue) -> Result<super::envelope::ProducerInfo, SerializeError> {
    let map = require_map("producer", v)?;
    Ok(super::envelope::ProducerInfo {
        id: require_field(&map, "id")?,
        version: require_field(&map, "version")?,
        schema_version: require_field(&map, "schemaVersion")?,
    })
}

fn cbor_to_subject(v: CborValue) -> Result<super::envelope::SubjectRef, SerializeError> {
    let map = require_map("subject", v)?;
    Ok(super::envelope::SubjectRef {
        revision: require_field(&map, "revision")?,
        kind: require_field(&map, "kind")?,
    })
}

fn cbor_to_manifest(v: CborValue) -> Result<super::envelope::ManifestSection, SerializeError> {
    let map = require_map("manifest", v)?;
    let requested_capabilities = require_array(&map, "requestedCapabilities")?
        .into_iter()
        .map(require_string_of)
        .collect::<Result<Vec<_>, _>>()?;
    let produced_capabilities = require_array(&map, "producedCapabilities")?
        .into_iter()
        .map(require_string_of)
        .collect::<Result<Vec<_>, _>>()?;
    let completeness_by_capability = require_map_of(&map, "completenessByCapability")?;
    let capability_map = completeness_by_capability
        .into_iter()
        .map(|(k, v)| cbor_to_capability_completeness(k.clone(), v).map(|cc| (k, cc)))
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
    Ok(super::envelope::ManifestSection {
        requested_capabilities,
        produced_capabilities,
        completeness_by_capability: capability_map,
        schema_version: require_field(&map, "schemaVersion")?,
        digest: require_field(&map, "digest")?,
    })
}

fn cbor_to_provenance(v: CborValue) -> Result<super::envelope::Provenance, SerializeError> {
    let map = require_map("provenance", v)?;
    Ok(super::envelope::Provenance {
        producer_id: require_field(&map, "producerId")?,
        producer_version: require_field(&map, "producerVersion")?,
        subject_revision: require_field(&map, "subjectRevision")?,
        capability: require_field(&map, "capability")?,
        artifact_ref: optional_field(&map, "artifactRef")?,
        artifact_digest: optional_field(&map, "artifactDigest")?,
    })
}

fn cbor_to_entity(v: CborValue) -> Result<super::envelope::Entity, SerializeError> {
    let map = require_map("entity", v)?;
    Ok(super::envelope::Entity {
        id: require_field(&map, "id")?,
        kind: require_field(&map, "kind")?,
        name: require_field(&map, "name")?,
        layer: optional_field(&map, "layer")?,
    })
}

fn cbor_to_fact(v: CborValue) -> Result<super::envelope::Fact, SerializeError> {
    let map = require_map("fact", v)?;
    Ok(super::envelope::Fact {
        id: require_field(&map, "id")?,
        entity_ref: require_field(&map, "entityRef")?,
        predicate: require_field(&map, "predicate")?,
        object_value: optional_field(&map, "objectValue")?,
        authority: require_field(&map, "authority")?,
        source_anchor_ref: optional_field(&map, "sourceAnchorRef")?,
    })
}

fn cbor_to_relation(v: CborValue) -> Result<super::envelope::Relation, SerializeError> {
    let map = require_map("relation", v)?;
    Ok(super::envelope::Relation {
        from: require_field(&map, "from")?,
        to: require_field(&map, "to")?,
        kind: require_field(&map, "kind")?,
        evidence: require_field(&map, "evidence")?,
    })
}

fn cbor_to_signal(v: CborValue) -> Result<super::envelope::Signal, SerializeError> {
    let map = require_map("signal", v)?;
    let thresholds_map = require_map_of(&map, "thresholds")?;
    let thresholds = thresholds_map
        .into_iter()
        .map(|(k, v)| {
            let v_str = match v {
                CborValue::Text(s) => s,
                other => {
                    return Err(SerializeError::Cbor(format!(
                        "thresholds[{k}]: esperaba string, encontrado {:?}",
                        cbor_kind(&other)
                    )));
                }
            };
            Ok((k, v_str))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
    Ok(super::envelope::Signal {
        id: require_field(&map, "id")?,
        entity_ref: require_field(&map, "entityRef")?,
        kind: require_field(&map, "kind")?,
        score: require_field(&map, "score")?,
        algorithm_id: require_field(&map, "algorithmId")?,
        algorithm_version: require_field(&map, "algorithmVersion")?,
        thresholds,
    })
}

fn cbor_to_source_anchor(v: CborValue) -> Result<super::envelope::SourceAnchor, SerializeError> {
    let map = require_map("sourceAnchor", v)?;
    Ok(super::envelope::SourceAnchor {
        file: require_field(&map, "file")?,
        line: require_int_field(&map, "line")?,
        column: optional_int_field(&map, "column")?,
        symbol_ref: optional_field(&map, "symbolRef")?,
    })
}

fn cbor_to_capability_gap(v: CborValue) -> Result<super::envelope::CapabilityGap, SerializeError> {
    let map = require_map("gap", v)?;
    Ok(super::envelope::CapabilityGap {
        capability: require_field(&map, "capability")?,
        reason: require_field(&map, "reason")?,
        detail: optional_field(&map, "detail")?,
    })
}

fn cbor_to_capability_completeness(
    key: String,
    v: CborValue,
) -> Result<super::envelope::CapabilityCompleteness, SerializeError> {
    use super::envelope::CapabilityCompleteness;
    let map = require_map(&format!("completeness[{key}]"), v)?;
    let kind = require_field(&map, "kind")?;
    match kind.as_str() {
        "complete" => Ok(CapabilityCompleteness::Complete),
        "partial" => {
            let gaps_v = map
                .iter()
                .find(|(k, _)| k == "gaps")
                .map(|(_, v)| v.clone())
                .ok_or_else(|| SerializeError::Cbor("Partial sin gaps".into()))?;
            let gaps = cbor_to_vec("gaps".into(), gaps_v, cbor_to_capability_gap)?;
            if gaps.is_empty() {
                return Err(SerializeError::Cbor(
                    "Partial expone al menos un gap, o deja de ser Partial".into(),
                ));
            }
            Ok(CapabilityCompleteness::Partial { gaps })
        }
        "unknown" => Ok(CapabilityCompleteness::Unknown),
        "unsupported" => {
            let reason = require_field(&map, "reason")?;
            if reason.is_empty() {
                return Err(SerializeError::Cbor(
                    "Unsupported.reason no puede estar en blanco".into(),
                ));
            }
            Ok(CapabilityCompleteness::Unsupported { reason })
        }
        other => Err(SerializeError::Cbor(format!(
            "CapabilityCompleteness.kind desconocido: {other}"
        ))),
    }
}

fn require_map(where_: &str, v: CborValue) -> Result<Vec<(String, CborValue)>, SerializeError> {
    match v {
        CborValue::Map(m) => m
            .into_iter()
            .map(|(k, v)| {
                let key = match k {
                    CborValue::Text(t) => t,
                    other => {
                        return Err(SerializeError::Cbor(format!(
                            "{where_}: key no es text, encontrado {:?}",
                            cbor_kind(&other)
                        )));
                    }
                };
                Ok((key, v))
            })
            .collect(),
        other => Err(SerializeError::Cbor(format!(
            "{where_}: esperaba map, encontrado {:?}",
            cbor_kind(&other)
        ))),
    }
}

fn require_map_of(
    map: &[(String, CborValue)],
    key: &str,
) -> Result<Vec<(String, CborValue)>, SerializeError> {
    map.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| match v {
            CborValue::Map(m) => m
                .iter()
                .map(|(k2, v2)| {
                    let k2_str = match k2 {
                        CborValue::Text(s) => s.clone(),
                        other => {
                            return Err(SerializeError::Cbor(format!(
                                "{key}: key no es text, encontrado {:?}",
                                cbor_kind(other)
                            )));
                        }
                    };
                    Ok((k2_str, v2.clone()))
                })
                .collect(),
            other => Err(SerializeError::Cbor(format!(
                "{key}: esperaba map, encontrado {:?}",
                cbor_kind(other)
            ))),
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn require_array(map: &[(String, CborValue)], key: &str) -> Result<Vec<CborValue>, SerializeError> {
    map.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| match v {
            CborValue::Array(items) => Ok(items.clone()),
            other => Err(SerializeError::Cbor(format!(
                "{key}: esperaba array, encontrado {:?}",
                cbor_kind(other)
            ))),
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn require_field(map: &[(String, CborValue)], key: &str) -> Result<String, SerializeError> {
    map.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| require_string_of(v.clone()))
        .unwrap_or_else(|| {
            Err(SerializeError::Cbor(format!(
                "falta campo requerido: {key}"
            )))
        })
}

fn optional_field(
    map: &[(String, CborValue)],
    key: &str,
) -> Result<Option<String>, SerializeError> {
    match map.iter().find(|(k, _)| k == key) {
        None => Ok(None),
        // `Option::None` se serializa como CBOR null (no se omite con
        // `skip_serializing_if` — ver comentario en `envelope.rs`). El
        // null no es string; lo aceptamos como `None` para que el
        // roundtrip de un campo opcional nulo funcione.
        Some((_, CborValue::Null)) => Ok(None),
        Some((_, v)) => require_string_of(v.clone()).map(Some),
    }
}

fn require_int_field(map: &[(String, CborValue)], key: &str) -> Result<i64, SerializeError> {
    map.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| match v {
            CborValue::Integer(i) => match i64::try_from(*i) {
                Ok(v) => Ok(v),
                Err(_) => Err(SerializeError::Cbor(format!(
                    "{key}: integer fuera de rango i64"
                ))),
            },
            other => Err(SerializeError::Cbor(format!(
                "{key}: esperaba integer, encontrado {:?}",
                cbor_kind(other)
            ))),
        })
        .unwrap_or_else(|| {
            Err(SerializeError::Cbor(format!(
                "falta campo requerido: {key}"
            )))
        })
}

fn optional_int_field(
    map: &[(String, CborValue)],
    key: &str,
) -> Result<Option<i64>, SerializeError> {
    match map.iter().find(|(k, _)| k == key) {
        None => Ok(None),
        // Ver `optional_field`: `Option::None` es CBOR null, no Integer.
        Some((_, CborValue::Null)) => Ok(None),
        Some((_, v)) => match v {
            CborValue::Integer(i) => match i64::try_from(*i) {
                Ok(v) => Ok(Some(v)),
                Err(_) => Err(SerializeError::Cbor(format!(
                    "{key}: integer fuera de rango i64"
                ))),
            },
            other => Err(SerializeError::Cbor(format!(
                "{key}: esperaba integer, encontrado {:?}",
                cbor_kind(other)
            ))),
        },
    }
}

fn require_string_of(v: CborValue) -> Result<String, SerializeError> {
    match v {
        CborValue::Text(s) => Ok(s),
        other => Err(SerializeError::Cbor(format!(
            "esperaba string, encontrado {:?}",
            cbor_kind(&other)
        ))),
    }
}

fn cbor_to_capability_map(
    key: String,
    v: CborValue,
) -> Result<
    std::collections::BTreeMap<String, super::envelope::CapabilityCompleteness>,
    SerializeError,
> {
    let map = require_map(&key, v)?;
    let mut out = std::collections::BTreeMap::new();
    for (k, v) in map {
        let cc = cbor_to_capability_completeness(k.clone(), v)?;
        out.insert(k, cc);
    }
    Ok(out)
}
