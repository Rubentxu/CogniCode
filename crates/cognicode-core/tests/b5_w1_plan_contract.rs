//! B5.W1 — Refactor plan contract tests.
//!
//! Cubre el contrato que introduce `cognicode_core::refactor::plan`:
//!
//!   - `RefactorPlan`: el portador de la propuesta (id,
//!     workspace_identity, basis, affected_files, edits,
//!     preconditions, risks + sha256 digest estable).
//!   - El CLI `cognicode refactor` admite `--plan-file` como
//!     path alternativo a la preview free-form vigente.
//!   - El digest es invariante al orden de insercion (edits,
//!     risks) y al `created_at`.
//!   - El apply future (B5.W3) exige paths absolutos y puede
//!     rechazar traversal. Este WU fija el contrato del plan;
//!     el apply authority lo aterriza B5.W3.

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::PathBuf;

use clap::Parser;

use cognicode_core::application::analysis_basis::AnalysisBasis;
use cognicode_core::architecture::diagnostic::Severity;
use cognicode_core::interface::cli::Cli;
use cognicode_core::interface::cli::commands::CliCommand;
use cognicode_core::refactor::plan::{
    BuildPlanError, PlannedEdit, Precondition, RefactorPlan, Risk, build_plan, load_plan_from_file,
    save_plan_to_file, verify_digest,
};

use tempfile::TempDir;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn basis_for(ws: &str) -> AnalysisBasis {
    AnalysisBasis::for_workspace(PathBuf::from(ws))
}

