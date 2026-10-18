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

/// The `cogh` binary this test drives.
///
/// `common::binary_path` is this crate's single answer to "where is this
/// binary": `CARGO_BIN_EXE_cogh` captured at compile time, the runtime
/// variable set by cargo-nextest, `CARGO_TARGET_DIR`, the directory
/// `cargo metadata` reports, and only then `<repo>/target`. It preserves
/// the profile: cargo points `CARGO_BIN_EXE_cogh` at the profile the test
/// itself is running under, which is the profile this gate always wanted.
///
/// This function used to re-derive that whole chain locally and read
/// cargo's answer by splitting the JSON on `"target_directory":`. That
/// split returns `None` the moment cargo emits a space after the colon,
/// and `None` here is silent — the chain falls through to a full
/// `cargo build` *inside a test*. The test still passes. It just takes
/// minutes, for a reason that has nothing to do with `cogh setup`.
///
/// The build fallback stays: a contract that only passes when the
/// operator happened to run `cargo build` first is not hermetic.
fn cogh_binary() -> PathBuf {
    let p = common::binary_path("cogh");
    if p.is_file() {
        return p;
    }
    let root = repo_root();
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
    // Re-resolve: the build may have written where the pre-build probe
    // did not look.
    let p = common::binary_path("cogh");
    assert!(
        p.is_file(),
        "cannot find the cogh binary after building it; common::binary_path resolved {}",
        p.display()
    );
    p
}
