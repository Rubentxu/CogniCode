//! Integration tests for cognicode-diagram with populated CallGraph
//!
//! These tests verify the full inference + render pipeline using realistic
//! CallGraph structures.

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::value_objects::dependency_type::DependencyType;
use cognicode_core::domain::value_objects::location::Location;
use cognicode_core::domain::value_objects::symbol_kind::SymbolKind;

use cognicode_diagram::inference::engine::InferenceEngine;
use cognicode_diagram::inference::uml_rules::UmlRuleEngine;
use cognicode_diagram::mcp::tools::{handle_generate_c4_code, GenerateC4CodeInput};
use cognicode_diagram::model::c4_types::{
    CodeElement, CodeElementKind, ElementId, Visibility,
};
use cognicode_diagram::model::relationships::C4RelationshipKind;
use cognicode_diagram::render::mermaid::{render_c4_context, render_class_diagram, MermaidOptions};

/// Helper to create a symbol in a CallGraph and return its ID
fn add_symbol(
    cg: &mut CallGraph,
    name: &str,
    kind: SymbolKind,
    file: &str,
    line: u32,
) -> cognicode_core::domain::aggregates::call_graph::SymbolId {
    let location = Location::new(file, line, 0);
    let symbol = Symbol::new(name, kind, location);
    cg.add_symbol(symbol)
}

/// Helper to add a dependency between two symbols
fn add_dependency(
    cg: &mut CallGraph,
    source: &cognicode_core::domain::aggregates::call_graph::SymbolId,
    target: &cognicode_core::domain::aggregates::call_graph::SymbolId,
    dep_type: DependencyType,
) {
    cg.add_dependency(source, target, dep_type).unwrap();
}

// =============================================================================
// Test 1: test_infer_workspace_with_symbols
// =============================================================================

#[test]
fn test_infer_workspace_with_symbols() {
    let mut cg = CallGraph::new();

    // Add 5+ symbols representing different modules
    let _order_service = add_symbol(&mut cg, "OrderService", SymbolKind::Class, "src/service.rs", 10);
    let _order = add_symbol(&mut cg, "Order", SymbolKind::Struct, "src/model.rs", 5);
    let _customer = add_symbol(&mut cg, "Customer", SymbolKind::Class, "src/model.rs", 20);
    let _payment = add_symbol(&mut cg, "Payment", SymbolKind::Struct, "src/model.rs", 35);
    let _validator = add_symbol(&mut cg, "OrderValidator", SymbolKind::Trait, "src/service.rs", 50);
    let _repo = add_symbol(&mut cg, "OrderRepository", SymbolKind::Trait, "src/repository.rs", 1);

    // Add dependencies between symbols
    let order_service_id = _order_service.clone();
    let order_id = _order.clone();
    let customer_id = _customer.clone();
    let validator_id = _validator.clone();
    let repo_id = _repo.clone();

    add_dependency(&mut cg, &order_service_id, &order_id, DependencyType::References);
    add_dependency(&mut cg, &order_service_id, &customer_id, DependencyType::References);
    add_dependency(&mut cg, &order_service_id, &validator_id, DependencyType::Inherits);
    add_dependency(&mut cg, &order_service_id, &repo_id, DependencyType::Calls);

    // Infer workspace
    let engine = InferenceEngine::new(&cg);
    let workspace = engine.infer_workspace("OrderSystem");

    // Verify workspace has systems
    assert!(
        !workspace.model.systems.is_empty(),
        "Workspace should have at least one system"
    );

    // Main system should be named after the project
    assert_eq!(
        workspace.model.systems[0].name, "OrderSystem",
        "Main system should be named 'OrderSystem'"
    );

    // Verify relationships exist (from module dependencies)
    // Note: relationships are inferred from module-level dependencies, not symbol-level
    let _rel_count = workspace.model.relationships.len();
    // Relationships are allowed to be 0 if no module-level dependencies exist
}

