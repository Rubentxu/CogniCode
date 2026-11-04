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
//! 4. `<target_directory>/{release,debug}/<NAME>` — donde cargo dice que
//!    escribe (`cargo metadata`), no `<repo>/target`: un checkout con
//!    `target-dir` en `~/.cargo/config.toml` no escribe ahí, y cargo NO
//!    exporta `CARGO_TARGET_DIR` a los procesos de test, así que la
//!    variable no es una autoridad.
//! 5. `<workspace_root>/target/{release,debug}/<NAME>` — fallback
//!    histórico.
//!
//! Las branches 2-5 son robustas bajo `cargo-nextest` (que solo setea
//! env vars en runtime, no en compile-time) y bajo `CARGO_TARGET_DIR`
//! custom.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

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

    // 4. Where cargo says it writes.
    //
    // `<repo>/target` is an assumption, and on this repository it is
    // simply false: `~/.cargo/config.toml` sets `target-dir`, and cargo
    // does NOT export `CARGO_TARGET_DIR` to test processes. A resolver
    // that stops here returns a path that does not exist, and the test
    // that consumes it reports "binary not found" — a message about the
    // harness, dressed as a message about the code.
    //
    // This branch only ever returns a path that exists, and every branch
    // above it is consulted first, so it can never change which bytes a
    // currently-passing test runs.
    if let Some(target) = cargo_target_dir() {
        let release = target.join("release").join(name);
        if release.exists() {
            return release;
        }
        let debug = target.join("debug").join(name);
        if debug.exists() {
            return debug;
        }
    }

    // 5. Workspace-relative fallback.
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

/// Where cargo is actually writing, according to cargo.
///
/// `<repo>/target` is an assumption; `CARGO_TARGET_DIR` is an override
/// that cargo does not propagate to test processes. `cargo metadata` is
/// the only authority for a checkout that redirects `target-dir`.
///
/// Cached for the life of the process: the answer cannot change while a
/// test binary runs, and the alternative is one `cargo metadata`
/// subprocess per resolution.
pub fn cargo_target_dir() -> Option<PathBuf> {
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let out = std::process::Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let meta: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        Some(PathBuf::from(meta.get("target_directory")?.as_str()?))
    })
    .clone()
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
// Antes, cada test de release-flow (`prf_f6_w1_release_coherence`,
// `prf_f6_w2_staging_contract`, `published_skill_bundles_are_required`)
// tenía su propio `repo_root()` calculado
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

/// Path to the `cognicode-release` binary built by the preflight.
///
/// Resolution order (first match wins):
///
/// 1. `COGNICODE_RELEASE_BIN` — absolute path to the release-profile
///    tool. Asserted to exist and returned verbatim.
/// 2. The same resolution chain as `binary_path` (robust under
///    `CARGO_TARGET_DIR` overrides and stale `target/` directories),
///    **plus** a profile assertion: the resolved binary must live in a
///    directory named `release`.
///
/// Why the profile assertion exists: Cargo exports
/// `CARGO_BIN_EXE_<name>` to integration tests, and under a plain
/// `cargo test` that variable resolves to the **debug** build. Without
/// the assertion the release-flow suites would execute debug-profile
/// bytes and certify a release candidate nobody built — green results
/// about the wrong artifact. A suite whose contract is "the release
/// binary does X" has to fail when the bytes it is about to run are not
/// release bytes.
pub fn release_bin_path() -> PathBuf {
    if let Some(raw) = std::env::var_os("COGNICODE_RELEASE_BIN") {
        let p = PathBuf::from(raw);
        assert!(
            p.exists(),
            "COGNICODE_RELEASE_BIN={} does not exist; point it at the absolute \
             path of the release-profile `cognicode-release` binary (built with \
             `cargo build --release --bin cognicode-release`)",
            p.display()
        );
        // The override names the artifact, so it has to satisfy the property
        // the override exists to establish: an override pointing at
        // `debug/cognicode-release` would re-open, through the variable that is
        // meant to close it, the hole the profile assertion below closes.
        assert_release_profile(&p, "COGNICODE_RELEASE_BIN");
        return p;
    }
    let p = release_profile_preferred(binary_path("cognicode-release"));
    assert!(
        p.exists(),
        "cognicode-release binary missing at {}; build it first (`cargo build --release --bin cognicode-release`)",
        p.display()
    );
    assert_release_profile(&p, "release_bin_path");
    p
}

