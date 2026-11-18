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
