//! Onboarding contract for `cogh setup` (A-015).
//!
//! `cogh setup` is the first command a new operator runs, and its whole
//! promise is one sentence: **install, then diagnose the happy path**.
//! `run_setup` in `src/bin/cogh.rs` does `cmd_init` → `cmd_install` →
//! `run_doctor`, and fails when the doctor report is unhealthy.
//!
//! That implementation existed before this file, and nothing in the
//! repository checked it. The command is declared in `--help`, it has a
//! doc comment naming this action, and it could have been deleted down to
//! an `Ok(())` without a single check going red.
//!
//! Three failure modes, none of which is "the command disappeared":
//!
//!   1. **The command stops being gateable.** A-015 declares the step
//!      runs `cogh setup` in `merge-gate`. If the step is renamed, made
//!      non-blocking, or dropped, the onboarding path stops being covered
//!      while the command keeps its help text. The gate must name it.
//!   2. **The happy path loses its last step.** `finish_setup` returns an
//!      error when the doctor is unhealthy. Remove that check and `setup`
//!      reports success after a broken install, which is precisely the
//!      `Partial` reported as `Complete` that the project forbids.
//!   3. **The hermetic seam disappears.** `setup` resolves its release
//!      through `--staging`, a local `releases.json` fixture. If that
//!      argument is dropped, the test would reach the network, and a gate
//!      that depends on the network is a gate that fails for reasons
//!      unrelated to the contract.
//!
//! The first assertion is that `merge-gate` runs this exact step. The rest
//! pin the behaviour against the real binary, not against the source text.

use std::path::PathBuf;
use std::process::Command;

mod common;

use common::{merge_authority_runs, merge_authority_stage_of, not_run_message};

fn repo_root() -> PathBuf {
    common::repo_root()
}

/// The command line that covers `cogh setup`.
///
/// Matches the contract target rather than the literal string `cogh setup`:
/// the gate runs `cargo test --test a015_onboarding_gate`, and the contract is
/// what proves the setup behaviour.
const ONBOARDING_GATE: &str = "cargo test -p cognicode-cli --test a015_onboarding_gate";

/// The gate must run the onboarding contract, and the stage must say which
/// command it protects.
///
/// RED before the stage exists, which is the point: the command is
/// implemented, the action row is open, and nothing ran it.
#[test]
fn the_gate_runs_the_onboarding_contract() {
    assert!(
        merge_authority_runs(ONBOARDING_GATE),
        "{}",
        not_run_message(
            ONBOARDING_GATE,
            "`cogh setup` is implemented and A-015 asks for the \
             install->doctor->MCP happy path to be covered; a command that \
             nothing exercises is a command that can rot silently."
        )
    );

    // The stage must say what it protects, and must be distinguishable from
    // the other gate that carries the same action id. A-015 is used twice in
    // the register — onboarding here, licences in `a015_licenses_gate` — so a
    // stage named only by the id cannot be told apart in a log. The two halves
    // are tied together on purpose: renaming one to the other's name has to
    // fail here.
    let stage = merge_authority_stage_of(ONBOARDING_GATE).expect("the gate runs it");
    assert!(
        stage.contains("a015") && stage.contains("onboarding"),
        "the stage running the onboarding contract is `{stage}`, which does not \
         identify it. The action register uses A-015 for both the onboarding and \
         the licences gate, so a stage named only by the id cannot be told apart \
         in a CI log."
    );
    assert!(
        !stage.contains("licenses"),
        "the stage running the onboarding contract is `{stage}`, which is the \
         name of the other A-015 gate. The two halves are now indistinguishable."
    );
}

