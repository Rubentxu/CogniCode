//! B4.W5 — Composite impact query for agents.
//!
//! Que cierra este WU:
//!   - `cognicode_core::impact::composite::execute_composite` returns
//!     `CompositeImpactResult` with `impact`, `architecture`,
//!     `findings`, `suggested_follow_ups`,
//!     `suggested_prerequisites`, `verdict_refused=true`, y la
//!     razon canonica `verdict_refused_reason`.
//!   - El composite NUNCA produce un veredicto (la pieza
//!     load-bearing del contrato: `verdict_refused == true` en
//!     toda llamada).
//!   - `suggested_follow_ups` y `suggested_prerequisites` se
//!     extraen EXCLUSIVAMENTE del catalogo `product/tools.json`
//!     para la entrada del simbolo (B3.W3 parity).
//!   - El sub-comando CLI `cogh graph impact-composite <symbol>`
//!     emite un objeto JSON con los seis campos; el help text
//!     avisa que esto es investigativo, no autoritativo.
//!
//! Degradacion graciosa (a este commit):
//!   - B4.W1 (typed `ImpactResult` extractor) NO mergea: `impact: None`.
//!   - B4.W2 (typed `ArchitectureDiagnostic` extractor) NO mergea:
//!     `architecture: None`.
//!   - B4.W3 (Finding extractor wired al composite) NO mergea:
//!     `findings: Vec::empty()`. Vec, no Option: cero hallazgos es
//!     resultado valido.

use cognicode_core::impact::composite::{
    CompositeImpactQuery, CompositeImpactResult, COMPOSITE_VERDICT_REFUSED_REASON,
    execute_composite,
};
use cognicode_core::interface::catalog::load_tool_catalog;
use cognicode_core::interface::symbol_identity::SymbolId;

use serde_json::{Value, json};
use std::collections::HashSet;
use std::ops::Range;
use std::path::PathBuf;

fn stub_query(symbol_qualified: &str) -> CompositeImpactQuery {
    CompositeImpactQuery {
        symbol: SymbolId {
            qualified_name: symbol_qualified.to_string(),
            file_path: PathBuf::from("src/lib.rs"),
            byte_range: Range { start: 0, end: 10 },
        },
        workspace_root: PathBuf::from("."),
        depth_limit: None,
    }
}

fn catalog_names_set() -> HashSet<String> {
    load_tool_catalog().into_iter().map(|d| d.name).collect()
}

// 1. La espina de B4.W5: verdict_refused siempre true.
#[test]
fn composite_result_always_carries_verdict_refused_true() {
    let cases = [
        stub_query("foo::bar"),
        stub_query(""),
        CompositeImpactQuery {
            symbol: SymbolId {
                qualified_name: "x".into(),
                file_path: PathBuf::from("/nonexistent/file.rs"),
                byte_range: Range { start: 0, end: 1 },
            },
            workspace_root: PathBuf::from("/nonexistent/root"),
            depth_limit: Some(0),
        },
    ];
    for q in &cases {
        let r = execute_composite(q);
        assert!(r.verdict_refused, "verdict_refused must be true");
        assert_eq!(
            r.verdict_refused_reason, COMPOSITE_VERDICT_REFUSED_REASON,
            "verdict_refused_reason must be the canonical static reason"
        );
    }
}

// 2. follow_ups vienen exclusivamente del catalogo.
#[test]
fn composite_result_pulls_follow_ups_from_catalog() {
    let catalog = load_tool_catalog();
    let target = catalog
        .iter()
        .find(|d| d.follow_ups.as_ref().is_some_and(|v| !v.is_empty()))
        .expect("catalog must contain at least one tool with follow_ups");
    let target_follow_ups = target.follow_ups.as_ref().unwrap();
    let query = CompositeImpactQuery {
        symbol: SymbolId {
            qualified_name: target.name.clone(),
            file_path: PathBuf::from(target.name.clone()),
            byte_range: Range { start: 0, end: 1 },
        },
        workspace_root: PathBuf::from("."),
        depth_limit: None,
    };
    let result = execute_composite(&query);
    assert_eq!(result.suggested_follow_ups.len(), target_follow_ups.len());
    for tool_ref in &result.suggested_follow_ups {
        assert!(target_follow_ups.iter().any(|f| f == &tool_ref.tool));
    }
}

// 3. prerequisites vienen exclusivamente del catalogo.
#[test]
fn composite_result_pulls_prerequisites_from_catalog() {
    let catalog = load_tool_catalog();
    let target = catalog
        .iter()
        .find(|d| d.prerequisites.as_ref().is_some_and(|v| !v.is_empty()))
        .expect("catalog must contain at least one tool with prerequisites");
    let target_prereqs = target.prerequisites.as_ref().unwrap();
    let query = CompositeImpactQuery {
        symbol: SymbolId {
            qualified_name: target.name.clone(),
            file_path: PathBuf::from(target.name.clone()),
            byte_range: Range { start: 0, end: 1 },
        },
        workspace_root: PathBuf::from("."),
        depth_limit: None,
    };
    let result = execute_composite(&query);
    assert_eq!(result.suggested_prerequisites.len(), target_prereqs.len());
    for tool_ref in &result.suggested_prerequisites {
        assert!(target_prereqs.iter().any(|p| p == &tool_ref.tool));
    }
}

