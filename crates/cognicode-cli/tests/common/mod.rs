//! Harness compartido para UATs de CLI (binarios reales).
//! Helper centralizado para resolver la ruta de los binarios `cogh` y
//! `cognicode` bajo test, además de helpers de workspace (versión, tag,
//! repo_root) compartidos entre los tests de release-flow.
//!
//! Cada archivo `tests/*.rs` es un test binary independiente (Cargo
//! genera un binario por archivo), por lo que `tests/common/mod.rs`
//! está disponible vía `mod common;` + `use common::binary_path;` en
//! cada test.
//!
//! Resolución (primer match gana):
//!
//! 1. `CARGO_BIN_EXE_<NAME>` (compile-time) — set por Cargo cuando un
//!    integration test del mismo crate se ejecuta. Correcto siempre,
//!    sin traversal del filesystem. Fallback graceful si está unset.
//! 2. `CARGO_BIN_EXE_<NAME>` (runtime) — set por cargo-nextest. Si el
//!    compile-time fallback no encontró nada, intenta leer el env var
//!    en runtime.
//! 3. `CARGO_TARGET_DIR/release/<NAME>` y
//!    `CARGO_TARGET_DIR/debug/<NAME>` — honra el override de target-dir
//!    del usuario.
//! 4. `<workspace_root>/target/{release,debug}/<NAME>` — fallback
//!    histórico.
//!
//! Las branches 2-4 son robustas bajo `cargo-nextest` (que solo setea
//! env vars en runtime, no en compile-time) y bajo `CARGO_TARGET_DIR`
//! custom.

#![allow(dead_code)]

use std::path::PathBuf;

/// Resolve the absolute path to the given binary under test.
///
/// `name` is the cargo binary name (e.g. `"cogh"` or `"cognicode"`).
/// The returned path is absolute and points at the binary that cargo
/// built for this test target (release profile by default, debug if
/// the binary doesn't exist in release).
pub fn binary_path(name: &str) -> PathBuf {
    resolve_binary_path(name)
}

fn resolve_binary_path(name: &str) -> PathBuf {
    // 1. Compile-time env var: set by `cargo test --bin <name>` and
    //    by integration tests in the same crate as the binary.
    if let Some(p) = compile_time_bin_exe(name) {
        return PathBuf::from(p);
    }

    // 2. Runtime env var: set by `cargo-nextest` (and by wrappers
    //    that invoke cargo manually). Independent of compile-time
    //    capture.
    if let Some(p) = runtime_bin_exe(name) {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }

    // 3. CARGO_TARGET_DIR override (release profile).
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let release = PathBuf::from(&target).join("release").join(name);
        if release.exists() {
            return release;
        }
        let debug = PathBuf::from(&target).join("debug").join(name);
        if debug.exists() {
            return debug;
        }
    }

    // 4. Workspace-relative fallback.
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate parent")
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let release = workspace_root.join("target").join("release").join(name);
    if release.exists() {
        return release;
    }
    workspace_root.join("target").join("debug").join(name)
}

fn compile_time_bin_exe(name: &str) -> Option<&'static str> {
    // env! requires a literal string. Match on the known names.
    match name {
        "cogh" => option_env!("CARGO_BIN_EXE_cogh"),
        "cognicode" => option_env!("CARGO_BIN_EXE_cognicode"),
        _ => None,
    }
}

fn runtime_bin_exe(name: &str) -> Option<std::ffi::OsString> {
    let var = format!("CARGO_BIN_EXE_{name}");
    std::env::var_os(var)
}

// ---------------------------------------------------------------------------
// CR-00c: helpers de workspace para tests de release-flow.
//
// Antes, cada test (`prf_dist_01_06_release_candidate_uat`,
// `prf_dist_workflow_flatten_uat`, `prf_f6_w2_staging_contract`,
// `prf_f6_w1_release_coherence`) tenía su propio `repo_root()` calculado
// y `const VERSION: &str = "0.97.4"` hardcoded. Esto los hacía
// NO-herméticos en clean clone (la versión 0.97.4 no corresponde al
// workspace actual, los payloads generados no satisfacían el check R8 del
// binario `cognicode-release`). Los helpers aquí centralizan:
//   - el cálculo del workspace root (reutilizable entre tests),
//   - la lectura de la versión canónica del workspace desde
//     `[workspace.package]` (acepta también `[workspace]` legacy),
//   - el tag derivado (`v<version>`),
//   - el path del binario `cognicode-release` con assert de existencia.
// ---------------------------------------------------------------------------

