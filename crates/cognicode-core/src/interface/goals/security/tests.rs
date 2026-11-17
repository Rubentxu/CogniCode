//! Behaviour of the `security` goal over real Terraform and Helm files.
//!
//! The Terraform projects are the committed fixtures and temporary projects
//! written to disk; the Helm chart is read by the real chart reader. The chart
//! renderer is injected: `NoRenderer` is the honest "engine missing" case and
//! `HandAuthored` is a labelled test double that stands in for `helm template`.

use std::path::{Path, PathBuf};

use serde_json::json;

use super::*;
use crate::domain::findings::semantic_model::PACK_VERSION;
use crate::domain::iac::helm::{NoRenderer, RenderRequest, RenderResponse};
use crate::infrastructure::helm::parse_rendered_stream;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .join(name)
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Test double: hand-authored `helm template` output with a privileged
/// container. It stands in for the engine and nothing more.
struct HandAuthored;

impl ChartRenderer for HandAuthored {
    fn render(&self, _request: &RenderRequest<'_>) -> RenderResponse {
        RenderResponse::Rendered(
            parse_rendered_stream(
                "---\n# Source: demo/templates/deployment.yaml\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: rel\nspec:\n  template:\n    spec:\n      containers:\n        - name: app\n          image: demo:latest\n          securityContext:\n            privileged: true\n",
            )
            .unwrap(),
        )
    }
}

/// A test double for an engine that ran and refused the chart.
struct Refuses;

impl ChartRenderer for Refuses {
    fn render(&self, _request: &RenderRequest<'_>) -> RenderResponse {
        RenderResponse::Failed {
            message: "template: parse error at line 3".to_string(),
        }
    }
}

fn write_chart(root: &Path, at: &str) {
    write(
        root,
        &format!("{at}/Chart.yaml"),
        "apiVersion: v2\nname: demo\nversion: 0.3.0\n",
    );
    write(root, &format!("{at}/values.yaml"), "image: demo:latest\n");
    write(
        root,
        &format!("{at}/templates/deployment.yaml"),
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: {{ .Release.Name }}\n",
    );
}

fn report(workspace: &Path, renderer: &dyn ChartRenderer) -> GoalSecurity {
    security(workspace, renderer, &SecurityInput::default()).expect("the goal answers")
}

fn cell<'a>(unit: &'a UnitReport, family: &str) -> &'a FamilyVerdict {
    unit.families
        .iter()
        .find(|f| f.family == family)
        .unwrap_or_else(|| panic!("the unit {} has no verdict for {family}", unit.path))
}

fn unit<'a>(units: &'a [UnitReport], path: &str) -> &'a UnitReport {
    units.iter().find(|u| u.path == path).unwrap_or_else(|| {
        panic!(
            "no unit {path} in {:?}",
            units.iter().map(|u| &u.path).collect::<Vec<_>>()
        )
    })
}

fn all_cells(answer: &GoalSecurity) -> Vec<&FamilyVerdict> {
    let terraform = answer.terraform.iter().flat_map(|t| &t.units);
    let helm = answer.helm.iter().flat_map(|h| &h.units);
    terraform
        .chain(helm)
        .flat_map(|u| u.families.iter())
        .collect()
}

// ---------------------------------------------------------------------------
// Terraform: verdicts per unit and family
// ---------------------------------------------------------------------------

#[test]
fn every_terraform_file_gets_a_verdict_for_every_structural_family() {
    let answer = report(&fixture("terraform-risk"), &NoRenderer);

    assert_eq!(answer.terraform.len(), 1, "the modules belong to one root");
    assert_eq!(answer.terraform[0].root, ".");
    let main = unit(&answer.terraform[0].units, "main.tf");
    let families: Vec<&str> = main.families.iter().map(|f| f.family.as_str()).collect();
    assert_eq!(
        families,
        [
            "hardcoded_secret",
            "unsafe_code",
            "infra_public_exposure",
            "infra_encryption",
            "infra_permissions",
            "infra_workload_hardening",
        ]
    );
}

#[test]
fn a_verifiable_risk_is_found_with_its_finding_and_a_module_file_is_reported_by_its_path() {
    let answer = report(&fixture("terraform-risk"), &NoRenderer);
    let units = &answer.terraform[0].units;

    let main = unit(units, "main.tf");
    let secret = cell(main, "hardcoded_secret");
    assert_eq!(secret.verdict, VerdictKind::Found, "{secret:?}");
    assert!(!secret.findings.is_empty());
    assert!(
        !secret.findings[0].message.is_empty() && !secret.findings[0].evidence_class.is_empty()
    );

    let network = unit(units, "modules/network/main.tf");
    assert_eq!(
        cell(network, "infra_public_exposure").verdict,
        VerdictKind::Found
    );
}