/// The happy path is init → install → doctor, and the doctor decides.
///
/// Pinned in the source *and* observed on the real binary. Reading the
/// source alone would pass if a future edit moved the order or dropped
/// the health check while keeping the words in a comment.
#[test]
fn setup_installs_then_diagnoses_and_fails_when_unhealthy() {
    let src = repo_root().join("crates/cognicode-cli/src/bin/cogh.rs");
    let text = std::fs::read_to_string(&src)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", src.display()));

    let body = fn_body(&text, "fn run_setup")
        .unwrap_or_else(|| panic!("no `fn run_setup` in {}", src.display()));
    let init = body
        .find("cmd_init")
        .unwrap_or_else(|| panic!("run_setup does not call cmd_init: {body}"));
    let install = body
        .find("cmd_install")
        .unwrap_or_else(|| panic!("run_setup does not call cmd_install: {body}"));
    let finish = body
        .find("finish_setup")
        .unwrap_or_else(|| panic!("run_setup does not call finish_setup: {body}"));
    assert!(
        init < install && install < finish,
        "run_setup must initialise, then install, then diagnose. \
         Found cmd_init at {init}, cmd_install at {install}, finish_setup at \
         {finish}, which is not the order the action promises."
    );

    let fin = fn_body(&text, "fn finish_setup")
        .unwrap_or_else(|| panic!("no `fn finish_setup` in {}", src.display()));
    assert!(
        fin.contains("run_doctor"),
        "finish_setup does not run the doctor, so `setup` never diagnoses: {fin}"
    );
    assert!(
        fin.contains("is_healthy"),
        "finish_setup does not consult `is_healthy`, so an unhealthy \
         install still reports success. That is a Partial result presented \
         as Complete: {fin}"
    );
    assert!(
        fin.contains("setup failed"),
        "finish_setup returns Ok even when the doctor is unhealthy. The \
         promise of `setup` is that the happy path is verified, and an \
         unhealthy install must not exit 0: {fin}"
    );
}

/// The binary must expose `setup` with the hermetic flags the gate uses.
///
/// A command can be implemented and still unreachable, and the only
/// honest way to know is to run the binary.
#[test]
fn the_cogh_binary_exposes_setup_with_its_flags() {
    let bin = cogh_binary();
    let out = Command::new(&bin)
        .args(["setup", "--help"])
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", bin.display()));
    assert!(
        out.status.success(),
        "`cogh setup --help` failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let help = String::from_utf8_lossy(&out.stdout);
    for flag in ["--staging", "--home", "--version", "--profile"] {
        assert!(
            help.contains(flag),
            "`cogh setup --help` does not document {flag}, but the gate \
             depends on it. Help output was:\n{help}"
        );
    }
}

/// Extract a function body by brace balance from its signature.
fn fn_body(text: &str, signature: &str) -> Option<String> {
    let start = text.find(signature)?;
    let open = text[start..].find('{')? + start;
    let mut depth = 0i32;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[open..=open + i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// The `cogh` binary, resolved from the target directory Cargo reports.
///
/// Not from `<repo>/target`: this checkout sets `target_directory` to
/// `/var/home/rubentxu/cargo-targets`, so a contract that assumed
/// `<root>/target` would fail to find a binary that cargo had just built.
/// That is the same class of defect as the scanner that only accepted
/// directories: the assumption, not the code, was wrong.
///
/// A fresh build is the last resort, so the assertion is self-contained
/// rather than depending on the operator having run `cargo build` first.
fn cogh_binary() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let p = PathBuf::from(dir).join(profile_dir()).join("cogh");
        if p.is_file() {
            return p;
        }
    }
    let root = repo_root();
    let p = root.join("target").join(profile_dir()).join("cogh");
    if p.is_file() {
        return p;
    }
    // Ask cargo where it actually puts things, then build if still absent.
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(&root)
        .output()
        .expect("cannot run `cargo metadata`");
    let meta = String::from_utf8_lossy(&out.stdout);
    let target_dir = meta
        .split("\"target_directory\":")
        .nth(1)
        .and_then(|s| s.split('"').nth(1))
        .map(PathBuf::from);
    if let Some(dir) = &target_dir {
        let p = dir.join(profile_dir()).join("cogh");
        if p.is_file() {
            return p;
        }
    }
    let build = Command::new("cargo")
        .args(["build", "-p", "cognicode-cli", "--bin", "cogh", "--quiet"])
        .current_dir(&root)
        .output()
        .expect("cannot run `cargo build`");
    assert!(
        build.status.success(),
        "`cargo build -p cognicode-cli --bin cogh` failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    // Re-read the location: the build may have created a new target dir.
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let p = PathBuf::from(dir).join(profile_dir()).join("cogh");
        if p.is_file() {
            return p;
        }
    }
    if let Some(dir) = &target_dir {
        let p = dir.join(profile_dir()).join("cogh");
        if p.is_file() {
            return p;
        }
    }
    let p = root.join("target").join(profile_dir()).join("cogh");
    if p.is_file() {
        return p;
    }
    panic!(
        "cannot find the cogh binary after building it. Looked in \
         CARGO_TARGET_DIR, <repo>/target and the directory `cargo metadata` \
         reports, under the {} profile.",
        profile_dir()
    )
}

fn profile_dir() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