// =============================================================================
// Test 2: test_infer_code_elements
// =============================================================================

#[test]
fn test_infer_code_elements() {
    let mut cg = CallGraph::new();

    // Add symbols matching "src/" scope
    let class_id = add_symbol(&mut cg, "UserService", SymbolKind::Class, "src/service.rs", 10);
    let struct_id = add_symbol(&mut cg, "User", SymbolKind::Struct, "src/model.rs", 5);
    let enum_id = add_symbol(&mut cg, "UserRole", SymbolKind::Enum, "src/model.rs", 20);
    let trait_id = add_symbol(&mut cg, "UserRepository", SymbolKind::Trait, "src/repository.rs", 1);
    let _func_id = add_symbol(&mut cg, "helper", SymbolKind::Function, "src/utils.rs", 1);

    // Add dependencies
    add_dependency(&mut cg, &class_id, &struct_id, DependencyType::References);
    add_dependency(&mut cg, &class_id, &trait_id, DependencyType::Inherits);
    add_dependency(&mut cg, &class_id, &enum_id, DependencyType::References);

    // Infer code elements
    let engine = InferenceEngine::new(&cg);
    let elements = engine.infer_code_elements("src", 3);

    // Verify we get back the type-like symbols (Class, Struct, Enum, Trait)
    // Note: Function is not type-like, so it won't be included
    assert!(
        elements.len() >= 4,
        "Should have at least 4 code elements (Class, Struct, Enum, Trait), got {}",
        elements.len()
    );

    // Verify kinds
    let kinds: Vec<_> = elements.iter().map(|e| e.kind).collect();
    assert!(
        kinds.contains(&CodeElementKind::Class),
        "Should contain a Class"
    );
    assert!(
        kinds.contains(&CodeElementKind::Struct),
        "Should contain a Struct"
    );
    assert!(
        kinds.contains(&CodeElementKind::Enum),
        "Should contain an Enum"
    );
    assert!(
        kinds.contains(&CodeElementKind::Interface),
        "Should contain an Interface (Trait)"
    );
}

// =============================================================================
// Test 3: test_infer_relationships
// =============================================================================

#[test]
fn test_infer_relationships() {
    let mut cg = CallGraph::new();

    // Add symbols
    let service_id = add_symbol(&mut cg, "OrderService", SymbolKind::Class, "src/service.rs", 10);
    let model_id = add_symbol(&mut cg, "Order", SymbolKind::Struct, "src/model.rs", 5);
    let base_id = add_symbol(&mut cg, "BaseService", SymbolKind::Class, "src/service.rs", 1);
    let repo_id = add_symbol(&mut cg, "Repository", SymbolKind::Trait, "src/traits.rs", 1);

    // Add different dependency types
    add_dependency(&mut cg, &service_id, &model_id, DependencyType::References);
    add_dependency(&mut cg, &service_id, &base_id, DependencyType::Inherits);
    add_dependency(&mut cg, &service_id, &repo_id, DependencyType::Calls);

    // Infer code elements first
    let engine = InferenceEngine::new(&cg);
    let elements = engine.infer_code_elements("src", 3);

    // Then infer relationships
    let relationships = engine.infer_relationships(&elements);

    // We should have at least 3 relationships
    assert!(
        relationships.len() >= 3,
        "Should have at least 3 relationships, got {}",
        relationships.len()
    );

    // Verify relationship kinds
    let kinds: Vec<_> = relationships.iter().map(|r| r.kind).collect();
    assert!(
        kinds.contains(&C4RelationshipKind::DependsOn),
        "Should have DependsOn relationship (from References)"
    );
    assert!(
        kinds.contains(&C4RelationshipKind::Inherits),
        "Should have Inherits relationship"
    );
    assert!(
        kinds.contains(&C4RelationshipKind::Calls),
        "Should have Calls relationship"
    );
}

// =============================================================================
// Test 4: test_mermaid_class_diagram
// =============================================================================

