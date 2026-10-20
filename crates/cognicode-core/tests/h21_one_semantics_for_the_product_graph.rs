//! One semantics for the graph the product builds.
//!
//! The product asks the graph questions about infrastructure code: which
//! resource is this, what does it depend on. The graph answers with names,
//! and the name of a Terraform block or an Ansible play is the one the
//! domain reader gave it: `tf:{file}:{block}`, `ansible:{file}:play:{name}`.
//!
//! Two paths build that graph — the per-file cache and the product's own
//! builder — and one of them answered with the generic name of a tree-sitter
//! node, so the product asked for `aws_s3_bucket.app_bucket` and the graph
//! answered that it had only ever heard of `resource`. The tests of that
//! path were green while the path the product actually walks was wrong:
//! a green test of one way of building a graph says nothing about the other.
//!
//! ## What this file pins
//!
//! That the graph the product builds names a declaration the way the domain
//! reader named it, that the real consumer finds it there, and that both
//! paths name it the same way.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::SymbolId;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    IacQueryInput, handle_iac_query,
};
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};

/// The real fixtures, copied into a scratch directory so the repository
/// stays clean. The content is the versioned one: these tests run against
/// what the repository really holds, not against a toy.
fn copy_fixture(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .join(name);
    let target = dir.join(name);
    copy_tree(&source, &target);
    target
}

fn copy_tree(source: &std::path::Path, target: &std::path::Path) {
    std::fs::create_dir_all(target).expect("mkdir");
    for entry in std::fs::read_dir(source).expect("read fixture") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &destination);
        } else {
            std::fs::copy(&path, &destination).expect("copy fixture");
        }
    }
}

fn every_file(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    collect_files(dir, &mut found);
    found.sort();
    found
}

fn collect_files(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            collect_files(&path, found);
        } else {
            found.push(path);
        }
    }
}

#[tokio::test]
async fn the_consumer_finds_a_terraform_resource_the_way_it_is_named() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixtures = copy_fixture(dir.path(), "terraform-iac");
    let main = fixtures.join("main.tf");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "aws_s3_bucket.app_bucket".to_string(),
            depth: 2,
        },
    )
    .await
    .expect("the consumer finds the resource it asks for");

    assert_eq!(
        answer.resource_id,
        format!("tf:{}:aws_s3_bucket.app_bucket", main.display()),
        "the graph names the resource the way the domain reader named it"
    );
}

#[tokio::test]
async fn the_consumer_finds_an_ansible_play_the_way_it_is_named() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixtures = copy_fixture(dir.path(), "ansible-playbook");
    let site = fixtures.join("site.yml");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "Configure infrastructure".to_string(),
            depth: 2,
        },
    )
    .await
    .expect("the consumer finds the play it asks for");

    assert_eq!(
        answer.resource_id,
        format!("ansible:{}:play:Configure_infrastructure", site.display()),
        "the graph names the play the way the domain reader named it"
    );
}

#[test]
fn both_paths_name_the_same_declaration_the_same_way() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixtures = copy_fixture(dir.path(), "terraform-iac");
    let main = fixtures.join("main.tf");

    // The builder the product walks.
    let service = AnalysisService::new();
    service
        .build_project_graph(dir.path())
        .expect("the product builds its graph");
    let product = service.graph_cache().get();

    // The per-file cache, over the same files.
    let cache = PerFileGraphCache::with_project_dir(dir.path().to_string_lossy().into_owned());
    let paths = every_file(&fixtures);
    let borrowed: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    let per_file = cache.merge_with_report(&borrowed).graph;

    let expected = SymbolId::new(format!("tf:{}:aws_s3_bucket.app_bucket", main.display()));
    assert!(
        product.get_symbol(&expected).is_some(),
        "the graph the product builds names the resource the way the domain reader named it"
    );
    assert!(
        per_file.get_symbol(&expected).is_some(),
        "the per-file cache names it the same way, or the two paths answer different questions"
    );
}
