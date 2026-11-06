//! El workspace que el producto mira: qué hay en él.
//!
//! Las preguntas que se hacen del workspace —qué lenguajes tiene, qué
//! lenguajes detecta el doctor— se responden con los mismos datos. Por eso
//! la detección vive aquí y no dentro del comando que la usaba primero: dos
//! copias de «qué tiene este workspace» son dos verdades, y un `status` que
//! dijera un idioma y un `doctor` que dijera otro sería una contradicción
//! que nadie podría resolver.

use crate::infrastructure::parser::Language;
use std::collections::HashSet;
use std::path::Path;

/// Los lenguajes que se ven en un workspace, por sus marcadores.
///
/// La detección es por marcadores de proyecto (`Cargo.toml`, `go.mod`,
/// `package.json`, `pom.xml`, `*.py`), no por análisis: es una respuesta
/// barata que el doctor y el goal `status` pueden dar sin tocar el grafo.
pub fn detect_languages(workspace_path: &Path) -> Vec<Language> {
    let mut languages = Vec::new();
    let mut seen = HashSet::new();

    // Check for Cargo.toml -> Rust
    if workspace_path.join("Cargo.toml").exists() && seen.insert(Language::Rust) {
        languages.push(Language::Rust);
    }

    // Check for go.mod -> Go
    if workspace_path.join("go.mod").exists() && seen.insert(Language::Go) {
        languages.push(Language::Go);
    }

    // Check for package.json -> JavaScript/TypeScript
    if workspace_path.join("package.json").exists() {
        if seen.insert(Language::JavaScript) {
            languages.push(Language::JavaScript);
        }
        if seen.insert(Language::TypeScript) {
            languages.push(Language::TypeScript);
        }
    }

    // Check for pom.xml or *.java -> Java
    if (workspace_path.join("pom.xml").exists()
        || glob::glob(&workspace_path.join("**/*.java").to_string_lossy())
            .ok()
            .map(|g| g.count())
            .unwrap_or(0)
            > 0)
        && seen.insert(Language::Java)
    {
        languages.push(Language::Java);
    }

    // Check for *.py files -> Python
    if glob::glob(&workspace_path.join("**/*.py").to_string_lossy())
        .ok()
        .map(|g| g.count())
        .unwrap_or(0)
        > 0
        && seen.insert(Language::Python)
    {
        languages.push(Language::Python);
    }

    languages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_without_markers_has_no_languages() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(detect_languages(dir.path()).is_empty());
    }

    #[test]
    fn a_rust_workspace_is_detected_as_rust() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").expect("write");
        assert_eq!(detect_languages(dir.path()), vec![Language::Rust]);
    }
}