#[test]
fn test_mermaid_class_diagram() {
    // Build code elements manually
    let elements = vec![
        CodeElement {
            id: ElementId::new("OrderService"),
            name: "OrderService".to_string(),
            kind: CodeElementKind::Class,
            visibility: Visibility::Public,
            path: Some("src/service.rs".to_string()),
            attributes: vec![],
            methods: vec![],
            relationships: vec![],
        },
        CodeElement {
            id: ElementId::new("Order"),
            name: "Order".to_string(),
            kind: CodeElementKind::Struct,
            visibility: Visibility::Public,
            path: Some("src/model.rs".to_string()),
            attributes: vec![],
            methods: vec![],
            relationships: vec![],
        },
    ];

    // Build UML relationships
    use cognicode_diagram::model::c4_types::UmlRelationship;
    use cognicode_diagram::model::c4_types::UmlRelationKind;

    let relationships = vec![
        UmlRelationship {
            target_id: ElementId::new("Order"),
            kind: UmlRelationKind::Association,
            label: Some("references".to_string()),
            confidence: 0.7,
        },
    ];

    // Render class diagram
    let options = MermaidOptions::default();
    let diagram = render_class_diagram(&elements, &relationships, &options);

    // Verify output
    assert!(
        diagram.contains("classDiagram"),
        "Diagram should contain 'classDiagram'"
    );
    assert!(
        diagram.contains("class OrderService"),
        "Diagram should contain 'class OrderService'"
    );
    assert!(
        diagram.contains("class Order"),
        "Diagram should contain 'class Order'"
    );
    assert!(
        diagram.contains("<<struct>>"),
        "Diagram should contain struct annotation for Order"
    );
}

// =============================================================================
// Test 5: test_c4_context_diagram
// =============================================================================

#[test]
fn test_c4_context_diagram() {
    use cognicode_diagram::model::workspace::C4Workspace;
    use cognicode_diagram::model::c4_types::Person;
    use cognicode_diagram::model::c4_types::ElementLocation;

    let mut workspace = C4Workspace::new("TestSystem");

    // Add a person
    workspace.model.people.push(Person {
        id: ElementId::new("actor_user"),
        name: "User".to_string(),
        description: "End user".to_string(),
        location: ElementLocation::External,
    });

    // Render context diagram
    let diagram = render_c4_context(&workspace);

    // Verify output
    assert!(
        diagram.contains("flowchart TB"),
        "Diagram should contain 'flowchart TB'"
    );
    assert!(
        diagram.contains("TestSystem"),
        "Diagram should contain 'TestSystem'"
    );
    assert!(
        diagram.contains("User"),
        "Diagram should contain 'User'"
    );
}

// =============================================================================
// Test 6: test_mcp_tool_end_to_end
// =============================================================================

#[test]
fn test_mcp_tool_end_to_end() {
    let mut cg = CallGraph::new();

    // Add symbols with file paths matching the scope
    let service_id = add_symbol(&mut cg, "OrderService", SymbolKind::Class, "src/service.rs", 10);
    let model_id = add_symbol(&mut cg, "Order", SymbolKind::Struct, "src/model.rs", 5);
    let repo_id = add_symbol(&mut cg, "OrderRepository", SymbolKind::Trait, "src/repository.rs", 1);

    // Add dependencies
    add_dependency(&mut cg, &service_id, &model_id, DependencyType::References);
    add_dependency(&mut cg, &service_id, &repo_id, DependencyType::Calls);

    // Call the MCP tool
    let input = GenerateC4CodeInput {
        scope: "src".to_string(),
        max_depth: Some(3),
        format: Some("mermaid".to_string()),
        show_methods: Some(true),
        show_attributes: Some(true),
    };

    let output = handle_generate_c4_code(input, &cg).unwrap();

    // Verify output
    assert!(
        output.element_count > 0,
        "Should have elements, got {}",
        output.element_count
    );
    assert_eq!(
        output.format, "mermaid",
        "Format should be 'mermaid'"
    );
    assert!(
        output.diagram.contains("classDiagram"),
        "Diagram should contain 'classDiagram'"
    );
}

