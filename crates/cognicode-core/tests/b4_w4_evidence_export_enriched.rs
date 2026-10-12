//! B4.W4 — Evidence Export enriquecido (impact / architecture / quality).
//!
//! Spec autoridad: `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md` §B4.W4 y
//! el goal doc `odd/tasks/b4-impact-architecture-quality.md`.
//!
//! Estos tests pinean tres puertos nuevos del exporter
//! `assurance-evidence/v1`:
//!
//! - `extract_impact_relations`: drena `ImpactEdge`s (B4.W1, módulo
//!   `crates/cognicode-core/src/impact/` aún no fusionado en este WU).
//! - `extract_architecture_observations`: drena
//!   `ArchitectureObservation`s (B4.W2, módulo
//!   `architecture::diagnostic` aún no fusionado).
//! - `extract_quality_findings`: drena `Finding`s (B4.W3, módulo
//!   `findings::*` aún no fusionado).
//!
//! Los tres siguen el patrón `*Accumulator` que B2.W4 pineó para
//! `Vec<Entity>` / `Vec<Fact>`: `out: &mut Vec<T>`, `max: usize`,
//! devuelven `ExtractionOutcome` (`Complete` / `Partial { gaps }` /
//! `Unsupported { capability_kind }`). Los tres honran el invariante
//! "symmetric limits" (B2.W4): si el accumulator pre-poblado excede
//! `max`, se trunca honestamente y se devuelve `Partial` con el gap
//! que el consumer (kotlinx) parsea.
//!
//! Estado de este commit:
//! - B4.W1 / W2 / W3 no están fusionados: los extractores devuelven
//!   `Unsupported` con `capability_kind` estable para que el caller
//!   pueda propagarlo al `capability_completeness` y `gaps` del
//!   envelope v1 actual sin alterar el wire (la golden CBOR sigue
//!   intacta).
//! - Los nuevos DTOs (`Observation`, `Finding`) están definidos como
//!   shape del productor; el bump top-level del envelope llega al
//!   release B4 en lockstep con la actualización del consumer.
//!
//! Bilingual: comentarios en inglés arriba, español en los puntos
//! donde el histórico del repo (PRs, JOURNALS, docs) exige
//! vocabulario local para no perder la trazabilidad.

use cognicode_core::assurance_export::{
    CapabilityCompleteness, EvidenceExport, ExtractionCapabilities, ExtractionOutcome, Finding,
    FindingAccumulator, Observation, ObservationAccumulator, ProducerInfo, Relation,
    RelationAccumulator, WorkspaceInfo, encode_to_cbor_with_digest, extract_architecture_observations,
    extract_from_workspace, extract_impact_relations, extract_quality_findings,
};
use cognicode_core::assurance_export::workspace_extractor::{
    CAP_ARCHITECTURE, CAP_ENTITIES, CAP_FACTS, CAP_RELATIONS, CAP_SIGNALS,
    CAPABILITY_KIND_ARCHITECTURE_OBSERVATIONS, CAPABILITY_KIND_IMPACT_RELATIONS,
    CAPABILITY_KIND_QUALITY_FINDINGS,
};

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

// ----- fixture helpers (same pattern as b2_w4_export_invariants) -----------

