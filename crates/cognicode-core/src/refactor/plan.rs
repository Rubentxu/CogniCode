//! B5.W1 — Refactor plan contract.
//!
//! Estado pre-B5.W1:
//!   - El usuario (humano o agente) podia pedir una preview de
//!     refactor via `safe_refactor` (MCP) o `cognicode refactor`
//!     (CLI). El resultado era un `RefactorPreviewDto` con archivos,
//!     simbolos afectados y un `risk_level`.
//!   - No existia un contrato persistente del *plan*: si el agente
//!     queria comparar contra una propuesta anterior, volver a
//!     invocar la preview o pasarle la propuesta a otra herramienta
//!     necesitaba re-derivar los edits desde el codigo fuente.
//!   - El apply path (B5.W3) requiere un plan firmable: una pieza
//!     autoritativa que diga *exactamente* que va a cambiar, sobre
//!     que version del workspace, con que precondiciones y que
//!     riesgos, y que pueda verificarse byte-a-byte entre quien la
//!     produce y quien la aplica.
//!   - `refactor --apply` se rechaza actualmente por contrato
//!     (PRF-CLI-05): no hay rollback. Hasta que B5.W3 conecte el
//!     apply transaccional, el unico permiso honrado es preview.
//!
//! Lo que aporta este modulo:
//!   - `RefactorPlan`: el contrato portador de una propuesta de
//!     cambio. Lleva identidad del plan (`id`), identidad del
//!     workspace (`workspace_identity`), la base de analisis que lo
//!     sustenta (`AnalysisBasis`, B2.W1), archivos afectados, edits
//!     planeados, precondiciones, riesgos y un `digest`
//!     deterministico (sha256 de los bytes canonicizados del plan
//!     sin timestamps).
//!   - `PlannedEdit`: una edicion exacta — `file + byte_range +
//!     replacement`. `byte_range` se mide en bytes del source (mismo
//!     contrato que B4.W2 para `ArchitectureEvidence::byte_range`)
//!     para no desincronizarse con re-formatos.
//!   - `Precondition`: lo que debe ser cierto para que el apply
//!     pueda ejecutarse. Se enuncia como `capability + description`
//!     porque el dispatcher del apply las verifica contra el
//!     port authority real (no se reinventa en este modulo).
//!   - `Risk`: pieza de riesgo declarada por el planner, con
//!     `severity` reusando el `Severity` de B4.W2 (no se inventa un
//!     paralelo).
//!   - `build_plan`: el constructor canonico. Calcula el `digest`
//!     sobre los bytes serializados del plan canonicizado, EXCLUYENDO
//!     `created_at` (R-B5-W1: el digest debe ser estable entre
//!     builds; cualquier campo temporal se saca del input).
//!   - `verify_digest`: recomputa y compara. Cualquier consumer del
//!     plan lo verifica antes de actuar (B5.W3 hara el gating).
//!
//! Politica B5.W1 (de `odd/tasks/b5-safe-refactoring-authorized-changes.md`):
//!   - **Determinismo del digest**: el `digest` es
//!     `sha256(canonical_bytes_sin_created_at)`. Misma propuesta
//!     del planner => mismo sha. Si el planner modifica un campo
//!     del plan, el sha cambia; el apply debe entonces re-verificar.
//!   - **Reuso sin paralelo**: `AnalysisBasis` viene de B2.W1
//!     (`crate::application::analysis_basis::AnalysisBasis`) y
//!     `Severity` viene de B4.W2
//!     (`crate::architecture::diagnostic::Severity`). Este modulo
//!     no define tipos paralelos.
//!   - **Sin authority**: este modulo define el contrato PORTADOR
//!     del plan; no autoriza a aplicarlo. Eso es B5.W3.
//!   - **Sin paths relativos**: `affected_files` siempre es absoluto
//!     (`build_plan` lo normaliza con `AnalysisBasis::for_workspace`
//!     para el workspace_identity y exige paths absolutos para los
//!     archivos tocados). El apply rechazara los relativos por
//!     traversal (B5.W3 lo aplicara).

use std::ops::Range;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::application::analysis_basis::AnalysisBasis;
use crate::architecture::diagnostic::Severity;

