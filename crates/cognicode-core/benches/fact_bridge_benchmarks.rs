//! Criterion benchmarks for the E37 fact bridge and fact-sourced projections.
//!
//! ADVISORY BASELINE (E37 design D8): this file captures the FIRST bridge
//! baseline — there is NO threshold on these benchmarks in M2, and they are
//! NOT a gate. The hard perf gate for the M2 exit remains e36's
//! `just lsi-baseline compare --fail-above N` over the unchanged 24
//! default-path benchmarks in `graph_benchmarks.rs`. A future milestone
//! turns this captured baseline into the bridge's own reference.
//!
//! Run (advisory capture):
//! `cargo bench -p cognicode-core --bench fact_bridge_benchmarks --features evidence-kernel,multimodal`
//!
//! Everything is feature-gated behind `evidence-kernel` (the generic
//! projection additionally needs `multimodal`); with the feature off the
//! bench target compiles to an empty binary so the default bench sweep is
//! unaffected.

// `criterion_group!` / `criterion_main!` are macros consumed at the
// bottom of this file under `#[cfg(feature = "evidence-kernel")]`.  When
// that feature is OFF (the default), the imports would otherwise be
// unused and trip `-D warnings`.  The `#[allow(unused_imports)]` is
// attached directly to the `use` to scope the lint suppression tightly.
#[allow(unused_imports)]
use criterion::{criterion_group, criterion_main};

#[cfg(feature = "evidence-kernel")]
mod bridge_benches {
    use std::path::Path;
    use std::sync::Arc;

    use criterion::{Criterion, black_box};

    use cognicode_core::application::fact_bridge::FactBatchBuilder;
    use cognicode_core::application::ingest::extractor::extract_file;
    use cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry;
    use cognicode_core::domain::evidence_kernel::fact::Fact;
    use cognicode_core::domain::evidence_kernel::ids::SnapshotId;
    use cognicode_core::domain::evidence_kernel::ports::{FactStore, SchemaRegistry};
    // The generic-projection surface is dual-gated (`evidence-kernel` AND
    // `multimodal`, DEAD-1): gate these references so the bench compiles
    // with `evidence-kernel` alone (the two fact-path benches stay valid).
    #[cfg(feature = "multimodal")]
    use cognicode_core::domain::ports::generic_graph_projection::GenericGraphProjectionPort;
    use cognicode_core::domain::value_objects::WorkspaceId;
    use cognicode_core::infrastructure::evidence_kernel::in_memory::{
        InMemoryFactStore, InMemorySchemaRegistry,
    };
    use cognicode_core::infrastructure::graph::CallGraphProjection;
    #[cfg(feature = "multimodal")]
    use cognicode_core::infrastructure::graph::generic_graph_projection::FactGenericGraphProjection;
    use cognicode_core::infrastructure::parser::language_config::PYTHON_CONFIG;

    const SNAPSHOT: SnapshotId = SnapshotId::new(1);
    const FILES: usize = 1000;

    /// Generates one small Python source with `index` baked in so every
    /// file carries distinct symbol identities and a couple of call edges.
    fn generate_python_source(index: usize) -> String {
        let mut source = String::new();
        source.push_str(&format!("import os\n\n# file {index}\n\n"));
        for f in 0..5 {
            source.push_str(&format!("def func_{index}_{f}(x):\n"));
            source.push_str(&format!("    return helper_{index}(x) + {f}\n\n"));
        }
        source.push_str(&format!("def helper_{index}(x):\n    return x\n"));
        source
    }

    /// Extracts `FILES` generated sources into a canonical fact batch
    /// (producer `DeterministicAnalyzer`, design D1/D3).
    fn build_facts() -> Vec<Fact> {
        let mut builder = FactBatchBuilder::new(SNAPSHOT);
        for index in 0..FILES {
            let source = generate_python_source(index);
            let path = Path::new(&format!("bench/src/file_{index}.py")).to_path_buf();
            let result = extract_file(&PYTHON_CONFIG, &path, &source, "bench-hash");
            builder.add_extraction(&result);
        }
        builder.finish()
    }

    /// A bootstrapped in-memory kernel store.
    fn fresh_store() -> Arc<InMemoryFactStore> {
        let registry = InMemorySchemaRegistry::new();
        bootstrap_registry(&registry as &dyn SchemaRegistry).expect("canonical bootstrap");
        Arc::new(InMemoryFactStore::new(Arc::new(registry)))
    }

    /// Fact COMMIT of a 1000-file batch (validation + insertion), design D8.
    pub fn fact_commit_1000_files(c: &mut Criterion) {
        let facts = build_facts();
        let workspace = WorkspaceId::try_new("ws-bench").expect("valid workspace");
        let mut group = c.benchmark_group("fact_bridge");
        group.throughput(criterion::Throughput::Elements(facts.len() as u64));
        group.bench_function("fact_commit_1000_files", |b| {
            b.iter(|| {
                let store = fresh_store();
                let committed = tokio_test::block_on(store.commit(
                    black_box(&workspace),
                    &SNAPSHOT,
                    black_box(facts.clone()),
                ))
                .expect("bench commit");
                black_box(committed.len())
            })
        });
        group.finish();
    }

    /// `CallGraphProjection::from_facts` over the committed 1000-file batch,
    /// design D4/D8.
    pub fn call_graph_from_facts_1000_files(c: &mut Criterion) {
        let facts = build_facts();
        let mut group = c.benchmark_group("fact_bridge");
        group.throughput(criterion::Throughput::Elements(facts.len() as u64));
        group.bench_function("call_graph_from_facts_1000_files", |b| {
            b.iter(|| {
                let projection = CallGraphProjection::from_facts(black_box(&facts));
                black_box((projection.node_count(), projection.edge_count()))
            })
        });
        group.finish();
    }

    /// Generic projection build over the committed 1000-file batch (port
    /// path: `facts_in_snapshot` + fact→node/edge mapping), design D5/D8.
    /// Requires `multimodal` for the `GraphNode`/`GraphEdge` output types.
    #[cfg(feature = "multimodal")]
    pub fn generic_projection_1000_files(c: &mut Criterion) {
        let facts = build_facts();
        let store = fresh_store();
        let workspace = WorkspaceId::try_new("ws-bench").expect("valid workspace");
        tokio_test::block_on(store.commit(&workspace, &SNAPSHOT, facts)).expect("bench commit");
        let adapter = FactGenericGraphProjection::new(store);
        let mut group = c.benchmark_group("fact_bridge");
        group.bench_function("generic_projection_1000_files", |b| {
            b.iter(|| {
                let projection =
                    tokio_test::block_on(adapter.project(black_box(&workspace), &SNAPSHOT))
                        .expect("bench projection");
                black_box((projection.nodes.len(), projection.edges.len()))
            })
        });
        group.finish();
    }
}

#[cfg(all(feature = "evidence-kernel", feature = "multimodal"))]
criterion_group!(
    benches,
    bridge_benches::fact_commit_1000_files,
    bridge_benches::call_graph_from_facts_1000_files,
    bridge_benches::generic_projection_1000_files,
);

#[cfg(all(feature = "evidence-kernel", not(feature = "multimodal")))]
criterion_group!(
    benches,
    bridge_benches::fact_commit_1000_files,
    bridge_benches::call_graph_from_facts_1000_files,
);

#[cfg(feature = "evidence-kernel")]
criterion_main!(benches);

/// With the feature off the bench target is an empty binary: `cargo bench`
/// over the default feature set keeps working untouched.
#[cfg(not(feature = "evidence-kernel"))]
fn main() {}
