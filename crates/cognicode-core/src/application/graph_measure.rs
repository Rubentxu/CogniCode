//! What was measured about a graph, and which graph it was measured for.
//!
//! A coverage is a statement about one graph. The service holds one graph
//! at a time and replaces it whenever a route rebuilds, reloads or edits
//! it; a coverage remembered on the side outlives the graph it described
//! unless something ties the two together. This is that tie: the measure
//! carries the revision of the graph it was taken for, and a reader that
//! has just read a graph asks whether the measure describes *that* graph,
//! not "the last thing somebody measured".
//!
//! The answer to "no" is "nobody measured it", which a reader publishes as
//! `unknown`. It is never promoted to `complete`.

use std::sync::Mutex;

use crate::application::analysis_basis::GraphSource;
use crate::application::services::analyze::Coverage;
use crate::domain::aggregates::CallGraph;

/// The coverage and the origin of one revision of the graph.
#[derive(Debug, Clone)]
pub struct GraphMeasure {
    /// The revision of the graph the service held when it was measured.
    revision: u64,
    /// The size of that graph: a second witness that the graph a reader
    /// holds is the one that was measured, for a reader that was handed a
    /// graph by something other than the service's own cache.
    symbols: usize,
    edges: usize,
    /// What the graph covers.
    pub coverage: Coverage,
    /// Where the graph came from.
    pub source: GraphSource,
    /// The canonical workspace the graph was read from.
    pub workspace: String,
    /// The digest of the source manifest the graph was read against, when
    /// one is known.
    pub source_manifest_digest: Option<String>,
}

impl GraphMeasure {
    /// The measure of `graph`, which the service holds at `revision`.
    pub fn new(
        revision: u64,
        graph: &CallGraph,
        coverage: Coverage,
        source: GraphSource,
        workspace: String,
        source_manifest_digest: Option<String>,
    ) -> Self {
        Self {
            revision,
            symbols: graph.symbol_count(),
            edges: graph.edge_count(),
            coverage,
            source,
            workspace,
            source_manifest_digest,
        }
    }

    /// Whether this measure was taken for the graph a reader holds, which
    /// the service held at `revision` when it was read.
    pub fn describes(&self, revision: u64, graph: &CallGraph) -> bool {
        self.revision == revision
            && self.symbols == graph.symbol_count()
            && self.edges == graph.edge_count()
    }
}

/// The measure of the graph the service holds: one at a time, because the
/// service holds one graph at a time.
#[derive(Debug, Default)]
pub struct MeasureCell(Mutex<Option<GraphMeasure>>);

impl MeasureCell {
    /// Keeps `measure` as the measure of the graph the service holds now.
    pub fn record(&self, measure: GraphMeasure) {
        *self.0.lock().unwrap() = Some(measure);
    }

    /// The coverage that was measured last, whatever graph it was for;
    /// `None` when nobody measured anything.
    pub fn last_coverage(&self) -> Option<Coverage> {
        self.0.lock().unwrap().as_ref().map(|m| m.coverage.clone())
    }

    /// What was measured about `graph`, which the service held at
    /// `revision` when a reader took it. `None` when nobody measured that
    /// graph: it is unmeasured, never complete.
    pub fn of(&self, revision: u64, graph: &CallGraph) -> Option<GraphMeasure> {
        let held = self.0.lock().unwrap();
        held.as_ref()
            .filter(|m| m.describes(revision, graph))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::{Location, SymbolKind};

    fn graph_of(names: &[&str]) -> CallGraph {
        let mut graph = CallGraph::new();
        for (line, name) in names.iter().enumerate() {
            graph.add_symbol(Symbol::new(
                *name,
                SymbolKind::Function,
                Location::new("lib.rs", line as u32, 0),
            ));
        }
        graph
    }

    fn measure_of(revision: u64, graph: &CallGraph) -> GraphMeasure {
        GraphMeasure::new(
            revision,
            graph,
            Coverage::Unmeasured,
            GraphSource::Built,
            "/w".to_string(),
            None,
        )
    }

    #[test]
    fn a_measure_describes_the_revision_and_the_graph_it_was_taken_for() {
        let graph = graph_of(&["a", "b"]);
        let measure = measure_of(3, &graph);
        assert!(measure.describes(3, &graph));
    }

    #[test]
    fn a_measure_does_not_describe_another_revision() {
        let graph = graph_of(&["a", "b"]);
        let measure = measure_of(3, &graph);
        assert!(!measure.describes(4, &graph));
    }

    #[test]
    fn a_measure_does_not_describe_a_graph_of_another_size() {
        let measure = measure_of(3, &graph_of(&["a", "b"]));
        assert!(!measure.describes(3, &graph_of(&["a", "b", "c"])));
    }
}