#[test]
fn a_family_without_a_model_for_terraform_is_unsupported_and_never_clean() {
    let answer = report(&fixture("terraform-risk"), &NoRenderer);

    for unit in &answer.terraform[0].units {
        for family in ["unsafe_code", "infra_workload_hardening"] {
            let verdict = cell(unit, family);
            assert_eq!(
                verdict.verdict,
                VerdictKind::Unsupported,
                "{} {family}: {verdict:?}",
                unit.path
            );
            assert!(verdict.detail.contains("no model"), "{}", verdict.detail);
            assert!(verdict.findings.is_empty());
        }
    }
}

#[test]
fn a_value_the_configuration_does_not_fix_is_unknown_and_a_fixed_one_is_not_observed() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "unbound/main.tf",
        "variable \"acl\" {\n  type = string\n}\n\nresource \"aws_s3_bucket\" \"b\" {\n  bucket = \"b\"\n  acl    = var.acl\n}\n",
    );
    write(
        dir.path(),
        "fixed/main.tf",
        "resource \"aws_s3_bucket\" \"b\" {\n  bucket = \"b\"\n  acl    = \"private\"\n}\n",
    );

    let answer = report(dir.path(), &NoRenderer);

    let unbound = answer
        .terraform
        .iter()
        .find(|t| t.root == "unbound")
        .expect("the unbound project is a root");
    let verdict = cell(
        unit(&unbound.units, "unbound/main.tf"),
        "infra_public_exposure",
    );
    assert_eq!(verdict.verdict, VerdictKind::Unknown, "{verdict:?}");
    assert!(
        !verdict.unresolved.is_empty(),
        "an unknown says what could not be decided"
    );
    assert!(
        unbound
            .coverage
            .unbound_variables
            .iter()
            .any(|v| v.contains("acl")),
        "{:?}",
        unbound.coverage
    );
    assert!(!unbound.coverage.complete);

    let fixed = answer
        .terraform
        .iter()
        .find(|t| t.root == "fixed")
        .expect("the fixed project is a root");
    let verdict = cell(unit(&fixed.units, "fixed/main.tf"), "infra_public_exposure");
    assert_eq!(verdict.verdict, VerdictKind::NotObserved, "{verdict:?}");
    assert!(
        verdict.detail.contains("analysed unit"),
        "a negative states its scope: {}",
        verdict.detail
    );
}

#[test]
fn terraform_coverage_lists_what_was_not_evaluated() {
    let answer = report(&fixture("terraform-risk"), &NoRenderer);
    let coverage = &answer.terraform[0].coverage;

    assert!(!coverage.complete);
    assert!(
        coverage
            .unresolved_modules
            .iter()
            .any(|m| m.contains("terraform-aws-modules/s3-bucket/aws")),
        "the remote module is not opened and says so: {coverage:?}"
    );
    assert!(
        coverage
            .unbound_variables
            .iter()
            .any(|v| v.contains("ssh_cidr")),
        "{coverage:?}"
    );
    assert!(coverage.resources >= coverage.assessed && coverage.assessed > 0);
}

#[test]
fn modules_are_not_reported_as_roots_of_their_own() {
    let answer = report(&fixture("terraform-iac"), &NoRenderer);

    assert_eq!(
        answer.terraform.len(),
        1,
        "{:?}",
        answer.terraform.iter().map(|t| &t.root).collect::<Vec<_>>()
    );
    assert_eq!(answer.terraform[0].root, ".");
}

// ---------------------------------------------------------------------------
// Helm: render status and coverage
// ---------------------------------------------------------------------------

#[test]
fn a_chart_without_an_engine_is_not_rendered_and_every_dependent_check_is_unknown() {
    let dir = tempfile::tempdir().unwrap();
    write_chart(dir.path(), "charts/demo");

    let answer = report(dir.path(), &NoRenderer);

    assert_eq!(answer.helm.len(), 1);
    let chart = &answer.helm[0];
    assert_eq!(chart.path, "charts/demo");
    assert_eq!(chart.chart, "demo");
    assert_eq!(chart.render.status, "not_rendered");
    assert_eq!(chart.render.documents, None);
    assert!(
        chart
            .render
            .cause
            .as_deref()
            .unwrap()
            .contains("no chart renderer configured"),
        "{:?}",
        chart.render
    );
    assert_eq!(chart.coverage.state, "unknown");

    let descriptor = unit(&chart.units, "demo/Chart.yaml");
    for family in [
        "infra_public_exposure",
        "infra_permissions",
        "infra_workload_hardening",
    ] {
        assert_eq!(
            cell(descriptor, family).verdict,
            VerdictKind::Unknown,
            "a chart that was not rendered is never clean: {family}"
        );
    }
    // Negative control: nothing of an unrendered chart may read as a negative.
    assert_eq!(answer.totals.not_observed, 0, "{:?}", answer.totals);
    assert_eq!(answer.totals.found, 0);
}

