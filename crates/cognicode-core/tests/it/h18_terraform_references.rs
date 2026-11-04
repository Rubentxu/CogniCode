//! Terraform relations: what makes a `.tf` file a graph.
//!
//! The relations of a Terraform configuration live in the body of its
//! blocks —`bucket = aws_s3_bucket.app_bucket.id`, on line 55 of the
//! fixture— and the header of a block is not its body. Reading labels
//! therefore finds no relations at all, and the graph it produces has no
//! relation edges.
//!
//! ## What this file pins
//!
//! A relation reaches the public graph by its *declared identity*, not by a
//! positional guess: the handler knows the set of blocks the file declares
//! —it has just built it— and it is the only place where both facts exist
//! at once.
//!
//! And what does not reach it: `var.environment` is used inside `main.tf`
//! but declared in `variables.tf`. That edge comes out unresolved and the
//! graph does not count it. The coverage of relations is *per file*, and
//! this file says so out loud instead of pretending it is global.

use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::application::ingest::types::{ExtractionResult, TargetRef};
use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::value_objects::DependencyType;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use cognicode_core::infrastructure::parser::language_config::HCL_CONFIG;
use std::collections::BTreeMap;

/// The real fixture, read with the only honesty possible: if it is missing,
/// this test falls. It is versioned and the test runs in the repository.
fn fixture_source() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/terraform-iac/main.tf");
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
    extract_file(&HCL_CONFIG, std::path::Path::new("main.tf"), source, "h18")
}

fn tf_id(identity: &str) -> String {
    format!("tf:main.tf:{identity}")
}

/// The destinations of the relation edges that leave `source`, sorted and
/// without repetition: a relation is a pair, not a count of how many times
/// it was read.
///
/// The pair `(block, destination)` is deduplicated because reading
/// `var.environment` twice inside the same block is ONE relation, not two.
/// Emitting it twice informs nobody and muddies the count the graph shows.
fn references_from(result: &ExtractionResult, source: &str) -> Vec<String> {
    let mut targets: BTreeMap<String, ()> = BTreeMap::new();
    for edge in result
        .edges
        .iter()
        .filter(|e| e.kind == "dependency.references" && e.source == source)
    {
        match &edge.target_ref {
            TargetRef::Resolved(r) | TargetRef::Unresolved(r) => {
                targets.insert(r.clone(), ());
            }
        }
    }
    targets.into_keys().collect()
}

// ── The seam ───────────────────────────────────────────────────────────────

/// The law of this file: the relation between the versioned bucket and the
/// bucket it versions, on the real fixture, by its two exact identities.
#[test]
fn the_real_fixture_links_the_versioned_bucket_to_its_bucket() {
    let result = extract(&fixture_source());
    let versioning = tf_id("aws_s3_bucket_versioning.app_bucket_versioning");
    let targets = references_from(&result, &versioning);
    assert_eq!(
        targets,
        vec![tf_id("aws_s3_bucket.app_bucket")],
        "`bucket = aws_s3_bucket.app_bucket.id` (line 55 of the real fixture) must \
         produce ONE relation from the versioned block to the bucket. Got {targets:?}."
    );
}

/// Two different consumers of the same bucket: the relation is not an
/// artefact of the first match.
#[test]
fn both_consumers_of_the_bucket_are_linked() {
    let result = extract(&fixture_source());
    let bucket = tf_id("aws_s3_bucket.app_bucket");
    for consumer in [
        "aws_s3_bucket_versioning.app_bucket_versioning",
        "aws_s3_bucket_server_side_encryption_configuration.app_bucket_encryption",
    ] {
        let from = tf_id(consumer);
        assert_eq!(
            references_from(&result, &from),
            vec![bucket.clone()],
            "`{consumer}` consumes the bucket (lines 55 and 63 of the real fixture)."
        );
    }
}

/// The resolution is against the identities declared in the file: what is
/// declared resolves, what is not, does not. Both at once, in the same
/// block, so that neither can pass without the other.
#[test]
fn declared_targets_resolve_and_external_ones_do_not() {
    let result = extract(&fixture_source());
    let targets = references_from(&result, &tf_id("module.compute"));
    assert_eq!(
        targets,
        vec![
            // `module "vpc"` is declared in THIS file (line 20).
            tf_id("module.vpc"),
            // The variables are read (lines 36-39) but declared in
            // variables.tf: real relations that are not declared here.
            "var.environment".to_string(),
            "var.instance_type".to_string(),
            "var.ssh_key_name".to_string(),
            "var.webserver_count".to_string(),
        ],
        "the traversals of `module \"compute\"` (lines 34-39) must resolve \
         `module.vpc` —declared in this same file— and leave `var.*` unresolved, \
         because those variables live in another file."
    );

    // And the nature of the edge says which of the two it is.
    let module_edge = result
        .edges
        .iter()
        .find(|e| {
            e.source == tf_id("module.compute")
                && matches!(&e.target_ref, TargetRef::Resolved(r) if r == &tf_id("module.vpc"))
        })
        .expect("module.vpc is declared here: its edge must come back resolved");
    assert!(matches!(&module_edge.target_ref, TargetRef::Resolved(_)));

    let var_edge = result
        .edges
        .iter()
        .find(|e| {
            e.source == tf_id("module.compute")
                && matches!(&e.target_ref, TargetRef::Unresolved(r) if r == "var.environment")
        })
        .expect("var.environment is not declared here: its edge comes back unresolved");
    assert!(matches!(&var_edge.target_ref, TargetRef::Unresolved(_)));
}

