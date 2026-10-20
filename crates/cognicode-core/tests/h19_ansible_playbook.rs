//! Ansible playbooks: what makes a `.yml` file a graph.
//!
//! A playbook is not a list of labels. It is a structure: plays contain
//! tasks, a task calls a module, a play uses roles. The generic extraction
//! reduces a YAML document to the node of the file itself, so a handler that
//! reads those labels never finds `hosts:`, never finds a play, and never
//! finds a task — and the playbook part of the graph is empty.
//!
//! ## What this file pins
//!
//! That the structure of a playbook arrives, and that it arrives with the
//! precision the file states:
//!
//! - the plays of the real fixture are nodes, and so are their tasks;
//! - a task calls the module its own key names — not a module guessed from
//!   a list of the ones seen before;
//! - a role, which lives in another file, is reported as unresolved and no
//!   node is invented for it: the coverage of a playbook is *per file*;
//! - a YAML file that is not a playbook is left exactly as it was.
//!
//! And that what arrives reaches the graph the user queries, not only the
//! extraction result that produces it.

use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::application::ingest::types::{ExtractionResult, TargetRef};
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::DependencyType;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use cognicode_core::infrastructure::parser::language_config::YAML_CONFIG;
use std::collections::BTreeMap;

/// The real fixture, read with the only honesty possible: if it is missing,
/// this test falls. It is versioned and the test runs in the repository.
fn fixture_source() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/ansible-playbook/site.yml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the real fixture must exist: it is versioned in the repository and this \
             test runs in the repository. If it is missing, it was deleted or moved. \
             Path: {} ({e})",
            path.display()
        )
    })
}

fn extract(source: &str) -> ExtractionResult {
    extract_file(
        &YAML_CONFIG,
        std::path::Path::new("site.yml"),
        source,
        "h19",
    )
}

/// The labels of the nodes the handler produced, sorted.
fn labels(result: &ExtractionResult) -> Vec<String> {
    let mut labels: Vec<String> = result.nodes.iter().map(|n| n.label.clone()).collect();
    labels.sort();
    labels
}

/// The identities of the edges that leave `source`, sorted and deduplicated.
fn targets_from(result: &ExtractionResult, source: &str, kind: &str) -> Vec<String> {
    let mut targets: BTreeMap<String, ()> = BTreeMap::new();
    for edge in result
        .edges
        .iter()
        .filter(|e| e.kind == kind && e.source == source)
    {
        match &edge.target_ref {
            TargetRef::Resolved(r) | TargetRef::Unresolved(r) => {
                targets.insert(r.clone(), ());
            }
        }
    }
    targets.into_keys().collect()
}

/// The identity of a node whose label is `label`, if the handler produced it.
fn id_of(result: &ExtractionResult, label: &str) -> Option<String> {
    result
        .nodes
        .iter()
        .find(|n| n.label == label)
        .map(|n| n.id.as_str().to_string())
}

// ── The structure ──────────────────────────────────────────────────────────

/// The fixture declares two plays. A playbook whose plays are invisible is a
/// file that contributes nothing to the graph.
#[test]
fn the_real_playbook_produces_its_plays() {
    let result = extract(&fixture_source());
    let labels = labels(&result);
    assert!(
        labels.contains(&"Configure infrastructure".to_string()),
        "the first play of the real fixture is named `Configure infrastructure` and \
         has to be a node. The handler produced {labels:?}."
    );
    assert!(
        labels.contains(&"Verify deployment".to_string()),
        "the second play of the real fixture is named `Verify deployment` and has \
         to be a node. The handler produced {labels:?}."
    );
}

/// The id of a declared thing carries its name, so it can be told from the
/// file alone: a reader who knows the playbook knows the id, without
/// running anything. An id made of positions says nothing until the parser
/// has run, and the id of a thing has to survive the parser being wrong.
#[test]
fn the_id_of_a_play_carries_its_name() {
    let result = extract(&fixture_source());
    let id = id_of(&result, "Configure infrastructure").expect("the play must exist");
    assert_eq!(
        id, "ansible:site.yml:play:Configure_infrastructure",
        "the id of a play is its name, so it can be told from the file alone."
    );
}

/// A play contains its tasks, and the containment is a relation between two
/// identities of the same file — which is the only kind of relation the
/// graph can show without inventing a node.
#[test]
fn the_tasks_of_a_play_belong_to_it() {
    let result = extract(&fixture_source());
    let play = id_of(&result, "Verify deployment").expect("the play must exist");
    let tasks = targets_from(&result, &play, "dependency.contains");
    assert_eq!(
        tasks.len(),
        3,
        "the second play of the real fixture declares three tasks —checking the \
         app, checking nginx, checking postgres— and the play contains them. \
         Got {tasks:?}."
    );
    for expected in [
        "Check app is responding",
        "Check nginx is responding",
        "Check postgres is accepting connections",
    ] {
        let id = id_of(&result, expected).unwrap_or_else(|| {
            panic!(
                "the task `{expected}` of the real fixture has to be a node. \
                 The handler produced {:?}.",
                labels(&result)
            )
        });
        assert!(
            tasks.contains(&id),
            "`{expected}` belongs to the play that declares it. Got {tasks:?}."
        );
    }
}

