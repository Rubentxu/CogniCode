//! B2.W4 — Invariantes del productor `assurance-evidence/v1`.
//!
//! Spec autoridad: `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md` §B2.W4.
//!
//! Este fichero pinea los 6 invariantes que el goal doc exige al
//! productor:
//!
//! 1. **Order independence**: el orden de `--capability` flags no
//!    cambia el envelope (commutative).
//! 2. **Single extraction**: el walk del workspace se hace una sola
//!    vez, antes de procesar las capacidades.
//! 3. **Symmetric limits**: el productor aplica `MAX_COLLECTION_SIZE`
//!    a sus colecciones; el consumidor aplica el mismo límite y
//!    rechaza con el mismo nombre.
//! 4. **Effective worktree identity**: `subject.revision` y
//!    `provenance.subject_revision` reflejan la identidad real del
//!    worktree al tiempo de emisión.
//! 5. **Honest coverage**: `Complete` ⟹ colección no-vacía;
//!    `Partial` ⟹ `gaps` no-vacío; `Unsupported` ⟹ `reason` no-vacío.
//! 6. **Reference consistency**: cada `Fact.source_anchor_ref`
//!    resuelve a un `SourceAnchor`; cada `SourceAnchor.file` es un
//!    path real bajo el workspace.
//!
//! Estos tests son RED antes del refactor de `extract_from_workspace`
//! en `crates/cognicode-core/src/assurance_export/workspace_extractor.rs`.
//!
//! Estilo: convención con los 17 tests de
//! `assurance_export_roundtrip.rs` (mismo setup, mismas aserciones
//! semánticas; tests adicionales en archivo nuevo pinean la parte de
//! extracción que el roundtrip no cubre).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use cognicode_core::assurance_export::{
    API_VERSION, CapabilityCompleteness, Entity, EvidenceExport, ExtractionCapabilities,
    KIND_EVIDENCE_EXPORT, MAX_COLLECTION_SIZE, ManifestSection, ProducerInfo, Provenance,
    SourceAnchor, SubjectRef, WorkspaceInfo, decode_from_cbor, encode_to_cbor_with_digest,
    extract_from_workspace, workspace_extractor,
};
use workspace_extractor::{
    CAP_ARCHITECTURE, CAP_ENTITIES, CAP_FACTS, CAP_RELATIONS, CAP_SIGNALS, known_capabilities,
};

// ----- fixture: temp workspace with .rs files ------------------------------

/// Counter global para que cada test use un path único (cargo test corre
/// en paralelo; sin unicidad se interfieren entre sí).
static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Cleanup de un tempdir. Si el test falla a mitad, igualmente se borra.
struct FixtureDir(PathBuf);

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Crea un workspace temporal con `n` archivos `.rs` y `subdir/` con
/// archivos adicionales, suficiente para que `walk_rust_files` los
/// encuentre. NO es un git repo, así que `WorkspaceInfo::detect` cae
/// al fallback del nombre del directorio.
fn fixture_workspace(n: usize, with_subdir: bool) -> (FixtureDir, WorkspaceInfo) {
    let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let tmp = std::env::temp_dir().join(format!("cognicode-b2w4-{pid}-{counter}"));
    // Si quedó de un run anterior, lo limpiamos primero.
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create tempdir");

    for i in 0..n {
        let path = tmp.join(format!("f{i}.rs"));
        std::fs::write(&path, format!("// file {i}\n")).expect("write .rs");
    }
    if with_subdir {
        let sub = tmp.join("sub");
        std::fs::create_dir_all(&sub).expect("create subdir");
        for i in 0..3 {
            let path = sub.join(format!("inner{i}.rs"));
            std::fs::write(&path, format!("// inner {i}\n")).expect("write sub .rs");
        }
    }
    let info = WorkspaceInfo::detect(tmp.clone());
    (FixtureDir(tmp), info)
}

fn fixture_producer() -> ProducerInfo {
    ProducerInfo {
        id: "cognicode".into(),
        version: "b2.w4-test".into(),
        schema_version: "assurance-evidence/v1".into(),
    }
}

