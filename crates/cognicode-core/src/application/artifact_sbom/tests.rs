use super::*;
use crate::domain::dependency_inventory::sbom::FindingKind;
use crate::domain::dependency_inventory::{CoverageStatus, SourceFile, UnitInput, inventory_unit};
use serde_json::{Value, json};

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
const SUM: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn unit(root: &str) -> WorkspaceInventory {
    let lock = format!(
        "version = 3\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"serde\"]\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.5\"\nsource = \"{REGISTRY}\"\nchecksum = \"{SUM}\"\n"
    );
    inventory_unit(&UnitInput {
        root: root.to_string(),
        lock: Some(SourceFile {
            path: format!("{root}/Cargo.lock"),
            bytes: lock.into_bytes(),
        }),
        manifests: vec![SourceFile {
            path: format!("{root}/Cargo.toml"),
            bytes:
                b"[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n"
                    .to_vec(),
        }],
    })
}

fn document() -> Value {
    json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "lifecycles": [{"phase": "build"}],
            "component": {
                "type": "application",
                "bom-ref": "app",
                "name": "app",
                "version": "0.1.0",
            },
        },
        "components": [{
            "type": "library",
            "bom-ref": "serde",
            "name": "serde",
            "version": "1.0.5",
            "purl": "pkg:cargo/serde@1.0.5",
            "scope": "required",
            "hashes": [{"alg": "SHA-256", "content": SUM}],
        }],
    })
}

fn bytes(document: &Value) -> Vec<u8> {
    serde_json::to_vec(document).expect("serialisable")
}

fn supplied(document: &Value) -> SbomSupply {
    SbomSupply::Bytes {
        origin: "app.cdx.json".to_string(),
        bytes: bytes(document),
    }
}

fn query(sbom: SbomSupply) -> DistributionQuery {
    DistributionQuery {
        sbom,
        expected_digest: None,
        artifact_digest: None,
        member: None,
    }
}

fn distributed_layer(units: &[WorkspaceInventory]) -> CoverageStatus {
    units[0]
        .layers
        .iter()
        .find(|slot| slot.layer == DependencyLayer::Distributed)
        .expect("a distributed layer")
        .status
}

#[test]
fn a_document_that_agrees_is_compared_and_fills_the_layer_as_a_claim() {
    let mut units = vec![unit("")];
    assert_eq!(distributed_layer(&units), CoverageStatus::NotCollected);

    let answer = distribute(
        &mut units,
        &query(supplied(&document())),
        &SbomLimits::default(),
    );

    assert_eq!(answer.status, SbomStatus::Usable);
    assert_eq!(answer.problem, None);
    let anchor = answer.document.expect("anchored to the bytes");
    assert_eq!(anchor.origin, "app.cdx.json");
    assert_eq!(
        anchor.digest,
        ContentDigest::from_bytes(&bytes(&document())).as_hex()
    );
    assert!(!anchor.pinned);
    let evaluated = &answer.units[0];
    assert_eq!(evaluated.evaluation, DistributionEvaluation::Evaluated);
    let report = evaluated.report.as_ref().expect("a report");
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert_eq!(distributed_layer(&units), CoverageStatus::Partial);
}

#[test]
fn a_pinned_digest_that_matches_is_recorded() {
    let raw = bytes(&document());
    let mut units = vec![unit("")];
    let mut asked = query(supplied(&document()));
    asked.expected_digest = Some(ContentDigest::from_bytes(&raw));

    let answer = distribute(&mut units, &asked, &SbomLimits::default());

    assert_eq!(answer.status, SbomStatus::Usable);
    assert!(answer.document.expect("document").pinned);
}