/// A block groups tasks, and the tasks it groups belong to the block and
/// not to the play that holds it. Ansible has three of them —the block
/// itself, the rescue and the always— and the three nest the same way.
#[test]
fn a_block_owns_the_tasks_it_groups() {
    let source = concat!(
        "- name: Deploy\n",
        "  hosts: all\n",
        "  tasks:\n",
        "    - name: Prepare\n",
        "      block:\n",
        "        - name: Stop the service\n",
        "          systemd:\n",
        "            name: app\n",
        "      rescue:\n",
        "        - name: Start it again\n",
        "          systemd:\n",
        "            name: app\n",
    );
    let result = extract(source);
    let block = id_of(&result, "Prepare").expect("the block must exist");
    let grouped = targets_from(&result, &block, "dependency.contains");
    for expected in ["Stop the service", "Start it again"] {
        let id = id_of(&result, expected).unwrap_or_else(|| {
            panic!(
                "the task `{expected}` has to be a node. The handler produced {:?}.",
                labels(&result)
            )
        });
        assert!(
            grouped.contains(&id),
            "`{expected}` belongs to the block that groups it, not to the \
             play. Got {grouped:?}."
        );
    }
}

/// The module is the key the task declares. The old handler compared the
/// label against a list of the modules it had seen before, so a module
/// outside that list —`postgresql_query`— was invisible even in a task it
/// had found. Reading the tree says what the task calls.
#[test]
fn the_module_is_the_key_the_task_declares() {
    let result = extract(&fixture_source());
    let task =
        id_of(&result, "Check postgres is accepting connections").expect("the task must exist");
    let calls = targets_from(&result, &task, "dependency.calls");
    assert!(
        calls.iter().any(|c| c.ends_with("postgresql_query")),
        "the task that checks postgres calls the `postgresql_query` module, and \
         the module is named by the task itself. Got {calls:?}."
    );
}

/// A role lives in another file. The relation is real, the destination is
/// not declared here, so it comes back unresolved — and no node is invented
/// for it. The coverage of a playbook is per file.
#[test]
fn a_role_is_reported_as_unresolved_not_invented() {
    let result = extract(&fixture_source());
    let play = id_of(&result, "Configure infrastructure").expect("the play must exist");
    let uses = targets_from(&result, &play, "dependency.imports");
    assert!(
        uses.iter().any(|u| u.contains("postgres")),
        "the first play of the real fixture uses the `postgres` role, and the \
         relation has to be reported. Got {uses:?}."
    );
    for target in &uses {
        let edge = result
            .edges
            .iter()
            .find(|e| {
                e.source == play && matches!(&e.target_ref, TargetRef::Unresolved(r) if r == target)
            })
            .expect("a role relation is unresolved: the role lives in another file");
        assert!(matches!(&edge.target_ref, TargetRef::Unresolved(_)));
        assert!(
            !result
                .nodes
                .iter()
                .any(|n| n.id.as_str() == target.as_str()),
            "no node is invented for `{target}`: the role is not declared in this \
             file."
        );
    }
}

// ── The product ───────────────────────────────────────────────────────────

fn graph_for(dir: &std::path::Path, files: &[(&str, &str)]) -> CallGraph {
    let mut paths = Vec::new();
    for (name, content) in files {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&p, content).expect("write fixture");
        paths.push(p);
    }
    let cache = PerFileGraphCache::with_project_dir(dir.to_string_lossy().into_owned());
    let borrowed: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    cache.merge_with_report(&borrowed).graph
}

fn ids_by_name(
    graph: &CallGraph,
) -> BTreeMap<String, cognicode_core::domain::aggregates::call_graph::SymbolId> {
    graph
        .symbol_ids()
        .map(|(id, s)| (s.name().to_string(), id.clone()))
        .collect()
}

/// The edge has to exist in the graph the user queries, not only in the
/// extraction result that produces it: what the handler read out of the tree
/// has to arrive too.
#[test]
fn the_nodes_reach_the_public_graph() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("site.yml", &fixture_source())]);
    let ids = ids_by_name(&graph);

    let (Some(play), Some(task)) = (
        ids.get("Verify deployment"),
        ids.get("Check app is responding"),
    ) else {
        panic!(
            "the plays and tasks of the real fixture have to be in the graph; got {:?}",
            ids.keys().collect::<Vec<_>>()
        );
    };

    let contained = graph
        .dependencies(play)
        .any(|(id, kind)| id == task && *kind == DependencyType::Contains);
    assert!(
        contained,
        "the relation between a play and its task has to be in the graph, not \
         only in the extraction result."
    );
}

/// The opposite of the law above, and that is why the pair matters: a
/// handler that claimed every YAML file would pass the test above by
/// inventing nodes.
#[test]
fn a_plain_yaml_file_is_left_alone() {
    let source = "version: 3\nservices:\n  web:\n    image: nginx\n    ports:\n      - \"80:80\"\n";
    let result = extract(source);
    let labels = labels(&result);
    assert_eq!(
        labels,
        vec!["site.yml".to_string()],
        "a YAML file that declares no play, task or role is not a playbook, and \
         the handler has to leave it exactly as the generic extraction left it."
    );
}
