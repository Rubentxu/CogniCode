//! A Helm chart reaches the security pack only through a real render.
//!
//! The chart is real files read by the real chart reader. The renderer is the
//! labelled test double of the Helm model tests, because the official engine
//! is not required to be installed; one test runs the real engine when it
//! exists and says so when it does not. Every run goes through the same
//! `SecurityPack` that judges Terraform and source code.

use cognicode_core::application::findings::{PackRun, RunAuthority, RunContext, SecurityPack};
use cognicode_core::domain::execution::CorrelationId;
use cognicode_core::domain::findings::semantic_model::{
    BlockingLevel, FindingNature, PackVerdict, SecurityFamily, SourceLanguage,
    StructuralClassification, StructuralFamily, Support, Weakness, classify_structural,
};
use cognicode_core::domain::findings::{AnalysisScope, GroundingRef};
use cognicode_core::domain::iac::helm::{
    ChartAnalysis, HelmCheck, NoRenderer, analyze_chart, observed_units, structural_family,
};
use cognicode_core::domain::kernel_ids::{EntityId, ExecutionId, FactId, SnapshotId};
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;
use cognicode_core::infrastructure::helm::{HelmCli, read_chart};
use serde_json::{Value, json};

use super::helm_model::{HandAuthoredRender, demo_chart, real_helm, write};

const HELM_FAMILIES: [StructuralFamily; 3] = [
    StructuralFamily::InfraPublicExposure,
    StructuralFamily::InfraPermissions,
    StructuralFamily::InfraWorkloadHardening,
];

fn scope() -> AnalysisScope {
    AnalysisScope::new(
        cognicode_core::domain::value_objects::WorkspaceId::try_new("workspace").unwrap(),
        SnapshotId::new(1),
    )
}

fn context(authority: RunAuthority) -> RunContext {
    RunContext {
        scope: scope(),
        execution_id: ExecutionId::new(1),
        correlation: CorrelationId::new("helm-pack-test").unwrap(),
        authority,
    }
}

fn promoted() -> RunAuthority {
    RunAuthority::Promoted {
        approver: "security-team".to_string(),
    }
}

fn grounded(_path: &str, line: u32) -> Option<GroundingRef> {
    Some(GroundingRef::entity(
        EntityId::new(u64::from(line)),
        FactId::new(700 + u64::from(line)),
    ))
}

fn ungrounded(_path: &str, _line: u32) -> Option<GroundingRef> {
    None
}

fn permissive() -> ChartAnalysis {
    let dir = demo_chart();
    let chart = read_chart(dir.path()).unwrap();
    analyze_chart(&chart, &[], "rel", &HandAuthoredRender)
}

fn hardened() -> ChartAnalysis {
    let dir = demo_chart();
    let chart = read_chart(dir.path()).unwrap();
    let values = [json!({"hardened": true, "rbac": {"wildcard": false}})];
    analyze_chart(&chart, &values, "rel", &HandAuthoredRender)
}

/// Every unit of the analysis through every Helm family, as the pack runs them.
fn run_all(
    analysis: &ChartAnalysis,
    grounding: &dyn Fn(&str, u32) -> Option<GroundingRef>,
    authority: &RunAuthority,
) -> Vec<(String, StructuralFamily, PackRun)> {
    let pack = SecurityPack::standard();
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    let mut runs = Vec::new();
    for unit in observed_units(analysis, grounding) {
        for family in HELM_FAMILIES {
            let run = pack
                .analyse_structural(family, &unit, &context(authority.clone()), &mut store)
                .expect("the run must be carried out");
            runs.push((unit.path.clone(), family, run));
        }
    }
    runs
}

fn runs_of(
    runs: &[(String, StructuralFamily, PackRun)],
    family: StructuralFamily,
) -> Vec<&PackRun> {
    runs.iter()
        .filter(|(_, f, _)| *f == family)
        .map(|(_, _, run)| run)
        .collect()
}

