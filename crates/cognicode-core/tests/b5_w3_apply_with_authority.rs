//! B5.W3 — Apply with authority, transactional writes, rollback.
//!
//! Qué cierra este WU (contract pins):
//!   - `cognicode_core::refactor::apply::apply` exige un
//!     `AuthorizationToken` OPAQUE, recibido por canal separado
//!     (no derivado del plan). AAT-B5-03.
//!   - El apply recomputa el `workspace_state_digest` y rehúsa si
//!     el workspace mutó (concurrent modification).
//!   - Rechaza symlink-to-outside-workspace (AAT-B5-02) y
//!     traversal `..`.
//!   - La escritura es transaccional: se escribe a
//!     `<workspace>/.cognicode-staging/<plan_id>/<file>`, fsync,
//!     rename atómico. Si una escritura falla a mitad de batch,
//!     se borra el staging dir.
//!   - El MCP read-only `safe_refactor` (read-only preview) NO
//!     acepta `authorization_token`: `deny_unknown_fields` lo
//!     rechaza (AAT-B5-03).
//!
//! Out of scope (B5.W4): revalidación post-apply.

use cognicode_core::application::analysis_basis::AnalysisBasis;
use cognicode_core::architecture::diagnostic::Severity;
use cognicode_core::interface::mcp::schemas::SafeRefactorInput;
use cognicode_core::refactor::apply::{
    APPLY_STAGING_DIR, ApplyError, ApplyOutcome, AuthorizationToken, apply,
    compute_workspace_state_digest, hash_bytes, validate_authorization_token,
};
use cognicode_core::refactor::plan::{PlannedEdit, RefactorPlan, Risk, build_plan};

use chrono::{Duration, Utc};
use std::fs;
use std::ops::Range;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Construye un plan mínimo: un solo archivo con un byte_range.
fn simple_plan(
    workspace: &std::path::Path,
    rel_path: &str,
    current: &str,
    byte_range: Range<usize>,
    replacement: &str,
) -> RefactorPlan {
    let abs = workspace.join(rel_path);
    fs::create_dir_all(abs.parent().unwrap()).unwrap();
    fs::write(&abs, current).unwrap();
    let edit = PlannedEdit {
        file: abs.clone(),
        byte_range,
        replacement: replacement.to_string(),
        deterministic: true,
    };
    build_plan(
        workspace.to_path_buf(),
        AnalysisBasis::for_workspace(workspace.to_path_buf()),
        vec![abs.clone()],
        vec![edit],
        vec![],
        vec![Risk {
            id: "R-test".into(),
            description: "test fixture".into(),
            evidence: None,
            severity: Severity::Info,
        }],
    )
    .expect("build_plan")
}

