//! The render check of a chart that lives inside a revision.
//!
//! A revision's chart is a set of bytes, not a directory, and `helm
//! template` needs a directory. The adapter lays the chart's files in a
//! throwaway directory, reads it with the same chart reader every Helm
//! analysis uses, and analyses it with the same function the Helm pack
//! uses ([`analyze_chart`]), so what counts as a faithful render is decided
//! in one place.
//!
//! A chart counts as rendered only when the engine rendered it *and* the
//! analysis found nothing that makes the render partial (a value a template
//! reads but nothing sets, a dependency that is not vendored, a construct
//! whose render differs from a cluster's). Anything else is not rendered,
//! with the cause.

use std::fs;
use std::path::{Component, Path};

use crate::application::change_tracking::helm_templates::{
    ChartFiles, ChartRender, ChartRenderCheck,
};
use crate::domain::iac::helm::{ChartRenderer, RenderCoverage, RenderStatus, analyze_chart};

use super::{HelmCli, RELEASE_NAME, read_chart};

/// Renders the charts of a revision through `R` and reports whether each
/// was faithfully and completely rendered.
#[derive(Debug, Clone)]
pub struct HelmRevisionCharts<R: ChartRenderer = HelmCli> {
    renderer: R,
}

impl HelmRevisionCharts<HelmCli> {
    /// Use the `helm` binary on `PATH`. When there is none, every chart is
    /// honestly not rendered.
    pub fn from_path() -> Self {
        Self::with_renderer(HelmCli::from_path())
    }
}

impl<R: ChartRenderer> HelmRevisionCharts<R> {
    /// Use a specific renderer.
    pub fn with_renderer(renderer: R) -> Self {
        Self { renderer }
    }
}

impl<R: ChartRenderer> ChartRenderCheck for HelmRevisionCharts<R> {
    fn check(&self, chart: &ChartFiles) -> ChartRender {
        match render_cause(&self.renderer, chart) {
            None => ChartRender::Rendered,
            Some(cause) => ChartRender::NotRendered { cause },
        }
    }
}

/// Why `chart` was not completely rendered; `None` when it was.
fn render_cause(renderer: &dyn ChartRenderer, chart: &ChartFiles) -> Option<String> {
    let scratch = match tempfile::tempdir() {
        Ok(scratch) => scratch,
        Err(error) => return Some(format!("cannot create a scratch directory: {error}")),
    };
    if let Err(cause) = materialize(chart, scratch.path()) {
        return Some(cause);
    }
    let model = match read_chart(scratch.path()) {
        Ok(model) => model,
        Err(error) => return Some(format!("chart cannot be read: {error}")),
    };
    let analysis = analyze_chart(&model, &[], RELEASE_NAME, renderer);
    match (&analysis.render, &analysis.coverage) {
        (RenderStatus::NotRendered(cause), _) => Some(cause.describe()),
        (RenderStatus::Rendered { .. }, RenderCoverage::Complete) => None,
        (RenderStatus::Rendered { .. }, RenderCoverage::Partial { reasons })
        | (RenderStatus::Rendered { .. }, RenderCoverage::Unknown { reasons }) => Some(format!(
            "the render is partial: {}",
            reasons
                .iter()
                .map(|reason| reason.describe())
                .collect::<Vec<_>>()
                .join("; ")
        )),
    }
}

