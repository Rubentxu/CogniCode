//! The chart as a static model: templates, default values, schema and
//! dependencies.
//!
//! The model is what the chart *declares*. It never contains rendered
//! objects; those exist only in a [`ManifestModel`](super::ManifestModel)
//! produced by a real render. What the model can do on its own is say which
//! values a template reads that nobody supplied, so a render can be reported
//! as partial before and after it runs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::template::{Construct, TemplateModel, is_manifest_template};
use super::values::lookup;

/// A declared chart dependency (`Chart.yaml` `dependencies`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartDependency {
    /// Chart name.
    pub name: String,
    /// Version constraint as written.
    pub version: String,
    /// Repository URL or `file://` path, when set.
    pub repository: Option<String>,
    /// Name the dependency answers to in the parent's values, when aliased.
    pub alias: Option<String>,
    /// Whether the dependency is present under `charts/`.
    pub vendored: bool,
}

impl ChartDependency {
    /// The key under which the parent's values configure the dependency.
    pub fn values_key(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

/// The chart's `values.schema.json`, reduced to what the analysis checks.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ValuesSchema {
    /// Dotted paths the schema lists under `required`.
    pub required_paths: Vec<String>,
}

/// The static model of one chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartModel {
    /// Chart name.
    pub name: String,
    /// Chart version.
    pub version: String,
    /// Where the chart was read from.
    pub location: PathBuf,
    /// Digest of the chart's files, so a model names the bytes it came from.
    pub content_digest: String,
    /// Default `values.yaml`.
    pub default_values: Value,
    /// Schema, when the chart ships one.
    pub schema: Option<ValuesSchema>,
    /// Dependencies.
    pub dependencies: Vec<ChartDependency>,
    /// Files below `templates/`.
    pub templates: Vec<TemplateModel>,
}

/// A value a template reads that is not set.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ValueGap {
    /// The unset path.
    pub path: String,
    /// The template that reads it.
    pub template: String,
    /// Line of the read.
    pub line: u32,
    /// The read is wrapped by `required`: the render fails without it.
    pub required: bool,
}

/// Why a result is not complete.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum PartialReason {
    /// A template reads a value that neither the defaults nor the supplied
    /// layers set, outside any condition or default.
    UnsetValue(ValueGap),
    /// The schema requires a value that is not set.
    SchemaRequiredMissing {
        /// The required path.
        path: String,
    },
    /// A template uses a construct that makes a render differ from the one a
    /// cluster or a second run would give.
    RenderFidelity {
        /// The template.
        template: String,
        /// What the construct is.
        construct: String,
    },
    /// A declared dependency is not vendored under `charts/`.
    DependencyMissing {
        /// The dependency name.
        name: String,
    },
}

impl PartialReason {
    /// One-line explanation for reports.
    pub fn describe(&self) -> String {
        match self {
            Self::UnsetValue(gap) => format!(
                "{}:{} reads unset value `.Values.{}`{}",
                gap.template,
                gap.line,
                gap.path,
                if gap.required { " (required)" } else { "" }
            ),
            Self::SchemaRequiredMissing { path } => {
                format!("schema requires `{path}` but it is not set")
            }
            Self::RenderFidelity {
                template,
                construct,
            } => format!("{template} {construct}"),
            Self::DependencyMissing { name } => {
                format!("dependency `{name}` is not vendored under charts/")
            }
        }
    }
}

impl ChartModel {
    /// Templates the engine renders into manifests.
    pub fn manifest_templates(&self) -> impl Iterator<Item = &TemplateModel> {
        self.templates
            .iter()
            .filter(|t| is_manifest_template(&t.path))
    }

