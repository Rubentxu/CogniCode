//! `cognicode-core::refactor::apply` — B5.W3.
//!
//! Apply con autoridad y rollback. Lee un [`crate::refactor::plan::RefactorPlan`]
//! (B5.W1) y un `workspace_state_digest` calculado en el momento de
//! la previsualización, y aplica las [`PlannedEdit`]s del plan sobre
//! el workspace con staging + fsync + rename atómico.
//!
//! ## Flujo
//!
//!   1. Validar `AuthorizationToken` (canal separado; no derivado del
//!      plan; AAT-B5-03).
//!   2. Recomputar `workspace_state_digest` desde los archivos del
//!      plan y compararlo con el digest del preview. Mismatch =>
//!      `WorkspaceStateMismatch` (el workspace mutó entre preview y
//!      apply; rehúsa, no se intenta parchear a ciegas).
//!   3. Para cada `PlannedEdit`: verificar que `file` está dentro
//!      del workspace, rechazar `..`/symlinks, leer el archivo,
//!      splicing `replacement` en `byte_range`, escribir al staging
//!      dir, `sync_all`, `rename` atómico.
//!   4. Si CUALQUIER paso falla, borrar el staging dir completo y
//!      reportar el error (AAT-B5-01).
//!
//! ## Por qué `workspace_state_digest` viene del preview
//!
//! El plan de B5.W1 lleva un `digest` (sha256 sobre los campos del
//! plan). NO lleva el estado del workspace sobre el que se calculó.
//! Ese dato lo emite el preview (B5.W2). Apply lo recibe por
//! argumento separado para que el caller (CLI o MCP) pase
//! explícitamente "el preview que vio el humano antes de autorizar".
//! Sin ese dato, apply no puede detectar mutaciones concurrentes.
//!
//! ## Por qué no se deriva el token del plan (AAT-B5-03)
//!
//! El `AuthorizationToken` se pasa por argumento separado. El apply
//! path NO lo lee del plan, NO lo computa desde el plan, y NO acepta
//! un campo `authorization_token` en ningún JSON que pase por el MCP
//! read-only (`SafeRefactorInput` lleva `deny_unknown_fields`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::application::ports::PathPolicyError;
use crate::domain::findings::digest::{DigestStream, sha256_short};
use crate::paths::PathRules;

use super::plan::{PlannedEdit, RefactorPlan};

// =============================================================================
// Constantes públicas
// =============================================================================

/// Nombre del subdirectorio de staging dentro del workspace.
///
/// Visible a los tests (`pub`) para que puedan verificar la limpieza
/// (AAT-B5-01).
pub const APPLY_STAGING_DIR: &str = ".cognicode-staging";

/// Caducidad de un token de write. Pasada esa ventana, el consent
/// ya no es "actual" y se rechaza.
const TOKEN_MAX_AGE_HOURS: i64 = 1;

// =============================================================================
// Hash utility
// =============================================================================

/// Hash determinista corto de un contenido (16 chars hex).
///
/// Lo usamos para computar `workspace_state_digest` sobre los bytes
/// actuales del workspace. Mismo input => mismo hash.
pub fn hash_bytes(bytes: &[u8]) -> String {
    sha256_short(bytes)
}

/// Compute el `workspace_state_digest` para un plan.
///
/// Estrategia: orden estable por `file.to_string_lossy()`, concat
/// `path || post-edit bytes` y hashea. Si el preview y el apply ven
/// los mismos bytes sobre los mismos paths, obtienen el mismo digest.
///
/// Si un archivo del plan no existe en el disco, se lo trata como
/// bytes vacíos — esto significa que un archivo borrado entre
/// preview y apply cambia el digest y dispara `WorkspaceStateMismatch`.
pub fn compute_workspace_state_digest(plan: &RefactorPlan) -> String {
    let mut stream = DigestStream::new();
    for edit in sorted_edits(plan) {
        stream
            .update(edit.file.to_string_lossy().as_bytes())
            .update(b"\0");
        let current_bytes = std::fs::read(&edit.file).unwrap_or_default();
        let new_bytes = splice_replacement(&current_bytes, edit);
        stream.update(&new_bytes).update(b"\0");
    }
    stream.finish_short()
}