/// Absolute path to the workspace root (the directory that contains the
/// top-level `Cargo.toml`).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate parent")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// Canonical workspace version, derived from `[workspace.package]` (or
/// the legacy `[workspace]` section) of the workspace `Cargo.toml`.
/// Panics if not found — tests must NOT bake the version into the
/// source as a constant.
pub fn workspace_version() -> String {
    let manifest = std::fs::read_to_string(repo_root().join("Cargo.toml"))
        .expect("workspace Cargo.toml must be readable from the clone");
    let mut section: Option<&str> = None;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = Some(trimmed);
            continue;
        }
        let in_workspace = matches!(section, Some("[workspace]") | Some("[workspace.package]"));
        if in_workspace && let Some(rest) = trimmed.strip_prefix("version") {
            let rest = rest.trim_start().strip_prefix('=').unwrap_or(rest);
            let rest = rest.trim();
            let stripped = rest.trim_matches('"');
            if !stripped.is_empty() {
                return stripped.to_string();
            }
        }
    }
    panic!("could not find workspace version in Cargo.toml");
}

/// Git tag for the workspace version (`v<version>`).
pub fn workspace_tag() -> String {
    format!("v{}", workspace_version())
}

/// Path to the `cognicode-release` binary built by the preflight. Uses
/// the same resolution chain as `binary_path` so it is robust under
/// `CARGO_TARGET_DIR` overrides and stale `target/` directories.
pub fn release_bin_path() -> PathBuf {
    let p = binary_path("cognicode-release");
    assert!(
        p.exists(),
        "cognicode-release binary missing at {}; build it first (`cargo build --release --bin cognicode-release`)",
        p.display()
    );
    p
}

/// Absolute path to the `release/` directory that contains the
/// release-profile binaries (e.g. `cogh`, `cognicode`, `cognicode-mcp`).
///
/// Resolution order (first match wins):
///
/// 1. The parent of the resolved cognicode binary when that parent
///    already ends with `release` (e.g. when `cargo test --release`
///    was used). Same path as `cargo build --release` would write to.
/// 2. The sibling `release/` directory when the resolved cognicode
///    binary lives in `debug/` (the usual case under `cargo test`
///    without `--release`). This directory is normally populated by
///    a separate `cargo build --release` invocation.
/// 3. The literal `<repo_root>/target/release` — workspace default.
///
/// Lesson 84 (M0.13): never hard-code `<repo_root>/target/release`
/// because it breaks under a global `~/.cargo/config.toml`
/// target-dir override. Honours the resolved target-dir chosen by
/// Cargo for the integration test, falling back to the workspace
/// default only when it fails.
pub fn release_dir() -> PathBuf {
    let probe = binary_path("cognicode");
    let parent = probe
        .parent()
        .map(PathBuf::from)
        .expect("release_dir: probe has no parent");
    if parent.ends_with("release") {
        return parent;
    }
    if parent.ends_with("debug") {
        let sibling = parent.parent().map(|p| p.join("release"));
        if let Some(s) = sibling
            && s.exists()
        {
            return s;
        }
    }
    repo_root().join("target").join("release")
}

/// The pipeline a merge is gated on. Declared here and nowhere else.
pub const MERGE_AUTHORITY: &str = "merge-gate.pipeline.kts";

/// The pipeline that builds and certifies a release candidate.
///
/// Named here for the same reason the merge authority is: a contract that says
/// "the release lane must generate SBOMs with the shared script" has to name
/// one file, and the file it names must be the one that runs.
pub const RELEASE_CANDIDATE_AUTHORITY: &str = "release-candidate.pipeline.kts";

