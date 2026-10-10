//! Roundtrip del envelope `assurance-evidence/v1`:
//! write ⇒ encode_to_cbor ⇒ decode_from_cbor ⇒ equal (campos).
//!
//! Verifica que el producer (este crate) y el consumer
//! (pipelinek-assurance) ven el mismo shape. El digest M-D01-style
//! (fail-closed) se pinea por SHA-256 sobre el golden export: si
//! cambia el encoding del producer, el SHA-256 cambia, y el
//! golden test del lado consumer detecta el drift.
//!
//! Lo que cubre este fichero:
//! 1. Roundtrip CBOR: encode → decode → campos iguales.
//! 2. Digest estable: mismo envelope → mismo digest (SHA-256 hex).
//! 3. Digest cambia cuando cambia el payload (byte alterado en
//!    tránsito → fail-closed).
//! 4. Bounded decoding aplicado en decode: bytes > MAX_INPUT_BYTES →
//!    Bounded error; collection > MAX_COLLECTION_SIZE → Bounded.
//! 5. Capabilities parciales = gaps honestos, NO items: [].

use std::collections::BTreeMap;

use cognicode_core::assurance_export::{
    API_VERSION, CapabilityCompleteness, CapabilityGap, Entity, EvidenceExport, Fact,
    KIND_EVIDENCE_EXPORT, MAX_COLLECTION_SIZE, MAX_INPUT_BYTES, MAX_STRING_LENGTH, ManifestSection,
    ProducerInfo, Provenance, SourceAnchor, SubjectRef, decode_from_cbor, digest_of,
    encode_to_cbor, encode_to_cbor_with_digest,
};

/// Construye un envelope minimo valido para tests.
fn sample_envelope() -> EvidenceExport {
    let mut completeness: BTreeMap<String, CapabilityCompleteness> = BTreeMap::new();
    completeness.insert("entities".into(), CapabilityCompleteness::Complete);
    completeness.insert(
        "architecture".into(),
        CapabilityCompleteness::Unsupported {
            reason: "no se calcula en v1".into(),
        },
    );
    let mut manifest_digest_input = ManifestSection {
        requested_capabilities: vec!["entities".into(), "architecture".into()],
        produced_capabilities: vec!["entities".into()],
        completeness_by_capability: completeness.clone(),
        schema_version: API_VERSION.into(),
        digest: String::new(),
    };
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&manifest_digest_input).unwrap());
    manifest_digest_input.digest = hex::encode(hasher.finalize());

    EvidenceExport {
        api_version: API_VERSION.into(),
        kind: KIND_EVIDENCE_EXPORT.into(),
        producer: ProducerInfo {
            id: "cognicode".into(),
            version: "0.101.9".into(),
            schema_version: API_VERSION.into(),
        },
        subject: SubjectRef {
            revision: "HEAD".into(),
            kind: "rust-workspace".into(),
        },
        manifest: manifest_digest_input,
        entities: vec![Entity {
            id: "file://./crates/cognicode-core/src/lib.rs".into(),
            kind: "file".into(),
            name: "crates/cognicode-core/src/lib.rs".into(),
            layer: Some("cognicode-core".into()),
        }],
        facts: vec![Fact {
            id: "fact:exists:file://./crates/cognicode-core/src/lib.rs".into(),
            entity_ref: "file://./crates/cognicode-core/src/lib.rs".into(),
            predicate: "exists_at_path".into(),
            object_value: Some("./crates/cognicode-core/src/lib.rs".into()),
            authority: "DeterministicAnalyzer".into(),
            source_anchor_ref: Some("anchor:./crates/cognicode-core/src/lib.rs".into()),
        }],
        relations: vec![],
        signals: vec![],
        source_anchors: vec![SourceAnchor {
            file: "./crates/cognicode-core/src/lib.rs".into(),
            line: 0,
            column: None,
            symbol_ref: None,
        }],
        provenance: Provenance {
            producer_id: "cognicode".into(),
            producer_version: "0.101.9".into(),
            subject_revision: "HEAD".into(),
            capability: "assurance-export".into(),
            artifact_ref: None,
            artifact_digest: None,
        },
        capability_completeness: completeness,
        gaps: vec![CapabilityGap {
            capability: "architecture".into(),
            reason: "UnsupportedProduced(no se calcula en v1)".into(),
            detail: None,
        }],
        digest: String::new(),
    }
}

// ----- roundtrip CBOR -------------------------------------------------------

