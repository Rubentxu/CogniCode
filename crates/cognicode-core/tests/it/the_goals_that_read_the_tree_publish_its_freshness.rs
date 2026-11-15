//! The goals that read the tree without building the graph publish the
//! freshness of the tree they read.
//!
//! ## The gap
//!
//! `status` and `context` are the goals a client calls before deciding: the
//! first one is `cognicode` without arguments. They do not build the graph,
//! so they published no freshness at all —and a freshness is a *measurement*,
//! not a thing that comes with the graph: the digest of the source manifest
//! describes the TREE, and the tree is what these goals read.
//!
//! The digest of the tree is the same digest the graph route publishes. The
//! manifest is built by the same two calls in both routes (`tree_state` and
//! `build_manifest`), and `source_digest` hashes the relative path and the
//! content hash of every file —not the symbol counts the graph fills in
//! afterwards. Measured over this repository: the digest of the manifest
//! built without a graph and the digest of the manifest the graph route
//! publishes are the same string, byte for byte.
//!
//! ## What this file pins
//!
//! The answer exactly as the terminal publishes it: the goal runs, and the
//! envelope owner (`envelope_for`) wraps the result. Three facts:
//!
//! - a goal that read the tree publishes a freshness (workspace, digest);
//! - it publishes NO `graph_source`: no graph was built, and a word that
//!   names where a graph came from would be a word nobody measured;
//! - its digest is the digest the graph route publishes for the same tree:
//!   two answers about the same tree can be compared.

use cognicode_core::interface::goals::{CONTEXT, GoalEnvironment, GoalSpec, OVERVIEW, STATUS};
use cognicode_core::interface::hateoas::{HandleContext, envelope_for};
use serde_json::{Value, json};
use std::path::Path;

const SOURCE: &str = "pub fn alpha() -> u32 { beta() + 1 }\npub fn beta() -> u32 { 2 }\n";

/// The goal's answer, wrapped the way `cognicode <goal> --format json` does.
fn envelope_of(goal: &GoalSpec, workspace: &Path, arguments: &Value) -> Value {
    let environment = GoalEnvironment::new(workspace, None);
    let answer = (goal.run)(&environment, arguments).expect("the goal answers");
    let context = HandleContext {
        workspace: workspace.to_string_lossy().to_string(),
        snapshot: "none".to_string(),
    };
    let args = arguments.as_object().cloned().unwrap_or_default();
    serde_json::to_value(envelope_for(goal.name, &args, &answer, &context))
        .expect("the envelope is serialisable")
}

/// The two goals that read the tree and never build the graph.
fn tree_goals() -> Vec<(&'static GoalSpec, Value)> {
    vec![(&STATUS, json!({})), (&CONTEXT, json!({}))]
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), SOURCE).expect("write");
    dir
}

/// The digest the envelope published for a goal.
fn digest_of(envelope: &Value) -> Option<String> {
    envelope["basis"]["freshness"]["snapshot"]
        .as_str()
        .map(str::to_string)
}

#[test]
fn a_goal_that_reads_the_tree_publishes_the_freshness_of_that_tree() {
    let dir = project();
    let workspace = dir.path().canonicalize().expect("canonical workspace");

    for (goal, arguments) in tree_goals() {
        let envelope = envelope_of(goal, dir.path(), &arguments);
        let name = goal.name;

        let freshness = &envelope["basis"]["freshness"];
        assert!(
            freshness.is_object(),
            "`{name}` measured the tree it read and published no freshness"
        );
        assert_eq!(
            freshness["workspace"].as_str(),
            Some(workspace.to_string_lossy().as_ref()),
            "`{name}` names the canonical workspace: {freshness}"
        );
        assert!(
            freshness["snapshot"]
                .as_str()
                .is_some_and(|digest| !digest.is_empty()),
            "`{name}` publishes the digest of the sources it read: {freshness}"
        );
        assert!(
            freshness["graph_source"].is_null(),
            "`{name}` built no graph, so it says no graph: {freshness}"
        );
    }
}

#[test]
fn the_freshness_of_a_goal_that_reads_the_tree_is_the_one_the_graph_publishes() {
    let dir = project();

    let from_graph = digest_of(&envelope_of(&OVERVIEW, dir.path(), &json!({})));
    assert!(
        from_graph.is_some(),
        "the goal that builds the graph publishes no digest for the same tree"
    );

    for (goal, arguments) in tree_goals() {
        let envelope = envelope_of(goal, dir.path(), &arguments);
        let name = goal.name;

        assert_eq!(
            digest_of(&envelope),
            from_graph,
            "`{name}` speaks of another tree than the goal that builds the graph: {:?}",
            envelope["basis"]["freshness"]
        );
    }
}

#[test]
fn the_digest_follows_the_sources_the_goal_read() {
    let dir = project();

    for (goal, arguments) in tree_goals() {
        let before = envelope_of(goal, dir.path(), &arguments);
        let again = envelope_of(goal, dir.path(), &arguments);
        std::fs::write(
            dir.path().join("lib.rs"),
            format!("{SOURCE}pub fn gamma() -> u32 {{ 3 }}\n"),
        )
        .expect("edit");
        let after = envelope_of(goal, dir.path(), &arguments);
        std::fs::write(dir.path().join("lib.rs"), SOURCE).expect("restore");

        let name = goal.name;
        assert!(digest_of(&before).is_some(), "`{name}` published no digest");
        assert_eq!(
            digest_of(&before),
            digest_of(&again),
            "`{name}`: the same sources have the same digest"
        );
        assert_ne!(
            digest_of(&before),
            digest_of(&after),
            "`{name}`: an edited file changes the digest"
        );
    }
}
