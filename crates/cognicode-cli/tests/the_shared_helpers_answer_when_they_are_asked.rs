//! The shared helpers of this crate answer when they are asked.
//!
//! These checks lived inside the module that every test binary of this
//! crate shares, so every binary compiled them and every binary ran
//! them: nine checks, twenty-two times each. A check that runs in
//! twenty-two places is not twenty-two checks, it is one check with
//! twenty-two copies of its cost — and when it goes red, the report says
//! twenty-two things failed when only one thing is wrong.
//!
//! The module that every binary shares is a module of helpers now. A
//! helper that is also a test is two things in one place, and the second
//! one gets paid for by everyone.

#[path = "common/mod.rs"]
mod common;

use common::{binary_path, invocations, reaches_stage_exit, sh_bodies_with_offsets};

/// El extractor sólo reconocía un cuerpo cuando la cadena empezaba
/// inmediatamente después de `sh(`. Con la forma que la lane usa en
/// muchas de sus stages,
///
/// ```text
///     sh(releasePaths + "\n" + """
///         ...cuerpo...
///     """.trimIndent())
/// ```
///
/// lo que sigue a `sh(` es el identificador `releasePaths`, no una cadena,
/// así que ninguna de las dos ramas lo aceptaba y la stage entera se
/// escapaba del extractor.
///
/// No se notó como un fallo de la stage, sino como un fallo de un
/// contrato: un contrato que miraba una stage con forma `sh("""` que no
/// era la que hace el trabajo seguía verde, porque la stage que hace el
/// trabajo nunca fue visible para él. Al darle a esa stage el preámbulo
/// que le hacía falta de verdad —el mismo preámbulo que ya llevaban las
/// otras— el contrato dejó de ver el comando.
///
/// Estos dos checks fijan las dos formas. El primero es el caso que ya
/// funcionaba y se conserva como regresión.
#[test]
fn a_plain_sh_string_body_is_extracted() {
    let text = r#"
        stage("a") {
            sh("""
                run-the-thing --now
            """.trimIndent())
        }
    "#;
    let bodies = sh_bodies_with_offsets(text);
    assert_eq!(bodies.len(), 1, "expected exactly one body, got {bodies:?}");
    assert!(
        bodies[0].1.contains("run-the-thing --now"),
        "{:?}",
        bodies[0]
    );
}

#[test]
fn a_sh_body_behind_a_preamble_expression_is_still_extracted() {
    let text = r#"
        stage("a") {
            sh(releasePaths + "\n" + """
                run-the-thing --now
            """.trimIndent())
        }
    "#;
    let bodies = sh_bodies_with_offsets(text);
    assert_eq!(
        bodies.len(),
        1,
        "un preambulo antes del cuerpo no puede volver la stage invisible: \
         got {bodies:?}"
    );
    assert!(
        bodies[0].1.contains("run-the-thing --now"),
        "el cuerpo es el ultimo literal de la llamada, no el primero: {:?}",
        bodies[0]
    );
}

/// El cuerpo es el ULTIMO literal porque un preambulo puede traer otro
/// delante —`releasePaths + "\n" + cuerpo`— y quedarse con el primero
/// devolvería la cadena de un caracter del separador, que no ejecuta nada.
#[test]
fn the_body_is_the_last_literal_not_the_first() {
    let text = r#"
        stage("a") {
            sh(releasePaths + "\n" + """
                the-real-body
            """.trimIndent())
        }
    "#;
    let bodies = sh_bodies_with_offsets(text);
    let body = &bodies[0].1;
    assert!(
        body.contains("the-real-body"),
        "el cuerpo real tiene que estar: {body:?}"
    );
    // Quedarse con el PRIMER literal devuelve el separador `"\n"` del
    // preambulo: un cuerpo de un caracter que no ejecuta nada, y que haria
    // que un contrato creyera que la stage corre un comando.
    assert_ne!(
        body, "\n",
        "el cuerpo extraido es el separador del preambulo, no el cuerpo"
    );
}