fn sorted_edits(plan: &RefactorPlan) -> Vec<&PlannedEdit> {
    let mut edits: Vec<&PlannedEdit> = plan.edits.iter().collect();
    edits.sort_by(|a, b| {
        a.file
            .to_string_lossy()
            .cmp(&b.file.to_string_lossy())
            .then_with(|| a.byte_range.start.cmp(&b.byte_range.start))
            .then_with(|| a.byte_range.end.cmp(&b.byte_range.end))
    });
    edits
}

/// Splice `edit.replacement` en `bytes` según `edit.byte_range`.
/// Devuelve los bytes nuevos. Si `byte_range` está fuera de bounds,
/// devuelve los originales (el apply path detecta esto como
/// `ConcurrentModification` en una capa superior).
fn splice_replacement(bytes: &[u8], edit: &PlannedEdit) -> Vec<u8> {
    let start = edit.byte_range.start;
    let end = edit.byte_range.end.min(bytes.len());
    if start > end || start > bytes.len() {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(start + edit.replacement.len() + (bytes.len() - end));
    out.extend_from_slice(&bytes[..start]);
    out.extend_from_slice(edit.replacement.as_bytes());
    out.extend_from_slice(&bytes[end..]);
    out
}

// =============================================================================
// AuthorizationToken
// =============================================================================

/// Token de autorización de write. **Opaque** para el apply path: se
/// recibe por canal separado (no del plan JSON) y NO se deriva del
/// contenido del plan (AAT-B5-03). Se serializa solo dentro de la
/// proposal (TASK-220), que es quien lo acuña y lo persiste con su
/// momento de emisión.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorizationToken {
    pub value: String,
    pub created_at: DateTime<Utc>,
}

/// Validar un `AuthorizationToken` por contrato.
///
/// Reglas:
///   - `value` no vacío (`AuthorizationMissing`).
///   - `created_at` dentro de la ventana de 1h (`AuthorizationInvalid`).
///
/// Esta función NO valida contra un server-side whitelist: el modelo
/// de amenazas es que el token se emite por canal separado al aplicar
/// y caduca por tiempo. Un token "envejecido" no es consentimiento
/// reciente.
pub fn validate_authorization_token(token: &AuthorizationToken) -> Result<(), ApplyError> {
    validate_authorization_token_at(token, Utc::now())
}

/// La misma validación con el reloj inyectado: la ventana de una hora
/// se juzga contra `now`, no contra el reloj de pared, y un test puede
/// demostrar que un token caducado se rechaza (TASK-220 AC #3).
pub fn validate_authorization_token_at(
    token: &AuthorizationToken,
    now: DateTime<Utc>,
) -> Result<(), ApplyError> {
    if token.value.trim().is_empty() {
        return Err(ApplyError::AuthorizationMissing);
    }
    let age = now.signed_duration_since(token.created_at);
    let max = chrono::Duration::hours(TOKEN_MAX_AGE_HOURS);
    if age > max || age < -max {
        return Err(ApplyError::AuthorizationInvalid);
    }
    Ok(())
}

// =============================================================================
// ApplyOutcome + ApplyError
// =============================================================================

/// Resultado de un apply. Si `committed == false`, no se ha escrito
/// nada; ver `error`. Si `committed == true`, `applied_files`
/// lista lo que se renombró.
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    pub plan_id: Uuid,
    pub plan_digest: String,
    pub applied_files: Vec<PathBuf>,
    pub rolled_back_files: Vec<PathBuf>,
    pub committed: bool,
    pub error: Option<String>,
}

