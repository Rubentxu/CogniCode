//! The search index reads a file the way the graph does.
//!
//! ## The gap
//!
//! A language with a reader of its own (Terraform, Ansible) declares its
//! symbols through that reader: a block is `aws_cloudwatch_log_group.app`, a
//! task is its name. The graph is built from those facts, and `reparse_on_edit`
//! hands the same facts to the search index. The other routes into the index —
//! the first population of a directory, `incremental_reindex`, a single file —
//! read the file with the generic parser, which does not know those grammars
//! and finds nothing in them. The same file was searchable after an edit and
//! invisible after a reindex.
//!
//! ## What this file pins
//!
//! For each file, the symbols the index holds after `index_file_from_path`
//! are the symbols the graph's reader declares for it: nothing missing,
//! nothing extra. A language without a reader of its own is the control: it
//! was read the generic way before and still is.
//!
//! The index is a set, so the comparison is on the set; the order of the
//! declarations belongs to the reader, which has its own tests.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::value_objects::SymbolSearchQuery;
use cognicode_core::infrastructure::semantic::semantic_search::{MatchType, SemanticSearchService};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .join(relative)
}

/// A symbol as the two sides must agree on it: what it is, what it is called
/// and where it sits.
fn identity(symbol: &Symbol) -> String {
    format!(
        "{:?} {} @ {:?}",
        symbol.kind(),
        symbol.name(),
        symbol.location()
    )
}

/// What the graph's reader says the file declares.
fn declared_by_the_graph(path: &Path) -> BTreeSet<String> {
    let facts = AnalysisService::new()
        .read_changed_file(path)
        .expect("the graph reads a versioned fixture");
    facts.symbols.iter().map(identity).collect()
}

/// What the index answers for every name the graph declares, and how many
/// entries it holds in total.
fn held_by_the_index(path: &Path, declared_names: &BTreeSet<String>) -> (BTreeSet<String>, usize) {
    let service = SemanticSearchService::new();
    service
        .index_file_from_path(path)
        .expect("the index takes a versioned fixture");

    let mut held = BTreeSet::new();
    for name in declared_names {
        let hits = service.search(SymbolSearchQuery {
            query: name.clone(),
            kinds: Vec::new(),
            max_results: 500,
        });
        for hit in hits
            .iter()
            .filter(|hit| matches!(hit.match_type, MatchType::Exact))
        {
            held.insert(identity(&hit.symbol));
        }
    }
    (held, service.index().len())
}

fn names_of(path: &Path) -> BTreeSet<String> {
    AnalysisService::new()
        .read_changed_file(path)
        .expect("the graph reads a versioned fixture")
        .symbols
        .iter()
        .map(|symbol| symbol.name().to_string())
        .collect()
}

fn assert_same_reading(path: &Path) {
    let declared = declared_by_the_graph(path);
    assert!(
        !declared.is_empty(),
        "premise: the graph declares symbols for {}",
        path.display()
    );

    let (held, total) = held_by_the_index(path, &names_of(path));

    assert_eq!(
        held,
        declared,
        "the index and the graph declare different symbols for {}",
        path.display()
    );
    assert_eq!(
        total,
        declared.len(),
        "the index holds entries the graph does not declare for {}",
        path.display()
    );
}

#[test]
fn a_terraform_file_is_searchable_by_what_the_graph_declares() {
    assert_same_reading(&fixture("terraform-iac/main.tf"));
}

#[test]
fn an_ansible_playbook_is_searchable_by_what_the_graph_declares() {
    assert_same_reading(&fixture("ansible-playbook/site.yml"));
}

#[test]
fn a_language_without_a_reader_of_its_own_is_read_as_before() {
    let dir = tempfile::tempdir().expect("a directory of its own");
    let path = dir.path().join("lib.rs");
    std::fs::write(
        &path,
        "pub fn first() -> u32 {\n    second()\n}\n\nfn second() -> u32 {\n    1\n}\n",
    )
    .expect("the file is written");

    assert_same_reading(&path);
}
