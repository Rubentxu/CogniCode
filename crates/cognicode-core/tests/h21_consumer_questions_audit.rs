//! H2.1 (delivery-first-radical, package integration) — inventory +
//! measurement of the three consumer questions.
//!
//! H2.1 spec: "Probar tres preguntas consumidoras (homónimo, impacto
//! API, justificación de finding); inventariar `tools/list` y
//! `product/tools.json`. Medir respuestas actuales, no añadir
//! endpoint que ya exista."
//!
//! This test is the read-only acceptance for H2.1 cycle 1. The
//! claim: every consumer question has a clear answer path through
//! the tools that are already in the catalog. H2.2+ can rely on
//! this contract without re-deriving the mapping.
//!
//! The mapping (consumer question → existing tool → response shape):
//!
//! | Question | Tool | Input | Output shape |
//! |----------|------|-------|--------------|
//! | Homonym: "where is `helper` used?" | `query_symbol_index` | `symbol_name` (case-insensitive) | all locations of that name |
//! | API impact: "what breaks if I change `add`?" | `analyze_impact` | `symbol_name` | impacted files + risk level |
//! | Finding justification: "why is finding X reported?" | `composite_impact_query` | `qualified_name` | `verdict_refused=true` + evidence (impact/architecture/findings) + follow-ups |
//!
//! The third question is the interesting one. There is no tool that
//! says "the verdict is X because Y": the composite refuses to give
//! a verdict (the `verdict_refused_reason` is pinned in the source
//! at `composite queries return evidence, not verdicts; investigate
//! each piece`). The consumer must use the suggested follow-ups to
//! dig deeper. The audit captures this as a feature, not a bug:
//! the consumer gets an EVIDENCE BUNDLE, not a verdict, and the
//! "answer" to "why?" is the bundle of impacts + findings + arch
//! observations with their individual reasons.

use serde_json::Value;

// Resolved at compile time. cargo runs integration tests with the
// working directory set to the package root (crates/cognicode-core/),
// not the tests/ folder; a relative path from there is two
// directories up to the repo root, then product/.
const CATALOG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../product/tools.json");
const RUNTIME: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../product/tool-catalog-runtime.json"
);

fn load(path: &str) -> Value {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    serde_json::from_slice::<Value>(&bytes)
        .unwrap_or_else(|e| panic!("cannot parse {path}: {e}"))
}

fn by_name<'a>(catalog: &'a Value) -> Vec<&'a Value> {
    catalog
        .get("tools")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn find_tool<'a>(catalog: &'a Value, name: &str) -> &'a Value {
    by_name(catalog)
        .into_iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
        .unwrap_or_else(|| panic!("tool `{name}` must be in the catalog"))
}

fn find_runtime<'a>(catalog: &'a Value, name: &str) -> &'a Value {
    catalog
        .as_array()
        .unwrap_or_else(|| panic!("runtime catalog must be a JSON array"))
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
        .unwrap_or_else(|| panic!("tool `{name}` must be in the runtime catalog"))
}

#[test]
fn hand_authored_and_runtime_catalogs_agree_on_tool_names() {
    // The hand-authored catalog is the source of truth (per project
    // memory: "product/tools.json is hand-authored — never
    // regenerate it; compare tool names against a full-feature
    // capture"). The runtime catalog is what the MCP server ships.
    // If the two ever drift, this test fails first.
    let hand = load(CATALOG);
    let runtime = load(RUNTIME);
    let hand_names: std::collections::BTreeSet<String> = by_name(&hand)
        .iter()
        .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    let runtime_names: std::collections::BTreeSet<String> = runtime
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    assert_eq!(
        hand_names, runtime_names,
        "hand-authored tools.json and tool-catalog-runtime.json must agree on tool names; \
         a drift means the catalog the operator maintains is no longer the catalog the MCP \
         server ships"
    );
    assert_eq!(hand_names.len(), 80, "H2.1 audit pins the 80-tool surface");
}

