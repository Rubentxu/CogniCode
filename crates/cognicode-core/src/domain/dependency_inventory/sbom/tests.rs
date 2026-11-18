use super::*;
use crate::domain::dependency_inventory::units::{SourceFile, UnitInput};
use crate::domain::dependency_inventory::{
    CoverageStatus, DependencyLayer, WorkspaceInventory, inventory_unit,
};
use serde_json::{Value, json};

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
const SUM: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_SUM: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ARTIFACT: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn file(path: &str, text: &str) -> SourceFile {
    SourceFile {
        path: path.into(),
        bytes: text.as_bytes().to_vec(),
    }
}

fn registry_package(name: &str, version: &str, dependencies: &[&str]) -> String {
    let list: Vec<String> = dependencies.iter().map(|d| format!("\"{d}\"")).collect();
    format!(
        "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\nsource = \"{REGISTRY}\"\nchecksum = \"{SUM}\"\ndependencies = [{}]\n",
        list.join(", ")
    )
}

fn app_package(dependencies: &[&str]) -> String {
    let list: Vec<String> = dependencies.iter().map(|d| format!("\"{d}\"")).collect();
    format!(
        "\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [{}]\n",
        list.join(", ")
    )
}

fn inventory(manifest_tables: &str, packages: &[String]) -> WorkspaceInventory {
    inventory_unit(&UnitInput {
        root: String::new(),
        lock: Some(file(
            "Cargo.lock",
            &format!("version = 3\n{}", packages.concat()),
        )),
        manifests: vec![file(
            "Cargo.toml",
            &format!("[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n{manifest_tables}"),
        )],
    })
}

/// app -> serde (normal) -> serde_derive; itoa (dev); cc (build).
fn standard_unit() -> WorkspaceInventory {
    inventory(
        "[dependencies]\nserde = \"1\"\n[dev-dependencies]\nitoa = \"1\"\n[build-dependencies]\ncc = \"1\"\n",
        &[
            app_package(&["cc", "itoa", "serde"]),
            registry_package("cc", "1.0.0", &[]),
            registry_package("itoa", "1.0.0", &[]),
            registry_package("serde", "1.0.5", &["serde_derive"]),
            registry_package("serde_derive", "1.0.5", &[]),
        ],
    )
}

fn component(name: &str, version: &str) -> Value {
    json!({
        "type": "library",
        "bom-ref": format!("{name}@{version}"),
        "name": name,
        "version": version,
        "purl": format!("pkg:cargo/{name}@{version}"),
        "scope": "required",
        "hashes": [{"alg": "SHA-256", "content": SUM}],
    })
}

fn document(components: Vec<Value>) -> Value {
    json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": "urn:uuid:3e671687-395b-41f5-a30f-a58921a69b79",
        "version": 1,
        "metadata": {
            "timestamp": "2024-01-01T00:00:00Z",
            "lifecycles": [{"phase": "build"}],
            "tools": {"components": [{"type": "application", "name": "exporter", "version": "1.0"}]},
            "component": {
                "type": "application",
                "bom-ref": "app@0.1.0",
                "name": "app",
                "version": "0.1.0",
                "purl": "pkg:cargo/app@0.1.0",
            },
        },
        "components": components,
    })
}

fn bytes(document: &Value) -> Vec<u8> {
    serde_json::to_vec(document).expect("serialisable")
}

fn parse(document: &Value) -> ParsedSbom {
    parse_cyclonedx(&bytes(document), &SbomLimits::default()).expect("readable")
}

fn compare(unit: &WorkspaceInventory, document: &Value) -> DistributionReport {
    compare_distribution(unit, &parse(document), &ComparisonRequest::default())
}

fn kinds(report: &DistributionReport) -> Vec<DistributionFindingKind> {
    report.findings.iter().map(|finding| finding.kind).collect()
}

fn row<'a>(report: &'a DistributionReport, id: &str) -> &'a PackageDistribution {
    report
        .packages
        .iter()
        .find(|row| row.package == id)
        .unwrap_or_else(|| panic!("no row {id}"))
}