/// One pipeline's source, whole-line comments removed.
///
/// A comment that quotes a command is otherwise indistinguishable from a step
/// that runs it, in both directions: a comment naming a removed gate would
/// satisfy "the gate exists", and a comment explaining why something is pinned
/// would trip "it is pinned".
pub fn pipeline_text(pipeline: &str) -> String {
    let path = repo_root().join(pipeline);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {pipeline}: {e}"));
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One pipeline's source as trimmed, comment-free lines.
pub fn pipeline_lines(pipeline: &str) -> Vec<String> {
    pipeline_text(pipeline)
        .lines()
        .map(|l| l.trim().to_owned())
        .collect()
}

/// Every `sh(...)` body in `text`, with its byte offset in that text.
///
/// Extracting bodies rather than scanning lines is what lets a command that
/// wraps count. The line-based check this replaced documented the limitation
/// and prescribed moving the command onto one line — the wrong remedy, because
/// it distorts a pipeline to satisfy a checker, and the distortion is invisible
/// to whoever reads the pipeline next. It also diverged: `pipeline_authority.py`
/// had already worked on bodies, and the two Rust copies had not caught up, so
/// a contract failed against a pipeline that does run the command.
///
/// A command outside an `sh(` body still does not count. A `val`, a stage name,
/// or a comment that survived stripping would run nothing.
///
/// Kotlin's `${'$'}` is resolved, because the contract reads the `.kts` source
/// rather than the emitted shell and the escape is an artefact of that.
/// The end (exclusive) of the call whose argument list starts at `from`.
///
/// String literals are skipped rather than scanned for parentheses, so a `)`
/// that belongs to a shell command cannot close the call early.
fn call_end(text: &str, from: usize) -> Option<usize> {
    const RAW: &str = "\"\"\"";
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if text[i..].starts_with(RAW) {
                let after = i + RAW.len();
                i = after + text[after..].find(RAW)? + RAW.len();
                continue;
            }
            i += 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i += 2,
                    b'"' => break,
                    _ => i += 1,
                }
            }
            i += 1;
            continue;
        }
        match bytes[i] {
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// Every string literal between `from` and `to`, in source order, as
/// `(absolute offset, contents)`.
///
/// Raw and regular literals alike, with Kotlin's `${'$'}` resolved and
/// regular-string escapes dropped: the contracts read the `.kts` source rather
/// than the shell it emits, and the escape is an artefact of that.
fn string_literals_in(text: &str, from: usize, to: usize) -> Vec<(usize, String)> {
    const RAW: &str = "\"\"\"";
    let bytes = text.as_bytes();
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut i = from;
    while i < to {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        if text[i..to].starts_with(RAW) {
            let start = i + RAW.len();
            let Some(len) = text[start..to].find(RAW) else {
                break;
            };
            out.push((start, text[start..start + len].replace("${'$'}", "$")));
            i = start + len + RAW.len();
            continue;
        }
        let start = i + 1;
        let mut j = start;
        let mut end = None;
        while j < to {
            match bytes[j] {
                b'\\' => j += 2,
                b'"' => {
                    end = Some(j);
                    break;
                }
                _ => j += 1,
            }
        }
        let Some(end) = end else { break };
        out.push((
            start,
            text[start..end].replace('\\', "").replace("${'$'}", "$"),
        ));
        i = end + 1;
    }
    out
}

pub fn sh_bodies_with_offsets(text: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut cursor = 0usize;

    while let Some(offset) = text[cursor..].find("sh(") {
        let after = cursor + offset + 3;
        let Some(end) = call_end(text, after) else {
            break;
        };
        // The LAST literal, not the first: a preamble such as
        // `releasePaths + "\n"` puts a one-character separator ahead of the
        // body, and a body of one character is a command that runs nothing.
        if let Some(body) = string_literals_in(text, after, end).pop() {
            out.push(body);
        }
        cursor = end + 1;
    }

    out
}

/// Whether `pipeline` runs `command` inside an `sh(...)` body.
///
/// The `sh(` requirement is what makes this fail-closed: a command quoted
/// anywhere else would run nothing.
pub fn pipeline_runs(pipeline: &str, command: &str) -> bool {
    sh_bodies_with_offsets(&pipeline_text(pipeline))
        .iter()
        .any(|(_, body)| body.contains(command))
}

/// The name of the stage that runs `command`, for the failure message.
///
/// Found by walking back from the body that contains the command rather than by
/// matching a line, so a command that is not on the same line as its `sh(` is
/// still attributed to the right stage.
pub fn pipeline_stage_of(pipeline: &str, command: &str) -> Option<String> {
    let text = pipeline_text(pipeline);
    let offset = sh_bodies_with_offsets(&text)
        .into_iter()
        .find(|(_, body)| body.contains(command))
        .map(|(offset, _)| offset)?;
    stage_before(&text[..offset])
}

/// The innermost `stage("…")` that opens before `offset`.
///
/// Textual rather than structural, and deliberately so: it has to name the
/// stage a failing contract points at, and parsing Kotlin's braces to do that
/// would be a second implementation of a language to answer a question about
/// error messages.
fn stage_before(prefix: &str) -> Option<String> {
    let mut current = None;
    for line in prefix.lines() {
        let line = line.trim();
        if let Some(name) = line
            .strip_prefix("stage(\"")
            .and_then(|rest| rest.find('"').map(|end| rest[..end].to_owned()))
        {
            current = Some(name);
        }
    }
    current.or_else(|| Some("(top level)".to_owned()))
}

/// A failure message naming the pipeline, the command and the stages that do
/// exist, so a RED from these contracts is actionable without a diff.
pub fn pipeline_not_run_message(pipeline: &str, command: &str, required: &str) -> String {
    let stages: Vec<String> = pipeline_lines(pipeline)
        .into_iter()
        .filter_map(|l| {
            l.strip_prefix("stage(\"")
                .and_then(|r| r.find('"').map(|e| r[..e].to_owned()))
        })
        .collect();
    format!(
        "{pipeline} does not run `{command}`.\n{required}\nstages present: {}",
        if stages.is_empty() {
            "none".to_owned()
        } else {
            stages.join(", ")
        }
    )
}

/// The merge authority's source with whole-line comments removed.
pub fn merge_authority_lines() -> Vec<String> {
    pipeline_lines(MERGE_AUTHORITY)
}

/// Whether the merge authority runs `command` inside an `sh(...)` body.
///
/// The `sh(` requirement is what makes this fail-closed. A command quoted
/// anywhere else — a `val`, a stage name, a comment that survived stripping —
/// does not count, because it would run nothing. A command spread over a
/// multi-line `sh("""…""")` also does not count, which is a false negative
/// rather than a false pass.
pub fn merge_authority_runs(command: &str) -> bool {
    pipeline_runs(MERGE_AUTHORITY, command)
}

/// The name of the stage that runs `command`, for the failure message.
pub fn merge_authority_stage_of(command: &str) -> Option<String> {
    pipeline_stage_of(MERGE_AUTHORITY, command)
}

/// A failure message naming the authority, the command and the stages that do
/// exist, so a RED from these contracts is actionable without a diff.
pub fn not_run_message(command: &str, required: &str) -> String {
    pipeline_not_run_message(MERGE_AUTHORITY, command, required)
}

/// `pipeline:stage` for every pipeline that runs `command`, sorted.
///
/// The list form matters: a caller asking "where does this run?" needs the
/// answer to be empty *because nothing runs it* rather than because a lookup
/// missed, and a single `bool` cannot tell those apart. It is the same shape
/// as `scripts/ci/pipeline_authority.py::invoked_by`, which answers the same
/// question for the Python contracts.
pub fn invoked_by(command: &str) -> Vec<String> {
    let mut out: Vec<String> = invocations(command).into_iter().map(|i| i.at).collect();
    out.sort();
    out
}

/// One `sh(...)` body that runs `command`, and whether it can stop the lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// `pipeline:stage`.
    pub at: String,
    /// `true` when a non-zero exit from `command` reaches the end of the stage,
    /// and a failing stage aborts the pipeline (measured, see
    /// `scripts/ci/probe-pipelinek-semantics.sh`).
    pub blocking: bool,
}

