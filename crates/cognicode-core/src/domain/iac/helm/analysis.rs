//! One chart analysed under one set of values, and the delta between two.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::chart::{ChartModel, PartialReason};
use super::checks::{HelmCheck, HelmFinding, check_manifest};
use super::manifest::{ManifestModel, ObjectId};
use super::render::{ChartRenderer, NotRenderedCause, RenderRequest, RenderResponse, RenderStatus};
use super::values::{effective_values, values_digest};

/// How much of the chart the result covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "coverage", rename_all = "snake_case")]
pub enum Coverage {
    /// Rendered, and nothing the analysis knows of leaves a part undecided.
    Complete,
    /// Rendered, but part of the chart or values is undecided.
    Partial {
        /// What is undecided.
        reasons: Vec<PartialReason>,
    },
    /// Nothing was rendered: nothing is known about the objects.
    Unknown {
        /// What the static model can say is also undecided.
        reasons: Vec<PartialReason>,
    },
}

/// The verdict of one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum CheckVerdict {
    /// The objects show at least one violation.
    Found {
        /// Number of findings.
        findings: usize,
    },
    /// Nothing was found, within a complete render.
    NotObserved {
        /// What was examined.
        within: String,
    },
    /// The check could not decide: not rendered, or a partial render.
    Unknown {
        /// Why.
        reasons: Vec<String>,
    },
}

/// A check with its verdict and findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    /// The check.
    pub check: HelmCheck,
    /// Its verdict.
    pub verdict: CheckVerdict,
    /// Findings; empty unless the verdict is `Found`.
    pub findings: Vec<HelmFinding>,
}

/// A chart under one set of values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartAnalysis {
    /// Chart name.
    pub chart: String,
    /// Chart version.
    pub chart_version: String,
    /// Digest of the effective values.
    pub values_digest: String,
    /// Whether a render exists.
    pub render: RenderStatus,
    /// Coverage of the result.
    pub coverage: Coverage,
    /// The concrete objects; `None` exactly when nothing was rendered.
    pub manifest: Option<ManifestModel>,
    /// One result per check.
    pub results: Vec<CheckResult>,
    /// What this analysis does not cover regardless of coverage.
    pub limits: Vec<String>,
}

impl ChartAnalysis {
    /// Every finding of every check.
    pub fn findings(&self) -> impl Iterator<Item = &HelmFinding> {
        self.results.iter().flat_map(|r| r.findings.iter())
    }

    /// The result of `check`.
    pub fn result(&self, check: HelmCheck) -> Option<&CheckResult> {
        self.results.iter().find(|r| r.check == check)
    }
}

/// Analyse `chart` with `overrides` layered on its defaults, rendering through
/// `renderer`. Without a usable renderer the result says so and contains no
/// objects.
pub fn analyze_chart(
    chart: &ChartModel,
    overrides: &[Value],
    release_name: &str,
    renderer: &dyn ChartRenderer,
) -> ChartAnalysis {
    let effective = effective_values(&chart.default_values, overrides);
    let digest = values_digest(&effective);
    let reasons = chart.partial_reasons(&effective);
    let response = renderer.render(&RenderRequest {
        chart,
        overrides,
        release_name,
    });
    let (render, manifest) = match response {
        RenderResponse::Rendered(documents) => {
            let model =
                ManifestModel::from_documents(&documents, &chart.name, &chart.version, &digest);
            (
                RenderStatus::Rendered {
                    documents: documents.len(),
                },
                Some(model),
            )
        }
        RenderResponse::Unavailable { reason } => (
            RenderStatus::NotRendered(NotRenderedCause::EngineUnavailable { reason }),
            None,
        ),
        RenderResponse::Failed { message } => (
            RenderStatus::NotRendered(NotRenderedCause::EngineFailed { message }),
            None,
        ),
    };
    let coverage = match (&manifest, reasons.is_empty()) {
        (None, _) => Coverage::Unknown { reasons },
        (Some(_), true) => Coverage::Complete,
        (Some(_), false) => Coverage::Partial { reasons },
    };
    let results = results_for(&render, &coverage, manifest.as_ref());
    let limits = limits_for(manifest.as_ref());
    ChartAnalysis {
        chart: chart.name.clone(),
        chart_version: chart.version.clone(),
        values_digest: digest,
        render,
        coverage,
        manifest,
        results,
        limits,
    }
}

