//! Three real SBOMs read against the Cargo fixture workspace.
//!
//! `cargo-cyclonedx` and the CogniCode Guard release generator export a
//! source-tree SBOM of the fixture; the external one is written the way an
//! image scanner writes an artifact SBOM. The exports are produced by
//! `sandbox/fixtures/dependency-inventory/sbom_fixtures.py`, outside the test
//! suite. None of the documents is signed: nothing here claims a document is
//! authentic, only what it says and how that compares with the lockfile.

use cognicode_core::application::artifact_sbom::{
    DistributionAnswer, DistributionEvaluation, DistributionQuery, SbomStatus, SbomSupply,
    distribute,
};
use cognicode_core::domain::dependency_inventory::sbom::{
    BindingState, DistributionReport, FindingKind, SbomLimits, SbomStage, SubjectResolution,
};
use cognicode_core::domain::dependency_inventory::{
    CoverageStatus, DependencyLayer, SourceFile, UnitInput, WorkspaceInventory, inventory_unit,
};
use cognicode_core::domain::scm::ContentDigest;
use std::path::Path;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../sandbox/fixtures/dependency-inventory"
);

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn inventory() -> WorkspaceInventory {
    let root = Path::new(FIXTURE).join("cargo-workspace");
    let file = |relative: &str| SourceFile {
        path: relative.to_string(),
        bytes: read(&root.join(relative)),
    };
    inventory_unit(&UnitInput {
        root: String::new(),
        lock: Some(file("Cargo.lock")),
        manifests: vec![
            file("Cargo.toml"),
            file("app/Cargo.toml"),
            file("util/Cargo.toml"),
        ],
    })
}

fn sbom(name: &str) -> Vec<u8> {
    read(&Path::new(FIXTURE).join(name))
}

fn query(name: &str, member: Option<&str>) -> DistributionQuery {
    DistributionQuery {
        sbom: SbomSupply::Bytes {
            origin: name.to_string(),
            bytes: sbom(name),
        },
        expected_digest: None,
        artifact_digest: None,
        member: member.map(str::to_string),
    }
}

fn run(asked: &DistributionQuery) -> (DistributionAnswer, WorkspaceInventory) {
    let mut units = vec![inventory()];
    let answer = distribute(&mut units, asked, &SbomLimits::default());
    (answer, units.remove(0))
}

fn report(answer: &DistributionAnswer) -> &DistributionReport {
    assert_eq!(answer.status, SbomStatus::Usable, "{:?}", answer.problem);
    let unit = &answer.units[0];
    assert_eq!(unit.evaluation, DistributionEvaluation::Evaluated);
    unit.report.as_ref().expect("a report")
}

fn kinds(report: &DistributionReport) -> Vec<FindingKind> {
    let mut kinds: Vec<_> = report.findings.iter().map(|f| f.kind).collect();
    kinds.sort_by_key(|kind| format!("{kind:?}"));
    kinds
}

const IMAGE_DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";

#[test]
fn a_cargo_cyclonedx_export_agrees_with_the_lockfile_and_states_no_stage() {
    let (answer, unit) = run(&query("cargo-workspace.cargo-cyclonedx.cdx.json", None));
    let report = report(&answer);

    assert_eq!(report.claim.stage, SbomStage::Unstated);
    assert!(matches!(report.subject, SubjectResolution::Member { .. }));
    assert!(kinds(report).is_empty(), "{:?}", report.findings);
    assert_eq!(report.counts.listed, 8);
    assert_eq!(report.counts.not_listed, 1, "the dev-only package");
    assert_eq!(
        report.counts.origin_differs + report.counts.digest_differs,
        0
    );
    assert_eq!(
        report.counts.digest_unresolved, 1,
        "the path package carries no checksum on either side"
    );
    assert_eq!(report.claim.diagnostics_total, 0);

    let layer = unit
        .layers
        .iter()
        .find(|slot| slot.layer == DependencyLayer::Distributed)
        .expect("a distributed layer");
    assert_eq!(layer.status, CoverageStatus::Partial);
    assert!(layer.note.contains("claim"), "{}", layer.note);
}

