//! Layout benchmark — measures Sugiyama layout performance for various node counts

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

use cognicode_diagram::layout::sugiyama::compute_layout;
use cognicode_diagram::layout::types::LayoutConfig;
use cognicode_diagram::model::c4_types::{ContainerType, Container, ElementId, ElementLocation, SoftwareSystem};
use cognicode_diagram::model::relationships::{C4Relationship, C4RelationshipKind};
use cognicode_diagram::model::workspace::C4Workspace;

fn create_workspace_with_n_nodes(n: usize) -> C4Workspace {
    let mut workspace = C4Workspace::new("TestSystem");

    // Calculate how many systems and containers we need
    let containers_per_system = 5;
    let num_systems = (n + containers_per_system - 1) / containers_per_system;
    let containers_remaining = n;

    let mut total_containers = 0;
    for sys_idx in 0..num_systems {
        let system = SoftwareSystem {
            id: ElementId::new(format!("system-{}", sys_idx)),
            name: format!("System {}", sys_idx),
            description: format!("System {}", sys_idx),
            location: ElementLocation::Internal,
            containers: vec![],
        };
        workspace.model.systems.push(system);
    }

    // Add containers to systems
    for sys_idx in 0..num_systems {
        let remaining = containers_remaining.saturating_sub(total_containers);
        let to_add = remaining.min(containers_per_system);

        for cont_idx in 0..to_add {
            let container = Container {
                id: ElementId::new(format!("container-{}-{}", sys_idx, cont_idx)),
                name: format!("Container {}:{}", sys_idx, cont_idx),
                container_type: ContainerType::Service,
                technology: "Rust".to_string(),
                description: format!("Container {}:{}", sys_idx, cont_idx),
                path: None,
                components: vec![],
            };
            if let Some(system) = workspace.model.systems.get_mut(sys_idx) {
                system.containers.push(container);
                total_containers += 1;
            }
        }
    }

    // Add relationships between adjacent containers
    for i in 0..total_containers.saturating_sub(1) {
        let source_idx = i;
        let target_idx = (i + 1) % total_containers;
        workspace.model.relationships.push(C4Relationship::new(
            ElementId::new(format!("container-{}-{}", source_idx / containers_per_system, source_idx % containers_per_system)),
            ElementId::new(format!("container-{}-{}", target_idx / containers_per_system, target_idx % containers_per_system)),
            C4RelationshipKind::Calls,
        ));
    }

    workspace
}

fn bench_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout");

    for n in [20, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, &n| {
            let workspace = create_workspace_with_n_nodes(n);
            let config = LayoutConfig::default();

            b.iter(|| {
                let _ = compute_layout(&workspace, &config);
            });
        });
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_layout
}
criterion_main!(benches);
