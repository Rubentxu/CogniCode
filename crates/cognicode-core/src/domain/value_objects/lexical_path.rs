//! Normalización léxica de rutas: resuelve `.` y `..` sin tocar el disco.
//!
//! Es la única dueña de esta regla. A diferencia de `canonicalize`, no falla
//! si la ruta no existe todavía (el directorio de un workspace aún sin
//! indexar, el destino de un refactor) y no sigue symlinks.

use std::path::{Component, Path, PathBuf};

/// Quita los `.` y resuelve cada `..` contra el componente anterior. Un `..`
/// que no tiene componente que retirar se conserva, para no ocultar que una
/// ruta escapa de su origen.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> PathBuf {
        normalize_path(Path::new(s))
    }

    #[test]
    fn drops_current_dir_components() {
        assert_eq!(n("./a/./b"), PathBuf::from("a/b"));
    }

    #[test]
    fn resolves_parent_against_the_previous_component() {
        assert_eq!(n("a/../b"), PathBuf::from("b"));
        assert_eq!(n("a/b/../../c"), PathBuf::from("c"));
    }

    #[test]
    fn keeps_a_parent_that_escapes_a_relative_origin() {
        assert_eq!(n("../a"), PathBuf::from("../a"));
        assert_eq!(n("a/../../b"), PathBuf::from("../b"));
    }

    #[test]
    fn an_absolute_path_cannot_climb_past_the_root_but_keeps_the_marker() {
        assert_eq!(n("/a/../b"), PathBuf::from("/b"));
        assert_eq!(n("/.."), PathBuf::from("/.."));
    }
}
