//! B5.W4 — Post-apply revalidation, evidence not verdict.
//!
//! Que cierra este WU:
//!   - `cognicode_core::refactor::revalidate::RevalidationReport` siempre
//!     arranca con `RevalidationVerdict::VerdictRefused` y la razon
//!     canonica (B4.W5 spine: composite queries return evidence, not
//!     verdicts; investigate each piece). AAT-B5-04.
//!   - `build_revalidation_plan(plan, outcome)` deriva `what_changed`
//!     desde `outcome.applied_files` del `ApplyOutcome` de B5.W3.
//!   - `run_revalidation(rp, workspace_root)` NO ejecuta todavia ningun
//!     analysis incremental: no hay extractor cableado sobre los archivos
//!     cambiados. El informe lo dice —`executed_checks` vacio,
//!     `not_executed_reason` con el motivo, y el bloque de hallazgos con
//!     `searched: false`— en vez de nombrar comprobaciones que nadie
//!     corrio.
//!   - `safe_refactor_revalidate` NO aparece en el catalogo: sin extractor
//!     ni indice `plan_id -> plan` no hay una superficie honesta que
//!     anunciar, y anunciarla seria volver al fantasma que este WU
//!     retira (medido 2026-10-18; lo fija
//!     `revalidation_is_not_advertised_over_mcp`).
//!   - El comando `refactor apply` del CLI publica el informe tras un
//!     apply correcto: la salida del apply dice que se verifico y que no.
//!
//! Politica B5.W4 (NO se debilita bajo ninguna circunstancia):
//!   - `RevalidationVerdict::InvariantsHold` SOLO se emite si hay un
//!     `evidence_digest` registrado. El default es `VerdictRefused`.
//!   - `run_revalidation` es PURA respecto al filesystem: no invoca
//!     `Command::new`, no usa `std::process`, no hace shell-out.
//!   - El revalidation report es evidencia, no verdict.

use std::collections::HashSet;
use std::path::PathBuf;

use cognicode_core::application::analysis_basis::AnalysisBasis;
use cognicode_core::interface::catalog::load_tool_catalog;
use cognicode_core::refactor::apply::ApplyOutcome;
use cognicode_core::refactor::plan::RefactorPlan;
use cognicode_core::refactor::revalidate::IncrementalAnalysis;
use cognicode_core::refactor::revalidate::{
    REVALIDATION_NO_CHECK_EXECUTED_REASON, REVALIDATION_VERDICT_REFUSED_REASON,
    RevalidationVerdict, build_revalidation_plan, run_revalidation,
};
use uuid::Uuid;

// =============================================================================
// Helpers
// =============================================================================

fn stub_plan() -> RefactorPlan {
    RefactorPlan {
        id: Uuid::new_v4(),
        workspace_identity: ".".to_string(),
        basis: AnalysisBasis::for_workspace(PathBuf::from(".")),
        affected_files: Vec::new(),
        edits: Vec::new(),
        preconditions: Vec::new(),
        risks: Vec::new(),
        created_at: chrono::Utc::now(),
        digest: "deadbeef".to_string(),
    }
}

fn stub_outcome(plan_id: Uuid, files: Vec<PathBuf>) -> ApplyOutcome {
    ApplyOutcome {
        plan_id,
        plan_digest: "deadbeef".to_string(),
        applied_files: files,
        rolled_back_files: Vec::new(),
        committed: true,
        error: None,
    }
}

fn catalog_names() -> HashSet<String> {
    load_tool_catalog().into_iter().map(|d| d.name).collect()
}

// =============================================================================
// Tests (>= 8 total, AAT-B5-04 spine)
// =============================================================================

/// AAT-B5-04: el primer `RevalidationReport` SIEMPRE carga
/// `RevalidationVerdict::VerdictRefused`. Una falla de verificacion
/// posterior NUNCA se declara como exito.
#[test]
fn revalidation_always_uses_verdict_refused_default() {
    let plan = stub_plan();
    let outcome = stub_outcome(plan.id, vec![PathBuf::from("src/lib.rs")]);
    let rp = build_revalidation_plan(&plan, &outcome);
    let r = run_revalidation(&rp, PathBuf::from(".").as_path());
    match &r.verdict {
        RevalidationVerdict::VerdictRefused { reason } => {
            assert_eq!(
                *reason, REVALIDATION_VERDICT_REFUSED_REASON,
                "the canonical reason is part of the B4.W5/B5.W4 spine"
            );
        }
        RevalidationVerdict::InvariantsHold { evidence_digest } => {
            panic!(
                "default MUST be VerdictRefused; got InvariantsHold \
                 without recorded evidence_digest (got {evidence_digest:?})"
            );
        }
        RevalidationVerdict::InvariantsDrift { diff } => {
            panic!("default MUST be VerdictRefused; got InvariantsDrift with diff={diff:?}");
        }
    }
    assert!(
        r.verdict_refused(),
        "verdict_refused() helper must return true"
    );
}

