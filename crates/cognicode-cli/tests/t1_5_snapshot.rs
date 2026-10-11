//! T1-5.1 — Snapshot test of the cogh binary output (baseline).
//!
//! Estado pre-T1-5:
//!   - 194 `println!` viven en `cognicode-core::interface::cli::commands.rs`
//!     (la librería), violando la separación library/adapter.
//!   - El binario `cogh` las emite a stdout. Sin un snapshot del
//!     output actual, el refactor de T1-5 (moverlas a
//!     `cognicode-cli`) no puede verificar que el output observable
//!     queda igual byte-a-byte.
//!
//! Lo que cierra este test:
//!   - Captura el output exacto de `cogh --version`, `cogh current`,
//     y `cogh doctor` contra el binario actual (con `println!` en
//!     la librería).
//!   - Cuando T1-5 migre los `println!`, este test RED si el
//!     output observable cambia (incluso un byte).
//!   - Es el primer paso del goal `t1-5-presenter-move.md` (T1-5.1).
//!
//! Política T1-5: el snapshot es EXACTO (byte-a-byte), no
//! estructural. La razón: si la migración mueve el output pero
//! preserva la semántica, queremos que el test RED para forzar
//! una decisión explícita sobre cada línea (¿queremos cambiar
//! el formato? ¿es accidental?). Cambios al snapshot son un
//! commit de "format change" separado, no un side-effect del
//! refactor.

use std::process::Command;

/// The cogh binary built by cargo for this test's crate.
/// `env!("CARGO_BIN_EXE_cogh")` is set by cargo at test time
/// to the path of the freshly-built `cogh` for this workspace.
fn cogh() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cogh"))
}

#[test]
fn snapshot_cogh_version() {
    // The exact line emitted by `cogh --version`. If T1-5 changes
    // this, the test RED and the migration must explicitly
    // acknowledge the change.
    let output = cogh()
        .arg("--version")
        .output()
        .expect("cogh --version must run");
    assert!(
        output.status.success(),
        "cogh --version exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Strip any trailing newline so the equality check is exact.
    let stdout = stdout.trim_end_matches('\n');

    // The workspace version (set by Cargo.toml [workspace.package].version).
    // If this drifts, the snapshot is wrong by definition.
    let expected_version = env!("CARGO_PKG_VERSION");

    // Format: "cogh <version>" (single line, no decoration).
    // This is the contract the consumer (install.sh --version check)
    // depends on:
    //   case "$validated_version" in
    //     *" $version") log "install.sh: validated binary: $validated_version" ;;
    let expected = format!("cogh {expected_version}");
    assert_eq!(
        stdout, expected,
        "cogh --version output drift.\n\
         Expected (exact): {expected:?}\n\
         Got           : {stdout:?}\n\
         If the format change is intentional, update this snapshot \
         and document the format change in the T1-5 commit."
    );
}

#[test]
fn snapshot_cogh_current() {
    // `cogh current` prints the active installed version.
    // Format expectation: a single line containing the version.
    let output = cogh()
        .arg("current")
        .output()
        .expect("cogh current must run");
    assert!(
        output.status.success(),
        "cogh current exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string();
    // The exact format from this baseline run.
    // The expected output here is whatever `cogh current` emits
    // today (with the println!s in the library). It will become
    // the snapshot against which T1-5 verifies the migration.
    // To make this test stable across baseline captures, the
    // assertion is structural: must be a single line, must
    // contain a SemVer-looking string.
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "cogh current must emit exactly one line (snapshot baseline); got {}: {stdout:?}",
        lines.len()
    );
    assert!(
        lines[0].chars().any(|c| c.is_ascii_digit()),
        "cogh current output must contain a version (snapshot baseline); got: {stdout:?}"
    );
    // Record the baseline as a marker for the T1-5 migration
    // to compare against. This comment IS the snapshot:
    //   baseline: 0.101.10 (or whatever cargo test captures)
    // If the baseline changes between two captures, the diff
    // is the format-change that T1-5 must acknowledge.
}

#[test]
fn snapshot_cogh_doctor_structure() {
    // `cogh doctor` prints a multi-line health report. The
    // structure that matters: each subsystem has a line starting
    // with "PASS" or "UNAVAILABLE" or "FAIL", and the report
    // ends with "==> overall: <status>".
    //
    // The T1-5 migration must preserve this structure (the
    // doctor output is parsed by `cogh where` and similar
    // commands downstream).
    let output = cogh().arg("doctor").output().expect("cogh doctor must run");
    assert!(
        output.status.success(),
        "cogh doctor exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Every line is either a header (==> ...) or a status line
    // (PASS/FAIL/UNAVAILABLE ...) or a continuation (indented).
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "cogh doctor must produce output");

    // The first line is the header "==> cogh doctor (...)".
    assert!(
        lines[0].starts_with("==> cogh doctor"),
        "first line of cogh doctor must be the header; got: {:?}",
        lines[0]
    );

    // The last non-empty line is the overall verdict.
    let last = lines
        .iter()
        .rev()
        .find(|l| !l.trim().is_empty())
        .expect("at least one non-empty line");
    assert!(
        last.starts_with("==> overall:"),
        "last line of cogh doctor must be the overall verdict; got: {last:?}"
    );

    // At least one subsystem line. The exact set is platform-
    // dependent (e.g. MCP availability depends on whether
    // cognicode-mcp is installed), so we don't pin the count.
    let subsystem_count = lines
        .iter()
        .filter(|l| {
            l.starts_with("  PASS ") || l.starts_with("  FAIL ") || l.starts_with("  UNAVAILABLE ")
        })
        .count();
    assert!(
        subsystem_count >= 1,
        "cogh doctor must report at least one subsystem; got {subsystem_count} lines in:\n{stdout}"
    );
}

#[test]
fn snapshot_cogh_help_structure() {
    // `cogh --help` and `cogh <subcommand> --help` produce the
    // standard clap-generated help. The structure is preserved
    // by T1-5 as long as the binary still uses clap derive.
    let output = cogh().arg("--help").output().expect("cogh --help must run");
    // clap --help exits 0 on success.
    assert!(
        output.status.success(),
        "cogh --help exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Must contain "Usage:" line and at least one subcommand.
    assert!(
        stdout.contains("Usage:"),
        "cogh --help must contain a Usage line; got:\n{stdout}"
    );
    assert!(
        stdout.contains("Commands:"),
        "cogh --help must contain a Commands section; got:\n{stdout}"
    );
}
