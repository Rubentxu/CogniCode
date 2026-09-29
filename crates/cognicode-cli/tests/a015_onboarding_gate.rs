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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-cli has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

fn read_workflow() -> String {
    let path = repo_root().join(".github/workflows/pr-ci.yml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The gate step that covers `cogh setup`, as the command line it runs.
///
/// Matches the contract target rather than the literal string `cogh
/// setup`: the gate runs `cargo test --test a015_onboarding_gate`, and the
/// contract is what proves the setup behaviour. The step is named after
/// the command in its `name:`, which is checked below, so the two halves
/// are tied together instead of either drifting alone.
fn gate_setup_step(workflow: &str) -> Option<String> {
    workflow.lines().find_map(|line| {
        let l = line.trim();
        if l.contains("a015_onboarding_gate") {
            Some(l.split('#').next().unwrap_or(l).trim().to_string())
        } else {
            None
        }
    })
}

/// The gate must run the onboarding contract, and name the command.
///
/// RED before the step exists, which is the point: the command is
/// implemented, the action row is open, and nothing ran it.
#[test]
fn the_gate_runs_the_onboarding_contract() {
    let workflow = read_workflow();
    let step = gate_setup_step(&workflow).unwrap_or_else(|| {
        panic!(
            "merge-gate has no step running `--test a015_onboarding_gate`. \
             `cogh setup` is implemented and A-015 asks for the \
             install->doctor->MCP happy path to be covered; a command that \
             nothing exercises is a command that can rot silently. \
             Workflow: {}",
            repo_root().join(".github/workflows/pr-ci.yml").display()
        )
    });
    assert!(
        step.contains("cargo test -p cognicode-cli --test a015_onboarding_gate"),
        "the onboarding step does not run the contract target: {step:?}"
    );
    // The step's name must say which command it protects. A step named
    // after an id that two different actions share is how the licenses
    // gate and the onboarding gate end up indistinguishable in a log.
    let named = workflow
        .lines()
        .any(|l| l.trim().starts_with("- name:") && l.contains("cogh setup"));
    assert!(
        named,
        "the onboarding step is not named after `cogh setup`. The action \
         register uses A-015 for both the onboarding and the licenses gate, \
         so a step named only by id cannot be told apart in a CI log."
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
