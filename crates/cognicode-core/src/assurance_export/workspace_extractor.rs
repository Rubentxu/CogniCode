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
    KIND_EVIDENCE_EXPORT, MEDIA_TYPE_CBOR, ManifestSection, ProducerInfo, Provenance, Relation,
    Signal, SourceAnchor, SubjectRef,
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
pub fn extract_from_workspace(
    info: &WorkspaceInfo,
    caps: &ExtractionCapabilities,
    producer: &ProducerInfo,
) -> Result<EvidenceExport, SerializeError> {
    let mut entities: Vec<Entity> = Vec::new();
    let mut facts: Vec<Fact> = Vec::new();
    let relations: Vec<Relation> = Vec::new();
    let signals: Vec<Signal> = Vec::new();
    let mut source_anchors: Vec<SourceAnchor> = Vec::new();
    let mut capability_completeness: BTreeMap<String, CapabilityCompleteness> = BTreeMap::new();
    let mut gaps: Vec<CapabilityGap> = Vec::new();

    for cap in &caps.requested {
        match cap.as_str() {
            CAP_ENTITIES => match collect_entities(&info.root) {
                Ok(e) => {
                    entities = e;
                    capability_completeness
                        .insert(cap.to_string(), CapabilityCompleteness::Complete);
                }
                Err(reason) => {
                    capability_completeness.insert(
                        cap.to_string(),
                        CapabilityCompleteness::unsupported(reason.clone()),
                    );
                    gaps.push(CapabilityGap {
                        capability: cap.to_string(),
                        reason: format!("UnsupportedProduced({reason})"),
                        detail: None,
                    });
                }
            },
            CAP_FACTS => match collect_facts(&info.root, &entities) {
                Ok(f) => {
                    facts = f;
                    source_anchors = collect_source_anchors(&info.root, &facts);
                    capability_completeness
                        .insert(cap.to_string(), CapabilityCompleteness::Complete);
                }
                Err(reason) => {
                    capability_completeness.insert(
                        cap.to_string(),
                        CapabilityCompleteness::unsupported(reason.clone()),
                    );
                    gaps.push(CapabilityGap {
                        capability: cap.to_string(),
                        reason: format!("UnsupportedProduced({reason})"),
                        detail: None,
                    });
                }
            },
            CAP_RELATIONS => {
                capability_completeness.insert(
                    cap.to_string(),
                    CapabilityCompleteness::unsupported(
                        "Tarjan SCC no se calcula en v1 (no reimplementa analisis)",
                    ),
                );
                gaps.push(CapabilityGap {
                    capability: cap.to_string(),
                    reason: "UnsupportedProduced(Tarjan SCC no se calcula en v1)".into(),
                    detail: Some(
                        "COGNICODE_WORKSTREAM C1: el export NO reimplementa Tarjan; \
                         la capability estara disponible cuando CogniCode exponga el \
                         grafo de dependencias como puerto reutilizable."
                            .into(),
                    ),
                });
            }
            CAP_SIGNALS => {
                capability_completeness.insert(
                    cap.to_string(),
                    CapabilityCompleteness::unsupported(
                        "SOLID heuristic scoring no se exporta en v1",
                    ),
                );
                gaps.push(CapabilityGap {
                    capability: cap.to_string(),
                    reason: "UnsupportedProduced(heuristic scoring no implementado)".into(),
                    detail: Some(
                        "COGNICODE_WORKSTREAM C5: heuristic signals iran como Signal \
                         con algorithmId/version/config; no se transforma en Fact."
                            .into(),
                    ),
                });
            }
            CAP_ARCHITECTURE => {
                capability_completeness.insert(
                    cap.to_string(),
                    CapabilityCompleteness::unsupported(
                        "Architecture constraints no se exportan en v1",
                    ),
                );
                gaps.push(CapabilityGap {
                    capability: cap.to_string(),
                    reason: "UnsupportedProduced(architecture constraints no expuestas)".into(),
                    detail: Some(
                        "COGNICODE_WORKSTREAM C6: las ArchitectureConstraint admitidas \
                         pueden exportarse como evidence/metadata pero no imponen verdict; \
                         la capability estara disponible cuando se exponga el puerto."
                            .into(),
                    ),
                });
            }
            other => {
                capability_completeness.insert(
                    other.to_string(),
                    CapabilityCompleteness::unsupported(format!(
                        "capability desconocida por v1: {other}"
                    )),
                );
                gaps.push(CapabilityGap {
                    capability: other.to_string(),
                    reason: format!("UnsupportedProduced(capability desconocida: {other})"),
                    detail: None,
                });
            }
        }
    }

    let manifest = build_manifest(producer, &caps.requested, &capability_completeness);
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

/// Hechos deterministas: un fact `exists_at_path` por cada entidad de
/// archivo, con la autoridad `DeterministicAnalyzer`. El fact usa
/// `sourceAnchorRef` para apuntar al anchor que se crea abajo.
fn collect_facts(root: &Path, entities: &[Entity]) -> Result<Vec<Fact>, String> {
    let mut out = Vec::new();
    let mut seen_anchors: BTreeMap<String, String> = BTreeMap::new();
    let mut anchor_seq = 0u64;
    for entity in entities {
        // Resolver el path real del file desde el id del entity.
        let path = entity.id.strip_prefix("file://").unwrap_or(&entity.id);
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => return Err(format!("metadata de {path} fallo: {e}")),
        };
        let anchor_id = format!("anchor:{}", path);
        if !seen_anchors.contains_key(&anchor_id) {
            anchor_seq += 1;
            seen_anchors.insert(anchor_id.clone(), format!("{}", anchor_seq));
        }
        out.push(Fact {
            id: format!("fact:exists:{}", entity.id),
            entity_ref: entity.id.clone(),
            predicate: "exists_at_path".into(),
            object_value: Some(path.to_string()),
            authority: "DeterministicAnalyzer".into(),
            source_anchor_ref: Some(anchor_id),
        });
        let _ = metadata;
    }
    let _ = root;
    Ok(out)
}

/// Genera los source anchors correspondientes a los facts. Para v1
/// cada anchor es solo el path + línea 0 (no se calcula offset preciso
/// para no reimplementar tree-sitter).
fn collect_source_anchors(_root: &Path, _facts: &[Fact]) -> Vec<SourceAnchor> {
    // SourceAnchor por entity_ref único (path). Line 0 = "archivo
    // existe"; precisión adicional vendria con tree-sitter parse.
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    let mut anchors = Vec::new();
    for fact in _facts {
        if let Some(anchor_ref) = &fact.source_anchor_ref
            && seen.insert(anchor_ref.clone(), ()).is_none()
        {
            let path = anchor_ref.strip_prefix("anchor:").unwrap_or(anchor_ref);
            anchors.push(SourceAnchor {
                file: path.to_string(),
                line: 0,
                column: None,
                symbol_ref: None,
            });
        }
    }
    anchors
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