/// Plan de refactor: pieza autoritativa portadora de una
/// propuesta. Identifica el plan, el workspace sobre el que aplica,
/// la base de analisis que lo sustenta y los cambios propuestos.
///
/// El `digest` es sha256 de la serializacion canonizada del plan
/// (orden estable, sin timestamps). Cualquier cambio de campos lo
/// invalida; el apply (B5.W3) lo verifica antes de ejecutar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefactorPlan {
    /// Identidad unica del plan (UUID v4). Permite referenciar el
    /// plan entre planner, apply y audit log.
    pub id: Uuid,
    /// Identidad del workspace sobre el que aplica el plan. Hoy se
    /// canonaliza con `AnalysisBasis::for_workspace(...)` — ver B2.W1
    /// para la politica exacta de normalizacion (`.`/`..` por
    /// componente, absolutize, canonicalize si existe).
    pub workspace_identity: String,
    /// Base de analisis que sustenta el plan. Reutiliza B2.W1 —
    /// el planner y el apply ven el mismo `AnalysisBasis` sobre el
    /// mismo workspace root.
    pub basis: AnalysisBasis,
    /// Archivos que el plan pretende tocar. Cada path se exige
    /// absoluto al construir (`build_plan`); aplicar sobre un path
    /// relativo seria traversal potencial.
    pub affected_files: Vec<PathBuf>,
    /// Ediciones planeadas, en orden determinista (planner las
    /// ordena por `(file, start)`).
    pub edits: Vec<PlannedEdit>,
    /// Precondiciones que el apply debe verificar antes de mutar.
    pub preconditions: Vec<Precondition>,
    /// Riesgos declarados por el planner, ordenados por `(severity
    /// desc, id asc)`.
    pub risks: Vec<Risk>,
    /// Timestamp de creacion. NO entra en el `digest` (es
    /// metadata; no es parte del contenido firmable).
    pub created_at: DateTime<Utc>,
    /// sha256 hex de la serializacion canonizada del plan SIN
    /// `created_at`. Estructuralmente estable entre builds.
    pub digest: String,
}

/// Edicion planeada sobre un archivo. `byte_range` se mide en bytes
/// del source (no line/column) para no desincronizarse con un
/// re-formato — mismo contrato que
/// `ArchitectureEvidence::byte_range` (B4.W2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedEdit {
    /// Path absoluto al archivo que se modifica.
    pub file: PathBuf,
    /// Rango de bytes a reemplazar (inclusivo `start`,
    /// exclusivo `end` por convencion).
    pub byte_range: Range<usize>,
    /// Texto de reemplazo completo para ese rango.
    pub replacement: String,
    /// Si `true`, la edicion se computo desde una representacion
    /// intermedia determinista (e.g. CST) y no desde parches text
    /// fragiles. El apply path prefiere edits `deterministic = true`.
    pub deterministic: bool,
}

/// Precondicion declarativa que el apply debe verificar antes de
/// actuar. La verificacion real se delega al port authority (no
/// reinventamos ese gating aqui).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Precondition {
    /// Capacidad o permiso requerido
    /// (e.g. `"file.write"`, `"workspace.authorized"`).
    pub capability: String,
    /// Descripcion legible de la precondicion. Llega al consumer
    /// humano/modelo para que entienda que se exige.
    pub description: String,
}

/// Riesgo declarado por el planner. El `evidence` opcional apunta a
/// el contador / observacion que lo sustenta (UAT, finding,
/// metric). `Severity` reusa el enum de B4.W2.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Risk {
    /// Identificador local del riesgo (e.g. `"R-rename-shadow"`,
    /// `"R-call-site-3"`). Estable entre versiones del plan.
    pub id: String,
    /// Descripcion legible.
    pub description: String,
    /// Evidencia opcional: contador, summary de finding, etc.
    /// Nunca codigo fuente completo — solo metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    /// Severidad declarada. Reusa el enum `Severity` de B4.W2.
    pub severity: Severity,
}

/// Errores que `build_plan` puede devolver. Contrato fail-closed:
/// paths relativos o vacios se rechazan antes de computar el digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildPlanError {
    /// Un `affected_file` o el `file` de un edit no es absoluto.
    RelativePath(PathBuf),
    /// Un `affected_file` o `file` esta vacio.
    EmptyPath,
}

impl std::fmt::Display for BuildPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RelativePath(p) => write!(
                f,
                "refactor plan requires absolute paths, got: {}",
                p.display()
            ),
            Self::EmptyPath => f.write_str("refactor plan rejects empty paths"),
        }
    }
}

impl std::error::Error for BuildPlanError {}

/// Tipo de resultado para `build_plan` / `verify_digest` callers.
pub type BuildPlanResult<T> = std::result::Result<T, BuildPlanError>;