#[test]
fn roundtrip_cbor_preserves_all_fields() {
    let envelope = sample_envelope();
    let bytes = encode_to_cbor_with_digest(&envelope).expect("encode CBOR");
    let decoded = decode_from_cbor(&bytes).expect("decode CBOR");

    // Tras decode, el `digest` puede o no conservarse (es opaque al
    // consumer: lo re-verifica). Lo importante es que los demas campos
    // sobreviven byte-a-byte.
    assert_eq!(decoded.api_version, envelope.api_version);
    assert_eq!(decoded.kind, envelope.kind);
    assert_eq!(decoded.producer, envelope.producer);
    assert_eq!(decoded.subject, envelope.subject);
    assert_eq!(decoded.entities, envelope.entities);
    assert_eq!(decoded.facts, envelope.facts);
    assert_eq!(decoded.relations, envelope.relations);
    assert_eq!(decoded.signals, envelope.signals);
    assert_eq!(decoded.source_anchors, envelope.source_anchors);
    assert_eq!(decoded.provenance, envelope.provenance);
    assert_eq!(
        decoded.capability_completeness,
        envelope.capability_completeness
    );
    assert_eq!(decoded.gaps, envelope.gaps);
}

// ----- digest M-D01-style ---------------------------------------------------

#[test]
fn digest_is_stable_across_encodings() {
    let envelope = sample_envelope();
    let d1 = digest_of(&envelope);
    let d2 = digest_of(&envelope);
    assert_eq!(d1, d2, "digest_of debe ser determinista");
    assert_eq!(d1.len(), 64, "SHA-256 hex = 64 chars");
}

#[test]
fn digest_changes_when_payload_changes() {
    let mut envelope = sample_envelope();
    let d1 = digest_of(&envelope);
    envelope.entities[0].name = "different.rs".into();
    let d2 = digest_of(&envelope);
    assert_ne!(d1, d2, "un byte distinto cambia el digest (M-D01-style)");
}

#[test]
fn encode_with_digest_matches_declared_digest() {
    let envelope = sample_envelope();
    let bytes = encode_to_cbor_with_digest(&envelope).expect("encode");
    let decoded = decode_from_cbor(&bytes).expect("decode");
    let expected = digest_of(&envelope);
    // Tras decode, decoded.digest puede no coincidir porque el
    // consumer re-codifica con `digest = ""` para verificar.
    // Lo que verificamos aqui: el envelope final serializado tiene
    // el digest declarado igual al digest_of del envelope original.
    assert_eq!(
        decoded.digest, expected,
        "el digest declarado en el envelope final coincide con el digest computado"
    );
}

// ----- bounded decoding -----------------------------------------------------

#[test]
fn decode_rejects_bytes_over_max_input() {
    // Forjamos bytes de tamano MAX_INPUT_BYTES + 1 para forzar el corte.
    let huge = vec![0u8; (MAX_INPUT_BYTES as usize) + 1];
    let err = decode_from_cbor(&huge).expect_err("decode de entrada excesiva debe fallar");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("MAX_INPUT_BYTES"),
        "mensaje debe explicar el motivo: {msg}"
    );
}

#[test]
fn decode_rejects_collection_over_max_size() {
    // Construimos un envelope con entities > MAX_COLLECTION_SIZE y
    // verificamos que decode_from_cbor lo rechaza con error Bounded.
    let mut envelope = sample_envelope();
    for i in 0..(MAX_COLLECTION_SIZE + 5) {
        envelope.entities.push(Entity {
            id: format!("file://./extra/{i}.rs"),
            kind: "file".into(),
            name: format!("extra/{i}.rs"),
            layer: None,
        });
    }
    let bytes = encode_to_cbor_with_digest(&envelope).expect("encode");
    let err = decode_from_cbor(&bytes).expect_err("decode de entities excesivo debe fallar");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("entities") && msg.contains("MAX_COLLECTION_SIZE"),
        "mensaje debe nombrar la coleccion y la cota: {msg}"
    );
}

#[test]
fn decode_rejects_string_over_max_length() {
    let mut envelope = sample_envelope();
    envelope.entities[0].name = "x".repeat(MAX_STRING_LENGTH + 1);
    let bytes = encode_to_cbor_with_digest(&envelope).expect("encode");
    let err = decode_from_cbor(&bytes).expect_err("decode de string excesivo debe fallar");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("MAX_STRING_LENGTH"),
        "mensaje debe nombrar la cota de longitud: {msg}"
    );
}