// ----- helpers para comparación order-independent -------------------------

fn sorted_entities(env: &EvidenceExport) -> Vec<&Entity> {
    let mut v: Vec<&Entity> = env.entities.iter().collect();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    v
}

fn sorted_source_anchors(env: &EvidenceExport) -> Vec<&SourceAnchor> {
    let mut v: Vec<&SourceAnchor> = env.source_anchors.iter().collect();
    v.sort_by(|a, b| a.file.cmp(&b.file));
    v
}

// ----- 1. Order independence ----------------------------------------------

#[test]
fn order_independent_capability_ordering() {
    let (_tmp, info) = fixture_workspace(5, true);
    let producer = fixture_producer();

    // Mismo conjunto de capacidades, dos órdenes distintos. entities
    // y facts deben ser byte-iguales (mismo set de paths canónicos).
    let caps_a = ExtractionCapabilities {
        requested: vec![
            CAP_ENTITIES.into(),
            CAP_FACTS.into(),
            CAP_ARCHITECTURE.into(),
            CAP_RELATIONS.into(),
        ],
    };
    let caps_b = ExtractionCapabilities {
        requested: vec![
            CAP_RELATIONS.into(),
            CAP_ARCHITECTURE.into(),
            CAP_FACTS.into(),
            CAP_ENTITIES.into(),
        ],
    };

    let env_a = extract_from_workspace(&info, &caps_a, &producer).expect("extract a");
    let env_b = extract_from_workspace(&info, &caps_b, &producer).expect("extract b");

    // Entities byte-iguales tras canonicalizar por id.
    assert_eq!(
        sorted_entities(&env_a),
        sorted_entities(&env_b),
        "entities deben ser iguales independientemente del orden de capabilities"
    );

    // Facts byte-iguales tras canonicalizar por id.
    let mut facts_a: Vec<_> = env_a.facts.iter().collect();
    let mut facts_b: Vec<_> = env_b.facts.iter().collect();
    facts_a.sort_by(|x, y| x.id.cmp(&y.id));
    facts_b.sort_by(|x, y| x.id.cmp(&y.id));
    assert_eq!(
        facts_a, facts_b,
        "facts deben ser iguales independientemente del orden de capabilities"
    );

    // Source anchors byte-iguales.
    assert_eq!(
        sorted_source_anchors(&env_a),
        sorted_source_anchors(&env_b),
        "source_anchors deben ser iguales independientemente del orden"
    );

    // capability_completeness (BTreeMap) y gaps deben coincidir.
    assert_eq!(env_a.capability_completeness, env_b.capability_completeness);
    assert_eq!(env_a.gaps, env_b.gaps);
}

// ----- 2. Single extraction -----------------------------------------------

#[test]
fn single_extraction_walks_workspace_once() {
    let (_tmp, info) = fixture_workspace(5, true);
    let producer = fixture_producer();

    // Si el walk se hace DESPUÉS de procesar `facts`, `facts` ve
    // `entities=[]` y produce 0 facts. Tras el refactor, el walk
    // ocurre antes de procesar cualquier capability, así que
    // `[facts, entities]` debe producir:
    //   - entities no-vacías (el walk ocurrió)
    //   - facts no-vacíos Y consistentes con entities
    let caps = ExtractionCapabilities {
        requested: vec![CAP_FACTS.into(), CAP_ENTITIES.into()],
    };

    let env = extract_from_workspace(&info, &caps, &producer).expect("extract");

    assert!(
        !env.entities.is_empty(),
        "entities debe ser no-vacío: el walk ocurrió antes del procesamiento"
    );
    assert!(
        !env.facts.is_empty(),
        "facts debe ser no-vacío: el walk ocurrió antes del procesamiento"
    );

    // Consistencia: cada fact referencia un entity que existe.
    let entity_ids: std::collections::BTreeSet<&str> =
        env.entities.iter().map(|e| e.id.as_str()).collect();
    for fact in &env.facts {
        assert!(
            entity_ids.contains(fact.entity_ref.as_str()),
            "fact {} referencia entity desconocido {}",
            fact.id,
            fact.entity_ref
        );
    }

    // Cada fact con source_anchor_ref debe tener su anchor en
    // source_anchors (referencia consistente).
    let anchor_files: std::collections::BTreeSet<&str> =
        env.source_anchors.iter().map(|a| a.file.as_str()).collect();
    for fact in &env.facts {
        if let Some(anchor_ref) = &fact.source_anchor_ref {
            let path = anchor_ref.strip_prefix("anchor:").unwrap_or(anchor_ref);
            assert!(
                anchor_files.contains(path),
                "fact {} source_anchor_ref {} no resuelve a un SourceAnchor",
                fact.id,
                anchor_ref
            );
        }
    }
}

