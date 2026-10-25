//! Analysis Service - Handles code analysis operations
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(
    clippy::doc_overindented_list_items,
    clippy::len_zero,
    clippy::type_complexity,
    clippy::useless_vec,
    unused_comparisons
)]

use crate::application::dto::GraphCoverageMetrics;
use crate::application::ports::SharedGraph;
use crate::domain::aggregates::CallGraph;
use crate::domain::services::{ComplexityCalculator, CycleDetector, ImpactAnalyzer};
use crate::infrastructure::graph::{GraphCache, LightweightIndex};
use std::sync::{Arc, Mutex};

/// Service for analyzing code structure and complexity
pub struct AnalysisService {
    complexity_calculator: ComplexityCalculator,
    cycle_detector: CycleDetector,
    impact_analyzer: ImpactAnalyzer,
    graph_cache: Arc<dyn SharedGraph>,
    symbol_index: Mutex<Option<LightweightIndex>>,
    /// File cache: maps file path to (mtime, size, content_hash,
    /// facts) — F2.W9 + PRF audit H-01. The
    /// content_hash field is SHA-256 of the file bytes at parse time,
    /// added in `5ed7f865` (H-01 RED pin) GREEN. This invalidates the
    /// cache when bytes change while mtime AND size stay the same.
    /// Uses Arc<Mutex<...>> to support Send across thread boundaries for async operations
    file_cache: Arc<
        Mutex<
            std::collections::HashMap<
                String,
                (
                    u64,
                    u64,
                    [u8; 32],
                    crate::application::ingest::types::FileFacts,
                ),
            >,
        >,
    >,
    /// Coverage metrics from the last graph build
    coverage_metrics: Mutex<Option<GraphCoverageMetrics>>,
    /// F2.W8: skipped-file report from the last graph build. None
    /// until the first successful `build_project_graph` /
    /// `build_project_graph_filtered` / `build_graph_per_file` /
    /// `build_graph_with_strategy` call.
    last_build_report: Mutex<Option<crate::infrastructure::graph::per_file_graph::BuildReport>>,
}

impl AnalysisService {
    /// Creates a new AnalysisService
    pub fn new() -> Self {
        Self {
            complexity_calculator: ComplexityCalculator::new(),
            cycle_detector: CycleDetector::new(),
            impact_analyzer: ImpactAnalyzer::new(),
            graph_cache: Arc::new(GraphCache::new()),
            symbol_index: Mutex::new(None),
            file_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            coverage_metrics: Mutex::new(None),
            last_build_report: Mutex::new(None),
        }
    }

    /// Creates a new AnalysisService with a shared GraphCache
    ///
    /// This allows multiple services (e.g., WorkspaceSession and HandlerContext)
    /// to share the same graph cache, preventing duplicate builds.
    pub fn with_graph_cache(cache: Arc<dyn SharedGraph>) -> Self {
        Self {
            complexity_calculator: ComplexityCalculator::new(),
            cycle_detector: CycleDetector::new(),
            impact_analyzer: ImpactAnalyzer::new(),
            graph_cache: cache,
            symbol_index: Mutex::new(None),
            file_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            coverage_metrics: Mutex::new(None),
            last_build_report: Mutex::new(None),
        }
    }

    /// Returns the graph cache for accessing the project graph
    pub fn graph_cache(&self) -> Arc<dyn SharedGraph> {
        self.graph_cache.clone()
    }

    /// Returns the project graph from cache
    pub fn get_project_graph(&self) -> Arc<CallGraph> {
        self.graph_cache.get()
    }
}

impl Default for AnalysisService {
    fn default() -> Self {
        Self::new()
    }
}

mod analysis;
mod cache;
mod construction;
mod metrics;
mod resolution;

#[cfg(test)]
mod find_symbol_usages_tests;
#[cfg(test)]
mod reader_failure_tests;
#[cfg(test)]
mod tests;

pub use resolution::{ContextData, UsageResult, UsageSearchParams};
