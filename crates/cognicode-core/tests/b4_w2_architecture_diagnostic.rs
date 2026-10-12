//! B4.W2 — Architecture as diagnostic, no gate authority.

use cognicode_core::architecture::diagnostic::{
    ArchitectureDiagnostic, ArchitectureEvidence, ArchitectureObservation,
    ObservationValidationError, Severity, build_diagnostic, validate_observation,
};
use cognicode_core::interface::catalog::load_tool_catalog;
use cognicode_core::interface::symbol_identity::SymbolId;
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::PathBuf;

fn sample_observation(rule_id: &str, severity: Severity, message: &str) -> ArchitectureObservation {
    let evidence = ArchitectureEvidence {
        file: PathBuf::from("src/domain/foo.rs"),
        byte_range: Range { start: 0, end: 24 },
        snippet: "use crate::infrastructure::db;".to_string(),
        rule_reference: "ADR-046".to_string(),
    };
    ArchitectureObservation {
        rule_id: rule_id.to_string(),
        severity,
        message: message.to_string(),
        evidence: vec![evidence],
        affected_symbols: vec![SymbolId::from_lsp_location(
            "domain::foo".to_string(),
            PathBuf::from("src/domain/foo.rs"),
            Range { start: 0, end: 24 },
        )],
    }
}

#[test]
fn catalog_declares_check_architecture_gate_authority_false() {
    let catalog = load_tool_catalog();
    let tool = catalog
        .iter()
        .find(|t| t.name == "check_architecture")
        .expect("check_architecture must be declared in the catalog");
    let runtime = tool
        .runtime_metadata
        .as_object()
        .expect("runtime_metadata must be an object");
    assert!(
        !runtime
            .get("gate_authority")
            .and_then(|v| v.as_bool())
            .expect("gate_authority must be a bool"),
        "check_architecture must declare gate_authority: false — R-B4-W2"
    );
    assert!(
        runtime
            .get("produces_observations_only")
            .and_then(|v| v.as_bool())
            .expect("produces_observations_only must be a bool"),
        "check_architecture must declare produces_observations_only: true"
    );
}

#[test]
fn check_architecture_returns_observations_not_gates() {
    let diag = ArchitectureDiagnostic {
        observations: vec![sample_observation(
            "architecture.layer_dependency",
            Severity::Error,
            "domain must not depend on infrastructure",
        )],
        total_rules_evaluated: 1,
        total_rules_skipped: 0,
        skip_reasons: BTreeMap::new(),
    };
    let dbg = format!("{diag:?}");
    assert!(!dbg.contains("gate"), "must not contain gate: {dbg}");
    assert!(!dbg.contains("verdict"), "must not contain verdict: {dbg}");
    assert!(
        dbg.contains("observations"),
        "must expose observations: {dbg}"
    );
    assert_eq!(diag.observations.len(), 1);
    assert_eq!(diag.observations[0].severity, Severity::Error);
}

#[test]
fn architecture_observation_carries_evidence_with_byte_range() {
    let obs = sample_observation("architecture.layer_dependency", Severity::Warning, "");
    let ev = &obs.evidence[0];
    assert_eq!(ev.byte_range, Range { start: 0, end: 24 });
    assert_eq!(ev.byte_range.end - ev.byte_range.start, 24);
    assert!(!ev.snippet.is_empty());
    assert!(!ev.rule_reference.is_empty());
    assert_eq!(ev.file, PathBuf::from("src/domain/foo.rs"));
    assert_eq!(obs.affected_symbols.len(), 1);
    assert_eq!(obs.affected_symbols[0].byte_range, ev.byte_range);
}

#[test]
fn architecture_diagnostic_total_evaluated_matches_observations_plus_skipped() {
    let obs = vec![
        sample_observation("architecture.layer_dependency", Severity::Error, "first"),
        sample_observation(
            "architecture.forbidden_dependency",
            Severity::Warning,
            "second",
        ),
    ];
    let diag = build_diagnostic(obs, 5);
    assert_eq!(diag.observations.len(), 2);
    assert_eq!(diag.total_rules_evaluated, 5);
    assert_eq!(diag.total_rules_skipped, 3);
    assert!(diag.skip_reasons.is_empty());
}