// ----- 3. Symmetric limits ------------------------------------------------

#[test]
fn symmetric_limits_producer_enforces_max_collection() {
    // Crear un workspace con > MAX_COLLECTION_SIZE archivos .rs.
    // El producer debe truncar a MAX_COLLECTION_SIZE y marcar la
    // capability como Partial con un gap honesto.
    let n = MAX_COLLECTION_SIZE + 10;
    let (_tmp, info) = fixture_workspace(n, false);
    let producer = fixture_producer();

    let caps = ExtractionCapabilities {
        requested: vec![CAP_ENTITIES.into(), CAP_FACTS.into()],
    };
    let env = extract_from_workspace(&info, &caps, &producer).expect("extract");

    assert!(
        env.entities.len() <= MAX_COLLECTION_SIZE,
        "producer debe truncar entities a MAX_COLLECTION_SIZE={}, got {}",
        MAX_COLLECTION_SIZE,
        env.entities.len()
    );
    assert!(
        env.facts.len() <= MAX_COLLECTION_SIZE,
        "producer debe truncar facts a MAX_COLLECTION_SIZE={}, got {}",
        MAX_COLLECTION_SIZE,
        env.facts.len()
    );

    // La capability entities debe marcarse Partial con un gap que
    // mencione la truncación (no Complete).
    let entities_comp = env
        .capability_completeness
        .get(CAP_ENTITIES)
        .expect("entities capability declarada");
    match entities_comp {
        CapabilityCompleteness::Partial { gaps } => {
            assert!(!gaps.is_empty(), "Partial requiere gaps no-vacíos");
            assert!(
                gaps.iter().any(|g| g.capability == CAP_ENTITIES),
                "gap de entities debe aparecer en gaps[]"
            );
        }
        CapabilityCompleteness::Complete => panic!(
            "entities con {n} archivos debe ser Partial, no Complete (producer no debe mentir)"
        ),
        other => panic!("kind inesperado para entities con truncación: {other:?}"),
    }
}

#[test]
fn symmetric_limits_consumer_rejects_with_same_constant() {
    // Verifica que el consumer usa MAX_COLLECTION_SIZE y rechaza con
    // el mismo nombre que el producer trunca. Si este test pasa, las
    // dos partes del producer/consumer comparten el límite.
    //
    // Construimos un envelope válido con MAX_COLLECTION_SIZE + 5
    // entities; el producer NO las habría producido (las habría
    // truncado), pero el consumer debe rechazarlas porque comparte
    // el mismo límite.
    let mut completeness: BTreeMap<String, CapabilityCompleteness> = BTreeMap::new();
    completeness.insert(CAP_ENTITIES.into(), CapabilityCompleteness::Complete);
    let mut entities = Vec::with_capacity(MAX_COLLECTION_SIZE + 5);
    for i in 0..(MAX_COLLECTION_SIZE + 5) {
        entities.push(Entity {
            id: format!("file:///extra/{i}.rs"),
            kind: "file".into(),
            name: format!("extra/{i}.rs"),
            layer: None,
        });
    }
    let env = EvidenceExport {
        api_version: API_VERSION.into(),
        kind: KIND_EVIDENCE_EXPORT.into(),
        producer: fixture_producer(),
        subject: SubjectRef {
            revision: "deadbeef".into(),
            kind: "test".into(),
        },
        manifest: ManifestSection {
            requested_capabilities: vec![CAP_ENTITIES.into()],
            produced_capabilities: vec![CAP_ENTITIES.into()],
            completeness_by_capability: completeness.clone(),
            schema_version: API_VERSION.into(),
            digest: String::new(),
        },
        entities,
        facts: vec![],
        relations: vec![],
        signals: vec![],
        source_anchors: vec![],
        provenance: Provenance {
            producer_id: "cognicode".into(),
            producer_version: "b2.w4-test".into(),
            subject_revision: "deadbeef".into(),
            capability: "assurance-export".into(),
            artifact_ref: None,
            artifact_digest: None,
        },
        capability_completeness: completeness,
        gaps: vec![],
        digest: String::new(),
    };
    let bytes = encode_to_cbor_with_digest(&env).expect("encode");
    let err = decode_from_cbor(&bytes).expect_err("decode debe rechazar");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("entities") && msg.contains("MAX_COLLECTION_SIZE"),
        "consumer debe rechazar con el nombre entities + MAX_COLLECTION_SIZE: {msg}"
    );
}