/// Errores honestos del apply path. Cada variante es una historia
/// distinta; no se presenta `Complete/clean/no impact` para
/// `Partial/Unknown/Failed` (AGENTS.md disciplina).
#[derive(Debug)]
pub enum ApplyError {
    /// El token estaba vacío o no se pasó.
    AuthorizationMissing,
    /// El token estaba presente pero caducado o mal-formado.
    AuthorizationInvalid,
    /// El workspace cambió desde el preview (digest mismatch).
    WorkspaceStateMismatch { expected: String, actual: String },
    /// El path es un symlink: no se escribe a través de él, apunte
    /// adentro o afuera (AAT-B5-02; el preview es simétrico).
    SymlinkEscape { path: PathBuf },
    /// El path no puede entrar al workspace: traversal textual, fuera
    /// de la frontera canónica, demasiado profundo o con caracteres
    /// inválidos (la misma familia de rechazo que producían las
    /// guardas textuales que sustituyó la delegación de TASK-219).
    TraversalEscape { path: PathBuf },
    /// El archivo del plan ya no tiene el `byte_range` esperado
    /// (cambió de tamaño o de contenido entre preview y apply).
    ConcurrentModification {
        path: PathBuf,
        expected_size: usize,
        actual_size: usize,
    },
    /// Error de I/O durante staging/rename/delete, o el path que no
    /// se pudo inspeccionar (rechazado por las reglas del workspace).
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::AuthorizationMissing => write!(
                f,
                "authorization_token missing: apply requires an explicit token \
                 received via a separate channel"
            ),
            ApplyError::AuthorizationInvalid => write!(
                f,
                "authorization_token invalid: empty, expired, or malformed"
            ),
            ApplyError::WorkspaceStateMismatch { expected, actual } => write!(
                f,
                "workspace state mismatch: digest expected={} actual={}; \
                 rebuild the plan",
                expected, actual
            ),
            ApplyError::SymlinkEscape { path } => write!(
                f,
                "refusing to write through symlink: {} (symlinked paths are not followed)",
                path.display()
            ),
            ApplyError::TraversalEscape { path } => write!(
                f,
                "refusing path outside the workspace boundary: {}",
                path.display()
            ),
            ApplyError::ConcurrentModification {
                path,
                expected_size,
                actual_size,
            } => write!(
                f,
                "concurrent modification on {}: expected_size={} actual_size={}",
                path.display(),
                expected_size,
                actual_size
            ),
            ApplyError::IoError { path, source } => {
                write!(f, "I/O error on {}: {}", path.display(), source)
            }
        }
    }
}

impl std::error::Error for ApplyError {}

// =============================================================================
// apply() — el corazón del WU
// =============================================================================

