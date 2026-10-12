//! Extracción de evidence desde un workspace de CogniCode para el envelope
//! `assurance-evidence/v1`.
//!
//! Regla dura (COGNICODE_WORKSTREAM §C1): el export NO reimplementa
//! análisis existente. `extract_from_workspace` consume lo que CogniCode
//! ya calculó (grafo, facts, símbolos) y lo serializa en la forma que
//! el consumer (pipelinek-assurance) espera.
//!
//! Para v1 (R1 de pipelinek-assurance), este módulo implementa el camino
//! mínimo viable: walk del filesystem + extracción de entidades y facts
//! de lo que ya está materializado en el workspace. Análisis más rico
//! (Tarjan SCC, dependency graph, signals SOLID) se enchufa en versiones
//! siguientes sin cambiar el shape del envelope.
//!
//! Capabilities soportadas en v1:
//! - `entities`: lista los archivos fuente del workspace como entidades
//!   (`Entity { id, kind="file", name }`).
//! - `facts`: produce un fact por archivo con la autoridad
//!   `DeterministicAnalyzer` y el predicate `exists_at_path`.
//! - `architecture`: marcado Unsupported en v1 (no se calcula SCC ni
//!   constraints; lo hara una version posterior sin cambiar el envelope).
//!
//! Capabilities declaradas como Unsupported devuelven el envelope con
//! `capabilityCompleteness["architecture"] = Unsupported { reason: "..." }`,
//! exit 0 y un gap honesto en `gaps[]`. No se devuelve `items: []` para
//! esa capability (regla C4 del workstream).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::envelope::{
    API_VERSION, CapabilityCompleteness, CapabilityGap, Entity, EvidenceExport, Fact,
    Finding, KIND_EVIDENCE_EXPORT, MAX_COLLECTION_SIZE, MEDIA_TYPE_CBOR, ManifestSection,
    Observation, ProducerInfo, Provenance, Relation, Signal, SourceAnchor, SubjectRef,
};
use super::serialize::{SerializeError, encode_to_cbor_with_digest};

/// Capabilities que el export puede satisfacer en v1. Las capacidades no
/// listadas se marcan `Unsupported` con motivo, no se omiten en silencio.
pub const CAP_ENTITIES: &str = "entities";
pub const CAP_FACTS: &str = "facts";
pub const CAP_RELATIONS: &str = "relations";
pub const CAP_SIGNALS: &str = "signals";
pub const CAP_ARCHITECTURE: &str = "architecture";

/// Lista canónica de capabilities conocidas por este exporter v1. El
/// consumer (`PROVIDER_SPI.md`) espera esta forma exacta en el campo
/// `evidenceCapabilities` del descriptor del provider.
pub fn known_capabilities() -> &'static [&'static str] {
    &[
        CAP_ENTITIES,
        CAP_FACTS,
        CAP_RELATIONS,
        CAP_SIGNALS,
        CAP_ARCHITECTURE,
    ]
}

/// Información mínima sobre el workspace que el envelope necesita.
/// `revision` se calcula como `git rev-parse HEAD`; si no se puede
/// obtener (no es repo git), se cae al nombre del directorio.
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub root: PathBuf,
    pub revision: String,
    pub kind: String,
}

impl WorkspaceInfo {
    /// Detecta la información del workspace. `kind` se infiere mirando el
    /// filesystem: `rust-workspace` si hay `Cargo.toml` en la raíz, sino
    /// `unknown`.
    ///
    /// El `root` se canonicaliza a un path absoluto: el M-D01 producer
    /// contract exige que `--workspace .` y `--workspace /abs/path`
    /// produzcan el mismo SHA, así que la forma del path nunca se filtra
    /// al envelope (todos los entity `id` son absolutos).
    pub fn detect(root: PathBuf) -> Self {
        let root = root.canonicalize().unwrap_or(root);
        let revision = git_rev(&root).unwrap_or_else(|| {
            root.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".to_string())
        });
        let kind = if root.join("Cargo.toml").is_file() {
            "rust-workspace".to_string()
        } else {
            "unknown".to_string()
        };
        Self {
            root,
            revision,
            kind,
        }
    }
}