// ----- 4. Effective worktree identity -------------------------------------

#[test]
fn effective_worktree_identity() {
    let (_tmp, info) = fixture_workspace(3, false);
    let producer = fixture_producer();

    let caps = ExtractionCapabilities {
        requested: vec![CAP_ENTITIES.into()],
    };
    let env = extract_from_workspace(&info, &caps, &producer).expect("extract");

    // Una sola fuente de verdad: env.subject.revision == env.provenance.subject_revision
    // == info.revision (que es git rev-parse HEAD o el fallback del directorio).
    assert_eq!(
        env.subject.revision, env.provenance.subject_revision,
        "subject.revision y provenance.subject_revision deben coincidir"
    );
    assert_eq!(
        env.subject.revision, info.revision,
        "subject.revision debe coincidir con info.revision"
    );

    // El workspace temporal NO es un git repo: la revision cae al
    // nombre del directorio. Eso es la identidad efectiva del
    // worktree para un workspace no-git.
    assert!(
        !env.subject.revision.is_empty(),
        "revision no debe estar vacía"
    );
    assert_ne!(
        env.subject.revision, "unknown",
        "revision debe caer al nombre del dir, no a 'unknown'"
    );
}

// ----- 5. Honest coverage -------------------------------------------------

#[test]
fn honest_coverage_invariants() {
    let (_tmp, info) = fixture_workspace(5, true);
    let producer = fixture_producer();

    let caps = ExtractionCapabilities {
        requested: known_capabilities().iter().map(|s| s.to_string()).collect(),
    };
    let env = extract_from_workspace(&info, &caps, &producer).expect("extract");

    // Para cada capability declarada:
    for (name, comp) in &env.capability_completeness {
        match comp {
            CapabilityCompleteness::Complete => {
                // Complete requiere colección no-vacía.
                let collection = collection_for(name, &env);
                assert!(
                    !collection.is_empty(),
                    "Complete {name} requiere colección no-vacía (got {} items)",
                    collection.len()
                );
            }
            CapabilityCompleteness::Partial { gaps } => {
                assert!(
                    !gaps.is_empty(),
                    "Partial {name} requiere gaps no-vacíos (invi del constructor)"
                );
                assert!(
                    gaps.iter().all(|g| !g.reason.is_empty()),
                    "todos los gaps de {name} deben llevar reason"
                );
            }
            CapabilityCompleteness::Unsupported { reason } => {
                assert!(
                    !reason.trim().is_empty(),
                    "Unsupported {name} requiere reason no-vacío (invi del constructor)"
                );
            }
            CapabilityCompleteness::Unknown => {
                // Unknown no requiere payload. Sin invariantes adicionales.
            }
        }
    }

    // Las 5 capabilities conocidas deben aparecer en capability_completeness
    // (incluso las Unsupported): no se omiten en silencio.
    for cap in known_capabilities() {
        assert!(
            env.capability_completeness.contains_key(*cap),
            "capability {cap} debe declararse en capability_completeness"
        );
    }
}