/// Every `sh(...)` body in every pipeline that runs `command`, paired with
/// whether it can stop the lane.
///
/// The distinction exists because *running* a checker and *enforcing* it are
/// different facts, and only the second one may be claimed. `merge-gate` runs
/// `clippy` and fails the merge; `certification` runs `perf-budget-check`,
/// prints that the verdict is not a PASS, and continues. Both invoke a
/// command, and a contract that only asks "is it invoked?" cannot tell them
/// apart — which is how `perf-budget.toml` came to declare `ENFORCEMENT: none`
/// while a lane ran the checker, and how nobody noticed for as long as the
/// reader could not see inside a multi-line `sh("""…""")` body.
///
/// Two mechanisms disable errexit around an invocation, and they are the two
/// the lanes themselves document ("ADVISORY IS `|| echo`, AND HERE THAT IS THE
/// ONLY OPTION"):
///
///   * `|| …` on the invocation line — `bash x.sh || echo 'ADVISORY: …'`;
///   * `set +e` (or `set +o errexit`) earlier in the same body, so the
///     invocation's status is captured instead of aborting.
///
/// A third mechanism is a deliberate edit to this list, not an accident: a
/// body this function cannot classify is reported as **blocking**, because the
/// dangerous reading of an unknown stage is the one that can turn a lane red.
pub fn invocations(command: &str) -> Vec<Invocation> {
    let mut out: Vec<Invocation> = Vec::new();
    for pipeline in pipeline_paths() {
        let name = match pipeline.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let Ok(text) = std::fs::read_to_string(&pipeline) else {
            continue;
        };
        let live: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for (offset, body) in sh_bodies_with_offsets(&live) {
            if !body.contains(command) {
                continue;
            }
            let Some(stage) = stage_before(&live[..offset]) else {
                continue;
            };
            out.push(Invocation {
                at: format!("{name}:{stage}"),
                blocking: reaches_stage_exit(&body, command),
            });
        }
    }
    out.sort_by(|a, b| a.at.cmp(&b.at));
    out
}