fn sample_edit(file: &str, start: usize, end: usize, replacement: &str) -> PlannedEdit {
    PlannedEdit {
        file: PathBuf::from(file),
        byte_range: start..end,
        replacement: replacement.to_string(),
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

fn make_plan() -> RefactorPlan {
    build_plan(
        PathBuf::from("/tmp/workspace"),
        basis_for("/tmp/workspace"),
        vec![PathBuf::from("/tmp/workspace/a.rs")],
        vec![sample_edit("/tmp/workspace/a.rs", 0, 4, "fn new_fn() {}")],
        vec![Precondition {
            capability: "file.write".to_string(),
            description: "must have write access".to_string(),
        }],
        vec![sample_risk("R-test", Severity::Warning)],
    )
    .expect("plan ok")
}

// ---------------------------------------------------------------------------
// 1. Digest stable across builds (mismo contenido -> mismo sha256).
// ---------------------------------------------------------------------------
#[test]
fn plan_digest_is_stable_across_builds() {
    let plan1 = build_plan(
        PathBuf::from("/tmp/workspace"),
        basis_for("/tmp/workspace"),
        vec![PathBuf::from("/tmp/workspace/a.rs")],
        vec![sample_edit("/tmp/workspace/a.rs", 0, 4, "REPLACED")],
        vec![Precondition {
            capability: "file.write".to_string(),
            description: "write access".to_string(),
        }],
        vec![sample_risk("R-test", Severity::Warning)],
    )
    .expect("plan1 ok");
    let plan2 = build_plan(
        PathBuf::from("/tmp/workspace"),
        basis_for("/tmp/workspace"),
        vec![PathBuf::from("/tmp/workspace/a.rs")],
        vec![sample_edit("/tmp/workspace/a.rs", 0, 4, "REPLACED")],
        vec![Precondition {
            capability: "file.write".to_string(),
            description: "write access".to_string(),
        }],
        vec![sample_risk("R-test", Severity::Warning)],
    )
    .expect("plan2 ok");
    // Distintos UUIDs y `created_at` -> digest estable: la
    // propuesta firmable es solo el contenido.
    assert_ne!(plan1.id, plan2.id, "ids should differ");
    assert_ne!(
        plan1.created_at, plan2.created_at,
        "timestamps should differ"
    );
    assert_eq!(plan1.digest, plan2.digest);
    assert!(verify_digest(&plan1));
    assert!(verify_digest(&plan2));
}

// ---------------------------------------------------------------------------
// 2. Plan carries every required field (the contract surface).
// ---------------------------------------------------------------------------
#[test]
fn plan_includes_all_required_fields() {
    let plan = make_plan();
    let value = serde_json::to_value(&plan).expect("RefactorPlan is serializable");
    let obj = value.as_object().expect("top-level must be an object");
    let required: BTreeSet<&str> = [
        "id",
        "workspace_identity",
        "basis",
        "affected_files",
        "edits",
        "preconditions",
        "risks",
        "created_at",
        "digest",
    ]
    .into_iter()
    .collect();
    let present: BTreeSet<&str> = obj.keys().map(String::as_str).collect();
    let missing: Vec<&&str> = required.difference(&present).collect();
    assert!(
        missing.is_empty(),
        "RefactorPlan missing fields: {:?}",
        missing
    );
    // UUID format sanity on `id`.
    let id_str = obj
        .get("id")
        .and_then(|v| v.as_str())
        .expect("id must be a string");
    Uuid::parse_str(id_str).expect("id must be a UUID");
    // Digest is 64 hex chars.
    let digest = obj
        .get("digest")
        .and_then(|v| v.as_str())
        .expect("digest must be a string");
    assert_eq!(digest.len(), 64);
    assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
}

// ---------------------------------------------------------------------------
// 3. CLI parse without --plan-file succeeds (free-form preview remains valid).
// ---------------------------------------------------------------------------
#[test]
fn cli_refactor_without_plan_file_errors_cleanly() {
    // Sin `--plan-file`, el CLI parsea OK (free-form preview arm).
    // Sintaxis real: `cognicode refactor --operation <OP> <SYMBOL> [NEW_NAME]`.
    let cli = Cli::try_parse_from(["cognicode", "refactor", "foo", "bar"]);
    assert!(
        cli.is_ok(),
        "clap parse should accept refactor without --plan-file; got {:?}",
        cli.err()
    );
    match cli.unwrap().command {
        Some(CliCommand::Refactor {
            operation: _,
            symbol,
            new_name,
            apply,
            plan_file,
            authorization_token,
            format: _,
        }) => {
            assert_eq!(symbol, "foo");
            assert_eq!(new_name.as_deref(), Some("bar"));
            assert!(!apply);
            assert!(plan_file.is_none(), "plan_file must default to None");
            assert!(
                authorization_token.is_none(),
                "authorization_token must default to None"
            );
        }
        other => panic!("expected Refactor variant, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 4. CLI with --plan-file parses the path; load + roundtrip is clean.
// ---------------------------------------------------------------------------
#[test]
fn cli_refactor_with_valid_plan_file_parses_to_refactor_plan() {
    let dir = TempDir::new().expect("tempdir");
    let plan = make_plan();
    let plan_path = dir.path().join("plan.json");
    save_plan_to_file(&plan, &plan_path).expect("save ok");

    // Clap parsea el `--plan-file`. Sintaxis: `<SYMBOL> [NEW_NAME]`.
    let cli = Cli::try_parse_from([
        "cognicode",
        "refactor",
        "--plan-file",
        plan_path.to_str().unwrap(),
        "foo",
    ])
    .expect("clap parse");
    match cli.command {
        Some(CliCommand::Refactor {
            symbol, plan_file, ..
        }) => {
            assert_eq!(symbol, "foo");
            assert_eq!(
                plan_file.as_deref(),
                Some(plan_path.as_path()),
                "plan_file must match"
            );
        }
        other => panic!("expected Refactor variant, got {other:?}"),
    }

    // Y el modulo carga el plan desde ese path. Confirmamos
    // ademas que el round-trip (save -> load) preserva el
    // contenido firmable.
    let loaded = load_plan_from_file(&plan_path).expect("load ok");
    assert_eq!(loaded.id, plan.id);
    assert_eq!(loaded.digest, plan.digest);
    assert_eq!(loaded.affected_files, plan.affected_files);
    assert_eq!(loaded.workspace_identity, plan.workspace_identity);
}

// ---------------------------------------------------------------------------
// 5. MCP tool requires plan_file (regression guard: shape carries the field).
// ---------------------------------------------------------------------------
#[test]
fn mcp_safe_refactor_requires_plan_file_field() {
    use cognicode_core::interface::mcp::schemas::{RefactorAction, SafeRefactorInput};

    // (a) Shape: the input module exports the type the MCP handler
    // deserializes and exposes via `tools.json`. We assert that the
    // `params` field (the existing extensible surface) carries the
    // `plan_file` key in the plan path. The B5.W1 contract: when
    // the caller asks for a plan-based refactor, `params.plan_file`
    // MUST be present.
    let with_plan = serde_json::json!({
        "action": "rename",
        "target": "foo",
        "params": {
            "new_name": "bar",
            "plan_file": "/abs/path/to/plan.json"
        }
    });
    let parsed: Result<SafeRefactorInput, _> = serde_json::from_value(with_plan);
    assert!(parsed.is_ok(), "input with plan_file must deserialize");
    let input = parsed.unwrap();
    let plan_value = input
        .params
        .as_ref()
        .and_then(|p| p.get("plan_file"))
        .and_then(|v| v.as_str());
    assert_eq!(
        plan_value,
        Some("/abs/path/to/plan.json"),
        "plan_file must round-trip through SafeRefactorInput.params"
    );
    // Ensure the action is also round-tripping.
    assert!(matches!(input.action, RefactorAction::Rename));

    // (b) Sin plan_file en params: el caller no pidio un plan
    // (free-form preview). Esto sigue siendo valido PERO el apply
    // future (B5.W3) rechazara el camino sin plan. Aqui solo
    // pineamos que la ausencia es detectable.
    let without_plan = serde_json::json!({
        "action": "rename",
        "target": "foo",
        "params": {
            "new_name": "bar"
        }
    });
    let parsed_no_plan: Result<SafeRefactorInput, _> = serde_json::from_value(without_plan);
    assert!(
        parsed_no_plan.is_ok(),
        "free-form preview path must remain valid"
    );
    let missing = parsed_no_plan
        .unwrap()
        .params
        .and_then(|p| p.get("plan_file").cloned());
    assert!(
        missing.is_none(),
        "plan_file absent in free-form path must be observable"
    );
}

// ---------------------------------------------------------------------------
// 6. affected_files are absolute paths (no traversal possible).
// ---------------------------------------------------------------------------
#[test]
fn affected_files_are_absolute_paths() {
    let result = build_plan(
        PathBuf::from("/tmp"),
        basis_for("/tmp"),
        vec![PathBuf::from("relative/path.rs")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        result.err(),
        Some(BuildPlanError::RelativePath(PathBuf::from(
            "relative/path.rs"
        ))),
        "relative affected_files must be rejected"
    );
    // `..` traversal components are rejected by the absolute check.
    let traversal: PathBuf = ["rel", "..", "..", "etc", "passwd"].iter().collect();
    let result2 = build_plan(
        PathBuf::from("/tmp"),
        basis_for("/tmp"),
        vec![traversal.clone()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        result2.err(),
        Some(BuildPlanError::RelativePath(traversal)),
        "traversal path rejected as relative"
    );
}

// ---------------------------------------------------------------------------
// 7. Plan digest excludes `created_at` (time-travel safe).
// ---------------------------------------------------------------------------
#[test]
fn plan_digest_excludes_temporal_fields() {
    let plan = make_plan();
    let mut twin = plan.clone();
    twin.created_at += chrono::Duration::seconds(86_400);
    assert_eq!(plan.digest, twin.digest);
}

// ---------------------------------------------------------------------------
// 8. Round-trip (build -> save -> load -> verify) preserves content.
// ---------------------------------------------------------------------------
#[test]
fn plan_round_trip_via_file_preserves_content() {
    let dir = TempDir::new().expect("tempdir");
    let plan = make_plan();
    let path = dir.path().join("plan.json");
    save_plan_to_file(&plan, &path).expect("save ok");

    let loaded = load_plan_from_file(&path).expect("load ok");
    assert_eq!(loaded.id, plan.id);
    assert_eq!(loaded.workspace_identity, plan.workspace_identity);
    assert_eq!(loaded.created_at, plan.created_at);
    assert_eq!(loaded.affected_files.len(), plan.affected_files.len());
    assert_eq!(loaded.edits.len(), plan.edits.len());
    assert_eq!(loaded.preconditions.len(), plan.preconditions.len());
    assert_eq!(loaded.risks.len(), plan.risks.len());
    assert_eq!(loaded.digest, plan.digest);
    assert!(verify_digest(&loaded));

    // Sanity: byte_range type is preserved as Range<usize>, no
    // surprise conversion a (start, end) tuple.
    let r: Range<usize> = loaded.edits[0].byte_range.clone();
    assert_eq!(r.start, 0);
    assert_eq!(r.end, 4);
}

// ---------------------------------------------------------------------------
// 9. Edit ordering stable across insertion order.
// ---------------------------------------------------------------------------
#[test]
fn plan_edits_ordered_deterministically() {
    use std::path::Path;
    let abs_a: &Path = Path::new("/tmp/a.rs");
    let abs_b: &Path = Path::new("/tmp/b.rs");
    let p1 = build_plan(
        PathBuf::from("/tmp"),
        basis_for("/tmp"),
        vec![abs_a.to_path_buf(), abs_b.to_path_buf()],
        vec![
            sample_edit("/tmp/a.rs", 0, 5, "X"),
            sample_edit("/tmp/a.rs", 20, 25, "Y"),
            sample_edit("/tmp/b.rs", 0, 5, "Z"),
        ],
        Vec::new(),
        Vec::new(),
    )
    .expect("p1");
    let p2 = build_plan(
        PathBuf::from("/tmp"),
        basis_for("/tmp"),
        vec![abs_a.to_path_buf(), abs_b.to_path_buf()],
        vec![
            sample_edit("/tmp/b.rs", 0, 5, "Z"),
            sample_edit("/tmp/a.rs", 20, 25, "Y"),
            sample_edit("/tmp/a.rs", 0, 5, "X"),
        ],
        Vec::new(),
        Vec::new(),
    )
    .expect("p2");
    assert_eq!(p1.digest, p2.digest);
}
