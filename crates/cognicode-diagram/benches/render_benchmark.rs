//! Render benchmark — measures diagram rendering performance for various formats

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

use cognicode_diagram::model::c4_types::{ContainerType, Container, ElementId, ElementLocation, Person, SoftwareSystem, CodeElement, CodeElementKind, UmlRelationship, UmlRelationKind, Visibility, Component, ComponentType};
use cognicode_diagram::model::relationships::{C4Relationship, C4RelationshipKind};
use cognicode_diagram::model::workspace::C4Workspace;
use cognicode_diagram::render::d2::{render_d2, D2Options};
use cognicode_diagram::render::mermaid::{render_class_diagram, MermaidOptions};
use cognicode_diagram::render::mermaid_c4::{render_component_diagram, C4MermaidOptions};
use cognicode_diagram::render::plantuml::{render_plantuml_c4, PlantUmlOptions, PlantUmlViewType};
use cognicode_diagram::render::structurizr_dsl::{render_structurizr_dsl, StructurizrDslOptions};
use cognicode_diagram::render::svg::{render_svg, SvgTheme};
use cognicode_diagram::layout::sugiyama::compute_layout;
use cognicode_diagram::layout::types::LayoutConfig;

fn create_test_workspace_50() -> C4Workspace {
    let mut workspace = C4Workspace::new("TestSystem");

    // Add a person
    workspace.model.people.push(Person {
        id: ElementId::new("person-1"),
        name: "Test User".to_string(),
        description: "A test user".to_string(),
        location: ElementLocation::Internal,
    });

    // Add 1 system with 10 containers (each with 5 components = 50 nodes total)
    let system = SoftwareSystem {
        id: ElementId::new("system-1"),
        name: "Test System".to_string(),
        description: "A test system".to_string(),
        location: ElementLocation::Internal,
        containers: (0..10).map(|i| {
            Container {
                id: ElementId::new(format!("container-{}", i)),
                name: format!("Container {}", i),
                container_type: ContainerType::Service,
                technology: "Rust".to_string(),
                description: format!("Container {}", i),
                path: None,
                components: (0..5).map(|j| {
                    Component {
                        id: ElementId::new(format!("component-{}-{}", i, j)),
                        name: format!("Component {}-{}", i, j),
                        component_type: ComponentType::Module,
                        technology: "Rust".to_string(),
                        description: format!("Component {}-{}", i, j),
                        path: None,
                        code_elements: vec![],
                    }
                }).collect(),
            }
        }).collect(),
    };
    workspace.model.systems.push(system);

    // Add relationships between adjacent containers
    for i in 0..9 {
        workspace.model.relationships.push(C4Relationship::new(
            ElementId::new(format!("container-{}", i)),
            ElementId::new(format!("container-{}", i + 1)),
            C4RelationshipKind::Calls,
        ));
    }

    workspace
}

fn bench_render_d2(c: &mut Criterion) {
    let workspace = create_test_workspace_50();

    c.bench_function("render_d2_50_nodes", |b| {
        b.iter(|| {
            let options = D2Options::default();
            let _ = render_d2(&workspace, &options);
        });
    });
}

fn bench_render_plantuml(c: &mut Criterion) {
    let workspace = create_test_workspace_50();

    c.bench_function("render_plantuml_50_nodes", |b| {
        b.iter(|| {
            let options = PlantUmlOptions::default();
            let _ = render_plantuml_c4(&workspace, PlantUmlViewType::Container, &options);
        });
    });
}

fn bench_render_structurizr(c: &mut Criterion) {
    let workspace = create_test_workspace_50();

    c.bench_function("render_structurizr_50_nodes", |b| {
        b.iter(|| {
            let options = StructurizrDslOptions::default();
            let _ = render_structurizr_dsl(&workspace, &options);
        });
    });
}

fn bench_render_svg(c: &mut Criterion) {
    let workspace = create_test_workspace_50();
    let config = LayoutConfig::default();
    let layout = compute_layout(&workspace, &config).unwrap();

    c.bench_function("render_svg_50_nodes", |b| {
        b.iter(|| {
            let _ = render_svg(&layout, &SvgTheme::default());
        });
    });
}

fn bench_render_mermaid_c4(c: &mut Criterion) {
    let workspace = create_test_workspace_50();

    // Extract containers and relationships
    let containers: Vec<_> = workspace.model.systems.iter()
        .flat_map(|s| s.containers.clone())
        .collect();
    let relationships: Vec<_> = workspace.model.relationships.clone();

    c.bench_function("render_mermaid_c4_50_nodes", |b| {
        b.iter(|| {
            let options = C4MermaidOptions::default();
            let _ = render_component_diagram(&containers, &relationships, &options);
        });
    });
}

fn bench_render_class_diagram(c: &mut Criterion) {
    // Create code elements from workspace
    let elements: Vec<CodeElement> = (0..50).map(|i| {
        CodeElement {
            id: ElementId::new(format!("elem-{}", i)),
            name: format!("Component{}", i),
            kind: CodeElementKind::Struct,
            visibility: Visibility::Public,
            path: Some(format!("src/components/{}.rs", i)),
            attributes: vec![],
            methods: vec![],
            relationships: vec![],
        }
    }).collect();

    let relationships: Vec<UmlRelationship> = (0..elements.len().saturating_sub(1))
        .map(|i| UmlRelationship {
            target_id: elements[i + 1].id.clone(),
            kind: UmlRelationKind::Association,
            label: None,
            confidence: 1.0,
        })
        .collect();

    c.bench_function("render_class_diagram_50_elements", |b| {
        b.iter(|| {
            let options = MermaidOptions::default();
            let _ = render_class_diagram(&elements, &relationships, &options);
        });
    });
}

fn bench_render_by_format(c: &mut Criterion) {
    let workspace = create_test_workspace_50();

    // Extract containers and relationships for mermaid_c4
    let containers: Vec<_> = workspace.model.systems.iter()
        .flat_map(|s| s.containers.clone())
        .collect();
    let relationships: Vec<_> = workspace.model.relationships.clone();

    let mut group = c.benchmark_group("render_formats");
    let formats = ["D2", "PlantUML", "Structurizr", "MermaidC4"];

    for format in formats.iter() {
        group.bench_function(BenchmarkId::from_parameter(format), |b| {
            b.iter(|| {
                match *format {
                    "D2" => {
                        let options = D2Options::default();
                        let _ = render_d2(&workspace, &options);
                    }
                    "PlantUML" => {
                        let options = PlantUmlOptions::default();
                        let _ = render_plantuml_c4(&workspace, PlantUmlViewType::Container, &options);
                    }
                    "Structurizr" => {
                        let options = StructurizrDslOptions::default();
                        let _ = render_structurizr_dsl(&workspace, &options);
                    }
                    "MermaidC4" => {
                        let options = C4MermaidOptions::default();
                        let _ = render_component_diagram(&containers, &relationships, &options);
                    }
                    _ => {}
                }
            });
        });
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_render_d2, bench_render_plantuml, bench_render_structurizr, bench_render_svg, bench_render_mermaid_c4, bench_render_class_diagram, bench_render_by_format
}
criterion_main!(benches);
