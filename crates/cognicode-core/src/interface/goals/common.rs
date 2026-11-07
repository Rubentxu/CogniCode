//! Las piezas que varios goals comparten.
//!
//! ## La cobertura tiene un dueño
//!
//! La cobertura del workspace la mide la lectura, y la lectura tiene un
//! dueño: el inventario de fuentes. Los goals que la publican la piden aquí,
//! así que el goal `status` y el goal `context` no pueden discrepar sobre lo
//! que la lectura dejó fuera. Un goal que publica su propia cuenta de lo que
//! no pudo leer es un goal que publica una segunda verdad.
//!
//! ## Lo que esta cobertura NO mide
//!
//! La cobertura de la **lectura**: los ficheros que no se pudieron leer. Las
//! referencias que el grafo no pudo atar a una declaración y las relaciones
//! que el repositorio rechazó las mide el grafo, y un goal que no lo
//! construye no puede publicarlas. Publicarlas vacías sería publicar una
//! cuenta que nadie hizo.

use std::path::Path;

use crate::application::services::analyze::{Coverage, CoverageFacts};
use crate::infrastructure::graph::source_files::inventory;
use crate::interface::coverage::CoverageInfo;

/// La cobertura de la lectura del workspace.
///
/// La mide el dueño de los agujeros y la publica el dueño de la cobertura:
/// la palabra (`complete`, `partial`) la dice
/// [`crate::application::services::analyze::Coverage`], y la forma la dice
/// [`crate::interface::coverage::CoverageInfo`]. Aquí sólo se junta lo que
/// el inventario leyó con lo que el dueño sabe decir de eso.
pub fn coverage_of(workspace: &Path) -> CoverageInfo {
    let facts = CoverageFacts {
        skipped: inventory(workspace).coverage_holes(),
        pending: Vec::new(),
        rejected: Vec::new(),
    };

    let coverage = if facts.is_complete() {
        Coverage::Complete
    } else {
        Coverage::Partial(facts)
    };

    CoverageInfo::of(&coverage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_that_can_be_read_is_complete() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");

        let coverage = coverage_of(dir.path());

        assert_eq!(coverage.status, "complete");
        assert!(coverage.skipped_files.is_empty());
    }

    #[test]
    fn the_coverage_says_what_the_read_could_not_do() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
        // Un directorio que no se puede leer no se puede leer en Unix
        // cuando no es el uid del propietario; aquí se simula con un
        // symlink, que la lectura declina a propósito.
        #[cfg(unix)]
        std::os::unix::fs::symlink("/nowhere", dir.path().join("link.rs")).expect("symlink");

        let coverage = coverage_of(dir.path());

        assert_eq!(coverage.status, "partial");
        assert!(
            coverage
                .skipped_files
                .iter()
                .any(|file| file.path.contains("link.rs")),
            "the declined symlink is a hole of the read: {coverage:?}"
        );
    }
}