/// Writes the chart files below `dir`, refusing any path that is not a
/// plain relative path.
fn materialize(chart: &ChartFiles, dir: &Path) -> Result<(), String> {
    for (relative, bytes) in &chart.files {
        let path = Path::new(relative);
        if relative.is_empty()
            || !path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(format!(
                "chart file path `{relative}` is not a relative path"
            ));
        }
        let target = dir.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot lay out chart file `{relative}`: {error}"))?;
        }
        fs::write(&target, bytes)
            .map_err(|error| format!("cannot lay out chart file `{relative}`: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::iac::helm::{RenderRequest, RenderResponse};
    use std::collections::BTreeMap;

    /// A labelled test double for the engine: it answers what it was told
    /// and renders nothing itself.
    struct ScriptedEngine(RenderResponse);

    impl ChartRenderer for ScriptedEngine {
        fn render(&self, _request: &RenderRequest<'_>) -> RenderResponse {
            self.0.clone()
        }
    }

    fn chart(files: &[(&str, &str)]) -> ChartFiles {
        ChartFiles {
            root: "deploy/web".to_string(),
            files: files
                .iter()
                .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    fn simple() -> ChartFiles {
        chart(&[
            ("Chart.yaml", "apiVersion: v2\nname: web\nversion: 0.1.0\n"),
            ("values.yaml", "replicas: 2\n"),
            (
                "templates/deployment.yaml",
                "replicas: {{ .Values.replicas }}\n",
            ),
        ])
    }

    fn check(response: RenderResponse, chart: &ChartFiles) -> ChartRender {
        HelmRevisionCharts::with_renderer(ScriptedEngine(response)).check(chart)
    }

    #[test]
    fn a_complete_render_of_the_chart_is_rendered() {
        assert_eq!(
            check(RenderResponse::Rendered(Vec::new()), &simple()),
            ChartRender::Rendered
        );
    }

    #[test]
    fn no_engine_is_never_a_render() {
        let outcome = check(
            RenderResponse::Unavailable {
                reason: "`helm` binary not found on PATH".to_string(),
            },
            &simple(),
        );
        let ChartRender::NotRendered { cause } = outcome else {
            panic!("an unavailable engine rendered nothing");
        };
        assert!(cause.contains("engine unavailable"), "{cause}");
    }

    #[test]
    fn an_engine_failure_is_never_a_render() {
        let outcome = check(
            RenderResponse::Failed {
                message: "parse error".to_string(),
            },
            &simple(),
        );
        let ChartRender::NotRendered { cause } = outcome else {
            panic!("a failed engine rendered nothing");
        };
        assert!(cause.contains("engine failed: parse error"), "{cause}");
    }

    #[test]
    fn a_render_that_leaves_a_value_unset_is_partial_and_so_not_rendered() {
        let chart = chart(&[
            ("Chart.yaml", "apiVersion: v2\nname: web\nversion: 0.1.0\n"),
            ("values.yaml", "replicas: 2\n"),
            (
                "templates/ingress.yaml",
                "host: {{ .Values.ingress.host }}\n",
            ),
        ]);

        let outcome = check(RenderResponse::Rendered(Vec::new()), &chart);

        let ChartRender::NotRendered { cause } = outcome else {
            panic!("a partial render must not count as rendered");
        };
        assert!(cause.contains("partial"), "{cause}");
        assert!(cause.contains("ingress.host"), "{cause}");
    }

    #[test]
    fn a_chart_without_chart_yaml_cannot_be_read_and_so_is_not_rendered() {
        let chart = chart(&[("templates/a.yaml", "a: {{ .Values.a }}\n")]);

        let outcome = check(RenderResponse::Rendered(Vec::new()), &chart);

        let ChartRender::NotRendered { cause } = outcome else {
            panic!("an unreadable chart rendered nothing");
        };
        assert!(cause.contains("chart cannot be read"), "{cause}");
    }

    #[test]
    fn a_path_that_leaves_the_chart_is_refused_and_nothing_is_written() {
        for hostile in ["../escape.yaml", "/abs.yaml", "a/../../b.yaml", ""] {
            let outside = tempfile::tempdir().expect("dir");
            let inside = outside.path().join("chart");
            fs::create_dir(&inside).expect("chart dir");
            let chart = ChartFiles {
                root: String::new(),
                files: BTreeMap::from([(hostile.to_string(), b"x".to_vec())]),
            };

            let refused = materialize(&chart, &inside);

            assert!(refused.is_err(), "{hostile:?} must be refused");
            assert!(!outside.path().join("escape.yaml").exists());
        }
    }
}