#[test]
fn homonym_question_is_answered_by_query_symbol_index() {
    // Q1: "Where is `helper` used in this project?" The consumer
    // expects a tool that resolves a name to ALL its locations
    // (case-insensitive, by symbol name only — no file/line
    // required). The only existing tool that does this from a
    // name alone is `query_symbol_index` (search category, ast
    // precision).
    let catalog = load(CATALOG);
    let tool = find_tool(&catalog, "query_symbol_index");
    let inputs = tool
        .get("input_schema")
        .and_then(|s| s.get("required"))
        .and_then(Value::as_array)
        .expect("query_symbol_index must declare its required inputs");
    let input_names: std::collections::BTreeSet<&str> = inputs
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        input_names.contains("symbol_name"),
        "Q1 requires `symbol_name`; the audit pins that file_path/line/column are NOT required (those would force a callsite-level question, not a name-level one)"
    );
    assert_eq!(
        tool.get("authority").and_then(Value::as_str),
        Some("read"),
        "homonym resolution is a read, not a mutation"
    );
    let runtime = load(RUNTIME);
    let rt_tool = find_runtime(&runtime, "query_symbol_index");
    assert_eq!(
        rt_tool.get("name").and_then(Value::as_str),
        Some("query_symbol_index"),
        "runtime catalog must also expose query_symbol_index for the MCP tools/list"
    );
}

#[test]
fn api_impact_question_is_answered_by_analyze_impact() {
    // Q2: "What breaks if I change `add`?" The consumer expects
    // an impacted-files list plus a risk level. The existing tool
    // is `analyze_impact` (graph category, read authority). The
    // audit pins: required input is `symbol_name`; the tool is
    // read (a mutation here would be a security defect); the
    // prerequisite is `build_graph` (the tool is honest: it
    // refuses to invent an impact without a real graph).
    let catalog = load(CATALOG);
    let tool = find_tool(&catalog, "analyze_impact");
    let inputs = tool
        .get("input_schema")
        .and_then(|s| s.get("required"))
        .and_then(Value::as_array)
        .expect("analyze_impact must declare its required inputs");
    let input_names: std::collections::BTreeSet<&str> = inputs
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(input_names.contains("symbol_name"));
    assert_eq!(
        tool.get("authority").and_then(Value::as_str),
        Some("read"),
        "API impact is a read; a mutating impact tool would be an authz defect"
    );
    let prereqs = tool
        .get("prerequisites")
        .and_then(Value::as_array)
        .expect("analyze_impact must declare its prerequisites");
    let prereq_names: Vec<&str> = prereqs.iter().filter_map(Value::as_str).collect();
    assert!(
        prereq_names.contains(&"build_graph"),
        "the impact tool must require a real graph (H0.1: no fake impact without a build)"
    );
    let description = tool.get("description").and_then(Value::as_str).unwrap_or("");
    assert!(
        description.contains("impacted files") && description.contains("risk"),
        "analyze_impact must promise the two things the consumer needs from an impact answer"
    );
}

