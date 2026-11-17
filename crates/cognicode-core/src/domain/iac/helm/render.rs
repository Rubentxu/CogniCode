//! The optional faithful render and what its absence means.
//!
//! The official engine is an *adapter* behind [`ChartRenderer`], never part of
//! the analysis core. Whatever the adapter answers is reduced to a
//! [`RenderStatus`]: objects exist only for `Rendered`; every other status is
//! an explicit statement that nothing was rendered and why.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::chart::ChartModel;
use super::manifest::RenderedDocument;

/// What the adapter is asked to render.
#[derive(Debug, Clone)]
pub struct RenderRequest<'a> {
    /// The chart.
    pub chart: &'a ChartModel,
    /// Value layers on top of the chart defaults, lowest priority first.
    pub overrides: &'a [Value],
    /// Release name handed to the engine.
    pub release_name: &'a str,
}

/// What the adapter answers.
#[derive(Debug, Clone, PartialEq)]
pub enum RenderResponse {
    /// The engine rendered the chart.
    Rendered(Vec<RenderedDocument>),
    /// No engine is available on this machine, or it was not allowed to run.
    Unavailable {
        /// Why (binary not found, disabled).
        reason: String,
    },
    /// The engine ran and refused or failed.
    Failed {
        /// Its diagnostic.
        message: String,
    },
}

/// The port to a faithful renderer. Implementations must not mutate the chart
/// or reach the network and must bound their own run time.
pub trait ChartRenderer {
    /// Render `request`, or say honestly why it was not rendered.
    fn render(&self, request: &RenderRequest<'_>) -> RenderResponse;
}

/// A renderer that is never available: the analysis core's default, so it
/// works with no Helm installed and says so.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRenderer;

impl ChartRenderer for NoRenderer {
    fn render(&self, _request: &RenderRequest<'_>) -> RenderResponse {
        RenderResponse::Unavailable {
            reason: "no chart renderer configured".to_string(),
        }
    }
}

/// Why nothing was rendered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cause", rename_all = "snake_case")]
pub enum NotRenderedCause {
    /// The engine is not available.
    EngineUnavailable {
        /// Why.
        reason: String,
    },
    /// The engine ran and failed.
    EngineFailed {
        /// Its diagnostic.
        message: String,
    },
}

/// Whether concrete objects exist for a chart and values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RenderStatus {
    /// The engine rendered the chart.
    Rendered {
        /// Number of documents in the rendered stream.
        documents: usize,
    },
    /// Nothing was rendered: no manifest exists, none is inferred.
    NotRendered(NotRenderedCause),
}

impl RenderStatus {
    /// Whether objects exist.
    pub fn is_rendered(&self) -> bool {
        matches!(self, Self::Rendered { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_default_renderer_is_honestly_unavailable() {
        let chart = ChartModel {
            name: "c".into(),
            version: "1".into(),
            location: PathBuf::new(),
            content_digest: String::new(),
            default_values: Value::Null,
            schema: None,
            dependencies: vec![],
            templates: vec![],
        };
        let request = RenderRequest {
            chart: &chart,
            overrides: &[],
            release_name: "r",
        };
        assert!(matches!(
            NoRenderer.render(&request),
            RenderResponse::Unavailable { .. }
        ));
    }
}
