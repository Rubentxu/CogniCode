//! The goals that build the graph publish what the graph covers and what it
//! was built from.
//!
//! ## The gap
//!
//! `audit` and `find` ask the service for the project's graph. The service
//! measures two facts while it loads it: the coverage (the files the walk
//! could not read, the references without a declaration) and the manifest of
//! the sources the graph describes. The goals kept the answer and dropped
//! both, so the envelope the CLI wraps around the answer had nothing to read:
//! `state` was `unknown` and `basis.coverage` and `basis.freshness` were
//! `null`, on a tree the product had measured to the byte.
//!
//! ## What this file pins
//!
//! The answer exactly as the terminal publishes it: the goal runs, and the
//! envelope owner (`envelope_for`) wraps the result. The premise is observed
//! before the claim: a tree with a declined symlink must read `partial`, so
//! the channel is not simply always `complete`, and an edited file must change
//! the digest, so the freshness is not a constant.

use cognicode_core::interface::goals::{AUDIT, FIND, GoalEnvironment, GoalSpec};
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

/// The two goals that load the project's graph, with the arguments they need.
fn graph_goals() -> Vec<(&'static GoalSpec, Value)> {
    vec![(&AUDIT, json!({})), (&FIND, json!({ "symbol": "alpha" }))]
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), SOURCE).expect("write");
    dir
}

#[test]
fn a_readable_tree_is_published_complete_with_its_coverage_and_freshness() {
    let dir = project();
    let workspace = dir.path().canonicalize().expect("canonical workspace");

    for (goal, arguments) in graph_goals() {
        let envelope = envelope_of(goal, dir.path(), &arguments);
        let name = goal.name;

        assert_eq!(
            envelope["state"], "complete",
            "`{name}` measured the whole tree and must say so: {envelope}"
        );

        let coverage = &envelope["basis"]["coverage"];
        assert!(
            coverage.is_object(),
            "`{name}` dropped the coverage the graph measured: {envelope}"
        );
        assert_eq!(coverage["status"], "complete", "`{name}`: {envelope}");
        // The envelope publishes the holes as counts; the names are in the
        // answer itself (see the partial case below).
        assert_eq!(
            coverage["skipped_files"].as_u64(),
            Some(0),
            "`{name}`: {envelope}"
        );

        let freshness = &envelope["basis"]["freshness"];
        assert!(
            freshness.is_object(),
            "`{name}` dropped what the graph was built from: {envelope}"
        );
        assert_eq!(
            freshness["workspace"].as_str(),
            Some(workspace.to_string_lossy().as_ref()),
            "`{name}` names the canonical workspace: {envelope}"
        );
        assert!(
            freshness["snapshot"]
                .as_str()
                .is_some_and(|digest| !digest.is_empty()),
            "`{name}` publishes the digest of the sources it read: {envelope}"
        );
        assert!(
            matches!(
                freshness["graph_source"].as_str(),
                Some("built" | "cache" | "daemon")
            ),
            "`{name}` says where the graph came from: {envelope}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_declined_symlink_makes_the_goal_partial_and_names_the_file() {
    let dir = project();
    std::os::unix::fs::symlink("/nowhere", dir.path().join("link.rs")).expect("symlink");

    for (goal, arguments) in graph_goals() {
        let envelope = envelope_of(goal, dir.path(), &arguments);
        let name = goal.name;

        assert_eq!(
            envelope["state"], "partial",
            "`{name}` over a tree with a file it declined cannot read as complete: {envelope}"
        );
        let coverage = &envelope["basis"]["coverage"];
        assert_eq!(coverage["status"], "partial", "`{name}`: {envelope}");
        assert_eq!(
            coverage["skipped_files"].as_u64(),
            Some(1),
            "`{name}` counts the file it left out: {envelope}"
        );
        // The count is in the envelope; the file is named in the answer.
        assert!(
            envelope["result"]["coverage"]["skipped_files"]
                .to_string()
                .contains("link.rs"),
            "`{name}` names the file it left out: {envelope}"
        );
        // A build that did not read the whole tree keeps no manifest, so the
        // answer does not claim the identity of a tree it did not describe.
        assert!(
            envelope["basis"]["freshness"].is_null(),
            "`{name}` published a digest for a tree it read only in part: {envelope}"
        );
    }
}

#[test]
fn the_digest_follows_the_sources_the_graph_was_built_from() {
    let dir = project();

    for (goal, arguments) in graph_goals() {
        let before = envelope_of(goal, dir.path(), &arguments);
        let again = envelope_of(goal, dir.path(), &arguments);
        std::fs::write(
            dir.path().join("lib.rs"),
            format!("{SOURCE}pub fn gamma() -> u32 {{ 3 }}\n"),
        )
        .expect("edit");
        let after = envelope_of(goal, dir.path(), &arguments);
        std::fs::write(dir.path().join("lib.rs"), SOURCE).expect("restore");

        let digest = |envelope: &Value| {
            envelope["basis"]["freshness"]["snapshot"]
                .as_str()
                .map(str::to_string)
        };
        let name = goal.name;
        assert!(digest(&before).is_some(), "`{name}`: {before}");
        assert_eq!(
            digest(&before),
            digest(&again),
            "`{name}`: the same sources have the same digest"
        );
        assert_ne!(
            digest(&before),
            digest(&after),
            "`{name}`: an edited file changes the digest"
        );
    }
}