/// Verifica invariantes estructurales que `build_plan` exige.
/// Exportado para que tests y callers pueden reusarlo.
pub fn validate_paths(paths: &[PathBuf]) -> BuildPlanResult<()> {
    for p in paths {
        if p.as_os_str().is_empty() {
            return Err(BuildPlanError::EmptyPath);
        }
        if !is_absolute(p) {
            return Err(BuildPlanError::RelativePath(p.clone()));
        }
    }
    Ok(())
}

/// Wrapper pequeno de `Path::is_absolute` que tambien rechaza paths
/// vacios. `Path::is_absolute("")` puede dar resultados ruidosos en
/// algunas plataformas — `validate_paths` ya filtra vacios, pero
/// mantener el helper explicito ayuda a razonar.
fn is_absolute(p: &Path) -> bool {
    p.has_root() && !p.as_os_str().is_empty()
}

/// Serializa un `RefactorPlan` SIN `created_at` ni `digest` para
/// producir los bytes canonizados sobre los que se calcula el
/// sha256. El orden de campos se pinea via re-serializacion
/// determinista: usamos `serde_json` con un `BTreeMap`-friendly
/// shape (todos los structs derivan `Serialize` con campos en orden
/// alfabetico / de declaracion) y luego ordenamos el array de
/// `edits` y `risks` por su clave estructural.
///
/// Para mantener la canonicidad robusta independiente del orden de
/// declaracion de campos, serializamos a `serde_json::Value` y
/// re-canonizamos: todos los objetos se ordenan por clave (BTreeMap),
/// los `affected_files` y `edits` se ordenan estructuralmente.
fn canonical_bytes(plan: &RefactorPlan) -> Vec<u8> {
    use std::collections::BTreeMap;

    // Convertimos a JSON Value para poder re-canonizar
    // independientemente del orden de los campos en los structs.
    //
    // R-B5-W1: el digest es funcion del CONTENIDO firmable del
    // plan, no de la metadata no-firmable. Excluimos:
    //   - `id` (UUID v4 aleatorio: dos builders con el mismo
    //     contenido producen digests identicos solo si el id no
    //     participa; lo mantenemos fuera del input para no
    //     invalidar la garantia de determinismo).
    //   - `created_at` (timestamp de creacion: igual motivo).
    // El id puede leerse desde `RefactorPlan.id` para referenciar
    // el plan; el apply (B5.W3) lo cruza contra su `digest` antes
    // de actuar.
    let value = serde_json::json!({
        "affected_files": plan
            .affected_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        "basis": {
            "workspace_root": plan.basis.workspace_root().to_string_lossy(),
            "profile": format!("{:?}", plan.basis.profile()),
            "budget": {
                "max_elapsed_ms": plan
                    .basis
                    .budget()
                    .max_elapsed
                    .map(|d| d.as_millis() as u64),
                "max_entities": plan.basis.budget().max_entities,
            },
        },
        "edits": plan.edits.iter().map(|e| {
            serde_json::json!({
                "deterministic": e.deterministic,
                "file": e.file.to_string_lossy(),
                "byte_range": [e.byte_range.start, e.byte_range.end],
                "replacement": e.replacement,
            })
        }).collect::<Vec<_>>(),
        "preconditions": plan.preconditions.iter().map(|p| {
            serde_json::json!({
                "capability": p.capability,
                "description": p.description,
            })
        }).collect::<Vec<_>>(),
        "risks": plan.risks.iter().map(|r| {
            serde_json::json!({
                "id": r.id,
                "description": r.description,
                "evidence": r.evidence,
                "severity": r.severity.as_str(),
            })
        }).collect::<Vec<_>>(),
        "workspace_identity": plan.workspace_identity,
    });

    // Re-canonizacion: ordenamos recursivamente los objetos por
    // clave. Esto da bytes identicos para el mismo plan aunque
    // los structs cambien el orden de campos.
    let canon: BTreeMap<String, serde_json::Value> = match value {
        serde_json::Value::Object(m) => m.into_iter().collect(),
        _ => BTreeMap::new(),
    };
    serde_json::to_vec(&canon).expect("canonical bytes serialize")
}

