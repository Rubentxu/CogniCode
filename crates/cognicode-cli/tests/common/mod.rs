//! Harness compartido para los tests de CLI (binarios reales).
//!
//! Resuelve la ruta de los binarios `cogh`, `cognicode` y
//! `cognicode-release` bajo test, y reúne los helpers de workspace
//! (versión, tag, repo_root) que comparten los tests de release-flow.
//!
//! Cada archivo bajo `tests/` es un módulo de un binario de test (o un
//! binario de test propio); `tests/common/mod.rs` se incluye vía
//! `mod common;`.
//!
//! ## Dónde está el binario bajo test
//!
//! Cargo construye los binarios del paquete ANTES de compilar sus tests de
//! integración y le entrega a cada uno la ruta en `CARGO_BIN_EXE_<nombre>`,
//! fijada en tiempo de compilación. Esa es la única autoridad: no se
//! pregunta a `cargo metadata` dónde escribe (un subproceso por ejecución de
//! la suite, y una respuesta sobre el directorio de quien corre, que puede
//! compartirse con otro checkout), no se construye nada desde un test y no
//! hay variables de entorno que la sustituyan.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Absolute path to the binary under test.
///
/// `name` is the cargo binary name: `"cogh"`, `"cognicode"` or
/// `"cognicode-release"`. The path is the one cargo fixed when it built this
/// test target, in whichever profile the suite is being run.
///
/// An unknown name is a defect of the caller and fails loudly: the match is
/// closed because `env!` needs a literal, and a name that falls through to a
/// guessed path would turn "the harness does not know this binary" into
/// "binary not found".
pub fn binary_path(name: &str) -> PathBuf {
    let path = match name {
        "cogh" => env!("CARGO_BIN_EXE_cogh"),
        "cognicode" => env!("CARGO_BIN_EXE_cognicode"),
        "cognicode-release" => env!("CARGO_BIN_EXE_cognicode-release"),
        other => panic!(
            "binary_path: `{other}` is not a binary of cognicode-cli; \
             add it to the match in tests/common/mod.rs"
        ),
    };
    PathBuf::from(path)
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

/// A cache of its own, for a test that runs the product.
///
/// The product keeps what it calculates in the user's cache
/// (`~/.cache/cognicode`). A test that ran the real binary against the
/// real cache would leave entries in the cache of whoever runs the suite —
/// nobody can clean them, and nobody knows which project they describe —
/// and it would read a snapshot that another run left behind, so the
/// answer would depend on what ran before.
///
/// The caller keeps the returned directory alive for as long as the test
/// needs it; dropping it takes the cache with it.
pub fn isolated_cache() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary cache")
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
/// 2. `binary_path("cognicode-release")`, or its release-profile sibling
///    when the suite runs in `debug/`, **plus** a profile assertion: the
///    resolved binary must live in a directory named `release`.
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
///
/// Without an override the resolved path must end with a `release`
/// component or this function fails. The reason is the one given in
/// `release_bin_path`: Cargo exports `CARGO_BIN_EXE_<name>` to
/// integration tests and under a plain `cargo test` that variable points
/// at the **debug** build, so a release-flow suite could splice release
/// names onto a debug directory and stay green while reading artifacts
/// that were never built for release.
///
/// There is no fallback to a literal `<repo_root>/target/release`: that
/// path is an assumption about where cargo writes, and on a checkout that
/// redirects `target-dir` it names a directory that does not exist or, worse,
/// one that belongs to another checkout. When the sibling `release/` is not
/// there the answer is a failure that names the build that fills it.
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
    panic!(
        "release_dir: no release-profile directory next to {}; build the \
         release binaries first (`cargo build --release -p cognicode-cli --bins` \
         and `cargo build --release -p cognicode-mcp`) or point \
         COGNICODE_RELEASE_BIN_DIR at the directory that holds them",
        probe.display()
    );
}