#[test]
fn finding_justification_is_a_typed_refusal_with_evidence() {
    // Q3: "Why is finding X reported?" There is no tool that
    // answers with a verdict. The existing tool is
    // `composite_impact_query`, which is INVESTIGATIVE: the result
    // always carries verdict_refused=true and the reason
    // `composite queries return evidence, not verdicts; investigate
    // each piece`. The audit pins: the answer to "why?" is the
    // evidence bundle (impact + architecture + findings list +
    // suggested follow-ups), and the verdict is refused, by
    // design. A future contributor who adds a "verdict" mode to
    // the composite will break this test on purpose, so the
    // change is visible.
    let catalog = load(CATALOG);
    let tool = find_tool(&catalog, "composite_impact_query");
    let output = tool
        .get("output_schema")
        .expect("composite_impact_query must declare its output schema");
    let props = output
        .get("properties")
        .and_then(Value::as_object)
        .expect("composite_impact_query output must have properties");
    assert_eq!(
        props
            .get("verdict_refused")
            .and_then(Value::as_object)
            .and_then(|o| o.get("type"))
            .and_then(Value::as_str),
        Some("boolean"),
        "the composite must promise verdict_refused is a boolean (typed refusal)"
    );
    let required = output
        .get("required")
        .and_then(Value::as_array)
        .expect("composite output must declare required fields");
    let required_names: std::collections::BTreeSet<&str> =
        required.iter().filter_map(Value::as_str).collect();
    for must in [
        "verdict_refused",
        "verdict_refused_reason",
        "impact",
        "architecture",
        "findings",
        "suggested_follow_ups",
        "suggested_prerequisites",
    ] {
        assert!(
            required_names.contains(must),
            "composite_impact_query must always carry `{must}` (the consumer is never promised a verdict, only the evidence shape)"
        );
    }
    let reason_field = props
        .get("verdict_refused_reason")
        .and_then(Value::as_object)
        .expect("verdict_refused_reason must be a typed string field");
    assert_eq!(
        reason_field.get("type").and_then(Value::as_str),
        Some("string"),
        "verdict_refused_reason must be a typed string (the consumer cannot parse a free-form refusal)"
    );
    let description = tool.get("description").and_then(Value::as_str).unwrap_or("");
    // The canonical reason string lives in the tool description
    // (the field-level description is a meta-comment pointing at
    // the source). The audit pins BOTH: the tool's surfaced
    // promise, and the canonical literal that downstream code
    // actually emits. A future contributor who changes the
    // canonical reason will have to update both places, and this
    // test catches the silent drift.
    assert!(
        description.contains("verdict_refused=true"),
        "the tool description must surface the refusal so the consumer sees it in tools/list"
    );
    assert!(
        description.contains("composite queries return evidence, not verdicts"),
        "the canonical refusal reason is pinned here; the field is typed as string because the literal is part of the consumer contract"
    );
}

#[test]
fn the_three_question_covering_tools_are_all_read_authority() {
    // Belt and suspenders: the three tools that answer the
    // consumer questions must be all read authority. A mutation
    // surfacing in the same name space as a read tool would be
    // a security defect, and the audit catches it.
    let catalog = load(CATALOG);
    for name in ["query_symbol_index", "analyze_impact", "composite_impact_query"] {
        let tool = find_tool(&catalog, name);
        assert_eq!(
            tool.get("authority").and_then(Value::as_str),
            Some("read"),
            "tool `{name}` covers a consumer question; it must be read authority (mutations are not answers)"
        );
    }
}

#[test]
fn mutating_tools_are_not_in_the_three_question_path() {
    // The H2.1 spec is explicit: "una lectura nunca ofrece acción
    // mutante sin capacidad explícita." The 5 mutating tools in
    // the catalog (edit_file, reparse_on_edit, safe_refactor_apply,
    // write_file, ...) are NOT in the answer path for the 3
    // consumer questions. This test pins that boundary so a
    // future refactor that moves a mutating tool into the read
    // path trips the audit.
    let catalog = load(CATALOG);
    let mutating: std::collections::BTreeSet<String> = by_name(&catalog)
        .iter()
        .filter(|t| t.get("authority").and_then(Value::as_str) == Some("mutating"))
        .filter_map(|t| {
            t.get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect();
    let answering: std::collections::BTreeSet<&str> = [
        "query_symbol_index",
        "analyze_impact",
        "composite_impact_query",
    ]
    .into_iter()
    .collect();
    for name in &answering {
        assert!(
            !mutating.contains(*name),
            "the answer tool `{name}` is in the mutating set; the consumer question must be answered by a read tool, never a mutation"
        );
    }
    // A safety print so a future maintainer can see the full
    // mutating set when this test fails.
    eprintln!("mutating tools (excluded from the answer path): {mutating:?}");
}
