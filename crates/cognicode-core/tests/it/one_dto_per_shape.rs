//! A payload shape has one definition.
//!
//! The MCP schemas and the application DTOs describe the same payloads. When
//! each side keeps its own copy, a field added to one is silently absent from
//! the other and the two drift apart. Sharing the type makes that impossible,
//! so the check is on the identity of the types, not on their fields.
//!
//! `DeadCodeEntry` is not here on purpose: the wire carries `kind` and
//! `reason` as strings (`"function"`, `"NoIncomingEdges"`) while the
//! application entry is typed, and typing the wire would change a published
//! payload. What is shared is the projection, so it is pinned below.

use std::any::TypeId;

use cognicode_core::application::dto as app;
use cognicode_core::interface::mcp::schemas as mcp;

fn same<A: 'static, B: 'static>() -> bool {
    TypeId::of::<A>() == TypeId::of::<B>()
}

#[test]
fn the_architecture_shapes_are_defined_once() {
    assert!(same::<mcp::CycleInfo, app::CycleInfo>());
    assert!(same::<mcp::ViolationInfo, app::ViolationInfo>());
}

#[test]
fn the_refactor_result_shapes_are_defined_once() {
    assert!(same::<mcp::ChangeEntry, app::ChangeEntry>());
    assert!(same::<mcp::ValidationResult, app::ValidationResult>());
}

#[test]
fn the_module_graph_shapes_are_defined_once() {
    assert!(same::<mcp::ModuleDependency, app::ModuleDependency>());
    assert!(same::<mcp::ModuleDependencyGraph, app::ModuleDependencyGraph>());
}

#[test]
fn the_overview_shapes_are_defined_once() {
    assert!(same::<mcp::OverviewDetail, app::OverviewDetail>());
    assert!(same::<mcp::OverviewMeta, app::OverviewMeta>());
}

#[test]
fn the_refactor_action_is_defined_once_and_keeps_its_published_names() {
    assert!(same::<mcp::RefactorAction, app::RefactorAction>());
    let names: Vec<String> = [
        app::RefactorAction::Rename,
        app::RefactorAction::Extract,
        app::RefactorAction::Inline,
        app::RefactorAction::Move,
        app::RefactorAction::ChangeSignature,
    ]
    .iter()
    .map(|a| {
        serde_json::to_value(a)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    })
    .collect();
    assert_eq!(
        names,
        ["rename", "extract", "inline", "move", "change_signature"]
    );
}

#[test]
fn the_dead_code_wire_entry_is_one_projection_of_the_application_entry() {
    let entry = app::DeadCodeEntry {
        symbol: "f".into(),
        file: "a.rs".into(),
        line: 3,
        column: 1,
        kind: cognicode_core::domain::value_objects::SymbolKind::Function,
        reason: app::DeadCodeReason::NoIncomingEdges,
        confidence: 0.5,
    };
    let wire = serde_json::to_value(mcp::DeadCodeEntry::from(entry)).unwrap();
    assert_eq!(wire["kind"], "function");
    assert_eq!(wire["reason"], "NoIncomingEdges");
}
