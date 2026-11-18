use super::*;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let none = root.join("no-git-configuration");
    let output = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", &none)
        .env("GIT_CONFIG_SYSTEM", &none)
        .output()
        .expect("git is installed");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

const MANIFEST: &str =
    "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";

fn lock(version: &str) -> String {
    format!(
        "version = 3\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"serde\"]\n\n[[package]]\nname = \"serde\"\nversion = \"{version}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
        "a".repeat(64)
    )
}

fn commit(root: &Path, version: &str, message: &str) {
    std::fs::write(root.join("Cargo.toml"), MANIFEST).expect("manifest");
    std::fs::write(root.join("Cargo.lock"), lock(version)).expect("lock");
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", message]);
}

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    commit(root, "1.0.1", "base");
    directory
}

fn input() -> DependenciesInput {
    serde_json::from_value(serde_json::json!({})).expect("every argument has a default")
}

#[test]
fn the_revision_defaults_to_head_and_the_inventory_names_it() {
    let repo = repository();

    let answer = dependencies(repo.path(), &input()).expect("answer");

    assert_eq!(answer.inventory.units.len(), 1);
    assert_eq!(answer.inventory.units[0].packages.len(), 2);
    assert_eq!(answer.inventory.revision.requested_ref, "HEAD");
    assert_eq!(
        answer.inventory.units[0].packages[1].purl.as_deref(),
        Some("pkg:cargo/serde@1.0.1")
    );
    assert!(answer.limits.iter().any(|limit| limit.contains("offline")));
}

#[test]
fn a_base_ref_adds_the_lockfile_delta() {
    let repo = repository();
    git(repo.path(), &["tag", "base"]);
    commit(repo.path(), "1.0.2", "bump");
    let ask = DependenciesInput {
        base_ref: Some("base".into()),
        dependents_of: Some("serde".into()),
        ..input()
    };

    let answer = dependencies(repo.path(), &ask).expect("answer");

    let comparison = answer.inventory.comparison.expect("comparison");
    assert_eq!(comparison.units[0].delta.updated[0].to, vec!["1.0.2"]);
    let dependents = answer.inventory.dependents.expect("dependents");
    assert_eq!(
        dependents[0]
            .report
            .as_ref()
            .expect("known")
            .affected_members,
        vec!["app@0.1.0"]
    );
}

#[test]
fn an_unknown_revision_is_an_error_not_an_empty_inventory() {
    let repo = repository();
    let ask = DependenciesInput {
        revision: "no-such-ref".into(),
        ..input()
    };

    assert!(dependencies(repo.path(), &ask).is_err());
}

#[test]
fn the_text_names_the_stale_lockfile_and_what_was_not_collected() {
    let repo = repository();
    std::fs::write(
        repo.path().join("Cargo.toml"),
        format!("{MANIFEST}itoa = \"1\"\n"),
    )
    .expect("manifest");
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "--quiet", "-m", "declare itoa"]);
    let answer = dependencies(repo.path(), &input()).expect("answer");
    let value = serde_json::to_value(answer).expect("serialisable");

    let text = render(&value);

    assert!(text.contains("1 with a stale lockfile"), "{text}");
    assert!(text.contains("DeclaredNotLocked"), "{text}");
    assert!(
        text.contains("not collected: observed=notcollected"),
        "{text}"
    );
}

fn sbom_text(extra: Option<serde_json::Value>) -> String {
    let mut components = vec![serde_json::json!({
        "type": "library",
        "bom-ref": "serde",
        "name": "serde",
        "version": "1.0.1",
        "purl": "pkg:cargo/serde@1.0.1",
        "scope": "required",
        "hashes": [{"alg": "SHA-256", "content": "a".repeat(64)}],
    })];
    components.extend(extra);
    serde_json::json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "lifecycles": [{"phase": "build"}],
            "component": {"type": "application", "bom-ref": "app", "name": "app", "version": "0.1.0"},
        },
        "components": components,
    })
    .to_string()
}

fn with_sbom(repo: &Path, text: &str) -> DependenciesInput {
    std::fs::write(repo.join("app.cdx.json"), text).expect("sbom");
    DependenciesInput {
        sbom: Some("app.cdx.json".into()),
        ..input()
    }
}

#[test]
fn without_an_sbom_the_answer_carries_no_distribution_and_the_wire_shape_is_unchanged() {
    let repo = repository();

    let answer = dependencies(repo.path(), &input()).expect("answer");
    let value = serde_json::to_value(&answer).expect("serialisable");

    assert!(answer.inventory.distributed.is_none());
    assert!(value.get("distributed").is_none(), "{value}");
}

