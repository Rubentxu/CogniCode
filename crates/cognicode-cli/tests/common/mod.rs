//! Harness compartido para UATs de CLI (binarios reales).
//! Helper centralizado para resolver la ruta de los binarios `cogh` y
//! `cognicode` bajo test.
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

#[cfg(test)]
mod tests {
    use super::*;

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
