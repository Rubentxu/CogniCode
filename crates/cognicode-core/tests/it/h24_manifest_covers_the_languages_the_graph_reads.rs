//! The manifest that says whether a cached graph is still fresh must cover
//! the same files the graph was built from.
//!
//! The graph reads every file whose extension the language registry
//! recognises — Terraform, Ansible, C#, Ruby and the rest. The manifest
//! that decides whether the cached graph can be reused listed nine
//! extensions by hand, so every file outside that list was invisible to it:
//! editing a `.tf` file left the cache "fresh", and the product answered
//! with the graph it had before the edit — reporting a complete build over
//! sources that no longer exist.
//!
//! The defect was not visible in the tests that existed, because they asked
//! whether the graph was built, not whether the answer could be a stale one.
//!
//! ## What this file pins
//!
//! That a change in a file the graph was built from is seen by the
//! freshness check, whatever language that file is written in.

use cognicode_core::domain::aggregates::call_graph::SymbolId;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};

/// Builds the graph over the working directory of the context.
///
/// The second call is the interesting one: the graph is already in memory,
/// so the only reason to rebuild is that the freshness check saw the
/// change. A check that cannot see it answers with the previous graph.
async fn build(ctx: &HandlerContext) {
    handle_build_graph(ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");
}

fn has_symbol(ctx: &HandlerContext, id: &str) -> bool {
    ctx.analysis_service()
        .graph_cache()
        .get()
        .get_symbol(&SymbolId::new(id))
        .is_some()
}

#[tokio::test]
async fn a_changed_terraform_file_is_seen_by_the_freshness_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    let main = dir.path().join("main.tf");
    std::fs::write(&main, "resource \"aws_s3_bucket\" \"before\" {}\n").expect("write");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    build(&ctx).await;

    let before = format!("tf:{}:aws_s3_bucket.before", main.display());
    assert!(
        has_symbol(&ctx, &before),
        "the graph names the resource that was in the file"
    );

    std::fs::write(&main, "resource \"aws_s3_bucket\" \"after\" {}\n").expect("write");

    build(&ctx).await;

    assert!(
        has_symbol(&ctx, &format!("tf:{}:aws_s3_bucket.after", main.display())),
        "the change was seen: the graph the product answers with is the one built after the file changed"
    );
    assert!(
        !has_symbol(&ctx, &before),
        "the graph does not still carry a declaration the file no longer has"
    );
}

#[tokio::test]
async fn a_changed_ansible_file_is_seen_by_the_freshness_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    let site = dir.path().join("site.yml");
    std::fs::write(
        &site,
        "---\n- name: Before\n  hosts: all\n  tasks:\n    - name: Ping\n      ping:\n",
    )
    .expect("write");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    build(&ctx).await;

    let before = format!("ansible:{}:play:Before", site.display());
    assert!(
        has_symbol(&ctx, &before),
        "the graph names the play that was in the file"
    );

    std::fs::write(
        &site,
        "---\n- name: After\n  hosts: all\n  tasks:\n    - name: Ping\n      ping:\n",
    )
    .expect("write");

    build(&ctx).await;

    assert!(
        has_symbol(&ctx, &format!("ansible:{}:play:After", site.display())),
        "the change was seen: the graph the product answers with is the one built after the file changed"
    );
    assert!(
        !has_symbol(&ctx, &before),
        "the graph does not still carry a declaration the file no longer has"
    );
}