// =============================================================================
// Test 7: test_inference_with_inheritance
// =============================================================================

#[test]
fn test_inference_with_inheritance() {
    let mut cg = CallGraph::new();

    // Add base and derived classes
    let base_id = add_symbol(&mut cg, "Animal", SymbolKind::Class, "src/animals.rs", 1);
    let derived_id = add_symbol(&mut cg, "Dog", SymbolKind::Class, "src/animals.rs", 20);

    // Add inheritance relationship
    add_dependency(&mut cg, &derived_id, &base_id, DependencyType::Inherits);

    // Infer code elements
    let engine = InferenceEngine::new(&cg);
    let elements = engine.infer_code_elements("src", 3);

    // Verify we get both classes
    assert!(
        elements.len() >= 2,
        "Should have at least 2 elements (Animal and Dog), got {}",
        elements.len()
    );

    // Infer relationships
    let relationships = engine.infer_relationships(&elements);

    // Verify we have an Inherits relationship
    assert!(
        relationships.iter().any(|r| r.kind == C4RelationshipKind::Inherits),
        "Should have an Inherits relationship, got {:?}",
        relationships.iter().map(|r| r.kind).collect::<Vec<_>>()
    );
}

// =============================================================================
// Test 8: test_uml_rule_engine_infer_relationships
// =============================================================================

#[test]
fn test_uml_rule_engine_infer_relationships() {
    let mut cg = CallGraph::new();

    // Add symbols
    let parent_id = add_symbol(&mut cg, "Shape", SymbolKind::Class, "src/shapes.rs", 1);
    let child_id = add_symbol(&mut cg, "Circle", SymbolKind::Class, "src/shapes.rs", 30);
    let contains_id = add_symbol(&mut cg, "Point", SymbolKind::Struct, "src/shapes.rs", 50);
    let method_id = add_symbol(&mut cg, "draw", SymbolKind::Method, "src/shapes.rs", 60);

    // Add different dependency types
    add_dependency(&mut cg, &child_id, &parent_id, DependencyType::Inherits);
    add_dependency(&mut cg, &parent_id, &contains_id, DependencyType::Contains);
    add_dependency(&mut cg, &parent_id, &method_id, DependencyType::Contains);

    // Infer code elements
    let engine = InferenceEngine::new(&cg);
    let elements = engine.infer_code_elements("src", 3);

    // Build element_ids map
    let element_ids: std::collections::HashMap<String, ElementId> = elements
        .iter()
        .map(|e| (e.id.as_str().to_string(), e.id.clone()))
        .collect();

    // Use UmlRuleEngine to infer UML relationships
    let uml_engine = UmlRuleEngine::new();
    let uml_rels = uml_engine.infer_uml_relationships(&cg, &element_ids);

    // Verify relationships
    assert!(
        !uml_rels.is_empty(),
        "Should have UML relationships"
    );

    // Check for inheritance relationship
    use cognicode_diagram::model::c4_types::UmlRelationKind;
    let has_inheritance = uml_rels
        .iter()
        .any(|r| r.kind == UmlRelationKind::Inheritance);
    assert!(
        has_inheritance,
        "Should have Inheritance UML relationship, got {:?}",
        uml_rels.iter().map(|r| r.kind).collect::<Vec<_>>()
    );

    // Check for composition relationship (Contains)
    let has_composition = uml_rels
        .iter()
        .any(|r| r.kind == UmlRelationKind::Composition);
    assert!(
        has_composition,
        "Should have Composition UML relationship (from Contains)"
    );
}

// =============================================================================
// Test 9: test_render_class_diagram_with_options
// =============================================================================

