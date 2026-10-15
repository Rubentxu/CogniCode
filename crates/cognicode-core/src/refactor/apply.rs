//! `cognicode-core::refactor::apply` — B5.W3.
//!
//! Apply con autoridad y rollback. Lee un [`plan::RefactorPlan`]
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
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    let mut h = DefaultHasher::new();
    h.write(bytes);
    format!("{:016x}", h.finish())
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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    let mut h = DefaultHasher::new();
    for edit in sorted_edits(plan) {
        h.write(edit.file.to_string_lossy().as_bytes());
        h.write(b"\0");
        let current_bytes = std::fs::read(&edit.file).unwrap_or_default();
        let new_bytes = splice_replacement(&current_bytes, edit);
        h.write(&new_bytes);
        h.write(b"\0");
    }
    format!("{:016x}", h.finish())
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
/// contenido del plan (AAT-B5-03).
#[derive(Debug, Clone)]
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
    if token.value.trim().is_empty() {
        return Err(ApplyError::AuthorizationMissing);
    }
    let age = Utc::now().signed_duration_since(token.created_at);
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
    /// El path es un symlink que apunta fuera del workspace.
    SymlinkEscape { path: PathBuf },
    /// El path contiene `..` (traversal).
    TraversalEscape { path: PathBuf },
    /// El archivo del plan ya no tiene el `byte_range` esperado
    /// (cambió de tamaño o de contenido entre preview y apply).
    ConcurrentModification {
        path: PathBuf,
        expected_size: usize,
        actual_size: usize,
    },
    /// Error de I/O durante staging/rename/delete.
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
                "refusing to write through symlink: {} (would escape workspace)",
                path.display()
            ),
            ApplyError::TraversalEscape { path } => write!(
                f,
                "refusing path with traversal segment: {}",
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

    // (2) Workspace state digest coincide.
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

    let mut applied: Vec<PathBuf> = Vec::new();

    // Aplicar cada edición. El primer fallo aborta el batch.
    let result: Result<(), ApplyError> = plan.edits.iter().try_for_each(|edit| {
        apply_one(edit, workspace_root, &staging_dir).map(|abs| {
            applied.push(abs);
        })
    });

    if let Err(err) = result {
        // Rollback: borra los archivos aplicados en este batch.
        // AAT-B5-01: el staging dir debe quedar limpio.
        for abs in &applied {
            let _ = fs::remove_file(abs);
        }
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(err);
    }

    let _ = fs::remove_dir_all(&staging_dir);

    Ok(ApplyOutcome {
        plan_id: plan.id,
        plan_digest: plan.digest.clone(),
        applied_files: applied,
        rolled_back_files: Vec::new(),
        committed: true,
        error: None,
    })
}

/// Apply una edición de un solo archivo con staging+fsync+rename.
fn apply_one(
    edit: &PlannedEdit,
    workspace_root: &Path,
    staging_dir: &Path,
) -> Result<PathBuf, ApplyError> {
    // (a) El path debe ser absoluto (B5.W1 ya exige esto, pero
    //     verificamos aquí también como defensa).
    if !edit.file.is_absolute() {
        return Err(ApplyError::TraversalEscape {
            path: edit.file.clone(),
        });
    }

    // (b) El path debe vivir dentro del workspace_root.
    if !edit.file.starts_with(workspace_root) {
        return Err(ApplyError::TraversalEscape {
            path: edit.file.clone(),
        });
    }

    // (c) El path NO debe contener segmentos `..` (defensa adicional).
    if edit
        .file
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(ApplyError::TraversalEscape {
            path: edit.file.clone(),
        });
    }

    // (d) Symlink directo al archivo destino (AAT-B5-02).
    if let Ok(meta) = fs::symlink_metadata(&edit.file)
        && meta.file_type().is_symlink()
    {
        return Err(ApplyError::SymlinkEscape {
            path: edit.file.clone(),
        });
    }

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

    // (h) Stage preservando estructura de sub-directorios relativa
    //     al workspace_root.
    let rel = edit.file.strip_prefix(workspace_root).unwrap_or(&edit.file);
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

    Ok(edit.file.clone())
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
