//! A Helm chart is read from disk and judged only through a real render.
//!
//! The chart below is real files read by the real chart reader. The renderer
//! is a labelled test double: it answers with hand-authored `helm template`
//! output chosen by the values layers it receives, because the official
//! engine is not required to be installed. A separate test runs the real
//! engine when it exists and says so when it does not.

use std::path::Path;

use cognicode_core::domain::iac::helm::{
    ChartAnalysis, ChartDelta, ChartRenderer, CheckVerdict, HelmCheck, NoRenderer,
    NotRenderedCause, PartialReason, RenderCoverage, RenderRequest, RenderResponse, RenderStatus,
    analyze_chart, compare,
};
use cognicode_core::infrastructure::helm::{
    HelmCli, parse_rendered_stream, read_chart, read_values_file,
};
use serde_json::{Value, json};

pub(crate) fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

pub(crate) fn demo_chart() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "Chart.yaml",
        "apiVersion: v2\nname: demo\nversion: 0.3.0\n",
    );
    write(
        root,
        "values.yaml",
        "image: demo:latest\nhardened: false\nrbac:\n  wildcard: true\n",
    );
    write(
        root,
        "templates/deployment.yaml",
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: {{ .Release.Name }}\nspec:\n  template:\n    spec:\n      containers:\n        - name: app\n          image: {{ .Values.image }}\n          {{- if .Values.hardened }}\n          securityContext:\n            runAsNonRoot: true\n          {{- else }}\n          securityContext:\n            privileged: true\n          {{- end }}\n",
    );
    write(
        root,
        "templates/role.yaml",
        "{{- if .Values.rbac.wildcard }}\napiVersion: rbac.authorization.k8s.io/v1\nkind: ClusterRole\nmetadata:\n  name: {{ .Release.Name }}\nrules:\n  - apiGroups: [\"*\"]\n    resources: [\"*\"]\n    verbs: [\"*\"]\n{{- end }}\n",
    );
    dir
}

const PERMISSIVE: &str = "---
# Source: demo/templates/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rel
spec:
  template:
    spec:
      containers:
        - name: app
          image: demo:latest
          securityContext:
            privileged: true