#[test]
fn an_engine_that_refuses_the_chart_is_not_rendered_with_its_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    write_chart(dir.path(), "demo");

    let answer = report(dir.path(), &Refuses);

    let chart = &answer.helm[0];
    assert_eq!(chart.render.status, "not_rendered");
    assert!(
        chart
            .render
            .cause
            .as_deref()
            .unwrap()
            .contains("parse error"),
        "{:?}",
        chart.render
    );
    assert_eq!(answer.totals.not_observed, 0);
}

#[test]
fn a_rendered_chart_reports_its_documents_coverage_and_findings() {
    let dir = tempfile::tempdir().unwrap();
    write_chart(dir.path(), "demo");

    let answer = report(dir.path(), &HandAuthored);

    let chart = &answer.helm[0];
    assert_eq!(chart.render.status, "rendered");
    assert_eq!(chart.render.documents, Some(1));
    assert_eq!(chart.render.cause, None);
    assert!(
        ["complete", "partial"].contains(&chart.coverage.state.as_str()),
        "{:?}",
        chart.coverage
    );
    let template = unit(&chart.units, "demo/templates/deployment.yaml");
    let permissions = cell(template, "infra_permissions");
    assert_eq!(permissions.verdict, VerdictKind::Found, "{permissions:?}");
    assert!(
        permissions.findings[0]
            .kind
            .contains("excessive_permissions"),
        "{:?}",
        permissions.findings
    );
    // The pack has no encryption model for Helm: it says so.
    assert_eq!(
        cell(template, "infra_encryption").verdict,
        VerdictKind::Unsupported
    );
    assert!(!chart.limits.is_empty());
}

#[test]
fn an_unreadable_chart_is_listed_as_unanalysed_and_not_dropped() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "broken/Chart.yaml", "name: [unterminated\n");

    let answer = report(dir.path(), &HandAuthored);

    assert!(answer.helm.is_empty());
    assert_eq!(answer.unanalysed.len(), 1, "{:?}", answer.unanalysed);
    assert_eq!(answer.unanalysed[0].path, "broken");
    assert_eq!(answer.unanalysed[0].kind, "helm");
    assert!(!answer.unanalysed[0].reason.is_empty());
    assert!(
        answer.summary.contains("could not be analysed"),
        "{}",
        answer.summary
    );
}

#[test]
fn a_vendored_subchart_is_judged_through_its_parent() {
    let dir = tempfile::tempdir().unwrap();
    write_chart(dir.path(), "demo");
    write_chart(dir.path(), "demo/charts/dep");

    let answer = report(dir.path(), &HandAuthored);

    assert_eq!(
        answer.helm.len(),
        1,
        "{:?}",
        answer.helm.iter().map(|h| &h.path).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Totals, summary, empty workspace
// ---------------------------------------------------------------------------

#[test]
fn the_totals_count_every_cell_exactly_once() {
    let dir = tempfile::tempdir().unwrap();
    write_chart(dir.path(), "demo");
    write(
        dir.path(),
        "infra/main.tf",
        "resource \"aws_s3_bucket\" \"b\" {\n  acl = \"public-read\"\n}\n",
    );

    let answer = report(dir.path(), &HandAuthored);

    let cells = all_cells(&answer);
    let count = |kind| cells.iter().filter(|c| c.verdict == kind).count();
    assert_eq!(answer.totals.found, count(VerdictKind::Found));
    assert_eq!(answer.totals.not_observed, count(VerdictKind::NotObserved));
    assert_eq!(answer.totals.unknown, count(VerdictKind::Unknown));
    assert_eq!(answer.totals.unsupported, count(VerdictKind::Unsupported));
    assert_eq!(
        answer.totals.found
            + answer.totals.not_observed
            + answer.totals.unknown
            + answer.totals.unsupported,
        cells.len()
    );
    assert!(answer.totals.found > 0 && answer.totals.unsupported > 0);
    for word in ["found", "not observed", "unknown", "unsupported"] {
        assert!(
            answer.summary.contains(word),
            "{word} missing: {}",
            answer.summary
        );
    }
}

#[test]
fn a_workspace_without_terraform_or_helm_says_nothing_was_analysed_instead_of_clean() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "src/main.rs", "fn main() {}");

    let answer = report(dir.path(), &HandAuthored);

    assert!(answer.terraform.is_empty() && answer.helm.is_empty());
    assert_eq!(answer.totals, VerdictTotals::default());
    assert!(
        answer.summary.contains("nothing was analysed"),
        "{}",
        answer.summary
    );
    assert!(
        !answer.summary.to_lowercase().contains("clean"),
        "{}",
        answer.summary
    );
    // The matrix and the limits are published even when nothing was found.
    assert!(!answer.matrix.is_empty() && !answer.limits.is_empty());
}