#[test]
fn the_guard_export_lists_the_dev_and_build_packages_it_cannot_tell_apart() {
    let (answer, _) = run(&query("cargo-workspace.guard.cdx.json", None));
    let report = report(&answer);

    assert_eq!(report.claim.components.with_sha256, 0);
    assert_eq!(
        report.counts.digest_unresolved, 9,
        "no hash is ever guessed"
    );
    assert_eq!(
        report.counts.digest_agrees + report.counts.digest_differs,
        0
    );
    let flagged: Vec<&str> = report
        .findings
        .iter()
        .filter(|finding| finding.kind == FindingKind::NonShippingListed)
        .map(|finding| finding.subject.as_str())
        .collect();
    assert_eq!(flagged.len(), 2, "{:?}", report.findings);
    assert!(flagged.contains(&"autocfg@1.5.1") && flagged.contains(&"itoa@1.0.19"));
    assert_eq!(kinds(report).len(), 2);
}

#[test]
fn an_external_artifact_sbom_is_bound_by_its_subject_and_reports_each_disagreement() {
    let mut asked = query("external-image.cdx.json", Some("app"));
    asked.artifact_digest = Some(ContentDigest::from_hex(IMAGE_DIGEST).expect("hex"));
    let (answer, _) = run(&asked);
    let report = report(&answer);

    assert_eq!(report.claim.stage, SbomStage::Artifact);
    assert!(matches!(report.subject, SubjectResolution::Member { .. }));
    assert_eq!(report.artifact_binding.state, BindingState::Agrees);
    assert!(
        report.claim.diagnostics_total >= 1,
        "the dangling reference"
    );

    let found = kinds(report);
    for expected in [
        FindingKind::DigestMismatch,
        FindingKind::ListedNotResolved,
        FindingKind::NonShippingListed,
    ] {
        assert!(found.contains(&expected), "{expected:?} in {found:?}");
    }
    assert_eq!(
        report.counts.not_comparable_components, 2,
        "an OS and a deb"
    );
}

#[test]
fn without_naming_the_member_an_external_subject_is_unresolved_never_guessed() {
    let (answer, _) = run(&query("external-image.cdx.json", None));
    let report = report(&answer);

    assert!(matches!(
        report.subject,
        SubjectResolution::Unresolved { .. }
    ));
    assert!(kinds(report).contains(&FindingKind::SubjectUnresolved));
    assert!(!kinds(report).contains(&FindingKind::RequiredNotListed));
}

#[test]
fn another_artifact_digest_is_a_mismatch_and_a_pinned_digest_is_checked_on_the_bytes() {
    let mut asked = query("external-image.cdx.json", Some("app"));
    asked.artifact_digest = Some(ContentDigest::from_hex(&"2".repeat(64)).expect("hex"));
    let (answer, _) = run(&asked);
    assert_eq!(
        report(&answer).artifact_binding.state,
        BindingState::Differs
    );
    assert!(kinds(report(&answer)).contains(&FindingKind::ArtifactDigestMismatch));

    let mut pinned = query("external-image.cdx.json", Some("app"));
    pinned.expected_digest = Some(ContentDigest::from_bytes(&sbom("external-image.cdx.json")));
    let (answer, _) = run(&pinned);
    assert!(answer.document.as_ref().expect("document").pinned);

    let mut wrong = query("external-image.cdx.json", Some("app"));
    wrong.expected_digest = Some(ContentDigest::from_bytes(b"other bytes"));
    let (answer, unit) = run(&wrong);
    assert_eq!(answer.status, SbomStatus::DigestMismatch);
    let layer = unit
        .layers
        .iter()
        .find(|slot| slot.layer == DependencyLayer::Distributed)
        .expect("layer");
    assert_eq!(layer.status, CoverageStatus::NotCollected);
}

#[test]
fn an_answer_round_trips_through_its_wire_form_for_every_fixture() {
    for name in [
        "cargo-workspace.cargo-cyclonedx.cdx.json",
        "cargo-workspace.guard.cdx.json",
        "external-image.cdx.json",
    ] {
        let (answer, _) = run(&query(name, Some("app")));
        let wire = serde_json::to_string(&answer).expect("serialises");
        let back: DistributionAnswer = serde_json::from_str(&wire).expect("deserialises");
        assert_eq!(back, answer, "{name}");
    }
}