---
# Source: demo/templates/role.yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: rel
rules:
  - apiGroups: [\"*\"]
    resources: [\"*\"]
    verbs: [\"*\"]
";

const HARDENED: &str = "---
# Source: demo/templates/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rel
spec:
  template:
    spec:
      containers:
        - name: app
          image: demo@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
          securityContext:
            runAsNonRoot: true
            allowPrivilegeEscalation: false
";

/// Test double: hand-authored `helm template` output, chosen by the values
/// layers it is given. It stands in for the engine and nothing more.
pub(crate) struct HandAuthoredRender;

impl ChartRenderer for HandAuthoredRender {
    fn render(&self, request: &RenderRequest<'_>) -> RenderResponse {
        let hardened = request
            .overrides
            .iter()
            .any(|layer| layer.get("hardened") == Some(&Value::Bool(true)));
        let text = if hardened { HARDENED } else { PERMISSIVE };
        RenderResponse::Rendered(parse_rendered_stream(text).unwrap())
    }
}

fn analyze(overrides: &[Value]) -> ChartAnalysis {
    let dir = demo_chart();
    let chart = read_chart(dir.path()).unwrap();
    analyze_chart(&chart, overrides, "rel", &HandAuthoredRender)
}

/// The real Helm engine, or `None` after saying why it did not run.
///
/// A machine without a runnable `helm` skips the faithful-render lane loudly,
/// saying why (absent, or found but unable to run in the confined
/// environment). A lane that is supposed to have it sets
/// `COGNICODE_REQUIRE_HELM`, and then its absence fails the test instead of
/// passing by omission. That lane is `scripts/ci/helm-lane.sh`.
pub(crate) fn real_helm() -> Option<HelmCli> {
    let cli = HelmCli::from_path();
    if cli.is_available() {
        return Some(cli);
    }
    let reason = cli.unavailable_reason().unwrap_or("no reason given");
    assert!(
        std::env::var_os("COGNICODE_REQUIRE_HELM").is_none(),
        "COGNICODE_REQUIRE_HELM is set but `helm` is unavailable: {reason}"
    );
    eprintln!(
        "SKIPPED: `helm` is unavailable ({reason}); the faithful render was not \
         exercised and only the hand-authored renderer double ran"
    );
    None
}

fn verdict(analysis: &ChartAnalysis, check: HelmCheck) -> CheckVerdict {
    analysis.result(check).unwrap().verdict.clone()
}

#[test]
fn two_values_sets_give_different_objects_findings_and_deltas() {
    let permissive = analyze(&[]);
    let hardened = analyze(&[json!({"hardened": true, "rbac": {"wildcard": false}})]);

    assert_ne!(permissive.values_digest, hardened.values_digest);
    assert!(matches!(
        verdict(&permissive, HelmCheck::PrivilegedContainer),
        CheckVerdict::Found { findings: 1 }
    ));
    assert!(matches!(
        verdict(&permissive, HelmCheck::BroadRbac),
        CheckVerdict::Found { .. }
    ));
    assert!(matches!(
        verdict(&hardened, HelmCheck::PrivilegedContainer),
        CheckVerdict::NotObserved { .. }
    ));
    assert!(matches!(
        verdict(&hardened, HelmCheck::BroadRbac),
        CheckVerdict::NotObserved { .. }
    ));

    let ChartDelta::Compared(delta) = compare(&permissive, &hardened) else {
        panic!("both sides rendered, the delta must be a comparison");
    };
    assert!(
        delta.removed.iter().any(|id| id.kind == "ClusterRole"),
        "the cluster role disappears: {delta:?}"
    );
    assert!(
        delta
            .changed
            .iter()
            .any(|change| change.object.kind == "Deployment"),
        "the deployment changes: {delta:?}"
    );
    let resolved: Vec<_> = delta.findings_resolved.iter().map(|f| f.check).collect();
    assert!(resolved.contains(&HelmCheck::PrivilegedContainer));
    assert!(resolved.contains(&HelmCheck::BroadRbac));
    assert!(delta.findings_introduced.is_empty());
}

#[test]
fn findings_carry_the_template_chart_and_values_that_fixed_them() {
    let analysis = analyze(&[]);
    let finding = analysis
        .findings()
        .find(|f| f.check == HelmCheck::PrivilegedContainer)
        .expect("the permissive render is privileged");
    assert_eq!(finding.object.kind, "Deployment");
    assert_eq!(
        finding.origin.template.as_deref(),
        Some("demo/templates/deployment.yaml")
    );
    assert_eq!(finding.origin.chart, "demo");
    assert_eq!(finding.origin.chart_version, "0.3.0");
    assert_eq!(finding.origin.values_digest, analysis.values_digest);
    let mutable = analysis
        .findings()
        .find(|f| f.check == HelmCheck::MutableImage)
        .expect("`demo:latest` is a mutable reference");
    assert!(mutable.detail.contains("demo:latest"), "{}", mutable.detail);
}

#[test]
fn a_chart_that_was_not_rendered_has_no_objects_and_no_clean_verdicts() {
    let dir = demo_chart();
    let chart = read_chart(dir.path()).unwrap();
    let analysis = analyze_chart(&chart, &[], "rel", &NoRenderer);

    assert!(matches!(
        analysis.render,
        RenderStatus::NotRendered(NotRenderedCause::EngineUnavailable { .. })
    ));
    assert!(analysis.manifest.is_none(), "no manifest is invented");
    assert_eq!(analysis.findings().count(), 0);
    assert!(matches!(analysis.coverage, RenderCoverage::Unknown { .. }));
    for check in HelmCheck::ALL {
        assert!(
            matches!(verdict(&analysis, check), CheckVerdict::Unknown { .. }),
            "{check:?} must be unknown, not clean"
        );
    }
    let ChartDelta::NotComparable { .. } = compare(&analysis, &analyze(&[])) else {
        panic!("an unrendered side makes the delta not comparable");
    };
}

#[test]
fn partial_values_make_a_render_partial_and_negatives_unknown() {
    let dir = demo_chart();
    // A template reads a value that nothing sets, outside any condition.
    write(
        dir.path(),
        "templates/service.yaml",
        "apiVersion: v1\nkind: Service\nmetadata:\n  name: {{ .Values.service.name }}\n",
    );
    let chart = read_chart(dir.path()).unwrap();
    let analysis = analyze_chart(
        &chart,
        &[json!({"hardened": true, "rbac": {"wildcard": false}})],
        "rel",
        &HandAuthoredRender,
    );

    let RenderCoverage::Partial { reasons } = &analysis.coverage else {
        panic!("expected partial coverage, got {:?}", analysis.coverage);
    };
    assert!(
        reasons
            .iter()
            .any(|r| matches!(r, PartialReason::UnsetValue(gap) if gap.path == "service.name")),
        "{reasons:?}"
    );
    // Nothing privileged was observed, yet that is not a clean bill.
    assert!(matches!(
        verdict(&analysis, HelmCheck::PrivilegedContainer),
        CheckVerdict::Unknown { .. }
    ));
}

#[test]
fn a_chart_with_a_missing_dependency_and_required_value_is_not_complete() {
    let dir = demo_chart();
    write(
        dir.path(),
        "Chart.yaml",
        "apiVersion: v2\nname: demo\nversion: 0.3.0\ndependencies:\n  - name: redis\n    version: 1.x\n    repository: https://example.invalid\n",
    );
    write(
        dir.path(),
        "values.schema.json",
        r#"{"required": ["tenant"]}"#,
    );
    let chart = read_chart(dir.path()).unwrap();
    let analysis = analyze_chart(
        &chart,
        &[json!({"hardened": true})],
        "rel",
        &HandAuthoredRender,
    );
    let RenderCoverage::Partial { reasons } = &analysis.coverage else {
        panic!("expected partial coverage, got {:?}", analysis.coverage);
    };
    assert!(reasons.iter().any(|r| matches!(
        r,
        PartialReason::DependencyMissing { name } if name == "redis"
    )));
    assert!(reasons.iter().any(|r| matches!(
        r,
        PartialReason::SchemaRequiredMissing { path } if path == "tenant"
    )));
}

#[test]
fn without_helm_the_core_still_reads_the_chart_and_declares_what_it_cannot_verify() {
    let dir = demo_chart();
    let chart = read_chart(dir.path()).unwrap();
    // A renderer pointing at nothing is the "no Helm installed" machine.
    let absent = HelmCli::with_binary("/nonexistent/helm".into());
    let analysis = analyze_chart(&chart, &[], "rel", &absent);

    assert!(chart.templates.iter().any(|t| !t.is_helper));
    assert!(!analysis.render.is_rendered());
    assert!(analysis.manifest.is_none());
    assert!(
        !analysis.limits.is_empty(),
        "the result must say what it could not verify"
    );
    // The static model still says something true: the chart reads `image`.
    let reads: Vec<&str> = chart
        .templates
        .iter()
        .flat_map(|t| t.value_refs.iter().map(|r| r.path.as_str()))
        .collect();
    assert!(reads.contains(&"image"), "{reads:?}");
}

#[test]
fn the_values_file_a_caller_passes_changes_the_digest_that_names_the_render() {
    let dir = demo_chart();
    write(dir.path(), "prod.yaml", "hardened: true\n");
    let layer = read_values_file(&dir.path().join("prod.yaml")).unwrap();
    let chart = read_chart(dir.path()).unwrap();
    let base = analyze_chart(&chart, &[], "rel", &HandAuthoredRender);
    let prod = analyze_chart(&chart, &[layer], "rel", &HandAuthoredRender);
    assert_ne!(base.values_digest, prod.values_digest);
}

#[test]
fn the_real_engine_renders_the_chart_when_it_is_installed() {
    let Some(cli) = real_helm() else {
        return;
    };
    let dir = demo_chart();
    let chart = read_chart(dir.path()).unwrap();
    let analysis = analyze_chart(&chart, &[], "rel", &cli);
    assert!(
        analysis.render.is_rendered(),
        "helm was found but did not render: {:?}",
        analysis.render
    );
    assert!(matches!(
        verdict(&analysis, HelmCheck::PrivilegedContainer),
        CheckVerdict::Found { .. }
    ));
}

fn found(analysis: &ChartAnalysis, check: HelmCheck) -> bool {
    matches!(verdict(analysis, check), CheckVerdict::Found { .. })
}

/// The labelled double is only trustworthy where it matches the engine. For
/// the default values both give the same verdict on every check. For the
/// hardened values they agree on what the chart's `hardened` and `rbac` values
/// control, and they differ on two checks: the double's hardened output was
/// written with a pinned image digest and `allowPrivilegeEscalation: false`
/// that the demo chart never renders, so only the real engine keeps reporting
/// `PrivilegeEscalation` and `MutableImage`.
#[test]
fn the_real_engine_agrees_with_the_double_except_where_the_double_was_hardened_by_hand() {
    let Some(cli) = real_helm() else {
        return;
    };
    let dir = demo_chart();
    let chart = read_chart(dir.path()).unwrap();

    let real = analyze_chart(&chart, &[], "rel", &cli);
    let double = analyze_chart(&chart, &[], "rel", &HandAuthoredRender);
    for check in HelmCheck::ALL {
        assert_eq!(
            found(&real, check),
            found(&double, check),
            "default values, {check:?}"
        );
    }

    let values = [json!({"hardened": true, "rbac": {"wildcard": false}})];
    let real = analyze_chart(&chart, &values, "rel", &cli);
    let double = analyze_chart(&chart, &values, "rel", &HandAuthoredRender);
    for check in [
        HelmCheck::PrivilegedContainer,
        HelmCheck::RunsAsRoot,
        HelmCheck::HostAccess,
        HelmCheck::BroadRbac,
        HelmCheck::ExternalExposure,
    ] {
        assert_eq!(
            found(&real, check),
            found(&double, check),
            "hardened values, {check:?}"
        );
    }
    for check in [HelmCheck::PrivilegeEscalation, HelmCheck::MutableImage] {
        assert!(found(&real, check), "the real render still has {check:?}");
        assert!(!found(&double, check), "the double hides {check:?}");
    }
}