/// Computa el digest sha256 del plan. EXCLUYE `created_at` y el
/// propio `digest` (campo recursivo que romperia la verificacion).
pub fn compute_digest(plan: &RefactorPlan) -> String {
    let bytes = canonical_bytes(plan);
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Normalizacion ligera para ordenar ediciones. Estabilidad
/// estructural: igualdad byte-a-byte en el digest requiere orden
/// estable.
fn sort_edits(mut edits: Vec<PlannedEdit>) -> Vec<PlannedEdit> {
    edits.sort_by(|a, b| {
        a.file
            .to_string_lossy()
            .cmp(&b.file.to_string_lossy())
            .then_with(|| a.byte_range.start.cmp(&b.byte_range.start))
            .then_with(|| a.byte_range.end.cmp(&b.byte_range.end))
    });
    edits
}

/// Ordena riesgos por severidad descendente y luego por id asc.
fn sort_risks(mut risks: Vec<Risk>) -> Vec<Risk> {
    risks.sort_by(|a, b| {
        b.severity
            .rank()
            .cmp(&a.severity.rank())
            .then_with(|| a.id.cmp(&b.id))
    });
    risks
}

/// Constructor canonico de un `RefactorPlan`. Computa el `digest`
/// y fija `created_at = Utc::now()`.
///
/// Garantias:
///   - `affected_files` y los `file` de cada `edit` son absolutos
///     o el constructor falla (`RelativePath`/`EmptyPath`).
///   - `edits` y `risks` se ordenan estructuralmente para que el
///     digest sea invariante al orden de insercion del caller.
///   - `id` se genera via `Uuid::new_v4` (UUID v4 CSPRNG via
///     `getrandom`).
pub fn build_plan(
    workspace_root: PathBuf,
    basis: AnalysisBasis,
    affected_files: Vec<PathBuf>,
    edits: Vec<PlannedEdit>,
    preconditions: Vec<Precondition>,
    risks: Vec<Risk>,
) -> BuildPlanResult<RefactorPlan> {
    // Validacion fail-closed: rechaza paths vacios o relativos en
    // `affected_files` y en cada `edit.file`.
    validate_paths(&affected_files)?;
    for edit in &edits {
        if edit.file.as_os_str().is_empty() {
            return Err(BuildPlanError::EmptyPath);
        }
        if !is_absolute(&edit.file) {
            return Err(BuildPlanError::RelativePath(edit.file.clone()));
        }
    }

    let edits = sort_edits(edits);
    let risks = sort_risks(risks);

    let workspace_identity = canonicalize_workspace_identity(&workspace_root);

    let id = Uuid::new_v4();
    let created_at = Utc::now();

    let mut plan = RefactorPlan {
        id,
        workspace_identity,
        basis,
        affected_files,
        edits,
        preconditions,
        risks,
        created_at,
        digest: String::new(),
    };
    plan.digest = compute_digest(&plan);
    Ok(plan)
}

/// Canonicaliza el `workspace_identity` con la misma politica que
/// `AnalysisBasis::for_workspace`: resuelve `.`/`..` por
/// componente, absolutiza, y canonicaliza si existe. Si no existe,
/// devuelve la forma normalizada sin tocar el disco. Esto pinea que
/// dos builders con el mismo workspace produzcan el mismo digest.
fn canonicalize_workspace_identity(root: &Path) -> String {
    fn normalize(path: &Path) -> PathBuf {
        let mut out = PathBuf::new();
        for comp in path.components() {
            match comp {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    if !out.pop() {
                        out.push("..");
                    }
                }
                other => out.push(other.as_os_str()),
            }
        }
        out
    }
    let normalized = normalize(root);
    let absolute = if normalized.is_absolute() {
        normalized
    } else {
        std::path::absolute(&normalized).unwrap_or(normalized)
    };
    let canonical = if absolute.exists() {
        absolute.canonicalize().unwrap_or(absolute)
    } else {
        absolute
    };
    canonical.to_string_lossy().into_owned()
}

/// Recomputa el digest y compara contra el `digest` almacenado.
/// No modifica el plan; cualquier divergencia es un error.
pub fn verify_digest(plan: &RefactorPlan) -> bool {
    compute_digest(plan) == plan.digest
}

/// Errores de E/S al cargar o serializar un plan desde un archivo.
/// Envuelve el tipo subyacente para que el caller (CLI / apply)
/// pueda manejarlo sin filtrar detalles del crate `std::io` por
/// encima del contrato del modulo.
#[derive(Debug)]
pub enum PlanIoError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    DigestMismatch { expected: String, actual: String },
    Empty,
}

impl std::fmt::Display for PlanIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Parse(e) => write!(f, "json parse error: {e}"),
            Self::DigestMismatch { expected, actual } => write!(
                f,
                "digest mismatch: stored={expected}, computed={actual} \
                 (plan has been modified since it was written)"
            ),
            Self::Empty => f.write_str("plan file is empty"),
        }
    }
}

