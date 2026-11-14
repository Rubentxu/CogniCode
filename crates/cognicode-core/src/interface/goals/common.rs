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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::application::graph_snapshot::source_digest;
use crate::application::services::analyze::{Coverage, CoverageFacts};
use crate::infrastructure::graph::source_files::{SourceInventory, inventory};
use crate::interface::coverage::CoverageInfo;
use crate::interface::project_graph::ProjectGraph;

/// What a goal that loaded the project's graph says it was built from.
///
/// The envelope reads it as the freshness of the answer: the three fields
/// are the ones it looks for (`basis.workspace`, `basis.source_manifest_digest`,
/// `basis.graph_source`), and the words are the ones the MCP publishes for the
/// same facts. A goal that loads the graph and drops this leaves the consumer
/// with an answer it cannot date.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct GoalBasis {
    /// The canonical workspace the graph was built from.
    pub workspace: String,
    /// The digest of the sources the graph describes: the same files with the
    /// same contents give the same digest, and an edited file changes it.
    pub source_manifest_digest: String,
    /// Where the graph came from: `built`, `cache` or `daemon`.
    pub graph_source: String,
}

/// The basis of an answer computed over `built`.
///
/// `None` when the load kept no manifest: there is no digest to publish, and
/// an empty one would read as a measured identity. The coverage travels
/// separately and does not depend on this.
pub fn graph_basis(workspace: &Path, built: &ProjectGraph) -> Option<GoalBasis> {
    let manifest = built.manifest.as_ref()?;
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    Some(GoalBasis {
        workspace: workspace.to_string_lossy().to_string(),
        source_manifest_digest: source_digest(manifest),
        graph_source: built.source.published_label().to_string(),
    })
}

/// The line of text that says what an answer covered.
///
/// A goal that loads the graph can be partial for three reasons —files the
/// walk could not read, references without a declaration, relations the
/// repository refused— and the line names each one that applies. Only counts:
/// the detail is in the JSON, and a graph can have tens of thousands of
/// references without a declaration.
pub fn coverage_line(coverage: &CoverageInfo) -> String {
    let mut holes = Vec::new();
    if !coverage.skipped_files.is_empty() {
        holes.push(format!(
            "{} file(s) unreadable",
            coverage.skipped_files.len()
        ));
    }
    if !coverage.pending_references.is_empty() {
        holes.push(format!(
            "{} reference(s) without a declaration",
            coverage.pending_references.len()
        ));
    }
    if !coverage.rejected_relations.is_empty() {
        holes.push(format!(
            "{} relation(s) refused",
            coverage.rejected_relations.len()
        ));
    }
    if holes.is_empty() {
        format!("  coverage: {}\n", coverage.status)
    } else {
        format!("  coverage: {} ({})\n", coverage.status, holes.join(", "))
    }
}

/// La cobertura de la lectura del workspace.
///
/// La mide el dueño de los agujeros y la publica el dueño de la cobertura:
/// la palabra (`complete`, `partial`) la dice
/// [`crate::application::services::analyze::Coverage`], y la forma la dice
/// [`crate::interface::coverage::CoverageInfo`]. Aquí sólo se junta lo que
/// el inventario leyó con lo que el dueño sabe decir de eso.
pub fn coverage_of(workspace: &Path) -> CoverageInfo {
    coverage_of_inventory(&inventory(workspace))
}

/// La cobertura de una lectura que ya recorrió el árbol.
///
/// El recorrido es la mitad cara de la lectura: un goal que ya tiene el
/// inventario no vuelve a recorrer el árbol para saber lo que no pudo leer.
/// La palabra de la cobertura la sigue diciendo el mismo dueño.
pub fn coverage_of_inventory(inventory: &SourceInventory) -> CoverageInfo {
    let facts = CoverageFacts {
        skipped: inventory.coverage_holes(),
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

    fn coverage_with(holes: serde_json::Value) -> CoverageInfo {
        let mut value = serde_json::json!({
            "status": "partial",
            "skipped_files": [],
            "pending_references": [],
            "rejected_relations": [],
        });
        for (name, list) in holes.as_object().expect("an object of holes") {
            value[name] = list.clone();
        }
        serde_json::from_value(value).expect("a coverage the type describes")
    }

    #[test]
    fn a_complete_coverage_says_only_its_word() {
        let complete = CoverageInfo {
            status: "complete".to_string(),
            ..coverage_with(serde_json::json!({}))
        };

        assert_eq!(coverage_line(&complete), "  coverage: complete\n");
    }

    #[test]
    fn the_line_names_each_kind_of_hole_with_its_count() {
        let coverage = coverage_with(serde_json::json!({
            "skipped_files": [
                { "path": "a.rs", "reason": "unreadable" },
                { "path": "b.rs", "reason": "unreadable" },
            ],
            "pending_references": [
                { "file": "a.rs", "line": 3, "target": "x", "kind": "calls",
                  "reason": "unresolved", "candidates": null },
            ],
            "rejected_relations": [
                { "source": "a", "target": "b", "kind": "calls", "reason": "no such symbol" },
            ],
        }));

        assert_eq!(
            coverage_line(&coverage),
            "  coverage: partial (2 file(s) unreadable, 1 reference(s) without a declaration, \
             1 relation(s) refused)\n"
        );
    }

    #[test]
    fn a_hole_of_one_kind_does_not_name_the_others() {
        let coverage = coverage_with(serde_json::json!({
            "skipped_files": [{ "path": "a.rs", "reason": "unreadable" }],
        }));

        assert_eq!(
            coverage_line(&coverage),
            "  coverage: partial (1 file(s) unreadable)\n"
        );
    }

    #[test]
    fn the_basis_says_where_the_graph_came_from_and_which_sources_it_describes() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
        let (_service, built) = crate::interface::project_graph::service_with_graph(dir.path())
            .expect("the graph of a readable tree");

        let basis = graph_basis(dir.path(), &built).expect("a complete build keeps its manifest");

        assert_eq!(
            basis.workspace,
            dir.path()
                .canonicalize()
                .expect("canonical")
                .to_string_lossy()
        );
        assert_eq!(
            basis.graph_source,
            built.source.published_label(),
            "the word is the one the graph's own source publishes"
        );
        assert_eq!(
            basis.source_manifest_digest,
            source_digest(built.manifest.as_ref().expect("the manifest")),
            "the digest is the one the load's manifest has"
        );
    }

    #[test]
    fn without_a_manifest_there_is_no_basis_and_no_empty_digest() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
        let (_service, mut built) = crate::interface::project_graph::service_with_graph(dir.path())
            .expect("the graph of a readable tree");
        built.manifest = None;

        assert_eq!(graph_basis(dir.path(), &built), None);
    }
}