    /// Reasons a result for `effective` values cannot be complete, determined
    /// from the chart and the values alone.
    pub fn partial_reasons(&self, effective: &Value) -> Vec<PartialReason> {
        let mut reasons = Vec::new();
        for template in self.manifest_templates() {
            for read in &template.value_refs {
                if !read.guarded && lookup(effective, &read.path).is_none() {
                    reasons.push(PartialReason::UnsetValue(ValueGap {
                        path: read.path.clone(),
                        template: template.path.clone(),
                        line: read.line,
                        required: read.required,
                    }));
                }
            }
            for construct in template.fidelity_limits() {
                reasons.push(PartialReason::RenderFidelity {
                    template: template.path.clone(),
                    construct: construct.describe(),
                });
            }
        }
        for path in self.schema.iter().flat_map(|s| &s.required_paths) {
            if lookup(effective, path).is_none() {
                reasons.push(PartialReason::SchemaRequiredMissing { path: path.clone() });
            }
        }
        for dependency in self.dependencies.iter().filter(|d| !d.vendored) {
            reasons.push(PartialReason::DependencyMissing {
                name: dependency.name.clone(),
            });
        }
        reasons.sort();
        reasons.dedup();
        reasons
    }

    /// Constructs used anywhere in the chart's manifest templates that the
    /// analysis does not evaluate (reported as limits, not failures).
    pub fn unevaluated_constructs(&self) -> Vec<(String, Construct)> {
        self.manifest_templates()
            .flat_map(|t| {
                t.constructs
                    .iter()
                    .filter(|c| matches!(c, Construct::Include(_) | Construct::WholeValues))
                    .map(|c| (t.path.clone(), c.clone()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chart(templates: Vec<TemplateModel>) -> ChartModel {
        ChartModel {
            name: "demo".into(),
            version: "0.1.0".into(),
            location: PathBuf::from("demo"),
            content_digest: "sha256:c".into(),
            default_values: json!({}),
            schema: None,
            dependencies: Vec::new(),
            templates,
        }
    }

    fn template(source: &str) -> TemplateModel {
        TemplateModel::analyze("templates/d.yaml", source)
    }

    #[test]
    fn an_unguarded_read_of_an_unset_value_is_a_reason_and_a_set_one_is_not() {
        let c = chart(vec![template(
            "r: {{ .Values.replicas }}\ni: {{ .Values.image }}",
        )]);
        let reasons = c.partial_reasons(&json!({"replicas": 2}));
        assert_eq!(reasons.len(), 1);
        assert!(matches!(&reasons[0], PartialReason::UnsetValue(g) if g.path == "image"));
    }

    #[test]
    fn a_guarded_read_of_an_unset_value_is_a_legitimate_branch() {
        let c = chart(vec![template(
            "{{- if .Values.ingress.enabled }}x{{- end }}",
        )]);
        assert!(c.partial_reasons(&json!({})).is_empty());
    }

    #[test]
    fn schema_required_dependencies_and_fidelity_limits_all_count() {
        let mut c = chart(vec![template(r#"{{ lookup "v1" "Secret" "n" "s" }}"#)]);
        c.schema = Some(ValuesSchema {
            required_paths: vec!["auth.user".into()],
        });
        c.dependencies.push(ChartDependency {
            name: "redis".into(),
            version: "1.x".into(),
            repository: None,
            alias: None,
            vendored: false,
        });
        let reasons = c.partial_reasons(&json!({}));
        assert_eq!(reasons.len(), 3);
        assert!(
            reasons
                .iter()
                .any(|r| matches!(r, PartialReason::SchemaRequiredMissing { .. }))
        );
        assert!(
            reasons
                .iter()
                .any(|r| matches!(r, PartialReason::DependencyMissing { .. }))
        );
        assert!(
            reasons
                .iter()
                .any(|r| matches!(r, PartialReason::RenderFidelity { .. }))
        );
    }

    #[test]
    fn helper_templates_do_not_contribute_reasons() {
        let helper = TemplateModel::analyze("templates/_helpers.tpl", "{{ .Values.unset }}");
        assert!(chart(vec![helper]).partial_reasons(&json!({})).is_empty());
    }
}