fn results_for(
    render: &RenderStatus,
    coverage: &Coverage,
    manifest: Option<&ManifestModel>,
) -> Vec<CheckResult> {
    let findings = manifest.map(check_manifest).unwrap_or_default();
    HelmCheck::ALL
        .iter()
        .map(|&check| {
            let own: Vec<HelmFinding> = findings
                .iter()
                .filter(|f| f.check == check)
                .cloned()
                .collect();
            let verdict = verdict_for(render, coverage, manifest, &own);
            CheckResult {
                check,
                verdict,
                findings: own,
            }
        })
        .collect()
}

fn verdict_for(
    render: &RenderStatus,
    coverage: &Coverage,
    manifest: Option<&ManifestModel>,
    findings: &[HelmFinding],
) -> CheckVerdict {
    if !findings.is_empty() {
        return CheckVerdict::Found {
            findings: findings.len(),
        };
    }
    match (render, coverage, manifest) {
        (RenderStatus::NotRendered(cause), _, _) => CheckVerdict::Unknown {
            reasons: vec![format!(
                "not evaluated, chart was not rendered: {}",
                describe(cause)
            )],
        },
        (_, Coverage::Partial { reasons } | Coverage::Unknown { reasons }, _) => {
            CheckVerdict::Unknown {
                reasons: reasons.iter().map(PartialReason::describe).collect(),
            }
        }
        (_, Coverage::Complete, Some(manifest)) => CheckVerdict::NotObserved {
            within: format!("{} rendered object(s)", manifest.objects.len()),
        },
        (_, Coverage::Complete, None) => CheckVerdict::Unknown {
            reasons: vec!["no manifest available".to_string()],
        },
    }
}

fn describe(cause: &NotRenderedCause) -> String {
    match cause {
        NotRenderedCause::EngineUnavailable { reason } => format!("engine unavailable ({reason})"),
        NotRenderedCause::EngineFailed { message } => format!("engine failed ({message})"),
    }
}

fn limits_for(manifest: Option<&ManifestModel>) -> Vec<String> {
    let mut limits = vec![
        "template text is never read as manifests: objects exist only when an engine rendered them"
            .to_string(),
        "only workloads, RBAC and service/ingress exposure are modeled".to_string(),
    ];
    if let Some(manifest) = manifest {
        let kinds = manifest.unmodeled_kinds();
        if !kinds.is_empty() {
            limits.push(format!(
                "kinds without a security model: {}",
                kinds.join(", ")
            ));
        }
        if !manifest.skipped.is_empty() {
            limits.push(format!(
                "{} rendered document(s) were not objects and were skipped",
                manifest.skipped.len()
            ));
        }
    }
    limits
}

/// One changed fact of an object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactChange {
    /// The fact path (`containers[app].privileged`).
    pub path: String,
    /// Value before; `None` when the fact did not exist.
    pub before: Option<String>,
    /// Value after; `None` when the fact no longer exists.
    pub after: Option<String>,
}

/// The changed facts of one object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectChange {
    /// The object.
    pub object: ObjectId,
    /// Changed facts.
    pub changes: Vec<FactChange>,
}

/// The difference between two renders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparedDelta {
    /// Objects only in the second render.
    pub added: Vec<ObjectId>,
    /// Objects only in the first render.
    pub removed: Vec<ObjectId>,
    /// Objects in both whose facts differ.
    pub changed: Vec<ObjectChange>,
    /// Findings only in the second render.
    pub findings_introduced: Vec<HelmFinding>,
    /// Findings only in the first render.
    pub findings_resolved: Vec<HelmFinding>,
    /// Caveats: a delta over partial renders only covers what rendered.
    pub caveats: Vec<String>,
}