#[test]
fn bytes_that_differ_from_the_pinned_digest_are_not_used() {
    let mut units = vec![unit("")];
    let mut asked = query(supplied(&document()));
    asked.expected_digest = Some(ContentDigest::from_bytes(b"something else"));

    let answer = distribute(&mut units, &asked, &SbomLimits::default());

    assert_eq!(answer.status, SbomStatus::DigestMismatch);
    assert!(answer.problem.expect("a problem").contains("pinned digest"));
    assert!(!answer.document.expect("anchored").pinned);
    assert_eq!(
        answer.units[0].evaluation,
        DistributionEvaluation::NotEvaluated
    );
    assert_eq!(distributed_layer(&units), CoverageStatus::NotCollected);
}

#[test]
fn a_document_that_could_not_be_read_leaves_the_layer_uncollected() {
    let mut units = vec![unit("")];
    let asked = query(SbomSupply::Unreadable {
        reason: "no such file".to_string(),
    });

    let answer = distribute(&mut units, &asked, &SbomLimits::default());

    assert_eq!(answer.status, SbomStatus::Unreadable);
    assert_eq!(answer.problem.as_deref(), Some("no such file"));
    assert!(answer.document.is_none());
    assert_eq!(distributed_layer(&units), CoverageStatus::NotCollected);
}

#[test]
fn a_format_that_is_not_read_is_refused_with_its_name_and_anchored_to_its_bytes() {
    let spdx = json!({"spdxVersion": "SPDX-2.3", "SPDXID": "SPDXRef-DOCUMENT"});
    let mut units = vec![unit("")];

    let answer = distribute(&mut units, &query(supplied(&spdx)), &SbomLimits::default());

    assert_eq!(answer.status, SbomStatus::Refused);
    assert!(answer.problem.expect("a problem").contains("spdx"));
    assert!(answer.document.is_some());
    assert_eq!(distributed_layer(&units), CoverageStatus::NotCollected);
}

#[test]
fn an_oversized_document_is_refused_by_the_limits() {
    let mut units = vec![unit("")];
    let limits = SbomLimits {
        max_bytes: 10,
        ..SbomLimits::default()
    };

    let answer = distribute(&mut units, &query(supplied(&document())), &limits);

    assert_eq!(answer.status, SbomStatus::Refused);
}

#[test]
fn several_units_in_scope_are_not_compared_with_one_artifact() {
    let mut units = vec![unit("a"), unit("b")];

    let answer = distribute(
        &mut units,
        &query(supplied(&document())),
        &SbomLimits::default(),
    );

    assert_eq!(answer.status, SbomStatus::Usable);
    assert_eq!(answer.units.len(), 2);
    for entry in &answer.units {
        assert_eq!(entry.evaluation, DistributionEvaluation::NotEvaluated);
        assert!(entry.report.is_none());
        assert!(entry.reason.as_deref().expect("a reason").contains("path"));
    }
    assert!(units.iter().all(|unit| {
        unit.layers.iter().any(|slot| {
            slot.layer == DependencyLayer::Distributed
                && slot.status == CoverageStatus::NotCollected
        })
    }));
}

#[test]
fn a_component_the_lockfile_does_not_resolve_is_reported_never_dropped() {
    let mut doc = document();
    doc["components"]
        .as_array_mut()
        .expect("components")
        .push(json!({
            "type": "library",
            "bom-ref": "ghost",
            "name": "ghost",
            "version": "9.9.9",
            "purl": "pkg:cargo/ghost@9.9.9",
        }));
    let mut units = vec![unit("")];

    let answer = distribute(&mut units, &query(supplied(&doc)), &SbomLimits::default());

    let report = answer.units[0].report.as_ref().expect("a report");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.kind == FindingKind::ListedNotResolved)
    );
}

#[test]
fn the_answer_round_trips_through_its_wire_form() {
    let mut units = vec![unit("")];
    let answer = distribute(
        &mut units,
        &query(supplied(&document())),
        &SbomLimits::default(),
    );

    let wire = serde_json::to_string(&answer).expect("serialises");
    let back: DistributionAnswer = serde_json::from_str(&wire).expect("deserialises");

    assert_eq!(back, answer);
}
