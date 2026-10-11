//! B1.W2 — Identity contract for `cognicode-cli`.
//!
//! Estado pre-B1.W2:
//!   - `cognicode --version` solo emitia `cogh {CARGO_PKG_VERSION}`
//!     (snapshot pin T1-5.1).
//!   - El binario no llevaba source SHA baked-in. Cualquier auditoria
//!     que necesitara verificar "este binario se construyo desde el
//!     commit X" tenia que correr `git rev-parse HEAD` por su cuenta
//!     y confiar en que el binario y el checkout son del mismo repo.
//!   - Las features compiladas (e.g. `ladybug`, `evidence-cli-ladybug`)
//!     no eran inspeccionables; un binario podia reportar capacidades
//!     que su build no tenia.
//!
//! B1.W2 aporta:
//!   - `build.rs` (en este crate) corre `git rev-parse --short HEAD`
//!     al compilar y emite `cargo:rustc-env=GIT_SHA=...`. La SHA queda
//!     baked-in como string literal.
//!   - Este modulo expone `source_sha()`, `version()`, `features()`,
//!     y `print(w)` que escribe un envelope textual con los tres.
//!   - `CliCommand::Identity` (commands.rs) + dispatcher en main.rs
//!     invocan `print()` para responder al usuario.
//!   - Contratos: tests que invocan el binario verifican que las
//!     tres campos son no-vacios cuando el binario se construyo
//!     desde un checkout git.
//!
//! Politica B1.W2:
//!   - `source_sha()` devuelve `"unknown"` cuando el binario se
//!     construyo fuera de un checkout (tarball sin GIT_SHA). Eso es
//!     un signal valido: un release tarball DEBE llevar la SHA baked
//!     al archivarse, no un placeholder. El caso "unknown" sale por
//!     el envelope con etiqueta explicita, no como un empty.
//!   - `features()` lista solo las que fueron ON al compilar. No
//!     incluye features que el workspace declara pero este crate no
//!     usa (e.g. `multimodal` declarada pero no enlazada aqui).
//!   - El byte stream de `--version` NO cambia (T1-5.1 snapshot pin
//!     preservado). El byte stream NUEVO vive en `identity`.

use std::io::{self, Write};

/// Source SHA baked-in por `build.rs`. Vacio cuando no es un repo git.
pub const GIT_SHA: &str = match option_env!("GIT_SHA") {
    Some(s) if !s.is_empty() => s,
    _ => "\0",
};

/// Versión del workspace al momento de compilar.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Lista de features que estaban ON al compilar este binario.
#[allow(unused_mut)] // `mut` solo se usa bajo `#[cfg(feature = "ladybug")]`
pub fn features() -> Vec<&'static str> {
    // Lista hardcoded contra `cognicode-cli/Cargo.toml [features]`.
    // Si anades una feature a `cognicode-cli`, agregala aqui.
    // El binario NO expone features que otros crates declaran pero
    // este no usa.
    let mut out: Vec<&'static str> = Vec::new();
    #[cfg(feature = "ladybug")]
    out.push("ladybug");
    out
}

/// Source SHA real (no el sentinel "unknown"). El binario reportara
/// "unknown" si fue construido fuera de un checkout git (tarball
/// sin GIT_SHA baked). En checkout normal, devuelve los 7 chars
/// cortos del HEAD al momento de compilar.
pub fn source_sha() -> &'static str {
    if GIT_SHA == "\0" {
        "unknown"
    } else {
        // Strip the trailing NUL that the const trick uses to keep
        // the const literal non-empty under `option_env!`.
        GIT_SHA.trim_end_matches('\0')
    }
}

/// Escribe el envelope de identidad a `w`. Formato:
///   `cognicode-cli {version} sha={source_sha} features=[f1,f2,...]\n`
///   followed by a `source: {git_url_or_unknown}` line for full
///   provenance.
pub fn print(w: &mut dyn Write) -> io::Result<()> {
    let feats = features();
    let feats_str = if feats.is_empty() {
        "none".to_string()
    } else {
        feats.join(",")
    };
    writeln!(
        w,
        "cognicode-cli {} sha={} features={}",
        VERSION,
        source_sha(),
        feats_str
    )?;
    // El source URL es lo que el contrato B1.W2 espera como segunda
    // pieza de identidad. Para builds desde este repo, el remote
    // origen esta en `git config --get remote.origin.url`.
    let origin = std::process::Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    writeln!(w, "source: {}", origin)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!VERSION.is_empty());
        assert!(
            VERSION.contains('.'),
            "version should look like semver: got {VERSION:?}"
        );
    }

    #[test]
    fn source_sha_is_either_real_or_unknown() {
        let s = source_sha();
        assert!(!s.is_empty(), "source_sha must not be empty");
        // Si fue built desde un checkout, son 7-12 hex chars
        // (`git rev-parse --short` adapts to repo ambiguity;
        // typically 7 in fresh ones, 9+ in repos with collisions).
        // Si no, "unknown".
        assert!(
            s == "unknown"
                || (s.len() >= 7 && s.len() <= 12 && s.chars().all(|c| c.is_ascii_hexdigit())),
            "source_sha must be 'unknown' or 7-12 hex chars; got {s:?}"
        );
    }

    #[test]
    fn print_writes_two_lines() {
        let mut buf: Vec<u8> = Vec::new();
        print(&mut buf).expect("print infallible");
        let s = String::from_utf8(buf).expect("UTF-8");
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 2, "expected exactly 2 lines, got {lines:?}");
        assert!(
            lines[0].starts_with("cognicode-cli "),
            "first line should declare identity; got {lines:?}"
        );
        assert!(
            lines[1].starts_with("source: "),
            "second line should declare source; got {lines:?}"
        );
    }

    #[test]
    fn features_are_non_empty_or_none_labeled() {
        let f = features();
        // Si no hay features, `print` las etiqueta como "none".
        // El test solo verifica que la lista no contiene vacios.
        for feat in &f {
            assert!(!feat.is_empty(), "feature labels must not be empty");
        }
    }
}