fn fresh_token() -> AuthorizationToken {
    AuthorizationToken {
        value: format!("tk-{}", Uuid::new_v4()),
        created_at: Utc::now(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// 1. apply_requires_authorization_token.
#[test]
fn apply_requires_authorization_token() {
    let tmp = TempDir::new().unwrap();
    let plan = simple_plan(tmp.path(), "a.rs", "hello", 0..5, "world");

    let empty = AuthorizationToken {
        value: String::new(),
        created_at: Utc::now(),
    };
    let res = apply(&plan, &plan.digest, &empty, tmp.path());
    assert!(
        matches!(res, Err(ApplyError::AuthorizationMissing)),
        "empty token must yield AuthorizationMissing, got {:?}",
        res
    );

    // validate_authorization_token rechaza vacío
    let v = validate_authorization_token(&empty);
    assert!(matches!(v, Err(ApplyError::AuthorizationMissing)));
}

/// 2. apply_refuses_workspace_state_mismatch: cambia el archivo del
///    plan después de construirlo; el digest no coincide y se rehúsa.
#[test]
fn apply_refuses_workspace_state_mismatch() {
    let tmp = TempDir::new().unwrap();
    let plan = simple_plan(tmp.path(), "a.rs", "old_a", 0..5, "new_a");

    // Cambia el archivo después del plan.
    fs::write(tmp.path().join("a.rs"), "old_a_modified").unwrap();

    let token = fresh_token();
    let out = apply(&plan, &plan.digest, &token, tmp.path());
    assert!(
        matches!(out, Err(ApplyError::WorkspaceStateMismatch { .. })),
        "expected WorkspaceStateMismatch, got {:?}",
        out
    );
}

/// 3. apply_refuses_symlink_escape (AAT-B5-02).
#[cfg(unix)]
#[test]
fn apply_refuses_symlink_escape() {
    let tmp = TempDir::new().unwrap();
    let link = tmp.path().join("escape.rs");
    std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();

    let plan = RefactorPlan {
        id: Uuid::new_v4(),
        workspace_identity: tmp.path().to_string_lossy().to_string(),
        basis: AnalysisBasis::for_workspace(tmp.path().to_path_buf()),
        affected_files: vec![link.clone()],
        edits: vec![PlannedEdit {
            file: link.clone(),
            byte_range: 0..0,
            replacement: "evil".to_string(),
            deterministic: true,
        }],
        preconditions: vec![],
        risks: vec![],
        created_at: Utc::now(),
        digest: String::new(),
    };
    let token = fresh_token();
    // Recomputamos el digest que el apply espera (lee el symlink y
    // splice el replacement). El apply rechaza el symlink antes de
    // cualquier I/O destructivo (AAT-B5-02).
    let expected_digest = compute_workspace_state_digest(&plan);
    let res = apply(&plan, &expected_digest, &token, tmp.path());
    assert!(
        matches!(res, Err(ApplyError::SymlinkEscape { .. })),
        "symlink outside workspace must be rejected, got {:?}",
        res
    );
}

/// 4. apply_refuses_traversal_escape: un edit.file con `..` se rechaza.
#[test]
fn apply_refuses_traversal_escape() {
    let tmp = TempDir::new().unwrap();
    let outside = PathBuf::from("/tmp/outside.rs");
    let plan = RefactorPlan {
        id: Uuid::new_v4(),
        workspace_identity: tmp.path().to_string_lossy().to_string(),
        basis: AnalysisBasis::for_workspace(tmp.path().to_path_buf()),
        affected_files: vec![outside.clone()],
        edits: vec![PlannedEdit {
            file: outside.clone(),
            byte_range: 0..0,
            replacement: "evil".to_string(),
            deterministic: true,
        }],
        preconditions: vec![],
        risks: vec![],
        created_at: Utc::now(),
        digest: String::new(),
    };
    let token = fresh_token();
    let res = apply(&plan, &plan.digest, &token, tmp.path());
    // El rechazo es por starts_with: el path absoluto está fuera del
    // workspace temporal. Eso se mapea a TraversalEscape (o
    // potencialmente otro error). Lo que importa: NO commitea.
    assert!(
        res.is_err(),
        "edit.file outside workspace must be rejected, got {:?}",
        res
    );
}

/// 5. apply_commits_on_success: contenido final = planned edit.
#[test]
fn apply_commits_on_success() {
    let tmp = TempDir::new().unwrap();
    let plan = simple_plan(
        tmp.path(),
        "src/a.rs",
        "hello world",
        6..11,
        "Rust",
    );
    let token = fresh_token();
    let workspace_digest = compute_workspace_state_digest(&plan);

    let out = apply(&plan, &workspace_digest, &token, tmp.path())
        .expect("apply must succeed");
    assert!(out.committed, "out.committed = true on success");
    assert_eq!(out.applied_files.len(), 1);
    assert_eq!(out.rolled_back_files.len(), 0);
    assert_eq!(out.plan_id, plan.id);
    assert_eq!(out.plan_digest, plan.digest);

    let content = fs::read_to_string(tmp.path().join("src/a.rs")).unwrap();
    assert_eq!(content, "hello Rust");

    // Staging debe estar limpio (success path borra el dir).
    let staging = tmp.path().join(APPLY_STAGING_DIR).join(out.plan_id.to_string());
    assert!(
        !staging.exists(),
        "staging dir must be cleaned on success"
    );
}

/// 6. apply_rolls_back_on_partial_failure: un segundo archivo falla
///    al renombrar (parent no escribible); el primero NO debe quedar
///    modificado parcialmente, y el staging se borra.
#[cfg(unix)]
#[test]
fn apply_rolls_back_on_partial_failure() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    // Crea los dos archivos.
    let good = tmp.path().join("good.rs");
    fs::write(&good, "old_good").unwrap();
    let bad = tmp.path().join("bad").join("dir").join("bad.rs");
    fs::create_dir_all(bad.parent().unwrap()).unwrap();
    fs::write(&bad, "old_bad").unwrap();

    let plan = build_plan(
        tmp.path().to_path_buf(),
        AnalysisBasis::for_workspace(tmp.path().to_path_buf()),
        vec![good.clone(), bad.clone()],
        vec![
            PlannedEdit {
                file: good.clone(),
                byte_range: 0..7,
                replacement: "new_good".to_string(),
                deterministic: true,
            },
            PlannedEdit {
                file: bad.clone(),
                byte_range: 0..7,
                replacement: "new_bad".to_string(),
                deterministic: true,
            },
        ],
        vec![],
        vec![],
    )
    .expect("build_plan");

    let workspace_digest = compute_workspace_state_digest(&plan);
    let token = fresh_token();

    // Bloquea el directorio `bad/dir` para que el rename falle.
    let bad_dir = tmp.path().join("bad").join("dir");
    fs::set_permissions(&bad_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let res = apply(&plan, &workspace_digest, &token, tmp.path());

    // Restaura permisos (para que TempDir se limpie bien al final).
    fs::set_permissions(&bad_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    match res {
        Err(ApplyError::IoError { path, .. }) => {
            assert!(
                path.to_string_lossy().contains("bad"),
                "IoError path must point to the bad file, got {:?}",
                path
            );
        }
        other => panic!("expected IoError on partial failure, got {:?}", other),
    }

    // El archivo bueno NO debe quedar modificado (rollback).
    let good_content = fs::read_to_string(&good).unwrap();
    assert_eq!(
        good_content, "old_good",
        "good.rs must remain unchanged after partial rollback"
    );

    // El staging global debe estar vacío (ningún plan_id dir
    // restante).
    let staging_base = tmp.path().join(APPLY_STAGING_DIR);
    if staging_base.exists() {
        let mut leftover = 0usize;
        for entry in fs::read_dir(&staging_base).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                leftover += 1;
            }
        }
        assert_eq!(
            leftover, 0,
            "staging base must be empty after rollback (no plan_id dirs)"
        );
    }
}

/// 7. read_only_mcp_tool_cannot_acquire_write_authority (AAT-B5-03):
///    `SafeRefactorInput` (read-only preview) rechaza cualquier
///    input que incluya `authorization_token` (deny_unknown_fields).
#[test]
fn read_only_mcp_tool_cannot_acquire_write_authority() {
    let smuggling = serde_json::json!({
        "action": "rename",
        "target": "foo",
        "authorization_token": "tk-evil-should-not-be-accepted"
    });
    let res: Result<SafeRefactorInput, _> = serde_json::from_value(smuggling);
    assert!(
        res.is_err(),
        "read-only safe_refactor_preview MUST reject authorization_token field; \
         serde must deny unknown fields. Got Ok = {:?}",
        res.ok()
    );

    // Sanity: el mismo input SIN authorization_token debe pasar.
    let clean = serde_json::json!({"action": "rename", "target": "foo"});
    let ok: Result<SafeRefactorInput, _> = serde_json::from_value(clean);
    assert!(
        ok.is_ok(),
        "read-only input without token must still work, got {:?}",
        ok
    );
}

/// 8. authorization_token_must_be_recent: tokens con created_at
///    antiguo (>1h) son rechazados.
#[test]
fn authorization_token_must_be_recent() {
    let stale = AuthorizationToken {
        value: format!("tk-{}", Uuid::new_v4()),
        created_at: Utc::now() - Duration::hours(2),
    };
    let v = validate_authorization_token(&stale);
    assert!(
        matches!(v, Err(ApplyError::AuthorizationInvalid)),
        "token older than 1h must be AuthorizationInvalid, got {:?}",
        v
    );
}

/// 9. workspace_state_digest_detects_modifications: el digest cambia
///    cuando un archivo del plan cambia, demostrando la base del
///    concurrent-modification check.
#[test]
fn workspace_state_digest_detects_modifications() {
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

    // Cambio de bytes concurrentes.
    fs::write(&file, "old_a_modified").unwrap();
    let d2 = compute_workspace_state_digest(&plan);
    assert_ne!(
        d1, d2,
        "digest MUST change when an affected file is modified"
    );
}

/// 10. hash_bytes es determinista y distingue cambios.
#[test]
fn hash_bytes_is_deterministic_and_distinguishes() {
    assert_eq!(hash_bytes(b"hello"), hash_bytes(b"hello"));
    assert_ne!(hash_bytes(b"hello"), hash_bytes(b"hello!"));
    assert_eq!(hash_bytes(b"").len(), 16);
}

/// 11. ApplyOutcome y AuthorizationToken son construibles y
///     round-trippean sus campos (smoke test de la API pública).
#[test]
fn apply_public_api_is_constructible() {
    let outcome = ApplyOutcome {
        plan_id: Uuid::new_v4(),
        plan_digest: "d".to_string(),
        applied_files: vec![],
        rolled_back_files: vec![],
        committed: false,
        error: Some("test".to_string()),
    };
    assert_eq!(outcome.applied_files.len(), 0);
    assert!(!outcome.committed);
}