fn git_rev(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["-C", &root.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    Some(s.trim().to_string())
}

/// Selección de capabilities a producir. Cada capability no listada se
/// marca `Unsupported` con motivo y se reporta en `gaps[]`.
#[derive(Debug, Clone)]
pub struct ExtractionCapabilities {
    pub requested: Vec<String>,
}

impl ExtractionCapabilities {
    pub fn from_flags(flags: &[String]) -> Self {
        // Si el usuario no pasa ninguna capability, se piden todas las
        // conocidas (modo "produce lo que puedas"). Esto evita el caso
        // trivial de "pedi nada, recibo nada" que el workstream considera
        // gap deshonesto.
        let requested: Vec<String> = if flags.is_empty() {
            known_capabilities().iter().map(|s| s.to_string()).collect()
        } else {
            flags.to_vec()
        };
        Self { requested }
    }
}

/// Extrae el envelope desde un workspace.
///
/// Reglas duras (COGNICODE_WORKSTREAM):
/// - Capabilities no soportadas se marcan `Unsupported` con motivo. Nunca
///   se devuelve `items: []` para una capability Partial o Unknown.
/// - El envelope declara y verifica su digest al final (hecho por
///   `encode_to_cbor_with_digest`).
/// - Capabilities parciales se marcan con `Partial { gaps: [...] }` y
///   `reason = PartialProduced(coveredFraction)` (string semánticamente
///   honesto; el consumer parsea contra `EvidenceGap.GapReason`).
///
/// B2.W4 — invariantes adicionales que este extractor pinea:
///
/// - **Single extraction**: el walk del workspace se hace una sola vez,
///   antes de procesar las capabilities. El orden de `--capability` ya no
///   determina si el walk ocurre antes o después del procesamiento de
///   cada capability.
/// - **Order independence**: las capabilities se canonicalizan (dedup +
///   sort por nombre) antes de procesarlas, así el envelope es byte-
///   estable para el mismo conjunto de capabilities en cualquier orden.
/// - **Symmetric limits**: `entities` y `facts` se truncan a
///   `MAX_COLLECTION_SIZE` con un gap honesto (`Partial`), igual que el
///   consumer rechaza al mismo número.
/// - **Honest coverage**: una capability marcada `Complete` exige
///   colección no-vacía; si el walk tuvo éxito pero no encontró
///   archivos, se marca `Partial` en vez de mentir con `Complete`.
/// - **Reference consistency**: `facts` se deriva de las `entities` que
///   van en el envelope (post-truncación), así cada `fact.entity_ref`
///   resuelve a un `entity` presente y cada `fact.source_anchor_ref`
///   resuelve a un `SourceAnchor` generado en el mismo pass.
pub fn extract_from_workspace(
    info: &WorkspaceInfo,
    caps: &ExtractionCapabilities,
    producer: &ProducerInfo,
) -> Result<EvidenceExport, SerializeError> {
    // B2.W4 — single extraction. Sólo caminamos el workspace si una
    // capability que lo necesita fue solicitada: entities o facts. El
    // resto son Unsupported estáticas y no requieren filesystem.
    let needs_walk = caps
        .requested
        .iter()
        .any(|c| matches!(c.as_str(), CAP_ENTITIES | CAP_FACTS));

    let mut walked_entities: Vec<Entity> = Vec::new();
    let mut walk_failure: Option<String> = None;
    if needs_walk {
        match collect_entities(&info.root) {
            Ok(e) => walked_entities = e,
            Err(reason) => walk_failure = Some(reason),
        }
    }

    // B2.W4 — order independence. Canonicalizamos (dedup + sort) las
    // capabilities pedidas antes de procesarlas. Así el orden del
    // usuario no afecta al envelope: dos pedidos con el mismo conjunto
    // producen el mismo envelope byte-a-byte.
    let mut canonical: Vec<&str> = caps.requested.iter().map(String::as_str).collect();
    canonical.sort_unstable();
    canonical.dedup();

    let mut entities: Vec<Entity> = Vec::new();
    let mut facts: Vec<Fact> = Vec::new();
    let relations: Vec<Relation> = Vec::new();
    let signals: Vec<Signal> = Vec::new();
    let mut source_anchors: Vec<SourceAnchor> = Vec::new();
    let mut capability_completeness: BTreeMap<String, CapabilityCompleteness> = BTreeMap::new();
    let mut gaps: Vec<CapabilityGap> = Vec::new();

    for cap in canonical.iter().copied() {
        match cap {
            CAP_ENTITIES => {
                if let Some(reason) = &walk_failure {
                    record_walk_unsupported(cap, reason, &mut capability_completeness, &mut gaps);
                } else {
                    let (truncated, gap) = truncate_entities_to_limit(walked_entities.clone());
                    entities = truncated;
                    match gap {
                        Some(g) => {
                            capability_completeness.insert(
                                cap.to_string(),
                                CapabilityCompleteness::partial(vec![g.clone()]),
                            );
                            gaps.push(g);
                        }
                        None => {
                            // B2.W4 — honest coverage. Walk OK pero 0
                            // entidades: marcamos Partial con gap
                            // honesto en vez de mentir con Complete.
                            if entities.is_empty() {
                                let g = CapabilityGap {
                                    capability: cap.to_string(),
                                    reason:
                                        "PartialProduced(workspace contains no Rust source files)"
                                            .into(),
                                    detail: Some(
                                        "Solo se exportan archivos .rs en v1; \
                                         otros lenguajes se anaden en versiones \
                                         futuras sin cambiar el shape del envelope."
                                            .into(),
                                    ),
                                };
                                capability_completeness.insert(
                                    cap.to_string(),
                                    CapabilityCompleteness::partial(vec![g.clone()]),
                                );
                                gaps.push(g);
                            } else {
                                capability_completeness
                                    .insert(cap.to_string(), CapabilityCompleteness::Complete);
                            }
                        }
                    }
                }
            }
            CAP_FACTS => {
                if let Some(reason) = &walk_failure {
                    record_walk_unsupported(cap, reason, &mut capability_completeness, &mut gaps);
                } else {
                    // facts se deriva de `entities` (el envelope field,
                    // post-truncación). Esto pinea reference consistency:
                    // cada fact.entity_ref resuelve a un entity presente
                    // y cada fact.source_anchor_ref resuelve a un anchor
                    // generado en el mismo pass.
                    let (raw_facts, raw_anchors) = collect_facts_and_anchors(&entities);
                    let (truncated_facts, gap) = truncate_facts_to_limit(raw_facts);
                    facts = truncated_facts;
                    source_anchors = truncate_anchors_to_limit(raw_anchors, facts.len());
                    match gap {
                        Some(g) => {
                            capability_completeness.insert(
                                cap.to_string(),
                                CapabilityCompleteness::partial(vec![g.clone()]),
                            );
                            gaps.push(g);
                        }
                        None => {
                            if facts.is_empty() {
                                let g = CapabilityGap {
                                    capability: cap.to_string(),
                                    reason: "PartialProduced(no entities para derivar facts)"
                                        .into(),
                                    detail: Some(
                                        "facts se deriva de entities; si entities \
                                         no fue solicitada o esta vacia, no hay \
                                         facts que producir."
                                            .into(),
                                    ),
                                };
                                capability_completeness.insert(
                                    cap.to_string(),
                                    CapabilityCompleteness::partial(vec![g.clone()]),
                                );
                                gaps.push(g);
                            } else {
                                capability_completeness
                                    .insert(cap.to_string(), CapabilityCompleteness::Complete);
                            }
                        }
                    }
                }
            }
            CAP_RELATIONS => record_static_unsupported(
                cap,
                "Tarjan SCC no se calcula en v1 (no reimplementa analisis)",
                "UnsupportedProduced(Tarjan SCC no se calcula en v1)",
                "COGNICODE_WORKSTREAM C1: el export NO reimplementa Tarjan; \
                 la capability estara disponible cuando CogniCode exponga el \
                 grafo de dependencias como puerto reutilizable.",
                &mut capability_completeness,
                &mut gaps,
            ),
            CAP_SIGNALS => record_static_unsupported(
                cap,
                "SOLID heuristic scoring no se exporta en v1",
                "UnsupportedProduced(heuristic scoring no implementado)",
                "COGNICODE_WORKSTREAM C5: heuristic signals iran como Signal \
                 con algorithmId/version/config; no se transforma en Fact.",
                &mut capability_completeness,
                &mut gaps,
            ),
            CAP_ARCHITECTURE => record_static_unsupported(
                cap,
                "Architecture constraints no se exportan en v1",
                "UnsupportedProduced(architecture constraints no expuestas)",
                "COGNICODE_WORKSTREAM C6: las ArchitectureConstraint admitidas \
                 pueden exportarse como evidence/metadata pero no imponen verdict; \
                 la capability estara disponible cuando se exponga el puerto.",
                &mut capability_completeness,
                &mut gaps,
            ),
            other => record_walk_unsupported(
                other,
                &format!("capability desconocida por v1: {other}"),
                &mut capability_completeness,
                &mut gaps,
            ),
        }
    }

    let manifest = build_manifest(
        producer,
        &canonical_strings(&canonical),
        &capability_completeness,
    );
    let provenance = Provenance {
        producer_id: producer.id.clone(),
        producer_version: producer.version.clone(),
        subject_revision: info.revision.clone(),
        capability: "assurance-export".into(),
        artifact_ref: None,
        artifact_digest: None,
    };

    Ok(EvidenceExport {
        api_version: API_VERSION.to_string(),
        kind: KIND_EVIDENCE_EXPORT.to_string(),
        producer: producer.clone(),
        subject: SubjectRef {
            revision: info.revision.clone(),
            kind: info.kind.clone(),
        },
        manifest,
        entities,
        facts,
        relations,
        signals,
        source_anchors,
        provenance,
        capability_completeness,
        gaps,
        digest: String::new(), // se asigna en encode_to_cbor_with_digest
    })
}

// ----- extraction helpers (B2.W4) ----------------------------------------

/// Registra una capability como `Unsupported` con motivo derivado del
/// walk failure (o de un cap desconocido). El gap lleva el prefijo
/// `UnsupportedProduced(...)` que el consumer parsea.
fn record_walk_unsupported(
    cap: &str,
    reason: &str,
    completeness: &mut BTreeMap<String, CapabilityCompleteness>,
    gaps: &mut Vec<CapabilityGap>,
) {
    completeness.insert(cap.to_string(), CapabilityCompleteness::unsupported(reason));
    gaps.push(CapabilityGap {
        capability: cap.to_string(),
        reason: format!("UnsupportedProduced({reason})"),
        detail: None,
    });
}

/// Variante con `reason` y `detail` ya prefijados para las
/// capabilities estáticas (relations, signals, architecture). Mantiene
/// la paridad exacta con el formato que el consumer espera
/// (`UnsupportedProduced(<motivo canon>)` + `detail` con cita al
/// workstream).
fn record_static_unsupported(
    cap: &str,
    reason: &str,
    gap_reason: &str,
    detail: &str,
    completeness: &mut BTreeMap<String, CapabilityCompleteness>,
    gaps: &mut Vec<CapabilityGap>,
) {
    completeness.insert(cap.to_string(), CapabilityCompleteness::unsupported(reason));
    gaps.push(CapabilityGap {
        capability: cap.to_string(),
        reason: gap_reason.into(),
        detail: Some(detail.into()),
    });
}

/// Trunca `entities` a `MAX_COLLECTION_SIZE`. Devuelve la colección
/// truncada y, si truncó, un `CapabilityGap` honesto con motivo
/// `PartialProduced(N truncadas a M)`. Constante compartida con el
/// consumer (`MAX_COLLECTION_SIZE`) — invariante 3 (symmetric limits).
fn truncate_entities_to_limit(entities: Vec<Entity>) -> (Vec<Entity>, Option<CapabilityGap>) {
    if entities.len() > MAX_COLLECTION_SIZE {
        let gap = CapabilityGap {
            capability: CAP_ENTITIES.into(),
            reason: format!(
                "PartialProduced({} entities truncadas a {})",
                entities.len(),
                MAX_COLLECTION_SIZE
            ),
            detail: Some(
                "AAT-B2-01: el envelope no excede MAX_COLLECTION_SIZE del \
                 consumer; el resto se omite. Para procesar más entidades, \
                 divide el workspace en chunks más pequenos."
                    .into(),
            ),
        };
        let truncated = entities.into_iter().take(MAX_COLLECTION_SIZE).collect();
        (truncated, Some(gap))
    } else {
        (entities, None)
    }
}

/// Trunca `facts` a `MAX_COLLECTION_SIZE`, mismo contrato que
/// `truncate_entities_to_limit`. Los anchors derivados se truncan en
/// `truncate_anchors_to_limit` para mantener la paridad fact↔anchor.
fn truncate_facts_to_limit(facts: Vec<Fact>) -> (Vec<Fact>, Option<CapabilityGap>) {
    if facts.len() > MAX_COLLECTION_SIZE {
        let gap = CapabilityGap {
            capability: CAP_FACTS.into(),
            reason: format!(
                "PartialProduced({} facts truncados a {})",
                facts.len(),
                MAX_COLLECTION_SIZE
            ),
            detail: Some(
                "AAT-B2-01: el envelope no excede MAX_COLLECTION_SIZE del \
                 consumer; el resto se omite."
                    .into(),
            ),
        };
        let truncated = facts.into_iter().take(MAX_COLLECTION_SIZE).collect();
        (truncated, Some(gap))
    } else {
        (facts, None)
    }
}

/// Trunca `anchors` al `limit` dado. Usado por el path de facts para
/// mantener 1:1 la cardinalidad fact↔anchor después de truncar facts.
fn truncate_anchors_to_limit(anchors: Vec<SourceAnchor>, limit: usize) -> Vec<SourceAnchor> {
    anchors.into_iter().take(limit).collect()
}

/// Genera facts y source anchors en un solo pass por `entities`. Cada
/// entity produce exactamente un fact (`exists_at_path`) y un anchor
/// (file + line 0). Sin dedup: el walk ya garantiza entidades únicas.
///
/// Sustituye a las funciones separadas `collect_facts` y
/// `collect_source_anchors` que duplicaban el recorrido sobre la misma
/// colección (single extraction).
fn collect_facts_and_anchors(entities: &[Entity]) -> (Vec<Fact>, Vec<SourceAnchor>) {
    let mut facts = Vec::with_capacity(entities.len());
    let mut anchors = Vec::with_capacity(entities.len());
    for entity in entities {
        let path = entity.id.strip_prefix("file://").unwrap_or(&entity.id);
        let anchor_id = format!("anchor:{path}");
        facts.push(Fact {
            id: format!("fact:exists:{}", entity.id),
            entity_ref: entity.id.clone(),
            predicate: "exists_at_path".into(),
            object_value: Some(path.to_string()),
            authority: "DeterministicAnalyzer".into(),
            source_anchor_ref: Some(anchor_id),
        });
        anchors.push(SourceAnchor {
            file: path.to_string(),
            line: 0,
            column: None,
            symbol_ref: None,
        });
    }
    (facts, anchors)
}

/// Adapta la lista canónica de `&str` a `Vec<String>` para el campo
/// `requested_capabilities` del manifest. Evita alocaciones duplicadas
/// en el path caliente (la canonicalización ya ocurrió arriba).
fn canonical_strings(canonical: &[&str]) -> Vec<String> {
    canonical.iter().map(|s| (*s).to_string()).collect()
}

// ----- collectors ---------------------------------------------------------

/// Recorre el workspace y devuelve una entidad por archivo fuente Rust
/// encontrado. `id` es el path absoluto normalizado; `kind = "file"`.
/// Solo se cuentan `.rs` por ahora; los Cargo.toml, README, etc. salen
/// como entidades de su `kind` correspondiente cuando aplique.
fn collect_entities(root: &Path) -> Result<Vec<Entity>, String> {
    let mut out = Vec::new();
    walk_rust_files(root, &mut |path| {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        out.push(Entity {
            id: format!("file://{}", path.to_string_lossy()),
            kind: "file".into(),
            name: rel,
            layer: detect_layer(root, path),
        });
    })
    .map_err(|e| format!("filesystem walk fallo: {e}"))?;
    Ok(out)
}

/// Detecta la capa arquitectónica del path relativo a la raíz.
/// Versión v1: si el path relativo empieza por `crates/`, devuelve la
/// primera componente (e.g. `cognicode-core`). Si empieza por `apps/`,
/// idem. Si no, `None` (no es un módulo con capa).
fn detect_layer(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut comps = rel.components();
    let first = comps.next()?.as_os_str().to_string_lossy().into_owned();
    let second = comps.next()?.as_os_str().to_string_lossy().into_owned();
    if first == "crates" || first == "apps" {
        Some(second)
    } else {
        None
    }
}

fn walk_rust_files(root: &Path, cb: &mut dyn FnMut(&Path)) -> std::io::Result<()> {
    // `read_dir` does not guarantee any ordering. The M-D01 producer
    // contract (this exporter's digest is computed over the envelope and
    // compared by the consumer) requires byte-stable output for the
    // same input, so we sort entries by path before iterating.
    let mut entries: Vec<_> = std::fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            // No recursar en .git, target, node_modules, .venv, dist.
            if matches!(
                s.as_ref(),
                ".git" | "target" | "node_modules" | ".venv" | "dist" | "build"
            ) {
                continue;
            }
            walk_rust_files(&path, cb)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            cb(&path);
        }
    }
    Ok(())
}