#[test]
fn an_sbom_in_the_workspace_is_compared_and_bound_to_its_bytes() {
    let repo = repository();
    let text = sbom_text(None);
    let ask = with_sbom(repo.path(), &text);

    let answer = dependencies(repo.path(), &ask).expect("answer");

    let distributed = answer.inventory.distributed.as_ref().expect("distributed");
    assert_eq!(distributed.status, SbomStatus::Usable);
    let document = distributed.document.as_ref().expect("document");
    assert_eq!(
        document.digest,
        crate::domain::scm::ContentDigest::from_bytes(text.as_bytes()).as_hex()
    );
    let report = distributed.units[0].report.as_ref().expect("report");
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert!(
        answer.summary.contains("SBOM (a producer claim)"),
        "{}",
        answer.summary
    );
    let text = render(&serde_json::to_value(&answer).expect("serialisable"));
    assert!(text.contains("SBOM app.cdx.json digest"), "{text}");
}

#[test]
fn an_sbom_that_lists_a_package_the_lockfile_does_not_resolve_is_reported() {
    let repo = repository();
    let ghost = serde_json::json!({
        "type": "library", "bom-ref": "ghost", "name": "ghost", "version": "9.9.9",
        "purl": "pkg:cargo/ghost@9.9.9",
    });
    let ask = with_sbom(repo.path(), &sbom_text(Some(ghost)));

    let answer = dependencies(repo.path(), &ask).expect("answer");

    let report = answer
        .inventory
        .distributed
        .as_ref()
        .expect("distributed")
        .units[0]
        .report
        .as_ref()
        .expect("report");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.subject.contains("ghost"))
    );
    let text = render(&serde_json::to_value(&answer).expect("serialisable"));
    assert!(text.contains("ListedNotResolved"), "{text}");
}

#[test]
fn a_missing_sbom_is_reported_as_unreadable_not_as_an_empty_one() {
    let repo = repository();
    let ask = DependenciesInput {
        sbom: Some("missing.cdx.json".into()),
        ..input()
    };

    let answer = dependencies(repo.path(), &ask).expect("answer");

    let distributed = answer.inventory.distributed.as_ref().expect("distributed");
    assert_eq!(distributed.status, SbomStatus::Unreadable);
    assert!(distributed.problem.is_some());
    assert!(
        answer.summary.contains("SBOM not used"),
        "{}",
        answer.summary
    );
}

#[test]
fn an_sbom_outside_the_workspace_is_never_read() {
    let repo = repository();
    let outside = tempfile::tempdir().expect("outside");
    let path = outside.path().join("other.cdx.json");
    std::fs::write(&path, sbom_text(None)).expect("sbom");
    let ask = DependenciesInput {
        sbom: Some(path.to_string_lossy().into_owned()),
        ..input()
    };

    let answer = dependencies(repo.path(), &ask).expect("answer");

    let distributed = answer.inventory.distributed.as_ref().expect("distributed");
    assert_eq!(distributed.status, SbomStatus::Unreadable);
    assert!(distributed.document.is_none());
}

#[test]
fn a_pinned_digest_that_differs_refuses_the_sbom_and_a_malformed_pin_refuses_the_goal() {
    let repo = repository();
    let mut ask = with_sbom(repo.path(), &sbom_text(None));
    ask.sbom_expected_digest = Some(format!("sha256:{}", "f".repeat(64)));

    let answer = dependencies(repo.path(), &ask).expect("answer");
    assert_eq!(
        answer
            .inventory
            .distributed
            .as_ref()
            .expect("distributed")
            .status,
        SbomStatus::DigestMismatch
    );

    ask.sbom_expected_digest = Some("not-a-digest".into());
    let error = dependencies(repo.path(), &ask).expect_err("refused");
    assert!(error.contains("sbom_expected_digest"), "{error}");
}

#[test]
fn an_sbom_argument_without_an_sbom_is_a_contradiction_not_a_silent_default() {
    let repo = repository();
    for ask in [
        DependenciesInput {
            artifact_digest: Some("a".repeat(64)),
            ..input()
        },
        DependenciesInput {
            sbom_member: Some("app".into()),
            ..input()
        },
        DependenciesInput {
            sbom_expected_digest: Some("a".repeat(64)),
            ..input()
        },
    ] {
        let error = dependencies(repo.path(), &ask).expect_err("refused");
        assert!(error.contains("needs `sbom`"), "{error}");
    }
}

#[test]
fn the_published_limits_say_the_distributed_layer_is_a_claim() {
    let repo = repository();

    let answer = dependencies(repo.path(), &input()).expect("answer");

    assert!(
        answer
            .limits
            .iter()
            .any(|limit| limit.contains("producer") && limit.contains("never verified"))
    );
}