/// Devuelve la colección que corresponde a una capability. Las 5
/// capabilities conocidas tienen una sección clara en el envelope.
fn collection_for<'a>(name: &str, env: &'a EvidenceExport) -> Vec<&'a str> {
    // Para cada capability, devolvemos un Vec con un item por entry
    // (para contar). Esto evita devolver referencias que dependen
    // del lifetime de tipos concretos.
    if name == CAP_ENTITIES {
        env.entities.iter().map(|e| e.id.as_str()).collect()
    } else if name == CAP_FACTS {
        env.facts.iter().map(|f| f.id.as_str()).collect()
    } else if name == CAP_RELATIONS {
        env.relations.iter().map(|r| r.from.as_str()).collect()
    } else if name == CAP_SIGNALS {
        env.signals.iter().map(|s| s.id.as_str()).collect()
    } else if name == CAP_ARCHITECTURE {
        // architecture no tiene colección directa; para el test de
        // Complete, devolvemos un Vec vacío que el assert interpreta
        // como "no Complete sin colección". Si en el futuro se
        // implementa architecture, ajustar.
        Vec::new()
    } else {
        panic!("capability desconocida en test: {name}")
    }
}

// ----- 6. Reference consistency -------------------------------------------

#[test]
fn evidence_references_resolve_to_existing_paths() {
    let (_tmp, info) = fixture_workspace(5, true);
    let producer = fixture_producer();

    let caps = ExtractionCapabilities {
        requested: vec![CAP_ENTITIES.into(), CAP_FACTS.into()],
    };
    let env = extract_from_workspace(&info, &caps, &producer).expect("extract");

    // (a) Cada Fact.source_anchor_ref resuelve a un SourceAnchor.file en el envelope.
    let anchor_files: std::collections::BTreeSet<&str> =
        env.source_anchors.iter().map(|a| a.file.as_str()).collect();
    for fact in &env.facts {
        if let Some(anchor_ref) = &fact.source_anchor_ref {
            let path = anchor_ref.strip_prefix("anchor:").unwrap_or(anchor_ref);
            assert!(
                anchor_files.contains(path),
                "fact {} source_anchor_ref={} no resuelve a un SourceAnchor del envelope (anchors={:?})",
                fact.id,
                anchor_ref,
                env.source_anchors
                    .iter()
                    .map(|a| &a.file)
                    .collect::<Vec<_>>()
            );
        }
    }

    // (b) Cada SourceAnchor.file es un path real bajo el workspace al tiempo
    //     de emisión.
    let root = &info.root;
    for anchor in &env.source_anchors {
        let p = PathBuf::from(&anchor.file);
        // El anchor file puede ser absoluto (canonicalizado) o relativo al root.
        let exists_under_root = if p.is_absolute() {
            p.starts_with(root) && p.exists()
        } else {
            root.join(&p).exists()
        };
        assert!(
            exists_under_root,
            "SourceAnchor.file {} no es un path real bajo el workspace {}",
            anchor.file,
            root.display()
        );
    }
}

// ----- Bonus: deterministic across runs -----------------------------------

#[test]
fn producer_is_deterministic_across_runs() {
    let (_tmp, info) = fixture_workspace(3, false);
    let producer = fixture_producer();
    let caps = ExtractionCapabilities {
        requested: vec![CAP_ENTITIES.into(), CAP_FACTS.into()],
    };

    let env_a = extract_from_workspace(&info, &caps, &producer).expect("extract a");
    let env_b = extract_from_workspace(&info, &caps, &producer).expect("extract b");

    // Digest declarado debe coincidir (mismo DTO → mismo SHA-256).
    let bytes_a = encode_to_cbor_with_digest(&env_a).expect("encode a");
    let bytes_b = encode_to_cbor_with_digest(&env_b).expect("encode b");
    assert_eq!(
        bytes_a, bytes_b,
        "el producer debe producir bytes idénticos sobre el mismo input"
    );
}