/// Construye la sección `manifest`. El digest del manifest es el SHA-256
/// del `manifest` re-codificado (sin el campo digest); el consumer lo
/// lee pero no lo verifica (es informativo).
fn build_manifest(
    producer: &ProducerInfo,
    requested: &[String],
    capability_completeness: &BTreeMap<String, CapabilityCompleteness>,
) -> ManifestSection {
    use sha2::{Digest as Sha2Digest, Sha256};
    let produced: Vec<String> = capability_completeness
        .iter()
        .filter(|(_, cc)| {
            matches!(
                cc,
                CapabilityCompleteness::Complete | CapabilityCompleteness::Partial { .. }
            )
        })
        .map(|(k, _)| k.clone())
        .collect();

    let mut placeholder = ManifestSection {
        requested_capabilities: requested.to_vec(),
        produced_capabilities: produced,
        completeness_by_capability: capability_completeness.clone(),
        schema_version: producer.schema_version.clone(),
        digest: String::new(),
    };
    let canonical_bytes = serde_json::to_vec(&placeholder).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&canonical_bytes);
    placeholder.digest = hex::encode(hasher.finalize());
    placeholder
}

// ----- high-level pipeline ------------------------------------------------

/// Construye el envelope final con digest declarado, lo serializa a CBOR
/// con el media type oficial y lo escribe en `path`. Convenience para
/// el CLI.
pub fn export_to_file(
    info: &WorkspaceInfo,
    caps: &ExtractionCapabilities,
    producer: &ProducerInfo,
    path: &Path,
) -> Result<(), SerializeError> {
    let envelope = extract_from_workspace(info, caps, producer)?;
    let bytes = encode_to_cbor_with_digest(&envelope)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            SerializeError::Bounded(format!(
                "no se pudo crear directorio padre {}: {e}",
                parent.display()
            ))
        })?;
    }
    std::fs::write(path, &bytes).map_err(|e| {
        SerializeError::Bounded(format!("escritura de {} fallo: {e}", path.display()))
    })?;
    Ok(())
}