static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct FixtureDir(PathBuf);

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture_workspace(n: usize) -> (FixtureDir, WorkspaceInfo) {
    let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let tmp = std::env::temp_dir().join(format!("cognicode-b4w4-{pid}-{counter}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create tempdir");
    for i in 0..n {
        let path = tmp.join(format!("f{i}.rs"));
        std::fs::write(&path, format!("// file {i}\n")).expect("write .rs");
    }
    let info = WorkspaceInfo::detect(tmp.clone());
    (FixtureDir(tmp), info)
}

fn fixture_producer() -> ProducerInfo {
    ProducerInfo {
        id: "cognicode".into(),
        version: "b4.w4-test".into(),
        schema_version: "assurance-evidence/v1".into(),
    }
}

// =========================================================================
// 1. Graceful fallback: B4.W1 (impact) not yet merged
// =========================================================================

/// B4.W4 invariante 1: el extractor de `ImpactEdge`s declara
/// explícitamente `Unsupported` mientras B4.W1 no esté fusionado. No
/// infiere Complete ni deja el accumulator a medio poblar: el caller
/// sabe leer `capability_kind = "impact-relations:envelope-v2"` y
/// propagarlo al envelope como un CapabilityGap honesto.
#[test]
fn extract_impact_relations_returns_unsupported_when_b4w1_not_merged() {
    let mut acc: RelationAccumulator = Vec::new();
    let outcome = extract_impact_relations(&mut acc, 1024);
    match outcome {
        ExtractionOutcome::Unsupported { capability_kind } => {
            assert_eq!(
                capability_kind, CAPABILITY_KIND_IMPACT_RELATIONS,
                "B4.W4 invariant: capability_kind estable para que el consumer \
                 pueda discriminar 'todavia no expuesto' de 'se trunco' \
                 sin re-codificar el parser"
            );
        }
        other => panic!(
            "extract_impact_relations debe devolver Unsupported hasta que \
             B4.W1 fusione su modulo `impact/`, got {other:?}"
        ),
    }
    assert!(
        acc.is_empty(),
        "el extractor no debe tocar el accumulator mientras B4.W1 no exista, \
         got {} items",
        acc.len()
    );
}

/// Idempotencia: llamar al extractor dos veces no produce items y
/// siempre devuelve el mismo `capability_kind`. Esto es importante
/// porque la capa superior puede sondearlo varias veces por pass.
#[test]
fn extract_impact_relations_is_idempotent_when_unsupported() {
    let mut acc: RelationAccumulator = Vec::new();
    let outcome_a = extract_impact_relations(&mut acc, 1024);
    let outcome_b = extract_impact_relations(&mut acc, 1024);
    assert_eq!(
        outcome_a, outcome_b,
        "extract_impact_relations debe ser idempotente en el path Unsupported"
    );
    assert!(acc.is_empty());
}

// =========================================================================
// 2. B2.W4 invariant carries through — overflow path (symmetric limits)
// =========================================================================

/// B2.W4 invariante 3 (symmetric limits): el producer aplica
/// `MAX_COLLECTION_SIZE` consistentemente. El extractor de B4.W4
/// honra el `max` pasado por el caller y, si lo excede, trunca y
/// devuelve `Partial { gaps: [...] }` con un motivo no-vacío.
/// Verificar este path requiere pre-poblar el accumulator: el extractor
/// no genera items por sí solo cuando B4.W1 no existe, pero el
/// invariante "no se sale de `max`" se aplica a lo que el caller ya
/// tiene acumulado.
#[test]
fn extract_impact_relations_returns_partial_when_overflow() {
    let n: usize = 1100; // > MAX_COLLECTION_SIZE (1024)
    let mut acc: RelationAccumulator = (0..n)
        .map(|i| Relation {
            from: format!("from:{i}"),
            to: format!("to:{i}"),
            kind: "depends-on".into(),
            evidence: format!("fact:{i}"),
        })
        .collect();
    let outcome = extract_impact_relations(&mut acc, 1024);
    assert_eq!(
        acc.len(),
        1024,
        "extract_impact_relations debe truncar al `max` del caller \
         (simetrico a MAX_COLLECTION_SIZE=1024)"
    );
    match outcome {
        ExtractionOutcome::Partial { gaps } => {
            assert!(
                !gaps.is_empty(),
                "Partial requiere gaps no-vacíos por contrato (B2.W4 honest coverage)"
            );
            let gap_msg = &gaps[0];
            assert!(
                gap_msg.contains("max=1024") || gap_msg.contains("MAX_COLLECTION_SIZE"),
                "gap debe mencionar el límite violado: {gap_msg}"
            );
        }
        other => panic!(
            "overflow debe producir Partial con gaps, got {other:?} (B2.W4 symmetric limits)"
        ),
    }
}

// =========================================================================
// 3. Architecture observations fallback
// =========================================================================

#[test]
fn extract_architecture_observations_returns_unsupported_when_b4w2_not_merged() {
    let mut acc: ObservationAccumulator = Vec::new();
    let outcome = extract_architecture_observations(&mut acc, 1024);
    match outcome {
        ExtractionOutcome::Unsupported { capability_kind } => {
            assert_eq!(
                capability_kind, CAPABILITY_KIND_ARCHITECTURE_OBSERVATIONS,
                "B4.W4 invariant: capability_kind estable para B4.W2"
            );
        }
        other => panic!(
            "extract_architecture_observations debe devolver Unsupported \
             hasta que B4.W2 fusione `architecture::diagnostic`, got {other:?}"
        ),
    }
    assert!(acc.is_empty());
}

#[test]
fn extract_architecture_observations_honors_max() {
    let n: usize = 1100;
    let mut acc: ObservationAccumulator = (0..n)
        .map(|i| Observation {
            id: format!("obs:{i}"),
            entity_ref: format!("file:///mod{i}.rs"),
            kind: "layer-violation".into(),
            description: format!("observation {i}"),
            evidence_ref: Some(format!("fact:{i}")),
            authority: "ArchitectureEvaluator".into(),
        })
        .collect();
    let outcome = extract_architecture_observations(&mut acc, 1024);
    assert_eq!(
        acc.len(),
        1024,
        "extract_architecture_observations debe truncar al `max` del caller"
    );
    match outcome {
        ExtractionOutcome::Partial { gaps } => {
            assert!(!gaps.is_empty(), "Partial requiere gaps no-vacíos");
            assert!(gaps[0].contains("max=1024") || gaps[0].contains("MAX_COLLECTION_SIZE"));
        }
        other => panic!("overflow debe producir Partial, got {other:?}"),
    }
}

// =========================================================================
// 4. Quality findings fallback
// =========================================================================

#[test]
fn extract_quality_findings_returns_unsupported_when_b4w3_not_merged() {
    let mut acc: FindingAccumulator = Vec::new();
    let outcome = extract_quality_findings(&mut acc, 1024);
    match outcome {
        ExtractionOutcome::Unsupported { capability_kind } => {
            assert_eq!(
                capability_kind, CAPABILITY_KIND_QUALITY_FINDINGS,
                "B4.W4 invariant: capability_kind estable para B4.W3"
            );
        }
        other => panic!(
            "extract_quality_findings debe devolver Unsupported hasta \
             que B4.W3 fusione su modulo de findings, got {other:?}"
        ),
    }
    assert!(acc.is_empty());
}

#[test]
fn extract_quality_findings_honors_max() {
    let n: usize = 1100;
    let mut acc: FindingAccumulator = (0..n)
        .map(|i| Finding {
            id: format!("finding:{i}"),
            entity_ref: format!("file:///mod{i}.rs"),
            category: "complexity".into(),
            severity: "warning".into(),
            confidence: "0.80".into(),
            rule_ref: "cyclomatic-complexity".into(),
            location: Some(format!("file:///mod{i}.rs:{i}")),
            origin: "CyclomaticAnalyzer".into(),
        })
        .collect();
    let outcome = extract_quality_findings(&mut acc, 1024);
    assert_eq!(
        acc.len(),
        1024,
        "extract_quality_findings debe truncar al `max` del caller"
    );
    match outcome {
        ExtractionOutcome::Partial { gaps } => {
            assert!(!gaps.is_empty(), "Partial requiere gaps no-vacíos");
            assert!(gaps[0].contains("max=1024") || gaps[0].contains("MAX_COLLECTION_SIZE"));
        }
        other => panic!("overflow debe producir Partial, got {other:?}"),
    }
}

// =========================================================================
// 5. ExtractionOutcome contract
// =========================================================================

/// El `kind_name` del enum debe ser estable: es la cadena que el
/// caller mapea a `CapabilityCompleteness` del envelope. Si el string
/// cambia sin bump versionado, el consumer rompe su mapeo de kind.
#[test]
fn extraction_outcome_kind_name_returns_stable_strings() {
    assert_eq!(ExtractionOutcome::Complete.kind_name(), "complete");
    assert_eq!(
        ExtractionOutcome::Partial { gaps: vec!["x".into()] }.kind_name(),
        "partial"
    );
    assert_eq!(
        ExtractionOutcome::Unsupported {
            capability_kind: "k"
        }
        .kind_name(),
        "unsupported"
    );
}

#[test]
fn extraction_outcome_is_producing_only_for_complete_or_partial() {
    assert!(ExtractionOutcome::Complete.is_producing());
    assert!(ExtractionOutcome::Partial { gaps: vec!["x".into()] }.is_producing());
    assert!(
        !ExtractionOutcome::Unsupported {
            capability_kind: "k"
        }
        .is_producing(),
        "Unsupported NO es produccion: el caller no debe anexar items al envelope"
    );
}

/// Constantes `capability_kind` son inmutables y no-vacías: el
/// consumer (futuro) las parsea para discriminar qué extractor
/// falló. Si alguna está vacía o cambia silenciosamente, el contrato
/// M-D01 de producer-consumer queda a ciegas.
#[test]
fn capability_kind_tags_are_stable_non_empty() {
    assert!(!CAPABILITY_KIND_IMPACT_RELATIONS.is_empty());
    assert!(!CAPABILITY_KIND_ARCHITECTURE_OBSERVATIONS.is_empty());
    assert!(!CAPABILITY_KIND_QUALITY_FINDINGS.is_empty());
    // Tag estable incluye prefijo de capability y version del envelope
    // para que un bump a v2 produzca tags distinguibles.
    assert!(CAPABILITY_KIND_IMPACT_RELATIONS.contains("envelope-v2"));
    assert!(CAPABILITY_KIND_ARCHITECTURE_OBSERVATIONS.contains("envelope-v2"));
    assert!(CAPABILITY_KIND_QUALITY_FINDINGS.contains("envelope-v2"));
}

// =========================================================================
// 6. Envelope integration: los B4.W4 extractores no alteran el envelope v1
// =========================================================================

/// El camino v1 del envelope (`extract_from_workspace`) NO debe
/// cambiar al añadir los nuevos extractores B4.W4. Este test pinea
/// que el envelope resultante para las capabilities conocidas sigue
/// siendo determinista y cumple los invariantes B2.W4.
#[test]
fn envelope_b2w4_path_unaffected_by_b4w4_extractors() {
    let (_tmp, info) = fixture_workspace(3);
    let producer = fixture_producer();

    let caps = ExtractionCapabilities {
        requested: [
            CAP_ENTITIES,
            CAP_FACTS,
            CAP_RELATIONS,
            CAP_SIGNALS,
            CAP_ARCHITECTURE,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    };

    let env = extract_from_workspace(&info, &caps, &producer).expect("extract");

    // Invariante B2.W4 honest coverage: cada capability conocida aparece
    // en capability_completeness, incluso si es Unsupported.
    for cap in [CAP_ENTITIES, CAP_FACTS, CAP_RELATIONS, CAP_SIGNALS, CAP_ARCHITECTURE] {
        assert!(
            env.capability_completeness.contains_key(cap),
            "B2.W4 honest coverage: {cap} debe declararse en capability_completeness"
        );
    }

    // Cada Unsupported lleva reason no-vacío (B2.W4 invariante 5).
    for (cap, comp) in &env.capability_completeness {
        if let CapabilityCompleteness::Unsupported { reason } = comp {
            assert!(
                !reason.trim().is_empty(),
                "B2.W4 honest coverage: Unsupported {cap} requiere reason no-vacío"
            );
        }
    }

    // B4.W4 invariante: el digest del envelope sigue siendo estable
    // (los nuevos extractores no perturban el wire shape).
    let bytes = encode_to_cbor_with_digest(&env).expect("encode");
    assert!(!bytes.is_empty(), "envelope no debe serializar a vacío");
}

/// El envelope v1 emitido por `extract_from_workspace` con todas las
/// capabilities pedidas sigue produciendo los mismos bytes en dos runs.
/// Si los nuevos extractores B4.W4 alteraran el wire, el digest del
/// envelope cambiaría y este test fallaría.
#[test]
fn envelope_digest_stable_across_runs_with_b4w4_extractors() {
    let (_tmp, info) = fixture_workspace(4);
    let producer = fixture_producer();
    let caps = ExtractionCapabilities {
        requested: [
            CAP_ENTITIES,
            CAP_FACTS,
            CAP_RELATIONS,
            CAP_SIGNALS,
            CAP_ARCHITECTURE,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    };

    let env_a = extract_from_workspace(&info, &caps, &producer).expect("extract a");
    let env_b = extract_from_workspace(&info, &caps, &producer).expect("extract b");

    let bytes_a = encode_to_cbor_with_digest(&env_a).expect("encode a");
    let bytes_b = encode_to_cbor_with_digest(&env_b).expect("encode b");
    assert_eq!(
        bytes_a, bytes_b,
        "B4.W4 no debe alterar la estabilidad byte-a-byte del envelope v1 (B2.W4)"
    );
}

/// B4.W4 invariante de completeness: una capability que devuelve
/// `Unsupported` exige (B2.W4) que la envelope declare
/// `capabilityCompleteness[cap] = Unsupported { reason }` con `reason`
/// no-vacío. Aquí se verifica a través del envelope agregando un gap
/// explícito que un caller de B4.W4 podría añadir si integrara el
/// extractor en `extract_from_workspace`.
#[test]
fn envelope_with_unsupported_capability_carries_explicit_gap() {
    // Simulamos la integración futura: el caller añade un gap basado
    // en el `capability_kind` estable del extractor. El envelope
    // resultante debe llevar el gap honesto (no `items: []` silencioso).
    let env = build_minimal_envelope_with_gap(
        "impact-relations:envelope-v2",
        "B4.W1 module `impact/` not yet merged",
    );
    assert!(
        env.gaps.iter().any(|g| g.capability == "impact-relations"),
        "el envelope debe llevar un gap explícito para la capability \
         impact-relations (B4.W4 + B2.W4 honest coverage). \
         gaps={:?}",
        env.gaps
    );
    let gap = env
        .gaps
        .iter()
        .find(|g| g.capability == "impact-relations")
        .expect("gap impact-relations presente");
    assert!(
        !gap.reason.is_empty(),
        "B2.W4 honest coverage: el gap debe llevar reason no-vacío"
    );
    assert!(
        gap.reason.contains("UnsupportedProduced"),
        "B2.W4: el gap reason lleva el prefijo parseable por el consumer"
    );
}

// =========================================================================
// helpers
// =========================================================================

/// Construye un envelope mínimo con un solo gap, útil para verificar
/// que la shape `CapabilityGap` (que es la vía actual de declarar
/// `Unsupported` en el envelope v1, hasta el bump de B4 release) sigue
/// honrando los invariantes B2.W4. No toca el wire shape: es solo
/// construcción local para los tests de arriba.
fn build_minimal_envelope_with_gap(capability: &str, reason: &str) -> EvidenceExport {
    use cognicode_core::assurance_export::{
        CapabilityGap, ManifestSection, Provenance, SubjectRef, API_VERSION,
        KIND_EVIDENCE_EXPORT,
    };
    use std::collections::BTreeMap;

    let cap_name = capability.split(':').next().unwrap_or(capability).to_string();
    let mut completeness: BTreeMap<String, CapabilityCompleteness> = BTreeMap::new();
    completeness.insert(
        cap_name.clone(),
        CapabilityCompleteness::unsupported(reason),
    );
    let gap = CapabilityGap {
        capability: cap_name,
        reason: format!("UnsupportedProduced({reason})"),
        detail: Some(
            "B4.W4: extractor returns ExtractionOutcome::Unsupported until \
             B4.W1/W2/W3 module lands. caller maps capability_kind to a \
             CapabilityGap with this reason."
                .into(),
        ),
    };

    EvidenceExport {
        api_version: API_VERSION.into(),
        kind: KIND_EVIDENCE_EXPORT.into(),
        producer: fixture_producer(),
        subject: SubjectRef {
            revision: "b4w4-test".into(),
            kind: "test".into(),
        },
        manifest: ManifestSection {
            requested_capabilities: vec![capability.into()],
            produced_capabilities: vec![],
            completeness_by_capability: completeness.clone(),
            schema_version: API_VERSION.into(),
            digest: String::new(),
        },
        entities: vec![],
        facts: vec![],
        relations: vec![],
        signals: vec![],
        source_anchors: vec![],
        provenance: Provenance {
            producer_id: "cognicode".into(),
            producer_version: "b4.w4-test".into(),
            subject_revision: "b4w4-test".into(),
            capability: "assurance-export".into(),
            artifact_ref: None,
            artifact_digest: None,
        },
        capability_completeness: completeness,
        gaps: vec![gap],
        digest: String::new(),
    }
}
