//! Helm chart templates inside a revision, and the one honest thing the
//! facts can say about them.
//!
//! ## Why a template is not extracted
//!
//! A file under a chart's `templates/` ends in `.yaml`, but until the
//! engine renders it, it is Go-template text, not YAML: `{{ ... }}` is not
//! a mapping, so the YAML grammar either faults on it or, worse, parses a
//! lucky fragment into facts about a document that never exists. Neither
//! outcome describes the chart.
//!
//! So a chart template is classified before any grammar sees it, and is
//! decided by a *render check* ([`ChartRenderCheck`]) of the whole chart:
//!
//! - **Rendered**: the engine rendered the chart and nothing the analysis
//!   depends on is missing. The templates are covered by that render; they
//!   contribute no facts of their own, and a change to one is invisible to
//!   the fact delta, which the impact says.
//! - **Not rendered**: no engine, the engine failed, or the render is
//!   partial. Each template is a gap with its own cause
//!   ([`GapReason::TemplateNotRendered`](super::revision_facts::GapReason)),
//!   distinct from a syntax error, and the revision is never presented as
//!   complete.
//!
//! The default check ([`NoChartRender`]) never renders, so a producer that
//! was given no engine can never call a chart clean.

use std::collections::{BTreeMap, BTreeSet};

/// The files that make up one chart of a revision, relative to its root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartFiles {
    /// The chart root inside the revision; empty for the repository root.
    pub root: String,
    /// Chart files by path relative to the root.
    pub files: BTreeMap<String, Vec<u8>>,
}

impl ChartFiles {
    /// The chart root for display: `.` for the repository root.
    pub fn display_root(&self) -> &str {
        if self.root.is_empty() {
            "."
        } else {
            &self.root
        }
    }
}

/// What a render check concluded about one chart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChartRender {
    /// The chart was faithfully and completely rendered.
    Rendered,
    /// The chart was not (completely) rendered.
    NotRendered {
        /// Why, in one line.
        cause: String,
    },
}

/// Renders a chart of a revision, or says honestly that it did not.
///
/// An implementation must answer `Rendered` only for a real render of
/// exactly these files that left nothing the analysis relies on unknown;
/// everything else is `NotRendered`.
pub trait ChartRenderCheck {
    /// Check `chart`.
    fn check(&self, chart: &ChartFiles) -> ChartRender;
}

/// The check of a producer given no engine: nothing is ever rendered.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoChartRender;

impl ChartRenderCheck for NoChartRender {
    fn check(&self, _chart: &ChartFiles) -> ChartRender {
        ChartRender::NotRendered {
            cause: "no chart renderer configured".to_string(),
        }
    }
}

/// The roots of the charts among `paths`: the directory of every
/// `Chart.yaml`, empty for the repository root.
pub(super) fn chart_roots<'a>(paths: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    paths
        .into_iter()
        .filter_map(|path| match path.rsplit_once('/') {
            Some((dir, "Chart.yaml")) => Some(dir.to_string()),
            None if path == "Chart.yaml" => Some(String::new()),
            _ => None,
        })
        .collect()
}

/// The chart whose `templates/` directory holds `path`, when it is one.
pub(super) fn template_chart_root(path: &str, roots: &BTreeSet<String>) -> Option<String> {
    let mut offset = 0;
    for component in path.split('/') {
        if component == "templates" && offset < path.len() {
            let root = path[..offset].trim_end_matches('/');
            if roots.contains(root) && path.len() > offset + "templates/".len() {
                return Some(root.to_string());
            }
        }
        offset += component.len() + 1;
    }
    None
}

/// The path of `path` below `root`, when it is inside it.
pub(super) fn below_root<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    if root.is_empty() {
        Some(path)
    } else {
        path.strip_prefix(root)?.strip_prefix('/')
    }
}

/// Whether `relative` is a file the engine reads to render a chart.
pub(super) fn is_chart_file(relative: &str) -> bool {
    matches!(
        relative,
        "Chart.yaml" | "Chart.lock" | "values.yaml" | "values.schema.json" | ".helmignore"
    ) || ["templates/", "charts/", "crds/"]
        .iter()
        .any(|dir| relative.starts_with(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(paths: &[&str]) -> BTreeSet<String> {
        chart_roots(paths.iter().copied())
    }

    #[test]
    fn chart_roots_are_the_directories_of_chart_yaml() {
        assert_eq!(
            roots(&["Chart.yaml", "deploy/web/Chart.yaml", "src/lib.rs"]),
            BTreeSet::from([String::new(), "deploy/web".to_string()])
        );
    }

    #[test]
    fn a_template_belongs_to_the_chart_that_owns_its_templates_directory() {
        let set = roots(&["deploy/web/Chart.yaml"]);
        assert_eq!(
            template_chart_root("deploy/web/templates/svc.yaml", &set),
            Some("deploy/web".to_string())
        );
        assert_eq!(
            template_chart_root("deploy/web/templates/tests/t.yaml", &set),
            Some("deploy/web".to_string())
        );
    }

    #[test]
    fn a_subchart_template_belongs_to_the_subchart() {
        let set = roots(&["Chart.yaml", "charts/db/Chart.yaml"]);
        assert_eq!(
            template_chart_root("charts/db/templates/s.yaml", &set),
            Some("charts/db".to_string())
        );
        assert_eq!(
            template_chart_root("templates/s.yaml", &set),
            Some(String::new())
        );
    }

    #[test]
    fn a_templates_directory_outside_any_chart_is_not_a_chart_template() {
        // Plain YAML that merely lives in a directory called `templates`.
        let set = roots(&["deploy/web/Chart.yaml"]);
        assert_eq!(template_chart_root("docs/templates/a.yaml", &set), None);
        assert_eq!(template_chart_root("deploy/web/values.yaml", &set), None);
        assert_eq!(template_chart_root("deploy/web/templates", &set), None);
    }

    #[test]
    fn below_root_respects_directory_boundaries() {
        assert_eq!(below_root("a/b/c.yaml", "a/b"), Some("c.yaml"));
        assert_eq!(below_root("a/bc/c.yaml", "a/b"), None);
        assert_eq!(below_root("c.yaml", ""), Some("c.yaml"));
    }

    #[test]
    fn only_chart_inputs_are_chart_files() {
        for yes in [
            "Chart.yaml",
            "values.yaml",
            "values.schema.json",
            "templates/a.yaml",
            "charts/db/Chart.yaml",
            "crds/x.yaml",
        ] {
            assert!(is_chart_file(yes), "{yes}");
        }
        for no in ["src/main.rs", "README.md", "ci/values.yaml"] {
            assert!(!is_chart_file(no), "{no}");
        }
    }

    #[test]
    fn the_default_check_never_renders() {
        let chart = ChartFiles {
            root: String::new(),
            files: BTreeMap::new(),
        };
        assert!(matches!(
            NoChartRender.check(&chart),
            ChartRender::NotRendered { .. }
        ));
    }
}