fn agreeing_document() -> Value {
    document(vec![
        component("serde", "1.0.5"),
        component("serde_derive", "1.0.5"),
    ])
}

// ---- reading ----

#[test]
fn a_document_is_read_with_its_claims_and_components() {
    let parsed = parse(&agreeing_document());

    assert_eq!(parsed.spec_version, "1.5");
    assert_eq!(parsed.stage, SbomStage::Artifact);
    assert_eq!(parsed.tools, vec!["exporter 1.0".to_string()]);
    assert_eq!(parsed.components.len(), 2);
    assert_eq!(parsed.components[0].sha256, vec![SUM.to_string()]);
    assert_eq!(
        parsed.subject.as_ref().map(|s| s.name.as_str()),
        Some("app")
    );
    assert_eq!(parsed.diagnostics_total, 0);
    assert!(!parsed.signature_present);
}

#[test]
fn nested_components_are_flattened_with_their_depth() {
    let mut outer = component("serde", "1.0.5");
    outer["components"] = json!([component("serde_derive", "1.0.5")]);
    let parsed = parse(&document(vec![outer]));

    let depths: Vec<usize> = parsed.components.iter().map(|c| c.depth).collect();
    assert_eq!(depths, vec![0, 1]);
}

#[test]
fn a_signature_is_noted_and_never_verified() {
    let mut signed = agreeing_document();
    signed["signature"] = json!({"algorithm": "RS256", "value": "AAAA"});
    signed["formulation"] = json!([]);
    let parsed = parse(&signed);

    assert!(parsed.signature_present);
    assert_eq!(parsed.ignored_sections, vec!["formulation".to_string()]);
}

#[test]
fn lifecycle_phases_decide_the_stage() {
    let stage = |phases: Value| {
        let mut doc = agreeing_document();
        doc["metadata"]["lifecycles"] = phases;
        parse(&doc).stage
    };

    assert_eq!(stage(json!([{"phase": "pre-build"}])), SbomStage::Source);
    assert_eq!(stage(json!([{"phase": "post-build"}])), SbomStage::Artifact);
    assert_eq!(
        stage(json!([{"phase": "design"}, {"phase": "operations"}])),
        SbomStage::Ambiguous
    );
    assert_eq!(stage(json!([])), SbomStage::Unstated);
}

fn refusal(raw: &[u8], limits: &SbomLimits) -> SbomError {
    parse_cyclonedx(raw, limits).expect_err("refused")
}

