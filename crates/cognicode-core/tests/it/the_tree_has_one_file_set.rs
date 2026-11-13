//! The tree has one file set: every route that reads a project reads the same
//! files.
//!
//! ## The gap
//!
//! The source inventory is the owner of "which files are the project": the
//! blocklist, the performance skips, `.gitignore`, hidden entries and the
//! deterministic order are decided in one walk. Several routes nevertheless
//! walked the tree on their own, each with the exclusion list it had at the
//! time, so the same project answered with different files depending on who
//! asked: the graph saw what the inventory saw, the light index read a file
//! the `.gitignore` excluded, the per-file graph descended into hidden
//! directories, and the usage search read whatever its own filter let through.
//!
//! ## What this file pins
//!
//! One fixture, one premise, one test per route. The premise is observed first:
//! the inventory yields exactly `src/lib.rs`. Each route is then asked which of
//! the four symbols the fixture declares it knows, and the answer has to be
//! the one symbol the inventory's file declares.
//!
//! Nothing here sleeps and no route is given a network.

use cognicode_core::application::services::analysis_service::{AnalysisService, UsageSearchParams};
use cognicode_core::domain::value_objects::SymbolSearchQuery;
use cognicode_core::infrastructure::graph::{LightweightIndex, source_files};
use cognicode_core::infrastructure::semantic::semantic_search::SemanticSearchService;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The one symbol that lives in a file the project owns.
const KEPT: &str = "kept_symbol";

/// Every symbol the fixture declares, with the file that excludes it.
const DECLARED: [&str; 4] = [
    KEPT,
    // `generated/` is named in `.gitignore`.
    "ignored_by_gitignore",
    // `.hidden/` is a hidden directory.
    "hidden_dir_symbol",
    // `vendor/` is on the walk filter's list of artifact directories.
    "vendored_symbol",
];

/// A repository with one file the project owns and three that something
/// excludes, each for a different reason.
fn project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("a temporary directory");
    let root = tmp.path().join("project");
    // `.gitignore` is honoured inside a git repository only.
    std::fs::create_dir_all(root.join(".git")).expect("the repository marker");
    std::fs::write(root.join(".gitignore"), "generated/\n").expect("the ignore file");
    for (path, symbol) in [
        ("src/lib.rs", KEPT),
        ("generated/out.rs", "ignored_by_gitignore"),
        (".hidden/h.rs", "hidden_dir_symbol"),
        ("vendor/v.rs", "vendored_symbol"),
    ] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a file has a parent"))
            .expect("the directory is created");
        std::fs::write(file, format!("pub fn {symbol}() -> u32 {{ 1 }}\n"))
            .expect("the source is written");
    }
    (tmp, root)
}

/// What a route knows, as the declared symbols it can name.
fn only_kept() -> BTreeSet<String> {
    BTreeSet::from([KEPT.to_string()])
}

fn declared_among<'a>(names: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    names
        .into_iter()
        .filter(|name| {
            DECLARED
                .iter()
                .any(|declared| declared.eq_ignore_ascii_case(name))
        })
        .map(|name| name.to_ascii_lowercase())
        .collect()
}

#[test]
fn the_premise_the_inventory_reads_only_the_file_the_project_owns() {
    let (_tmp, root) = project();
    let inventory = source_files::inventory(&root);
    let read: Vec<String> = inventory
        .source_files()
        .map(|file| relative(&root, &file.path))
        .collect();

    assert_eq!(
        read,
        vec!["src/lib.rs".to_string()],
        "the fixture is only worth something if the inventory excludes the other three"
    );
}

#[test]
fn the_project_graph_knows_what_the_inventory_reads() {
    let (_tmp, root) = project();
    let service = AnalysisService::new();
    service
        .build_project_graph(&root)
        .expect("the graph builds");

    let graph = service.get_project_graph();
    let known = declared_among(graph.symbols().map(|symbol| symbol.name()));

    assert_eq!(known, only_kept(), "the graph and the inventory disagree");
}

#[test]
fn the_per_file_graph_knows_what_the_inventory_reads() {
    let (_tmp, root) = project();
    let service = AnalysisService::new();
    service
        .build_graph_per_file(&root)
        .expect("the per-file graph builds");

    let graph = service.get_project_graph();
    let known = declared_among(graph.symbols().map(|symbol| symbol.name()));

    assert_eq!(
        known,
        only_kept(),
        "the per-file graph and the inventory disagree"
    );
}

#[test]
fn the_light_index_knows_what_the_inventory_reads() {
    let (_tmp, root) = project();
    let mut index = LightweightIndex::new();
    index.build_index(&root).expect("the index builds");

    let known = declared_among(index.symbols());

    assert_eq!(
        known,
        only_kept(),
        "the light index and the inventory disagree"
    );
}

#[test]
fn the_semantic_index_knows_what_the_inventory_reads() {
    let (_tmp, root) = project();
    let service = SemanticSearchService::new();
    service
        .populate_from_directory(&root)
        .expect("the semantic index is populated");

    let mut names = Vec::new();
    for declared in DECLARED {
        let hits = service.search(SymbolSearchQuery {
            query: declared.to_string(),
            kinds: Vec::new(),
            max_results: 20,
        });
        names.extend(hits.into_iter().map(|hit| hit.symbol.name().to_string()));
    }
    let known = declared_among(names.iter().map(String::as_str));

    assert_eq!(
        known,
        only_kept(),
        "the semantic index and the inventory disagree"
    );
}

#[test]
fn the_usage_search_reads_what_the_inventory_reads() {
    let (_tmp, root) = project();
    let service = AnalysisService::new();

    let known: BTreeSet<String> = DECLARED
        .iter()
        .filter(|name| {
            !service
                .find_symbol_usages(UsageSearchParams {
                    project_dir: root.clone(),
                    symbol_name: (**name).to_string(),
                    include_declaration: true,
                    context_lines: None,
                    first_only_definition: false,
                })
                .expect("the usage search answers")
                .is_empty()
        })
        .map(|name| name.to_string())
        .collect();

    assert_eq!(
        known,
        only_kept(),
        "the usage search and the inventory disagree"
    );
}

/// `path` relative to `root`, with `/` separators.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