/// Paridad con apply: `what_changed` del revalidation plan viene
/// exactamente de `apply_outcome.applied_files`.
#[test]
fn revalidation_report_includes_what_changed_files() {
    let plan = stub_plan();
    let applied = vec![PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")];
    let outcome = stub_outcome(plan.id, applied.clone());
    let rp = build_revalidation_plan(&plan, &outcome);
    assert_eq!(
        rp.what_changed, applied,
        "what_changed MUST equal applied_files (apply outcome parity)"
    );
}

/// Hard contract: `run_revalidation` NO invoca `Command::new`, ni
/// `std::process::*`, ni shell-out. La verificacion es evidencia, no
/// una shell. Esto se garantiza por construccion (el modulo SOLO
/// computa digests) y se testea leyendo el codigo fuente: NO debe
/// aparecer `Command::new` ni `std::process` en el source.
#[test]
fn revalidation_does_not_execute_arbitrary_commands() {
    let src = include_str!("../../src/refactor/revalidate.rs");
    assert!(
        !src.contains("Command::new") && !src.contains("std::process"),
        "revalidate.rs MUST NOT shell out: a finding or comment is data, \
         not an instruction. Found process:: / Command::new in source."
    );
}

/// B5.W4: las incremental analyses corren SOLO sobre `what_changed`,
/// no sobre el workspace entero. `analyzed_files` cuenta los
/// archivos cambiados, no los del workspace.
#[test]
fn revalidation_uses_incremental_analysis_only() {
    let plan = stub_plan();
    let outcome = stub_outcome(
        plan.id,
        vec![PathBuf::from("src/foo.rs"), PathBuf::from("src/bar.rs")],
    );
    let rp = build_revalidation_plan(&plan, &outcome);
    let r = run_revalidation(&rp, PathBuf::from(".").as_path());
    assert_eq!(
        r.analyzed_files, 2,
        "analyzed_files counts only the changed files, not the workspace"
    );
    assert!(
        r.incremental_analyses_count_is_zero_for_default_run(),
        "default run returns an empty incremental_analyses count (analyses \
         wire in once B5.W3 + B4.W1/W2/W3 extractors land together)"
    );
}

/// MEDIDO 2026-10-18. `safe_refactor_revalidate` se retiro del catalogo.
///
/// El dominio B5.W4 existe y sus tests de nucleo siguen verdes, pero no
/// hay forma honesta de exponerlo por MCP: `run_revalidation` es un stub,
/// no existe indice `plan_id -> plan` que resolver, y el texto del
/// catalogo describia un servicio que no existe. Anunciarlo hacia un
/// cliente era peor que no anunciarlo.
///
/// Precedente en este repo: `get_graph_report` se resolvio quitandolo del
/// catalogo, no despachandolo. La invariante que lo sustituye esta en
/// `mcp_roundtrip_tests::tool_surface_parity::every_advertised_tool_has_a
/// _dispatch_arm`: una herramienta anunciada tiene brazo de dispatch, y
/// este test ya no puede volver a fijarse en una que no existe.
#[test]
fn revalidation_is_not_advertised_over_mcp() {
    let names = catalog_names();
    assert!(
        !names.contains("safe_refactor_revalidate"),
        "safe_refactor_revalidate has no honest MCP surface; announcing it \
         again re-creates the ghost this unit removed. found tools: {:?}",
        names
    );
}

/// `executed_checks` nombra SOLO las comprobaciones que se ejecutaron.
///
/// Este test fijaba la mentira: el informe declaraba tres comprobaciones
/// (arquitectura, quality findings e impacto) y no ejecutaba ninguna. Un
/// consumidor que lee `executed_checks` cree que hay evidencia detras, y
/// la lista vacia es lo unico que no afirma nada que nadie midio.
#[test]
fn executed_checks_names_only_what_ran() {
    let plan = stub_plan();
    let outcome = stub_outcome(plan.id, vec![PathBuf::from("a.rs")]);
    let rp = build_revalidation_plan(&plan, &outcome);
    let r = run_revalidation(&rp, PathBuf::from(".").as_path());
    assert!(
        r.executed_checks.is_empty(),
        "a report that names a check it did not run is a claim nobody \
         measured; got {:?}",
        r.executed_checks
    );
    assert_eq!(
        r.not_executed_reason.as_deref(),
        Some(REVALIDATION_NO_CHECK_EXECUTED_REASON),
        "the report says WHY nothing ran"
    );
}

/// El bloque de hallazgos distingue «se busco y no se encontro» de
/// «no se busco».
///
/// Una lista vacia de hallazgos con `searched: false` no es «no se
/// encontro nada»: es «nadie miro». El bloque lo dice.
#[test]
fn the_findings_block_says_when_nothing_was_searched() {
    let plan = stub_plan();
    let outcome = stub_outcome(plan.id, vec![PathBuf::from("a.rs")]);
    let rp = build_revalidation_plan(&plan, &outcome);
    let r = run_revalidation(&rp, PathBuf::from(".").as_path());
    assert!(
        !r.findings.searched,
        "nobody searched the changed files, and the block says so"
    );
    assert!(
        r.findings.findings.is_empty(),
        "an empty list is consistent with «nobody looked»"
    );
}

/// La revalidacion tiene una casa de produccion.
///
/// Una funcion que solo sus propios tests ejercitan no es una capacidad:
/// es una promesa. El comando `refactor apply` del CLI la llama tras un
/// apply correcto y publica el informe, de modo que la salida del apply
/// dice que se verifico y que no.
#[test]
fn the_revalidation_has_a_production_home() {
    let source = include_str!("../../src/interface/cli/commands.rs");
    assert!(
        source.contains("run_revalidation"),
        "the CLI's `refactor apply` must call the revalidation; a function \
         that only its own tests exercise is a promise nobody keeps"
    );
    assert!(
        source.contains("build_revalidation_plan"),
        "the CLI's `refactor apply` must build the revalidation plan from \
         the plan and the apply outcome"
    );
}

/// El verdict NO serializa como booleano: es un enum con tres
/// variantes. Si un consumer lo serializa y ve `true`/`false`,
/// significa que alguien aplano el contrato — el pin lo evita.
#[test]
fn verdict_serializes_as_enum_not_as_bool() {
    let r = RevalidationVerdict::VerdictRefused {
        reason: REVALIDATION_VERDICT_REFUSED_REASON.to_string(),
    };
    let json = serde_json::to_string(&r).expect("verdict serializable");
    assert!(
        json.contains("verdict_refused") && json.contains(REVALIDATION_VERDICT_REFUSED_REASON),
        "verdict MUST serialise as enum variant `verdict_refused` with the \
         canonical reason; got {json}"
    );
    assert!(
        !json.contains("\"true\"") && !json.contains("\"false\""),
        "verdict MUST NOT serialise as a bare bool; got {json}"
    );
}

/// El digest del plan es estable para entradas iguales (no contiene
/// randomness ni orden de iteracion no determinista).
#[test]
fn plan_digest_is_stable_for_identical_inputs() {
    let plan_id = Uuid::nil();
    let plan = RefactorPlan {
        id: plan_id,
        workspace_identity: "/tmp/ws".to_string(),
        basis: AnalysisBasis::for_workspace(PathBuf::from("/tmp/ws")),
        affected_files: Vec::new(),
        edits: Vec::new(),
        preconditions: Vec::new(),
        risks: Vec::new(),
        created_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
        digest: "stable".to_string(),
    };
    let outcome = stub_outcome(plan_id, vec![PathBuf::from("a.rs")]);
    let rp1 = build_revalidation_plan(&plan, &outcome);
    let rp2 = build_revalidation_plan(&plan, &outcome);
    let r1 = run_revalidation(&rp1, PathBuf::from("/tmp/ws").as_path());
    let r2 = run_revalidation(&rp2, PathBuf::from("/tmp/ws").as_path());
    assert_eq!(
        r1.plan_digest, r2.plan_digest,
        "identical inputs MUST yield identical plan_digest"
    );
}

/// El incremental analysis enum lleva la pieza B4 que
/// corresponde: su existencia es la prueba de que el reuse bypassea
/// la duplication de shape (R-B4-W5: una sola fuente de verdad para
/// "que cambio y donde se observa").
#[test]
fn incremental_analysis_variants_carry_b4_types() {
    // Si los tipos B4 cambian, este import rompe: pin estructural.
    let _impact: Option<IncrementalAnalysis> = None;
}

/// TASK-22 — la forma publicada del bloque `findings` del reporte no
/// cambia al unificar `Finding`: un reporte sin hallazgos serializa los
/// mismos cuatro campos.
#[test]
fn revalidation_report_findings_block_keeps_its_published_shape() {
    let plan = stub_plan();
    let outcome = stub_outcome(plan.id, vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")]);
    let rp = build_revalidation_plan(&plan, &outcome);
    let r = run_revalidation(&rp, PathBuf::from(".").as_path());
    let json = serde_json::to_value(&r).expect("report serializable");
    assert_eq!(
        json["findings"],
        serde_json::json!({
            "findings": [],
            "searched": false,
            "total_files_scanned": 2,
            "language_breakdown": {},
            "truncated": false,
        })
    );
}