#[test]
fn a_rendered_violation_is_a_finding_that_names_its_object_template_and_values() {
    let analysis = permissive();
    let runs = run_all(&analysis, &grounded, &RunAuthority::Candidate);
    let (_, _, run) = runs
        .iter()
        .find(|(path, family, run)| {
            path == "demo/templates/deployment.yaml"
                && *family == StructuralFamily::InfraPermissions
                && matches!(run.verdict, PackVerdict::Found { .. })
        })
        .expect("the privileged container is found in the template that rendered it");
    let finding = &run.findings[0];
    assert!(finding.is_explainable());
    assert!(
        finding
            .causal_chain
            .iter()
            .any(|step| step.detail.contains("demo/templates/deployment.yaml")),
        "{:?}",
        finding.causal_chain
    );

    // The text of the evidence is built from the construct the pack hands the
    // backend: that is where the object and the values that fixed it live.
    let unit = observed_units(&analysis, &grounded)
        .into_iter()
        .find(|u| u.path == "demo/templates/deployment.yaml")
        .unwrap();
    let StructuralClassification::Modeled(ast) =
        classify_structural(StructuralFamily::InfraPermissions, &unit)
    else {
        panic!("permissions are modeled for helm");
    };
    let details: Vec<&str> = ast.units[0]
        .constructs
        .iter()
        .map(|c| c.detail.as_str())
        .collect();
    let privileged = details
        .iter()
        .find(|d| d.contains("helm.container.privileged"))
        .unwrap_or_else(|| panic!("{details:?}"));
    for needle in [
        "Deployment/rel",
        "demo/templates/deployment.yaml",
        "0.3.0",
        analysis.values_digest.as_str(),
    ] {
        assert!(
            privileged.contains(needle),
            "`{needle}` missing: {privileged}"
        );
    }
}

#[test]
fn only_a_promoted_grounded_permission_finding_blocks() {
    let analysis = permissive();
    let blocking = |grounding: &dyn Fn(&str, u32) -> Option<GroundingRef>,
                    authority: RunAuthority| {
        run_all(&analysis, grounding, &authority)
            .iter()
            .filter(|(_, family, _)| *family == StructuralFamily::InfraPermissions)
            .map(|(_, _, run)| run.blocking)
            .sum::<usize>()
    };
    assert!(blocking(&grounded, promoted()) > 0);
    assert_eq!(blocking(&grounded, RunAuthority::Candidate), 0);
    assert_eq!(blocking(&ungrounded, promoted()), 0);
}

#[test]
fn workload_hardening_is_found_but_never_blocks() {
    let analysis = permissive();
    let runs = run_all(&analysis, &grounded, &promoted());
    let hardening = runs_of(&runs, StructuralFamily::InfraWorkloadHardening);
    let found: Vec<_> = hardening
        .iter()
        .filter(|run| {
            matches!(
                run.verdict,
                PackVerdict::Found {
                    nature: FindingNature::Risk,
                    ..
                }
            )
        })
        .collect();
    assert!(!found.is_empty(), "the mutable `latest` image is a risk");
    assert!(found.iter().all(|run| run.blocking == 0));
}

#[test]
fn a_chart_that_was_not_rendered_is_unknown_for_every_family_never_clean() {
    let dir = demo_chart();
    let chart = read_chart(dir.path()).unwrap();
    let absent = HelmCli::with_binary("/nonexistent/helm".into());
    let analyses = [
        analyze_chart(&chart, &[], "rel", &NoRenderer),
        analyze_chart(&chart, &[], "rel", &absent),
    ];
    for analysis in &analyses {
        assert!(analysis.manifest.is_none());
        let runs = run_all(analysis, &grounded, &promoted());
        assert!(!runs.is_empty(), "an unrendered chart still reports");
        for (path, family, run) in &runs {
            assert!(
                matches!(run.verdict, PackVerdict::Unknown { .. }),
                "{path} {family:?}: {:?}",
                run.verdict
            );
            assert!(run.findings.is_empty());
            assert_eq!(run.blocking, 0);
        }
    }
}