/// Aplica `plan` al `workspace_root` con autoridad separada.
///
/// Pre-condiciones:
///   - `token` válido y reciente.
///   - `workspace_state_digest` igual al digest que el preview
///     calculó sobre los mismos archivos.
///
/// Si todo pasa: `Ok(ApplyOutcome { committed: true, .. })`. Si algo
/// falla: `Err(ApplyError)` Y el staging dir está limpio.
pub fn apply(
    plan: &RefactorPlan,
    workspace_state_digest: &str,
    token: &AuthorizationToken,
    workspace_root: &Path,
) -> Result<ApplyOutcome, ApplyError> {
    // (1) Token válido.
    validate_authorization_token(token)?;

    // (2) Todos los paths del plan pasan por las reglas del workspace
    //     ANTES de leer un byte: el digest se calcula sobre lo que las
    //     reglas ya dejaron entrar. Leer a través de un symlink que
    //     escapa para calcular el digest era tocar fuera del workspace
    //     (goldfish TASK-219). La puerta es la de paths internos: el
    //     plan nombra ubicaciones resueltas, no texto de cliente.
    let rules = PathRules::new().with_workspace(vec![workspace_root.to_path_buf()]);
    for edit in &plan.edits {
        if !edit.file.is_absolute() {
            return Err(ApplyError::TraversalEscape {
                path: edit.file.clone(),
            });
        }
        rules
            .validate_internal_path(&edit.file)
            .map_err(|e| apply_policy_error(&edit.file, e))?;
    }

    // (3) Workspace state digest coincide.
    let actual_digest = compute_workspace_state_digest(plan);
    if actual_digest != workspace_state_digest {
        return Err(ApplyError::WorkspaceStateMismatch {
            expected: workspace_state_digest.to_string(),
            actual: actual_digest,
        });
    }

    // (3) Pre-flight por edición.
    let staging_base = workspace_root.join(APPLY_STAGING_DIR);
    let staging_dir = staging_base.join(plan.id.to_string());

    if staging_dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    fs::create_dir_all(&staging_dir).map_err(|e| ApplyError::IoError {
        path: staging_dir.clone(),
        source: e,
    })?;

    let mut applied: Vec<AppliedFile> = Vec::new();

    // Aplicar cada edición. El primer fallo aborta el batch.
    let result: Result<(), ApplyError> = plan.edits.iter().try_for_each(|edit| {
        apply_one(edit, workspace_root, &staging_dir).map(|rec| {
            applied.push(rec);
        })
    });

    if let Err(err) = result {
        // Rollback: RESTITUYE el estado previo de cada archivo ya
        // aplicado en este batch. Borrar el archivo sería pérdida de
        // datos: el rename atómico de `apply_one` ya reemplazó los
        // bytes originales del usuario, así que deshacer exige
        // reescribirlos (AAT-B5-01).
        for rec in &applied {
            restore_prior_state(rec);
        }
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(err);
    }

    let _ = fs::remove_dir_all(&staging_dir);

    Ok(ApplyOutcome {
        plan_id: plan.id,
        plan_digest: plan.digest.clone(),
        applied_files: applied.into_iter().map(|rec| rec.path).collect(),
        rolled_back_files: Vec::new(),
        committed: true,
        error: None,
    })
}

/// Estado previo de un archivo que `apply_one` ya sobrescribió.
struct AppliedFile {
    /// Path absoluto destino (el que quedó renombrado).
    path: PathBuf,
    /// Bytes previos. `None` = el archivo NO existía antes del
    /// apply, así que deshacer es borrarlo, no restaurarlo.
    previous: Option<Vec<u8>>,
}

/// Deshace un `AppliedFile`, devolviendo el archivo a su estado
/// previo.
///
/// Best-effort: `apply` ya devuelve `Err` por el fallo que provocan
/// el rollback, y en esa rama no existe `ApplyOutcome` donde cabría
/// reportar un restore fallido. Un `None` en `previous` significa que
/// el archivo no existía antes, así que deshacer es borrarlo.
fn restore_prior_state(rec: &AppliedFile) {
    match &rec.previous {
        Some(bytes) => {
            let _ = fs::write(&rec.path, bytes);
        }
        None => {
            let _ = fs::remove_file(&rec.path);
        }
    }
}

/// El rechazo de las reglas del workspace, en las palabras del apply.
/// La hoja symlink no se escribe a traves (AAT-B5-02); el escape del
/// workspace y el traversal son `TraversalEscape`, la misma respuesta
/// que daban las guardas textuales que sustituye esto; lo que no se
/// pudo ni inspeccionar es un error de I/O honesto.
fn apply_policy_error(path: &Path, error: PathPolicyError) -> ApplyError {
    match error {
        PathPolicyError::SymlinkDetected { .. } => ApplyError::SymlinkEscape {
            path: path.to_path_buf(),
        },
        PathPolicyError::PathOutsideWorkspace
        | PathPolicyError::PathTraversalAttempt { .. }
        | PathPolicyError::PathTooDeep { .. }
        | PathPolicyError::InvalidPathCharacters { .. } => ApplyError::TraversalEscape {
            path: path.to_path_buf(),
        },
        PathPolicyError::PathNotAccessible { .. } => ApplyError::IoError {
            path: path.to_path_buf(),
            source: std::io::Error::other(
                "path not accessible (rejected by the workspace path rules)",
            ),
        },
    }
}