// 4. impact es None mientras B4.W1 no mergea.
#[test]
fn composite_result_impact_is_none_when_b4w1_not_merged() {
    let r = execute_composite(&stub_query(
        "cognicode_core::impact::composite::execute_composite",
    ));
    assert!(r.impact.is_none());
}

// 5. architecture es None mientras B4.W2 no mergea.
#[test]
fn composite_result_architecture_is_none_when_b4w2_not_merged() {
    let r = execute_composite(&stub_query(
        "cognicode_core::architecture::diagnostic::check_architecture",
    ));
    assert!(r.architecture.is_none());
}

// 6. findings es Vec vacia cuando B4.W3 no mergea (no Option).
#[test]
fn composite_result_findings_is_empty_when_b4w3_not_merged() {
    let r = execute_composite(&stub_query("cognicode_core::findings::catalog::collect"));
    assert!(r.findings.is_empty());
}

// 7. Seis campos top-level (verdict_refused + verdict_refused_reason +
//    findings + suggested_*), con `verdict_refused=true` y la razon
//    anclada por la constante publica COMPOSITE_VERDICT_REFUSED_REASON.
#[test]
fn composite_result_includes_six_top_level_fields_with_expected_shapes() {
    let r = execute_composite(&stub_query("analyze_impact"));
    let v: Value = serde_json::to_value(&r).expect("serialisable");
    let obj = v.as_object().expect("top-level must be an object");
    for field in [
        "verdict_refused",
        "verdict_refused_reason",
        "findings",
        "suggested_follow_ups",
        "suggested_prerequisites",
    ] {
        assert!(obj.contains_key(field), "missing field `{field}` in {obj:?}");
    }
    assert_eq!(obj["verdict_refused"], json!(true));
    let reason = obj["verdict_refused_reason"]
        .as_str()
        .expect("reason must be a string");
    assert!(reason.contains("evidence") && reason.contains("verdict"));
    assert_eq!(obj["findings"], json!([]));
}

// 8. suggested_tool_ref_args referencia tools validos del catalogo.
#[test]
fn composite_result_suggested_tool_ref_args_reference_valid_catalog_tools() {
    let names_set = catalog_names_set();
    let catalog = load_tool_catalog();
    let target = catalog.iter().find(|d| {
        d.follow_ups.as_ref().is_some_and(|v| !v.is_empty())
            && d.prerequisites.as_ref().is_some_and(|v| !v.is_empty())
    });
    if let Some(target) = target {
        let query = CompositeImpactQuery {
            symbol: SymbolId {
                qualified_name: target.name.clone(),
                file_path: PathBuf::from(target.name.clone()),
                byte_range: Range { start: 0, end: 1 },
            },
            workspace_root: PathBuf::from("."),
            depth_limit: None,
        };
        let r = execute_composite(&query);
        for tool_ref in r
            .suggested_follow_ups
            .iter()
            .chain(r.suggested_prerequisites.iter())
        {
            assert!(
                names_set.contains(&tool_ref.tool),
                "composite referenced tool {:?} which is not in the catalog",
                tool_ref.tool
            );
            let args_value = serde_json::to_value(&tool_ref.args)
                .expect("ToolRef.args must be serialisable");
            assert!(
                args_value.is_object() || args_value.is_null(),
                "args must serialise to object or null, got {args_value:?}"
            );
        }
    }
}

// 9. La salida JSON del sub-comando CLI tiene los seis campos + la
//    pieza load-bearing verdict_refused=true en la raiz.
#[test]
fn cli_subcommand_emits_json_with_six_top_level_fields() {
    let r: CompositeImpactResult = execute_composite(&stub_query("analyze_impact"));
    let body = serde_json::to_string(&r).expect("serialisable");
    let parsed: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("emitted JSON must parse: {e}"));
    let obj = parsed.as_object().expect("emitted JSON must be an object");
    for field in [
        "verdict_refused",
        "verdict_refused_reason",
        "findings",
        "suggested_follow_ups",
        "suggested_prerequisites",
    ] {
        assert!(obj.contains_key(field), "missing CLI field `{field}`");
    }
    assert_eq!(obj["verdict_refused"], json!(true));
}

// 10. Determinismo: la misma query produce resultado byte-identical.
#[test]
fn composite_query_carries_symbol_identity_for_investigation() {
    let q = stub_query("deterministic::test::symbol");
    let r1 = execute_composite(&q);
    let r2 = execute_composite(&q);
    let v1 = serde_json::to_value(&r1).expect("serialisable");
    let v2 = serde_json::to_value(&r2).expect("serialisable");
    assert_eq!(v1, v2);
    assert!(r1.verdict_refused && r2.verdict_refused);
}
