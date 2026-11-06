//! The same project gives the same edges, every run.
//!
//! ## The guarantee
//!
//! The graph publishes its edges: the dependencies of the whole project,
//! and the same dependencies with their metadata. The map that holds the
//! edges was a hash map, and a hash map iterates in an order that differs
//! per instance, so the published order was an accident of the instance.
//!
//! Measured over eight pairs of independent builds of this witness before
//! the fix: the two published lists differed in **eight of eight** — and the
//! set of edges was the same in **eight of eight**. What changed between
//! two runs of the same project was the order and nothing else.
//!
//! ## The witness
//!
//! Real code, the domain of the core crate, read twice: two independent
//! builds of the same files, each with its own graph, and each hash map
//! with its own iteration order.
//!
//! A fixture would not do — a graph of a handful of symbols would not
//! fill the answers the graph publishes.
//!
//! ## What it asserts, and how
//!
//! Two things, and the first one carries the weight: the published list
//! is in the canonical order — it is compared against its own sorted
//! copy, which is a total check. Comparing two hash maps would only say
//! that two of them disagree, and would pass by luck once in 3,6 million.
//!
//! The second is the guarantee a consumer reads: two independent builds
//! of the same project publish the same list.
//!
//! ## What it does not assert
//!
//! It does not pin which edges exist: that moves with the repository's
//! own churn, and a test that pinned it would break on every commit that
//! touches the code it reads. It asserts that two runs of the same code
//! give the same answer, in the canonical order.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::DependencyType;
use std::path::PathBuf;
use std::sync::Arc;

/// The real code the witness reads: the domain of the core crate.
fn real_code() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/domain")
}

/// The real project, built independently each time.
fn real_project() -> Arc<CallGraph> {
    let service = AnalysisService::new();
    service
        .build_project_graph(&real_code())
        .expect("the product graph is built over real code");
    service.get_project_graph()
}

/// The edges the graph publishes, in the order it publishes them.
fn published_edges(graph: &CallGraph) -> Vec<(String, String, DependencyType)> {
    graph
        .all_dependencies()
        .map(|(source, target, kind)| {
            (
                source.as_str().to_string(),
                target.as_str().to_string(),
                *kind,
            )
        })
        .collect()
}

/// The edges with their metadata, in the order they are published.
fn published_edges_with_metadata(
    graph: &CallGraph,
) -> Vec<(String, String, DependencyType, String, String)> {
    graph
        .edges_with_metadata()
        .map(|(source, target, kind, provenance, confidence)| {
            (
                source.as_str().to_string(),
                target.as_str().to_string(),
                kind,
                format!("{provenance:?}"),
                format!("{confidence}"),
            )
        })
        .collect()
}

/// The first position where two published lists differ.
///
/// The witness reads a project with thousands of edges, and printing both
/// lists on every failure would bury the failure under a megabyte of
/// output. The position and the two entries that differ are what says
/// what went wrong.
fn first_difference(
    left: &[(String, String, DependencyType)],
    right: &[(String, String, DependencyType)],
) -> String {
    for (index, (published, canonical)) in left.iter().zip(right.iter()).enumerate() {
        if published != canonical {
            return format!(
                "position {index}: published {published:?}, canonical order says {canonical:?}"
            );
        }
    }
    format!(
        "the lists have different lengths: {} and {}",
        left.len(),
        right.len()
    )
}

/// The same project gives the same edges, in independent runs.
#[test]
fn the_same_project_gives_the_same_edges() {
    let first = real_project();
    let second = real_project();

    let first_edges = published_edges(&first);
    let second_edges = published_edges(&second);

    assert!(
        !first_edges.is_empty(),
        "the witness reads a project with edges: a graph without dependencies has nothing to disagree about"
    );

    // The published order is the canonical one: by source id, then by
    // target id and dependency type. This is a total check, not a
    // comparison of two hash maps.
    let mut canonical = first_edges.clone();
    canonical.sort();
    assert!(
        first_edges == canonical,
        "the edges come out ordered by source id, then target id and dependency type: {}",
        first_difference(&first_edges, &canonical)
    );

    assert!(
        first_edges == second_edges,
        "the same project publishes the same edges, in the same order: {}",
        first_difference(&first_edges, &second_edges)
    );

    // The walk that carries the metadata publishes the same edges in the
    // same order.
    let first_metadata = published_edges_with_metadata(&first);
    let second_metadata = published_edges_with_metadata(&second);
    assert!(
        first_metadata == second_metadata,
        "the same project publishes the same edges with their metadata, in the same order"
    );
    let metadata_order: Vec<(String, String, DependencyType)> = first_metadata
        .iter()
        .map(|(source, target, kind, _, _)| (source.clone(), target.clone(), *kind))
        .collect();
    assert!(
        metadata_order == first_edges,
        "both walks over the edges publish them in the same order: {}",
        first_difference(&metadata_order, &first_edges)
    );
}