/// Devuelve el media type que este exporter publica (para CLI/headers).
pub fn cbor_media_type() -> &'static str {
    MEDIA_TYPE_CBOR
}

// =============================================================================
// B4.W4 — Ports de assurance-export enriquecido (impact / architecture /
// quality findings). ADDITIVO sobre B2.W4: no rompe el shape del envelope
// vigente ni la golden CBOR. El bump real del wire (`observations` y
// `findings` como top-level fields) llega al release B4 con la versión
// bumped del workspace y la actualización del consumer en lockstep.
// =============================================================================

/// Accumulator para `Relation`s drenadas por `extract_impact_relations`.
/// Tipo alias para mantener la superficie simétrica con los extractores
/// B2.W4 (`Vec<Entity>` / `Vec<Fact>`). El parámetro de salida se pasa
/// por `&mut` para que el extractor pueda drenar al vuelo sin reasignar.
pub type RelationAccumulator = Vec<Relation>;

/// Accumulator para `Observation`s arquitectónicas (B4.W2). Misma forma.
pub type ObservationAccumulator = Vec<Observation>;

/// Accumulator para `Finding`s de calidad (B4.W3). Misma forma.
pub type FindingAccumulator = Vec<Finding>;

/// Resultado de un extractor B4.W4. Espejo a nivel de extractor del
/// `CapabilityCompleteness` del envelope: `Complete` cuando el
/// accumulator quedó exhaustivo, `Partial { gaps }` cuando la fuente
/// tenía más items que `max` y se truncó honestamente, `Unsupported {
/// capability_kind }` cuando la capability aún no tiene módulo que la
/// respalde en este commit (B4.W1/W2/W3 no fusionados todavía en la
/// rama de release).
///
/// `Unsupported` lleva siempre `capability_kind` no-vacío por contrato:
/// el caller lo usa para discriminar "todavía no expuesto" de "se
/// truncó", y un motivo vacío haría indistinguibles los dos casos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionOutcome {
    Complete,
    Partial { gaps: Vec<String> },
    Unsupported { capability_kind: &'static str },
}