#[test]
fn a_partial_render_keeps_what_it_found_and_does_not_read_the_rest_as_clean() {
    let dir = demo_chart();
    write(
        dir.path(),
        "templates/service.yaml",
        "apiVersion: v1\nkind: Service\nmetadata:\n  name: {{ .Values.service.name }}\n",
    );
    let chart = read_chart(dir.path()).unwrap();
    let hardened_values: [Value; 1] = [json!({"hardened": true, "rbac": {"wildcard": false}})];
    let clean_but_partial = analyze_chart(&chart, &hardened_values, "rel", &HandAuthoredRender);
    for (path, family, run) in run_all(&clean_but_partial, &grounded, &promoted()) {
        assert!(
            matches!(run.verdict, PackVerdict::Unknown { .. }),
            "{path} {family:?}: nothing observed in a partial render is not clean"
        );
    }

    let violating_and_partial = analyze_chart(&chart, &[], "rel", &HandAuthoredRender);
    let runs = run_all(&violating_and_partial, &grounded, &promoted());
    let found = runs
        .iter()
        .find(|(_, family, run)| {
            *family == StructuralFamily::InfraPermissions
                && matches!(run.verdict, PackVerdict::Found { .. })
        })
        .expect("a finding in a partial render is still a finding");
    assert!(
        found
            .2
            .diagnostics
            .iter()
            .any(|d| d.code == "sast.unresolved_checks"),
        "{:?}",
        found.2.diagnostics
    );
}

#[test]
fn a_complete_render_with_nothing_found_is_scoped_not_a_blanket_clean() {
    let analysis = hardened();
    let runs = run_all(&analysis, &grounded, &promoted());
    assert!(!runs.is_empty());
    for (path, family, run) in &runs {
        let PackVerdict::NotObserved { within } = &run.verdict else {
            panic!("{path} {family:?}: {:?}", run.verdict);
        };
        assert!(!within.is_empty());
        assert!(run.findings.is_empty());
    }
}

#[test]
fn every_check_maps_to_a_family_the_matrix_supports_for_helm() {
    let matrix = SecurityPack::standard().capability_matrix();
    for check in HelmCheck::ALL {
        let family = structural_family(check);
        assert!(family.supports(SourceLanguage::Helm), "{check:?}");
        assert!(
            matches!(
                matrix
                    .get(SourceLanguage::Helm, SecurityFamily::of_structural(family))
                    .unwrap()
                    .support,
                Support::Supported { .. }
            ),
            "{check:?}"
        );
    }
}

#[test]
fn the_matrix_states_what_helm_covers_and_what_it_does_not() {
    let matrix = SecurityPack::standard().capability_matrix();
    for family in HELM_FAMILIES {
        let cell = matrix
            .get(SourceLanguage::Helm, SecurityFamily::of_structural(family))
            .unwrap();
        let Support::Supported {
            backend,
            can_block,
            models,
            ..
        } = &cell.support
        else {
            panic!("{family:?} must be supported for helm");
        };
        assert_eq!(backend, "ast");
        assert!(models.is_none());
        assert_eq!(
            *can_block,
            family != StructuralFamily::InfraWorkloadHardening,
            "{family:?}"
        );
        let limits = cell.limits.join(" | ");
        assert!(limits.contains("helm template"), "{family:?}: {limits}");
        assert!(limits.contains("Unknown"), "{family:?}: {limits}");
    }
    let hardening = matrix
        .get(SourceLanguage::Helm, SecurityFamily::InfraWorkloadHardening)
        .unwrap();
    assert_eq!(hardening.nature, FindingNature::Risk);
    assert_eq!(hardening.blocking, BlockingLevel::AdvisoryOnly);

    let unsupported = [
        SecurityFamily::InfraEncryption,
        SecurityFamily::HardcodedSecret,
        SecurityFamily::UnsafeCode,
        SecurityFamily::of_weakness(Weakness::SqlInjection),
        SecurityFamily::of_weakness(Weakness::CommandInjection),
        SecurityFamily::of_weakness(Weakness::PathTraversal),
    ];
    for family in unsupported {
        let cell = matrix.get(SourceLanguage::Helm, family).unwrap();
        assert!(
            matches!(cell.support, Support::Unsupported { .. }),
            "{family:?} must not claim a Helm model"
        );
    }
}

#[test]
fn the_real_engine_feeds_the_pack_when_it_is_installed() {
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
    let runs = run_all(&analysis, &grounded, &promoted());
    assert!(runs.iter().any(|(_, family, run)| {
        *family == StructuralFamily::InfraPermissions
            && matches!(run.verdict, PackVerdict::Found { .. })
    }));
}