#[test]
fn a_path_that_is_not_a_directory_is_an_error_not_an_empty_report() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nowhere");

    let error = security(&missing, &NoRenderer, &SecurityInput::default()).unwrap_err();

    assert!(error.contains("not a directory"), "{error}");
}

// ---------------------------------------------------------------------------
// The capability matrix
// ---------------------------------------------------------------------------

#[test]
fn the_matrix_is_published_for_terraform_and_helm_with_its_limits() {
    let dir = tempfile::tempdir().unwrap();
    let answer = report(dir.path(), &NoRenderer);

    assert_eq!(answer.pack_version, PACK_VERSION);
    let languages: std::collections::BTreeSet<&str> =
        answer.matrix.iter().map(|r| r.language.as_str()).collect();
    assert_eq!(languages, ["helm", "terraform"].into_iter().collect());
    assert_eq!(answer.matrix.len(), 2 * 9);

    let row = |language: &str, family: &str| {
        answer
            .matrix
            .iter()
            .find(|r| r.language == language && r.family == family)
            .unwrap_or_else(|| panic!("no row {language} {family}"))
    };
    let supported = row("terraform", "infra_public_exposure");
    assert_eq!(supported.support, "supported");
    assert!(supported.backend.is_some() && supported.evidence_ceiling.is_some());
    assert!(
        !supported.limits.is_empty(),
        "a supported cell states where it stops"
    );

    let unsupported = row("helm", "infra_encryption");
    assert_eq!(unsupported.support, "unsupported");
    assert!(unsupported.reason.as_deref().unwrap().contains("no model"));
    assert!(unsupported.backend.is_none() && !unsupported.can_block);

    let advisory = row("helm", "infra_workload_hardening");
    assert_eq!(advisory.blocking, "advisory_only");
}

#[test]
fn the_whole_matrix_is_published_on_request() {
    let dir = tempfile::tempdir().unwrap();

    let answer = security(
        dir.path(),
        &NoRenderer,
        &SecurityInput {
            all_languages: true,
        },
    )
    .unwrap();

    let languages: std::collections::BTreeSet<&str> =
        answer.matrix.iter().map(|r| r.language.as_str()).collect();
    assert_eq!(languages.len(), 7);
    assert!(languages.contains("rust") && languages.contains("terraform"));
}

#[test]
fn the_report_states_that_it_is_not_a_gate_and_that_a_negative_is_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let answer = report(dir.path(), &NoRenderer);

    let limits = answer.limits.join("\n");
    assert!(limits.contains("not a gate"), "{limits}");
    assert!(limits.contains("not observed"), "{limits}");
}

// ---------------------------------------------------------------------------
// The goal: arguments and text
// ---------------------------------------------------------------------------

#[test]
fn the_goal_rejects_arguments_that_are_not_its_own() {
    let dir = tempfile::tempdir().unwrap();

    let error = run(
        &GoalEnvironment::new(dir.path(), None),
        &json!({"all_languages": "yes"}),
    )
    .unwrap_err();

    assert!(error.contains("not valid"), "{error}");
}

#[test]
fn the_text_names_every_verdict_by_its_own_word_and_the_render_state() {
    let dir = tempfile::tempdir().unwrap();
    write_chart(dir.path(), "demo");
    write(
        dir.path(),
        "infra/main.tf",
        "resource \"aws_s3_bucket\" \"b\" {\n  acl = \"public-read\"\n}\n",
    );

    let answer = security(dir.path(), &NoRenderer, &SecurityInput::default()).unwrap();
    let text = render(&serde_json::to_value(&answer).unwrap());

    for word in [
        "FOUND",
        "UNKNOWN",
        "UNSUPPORTED",
        "not rendered",
        "capability matrix",
    ] {
        assert!(text.contains(word), "{word} missing from:\n{text}");
    }
}