impl ExtractionOutcome {
    /// Nombre estable del variante, paralelo a `CapabilityCompleteness::kind_name`.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial { .. } => "partial",
            Self::Unsupported { .. } => "unsupported",
        }
    }

    /// `true` si el extractor aportó items utilizables (Complete o
    /// Partial). Útil para que el caller decida si anexar a `relations`
    /// / `observations` / `findings` en el envelope.
    pub fn is_producing(&self) -> bool {
        matches!(self, Self::Complete | Self::Partial { .. })
    }
}

/// `capability_kind` estable para el extractor de relaciones de impacto
/// mientras B4.W1 (`crates/cognicode-core/src/impact/`) no está
/// fusionado en la rama de release. Es la cadena que verá el consumer
/// (vía `ExtractionOutcome::Unsupported` propagado por la capa de
/// caller, mapeada después a un `CapabilityGap` con razón
/// `UnsupportedProduced(<esto>)` al integrarse).
pub const CAPABILITY_KIND_IMPACT_RELATIONS: &str = "impact-relations:envelope-v2";

/// `capability_kind` estable para el extractor de observaciones de
/// arquitectura mientras B4.W2 (`architecture::diagnostic`) no está
/// fusionado en la rama de release.
pub const CAPABILITY_KIND_ARCHITECTURE_OBSERVATIONS: &str = "architecture-observations:envelope-v2";

