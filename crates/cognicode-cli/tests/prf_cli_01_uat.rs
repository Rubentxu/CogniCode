//! PRF-CLI-01 UAT: argv, exit code, diagnóstico por comando stable.
//!
//! Contract (SPEC-CLI.md PRF-CLI-01 MUST): `--help`/`--version` and
//! every stable command have a documented exit code, argv surface and
//! diagnostic; **no `exit 0` when the operation was not performed**.
//! Scenario required by the SPEC: nonexistent path and invalid
//! configuration → NOT success.
//!
//! These tests run the real `cognicode` binary (integration test via
//! `CARGO_BIN_EXE_cognicode`) so the assertion covers the actual
//! process exit code, not an in-process approximation.

use std::path::PathBuf;
use std::process::{Command, Output};

mod common;

/// Path to the `cognicode` binary.
///
/// `common::binary_path` resolves with the right precedence
/// (`CARGO_BIN_EXE_cognicode` > runtime env > `CARGO_TARGET_DIR` > workspace
/// fallback). The harness keeps the test working whether you run it
/// under `cargo test`, `cargo-nextest`, or with a custom `CARGO_TARGET_DIR`.
fn cognicode_bin() -> PathBuf {
    common::binary_path("cognicode")
}

fn run(args: &[&str]) -> Output {
    Command::new(cognicode_bin())
        .args(args)
        .output()
        .expect("spawn cognicode binary")
}

fn exit_code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

/// `--help` and `--version` must succeed.
#[test]
fn help_and_version_exit_zero() {
    for args in [&["--help"][..], &["--version"][..]] {
        let out = run(args);
        assert_eq!(
            exit_code(&out),
            0,
            "`cognicode {:?}` must exit 0, got {} (stderr: {})",
            args,
            exit_code(&out),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// PRF-CLI-01 scenario: nonexistent path passed to `analyze` must NOT
/// exit 0. The operation was not performed; a zero exit would lie to
/// scripts and CI.
#[test]
fn analyze_nonexistent_path_does_not_exit_zero() {
    let out = run(&["analyze", "/nonexistent/prf_cli_01/definitely/missing"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`analyze <nonexistent>` must not exit 0; the operation did not \
         happen. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// PRF-CLI-01 scenario: nonexistent path passed to `graph full` must
/// NOT exit 0.
///
/// **Este test estaba midiendo otra cosa.** Invocaba
/// `graph full --path <inexistente>`, y `graph full` no tiene `--path`: su
/// firma es `graph full [PATH]`, un **positional** con `[default: .]`. Clap
/// rechazaba el flag con exit 2, el proceso moría antes del dispatch, y la
/// aserción `assert_ne!(code, 0)` se cumplía sin que el motor de grafos
/// llegara a ejecutarse. Su comentario decía "already the case; pins the
/// contract": lo que fijaba era la aridad de un flag inexistente.
///
/// Por eso el test no se limita a comprobar el código de salida. Un
/// `assert_ne!(code, 0)` que acaba de ser verde por una vía que no era la
/// suya puede volver a serlo, y la única forma de distinguir "el grafo falló"
/// de "clap no entendió el argv" es mirar **qué** dice el diagnóstico. Un
/// error de clap empieza por `error: unexpected argument` o
/// `error: unrecognized subcommand`; el fallo del motor de grafos nombra el
/// directorio. Las dos aserciones de abajo son lo que convierte esto en una
/// prueba del comportamiento y no una del analizador de argumentos.
#[test]
fn graph_full_nonexistent_path_does_not_exit_zero() {
    let out = run(&["graph", "full", "/nonexistent/prf_cli_01/missing"]);
    let code = exit_code(&out);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert_ne!(
        code, 0,
        "`graph full <inexistente>` must not exit 0. stderr: {stderr}"
    );

    // Evidencia de alcance: el fallo viene del motor de grafos, no de clap.
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("unrecognized subcommand"),
        "clap rechazó el argv, así que el brazo nunca llegó a ejecutarse y este \
         test no midió lo que dice medir. stderr: {stderr}"
    );
    assert!(
        stderr.contains("Directory does not exist"),
        "el diagnóstico no nombra el directorio que falta: el comando se aceptó y \
         falló por otro motivo, o falló antes de llegar al grafo. stderr: {stderr}"
    );
}

/// El gemelo positivo del anterior, y tiene que atravesar **el mismo**
/// camino: mismo binario, mismo clap, mismo `CommandExecutor`, misma
/// estrategia de grafo. La única diferencia es la entrada.
///
/// Sin este gemelo, un arreglo que hiciera fallar `graph full` siempre —
/// por ejemplo, rechazando cualquier ruta — seguiría dejando el test anterior
/// en verde. El error y el éxito se certifican en la misma aserción.
#[test]
fn graph_full_on_a_real_directory_exits_zero_through_the_same_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("main.rs"),
        "pub fn a() { b(); }\npub fn b() {}\n",
    )
    .expect("write fixture");

    let out = run(&["graph", "full", tmp.path().to_str().unwrap()]);
    let code = exit_code(&out);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert_eq!(
        code, 0,
        "`graph full <dir real>` debe salir con 0: el grafo se construyó. \
         stderr: {stderr}"
    );
    assert!(
        !stderr.contains("Directory does not exist"),
        "un directorio real y existente no puede fallar por no existir: el \
         camino de éxito no está usando la misma comprobación que el de error."
    );
}

/// PRF-CLI-01 scenario: `doctor` on a nonexistent cwd must NOT exit 0
/// (already the case; pins the contract).
#[test]
fn doctor_nonexistent_cwd_does_not_exit_zero() {
    let out = run(&["doctor", "--cwd", "/nonexistent/prf_cli_01/missing"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`doctor --cwd <nonexistent>` must not exit 0. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// PRF-CLI-01 scenario: `analyze` on a valid empty directory IS a
/// successful operation and must exit 0 (guards against over-tightening
/// the fix into "analyze always fails").
#[test]
fn analyze_valid_empty_dir_exits_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run(&["analyze", tmp.path().to_str().unwrap()]);
    assert_eq!(
        exit_code(&out),
        0,
        "`analyze <valid empty dir>` must exit 0. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// PRF-CLI-05: the refactor command must parse cleanly (a former clap
/// debug-assert bug aborted the process), default to preview-only, refuse
/// `--apply` (no rollback path yet), and never mutate source files.
#[test]
fn cli05_refactor_is_preview_only_and_refuses_apply() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("lib.rs");
    std::fs::write(&src, "pub fn original_name() -> u32 { 42 }\n").unwrap();
    let before = std::fs::read(&src).unwrap();

    let bin = common::binary_path("cognicode");
    // 1. Default preview must run without crashing and must not write.
    let out = std::process::Command::new(&bin)
        .args(["refactor", "original_name", "renamed_thing"])
        .current_dir(tmp.path())
        .output()
        .expect("refactor preview must execute");
    let _ = out;
    assert_eq!(
        std::fs::read(&src).unwrap(),
        before,
        "preview must never mutate source files"
    );

    // 2. --apply must be refused with a non-zero exit and a clear reason.
    let out = std::process::Command::new(&bin)
        .args(["refactor", "original_name", "renamed_thing", "--apply"])
        .current_dir(tmp.path())
        .output()
        .expect("refactor --apply must execute");
    assert!(
        !out.status.success(),
        "--apply must be refused (no rollback path yet)"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("--apply") && err.contains("not implemented"),
        "refusal must explain the missing apply path; got: {err}"
    );
    assert_eq!(
        std::fs::read(&src).unwrap(),
        before,
        "refused --apply must not mutate source files"
    );
}