/// Apply una edición de un solo archivo con staging+fsync+rename.
///
/// Devuelve el path renombrado junto con los bytes que tenía ANTES
/// del rename, para que `apply` pueda deshacer el cambio si una
/// edición posterior del mismo batch falla.
fn apply_one(
    edit: &PlannedEdit,
    workspace_root: &Path,
    staging_dir: &Path,
) -> Result<AppliedFile, ApplyError> {
    // (a) El path debe ser absoluto (B5.W1 ya exige esto, pero
    //     verificamos aquí también como defensa).
    if !edit.file.is_absolute() {
        return Err(ApplyError::TraversalEscape {
            path: edit.file.clone(),
        });
    }

    // (b) Un solo juicio sobre el path: las reglas del workspace
    //     (TASK-61/62) — hoja symlink, profundidad y frontera canonica,
    //     por la puerta de paths internos (el plan nombra ubicaciones
    //     resueltas, no texto de cliente). El `starts_with` textual que
    //     habia aqui dejaba colar un directorio intermedio que fuera
    //     symlink y escapara del workspace (TASK-219); canonicalizar es
    //     lo que lo ve. `apply` ya valido este plan; se vuelve a pedir
    //     el mismo juicio a la misma autoridad porque apply_one es la
    //     unidad que se prueba sola.
    PathRules::new()
        .with_workspace(vec![workspace_root.to_path_buf()])
        .validate_internal_path(&edit.file)
        .map_err(|e| apply_policy_error(&edit.file, e))?;

    // (e) Leer bytes actuales.
    let current_bytes = fs::read(&edit.file).map_err(|e| ApplyError::IoError {
        path: edit.file.clone(),
        source: e,
    })?;

    // (f) Verifica que el byte_range sigue siendo válido (no
    //     concurrent modification).
    if edit.byte_range.start > current_bytes.len() || edit.byte_range.end > current_bytes.len() {
        return Err(ApplyError::ConcurrentModification {
            path: edit.file.clone(),
            expected_size: edit.byte_range.end,
            actual_size: current_bytes.len(),
        });
    }

    // (g) Compute los nuevos bytes.
    let new_bytes = splice_replacement(&current_bytes, edit);

    // (h) Stage preservando la estructura de sub-directorios relativa
    //     al workspace. La comparación es canónica por los dos lados:
    //     si la raíz llega con otro nombre (un symlink de por medio),
    //     un `strip_prefix` textual falla y el stage degenera en
    //     escribir EN SITIO sobre el original, sin copia previa ni
    //     rename atómico (goldfish TASK-219, AAT-B5-01).
    let file_canonical = std::fs::canonicalize(&edit.file).map_err(|e| ApplyError::IoError {
        path: edit.file.clone(),
        source: e,
    })?;
    let root_canonical =
        std::fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
    let rel =
        file_canonical
            .strip_prefix(&root_canonical)
            .map_err(|_| ApplyError::TraversalEscape {
                path: edit.file.clone(),
            })?;
    let stage_abs = staging_dir.join(rel);
    if let Some(parent) = stage_abs.parent() {
        fs::create_dir_all(parent).map_err(|e| ApplyError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    // (i) Write + fsync.
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&stage_abs)
            .map_err(|e| ApplyError::IoError {
                path: stage_abs.clone(),
                source: e,
            })?;
        f.write_all(&new_bytes).map_err(|e| ApplyError::IoError {
            path: stage_abs.clone(),
            source: e,
        })?;
        f.sync_all().map_err(|e| ApplyError::IoError {
            path: stage_abs.clone(),
            source: e,
        })?;
    }

    // (j) Atomic rename.
    fs::rename(&stage_abs, &edit.file).map_err(|e| {
        let _ = fs::remove_file(&stage_abs);
        ApplyError::IoError {
            path: edit.file.clone(),
            source: e,
        }
    })?;

    Ok(AppliedFile {
        path: edit.file.clone(),
        // El rename de (j) ya consumió los bytes originales: es este
        // snapshot el único que permite restaurarlos en el rollback.
        previous: Some(current_bytes),
    })
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::analysis_basis::AnalysisBasis;
    use crate::architecture::diagnostic::Severity;
    use crate::refactor::plan::{Risk, build_plan};
    use std::fs;
    use tempfile::TempDir;

    fn simple_plan(tmp: &TempDir) -> RefactorPlan {
        let file = tmp.path().join("a.rs");
        fs::write(&file, "hello world").unwrap();
        build_plan(
            tmp.path().to_path_buf(),
            AnalysisBasis::for_workspace(tmp.path().to_path_buf()),
            vec![file.clone()],
            vec![PlannedEdit {
                file: file.clone(),
                byte_range: 6..11,
                replacement: "Rust".to_string(),
                deterministic: true,
            }],
            vec![],
            vec![Risk {
                id: "R-test".to_string(),
                description: "test".to_string(),
                evidence: None,
                severity: Severity::Info,
            }],
        )
        .expect("build_plan")
    }

    #[test]
    fn unit_hash_bytes_is_stable() {
        assert_eq!(hash_bytes(b"hello"), hash_bytes(b"hello"));
        assert_ne!(hash_bytes(b"hello"), hash_bytes(b"hello!"));
        assert_eq!(hash_bytes(b"").len(), 16);
    }

    #[test]
    fn unit_validate_authorization_token_rejects_empty_and_stale() {
        let empty = AuthorizationToken {
            value: String::new(),
            created_at: Utc::now(),
        };
        assert!(matches!(
            validate_authorization_token(&empty),
            Err(ApplyError::AuthorizationMissing)
        ));
        let stale = AuthorizationToken {
            value: "tk".into(),
            created_at: Utc::now() - chrono::Duration::hours(2),
        };
        assert!(matches!(
            validate_authorization_token(&stale),
            Err(ApplyError::AuthorizationInvalid)
        ));
        let fresh = AuthorizationToken {
            value: "tk-fresh".into(),
            created_at: Utc::now(),
        };
        assert!(validate_authorization_token(&fresh).is_ok());
    }

    /// TASK-220 AC #3 — la ventana se juzga contra el reloj inyectado:
    /// un token emitido «ahora» está vivo, caduca cuando `now` lo
    /// alcanza, y el límite exacto de la ventana sigue siendo válido.
    #[test]
    fn unit_validate_authorization_token_at_judges_with_the_injected_clock() {
        let issued_at = Utc::now();
        let token = AuthorizationToken {
            value: "tk".into(),
            created_at: issued_at,
        };

        // Vivo mientras la ventana no se agote, incluso si el token se
        // emite «en el futuro» del reloj de pared.
        assert!(validate_authorization_token_at(&token, issued_at).is_ok());
        assert!(
            validate_authorization_token_at(&token, issued_at + chrono::Duration::hours(1)).is_ok(),
            "the exact window boundary is still inside"
        );
        assert!(matches!(
            validate_authorization_token_at(&token, issued_at + chrono::Duration::hours(2)),
            Err(ApplyError::AuthorizationInvalid)
        ));
        assert!(matches!(
            validate_authorization_token_at(&token, issued_at - chrono::Duration::hours(2)),
            Err(ApplyError::AuthorizationInvalid)
        ));
    }

    #[test]
    fn unit_apply_rejects_traversal_at_apply_one_level() {
        let tmp = TempDir::new().unwrap();
        let edit = PlannedEdit {
            file: tmp.path().join("../escape.rs"),
            byte_range: 0..0,
            replacement: "y".to_string(),
            deterministic: true,
        };
        let res = apply_one(&edit, tmp.path(), &tmp.path().join("stage"));
        assert!(matches!(res, Err(ApplyError::TraversalEscape { .. })));
    }

    #[test]
    fn unit_apply_rejects_symlink_to_outside_workspace() {
        let tmp = TempDir::new().unwrap();
        let link = tmp.path().join("link.txt");
        std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();
        let edit = PlannedEdit {
            file: link.clone(),
            byte_range: 0..0,
            replacement: "new".to_string(),
            deterministic: true,
        };
        let res = apply_one(&edit, tmp.path(), &tmp.path().join("stage"));
        assert!(matches!(res, Err(ApplyError::SymlinkEscape { .. })));
    }

    /// AAT-B5-01 — la escritura pasa por el staging aunque la raíz
    /// llegue con otro nombre (un symlink de por medio): el fichero del
    /// plan está escrito canónicamente y la raíz que el caller pasa no
    /// lo contiene textualmente. Antes del fix, el `strip_prefix`
    /// fallaba y el stage degeneraba en escribir EN SITIO sobre el
    /// original, sin copia previa ni rename atómico. El observable: el
    /// staging no puede existir (es un fichero), así que un stage real
    /// falla y el original queda intacto; la escritura en sitio del
    /// defecto prosperaría y lo reemplazaría.
    #[cfg(unix)]
    #[test]
    fn unit_apply_one_writes_through_the_staging_even_when_the_root_arrives_under_another_name() {
        let tmp = TempDir::new().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink("real", &link).unwrap();
        let file = real.join("a.rs");
        std::fs::write(&file, "hello world\n").unwrap();

        let staging = tmp.path().join("stage");
        std::fs::write(&staging, b"not a directory").unwrap();

        let edit = PlannedEdit {
            file: file.clone(),
            byte_range: 6..11,
            replacement: "safe".to_string(),
            deterministic: true,
        };
        let res = apply_one(&edit, &link, &staging).map(|rec| rec.path);
        assert!(
            matches!(res, Err(ApplyError::IoError { .. })),
            "a real staging write must fail when the staging dir cannot \
             exist, got {res:?}"
        );
        // Sin staging no hay edición: el original queda intacto.
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "hello world\n",
            "the original must not be written in place"
        );
    }

    #[test]
    fn unit_compute_workspace_state_digest_changes_with_file_content() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("a.rs");
        fs::write(&file, "old_a").unwrap();
        let plan = build_plan(
            tmp.path().to_path_buf(),
            AnalysisBasis::for_workspace(tmp.path().to_path_buf()),
            vec![file.clone()],
            vec![PlannedEdit {
                file: file.clone(),
                byte_range: 0..5,
                replacement: "new_a".to_string(),
                deterministic: true,
            }],
            vec![],
            vec![],
        )
        .unwrap();
        let d1 = compute_workspace_state_digest(&plan);
        fs::write(&file, "old_a_modified").unwrap();
        let d2 = compute_workspace_state_digest(&plan);
        assert_ne!(d1, d2, "digest MUST change when bytes change");
    }

    /// El método `apply` (alto nivel) con un plan válido debe
    /// splicer el `byte_range` y commit el resultado.
    #[test]
    fn unit_apply_full_happy_path() {
        let tmp = TempDir::new().unwrap();
        let plan = simple_plan(&tmp);
        let digest = compute_workspace_state_digest(&plan);
        let token = AuthorizationToken {
            value: "tk-test".to_string(),
            created_at: Utc::now(),
        };
        let outcome = apply(&plan, &digest, &token, tmp.path()).expect("apply");
        assert!(outcome.committed);
        assert_eq!(outcome.applied_files.len(), 1);
        let content = fs::read_to_string(tmp.path().join("a.rs")).unwrap();
        assert_eq!(content, "hello Rust");
        // staging limpio
        let staging_base = tmp.path().join(APPLY_STAGING_DIR);
        assert!(
            !staging_base.join(plan.id.to_string()).exists(),
            "staging dir must be cleaned on success"
        );
    }
}