/// `capability_kind` estable para el extractor de hallazgos de calidad
/// mientras B4.W3 (`findings::*`) no está fusionado en la rama de
/// release.
pub const CAPABILITY_KIND_QUALITY_FINDINGS: &str = "quality-findings:envelope-v2";

/// Drena `ImpactEdge`s (B4.W1) al accumulator, honrando el límite
/// `max` (simétrico a `MAX_COLLECTION_SIZE = 1024`).
///
/// **Estado B4.W4 (este commit)**: B4.W1 no está fusionado en la rama
/// de release, así que el extractor no encuentra datos. Devuelve
/// `Unsupported` con `capability_kind = CAPABILITY_KIND_IMPACT_RELATIONS`
/// y deja el accumulator intacto. Si el caller pre-pobló el accumulator
/// con más de `max` items (escenario de integración cuando B4.W1
/// aterrice y exporte un set grande), se trunca al límite y se
/// devuelve `Partial` con el gap honesto. Esa rama es la que B2.W4
/// pineó como "symmetric limits".
///
/// Cuando B4.W1 se fusione, este extractor se reescribe para:
/// 1. Iterar `crates/cognicode-core::impact::iter_impact_edges()`.
/// 2. Mapear cada `ImpactEdge` a `Relation { from, to, kind, evidence }`.
/// 3. Misma regla de truncado.
///
/// El `capability_kind` permanece estable para que el consumer pueda
/// identificar el motivo de `Unsupported` byte-a-byte sin re-codificar
/// la lógica del parser.
pub fn extract_impact_relations(
    out: &mut RelationAccumulator,
    max: usize,
) -> ExtractionOutcome {
    if out.len() > max {
        out.truncate(max);
        return ExtractionOutcome::Partial {
            gaps: vec![format!(
                "ImpactAccumulator truncado a max={max} (simetrico a \
                 MAX_COLLECTION_SIZE={MAX_COLLECTION_SIZE})"
            )],
        };
    }
    ExtractionOutcome::Unsupported {
        capability_kind: CAPABILITY_KIND_IMPACT_RELATIONS,
    }
}