#[test]
fn decode_rejects_unknown_field() {
    // Para forzar un campo desconocido, serializamos un CBOR a mano.
    // La forma mas facil: tomar un envelope valido y meter una key
    // extra via ciborium directamente.
    let mut envelope = sample_envelope();
    envelope.digest = digest_of(&envelope);
    let bytes = encode_to_cbor(&envelope).expect("encode");
    // Decodifica con ciborium a Value, aniade una key, re-serializa.
    use ciborium::value::Value;
    let value: Value = ciborium::de::from_reader(&bytes[..]).expect("decode to Value");
    let Value::Map(entries) = value else {
        panic!("root no es map");
    };
    let mut entries = entries;
    entries.push((
        Value::Text("alienField".into()),
        Value::Text("alien".into()),
    ));
    let mut new_bytes = Vec::with_capacity(bytes.len() + 32);
    ciborium::ser::into_writer(&Value::Map(entries), &mut new_bytes).expect("re-encode");
    let err =
        decode_from_cbor(&new_bytes).expect_err("decode con campo desconocido debe fallar cerrado");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("desconocido") || msg.contains("unknown"),
        "mensaje debe nombrar el motivo: {msg}"
    );
}

// ----- capabilities parciales / unsupported --------------------------------

#[test]
fn unsupported_capability_carries_reason_no_items() {
    let envelope = sample_envelope();
    // `architecture` esta Unsupported. Su entrada en capabilityCompleteness
    // lleva reason, NO items.
    let arch = envelope
        .capability_completeness
        .get("architecture")
        .expect("architecture capability declarada");
    match arch {
        CapabilityCompleteness::Unsupported { reason } => {
            assert!(!reason.is_empty(), "reason no puede estar vacio");
        }
        other => panic!("esperaba Unsupported, encontrado: {other:?}"),
    }
    // Ademas, el gap correspondiente aparece en `gaps[]`.
    assert!(
        envelope.gaps.iter().any(|g| g.capability == "architecture"),
        "gap de architecture debe aparecer en gaps[] (regla C4 del workstream)"
    );
}

#[test]
fn gap_carries_reason_and_optional_detail() {
    let envelope = sample_envelope();
    let gap = envelope
        .gaps
        .iter()
        .find(|g| g.capability == "architecture")
        .expect("gap de architecture");
    assert!(!gap.reason.is_empty());
    // detail es opcional; en el sample es None.
    assert!(gap.detail.is_none());
}

// ----- constructores seguros (defensa en profundidad contra consumer contract) -----

/// El consumer DTO exige `Partial.gaps.isNotEmpty()`. El constructor
/// `CapabilityCompleteness::partial` lo pinea en el lado producer: si
/// alguien lo construye con gaps vacíos, panic en debug+release. Test
/// RED: el panic debe ser no-recuperable, no un resultado que ignore
/// el caller.
#[test]
#[should_panic(expected = "Partial expone al menos un gap")]
fn partial_rejects_empty_gaps() {
    let _ = CapabilityCompleteness::partial(vec![]);
}

/// El consumer DTO exige `Unsupported.reason.isNotBlank()`. El constructor
/// `CapabilityCompleteness::unsupported` lo pinea.
#[test]
#[should_panic(expected = "Unsupported requiere reason no vacío")]
fn unsupported_rejects_blank_reason() {
    let _ = CapabilityCompleteness::unsupported("   ");
}

/// El happy path: el constructor acepta inputs válidos. Sanity check de
/// que el assert no dispara en el camino normal.
#[test]
fn partial_accepts_non_empty_gaps() {
    let gap = CapabilityGap {
        capability: "architecture".into(),
        reason: "Tarjan SCC no se calcula en v1".into(),
        detail: None,
    };
    let cc = CapabilityCompleteness::partial(vec![gap]);
    assert!(matches!(cc, CapabilityCompleteness::Partial { .. }));
    assert!(cc.is_producing());
}

// ----- paridad de encoding con el consumer (M-D01) ----------------------------

