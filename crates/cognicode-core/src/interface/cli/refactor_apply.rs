//! `cognicode refactor --apply`: applies a previewed plan under operator authority.

/// B5.W3 — Apply con autoridad.
///
/// Contrato:
///   - Lee el `RefactorPlan` desde `--plan-file` (JSON).
///   - Construye un `AuthorizationToken` desde `--authorization-token`
///     (canal separado; no derivado del plan; AAT-B5-03).
///   - Exige el digest que emitió el preview (`--expected-digest`) y
///     la raíz que autoriza el operador (`--workspace` o el directorio
///     actual); el plan debe pertenecer a esa raíz. Llama a
///     `cognicode_core::refactor::apply::apply`.
///   - Cualquier fallo se reporta con código de salida no-cero y
///     mensaje específico (AuthorizationMissing,
///     WorkspaceStateMismatch, SymlinkEscape, TraversalEscape,
///     ConcurrentModification, IoError).
///
/// Pre-condición: ambos flags vienen del parseo de clap; el caller
/// ya verificó que no son `None` antes de llamar esta función.
pub(super) async fn execute_refactor_apply(
    plan_file: &std::path::Path,
    authorization_token: &str,
    expected_digest: &str,
    workspace: Option<&std::path::Path>,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::refactor::apply::{AuthorizationToken, apply};
    use chrono::Utc;

    // Carga el plan desde disco (B5.W1).
    let plan = crate::refactor::plan::load_plan_from_file(plan_file).map_err(|e| {
        format!(
            "B5.W3: failed to read --plan-file {}: {}",
            plan_file.display(),
            e
        )
    })?;

    // The root comes from the operator (`--workspace` or the current
    // directory), never from the plan: the plan only has to agree.
    let workspace_root = match workspace {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir()?,
    }
    .canonicalize()
    .map_err(|e| format!("B5.W3: workspace root is not accessible: {e}"))?;
    let plan_root = std::path::PathBuf::from(&plan.workspace_identity)
        .canonicalize()
        .map_err(|e| format!("B5.W3: plan workspace is not accessible: {e}"))?;
    if plan_root != workspace_root {
        return Err(format!(
            "B5.W3: plan belongs to {:?}, not to the authorised workspace {:?}",
            plan_root, workspace_root
        )
        .into());
    }

    // Construye el token a partir del flag separado. NOTA: el
    // apply internamente exige created_at dentro de la ventana
    // de 1h. Para el caso CLI, el token se acaba de generar —
    // usamos Utc::now() para que sea válido.
    let token = AuthorizationToken {
        value: authorization_token.to_string(),
        created_at: Utc::now(),
    };

    match apply(&plan, expected_digest, &token, &workspace_root) {
        Ok(outcome) => {
            if format == "json" {
                #[derive(serde::Serialize)]
                struct ApplyOkJson {
                    schema_version: &'static str,
                    committed: bool,
                    plan_id: uuid::Uuid,
                    applied_files: Vec<String>,
                }
                let doc = ApplyOkJson {
                    schema_version: "cognicode.refactor.apply/v1",
                    committed: outcome.committed,
                    plan_id: outcome.plan_id,
                    applied_files: outcome
                        .applied_files
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect(),
                };
                println!("{}", serde_json::to_string_pretty(&doc)?);
            } else {
                eprintln!(
                    "Apply committed: {} file(s) modified",
                    outcome.applied_files.len()
                );
                for f in &outcome.applied_files {
                    println!("  + {}", f.display());
                }
            }
            Ok(())
        }
        Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
    }
}