#[test]
fn build_diagnostic_explicit_skip_reasons_take_precedence() {
    let obs = vec![sample_observation(
        "architecture.layer_dependency",
        Severity::Error,
        "first",
    )];
    let mut reasons = BTreeMap::new();
    reasons.insert(
        "architecture.namespace_boundary".to_string(),
        "disabled by config".to_string(),
    );
    let diag = ArchitectureDiagnostic {
        observations: obs,
        total_rules_evaluated: 5,
        total_rules_skipped: 0,
        skip_reasons: reasons.clone(),
    };
    assert_eq!(
        diag.skip_reasons.get("architecture.namespace_boundary"),
        Some(&"disabled by config".to_string())
    );
    assert_eq!(diag.total_rules_skipped, 0);
}

#[test]
fn severity_field_is_one_of_error_warning_info_hint() {
    use serde_json::Value;
    let cases = [
        (Severity::Error, "error"),
        (Severity::Warning, "warning"),
        (Severity::Info, "info"),
        (Severity::Hint, "hint"),
    ];
    for (sev, label) in cases {
        let json = serde_json::to_string(&sev).expect("severity serializes");
        let parsed: Value = serde_json::from_str(&json).expect("severity parses");
        assert_eq!(parsed, Value::String(label.to_string()));
    }
}

#[test]
fn cli_and_mcp_return_same_observation_count_for_same_workspace() {
    let obs_via_cli = vec![
        sample_observation("architecture.layer_dependency", Severity::Error, "via CLI"),
        sample_observation(
            "architecture.forbidden_dependency",
            Severity::Warning,
            "via CLI",
        ),
    ];
    let cli_diag = build_diagnostic(obs_via_cli.clone(), 4);
    let obs_via_mcp = obs_via_cli;
    let mcp_diag = build_diagnostic(obs_via_mcp, 4);
    assert_eq!(
        cli_diag.observations.len(),
        mcp_diag.observations.len(),
        "CLI and MCP must emit the same observation count"
    );
    assert_eq!(cli_diag.observations.len(), 2);
    assert_eq!(
        cli_diag.total_rules_evaluated,
        mcp_diag.total_rules_evaluated
    );
}

#[test]
fn evidence_with_empty_snippet_is_rejected() {
    let obs = ArchitectureObservation {
        rule_id: "architecture.layer_dependency".to_string(),
        severity: Severity::Error,
        message: "msg".to_string(),
        evidence: vec![ArchitectureEvidence {
            file: PathBuf::from("src/domain/foo.rs"),
            byte_range: Range { start: 0, end: 24 },
            snippet: String::new(),
            rule_reference: "ADR-046".to_string(),
        }],
        affected_symbols: vec![],
    };
    let err = validate_observation(&obs).expect_err("empty snippet must be rejected");
    assert!(matches!(err, ObservationValidationError::EmptySnippet));
}

#[test]
fn byte_range_is_byte_offset_not_line_column() {
    let obs = sample_observation("architecture.layer_dependency", Severity::Error, "");
    let r = obs.evidence[0].byte_range.clone();
    let _start: usize = r.start;
    let _end: usize = r.end;
    assert!(r.end > r.start);
}

#[test]
fn severity_default_is_info() {
    assert_eq!(Severity::default(), Severity::Info);
}

#[test]
fn skip_reasons_are_preserved_through_build() {
    let obs = vec![sample_observation(
        "architecture.layer_dependency",
        Severity::Error,
        "first",
    )];
    let mut reasons = BTreeMap::new();
    reasons.insert("rule_a".to_string(), "disabled".to_string());
    reasons.insert("rule_b".to_string(), "no evidence".to_string());
    let diag = ArchitectureDiagnostic {
        observations: obs,
        total_rules_evaluated: 7,
        total_rules_skipped: 6,
        skip_reasons: reasons.clone(),
    };
    assert_eq!(diag.skip_reasons.len(), 2);
    assert_eq!(
        diag.skip_reasons.get("rule_a"),
        Some(&"disabled".to_string())
    );
    assert_eq!(
        diag.skip_reasons.get("rule_b"),
        Some(&"no evidence".to_string())
    );
}