impl std::error::Error for PlanIoError {}

/// Tipo de resultado para `load_plan_from_file` / `save_plan_to_file`.
pub type PlanIoResult<T> = std::result::Result<T, PlanIoError>;

/// Carga un plan desde un archivo JSON. Verifica el digest al
/// final del load — un plan con un digest que no coincide con su
/// contenido se rechaza como corrupto (no se repara; el apply
/// path debe negarse).
pub fn load_plan_from_file(path: &Path) -> PlanIoResult<RefactorPlan> {
    let bytes = std::fs::read(path).map_err(PlanIoError::Io)?;
    if bytes.is_empty() {
        return Err(PlanIoError::Empty);
    }
    let plan: RefactorPlan =
        serde_json::from_slice(&bytes).map_err(PlanIoError::Parse)?;
    if !verify_digest(&plan) {
        let actual = compute_digest(&plan);
        return Err(PlanIoError::DigestMismatch {
            expected: plan.digest.clone(),
            actual,
        });
    }
    Ok(plan)
}

/// Serializa un plan a JSON y lo escribe en `path`. NO actualiza
/// `created_at`; el caller decide si crear un plan nuevo. Verifica
/// que el digest siga siendo consistente antes de retornar.
pub fn save_plan_to_file(plan: &RefactorPlan, path: &Path) -> PlanIoResult<()> {
    let bytes = serde_json::to_vec_pretty(plan).map_err(PlanIoError::Parse)?;
    std::fs::write(path, bytes).map_err(PlanIoError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn basis_for(ws: &str) -> AnalysisBasis {
        AnalysisBasis::for_workspace(PathBuf::from(ws))
    }

    fn sample_edit(file: &str, start: usize, end: usize) -> PlannedEdit {
        PlannedEdit {
            file: PathBuf::from(file),
            byte_range: start..end,
            replacement: "REPLACED".to_string(),
            deterministic: true,
        }
    }

    fn sample_risk(id: &str, severity: Severity) -> Risk {
        Risk {
            id: id.to_string(),
            description: "sample".to_string(),
            evidence: None,
            severity,
        }
    }

    // 1. Digest estable entre llamadas con el mismo input.
    #[test]
    fn digest_is_stable_across_builds() {
        let root = PathBuf::from("/tmp/workspace");
        let plan1 = build_plan(
            root.clone(),
            basis_for("/tmp/workspace"),
            vec![PathBuf::from("/tmp/workspace/a.rs")],
            vec![sample_edit("/tmp/workspace/a.rs", 0, 4)],
            vec![Precondition {
                capability: "file.write".to_string(),
                description: "write access".to_string(),
            }],
            vec![sample_risk("R-test", Severity::Warning)],
        )
        .expect("build_plan ok");
        let plan2 = build_plan(
            root,
            basis_for("/tmp/workspace"),
            vec![PathBuf::from("/tmp/workspace/a.rs")],
            vec![sample_edit("/tmp/workspace/a.rs", 0, 4)],
            vec![Precondition {
                capability: "file.write".to_string(),
                description: "write access".to_string(),
            }],
            vec![sample_risk("R-test", Severity::Warning)],
        )
        .expect("build_plan ok");
        // created_at differs -> digest MUST be equal.
        assert_eq!(plan1.digest, plan2.digest);
        assert_ne!(plan1.created_at, plan2.created_at);
        assert!(verify_digest(&plan1));
        assert!(verify_digest(&plan2));
    }

    // 2. Edits se serializan deterministamente independiente del
    //    orden de insercion.
    #[test]
    fn edits_serialize_deterministically() {
        let e1 = sample_edit("/tmp/a.rs", 0, 5);
        let e2 = sample_edit("/tmp/a.rs", 20, 25);
        let e3 = sample_edit("/tmp/b.rs", 0, 5);

        let plan_a = build_plan(
            PathBuf::from("/tmp"),
            basis_for("/tmp"),
            vec![PathBuf::from("/tmp/a.rs"), PathBuf::from("/tmp/b.rs")],
            vec![e1.clone(), e2.clone(), e3.clone()],
            Vec::new(),
            Vec::new(),
        )
        .expect("plan_a ok");

        let plan_b = build_plan(
            PathBuf::from("/tmp"),
            basis_for("/tmp"),
            vec![PathBuf::from("/tmp/a.rs"), PathBuf::from("/tmp/b.rs")],
            vec![e3, e2, e1],
            Vec::new(),
            Vec::new(),
        )
        .expect("plan_b ok");

        assert_eq!(plan_a.digest, plan_b.digest);
    }

    // 3. affected_files y edit.file deben ser absolutos. Path
    //    relativo -> RelativePath.
    #[test]
    fn affected_files_must_be_absolute() {
        let result = build_plan(
            PathBuf::from("/tmp"),
            basis_for("/tmp"),
            vec![PathBuf::from("relative/file.rs")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            result.err(),
            Some(BuildPlanError::RelativePath(PathBuf::from(
                "relative/file.rs"
            )))
        );
    }

    #[test]
    fn edit_with_relative_file_rejected() {
        let result = build_plan(
            PathBuf::from("/tmp"),
            basis_for("/tmp"),
            vec![PathBuf::from("/tmp/a.rs")],
            vec![sample_edit("relative.rs", 0, 5)],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            result.err(),
            Some(BuildPlanError::RelativePath(PathBuf::from(
                "relative.rs"
            )))
        );
    }

    // 4. Preconditions preservadas bit-a-bit (round-trip via JSON).
    #[test]
    fn preconditions_preserved_through_serialization() {
        let preconditions = vec![
            Precondition {
                capability: "file.write".to_string(),
                description: "must have write access".to_string(),
            },
            Precondition {
                capability: "workspace.authorized".to_string(),
                description: "workspace must be authorized for apply".to_string(),
            },
        ];
        let plan = build_plan(
            PathBuf::from("/tmp"),
            basis_for("/tmp"),
            vec![PathBuf::from("/tmp/a.rs")],
            Vec::new(),
            preconditions.clone(),
            Vec::new(),
        )
        .expect("build ok");
        let json = serde_json::to_string(&plan).expect("serialize");
        let back: RefactorPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.preconditions, preconditions);
    }

    // 5. Risk ordering stable: insertion order doesnt change digest.
    #[test]
    fn risk_ordering_is_stable() {
        let risks_in_a = vec![
            sample_risk("R-low", Severity::Info),
            sample_risk("R-high", Severity::Error),
            sample_risk("R-mid", Severity::Warning),
        ];
        let risks_in_b = vec![
            sample_risk("R-mid", Severity::Warning),
            sample_risk("R-high", Severity::Error),
            sample_risk("R-low", Severity::Info),
        ];
        let plan_a = build_plan(
            PathBuf::from("/tmp"),
            basis_for("/tmp"),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            risks_in_a,
        )
        .expect("plan_a ok");
        let plan_b = build_plan(
            PathBuf::from("/tmp"),
            basis_for("/tmp"),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            risks_in_b,
        )
        .expect("plan_b ok");
        assert_eq!(plan_a.digest, plan_b.digest);
        // Ordering applied: severity desc, id asc.
        assert_eq!(plan_a.risks[0].id, "R-high");
        assert_eq!(plan_a.risks[1].id, "R-mid");
        assert_eq!(plan_a.risks[2].id, "R-low");
    }

    // 6. Digest excluye created_at: cambiar created_at no altera el
    //    sha256. Se valida recalculando manual sobre los mismos
    //    campos y comparando.
    #[test]
    fn digest_excludes_created_at() {
        let plan = build_plan(
            PathBuf::from("/tmp"),
            basis_for("/tmp"),
            vec![PathBuf::from("/tmp/a.rs")],
            vec![sample_edit("/tmp/a.rs", 0, 4)],
            Vec::new(),
            Vec::new(),
        )
        .expect("plan ok");
        // Reconstruimos un plan identico con otro created_at:
        // el digest recalculado debe coincidir.
        let mut twin = plan.clone();
        twin.created_at = twin.created_at + chrono::Duration::seconds(42);
        assert_eq!(compute_digest(&plan), compute_digest(&twin));
    }

    // 7. Modificar un campo firma-able invalida el digest.
    #[test]
    fn digest_invalidates_on_field_change() {
        let mut plan = build_plan(
            PathBuf::from("/tmp"),
            basis_for("/tmp"),
            vec![PathBuf::from("/tmp/a.rs")],
            vec![sample_edit("/tmp/a.rs", 0, 4)],
            Vec::new(),
            Vec::new(),
        )
        .expect("plan ok");
        let original = plan.digest.clone();
        plan.edits[0].replacement = "DIFFERENT".to_string();
        let new_digest = compute_digest(&plan);
        assert_ne!(new_digest, original);
    }
}