impl ComparedDelta {
    /// Whether the two renders are identical in every modeled fact.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.findings_introduced.is_empty()
            && self.findings_resolved.is_empty()
    }
}

/// A comparison, or the statement that none is possible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "comparison", rename_all = "snake_case")]
pub enum ChartDelta {
    /// Both sides have objects.
    Compared(ComparedDelta),
    /// At least one side was not rendered: no delta is inferred.
    NotComparable {
        /// Why.
        reason: String,
    },
}

/// Compare two analyses of the same chart under different values.
pub fn compare(before: &ChartAnalysis, after: &ChartAnalysis) -> ChartDelta {
    let (Some(old), Some(new)) = (&before.manifest, &after.manifest) else {
        return ChartDelta::NotComparable {
            reason: "at least one side was not rendered, so no objects exist to compare"
                .to_string(),
        };
    };
    let old_objects: BTreeMap<&ObjectId, BTreeMap<String, String>> = old
        .objects
        .iter()
        .map(|o| (&o.id, o.fact_lines()))
        .collect();
    let new_objects: BTreeMap<&ObjectId, BTreeMap<String, String>> = new
        .objects
        .iter()
        .map(|o| (&o.id, o.fact_lines()))
        .collect();
    let added = new_objects
        .keys()
        .filter(|id| !old_objects.contains_key(*id))
        .map(|id| (*id).clone())
        .collect();
    let removed = old_objects
        .keys()
        .filter(|id| !new_objects.contains_key(*id))
        .map(|id| (*id).clone())
        .collect();
    let changed = old_objects
        .iter()
        .filter_map(|(id, old_lines)| {
            let changes = fact_changes(old_lines, new_objects.get(id)?);
            (!changes.is_empty()).then(|| ObjectChange {
                object: (*id).clone(),
                changes,
            })
        })
        .collect();
    let old_keys: BTreeSet<_> = before.findings().map(HelmFinding::key).collect();
    let new_keys: BTreeSet<_> = after.findings().map(HelmFinding::key).collect();
    ChartDelta::Compared(ComparedDelta {
        added,
        removed,
        changed,
        findings_introduced: after
            .findings()
            .filter(|f| !old_keys.contains(&f.key()))
            .cloned()
            .collect(),
        findings_resolved: before
            .findings()
            .filter(|f| !new_keys.contains(&f.key()))
            .cloned()
            .collect(),
        caveats: caveats(before, after),
    })
}

fn fact_changes(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<FactChange> {
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    paths
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| FactChange {
            path: path.clone(),
            before: before.get(path).cloned(),
            after: after.get(path).cloned(),
        })
        .collect()
}