/// The edge exists even though its destination does not exist in the file:
/// the code reads that variable. What is not done is inventing a node for
/// it.
#[test]
fn a_variable_defined_elsewhere_is_reported_as_unresolved_not_invented() {
    let result = extract(&fixture_source());
    let edge = result
        .edges
        .iter()
        .find(|e| {
            e.source == tf_id("aws_db_instance.postgres")
                && e.kind == "dependency.references"
                && matches!(&e.target_ref, TargetRef::Unresolved(r) if r == "var.db_instance_class")
        })
        .expect(
            "`instance_class = var.db_instance_class` (line 90) must emit an \
             unresolved relation: `var \"db_instance_class\"` is not declared in \
             main.tf, it lives in variables.tf. A resolved edge here would be \
             inventing a node the file does not define.",
        );
    assert!(matches!(&edge.target_ref, TargetRef::Unresolved(_)));
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

fn ids_by_name(graph: &CallGraph) -> BTreeMap<String, SymbolId> {
    graph
        .symbol_ids()
        .map(|(id, s)| (s.name().to_string(), id.clone()))
        .collect()
}

/// The edge has to exist in the graph the user queries, not only in the
/// extraction result that produces it: what the domain layer got out of the
/// body has to arrive too.
#[test]
fn the_reference_reaches_the_public_graph() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("main.tf", &fixture_source())]);
    let ids = ids_by_name(&graph);

    let (Some(from), Some(to)) = (
        ids.get("aws_s3_bucket_versioning.app_bucket_versioning"),
        ids.get("aws_s3_bucket.app_bucket"),
    ) else {
        panic!(
            "both blocks of the real fixture have to be in the graph; got {:?}",
            ids.keys().collect::<Vec<_>>()
        );
    };

    let linked = graph
        .dependencies(from)
        .any(|(id, kind)| id == to && *kind == DependencyType::References);
    assert!(
        linked,
        "the relation between the two resources of the real fixture has to be \
         in the graph, not only in the extraction result."
    );
}

/// The opposite of the law above, and that is why the pair matters: if the
/// handler emitted edges by text similarity, or resolved by position, this
/// test would see it.
#[test]
fn the_graph_resolves_a_use_that_appears_before_its_declaration() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("main.tf", &fixture_source())]);
    let ids = ids_by_name(&graph);

    // `alarm_actions = [aws_sns_topic.alerts.arn]` is on line 250;
    // `resource "aws_sns_topic" "alerts"` is declared on line 258. That the
    // edge exists distinguishes a resolved identity from a positional
    // guess, which is the hardest shape to detect in retrospect.
    let (Some(from), Some(to)) = (
        ids.get("aws_cloudwatch_metric_alarm.app_cpu_high"),
        ids.get("aws_sns_topic.alerts"),
    ) else {
        panic!(
            "both blocks of the real fixture have to be in the graph; got {:?}",
            ids.keys().collect::<Vec<_>>()
        );
    };

    assert!(
        graph
            .dependencies(from)
            .any(|(id, kind)| id == to && *kind == DependencyType::References),
        "the resolution cannot depend on the order of the file: the use (250) \
         precedes the declaration (258) and the relation is still real."
    );
}

/// The graph does not inherit the noise of the generic path. Fragments of
/// literals once arrived as call targets; the domain layer must not
/// reintroduce them as relations.
#[test]
fn no_string_fragment_is_ever_a_reference() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let graph = graph_for(tmp.path(), &[("main.tf", &fixture_source())]);
    let ids = ids_by_name(&graph);

    for (name, id) in &ids {
        for (target, _) in graph.dependencies(id) {
            let Some(target_name) = graph.get_symbol(target) else {
                continue;
            };
            let target_name = target_name.name();
            assert!(
                !target_name.contains('"')
                    && !target_name.contains('{')
                    && !target_name.contains('}'),
                "`{name}` points to `{target_name}`, which is a fragment of source \
                 and not an identity. The destination of a relation is named with \
                 the identity of the block, never with a piece of a literal."
            );
        }
    }
}
