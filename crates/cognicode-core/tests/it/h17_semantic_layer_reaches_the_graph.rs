//! The semantic layer has to reach the graph the user queries.
//!
//! A `main.tf` with two blocks used to produce four symbols, and none of
//! them carried the identity of the block:
//!
//! ```text
//! .../main.tf:resource:1  name="resource"
//! .../main.tf:resource:5  name="resource"
//! .../main.tf:bucket:2    name="bucket"
//! .../main.tf:bucket:6    name="bucket"
//! ```
//!
//! Four symbols: the keyword and the name of the attribute. HCL is an
//! announced language, and its block produced two symbols.
//!
//! ## Why
//!
//! The semantic handler exists and is wired up, but it was invoked in ONE
//! place: the extraction seam. The path that builds the graph never
//! consulted it. Those are two authorities for the same question —"what is
//! a symbol in this language?"— and the second one won because it was the
//! one being asked.
//!
//! This is not a defect of HCL: a seam built to be optional stayed optional
//! in a sense nobody chose. The block identity was already delivered on the
//! extraction seam, and reaching the graph was assumed to follow. It did
//! not. This file moves the law from the seam to the product.
//!
//! ## The shape
//!
//! The fourth test is not about Terraform: it is about a language that has
//! no semantic layer. The seam is optional, and an optional branch nobody
//! exercises is not a seam: it is an `if` that can slip into the hot path
//! without any suite noticing. This file forces both sides to be covered.

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;

const TF_ONE_BLOCK: &str = r#"
resource "aws_s3_bucket" "only" {
  bucket = "x"
}
"#;

const TF_TWO_BLOCKS: &str = r#"
resource "aws_s3_bucket" "app_bucket" {
  bucket = "x"
}

resource "aws_s3_bucket" "audit_bucket" {
  bucket = "y"
}
"#;

/// The symbol names of the graph, sorted: comparison without depending on
/// the temporary directory.
fn symbol_names(graph: &CallGraph) -> Vec<String> {
    let mut names: Vec<String> = graph
        .symbol_ids()
        .map(|(_, s)| s.name().to_string())
        .collect();
    names.sort();
    names
}

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

/// The central law: the block reaches the graph with its domain identity,
/// not with the keyword of its header.
///
/// The set is EXACT, not a match: one symbol too many means that something
/// from the generic path survives mixed with the semantic layer, which is
/// exactly the defect this seam removes.
#[test]
fn a_terraform_block_reaches_the_graph_with_its_domain_identity() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("main.tf", TF_ONE_BLOCK)]);

    let names = symbol_names(&graph);
    assert_eq!(
        names,
        vec!["aws_s3_bucket.only".to_string()],
        "an HCL block must reach the graph as exactly one symbol with its \
         domain identity. A symbol named `resource` means the graph path is \
         still asking the generic parser; one symbol too many means the two \
         authorities are being mixed."
    );
}

/// One block, one symbol. The generic extraction delivers the node of the
/// block twice (same line, same column), and the domain layer turned it
/// into two nodes with the SAME id: the symbol count of an infrastructure
/// repository came out inflated and nothing revealed it.
#[test]
fn one_terraform_block_is_one_symbol() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("main.tf", TF_ONE_BLOCK)]);

    let occurrences = symbol_names(&graph)
        .iter()
        .filter(|n| *n == "aws_s3_bucket.only")
        .count();
    assert_eq!(
        occurrences, 1,
        "a block produces one symbol; {occurrences} were counted. The generic \
         extraction delivers the node of the block twice and, without \
         deduplication, the graph counts every block twice."
    );
}

/// Two blocks of the same type are not the same symbol.
#[test]
fn two_terraform_blocks_of_the_same_type_stay_distinct() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("main.tf", TF_TWO_BLOCKS)]);

    let names = symbol_names(&graph);
    assert!(
        names.iter().any(|n| n == "aws_s3_bucket.app_bucket")
            && names.iter().any(|n| n == "aws_s3_bucket.audit_bucket"),
        "two resources of the same type must reach the graph as two symbols; got {names:?}"
    );
}

/// The border. A language WITHOUT a semantic layer must not find out that
/// it exists: same symbol, same edge, same name. This test is not about
/// Terraform on purpose — it is the guarantee that the seam has not slipped
/// into the hot path of the other languages.
#[test]
fn a_language_without_a_semantic_layer_is_untouched() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let rust = "pub fn alpha() -> u32 { 1 }\npub fn caller() -> u32 { alpha() }\n";
    let graph = graph_for(tmp.path(), &[("lib.rs", rust)]);

    let names = symbol_names(&graph);
    assert!(
        names.iter().any(|n| n == "alpha") && names.iter().any(|n| n == "caller"),
        "the generic path must keep producing its symbols; got {names:?}"
    );
    assert!(
        graph.edge_count() >= 1,
        "`caller` calls `alpha`: the generic edge must still exist; {} edges \
         were counted",
        graph.edge_count()
    );
}