/// The same binary in the release profile, when a sibling `release/` exists.
///
/// `binary_path` prefers `CARGO_BIN_EXE_*`, and under a plain `cargo test` that
/// variable is the **debug** build the test harness just produced. A lane that
/// built the release profile (the clean-clone preflight, the integration lane,
/// the candidate lane) has the artifact one directory over. Preferring it is what
/// keeps the profile assertion below from failing in those lanes while still
/// failing when nothing release-profile exists at all.
fn release_profile_preferred(path: PathBuf) -> PathBuf {
    let parent = match path.parent() {
        Some(p) if p.file_name().is_some_and(|n| n == "debug") => p,
        _ => return path,
    };
    let Some(name) = path.file_name() else {
        return path;
    };
    let Some(dir) = parent.parent() else {
        return path;
    };
    let sibling = dir.join("release").join(name);
    if sibling.exists() { sibling } else { path }
}

/// Assert that `path` lives in a directory named `release`.
///
/// See `release_bin_path` for why the profile matters: the release-flow
/// suites must execute release-profile bytes, and Cargo's
/// `CARGO_BIN_EXE_*` fallback would otherwise let them run a debug build
/// that happens to exist.
fn assert_release_profile(path: &Path, ctx: &str) {
    let in_release = path
        .parent()
        .and_then(|dir| dir.file_name())
        .is_some_and(|name| name == "release");
    assert!(
        in_release,
        "{ctx}: resolved binary {} is not in a `release` profile directory; \
         the release-flow suites must test release-profile bytes. Build with \
         `cargo build --release --bin <name>` or set \
         COGNICODE_RELEASE_BIN=/abs/path/to/release/<name> and re-run.",
        path.display()
    );
}

/// Absolute path to the `release/` directory that contains the
/// release-profile binaries (e.g. `cogh`, `cognicode`, `cognicode-mcp`).
///
/// Resolution order (first match wins):
///
/// 1. `COGNICODE_RELEASE_BIN_DIR` — absolute path to the release
///    directory. Asserted to be an existing directory, returned
///    verbatim.
/// 2. The parent of the resolved cognicode binary when that parent
///    already ends with `release` (e.g. when `cargo test --release`
///    was used). Same path as `cargo build --release` would write to.
/// 3. The sibling `release/` directory when the resolved cognicode
///    binary lives in `debug/` (the usual case under `cargo test`
///    without `--release`). This directory is normally populated by
///    a separate `cargo build --release` invocation.
/// 4. The literal `<repo_root>/target/release` — workspace default.
///
/// Without an override the resolved path must end with a `release`
/// component or this function fails. The reason is the one given in
/// `release_bin_path`: Cargo exports `CARGO_BIN_EXE_<name>` to
/// integration tests and under a plain `cargo test` that variable points
/// at the **debug** build, so a release-flow suite could splice release
/// names onto a debug directory and stay green while reading artifacts
/// that were never built for release.
///
/// Lesson 84 (M0.13): never hard-code `<repo_root>/target/release`
/// because it breaks under a global `~/.cargo/config.toml`
/// target-dir override. Honours the resolved target-dir chosen by
/// Cargo for the integration test, falling back to the workspace
/// default only when it fails.
pub fn release_dir() -> PathBuf {
    if let Some(raw) = std::env::var_os("COGNICODE_RELEASE_BIN_DIR") {
        let dir = PathBuf::from(raw);
        assert!(
            dir.is_dir(),
            "COGNICODE_RELEASE_BIN_DIR={} is not an existing directory; point it \
             at the directory holding the release-profile binaries (cogh, \
             cognicode, cognicode-mcp), e.g. `$CARGO_TARGET_DIR/release`",
            dir.display()
        );
        assert!(
            dir.ends_with("release"),
            "COGNICODE_RELEASE_BIN_DIR={} does not end with a `release` \
             component; the release-flow suites must read release-profile \
             artifacts, and this variable is how the lane that built them says \
             so",
            dir.display()
        );
        return dir;
    }
    let resolved = resolve_release_dir();
    assert!(
        resolved.ends_with("release"),
        "release_dir: resolved {} does not end with a `release` component; the \
         release-flow suites must read release-profile artifacts, not the debug \
         build Cargo produced for this test. Build with `cargo build --release` \
         or set COGNICODE_RELEASE_BIN_DIR=/abs/path/to/release and re-run.",
        resolved.display()
    );
    // Existing at all is the other half of the guarantee: a path that ends in
    // `release` but is not there used to surface as a `src.exists()` panic
    // inside each suite, naming a binary without naming the build that produces
    // it. Failing here names the directory and the command that fills it.
    assert!(
        resolved.is_dir(),
        "release_dir: {} does not exist, so the release-profile binaries this \
         suite reads were never built. Build them with `cargo build --release \
         -p cognicode-cli --bins` and `cargo build --release -p cognicode-mcp`, \
         or point COGNICODE_RELEASE_BIN_DIR at the directory that holds them.",
        resolved.display()
    );
    resolved
}

/// `release_dir` without the overrides, so the profile assertion above
/// covers every branch it can return (today they all end in `release` by
/// construction; the assertion is what keeps that true).
fn resolve_release_dir() -> PathBuf {
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