/// Drena `ArchitectureObservation`s (B4.W2) al accumulator, honrando
/// `max`. Misma simetría que `extract_impact_relations`. B4.W2 todavía
/// no está fusionado en la rama de release, así que devuelve
/// `Unsupported` hasta el release.
pub fn extract_architecture_observations(
    out: &mut ObservationAccumulator,
    max: usize,
) -> ExtractionOutcome {
    if out.len() > max {
        out.truncate(max);
        return ExtractionOutcome::Partial {
            gaps: vec![format!(
                "ObservationAccumulator truncado a max={max} (simetrico a \
                 MAX_COLLECTION_SIZE={MAX_COLLECTION_SIZE})"
            )],
        };
    }
    ExtractionOutcome::Unsupported {
        capability_kind: CAPABILITY_KIND_ARCHITECTURE_OBSERVATIONS,
    }
}

/// Drena `Finding`s de calidad (B4.W3) al accumulator, honrando `max`.
/// Misma simetría que los otros dos extractores. B4.W3 todavía no está
/// fusionado en la rama de release, así que devuelve `Unsupported`
/// hasta el release.
pub fn extract_quality_findings(
    out: &mut FindingAccumulator,
    max: usize,
) -> ExtractionOutcome {
    if out.len() > max {
        out.truncate(max);
        return ExtractionOutcome::Partial {
            gaps: vec![format!(
                "FindingAccumulator truncado a max={max} (simetrico a \
                 MAX_COLLECTION_SIZE={MAX_COLLECTION_SIZE})"
            )],
        };
    }
    ExtractionOutcome::Unsupported {
        capability_kind: CAPABILITY_KIND_QUALITY_FINDINGS,
    }
}
