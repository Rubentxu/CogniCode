//! B1.W2 — Build script: bake source identity into the binary.
//!
//! Emits `cargo:rustc-env=GIT_SHA=<short SHA>` so the runtime can
//! answer `cognicode identity` (and `--version --verbose`) with the
//! exact commit the binary was built from. The contract `B1.W2`
//! requires this: a binary claiming v0.101.11 but built from a
//! different commit is a forged identity, not a release artifact.
//!
//! Behaviour:
//!   - `git rev-parse --short HEAD` at build time. If the workspace
//!     is not a git checkout (e.g. release tarball), the env var
//!     is empty and the runtime reports `"unknown"` instead of
//!     panicking — that case is itself a signal (a release tarball
//!     SHOULD carry the SHA baked at archive time, not a placeholder).
//!   - `cargo:rerun-if-changed=.git/HEAD` so a fresh commit triggers
//!     a rebuild. We do NOT rerun-if-changed on the working tree
//!     (it would rebuild on every edit), only on HEAD movement.
//!
//!   MEDIDO 2026-10-16. El `rerun-if-changed` apuntaba a `../../.git/HEAD`, y en
//!   un **worktree de git** `.git` es un fichero, no un directorio: la ruta no
//!   existe y cargo, que no distingue "no existe" de "cambió", marcaba el crate
//!   como sucio en **cada** invocación. Coste real: `cargo doc --workspace
//!   --no-deps` tardaba 163 s con la documentación ya construida y no la
//!   reutilizaba nunca (`Dirty cognicode-cli: the file ../../.git/HEAD is
//!   missing`), y todo crate que depende de `cognicode-cli` se recompilaba con
//!   él en cada etapa del gate. Ahora la ruta se resuelve con
//!   `git rev-parse --absolute-git-dir`, que funciona en checkout, worktree y
//!   submódulo, y sólo se declara si existe.
//!
//! Does NOT affect the byte stream of `--version` (T1-5.1 snapshot
//! pins `"cogh {CARGO_PKG_VERSION}"`); that flag stays byte-identical.
//! Identity is exposed via `cognicode identity` (B1.W2 new variant).

use std::path::PathBuf;
use std::process::Command;

/// Directorio real de git, o `None` si no se puede determinar.
///
/// `git rev-parse --absolute-git-dir` responde la ruta del gitdir de verdad: en
/// un worktree es `<repo-principal>/.git/worktrees/<nombre>`, y en un checkout
/// normal es `<repo>/.git`. Sin esto, un `rerun-if-changed` a una ruta
/// inexistente mantiene el crate sucio para siempre.
fn git_dir() -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(path.trim());
    path.is_dir().then_some(path)
}

fn main() {
    let short_sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    println!("cargo:rustc-env=GIT_SHA={}", short_sha);

    // Sólo se declara lo que existe: un `rerun-if-changed` a una ruta ausente
    // deja el crate permanentemente sucio, y eso se paga en cada etapa del gate.
    if let Some(dir) = git_dir() {
        for candidate in [dir.join("HEAD"), dir.join("refs").join("heads")] {
            if candidate.exists() {
                println!("cargo:rerun-if-changed={}", candidate.display());
            }
        }
    }
}