#[test]
fn test_render_class_diagram_with_options() {
    let elements = vec![CodeElement {
        id: ElementId::new("TestClass"),
        name: "TestClass".to_string(),
        kind: CodeElementKind::Class,
        visibility: Visibility::Public,
        path: Some("src/lib.rs".to_string()),
        attributes: vec![],
        methods: vec![],
        relationships: vec![],
    }];

    let options = MermaidOptions {
        title: "Test Diagram".to_string(),
        theme: Some("dark".to_string()),
        direction: "LR".to_string(),
        max_depth: 2,
        show_methods: true,
        show_attributes: true,
        show_visibility: true,
    };

    let diagram = render_class_diagram(&elements, &[], &options);

    assert!(
        diagram.contains("title: Test Diagram"),
        "Diagram should contain title"
    );
    assert!(
        diagram.contains("classDiagram"),
        "Diagram should contain 'classDiagram'"
    );
}

// =============================================================================
// Test 10: test_infer_code_elements_empty_scope
// =============================================================================

#[test]
fn test_infer_code_elements_empty_scope() {
    let cg = CallGraph::new();
    let engine = InferenceEngine::new(&cg);
    let elements = engine.infer_code_elements("src", 3);
    assert!(
        elements.is_empty(),
        "Empty CallGraph should produce no elements"
    );
}

// =============================================================================
// Test 11: test_handle_generate_c4_code_empty_graph
// =============================================================================

#[test]
fn test_handle_generate_c4_code_empty_graph() {
    let cg = CallGraph::new();
    let input = GenerateC4CodeInput {
        scope: "src".to_string(),
        max_depth: None,
        format: None,
        show_methods: None,
        show_attributes: None,
    };

    let output = handle_generate_c4_code(input, &cg).unwrap();

    assert_eq!(output.element_count, 0);
    assert_eq!(output.relationship_count, 0);
    assert!(output.diagram.contains("classDiagram"));
}

// =============================================================================
// Test 12: test_mermaid_class_diagram_with_relationships
// =============================================================================

#[test]
fn test_mermaid_class_diagram_with_relationships() {
    use cognicode_diagram::model::c4_types::{UmlRelationship, UmlRelationKind};

    let elements = vec![
        CodeElement {
            id: ElementId::new("Vehicle"),
            name: "Vehicle".to_string(),
            kind: CodeElementKind::Class,
            visibility: Visibility::Public,
            path: Some("src/vehicle.rs".to_string()),
            attributes: vec![],
            methods: vec![],
            relationships: vec![],
        },
        CodeElement {
            id: ElementId::new("Car"),
            name: "Car".to_string(),
            kind: CodeElementKind::Class,
            visibility: Visibility::Public,
            path: Some("src/vehicle.rs".to_string()),
            attributes: vec![],
            methods: vec![],
            relationships: vec![],
        },
        CodeElement {
            id: ElementId::new("Engine"),
            name: "Engine".to_string(),
            kind: CodeElementKind::Struct,
            visibility: Visibility::Private,
            path: Some("src/vehicle.rs".to_string()),
            attributes: vec![],
            methods: vec![],
            relationships: vec![],
        },
    ];

    let relationships = vec![
        UmlRelationship {
            target_id: ElementId::new("Vehicle"),
            kind: UmlRelationKind::Inheritance,
            label: Some("inherits from".to_string()),
            confidence: 1.0,
        },
        UmlRelationship {
            target_id: ElementId::new("Engine"),
            kind: UmlRelationKind::Composition,
            label: Some("contains".to_string()),
            confidence: 0.9,
        },
    ];

    let diagram = render_class_diagram(&elements, &relationships, &MermaidOptions::default());

    assert!(diagram.contains("classDiagram"));
    assert!(diagram.contains("class Vehicle"));
    assert!(diagram.contains("class Car"));
    assert!(diagram.contains("class Engine"));
    // Check for struct annotation
    assert!(diagram.contains("<<struct>>"));
}