/// The repository already contains one honest example of each kind, so the
/// classifier is checked against them rather than against fixtures written
/// to match it.
///
/// `certification.pipeline.kts` has both: a stage invokes its script bare,
/// and another invokes a script under `set +e` and prints that the exit
/// code is not a PASS. If the classifier answered one constant for both,
/// the contract that reads it would be a rubber stamp that happens to be
/// green.
#[test]
fn a_bare_invocation_is_blocking_and_a_guarded_one_is_not() {
    let bare = invocations("check_regression_test");
    let bare_blocking: Vec<&str> = bare
        .iter()
        .filter(|i| i.blocking)
        .map(|i| i.at.as_str())
        .collect();
    assert!(
        bare_blocking.contains(&"certification.pipeline.kts:t6-regression-test"),
        "a stage that runs its script with nothing between it and the exit \
         status can stop the lane, and must be classified as blocking. \
         Classified as: {bare:?}"
    );

    let guarded = invocations("perf-budget-check");
    assert!(
        !guarded.is_empty(),
        "the performance verdict is the repository's advisory example; if it \
         disappeared, this test would stop testing anything"
    );
    let guarded_blocking: Vec<&str> = guarded
        .iter()
        .filter(|i| i.blocking)
        .map(|i| i.at.as_str())
        .collect();
    assert!(
        guarded_blocking.is_empty(),
        "a stage that captures the exit code and says in its output that the \
         verdict is not a PASS cannot stop the lane, and must not be \
         classified as blocking. Classified as: {guarded:?}"
    );
}

/// What the classifier keys on, stated as behaviour rather than as prose.
/// A body it cannot read must come back blocking, because the expensive
/// mistake is a lane that turns red and nobody can say why.
#[test]
fn only_a_guard_that_precedes_the_invocation_makes_it_advisory() {
    assert!(
        reaches_stage_exit("out=$(bash x.sh 2>&1)", "x.sh"),
        "a bare command substitution has the script's exit status, so under \
         `set -e` it aborts the stage"
    );
    assert!(
        !reaches_stage_exit("set +e\nout=$(bash x.sh 2>&1)\nset -e\n", "x.sh"),
        "`set +e` in force when the script runs means the status was \
         captured, and a `set -e` afterwards does not un-capture it"
    );
    assert!(
        !reaches_stage_exit("bash x.sh || echo 'ADVISORY: …'", "x.sh"),
        "an inline guard consumes the status"
    );
    assert!(
        reaches_stage_exit("out=$(bash x.sh 2>&1)\nset +e\n", "x.sh"),
        "order matters: a `set +e` written after the invocation did not \
         protect it"
    );
    assert!(
        reaches_stage_exit("unrelated line", "x.sh"),
        "a body with no invocation of the command is unclassifiable, and \
         the dangerous reading is the one reported"
    );
}

/// `binary_path("cogh")` must return an absolute path ending in
/// `cogh`. Existence is environment-dependent (the binary may or
/// may not be built) so we only pin the path shape.
#[test]
fn binary_path_cogh_resolves_to_filename() {
    let path = binary_path("cogh");
    assert_eq!(
        path.file_name().and_then(|s| s.to_str()),
        Some("cogh"),
        "binary_path(\"cogh\") must end with cogh, got: {}",
        path.display()
    );
}

/// `binary_path("cognicode")` must return an absolute path ending
/// in `cognicode`.
#[test]
fn binary_path_cognicode_resolves_to_filename() {
    let path = binary_path("cognicode");
    assert_eq!(
        path.file_name().and_then(|s| s.to_str()),
        Some("cognicode"),
        "binary_path(\"cognicode\") must end with cognicode, got: {}",
        path.display()
    );
}

/// When `CARGO_BIN_EXE_cogh` is set at compile time, the path
/// must point at that env var. This is the canonical happy path
/// under `cargo test`.
#[test]
fn binary_path_prefers_cargo_bin_exe_env_var_when_set() {
    if let Some(expected) = option_env!("CARGO_BIN_EXE_cogh") {
        let path = binary_path("cogh");
        assert_eq!(
            path.to_str(),
            Some(expected),
            "binary_path(\"cogh\") must return CARGO_BIN_EXE_cogh when set at compile time"
        );
    }
    // If the env var is unset at compile time, the test is a
    // no-op: resolution falls through to the other branches,
    // which are environment-dependent and not worth pinning.
}

/// Unknown binary name returns the workspace-relative fallback
/// path (no panic). Existence is the caller's problem.
#[test]
fn binary_path_unknown_name_returns_fallback() {
    let path = binary_path("nonexistent-binary-xyz");
    assert_eq!(
        path.file_name().and_then(|s| s.to_str()),
        Some("nonexistent-binary-xyz"),
        "unknown name must still produce a path with that filename"
    );
}
