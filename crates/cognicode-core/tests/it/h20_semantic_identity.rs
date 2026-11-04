//! The identity of a node that came from a semantic layer.
//!
//! A handler that reads a file says what that file declares, and it names
//! each thing it declares: `tf:main.tf:aws_s3_bucket.app_bucket`,
//! `ansible:site.yml:play:Configure_infrastructure`. That name is the
//! identity the product asks for, and the graph is where the product looks.
//!
//! The graph identifies a symbol by its fully qualified name, and the
//! generic path builds that name out of the file and the symbol. A node
//! that comes from a semantic layer was not declared by that shape: it was
//! declared by a handler, and the handler gave it a name. If the graph
//! renames it on the way in, the product asks for the name the handler gave
//! and the graph answers that it has never heard of it.
//!
//! ## What this file pins
//!
//! That a node that came from a semantic layer keeps the identity its
//! handler gave it, and that the identity is the one a reader can name
//! without running anything: a Terraform block is `tf:{fichero}:{bloque}` and
//! an Ansible play is `ansible:{fichero}:play:{nombre}`.

use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use std::collections::BTreeMap;

/// The real fixtures, read with the only honesty possible: if they are
/// missing, these tests fall. They are versioned and the tests run in the
/// repository.
fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sandbox/fixtures");
    let path = path.join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the real fixture must exist: it is versioned in the repository and this \
             test runs in the repository. If it is missing, it was deleted or moved. \
             Path: {} ({e})",
            path.display()
        )
    })
}

/// The graph of a directory, built the way the product builds it.
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

/// The names the graph knows, with the identity each one has.
fn identities_by_name(graph: &CallGraph) -> BTreeMap<String, String> {
    graph
        .symbol_ids()
        .map(|(id, s)| (s.name().to_string(), id.as_str().to_string()))
        .collect()
}

/// A block of Terraform arrives in the graph under the identity its handler
/// gave it.
#[test]
fn a_terraform_block_keeps_the_identity_its_handler_gave_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source = fixture("terraform-iac/main.tf");
    let graph = graph_for(tmp.path(), &[("main.tf", &source)]);
    let identities = identities_by_name(&graph);

    let identity = identities
        .get("aws_s3_bucket.app_bucket")
        .unwrap_or_else(|| {
            panic!(
                "the block of the real fixture has to be in the graph; got {:?}",
                identities.keys().collect::<Vec<_>>()
            )
        });

    let expected = format!(
        "tf:{}:aws_s3_bucket.app_bucket",
        tmp.path().join("main.tf").to_string_lossy()
    );
    assert_eq!(
        identity, &expected,
        "the identity of a block is the one its handler gave it: the file and \
         the name of the block, and nothing else."
    );
}

/// A play of Ansible arrives in the graph under the identity its handler
/// gave it.
#[test]
fn an_ansible_play_keeps_the_identity_its_handler_gave_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source = fixture("ansible-playbook/site.yml");
    let graph = graph_for(tmp.path(), &[("site.yml", &source)]);
    let identities = identities_by_name(&graph);

    let identity = identities
        .get("Configure infrastructure")
        .unwrap_or_else(|| {
            panic!(
                "the play of the real fixture has to be in the graph; got {:?}",
                identities.keys().collect::<Vec<_>>()
            )
        });

    let expected = format!(
        "ansible:{}:play:Configure_infrastructure",
        tmp.path().join("site.yml").to_string_lossy()
    );
    assert_eq!(
        identity, &expected,
        "the identity of a play is the one its handler gave it: the file and \
         the name of the play, and nothing else."
    );
}

/// The identity is not only in the graph: the symbol a reader finds by that
/// identity is the symbol the handler declared, with its name and its file.
#[test]
fn the_identity_finds_the_symbol_the_handler_declared() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source = fixture("ansible-playbook/site.yml");
    let graph = graph_for(tmp.path(), &[("site.yml", &source)]);

    let identity = format!(
        "ansible:{}:play:Verify_deployment",
        tmp.path().join("site.yml").to_string_lossy()
    );
    let symbol = graph
        .get_symbol(&SymbolId::new(&identity))
        .unwrap_or_else(|| {
            panic!(
                "the graph has to answer to the identity the handler gave; it \
                 knows {:?}",
                identities_by_name(&graph).keys().collect::<Vec<_>>()
            )
        });
    assert_eq!(
        symbol.name(),
        "Verify deployment",
        "the symbol the identity finds is the one the handler declared."
    );
}