fn caveats(before: &ChartAnalysis, after: &ChartAnalysis) -> Vec<String> {
    [("before", before), ("after", after)]
        .into_iter()
        .filter(|(_, side)| side.coverage != Coverage::Complete)
        .map(|(name, _)| {
            format!("the {name} render has partial coverage: the delta covers only what rendered")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::iac::helm::manifest::RenderedDocument;
    use crate::domain::iac::helm::render::NoRenderer;
    use crate::domain::iac::helm::template::TemplateModel;
    use serde_json::json;
    use std::path::PathBuf;

    struct Fixed(RenderResponse);
    impl ChartRenderer for Fixed {
        fn render(&self, _: &RenderRequest<'_>) -> RenderResponse {
            self.0.clone()
        }
    }

    fn chart(source: &str) -> ChartModel {
        ChartModel {
            name: "demo".into(),
            version: "0.1.0".into(),
            location: PathBuf::from("demo"),
            content_digest: "sha256:c".into(),
            default_values: json!({"image": "app:1"}),
            schema: None,
            dependencies: vec![],
            templates: vec![TemplateModel::analyze("templates/d.yaml", source)],
        }
    }

    fn docs(privileged: bool) -> RenderResponse {
        RenderResponse::Rendered(vec![RenderedDocument {
            template: Some("demo/templates/d.yaml".into()),
            index: 0,
            value: json!({"kind": "Deployment", "metadata": {"name": "web"},
                "spec": {"template": {"spec": {"containers": [{"name": "a", "image": "app:1",
                    "securityContext": {"privileged": privileged,
                        "allowPrivilegeEscalation": false, "runAsNonRoot": true}}]}}}}),
        }])
    }

    #[test]
    fn without_an_engine_nothing_is_rendered_and_every_check_is_unknown() {
        let analysis = analyze_chart(&chart("i: {{ .Values.image }}"), &[], "r", &NoRenderer);
        assert!(!analysis.render.is_rendered());
        assert!(analysis.manifest.is_none());
        assert!(matches!(analysis.coverage, Coverage::Unknown { .. }));
        assert_eq!(analysis.findings().count(), 0);
        assert!(
            analysis
                .results
                .iter()
                .all(|r| matches!(r.verdict, CheckVerdict::Unknown { .. }))
        );
    }

    #[test]
    fn a_failed_engine_is_a_distinct_explicit_cause() {
        let renderer = Fixed(RenderResponse::Failed {
            message: "boom".into(),
        });
        let analysis = analyze_chart(&chart("x"), &[], "r", &renderer);
        assert!(matches!(
            analysis.render,
            RenderStatus::NotRendered(NotRenderedCause::EngineFailed { .. })
        ));
    }

    #[test]
    fn a_complete_render_without_findings_is_not_observed_and_names_its_scope() {
        let analysis = analyze_chart(
            &chart("i: {{ .Values.image }}"),
            &[],
            "r",
            &Fixed(docs(false)),
        );
        assert_eq!(analysis.coverage, Coverage::Complete);
        let verdict = &analysis
            .result(HelmCheck::PrivilegedContainer)
            .unwrap()
            .verdict;
        assert!(
            matches!(verdict, CheckVerdict::NotObserved { within } if within.contains("1 rendered"))
        );
    }

    #[test]
    fn a_partial_render_turns_a_negative_into_unknown_but_keeps_real_findings() {
        let partial = chart("i: {{ .Values.missing }}");
        let analysis = analyze_chart(&partial, &[], "r", &Fixed(docs(true)));
        assert!(matches!(analysis.coverage, Coverage::Partial { .. }));
        assert!(matches!(
            analysis
                .result(HelmCheck::PrivilegedContainer)
                .unwrap()
                .verdict,
            CheckVerdict::Found { findings: 1 }
        ));
        assert!(matches!(
            analysis.result(HelmCheck::BroadRbac).unwrap().verdict,
            CheckVerdict::Unknown { .. }
        ));
    }

    #[test]
    fn different_renders_give_different_deltas_and_findings() {
        let c = chart("i: {{ .Values.image }}");
        let safe = analyze_chart(&c, &[], "r", &Fixed(docs(false)));
        let risky = analyze_chart(&c, &[json!({"image": "app:2"})], "r", &Fixed(docs(true)));
        assert_ne!(safe.values_digest, risky.values_digest);
        let ChartDelta::Compared(delta) = compare(&safe, &risky) else {
            panic!("both sides rendered")
        };
        assert_eq!(delta.changed.len(), 1);
        assert_eq!(delta.changed[0].changes[0].path, "containers[a].privileged");
        assert_eq!(delta.findings_introduced.len(), 1);
        assert!(delta.findings_resolved.is_empty());
        let ChartDelta::Compared(same) = compare(&safe, &safe) else {
            panic!("both sides rendered")
        };
        assert!(same.is_empty());
    }

    #[test]
    fn an_unrendered_side_makes_the_delta_not_comparable() {
        let c = chart("x");
        let rendered = analyze_chart(&c, &[], "r", &Fixed(docs(false)));
        let missing = analyze_chart(&c, &[], "r", &NoRenderer);
        assert!(matches!(
            compare(&rendered, &missing),
            ChartDelta::NotComparable { .. }
        ));
    }
}
