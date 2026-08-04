//! Integration test for ProjectDiary and ExampleObject view executors.
//!
//! Verifies that both executors are properly registered in the ViewRegistry
//! and appear in the list_for(Workspace) and list_for(Symbol) results.

use cognicode_explorer::dto::{InspectableObjectType, ViewKind};
use cognicode_explorer::registry::ViewRegistry;

#[test]
fn test_registry_lists_project_diary_and_example_object() {
    let registry = ViewRegistry::new(None);

    // ProjectDiary should appear in list_for(Workspace)
    let workspace_views = registry.list_for(InspectableObjectType::Workspace);
    assert!(
        workspace_views.iter().any(|v| v.id == "project-diary"),
        "expected project-diary for Workspace, got {workspace_views:?}"
    );

    // ProjectDiary should be accessible by id
    let project_diary = registry.list_for(InspectableObjectType::Workspace)
        .into_iter()
        .find(|v| v.id == "project-diary");
    assert!(
        project_diary.is_some(),
        "expected project-diary to be registered"
    );
    let project_diary = project_diary.unwrap();
    assert_eq!(project_diary.title, "Project Diary");
    assert_eq!(project_diary.view_kind, ViewKind::ProjectDiary);

    // ExampleObject should appear in list_for(Symbol)
    let symbol_views = registry.list_for(InspectableObjectType::Symbol);
    assert!(
        symbol_views.iter().any(|v| v.id == "example-object"),
        "expected example-object for Symbol, got {symbol_views:?}"
    );

    // ExampleObject should be accessible by id
    let example_object = registry.list_for(InspectableObjectType::Symbol)
        .into_iter()
        .find(|v| v.id == "example-object");
    assert!(
        example_object.is_some(),
        "expected example-object to be registered"
    );
    let example_object = example_object.unwrap();
    assert_eq!(example_object.title, "Example Object");
    assert_eq!(example_object.view_kind, ViewKind::ExampleObject);
}
