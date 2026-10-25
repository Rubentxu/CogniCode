//! Metricas del grafo: cuanto cubre y que se dejo fuera.
//!
//! Las estadisticas se calculan sobre el grafo construido; la cobertura y el informe de construccion son los de la ultima construccion.

#![allow(
    clippy::doc_overindented_list_items,
    clippy::len_zero,
    clippy::type_complexity,
    clippy::useless_vec,
    unused_comparisons
)]

use super::AnalysisService;
use crate::application::dto::{GraphCoverageMetrics, GraphStatsDto};

impl AnalysisService {
    /// Returns statistics about the call graph, including coverage metrics
    ///
    /// Coverage metrics are only available after `build_project_graph` or
    /// `build_project_graph_async` has been called.
    pub fn get_graph_stats(&self) -> GraphStatsDto {
        use crate::infrastructure::parser::Language;
        use std::collections::HashMap;

        let graph = self.graph_cache.get();
        let symbol_count = graph.symbol_count();
        let edge_count = graph.edge_count();

        // Count unique files and compute language breakdown
        let mut unique_files: HashMap<String, bool> = HashMap::new();
        let mut language_breakdown: HashMap<String, usize> = HashMap::new();

        for symbol in graph.symbols() {
            let file = symbol.location().file().to_string();
            unique_files.insert(file.clone(), true);

            // Compute language from file extension
            let ext = std::path::Path::new(&file).extension();
            if let Some(lang) = Language::from_extension(ext) {
                *language_breakdown
                    .entry(lang.name().to_string())
                    .or_insert(0) += 1;
            } else {
                *language_breakdown.entry("Unknown".to_string()).or_insert(0) += 1;
            }
        }

        // Get coverage metrics
        let coverage = self.coverage_metrics.lock().unwrap().clone();

        GraphStatsDto {
            symbol_count,
            edge_count,
            file_count: unique_files.len(),
            language_breakdown,
            coverage,
        }
    }

    /// Returns coverage metrics from the last graph build.
    pub fn get_coverage_metrics(&self) -> Option<GraphCoverageMetrics> {
        self.coverage_metrics.lock().unwrap().clone()
    }

    /// F2.W8: returns the [`BuildReport`] from the last graph build,
    /// or `None` if no build has run yet on this service instance.
    ///
    /// The report carries a [`BuildStatus`] that is either `Complete`
    /// (every file in the walk was processed) or `Partial { skipped }`
    /// (one or more files were dropped — see [`SkippedFile`] for the
    /// per-file reason and the [`SkipReason`] variants).
    ///
    /// [`BuildReport`]: crate::infrastructure::graph::per_file_graph::BuildReport
    /// [`BuildStatus`]: crate::infrastructure::graph::per_file_graph::BuildStatus
    /// [`SkippedFile`]: crate::infrastructure::graph::per_file_graph::SkippedFile
    /// [`SkipReason`]: crate::infrastructure::graph::per_file_graph::SkipReason
    pub fn get_last_build_report(
        &self,
    ) -> Option<crate::infrastructure::graph::per_file_graph::BuildReport> {
        self.last_build_report.lock().unwrap().clone()
    }
}