/// Whether the exit status of `command` can become the exit status of the
/// `sh(...)` body that contains it.
fn reaches_stage_exit(body: &str, command: &str) -> bool {
    let Some(at) = body.find(command) else {
        return true;
    };
    let end = body[at..].find('\n').map_or(body.len(), |nl| at + nl);

    // `bash x.sh || echo …` — the status is consumed by the guard.
    if body[at + command.len()..end].contains("||") {
        return false;
    }
    // `set +e` earlier in the same body — the status is captured, not raised.
    // Its absence is what leaves errexit armed, which is the blocking case.
    !body[..at].lines().any(|l| {
        let t = l.trim();
        t == "set +e" || t == "set +o errexit"
    })
}

/// Every PipelineK script at the repository root, sorted.
///
/// The set of files that decide what runs, so it is what a scan for "is this
/// named by machine-readable orchestration" has to cover. While GitHub Actions
/// existed, the `.github/workflows/*.yml` files were scanned alongside these;
/// with the workflows gone, scanning anything else would make the rule
/// quietly cover less than it claims.
pub fn pipeline_paths() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(repo_root())
        .unwrap_or_else(|e| panic!("cannot read the repository root: {e}"))
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()?.to_str()? == "kts").then_some(p)
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MEDIDO 2026-10-03. El extractor solo reconocia un cuerpo cuando la
    /// cadena empieza INMEDIATAMENTE despues de `sh(`. Con la forma que la
    /// lane usa en 10 de sus stages,
    ///
    ///     sh(releasePaths + "\n" + """
    ///         ...cuerpo...
    ///     """.trimIndent())
    ///
    /// lo que sigue a `sh(` es el identificador `releasePaths`, no una cadena,
    /// asi que ninguna de las dos ramas lo aceptaba y la stage entera se
    /// escapaba del extractor.
    ///
    /// No se noto como un fallo de la stage, sino como un fallo de un
    /// contrato: `prf_f6_w3_bis_candidate_lane_stages_through_the_shared_flatten_script`
    /// seguia verde porque la unica stage con forma `sh("""` que contenia
    /// `verify --staging release` era `re-verify-after-upload`. Al darle a esa
    /// stage el preambulo que le hacia falta de verdad —el mismo preambulo que
    /// ya llevaban las otras tres— el contrato dejo de ver el comando. La stage
    /// que hace el trabajo, `re-verify-candidate`, nunca fue visible para el.
    ///
    /// Estos dos tests fijan las dos formas. El primero es el caso que ya
    /// funcionaba y se conserva como regresion.
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
        assert!(bodies[0].1.contains("run-the-thing --now"), "{:?}", bodies[0]);
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
    /// devolveria la cadena de un caracter del separador, que no ejecuta nada.
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
    /// `certification.pipeline.kts` has both: `t6-regression-test` invokes its
    /// script bare, and `perf-budget-verdict` invokes a script under `set +e`
    /// and prints that the exit code is not a PASS. If `reaches_stage_exit`
    /// answered one constant for both, the `ENFORCEMENT` contract in
    /// `perf_budget_checker_contract.rs` would be a rubber stamp that happens to
    /// be green.
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
}

// ---------------------------------------------------------------------------
// Merge authority
// ---------------------------------------------------------------------------
//
// Several contracts in this crate used to read `.github/workflows/pr-ci.yml`
// to find out whether the merge gate ran them. That file is being retired:
// the pipeline that gates a merge is now `merge-gate.pipeline.kts`. Each test
// that re-derived that fact on its own would be a second place to update, and
// the failure mode is not a compile error — it is a contract that silently
// stops asserting anything.
//
// So the fact lives here once. The same idea is implemented for the core
// contracts in `crates/cognicode-core/tests/common/mod.rs` and for the Python
// contracts in `scripts/ci/pipeline_authority.py`. Two languages cannot share
// a module, so there are three small implementations of one rule, and all of
// them are strict in the same direction: a match only counts on a line that
// actually invokes it.