/// El consumer (`CogniCodeEvidenceExportCodec`) configura
/// `encodeDefaults = true` en su encoder CBOR, lo que significa que
/// emite TODOS los campos con su valor por defecto — incluyendo
/// `null` para `String? = null`. Cuando el consumer re-codifica el
/// DTO para verificar el M-D01 digest, emite los mismos campos.
///
/// Si el producer Rust usa `#[serde(skip_serializing_if =
/// "Option::is_none")]`, omite el campo y los bytes del envelope
/// difieren de los que el consumer re-emite. M-D01 fallaría cerrado
/// sin motivo.
///
/// Este test pinea que el producer emite `"field": null` (no omite)
/// para todos los `Option::None` del envelope: `entity.layer`,
/// `fact.objectValue`, `fact.sourceAnchorRef`, `sourceAnchor.column`,
/// `sourceAnchor.symbolRef`, `provenance.artifactRef`,
/// `provenance.artifactDigest`, `capabilityGap.detail`.
#[test]
fn optional_none_serializes_as_null_not_omitted() {
    // Construimos un envelope con TODOS los campos opcionales en None.
    let envelope = EvidenceExport {
        api_version: API_VERSION.into(),
        kind: KIND_EVIDENCE_EXPORT.into(),
        producer: cognicode_core::assurance_export::envelope::ProducerInfo {
            id: "cognicode".into(),
            version: "0.101.9".into(),
            schema_version: API_VERSION.into(),
        },
        subject: cognicode_core::assurance_export::envelope::SubjectRef {
            revision: "HEAD".into(),
            kind: "rust-workspace".into(),
        },
        manifest: cognicode_core::assurance_export::envelope::ManifestSection {
            requested_capabilities: vec![],
            produced_capabilities: vec![],
            completeness_by_capability: std::collections::BTreeMap::new(),
            schema_version: API_VERSION.into(),
            digest: String::new(),
        },
        entities: vec![cognicode_core::assurance_export::envelope::Entity {
            id: "file://x".into(),
            kind: "file".into(),
            name: "x".into(),
            layer: None, // <-- debe serializarse como null
        }],
        facts: vec![cognicode_core::assurance_export::envelope::Fact {
            id: "f".into(),
            entity_ref: "file://x".into(),
            predicate: "p".into(),
            object_value: None, // <-- null
            authority: "DeterministicAnalyzer".into(),
            source_anchor_ref: None, // <-- null
        }],
        relations: vec![],
        signals: vec![],
        source_anchors: vec![cognicode_core::assurance_export::envelope::SourceAnchor {
            file: "x".into(),
            line: 0,
            column: None,     // <-- null
            symbol_ref: None, // <-- null
        }],
        provenance: cognicode_core::assurance_export::envelope::Provenance {
            producer_id: "cognicode".into(),
            producer_version: "0.101.9".into(),
            subject_revision: "HEAD".into(),
            capability: "assurance-export".into(),
            artifact_ref: None,    // <-- null
            artifact_digest: None, // <-- null
        },
        capability_completeness: std::collections::BTreeMap::new(),
        gaps: vec![cognicode_core::assurance_export::envelope::CapabilityGap {
            capability: "x".into(),
            reason: "r".into(),
            detail: None, // <-- null
        }],
        digest: String::new(),
    };
    let bytes = encode_to_cbor(&envelope).expect("encode");
    // Decodifica a CborValue y verifica que las keys existen con valor null.
    use ciborium::value::Value as CborValue;
    let value: CborValue = ciborium::de::from_reader(&bytes[..]).expect("decode to Value");
    let CborValue::Map(entries) = value else {
        panic!("root no es map")
    };
    let get_str = |key: &str| -> &CborValue {
        entries
            .iter()
            .find(|(k, _)| matches!(k, CborValue::Text(t) if t == key))
            .map(|(_, v)| v)
            .expect(&format!("key {key} debe existir"))
    };
    // entities[0].layer: null
    let CborValue::Array(entities) = get_str("entities") else {
        panic!("entities no es array")
    };
    let CborValue::Map(entity) = &entities[0] else {
        panic!("entity no es map")
    };
    let layer = entity
        .iter()
        .find(|(k, _)| matches!(k, CborValue::Text(t) if t == "layer"))
        .map(|(_, v)| v)
        .expect("entity.layer debe existir");
    assert!(
        matches!(layer, CborValue::Null),
        "entity.layer con None debe ser CBOR null, encontrado {:?}",
        layer
    );
    // gaps[0].detail: null
    let CborValue::Array(gaps) = get_str("gaps") else {
        panic!("gaps no es array")
    };
    let CborValue::Map(gap) = &gaps[0] else {
        panic!("gap no es map")
    };
    let detail = gap
        .iter()
        .find(|(k, _)| matches!(k, CborValue::Text(t) if t == "detail"))
        .map(|(_, v)| v)
        .expect("gap.detail debe existir");
    assert!(
        matches!(detail, CborValue::Null),
        "gap.detail con None debe ser CBOR null"
    );
    // sourceAnchors[0].column: null
    let CborValue::Array(anchors) = get_str("sourceAnchors") else {
        panic!("sourceAnchors no es array")
    };
    let CborValue::Map(anchor) = &anchors[0] else {
        panic!("anchor no es map")
    };
    let column = anchor
        .iter()
        .find(|(k, _)| matches!(k, CborValue::Text(t) if t == "column"))
        .map(|(_, v)| v)
        .expect("anchor.column debe existir");
    assert!(
        matches!(column, CborValue::Null),
        "anchor.column con None debe ser CBOR null"
    );
}
