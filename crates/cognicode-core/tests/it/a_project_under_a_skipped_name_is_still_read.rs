//! A project that lives under a skipped name is still a project.
//!
//! The walk filter decides what a project does not contain: `target`,
//! `build`, `.cache` and the rest are artifacts, not source. That
//! judgement belongs to the names *inside* the project. A directory
//! above it that happens to be called `build` says nothing about what
//! the project holds, and a project that lives under one of those names
//! still has source in it.
//!
//! The release candidate gate found this the hard way. Its clean-clone
//! stage ran inside a cache directory, the filter matched that name on
//! the way in, and every graph of that run came back empty: sixteen
//! tests red, all of them saying the same thing — the project has no
//! symbols.
//!
//! This file pins the contract on the three routes that read a project:
//! the inventory the service builds from, the handler that answers over
//! MCP, and the index the on-demand graph queries.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::infrastructure::graph::LightweightIndex;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use std::path::PathBuf;

/// A Rust project whose parent directory carries one of the skipped
/// names. The project itself is named plainly.
fn project_under_a_skipped_name() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("build").join("project");
    std::fs::create_dir_all(project.join("src")).expect("create src");
    std::fs::write(
        project.join("src").join("lib.rs"),
        "pub fn answer() -> u32 { 42 }\n",
    )
    .expect("write lib.rs");
    (tmp, project)
}

/// The name of the symbol the fixture declares.
const SYMBOL: &str = "answer";

#[test]
fn the_inventory_reads_a_project_under_a_skipped_name() {
    let (_tmp, project) = project_under_a_skipped_name();
    let service = AnalysisService::new();
    service
        .build_project_graph(&project)
        .expect("the graph builds");

    let graph = service.get_project_graph();
    assert!(
        graph.symbols().any(|s| s.name() == SYMBOL),
        "the project declares a symbol and the walk did not see it"
    );
}

#[tokio::test]
async fn the_mcp_handler_reads_a_project_under_a_skipped_name() {
    let (_tmp, project) = project_under_a_skipped_name();
    let ctx = HandlerContext::builder().with_working_dir(&project).build();

    let answer = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the handler answers");

    assert!(
        answer.symbols_found > 0,
        "the handler reported no symbols for a project that declares one"
    );
}

#[test]
fn the_lightweight_index_reads_a_project_under_a_skipped_name() {
    let (_tmp, project) = project_under_a_skipped_name();
    let mut index = LightweightIndex::new();
    index.build_index(&project).expect("the index builds");

    let locations = index.find_symbol(SYMBOL);
    assert!(
        !locations.is_empty(),
        "the index holds no location for a symbol the project declares"
    );
}
