//! HCL: a block must keep its type and its name.
//!
//! A generic extraction of an HCL block used to reduce it to its keyword:
//! `resource "aws_s3_bucket" "app_bucket" { ... }` produced the label
//! `"resource"`, and the type and the name were gone before the domain
//! layer could see them.
//!
//! Three consequences, all of them observable:
//!
//!  1. The file check looks for `"resource "` followed by a space. A label
//!     of `"resource"` alone does not contain it, so the domain layer never
//!     ran.
//!  2. Every `resource` block of the file shared a label, and therefore
//!     shared an identity. Two different resources were the same symbol.
//!  3. The edges that did come out came from the generic path
//!     (`call_types: ["expression"]`) and were noise: pieces of literals and
//!     fragments of source treated as call targets.
//!
//! The fix does not rewrite the domain layer: it hands back the name the
//! parser had already destroyed.

use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::infrastructure::parser::language_config::HCL_CONFIG;

fn labels_for(source: &str) -> Vec<String> {
    let result = extract_file(&HCL_CONFIG, std::path::Path::new("main.tf"), source, "h15");
    result.nodes.iter().map(|n| n.label.clone()).collect()
}

#[test]
fn an_hcl_block_keeps_its_type_and_name() {
    let labels = labels_for(
        r#"
resource "aws_s3_bucket" "app_bucket" {
  bucket = "x"
}
"#,
    );
    assert!(
        labels
            .iter()
            .any(|l| l.contains("aws_s3_bucket") && l.contains("app_bucket")),
        "an HCL block must keep its type and its name; got {labels:?}"
    );
}

#[test]
fn two_resources_of_the_same_type_are_not_the_same_symbol() {
    let labels = labels_for(
        r#"
resource "aws_s3_bucket" "app_bucket" {
  bucket = "x"
}

resource "aws_s3_bucket" "audit_bucket" {
  bucket = "y"
}
"#,
    );
    let has_app = labels.iter().any(|l| l.contains("app_bucket"));
    let has_audit = labels.iter().any(|l| l.contains("audit_bucket"));
    assert!(
        has_app && has_audit,
        "two resources of the same type must keep distinct names; got {labels:?}"
    );
}

#[test]
fn the_terraform_handler_becomes_reachable_and_unquotes() {
    // The file check requires the keyword followed by a space, so with the
    // complete label the domain layer does run. Its observable output is the
    // infrastructure identity `type.name`, already without quotes: the HCL
    // literal is `"aws_s3_bucket"`, and keeping the quotes contaminated both
    // the label and the identifier.
    let labels = labels_for(
        r#"
resource "aws_s3_bucket" "app_bucket" {
  bucket = "x"
}
"#,
    );
    assert!(
        labels.iter().any(|l| l == "aws_s3_bucket.app_bucket"),
        "the domain layer must produce the identity without quotes; got {labels:?}"
    );
    assert!(
        !labels.iter().any(|l| l.contains('"')),
        "no identity may keep the quotes of the HCL literal; got {labels:?}"
    );
}

#[test]
fn the_real_fixture_yields_resolvable_block_identities() {
    // The real fixture, not an invented fragment: it is the one that states
    // the relation `aws_s3_bucket_versioning -> aws_s3_bucket`.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/terraform-iac/main.tf");
    // The fixture is versioned in the repository and this test runs in the
    // repository: its absence is a regression, and falling is the only
    // honest reading. Returning early instead would leave the test green
    // over a deleted fixture.
    let source = std::fs::read_to_string(&fixture).unwrap_or_else(|e| {
        panic!(
            "the real fixture must exist: it is versioned in the repository and this \
             test runs in the repository. If it is missing, it was deleted or moved. \
             Path: {} ({e})",
            fixture.display()
        )
    });
    let labels = labels_for(&source);

    // Each identity is asked for EXACTLY, because that is what tells two
    // resources apart from one: `contains("app_bucket")` is also satisfied by
    // `aws_s3_bucket_versioning.app_bucket_versioning`, so an inclusion
    // check cannot see the difference.
    for expected in [
        "aws_s3_bucket.app_bucket",
        "aws_s3_bucket_versioning.app_bucket_versioning",
    ] {
        assert!(
            labels.iter().any(|l| l == expected),
            "the real fixture must produce the exact identity `{expected}`; \
             it produced {} labels and none matches exactly. An inclusion check \
             does not tell these two resources apart.",
            labels.len()
        );
    }
}
