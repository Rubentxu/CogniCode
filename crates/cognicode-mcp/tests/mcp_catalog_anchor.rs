//! The tool catalogue must resolve from the **configured workspace**,
//! not from whatever directory the process happened to inherit.
//!
//! Regression: `tools/list` returned `{"tools":[]}` against a healthy
//! server. The catalogue (`product/tools.json`) was located by walking
//! up from the process's current working directory, with a fallback to
//! `CARGO_MANIFEST_DIR` — a variable that only exists while `cargo` is
//! compiling, never in a shipped binary. The `--cwd` value the operator
//! (or the MCP client config) passes was used for analysis but never
//! reached the catalogue resolver.
//!
//! Why every other test missed it: `McpSession::spawn` passes `--cwd`
//! but does **not** set the child's process CWD, so the server inherits
//! the test's CWD — the crate root, always inside the repository. The
//! walk-up therefore always found the catalogue. A real MCP client
//! spawns the server with CWD set to the user's project, which is
//! outside the repository, and the catalogue resolved empty.
//!
//! The failure mode is the worst kind: exit code 0, no stderr
//! diagnostic, a well-formed JSON-RPC response, and a server that looks
//! healthy while advertising nothing. An agent integrating against it
//! sees "this server has no tools" and concludes the product is empty.
//!
//! ## What closes it, and what each test is for
//!
//! Two independent layers now keep this true, and this file pins the
//! **observable** one:
//!
//! 1. **Resolution** — the catalogue is located from the configured
//!    workspace (`--cwd`), then the process CWD, then the executable.
//!    Pinned by `a014_product_asset_resolution`, at the resolver's own
//!    level, so a regression there is caught even though this test
//!    would still pass.
//! 2. **Embedding** — `product/tools.json` is compiled into the binary
//!    by `include_str!`, so no install layout can lose it. This test
//!    pins the consequence an operator actually sees.
//!
//! Keeping both is deliberate. Layer 1 alone leaves a packaged binary
//! that has no product data on disk resolving empty; layer 2 alone
//! leaves a binary that cannot be pointed at a checkout's catalogue.
//! The wire test covers the symptom, the resolver test covers the
//! mechanism, and neither can be deleted without losing the other.

mod common;

use common::McpSession;

/// One well-known tool the catalogue must advertise. If the catalogue
/// resolved, this is present; if it did not, `tools/list` is empty.
const SENTINEL_TOOL: &str = "build_graph";

#[tokio::test]
async fn tools_list_is_not_empty_when_spawned_outside_the_repository() {
    let repo = common::repo_root();

    // A directory guaranteed to be outside the repository: a tempdir
    // under /tmp has no `product/tools.json` in any of its ancestors.
    let outside = tempfile::tempdir().expect("create tempdir outside the repo");
    assert!(
        !outside.path().starts_with(&repo),
        "test setup: the tempdir must be outside the repository"
    );

    // `--cwd` points at the repository (the analysis workspace) while the
    // process CWD points outside it. This is exactly how an MCP client
    // launches the server.
    let mut session = McpSession::spawn_from(&repo, outside.path())
        .await
        .expect("spawn server from outside the repository");

    let tools = session.list_tools().await.expect("tools/list");

    assert!(
        !tools.is_empty(),
        "tools/list returned an empty catalogue when the server was \
         launched from outside the repository (process CWD={}, \
         --cwd={}). The catalogue must resolve from the configured \
         workspace, not from the inherited process CWD: an empty \
         catalogue is indistinguishable from a product with no tools.",
        outside.path().display(),
        repo.display()
    );

    assert!(
        tools.iter().any(|t| t == SENTINEL_TOOL),
        "expected `{SENTINEL_TOOL}` among the advertised tools when the \
         catalogue resolves from the configured workspace; got {tools:?}"
    );
}

/// The same launch context under `--read-only` must still advertise the
/// read set. This guards the second half of the chain: the read-only
/// filter hides mutating tools, but if the *catalogue* resolved empty
/// first, the filter has nothing to filter and the read set disappears
/// too — which is precisely the "20 tools became 0" symptom the operator
/// reported, and which a mutating-tools test would not catch on its own.
#[tokio::test]
async fn read_only_tools_are_still_advertised_when_spawned_outside_the_repository() {
    let repo = common::repo_root();
    let outside = tempfile::tempdir().expect("create tempdir outside the repo");

    let mut session = McpSession::spawn_from_with_flags(&repo, outside.path(), &["--read-only"])
        .await
        .expect("spawn server in read-only posture");

    let tools = session.list_tools().await.expect("tools/list");
    assert!(
        !tools.is_empty(),
        "read-only posture advertised zero tools when spawned outside the \
         repository: the catalogue resolved empty before the read-only \
         filter had anything to filter"
    );
    assert!(
        tools.iter().any(|t| t == SENTINEL_TOOL),
        "expected the read sentinel `{SENTINEL_TOOL}` in the read-only \
         catalogue; got {tools:?}"
    );
}
