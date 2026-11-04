//! B1.W2 — Identity contract for `cognicode` binary.
//!
//! Estado pre-B1.W2:
//!   - `cognicode --version` solo emitia la version. El SHA fuente
//!     no estaba baked-in; las features compiladas no eran visibles.
//!     Cualquier auditoria que necesitara probar "este binario se
//!     construyo desde el commit X" tenia que correr `git rev-parse
//!     HEAD` por su cuenta y confiar en que el binario y el checkout
//!     son del mismo repo (forja facil de identidad falsa).
//!
//! B1.W2 aporta:
//!   - `cognicode identity` emite el envelope:
//!     `cognicode-cli {version} sha={sha} features={f1,f2,...}`
//!     `source: {git_remote_url}`
//!     La SHA viene de `build.rs` que corre `git rev-parse --short HEAD`
//!     al compilar y emite `cargo:rustc-env=GIT_SHA=...`.
//!   - `--version` se preserva byte-a-byte (snapshot T1-5.1).
//!
//! Tests que pinean:
//!   - `cognicode identity` exit 0 (identidad debe estar disponible).
//!   - Primera linea empieza con `cognicode-cli ` + semver.
//!   - Segunda linea empieza con `source: ` + URL o `unknown`.
//!   - El campo `sha=` es 7+ hex chars (real) o literal `unknown`
//!     (binario construido fuera de un checkout git).
//!   - `--version` sigue emitiendo `cognicode {version}` (no cambia).

use std::process::Command;

/// The cognicode binary built by cargo for this test's crate.
/// `env!("CARGO_BIN_EXE_cognicode")` is set by cargo at test time
/// to the path of the freshly-built `cognicode` for this workspace.
fn cognicode() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cognicode"))
}

#[test]
fn identity_exits_zero() {
    let output = cognicode()
        .arg("identity")
        .output()
        .expect("cognicode identity must run");
    assert!(
        output.status.success(),
        "cognicode identity exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn identity_first_line_declares_identity() {
    let output = cognicode()
        .arg("identity")
        .output()
        .expect("cognicode identity must run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout
        .lines()
        .next()
        .expect("identity must emit at least one line");
    assert!(
        first.starts_with("cognicode-cli "),
        "first line should start with 'cognicode-cli '; got {first:?}"
    );
    // After the prefix, expect a semver-looking version (x.y.z).
    let after_prefix = first.strip_prefix("cognicode-cli ").unwrap();
    let version_token = after_prefix
        .split_whitespace()
        .next()
        .expect("first token after prefix should be the version");
    assert!(
        version_token.contains('.'),
        "version token should look like semver (x.y.z); got {version_token:?}"
    );
}

#[test]
fn identity_sha_is_real_or_unknown() {
    let output = cognicode()
        .arg("identity")
        .output()
        .expect("cognicode identity must run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout
        .lines()
        .next()
        .expect("identity must emit at least one line");
    let sha_token = first
        .split_whitespace()
        .find(|tok| tok.starts_with("sha="))
        .expect("identity line should carry sha=<value>");
    let sha = sha_token
        .strip_prefix("sha=")
        .expect("sha= prefix should be present");
    assert!(
        sha == "unknown"
            || (sha.len() >= 7 && sha.len() <= 12 && sha.chars().all(|c| c.is_ascii_hexdigit())),
        "sha must be 'unknown' or 7-12 hex chars; got {sha:?}"
    );
}

#[test]
fn identity_second_line_declares_source() {
    let output = cognicode()
        .arg("identity")
        .output()
        .expect("cognicode identity must run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 2,
        "identity must emit at least 2 lines; got {lines:?}"
    );
    assert!(
        lines[1].starts_with("source: "),
        "second line should start with 'source: '; got {:?}",
        lines[1]
    );
    let url = lines[1]
        .strip_prefix("source: ")
        .expect("source: prefix should be present");
    assert!(!url.is_empty(), "source URL must not be empty; got {url:?}");
}

#[test]
fn version_is_unchanged_after_b1_w2() {
    // B1.W2 must NOT touch the byte stream of `cognicode --version`
    // (the T1-5.1 snapshot pins `cognicode {CARGO_PKG_VERSION}`).
    let output = cognicode()
        .arg("--version")
        .output()
        .expect("cognicode --version must run");
    assert!(
        output.status.success(),
        "cognicode --version exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next().expect("--version must emit a line");
    assert!(
        first.starts_with("cognicode "),
        "--version line should start with 'cognicode '; got {first:?}"
    );
    let version_token = first.strip_prefix("cognicode ").unwrap().trim().to_string();
    assert!(
        version_token.contains('.'),
        "--version should emit a semver-like string; got {version_token:?}"
    );
}
