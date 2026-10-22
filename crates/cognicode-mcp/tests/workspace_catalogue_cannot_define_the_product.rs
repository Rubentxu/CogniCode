//! The analysed workspace cannot define the product.
//!
//! The tool catalogue says which tools the product offers and, for each of
//! them, whether it changes something. The read-only posture filters on that
//! second answer, so the catalogue is what decides whether a mutating tool
//! can run.
//!
//! The catalogue used to be looked up by walking up from the directory the
//! server was pointed at. That directory is the *workspace*: the project a
//! user opened. A project that ships a `product/tools.json` of its own was
//! therefore able to say that `write_file` reads instead of writes, and the
//! read-only posture accepted it.
//!
//! That is the wrong direction for a security property. The authority of a
//! tool belongs to the product that implements it, not to the tree being
//! analysed. The tree is data; data does not get to lower a posture.
//!
//! The test drives the real binary over the real protocol, because the
//! failure mode it guards is invisible from the inside: the server answers
//! `tools/list` with a well-formed list, exit code 0, nothing on stderr.

mod common;

use common::McpSession;
use serde_json::{Value, json};

/// A workspace that ships a catalogue of its own, with one mutating tool
/// downgraded to a read-only one.
///
/// The catalogue starts as the product's own, so the only difference is the
/// one field under test.
fn workspace_that_downgrades_a_mutating_tool(repo: &std::path::Path) -> tempfile::TempDir {
    let workspace = tempfile::tempdir().expect("create the workspace");
    let product = workspace.path().join("product");
    std::fs::create_dir_all(&product).expect("create the product directory");

    let original = std::fs::read_to_string(repo.join("product").join("tools.json"))
        .expect("read the catalogue");
    let mut catalogue: Value =
        serde_json::from_str(&original).expect("the product catalogue must be valid JSON");

    let tools = catalogue["tools"]
        .as_array_mut()
        .expect("the catalogue must carry a `tools` array");
    let tool = tools
        .iter_mut()
        .find(|tool| tool["name"] == json!("write_file"))
        .expect("the catalogue must declare `write_file`");
    assert_eq!(
        tool["authority"],
        json!("mutating"),
        "precondition: `write_file` must be a mutating tool in the product's \
         own catalogue, otherwise this test proves nothing"
    );
    tool["authority"] = json!("read");

    std::fs::write(
        product.join("tools.json"),
        serde_json::to_string_pretty(&catalogue).expect("serialise the catalogue"),
    )
    .expect("write the workspace catalogue");

    workspace
}

/// A read-only server pointed at a workspace that claims `write_file` reads.
#[tokio::test]
async fn read_only_posture_ignores_the_catalogue_the_workspace_ships() {
    let repo = common::repo_root();
    let workspace = workspace_that_downgrades_a_mutating_tool(&repo);
    let elsewhere = tempfile::tempdir().expect("create a directory outside the repository");

    let mut session =
        McpSession::spawn_from_with_flags(workspace.path(), elsewhere.path(), &["--read-only"])
            .await
            .expect("spawn the server in read-only posture");

    let tools = session.list_tools().await.expect("tools/list");

    assert!(
        !tools.iter().any(|tool| tool == "write_file"),
        "the server advertised `write_file` in read-only posture because the \
         catalogue it read came from the analysed workspace, which declared \
         that tool as read-only. The authority of a tool is the product's, \
         not the tree's. Advertised: {tools:?}"
    );
}

/// The same workspace, without the read-only posture: the tool is still
/// offered, because the product really does have it. The fix must not turn
/// into "hide the tool everywhere".
#[tokio::test]
async fn a_mutating_tool_is_still_offered_when_the_posture_allows_it() {
    let repo = common::repo_root();
    let workspace = workspace_that_downgrades_a_mutating_tool(&repo);
    let elsewhere = tempfile::tempdir().expect("create a directory outside the repository");

    let mut session = McpSession::spawn_from_with_flags(workspace.path(), elsewhere.path(), &[])
        .await
        .expect("spawn the server");

    let tools = session.list_tools().await.expect("tools/list");

    assert!(
        tools.iter().any(|tool| tool == "write_file"),
        "the server stopped offering `write_file`; the workspace's catalogue \
         must not be able to remove a tool from the product either. \
         Advertised: {tools:?}"
    );
}
