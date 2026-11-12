//! TASK-220 — el apply del CLI comprueba el plan contra el estado que
//! vio el preview.
//!
//! Hoy (hallazgo H6) el CLI recomputa `compute_workspace_state_digest`
//! justo antes de llamar a `apply`, que lo vuelve a computar: la
//! comparación es tautológica y la guarda «el workspace cambió desde
//! el preview» nunca salta. El token, además, se acuña con
//! `Utc::now()` en el momento del apply, así que tampoco puede
//! caducar. El digest y el token que `apply` comprueba tienen que ser
//! los que emitió el preview, persistidos junto al plan.

use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use clap::Parser;

use cognicode_cli::cli::Cli;
use cognicode_cli::cli::commands::CommandExecutor;
use cognicode_core::application::analysis_basis::AnalysisBasis;
use cognicode_core::refactor::apply::compute_workspace_state_digest;
use cognicode_core::refactor::plan::{
    PlannedEdit, build_plan, load_plan_from_file, save_plan_to_file,
};

use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Workspace de un solo fichero y plan con una edición sobre él,
/// persistido en `plan.json` con su digest calculado (el loader lo
/// verifica).
fn workspace_with_plan() -> (TempDir, PathBuf, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let target = tmp.path().join("a.rs");
    fs::write(&target, "hello world\n").expect("write target");

    let plan = build_plan(
        tmp.path().to_path_buf(),
        AnalysisBasis::for_workspace(tmp.path().to_path_buf()),
        vec![target.clone()],
        vec![PlannedEdit {
            file: target.clone(),
            byte_range: 6..11,
            replacement: "safe".to_string(),
            deterministic: true,
        }],
        vec![],
        vec![],
    )
    .expect("build plan");
    let plan_path = tmp.path().join("plan.json");
    save_plan_to_file(&plan, &plan_path).expect("save plan");
    (tmp, plan_path, target)
}

fn refactor_cli(plan_path: &std::path::Path, apply: bool, token: Option<&str>) -> Cli {
    let mut args = vec!["cognicode".to_string(), "refactor".to_string()];
    if apply {
        args.push("--apply".to_string());
    }
    args.push("--plan-file".to_string());
    args.push(plan_path.to_string_lossy().to_string());
    if let Some(t) = token {
        args.push("--authorization-token".to_string());
        args.push(t.to_string());
    }
    args.push("foo".to_string());
    Cli::try_parse_from(args).expect("clap parses the refactor invocation")
}

/// El valor del token que el preview emitió, tal como lo recibió el
/// operador fuera de banda: de la proposal persistida.
fn token_from_proposal(proposal_path: &PathBuf) -> String {
    let raw = fs::read(proposal_path).expect("readable proposal");
    let doc: serde_json::Value = serde_json::from_slice(&raw).expect("valid proposal JSON");
    doc["token"]["value"]
        .as_str()
        .expect("the proposal carries the token value")
        .to_string()
}

// ---------------------------------------------------------------------------
// AC #1 — preview → modificar un fichero objetivo → apply se rechaza
// por estado cambiado. Hoy el apply PROSPERA: la guarda es tautológica
// (el digest se recomputa en el momento del apply).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cli_apply_rejects_when_the_workspace_changed_since_the_preview() {
    let (_tmp, plan_path, target) = workspace_with_plan();

    // Preview: el plan se lee y su proposal se persiste junto a él.
    CommandExecutor::execute(refactor_cli(&plan_path, false, None))
        .await
        .expect("plan preview succeeds");
    let proposal_path = plan_path.with_extension("proposal.json");
    assert!(
        proposal_path.exists(),
        "the preview must persist its proposal next to the plan"
    );
    let token = token_from_proposal(&proposal_path);

    // El workspace cambia DESPUÉS del preview.
    fs::write(&target, "hello CHANGED world\n").expect("mutate target");

    // Apply: debe rehusar, el estado ya no es el que el preview vio.
    let result = CommandExecutor::execute(refactor_cli(&plan_path, true, Some(&token))).await;
    let err = result.expect_err("apply must refuse a workspace that changed since the preview");
    assert!(
        err.to_string().contains("workspace state mismatch"),
        "the refusal must name the state mismatch, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// AC #3 — un token caducado se rechaza: el `created_at` que aplica es
// el que emitió el preview, persistido en la proposal. La proposal se
// escribe a mano con el token retrocedido 2 h y el estado actual (para
// aislar la caducidad del mismatch de estado).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cli_apply_rejects_a_token_that_expired_since_the_preview() {
    let (_tmp, plan_path, _target) = workspace_with_plan();
    let plan = load_plan_from_file(&plan_path).expect("load plan");
    let issued = Utc::now() - chrono::Duration::hours(2);
    let proposal = serde_json::json!({
        "schema_version": "cognicode.refactor.proposal/v1",
        "plan_id": plan.id.to_string(),
        "plan_digest": plan.digest,
        "workspace_state_digest": compute_workspace_state_digest(&plan),
        "token": {
            "value": "tok-123",
            "created_at": issued.to_rfc3339(),
        },
    });
    let proposal_path = plan_path.with_extension("proposal.json");
    fs::write(
        &proposal_path,
        serde_json::to_vec(&proposal).expect("proposal json"),
    )
    .expect("write proposal");

    let result = CommandExecutor::execute(refactor_cli(&plan_path, true, Some("tok-123"))).await;
    let err = result.expect_err("an expired token must be refused");
    assert!(
        err.to_string().contains("authorization_token invalid"),
        "the refusal must name the expired token, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// AC #4 — el camino feliz preview → apply sin cambios sigue aplicando.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cli_apply_still_applies_when_nothing_changed_since_the_preview() {
    let (_tmp, plan_path, target) = workspace_with_plan();

    CommandExecutor::execute(refactor_cli(&plan_path, false, None))
        .await
        .expect("plan preview succeeds");
    let token = token_from_proposal(&plan_path.with_extension("proposal.json"));

    CommandExecutor::execute(refactor_cli(&plan_path, true, Some(&token)))
        .await
        .expect("the happy path preview → apply still applies");
    assert_eq!(
        fs::read_to_string(&target).expect("read target"),
        "hello safe\n",
        "the edit was applied through the CLI"
    );
}

// ---------------------------------------------------------------------------
// AC #5 — el contrato cambia de forma explícita: sin proposal, el apply
// se rehúsa y dice cómo regenerarla. La compatibilidad con el
// comportamiento anterior era la guarda que nunca saltaba; la
// deprecación está en el mensaje, y el camino documentado (preview →
// apply) es el que AC #4 prueba.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cli_apply_without_a_proposal_file_refuses_with_guidance() {
    let (_tmp, plan_path, _target) = workspace_with_plan();
    // Sin preview previo: no hay proposal que aplique.

    let result = CommandExecutor::execute(refactor_cli(&plan_path, true, Some("tok-123"))).await;
    let err = result.expect_err("apply without a proposal must be refused");
    let msg = err.to_string();
    assert!(
        msg.contains("proposal") && msg.contains("preview"),
        "the refusal must point at the preview that must run first, got: {msg}"
    );
}
