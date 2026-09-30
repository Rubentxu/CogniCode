//! QW-09 — release-artifact reachability guard (contractual).
//!
//! Pinea el contrato de `scripts/ci/check-release-artifact-reachability.sh`.
//!
//! CI spent two 19-minute merge-gate runs on a single defect: `merge-gate`
//! asserted on `target/release/cognicode` while never downloading the
//! artifact that `build-binary` uploaded. `needs:` makes a job WAIT for
//! another job; it does not move artifacts between runners, because every
//! job gets its own fresh runner. The fix that worked was one `download-
//! artifact` step inside `merge-gate`, placed before the first step that
//! needs it.
//!
//! The contract is therefore ORDER-SENSITIVE, which is the part that is easy
//! to get wrong: having the build or download somewhere in the job is not
//! enough. Both RED scenarios below are real shapes this repo actually hit:
//!
//!   1. ABSENT   — the job needs binaries and never obtains them at all
//!      (the defect that broke PR-CI twice).
//!   2. TOO LATE — the job obtains them, but after the step that needs them
//!      (the `Release-profile binaries` step at line ~576 vs
//!      `prf_cli_04` at line 514, in file order).
//!
//! The test is RED if the guard does not catch these. Checking only that the
//! guard passes on a clean workflow would pin nothing: the guard's job is to
//! fail on broken ones, so the broken cases are what must be verified.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

use common::repo_root;

fn guard() -> PathBuf {
    let p = repo_root().join("scripts/ci/check-release-artifact-reachability.sh");
    assert!(p.exists(), "QW-09 guard missing at {}", p.display());
    p
}

fn run_guard(root: &Path) -> std::process::Output {
    Command::new("bash")
        .arg(guard())
        .arg(root)
        .output()
        .expect("failed to spawn check-release-artifact-reachability.sh")
}

/// Materialise `workflow` as the only workflow of a throwaway root and run
/// the guard against it. Keeps every scenario hermetic: the real repo's
/// `pr-ci.yml` is never mutated.
fn run_against(workflow: &str, tag: &str) -> std::process::Output {
    let tmp = tempdir(tag);
    let dir = tmp.join(".github/workflows");
    fs::create_dir_all(&dir).expect("mkdir .github/workflows");
    fs::write(dir.join("pr-ci.yml"), workflow).expect("write pr-ci.yml");
    run_guard(&tmp)
}

/// Pull a real historical version of the workflow out of git.
fn workflow_at(rev: &str) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_root())
        .args(["show", &format!("{rev}:.github/workflows/pr-ci.yml")])
        .output()
        .expect("git show");
    assert!(
        out.status.success(),
        "could not read pr-ci.yml at {rev}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf-8 workflow")
}

fn tempdir(tag: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "qw09-reachability-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = base.join(unique);
    fs::create_dir_all(&path).expect("create tempdir");
    path
}

#[test]
fn qw09_guard_passes_on_the_real_workflow() {
    // GREEN baseline: the workflow as it stands today is correctly wired.
    let out = run_guard(&repo_root());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "QW-09 guard failed on the real, fixed workflow:\n\
         --- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        stdout.contains("OK: every job that needs release binaries"),
        "guard success banner missing; got:\n{stdout}"
    );
}

/// RED 1 — the actual PR-CI defect.
///
/// `38f57443` is the commit that built and uploaded the CLI correctly. The
/// gate still failed, because `merge-gate` had no download step of its own:
/// the one in the workflow belonged to `test-pr`. This is the real file, not
/// a synthetic construction, so the guard is proven against history.
#[test]
fn qw09_guard_fails_on_the_workflow_that_broke_merge_gate() {
    let broken = workflow_at("38f57443");
    let out = run_against(&broken, "absent");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        !out.status.success(),
        "QW-09 guard did NOT detect the workflow that actually broke the \
         merge gate at run 36682728317. stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("merge-gate"),
        "guard did not name the offending job. Got:\n{stdout}"
    );
    assert!(
        stdout.contains("needs:"),
        "guard output does not explain the `needs:` misconception, which is \
         the whole point of the check. Got:\n{stdout}"
    );
}

/// RED 2 — order, not absence.
///
/// The download exists and the job is otherwise fine, but it sits AFTER the
/// first step that needs the binaries. Steps run in file order, so the
/// binaries still are not there when the black-box suite runs. The guard must
/// catch this and say so, because the two failures have different fixes and
/// a misleading message sends you to the wrong one.
#[test]
fn qw09_guard_fails_when_the_download_comes_after_its_first_user() {
    let fixed = workflow_at("514b5441");
    assert!(
        fixed.contains("Descargar binarios release (merge-gate)"),
        "expected the fix commit to contain the download step"
    );

    // Move the download step to just before `Release-profile binaries`,
    // which is after every black-box suite in the job.
    let lines: Vec<&str> = fixed.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("      - name: Descargar binarios release (merge-gate)"))
        .expect("download step present");
    let mut end = start + 1;
    while end < lines.len() && !lines[end].starts_with("      - ") {
        end += 1;
    }
    let block: Vec<&str> = lines[start..end].to_vec();

    let mut mutated: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    mutated.drain(start..end);
    let target = mutated
        .iter()
        .position(|l| l.starts_with("      - name: Release-profile binaries"))
        .expect("Release-profile step present");
    for (offset, line) in block.iter().enumerate() {
        mutated.insert(target + offset, (*line).to_string());
    }
    let mutated = mutated.join("\n") + "\n";

    // The mutation must actually be an inversion, otherwise this test would
    // pass for the wrong reason.
    let dl = mutated
        .lines()
        .position(|l| l.starts_with("      - name: Descargar binarios release (merge-gate)"))
        .expect("download still present");
    let first_user = mutated
        .lines()
        .position(|l| l.contains("black-box"))
        .expect("a black-box suite is present");
    assert!(
        dl > first_user,
        "mutation did not put the download after its first user \
         (download at {dl}, first user at {first_user})"
    );

    let out = run_against(&mutated, "too-late");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        !out.status.success(),
        "QW-09 guard did NOT detect a download placed after the step that \
         needs it. stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("AFTER the step that needs them"),
        "guard failed but did not diagnose the ORDERING problem, so the \
         message would send a reader to the wrong fix. Got:\n{stdout}"
    );
}

/// The guard must be pinned in the merge gate, or none of the above runs.
/// Mirrors `cli_gate_coverage_contract`'s self-pin.
///
/// This reads the workflow from the WORKING TREE, not from a revision: the
/// pin and the guard land in the same commit, so pinning against history
/// would make this test impossible to satisfy in the commit that introduces
/// it. `the_coverage_contract_itself_is_pinned` in
/// `cli_gate_coverage_contract.rs` reads the file on disk for the same reason.
#[test]
fn qw09_guard_itself_is_pinned_in_the_merge_gate() {
    let workflow = fs::read_to_string(repo_root().join(".github/workflows/pr-ci.yml"))
        .expect("read pr-ci.yml");
    assert!(
        workflow.contains("check-release-artifact-reachability.sh"),
        "the QW-09 guard is not named in pr-ci.yml, so it never runs in the \
         merge gate and this whole file is dead code"
    );
    assert!(
        workflow.contains("--test qw09_release_artifact_reachability"),
        "the QW-09 contractual test is not named in pr-ci.yml, so the guard \
         could rot in silence: the two RED scenarios would stop running"
    );
}