#[test]
fn a_document_that_is_not_readable_is_refused_with_the_reason() {
    let limits = SbomLimits::default();

    assert!(matches!(
        refusal(b"{", &limits),
        SbomError::Malformed { .. }
    ));
    assert_eq!(refusal(b"[]", &limits), SbomError::NotAnSbom);
    assert_eq!(refusal(b"{}", &limits), SbomError::NotAnSbom);
    assert_eq!(
        refusal(br#"{"spdxVersion":"SPDX-2.3"}"#, &limits),
        SbomError::UnsupportedFormat {
            found: "spdx".to_string()
        }
    );
    assert_eq!(
        refusal(br#"{"bomFormat":"CycloneDX","specVersion":"1.1"}"#, &limits),
        SbomError::UnsupportedSpecVersion {
            found: "1.1".to_string()
        }
    );
}

#[test]
fn size_component_and_depth_limits_are_enforced() {
    let doc = bytes(&agreeing_document());
    let small = SbomLimits {
        max_bytes: 10,
        ..SbomLimits::default()
    };
    assert!(matches!(refusal(&doc, &small), SbomError::TooLarge { .. }));

    let few = SbomLimits {
        max_components: 1,
        ..SbomLimits::default()
    };
    assert_eq!(
        refusal(&doc, &few),
        SbomError::TooManyComponents { limit: 1 }
    );

    let mut deep = component("a", "1.0.0");
    for _ in 0..20 {
        let mut parent = component("a", "1.0.0");
        parent["components"] = json!([deep]);
        deep = parent;
    }
    assert_eq!(
        refusal(&bytes(&document(vec![deep])), &SbomLimits::default()),
        SbomError::TooDeep { limit: 16 }
    );
}

#[test]
fn too_many_dependency_edges_are_refused() {
    let mut doc = agreeing_document();
    doc["dependencies"] =
        json!([{"ref": "app@0.1.0", "dependsOn": ["serde@1.0.5", "serde_derive@1.0.5"]}]);
    let limits = SbomLimits {
        max_edges: 1,
        ..SbomLimits::default()
    };

    assert_eq!(
        refusal(&bytes(&doc), &limits),
        SbomError::TooManyEdges { limit: 1 }
    );
}

fn diagnostic_kinds(doc: &Value) -> Vec<SbomDiagnosticKind> {
    parse(doc).diagnostics.iter().map(|d| d.kind).collect()
}

#[test]
fn structural_problems_are_reported_not_hidden() {
    let mut dup = document(vec![
        component("serde", "1.0.5"),
        component("serde", "1.0.5"),
    ]);
    dup["dependencies"] = json!([{"ref": "serde@1.0.5", "dependsOn": ["ghost@1.0.0"]}]);
    let kinds = diagnostic_kinds(&dup);
    assert!(kinds.contains(&SbomDiagnosticKind::DuplicateBomRef));
    assert!(kinds.contains(&SbomDiagnosticKind::DanglingDependencyRef));

    let mut bad = component("serde", "1.0.5");
    bad["purl"] = json!("not a purl");
    bad["hashes"] = json!([{"alg": "SHA-256", "content": "zz"}]);
    bad["scope"] = json!("sometimes");
    let kinds = diagnostic_kinds(&document(vec![bad]));
    assert!(kinds.contains(&SbomDiagnosticKind::InvalidPurl));
    assert!(kinds.contains(&SbomDiagnosticKind::InvalidHash));
    assert!(kinds.contains(&SbomDiagnosticKind::UnknownScope));

    let mut headless = agreeing_document();
    headless["metadata"]
        .as_object_mut()
        .expect("object")
        .remove("component");
    assert!(diagnostic_kinds(&headless).contains(&SbomDiagnosticKind::MissingSubject));

    let mut nameless = document(vec![json!({"type": "library", "version": "1"})]);
    nameless["components"][0]["bom-ref"] = json!("x");
    let parsed = parse(&nameless);
    assert_eq!(parsed.rejected_components, 1);
    assert!(parsed.components.is_empty());
}

#[test]
fn diagnostics_are_capped_but_counted() {
    let many: Vec<Value> = (0..10)
        .map(|index| json!({"type": "library", "name": format!("c{index}")}))
        .collect();
    let limits = SbomLimits {
        max_diagnostics: 3,
        ..SbomLimits::default()
    };
    let parsed = parse_cyclonedx(&bytes(&document(many)), &limits).expect("readable");

    assert_eq!(parsed.diagnostics.len(), 3);
    assert_eq!(parsed.diagnostics_total, 10);
}

// ---- comparing ----

#[test]
fn an_sbom_that_agrees_with_the_lockfile_has_no_finding() {
    let report = compare(&standard_unit(), &agreeing_document());

    assert_eq!(kinds(&report), vec![]);
    assert_eq!(report.counts.packages, 4);
    assert_eq!(report.counts.listed, 2);
    assert_eq!(report.counts.origin_agrees, 2);
    assert_eq!(report.counts.digest_agrees, 2);
    assert_eq!(
        report.subject,
        SubjectResolution::Member {
            name: "app".to_string(),
            version: Some("0.1.0".to_string())
        }
    );
    assert_eq!(
        row(&report, "serde@1.0.5").expectation,
        Expectation::Shipping
    );
    assert_eq!(
        row(&report, "itoa@1.0.0").expectation,
        Expectation::NonShipping
    );
    assert_eq!(row(&report, "itoa@1.0.0").presence, Presence::NotListed);
}

#[test]
fn absence_from_the_sbom_of_a_dev_or_build_package_is_not_a_discrepancy() {
    let report = compare(&standard_unit(), &agreeing_document());

    assert_eq!(
        row(&report, "cc@1.0.0").expectation,
        Expectation::NonShipping
    );
    assert!(report.findings.iter().all(|f| f.subject != "cc@1.0.0"));
}

#[test]
fn a_dev_only_package_listed_in_the_sbom_is_a_discrepancy() {
    let mut doc = agreeing_document();
    doc["components"]
        .as_array_mut()
        .expect("array")
        .push(component("itoa", "1.0.0"));
    let report = compare(&standard_unit(), &doc);

    assert_eq!(
        kinds(&report),
        vec![DistributionFindingKind::NonShippingListed]
    );
    assert_eq!(report.findings[0].subject, "itoa@1.0.0");
}

#[test]
fn a_dev_only_package_the_sbom_marks_excluded_is_consistent() {
    let mut itoa = component("itoa", "1.0.0");
    itoa["scope"] = json!("excluded");
    let mut doc = agreeing_document();
    doc["components"].as_array_mut().expect("array").push(itoa);

    assert_eq!(kinds(&compare(&standard_unit(), &doc)), vec![]);
}

#[test]
fn a_shipping_package_marked_excluded_is_a_scope_conflict() {
    let mut serde = component("serde", "1.0.5");
    serde["scope"] = json!("excluded");
    let doc = document(vec![serde, component("serde_derive", "1.0.5")]);

    assert_eq!(
        kinds(&compare(&standard_unit(), &doc)),
        vec![DistributionFindingKind::ScopeConflict]
    );
}

#[test]
fn a_direct_dependency_missing_is_required_and_a_transitive_one_is_an_upper_bound() {
    let report = compare(&standard_unit(), &document(vec![]));

    assert_eq!(
        kinds(&report),
        vec![
            DistributionFindingKind::RequiredNotListed,
            DistributionFindingKind::ReachableNotListed
        ]
    );
    assert_eq!(report.counts.not_listed, 4);
}

#[test]
fn an_unreachable_package_listed_is_reported() {
    let unit = inventory(
        "[dependencies]\nserde = \"1\"\n",
        &[
            app_package(&["serde"]),
            registry_package("serde", "1.0.5", &[]),
            registry_package("stray", "1.0.0", &[]),
        ],
    );
    let doc = document(vec![
        component("serde", "1.0.5"),
        component("stray", "1.0.0"),
    ]);
    let report = compare(&unit, &doc);

    assert_eq!(
        kinds(&report),
        vec![DistributionFindingKind::ListedNotReachable]
    );
}

#[test]
fn a_different_origin_or_hash_is_a_mismatch() {
    let mut serde = component("serde", "1.0.5");
    serde["purl"] = json!("pkg:cargo/serde@1.0.5?repository_url=https://other.example/index");
    let mut derive = component("serde_derive", "1.0.5");
    derive["hashes"] = json!([{"alg": "SHA-256", "content": OTHER_SUM}]);
    let report = compare(&standard_unit(), &document(vec![serde, derive]));

    assert_eq!(
        kinds(&report),
        vec![
            DistributionFindingKind::OriginMismatch,
            DistributionFindingKind::DigestMismatch
        ]
    );
    assert_eq!(report.counts.origin_differs, 1);
    assert_eq!(report.counts.digest_differs, 1);
}

#[test]
fn a_missing_hash_or_source_is_unresolved_not_a_finding() {
    let mut serde = component("serde", "1.0.5");
    serde.as_object_mut().expect("object").remove("hashes");
    serde.as_object_mut().expect("object").remove("purl");
    let report = compare(
        &standard_unit(),
        &document(vec![serde, component("serde_derive", "1.0.5")]),
    );

    assert_eq!(kinds(&report), vec![]);
    let listed = row(&report, "serde@1.0.5");
    assert_eq!(listed.presence, Presence::Listed);
    assert_eq!(listed.origin.state, CheckState::Unresolved);
    assert_eq!(listed.digest.state, CheckState::Unresolved);
    assert_eq!(report.counts.origin_unresolved, 1);
    assert_eq!(report.counts.digest_unresolved, 1);
}

#[test]
fn two_different_components_for_one_package_are_ambiguous() {
    let mut twin = component("serde", "1.0.5");
    twin["bom-ref"] = json!("serde-twin");
    twin["hashes"] = json!([{"alg": "SHA-256", "content": OTHER_SUM}]);
    let doc = document(vec![
        component("serde", "1.0.5"),
        twin,
        component("serde_derive", "1.0.5"),
    ]);
    let report = compare(&standard_unit(), &doc);

    assert_eq!(
        kinds(&report),
        vec![DistributionFindingKind::AmbiguousMatch]
    );
    assert_eq!(row(&report, "serde@1.0.5").presence, Presence::Ambiguous);
    assert_eq!(report.counts.ambiguous, 1);
}

#[test]
fn an_origin_that_agrees_narrows_two_components_to_one() {
    let mut elsewhere = component("serde", "1.0.5");
    elsewhere["bom-ref"] = json!("serde-elsewhere");
    elsewhere["purl"] = json!("pkg:cargo/serde@1.0.5?repository_url=https://other.example/index");
    let doc = document(vec![
        component("serde", "1.0.5"),
        elsewhere,
        component("serde_derive", "1.0.5"),
    ]);
    let report = compare(&standard_unit(), &doc);

    assert_eq!(row(&report, "serde@1.0.5").presence, Presence::Listed);
}

#[test]
fn identical_repeated_components_are_merged_and_counted() {
    let doc = document(vec![
        component("serde", "1.0.5"),
        component("serde", "1.0.5"),
        component("serde_derive", "1.0.5"),
    ]);
    let report = compare(&standard_unit(), &doc);

    assert_eq!(kinds(&report), vec![]);
    assert_eq!(report.counts.duplicates_merged, 1);
}

#[test]
fn one_version_of_two_locked_is_listed_and_the_other_is_not() {
    let unit = inventory(
        "[dependencies]\nbitflags = \"2\"\nserde = \"1\"\n",
        &[
            app_package(&["bitflags 2.0.0", "serde"]),
            registry_package("bitflags", "1.3.2", &[]),
            registry_package("bitflags", "2.0.0", &[]),
            registry_package("serde", "1.0.5", &["bitflags 1.3.2"]),
        ],
    );
    let doc = document(vec![
        component("bitflags", "2.0.0"),
        component("serde", "1.0.5"),
        component("bitflags", "9.9.9"),
    ]);
    let report = compare(&unit, &doc);

    assert_eq!(row(&report, "bitflags@2.0.0").presence, Presence::Listed);
    assert_eq!(row(&report, "bitflags@1.3.2").presence, Presence::NotListed);
    assert_eq!(
        kinds(&report),
        vec![
            DistributionFindingKind::ReachableNotListed,
            DistributionFindingKind::ListedNotResolved
        ]
    );
    let stray = report.unmatched.first().expect("unmatched");
    assert!(stray.reason.contains("1.3.2, 2.0.0"), "{}", stray.reason);
}

#[test]
fn components_that_are_not_cargo_packages_are_counted_not_matched() {
    let foreign = json!({
        "type": "library", "name": "libc6", "version": "2.36",
        "purl": "pkg:deb/debian/libc6@2.36",
    });
    let file = json!({"type": "file", "name": "/etc/passwd", "version": "1"});
    let mut doc = agreeing_document();
    doc["components"]
        .as_array_mut()
        .expect("array")
        .extend([foreign, file]);
    let report = compare(&standard_unit(), &doc);

    assert_eq!(kinds(&report), vec![]);
    assert_eq!(report.counts.not_comparable_components, 2);
    assert_eq!(report.counts.unmatched_components, 0);
}

#[test]
fn a_source_stage_sbom_is_not_a_distribution() {
    let mut doc = agreeing_document();
    doc["metadata"]["lifecycles"] = json!([{"phase": "pre-build"}]);
    let report = compare(&standard_unit(), &doc);

    assert!(kinds(&report).contains(&DistributionFindingKind::SourceStage));
    assert_eq!(report.layer().status, CoverageStatus::NotCollected);
}

#[test]
fn an_artifact_sbom_fills_the_distributed_layer_only_partially() {
    let layer = compare(&standard_unit(), &agreeing_document()).layer();

    assert_eq!(layer.layer, DependencyLayer::Distributed);
    assert_eq!(layer.status, CoverageStatus::Partial);
    assert!(layer.note.contains("claim"));
}

#[test]
fn a_subject_that_is_not_a_member_leaves_shipping_unassessed() {
    let mut doc = agreeing_document();
    doc["metadata"]["component"]["name"] = json!("other");
    doc["metadata"]["component"]["purl"] = json!("pkg:cargo/other@0.1.0");
    let unit = standard_unit();
    let report = compare(&unit, &doc);

    assert!(matches!(
        report.subject,
        SubjectResolution::Unresolved { .. }
    ));
    assert!(kinds(&report).contains(&DistributionFindingKind::SubjectUnresolved));
    assert_eq!(
        row(&report, "serde@1.0.5").expectation,
        Expectation::Unassessed
    );

    let request = ComparisonRequest {
        member: Some("app"),
        artifact_digest: None,
    };
    let named = compare_distribution(&unit, &parse(&doc), &request);
    assert!(matches!(named.subject, SubjectResolution::Member { .. }));
}

#[test]
fn a_subject_version_that_differs_from_the_member_is_reported() {
    let mut doc = agreeing_document();
    doc["metadata"]["component"]["version"] = json!("0.2.0");

    assert_eq!(
        kinds(&compare(&standard_unit(), &doc)),
        vec![DistributionFindingKind::SubjectVersionDiffers]
    );
}

#[test]
fn the_sbom_is_bound_to_an_artifact_digest_only_through_its_subject_hash() {
    let unit = standard_unit();
    let mut doc = agreeing_document();
    let state = |doc: &Value, digest: Option<&str>| {
        let request = ComparisonRequest {
            member: None,
            artifact_digest: digest,
        };
        compare_distribution(&unit, &parse(doc), &request)
    };

    assert_eq!(
        state(&doc, None).artifact_binding.state,
        BindingState::NotRequested
    );
    assert_eq!(
        state(&doc, Some(ARTIFACT)).artifact_binding.state,
        BindingState::NotStatedBySbom
    );

    doc["metadata"]["component"]["hashes"] = json!([{"alg": "SHA-256", "content": ARTIFACT}]);
    assert_eq!(
        state(&doc, Some(ARTIFACT)).artifact_binding.state,
        BindingState::Agrees
    );
    let other = state(&doc, Some(SUM));
    assert_eq!(other.artifact_binding.state, BindingState::Differs);
    assert!(kinds(&other).contains(&DistributionFindingKind::ArtifactDigestMismatch));
}

#[test]
fn a_git_origin_agrees_by_repository_and_commit_prefix() {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let unit = inventory(
        "[dependencies]\nfork = { git = \"https://g.example/fork\" }\n",
        &[
            app_package(&["fork"]),
            format!(
                "\n[[package]]\nname = \"fork\"\nversion = \"0.1.0\"\nsource = \"git+https://g.example/fork#{commit}\"\n"
            ),
        ],
    );
    let listed = |purl: &str| {
        let mut fork = component("fork", "0.1.0");
        fork["purl"] = json!(purl);
        fork.as_object_mut().expect("object").remove("hashes");
        compare(&unit, &document(vec![fork]))
    };

    let agreeing = listed(&format!(
        "pkg:cargo/fork@0.1.0?vcs_url=git%2Bhttps://g.example/fork.git%40{}",
        &commit[..12]
    ));
    assert_eq!(
        row(&agreeing, "fork@0.1.0").origin.state,
        CheckState::Agrees
    );

    let moved =
        listed("pkg:cargo/fork@0.1.0?vcs_url=git%2Bhttps://g.example/fork%40deadbeefdeadbeef");
    assert_eq!(kinds(&moved), vec![DistributionFindingKind::OriginMismatch]);

    let bare = listed("pkg:cargo/fork@0.1.0");
    assert_eq!(
        row(&bare, "fork@0.1.0").origin.state,
        CheckState::Unresolved
    );
}
