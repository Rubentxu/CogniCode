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

use crate::common;

use common::repo_root;

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
/// binary": the `CARGO_BIN_EXE_cogh` cargo fixed when it built this test, in
/// the profile the test itself is running under.
///
/// There is no build fallback. Cargo builds the package's binaries before it
/// compiles its integration tests, so a missing file here is a broken
/// harness, not a state to repair: a `cargo build` launched from inside a
/// test is paid on every run of the suite and on every mutant of the gate.
fn cogh_binary() -> PathBuf {
    let p = common::binary_path("cogh");
    assert!(
        p.is_file(),
        "the cogh binary cargo built for this test is missing at {}",
        p.display()
    );
    p
}
