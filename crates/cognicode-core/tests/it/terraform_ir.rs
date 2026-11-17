//! The static Terraform IR over real module and variable fixtures.
//!
//! Two committed fixtures are read from disk with the production loader:
//!
//! - `terraform-iac`: a realistic project with local modules whose values
//!   flow between them through variables and outputs;
//! - `terraform-risk`: a project built to separate a verifiable risk from a
//!   value the configuration does not fix.
//!
//! Nothing here runs `terraform`, a provider or a module, and no credential
//! is read: the tests pin what is Known, what is Unknown, and why.

use std::collections::BTreeSet;
use std::path::PathBuf;

use cognicode_core::application::findings::{PackRun, RunAuthority, RunContext, SecurityPack};
use cognicode_core::domain::execution::CorrelationId;
use cognicode_core::domain::findings::semantic_model::{
    FindingNature, ObservedUnit, PackVerdict, StructuralFact, StructuralFamily,
};
use cognicode_core::domain::findings::{AnalysisScope, GroundingRef};
use cognicode_core::domain::iac::terraform::*;
use cognicode_core::domain::kernel_ids::{EntityId, ExecutionId, FactId, SnapshotId};
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;
use cognicode_core::infrastructure::terraform_project::{LoadedTerraform, load_terraform_project};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sandbox/fixtures")
}

fn load(name: &str) -> LoadedTerraform {
    let path = fixtures().join(name);
    load_terraform_project(&path).unwrap_or_else(|e| {
        panic!(
            "the fixture {} is versioned in the repository: {e}",
            path.display()
        )
    })
}

fn analyse(name: &str) -> TfAnalysis {
    let loaded = load(name);
    TfAnalysis::run(&loaded.project, "", loaded.bindings)
}

fn addr(text: &str) -> NodeAddr {
    NodeAddr::new(text)
}

fn outcome<'a>(analysis: &'a TfAnalysis, resource: &str, attribute: &str) -> &'a Outcome {
    &analysis
        .results()
        .find(|r| r.resource == resource && r.attribute == attribute)
        .unwrap_or_else(|| panic!("no check for {resource} {attribute}"))
        .outcome
}

fn is_unbound(outcome: &Outcome, variable: &str) -> bool {
    matches!(outcome, Outcome::Unverified { causes }
        if causes.iter().any(|c| matches!(c, UnknownCause::UnboundVariable { address, .. } if address == variable)))
}

// ---------------------------------------------------------------------------
// AC1 - module and variable fixtures give resource graphs and an explainable
// reference flow.
// ---------------------------------------------------------------------------

#[test]
fn a_variable_flows_into_a_module_resource_and_the_flow_is_explained() {
    let analysis = analyse("terraform-risk");
    let acl = addr("module.storage.aws_s3_bucket.data#acl");

    let explanation = analysis
        .explain(&acl)
        .expect("the attribute is in the graph");
    assert_eq!(explanation.value, TfValue::Str("private".to_string()));
    assert!(explanation.causes.is_empty());
    assert_eq!(
        explanation.flow,
        vec![
            FlowEdge {
                from: acl.clone(),
                to: addr("module.storage.var.acl")
            },
            FlowEdge {
                from: addr("module.storage.var.acl"),
                to: addr("var.bucket_acl")
            },
        ],
        "the module variable reads the root variable the call passes"
    );
    assert_eq!(
        analysis.graph().path(&acl, &addr("var.bucket_acl")),
        Some(vec![
            acl,
            addr("module.storage.var.acl"),
            addr("var.bucket_acl")
        ])
    );
}

#[test]
fn a_chain_through_a_local_and_a_module_variable_reaches_the_root_variable() {
    let analysis = analyse("terraform-risk");
    let cidr = addr("module.network.aws_security_group.admin#ingress[0].cidr_blocks");
    let path = analysis
        .graph()
        .path(&cidr, &addr("var.ssh_cidr"))
        .expect("the value comes from the root variable");
    assert_eq!(
        path,
        vec![
            cidr,
            addr("module.network.local.admin_sources"),
            addr("module.network.var.admin_cidr"),
            addr("var.ssh_cidr"),
        ]
    );
}

#[test]
fn an_output_of_one_module_feeds_a_variable_of_another_in_the_real_project() {
    let analysis = analyse("terraform-iac");
    let graph = analysis.graph();
    assert_eq!(
        graph
            .node(&addr("module.compute.var.vpc_id"))
            .unwrap()
            .deps
            .iter()
            .collect::<Vec<_>>(),
        vec![&addr("module.vpc.output.vpc_id")]
    );
    assert!(
        graph
            .path(
                &addr("module.compute.aws_security_group.app#vpc_id"),
                &addr("module.vpc.aws_vpc.main")
            )
            .is_some(),
        "a resource of one module reaches a resource of the other through the outputs"
    );
    assert!(
        graph
            .path(&addr("output.vpc_id"), &addr("module.vpc.aws_vpc.main"))
            .is_some(),
        "a root output reaches the resource it exposes"
    );
}

#[test]
fn a_computed_attribute_is_unknown_with_its_cause_and_never_invented() {
    let analysis = analyse("terraform-iac");
    let value = analysis
        .value(&addr("module.compute.var.vpc_id"))
        .expect("evaluated");
    assert!(
        matches!(value, TfValue::Unknown(causes)
            if causes.iter().any(|c| matches!(c, UnknownCause::ComputedAttribute { address, .. } if address == "module.vpc.aws_vpc.main"))),
        "the id of a resource is assigned by the provider, got {value:?}"
    );
}

#[test]
fn the_graph_agrees_with_the_references_the_file_handler_extracts() {
    use cognicode_core::application::ingest::extractor::extract_file;
    use cognicode_core::infrastructure::parser::language_config::HCL_CONFIG;

    let source = std::fs::read_to_string(fixtures().join("terraform-iac/main.tf")).unwrap();
    let extracted = extract_file(
        &HCL_CONFIG,
        std::path::Path::new("main.tf"),
        &source,
        "terraform-ir",
    );
    let handler_edges: BTreeSet<(String, String)> = extracted
        .edges
        .iter()
        .filter(|e| e.kind == "dependency.references")
        .filter_map(|e| {
            let to = match &e.target_ref {
                cognicode_core::application::ingest::types::TargetRef::Resolved(r)
                | cognicode_core::application::ingest::types::TargetRef::Unresolved(r) => r,
            };
            Some((
                e.source.strip_prefix("tf:main.tf:")?.to_string(),
                to.clone(),
            ))
        })
        .filter(|(from, _)| from == "aws_s3_bucket.app_bucket")
        .collect();
    assert!(
        !handler_edges.is_empty(),
        "the handler sees the bucket's references"
    );

    let analysis = analyse("terraform-iac");
    let graph = analysis.graph();
    let bucket = addr("aws_s3_bucket.app_bucket");
    let graph_reads: BTreeSet<String> = analysis
        .graph()
        .flow_from(&bucket)
        .into_iter()
        .filter(|edge| edge.to.as_str().starts_with("var."))
        .map(|edge| edge.to.to_string())
        .collect();
    assert!(graph.node(&bucket).is_some());
    let handler_variables: BTreeSet<&str> = handler_edges
        .iter()
        .map(|(_, to)| to.strip_prefix("tf:main.tf:").unwrap_or(to))
        .filter(|to| to.starts_with("var."))
        .collect();
    assert!(
        handler_variables.contains("var.environment"),
        "the handler reads var.environment in the bucket: {handler_edges:?}"
    );
    for variable in &handler_variables {
        assert!(
            graph_reads.contains(*variable),
            "the handler links the bucket to {variable} and the graph must too"
        );
    }
    assert!(graph_reads.contains("var.environment"));
    assert!(graph_reads.contains("var.project_name"));
}

// ---------------------------------------------------------------------------
// AC2 - rules separate a verifiable risk from an Unknown value.
// ---------------------------------------------------------------------------

#[test]
fn literal_misconfigurations_are_verified_risks() {
    let analysis = analyse("terraform-risk");
    for (resource, attribute) in [
        ("aws_db_instance.main", "publicly_accessible"),
        ("aws_db_instance.main", "storage_encrypted"),
        ("aws_s3_bucket.logs", "acl"),
        (
            "module.network.aws_security_group.open",
            "ingress[0].cidr_blocks",
        ),
    ] {
        assert!(
            matches!(
                outcome(&analysis, resource, attribute),
                Outcome::Violation { .. }
            ),
            "{resource} {attribute}"
        );
    }
}

#[test]
fn a_value_the_configuration_does_not_fix_is_unverified_not_clean_and_not_a_violation() {
    let analysis = analyse("terraform-risk");
    assert!(is_unbound(
        outcome(
            &analysis,
            "module.network.aws_security_group.admin",
            "ingress[0].cidr_blocks"
        ),
        "var.ssh_cidr"
    ));
    assert!(is_unbound(
        outcome(&analysis, "aws_efs_file_system.shared", "encrypted"),
        "var.audit_enabled"
    ));
    assert!(is_unbound(
        outcome(&analysis, "aws_iam_policy.ops", "policy"),
        "var.admin_action"
    ));
}

#[test]
fn a_variable_bound_by_tfvars_is_known_and_satisfies_the_check() {
    let analysis = analyse("terraform-risk");
    assert_eq!(
        analysis.value(&addr("var.bucket_acl")),
        Some(&TfValue::Str("private".to_string()))
    );
    assert_eq!(
        outcome(&analysis, "module.storage.aws_s3_bucket.data", "acl"),
        &Outcome::Satisfied
    );
    assert_eq!(
        outcome(
            &analysis,
            "module.storage.aws_ebs_volume.scratch",
            "encrypted"
        ),
        &Outcome::Satisfied
    );
}

#[test]
fn public_https_is_satisfied_by_design_while_rdp_from_the_world_is_a_violation() {
    let analysis = analyse("terraform-risk");
    assert_eq!(
        outcome(
            &analysis,
            "module.network.aws_security_group.open",
            "ingress[1].cidr_blocks"
        ),
        &Outcome::Satisfied
    );
}

#[test]
fn a_declared_default_is_not_assumed_unless_the_caller_opts_in() {
    let loaded = load("terraform-risk");
    let plain = TfAnalysis::run(&loaded.project, "", loaded.bindings.clone());
    let environment = addr("var.environment");
    assert!(plain.value(&environment).unwrap().is_unknown());
    assert!(plain.unbound_variables().contains(&environment));

    let defaulted = TfAnalysis::run(&loaded.project, "", loaded.bindings.apply_defaults());
    assert_eq!(
        defaulted.value(&environment),
        Some(&TfValue::Str("dev".to_string()))
    );
    assert!(
        defaulted
            .unbound_variables()
            .iter()
            .all(|v| v != &environment)
    );
    assert!(
        defaulted
            .unbound_variables()
            .contains(&addr("var.ssh_cidr")),
        "a variable with no default and no input stays unknown"
    );
}

#[test]
fn coverage_lists_what_the_analysis_could_not_open_instead_of_calling_it_clean() {
    let analysis = analyse("terraform-risk");
    let coverage = analysis.coverage();
    assert_eq!(
        coverage
            .unresolved_modules
            .iter()
            .map(|m| (m.address.to_string(), m.source.as_str()))
            .collect::<Vec<_>>(),
        vec![(
            "module.shared".to_string(),
            "terraform-aws-modules/s3-bucket/aws"
        )],
        "a registry module is never fetched or guessed"
    );
    assert!(!coverage.is_complete());
    assert!(coverage.unread.is_empty());

    let real = analyse("terraform-iac").coverage();
    assert!(
        real.assessed < real.resources,
        "the real project has resources of types no check models"
    );
    assert!(!real.not_evaluated_types.is_empty());
    assert!(!real.is_complete());
}

// ---------------------------------------------------------------------------
// AC2 through the findings seam - only verified facts become findings, and a
// block needs a promoted detector plus grounded evidence.
// ---------------------------------------------------------------------------

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
        correlation: CorrelationId::new("terraform-ir-test").unwrap(),
        authority,
    }
}

fn promoted() -> RunAuthority {
    RunAuthority::Promoted {
        approver: "security-team".to_string(),
    }
}

fn run_pack(family: StructuralFamily, unit: &ObservedUnit, authority: RunAuthority) -> PackRun {
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    SecurityPack::standard()
        .analyse_structural(family, unit, &context(authority), &mut store)
        .expect("the run must be carried out")
}

fn grounded(_file: &str, line: u32) -> Option<GroundingRef> {
    Some(GroundingRef::entity(
        EntityId::new(u64::from(line)),
        FactId::new(900 + u64::from(line)),
    ))
}

fn ungrounded(_file: &str, _line: u32) -> Option<GroundingRef> {
    None
}

fn unit_of<'a>(units: &'a [ObservedUnit], path: &str) -> &'a ObservedUnit {
    units
        .iter()
        .find(|u| u.path == path)
        .unwrap_or_else(|| panic!("no unit for {path}"))
}

#[test]
fn a_promoted_grounded_verified_risk_blocks() {
    let analysis = analyse("terraform-risk");
    let units = observed_units(&analysis, &grounded);
    let unit = unit_of(&units, "main.tf");

    let run = run_pack(StructuralFamily::InfraPublicExposure, unit, promoted());
    assert!(matches!(
        run.verdict,
        PackVerdict::Found {
            nature: FindingNature::Risk | FindingNature::Vulnerability,
            findings: 2
        }
    ));
    assert_eq!(run.blocking, 2, "the open database and the public ACL");
}

#[test]
fn a_candidate_detector_or_ungrounded_evidence_finds_but_never_blocks() {
    let analysis = analyse("terraform-risk");

    let units = observed_units(&analysis, &grounded);
    let candidate = run_pack(
        StructuralFamily::InfraPublicExposure,
        unit_of(&units, "main.tf"),
        RunAuthority::Candidate,
    );
    assert_eq!(candidate.findings.len(), 2);
    assert_eq!(candidate.blocking, 0);

    let units = observed_units(&analysis, &ungrounded);
    let floating = run_pack(
        StructuralFamily::InfraPublicExposure,
        unit_of(&units, "main.tf"),
        promoted(),
    );
    assert_eq!(floating.findings.len(), 2);
    assert_eq!(floating.blocking, 0);
}

#[test]
fn an_unknown_value_alone_is_unknown_and_never_a_finding_or_a_pass() {
    let analysis = analyse("terraform-risk");
    let units = observed_units(&analysis, &grounded);
    let unit = unit_of(&units, "main.tf");

    let run = run_pack(StructuralFamily::InfraPermissions, unit, promoted());
    assert!(run.findings.is_empty());
    assert_eq!(run.blocking, 0);
    match run.verdict {
        PackVerdict::Unknown { unresolved } => {
            assert_eq!(unresolved.len(), 1);
            assert!(unresolved[0].contains("aws_iam_policy.ops"));
            assert!(unresolved[0].contains("var.admin_action"));
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}

#[test]
fn a_verified_risk_does_not_hide_the_checks_that_could_not_decide() {
    let analysis = analyse("terraform-risk");
    let units = observed_units(&analysis, &grounded);
    let run = run_pack(
        StructuralFamily::InfraEncryption,
        unit_of(&units, "main.tf"),
        promoted(),
    );
    assert!(matches!(
        run.verdict,
        PackVerdict::Found { findings: 1, .. }
    ));
    assert!(
        run.diagnostics
            .iter()
            .any(|d| d.code == "sast.unresolved_checks" && d.message.contains("var.audit_enabled"))
    );
}

#[test]
fn a_unit_with_only_satisfied_checks_is_not_observed_within_scope() {
    let analysis = analyse("terraform-risk");
    let units = observed_units(&analysis, &grounded);
    let unit = unit_of(&units, "modules/storage/main.tf");
    for family in [
        StructuralFamily::InfraPublicExposure,
        StructuralFamily::InfraEncryption,
        StructuralFamily::InfraPermissions,
    ] {
        let run = run_pack(family, unit, promoted());
        assert!(run.findings.is_empty(), "{family:?}");
        assert!(
            matches!(run.verdict, PackVerdict::NotObserved { .. }),
            "{family:?}: {:?}",
            run.verdict
        );
    }
}

#[test]
fn committed_secrets_are_found_through_the_shared_detector_and_never_echoed() {
    let analysis = analyse("terraform-risk");
    let units = observed_units(&analysis, &grounded);
    let unit = unit_of(&units, "main.tf");
    let run = run_pack(StructuralFamily::HardcodedSecret, unit, promoted());
    assert!(matches!(run.verdict, PackVerdict::Found { .. }));
    assert!(
        run.findings.len() >= 2,
        "the database password and a provider key"
    );
    let rendered = format!("{:?} {:?}", run.findings, run.diagnostics);
    for secret in [
        "Sup3rS3cretPassw0rd!",
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
    ] {
        assert!(
            !rendered.contains(secret),
            "the finding must not echo a secret"
        );
    }

    let facts: Vec<_> = unit
        .facts
        .iter()
        .filter(|f| matches!(f.fact, StructuralFact::StringBinding { .. }))
        .collect();
    assert!(
        facts.iter().all(|f| f.line > 0),
        "every secret is anchored to a line"
    );
}

#[test]
fn a_value_that_flows_from_a_variable_is_not_a_committed_secret() {
    let analysis = analyse("terraform-iac");
    let units = observed_units(&analysis, &grounded);
    let flowing = units
        .iter()
        .flat_map(|u| &u.facts)
        .filter(
            |f| matches!(&f.fact, StructuralFact::StringBinding { name, .. } if name == "password"),
        )
        .count();
    let literal_defaults = ["variables.tf"]
        .iter()
        .filter_map(|p| units.iter().find(|u| u.path == *p))
        .flat_map(|u| &u.facts)
        .filter(|f| {
            matches!(&f.fact, StructuralFact::StringBinding { name, .. } if name == "db_password")
        })
        .count();
    assert_eq!(
        flowing, 0,
        "`password = var.db_password` is a read, not a literal"
    );
    assert_eq!(
        literal_defaults, 1,
        "the committed default of the variable is the secret"
    );
}

// ---------------------------------------------------------------------------
// AC3 - changing a variable invalidates only what depends on it.
// ---------------------------------------------------------------------------

fn nodes(analysis: &TfAnalysis) -> Vec<(NodeAddr, Option<TfValue>)> {
    analysis
        .graph()
        .nodes()
        .map(|n| (n.addr.clone(), analysis.value(&n.addr).cloned()))
        .collect()
}

#[test]
fn changing_a_variable_recomputes_only_its_readers_and_reassesses_only_their_resources() {
    let loaded = load("terraform-risk");
    let before = TfAnalysis::run(&loaded.project, "", loaded.bindings.clone());
    let bindings = loaded
        .bindings
        .clone()
        .with("ssh_cidr", TfValue::Str("0.0.0.0/0".to_string()));
    let (after, report) = before.rebind(bindings);

    let expected: BTreeSet<NodeAddr> = [
        "var.ssh_cidr",
        "module.network.var.admin_cidr",
        "module.network.local.admin_sources",
        "module.network.aws_security_group.admin#ingress[0].cidr_blocks",
    ]
    .map(addr)
    .into();
    assert!(
        expected.is_subset(&report.recomputed),
        "{:?}",
        report.recomputed
    );
    assert!(
        report
            .recomputed
            .iter()
            .all(|a| a.as_str() == "var.ssh_cidr" || a.as_str().starts_with("module.network")),
        "nothing outside the readers of the variable is recomputed: {:?}",
        report.recomputed
    );
    assert_eq!(
        report.reassessed,
        BTreeSet::from([addr("module.network.aws_security_group.admin")])
    );
    for untouched in [
        "aws_db_instance.main",
        "aws_s3_bucket.logs",
        "aws_efs_file_system.shared",
        "aws_iam_policy.ops",
        "module.storage.aws_s3_bucket.data",
        "module.storage.aws_ebs_volume.scratch",
        "module.network.aws_security_group.open",
    ] {
        assert!(report.reused.contains(&addr(untouched)), "{untouched}");
    }

    assert!(
        matches!(
            outcome(
                &after,
                "module.network.aws_security_group.admin",
                "ingress[0].cidr_blocks"
            ),
            Outcome::Violation { .. }
        ),
        "the SSH rule is now verifiably open"
    );
    assert!(matches!(
        outcome(
            &before,
            "module.network.aws_security_group.admin",
            "ingress[0].cidr_blocks"
        ),
        Outcome::Unverified { .. }
    ));
}

#[test]
fn an_incremental_rebind_equals_analysing_from_scratch() {
    let loaded = load("terraform-risk");
    let before = TfAnalysis::run(&loaded.project, "", loaded.bindings.clone());
    for (name, value) in [
        ("ssh_cidr", TfValue::Str("10.0.0.0/8".to_string())),
        ("bucket_acl", TfValue::Str("public-read".to_string())),
        ("encrypt_volumes", TfValue::Bool(false)),
        ("audit_enabled", TfValue::Bool(false)),
        ("admin_action", TfValue::Str("*".to_string())),
    ] {
        let bindings = loaded.bindings.clone().with(name, value);
        let (incremental, _) = before.rebind(bindings.clone());
        let scratch = TfAnalysis::run(&loaded.project, "", bindings);
        assert_eq!(nodes(&incremental), nodes(&scratch), "{name}");
        assert_eq!(
            incremental.results().collect::<Vec<_>>(),
            scratch.results().collect::<Vec<_>>(),
            "{name}"
        );
        assert_eq!(incremental.coverage(), scratch.coverage(), "{name}");
    }
}

#[test]
fn rebinding_to_the_same_values_recomputes_nothing_and_keeps_every_projection() {
    let loaded = load("terraform-risk");
    let before = TfAnalysis::run(&loaded.project, "", loaded.bindings.clone());
    let (after, report) = before.rebind(loaded.bindings.clone());
    assert!(report.recomputed.is_empty());
    assert!(report.reassessed.is_empty());
    assert_eq!(
        after.results().collect::<Vec<_>>(),
        before.results().collect::<Vec<_>>()
    );
}

#[test]
fn a_change_reaches_through_a_module_boundary_to_the_resource_and_stops_there() {
    let loaded = load("terraform-risk");
    let before = TfAnalysis::run(&loaded.project, "", loaded.bindings.clone());
    let (after, report) = before.rebind(
        loaded
            .bindings
            .clone()
            .with("bucket_acl", TfValue::Str("public-read".to_string())),
    );
    assert_eq!(
        report.reassessed,
        BTreeSet::from([addr("module.storage.aws_s3_bucket.data")]),
        "the encrypted volume of the same module does not read the ACL"
    );
    assert!(matches!(
        outcome(&after, "module.storage.aws_s3_bucket.data", "acl"),
        Outcome::Violation { .. }
    ));
    assert_eq!(
        outcome(&after, "module.storage.aws_ebs_volume.scratch", "encrypted"),
        &Outcome::Satisfied
    );
}

#[test]
fn the_projection_of_a_rebound_analysis_changes_only_for_the_affected_unit() {
    let loaded = load("terraform-risk");
    let before = TfAnalysis::run(&loaded.project, "", loaded.bindings.clone());
    let (after, _) = before.rebind(
        loaded
            .bindings
            .clone()
            .with("ssh_cidr", TfValue::Str("0.0.0.0/0".to_string())),
    );
    let a = observed_units(&before, &grounded);
    let b = observed_units(&after, &grounded);
    let changed: Vec<&str> = a
        .iter()
        .zip(&b)
        .filter(|(x, y)| x != y)
        .map(|(x, _)| x.path.as_str())
        .collect();
    assert_eq!(changed, vec!["modules/network/main.tf"]);
}

// ---------------------------------------------------------------------------
// AC4 - no plan, no credentials, no untrusted module code.
// ---------------------------------------------------------------------------

#[test]
fn a_module_outside_the_project_is_not_read_and_the_analysis_says_so() {
    let dir = tempdir("escape");
    let outside = dir.join("outside");
    let root = dir.join("root");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        outside.join("main.tf"),
        "resource \"aws_db_instance\" \"leak\" {\n  publicly_accessible = true\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("main.tf"),
        "module \"x\" {\n  source = \"../outside\"\n}\n",
    )
    .unwrap();

    let loaded = load_terraform_project(&root).unwrap();
    let analysis = TfAnalysis::run(&loaded.project, "", loaded.bindings);
    assert_eq!(
        analysis.results().count(),
        0,
        "the outside module is not read"
    );
    let coverage = analysis.coverage();
    assert_eq!(coverage.unresolved_modules.len(), 1);
    assert!(!coverage.is_complete());
    std::fs::remove_dir_all(&dir).ok();
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_project_is_not_followed() {
    let dir = tempdir("symlink");
    let outside = dir.join("outside");
    let root = dir.join("root");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(root.join("modules")).unwrap();
    std::fs::write(
        outside.join("main.tf"),
        "resource \"aws_db_instance\" \"leak\" {\n  publicly_accessible = true\n}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, root.join("modules/linked")).unwrap();
    std::fs::write(
        root.join("main.tf"),
        "module \"x\" {\n  source = \"./modules/linked\"\n}\n",
    )
    .unwrap();

    let loaded = load_terraform_project(&root).unwrap();
    let analysis = TfAnalysis::run(&loaded.project, "", loaded.bindings);
    assert_eq!(analysis.results().count(), 0);
    assert!(!analysis.coverage().is_complete());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn json_configuration_and_broken_files_are_reported_unread_never_clean() {
    let dir = tempdir("unread");
    std::fs::write(dir.join("main.tf"), "resource \"aws_s3_bucket\" \"b\" {\n").unwrap();
    std::fs::write(dir.join("extra.tf.json"), "{}").unwrap();
    std::fs::write(
        dir.join("ok.tf"),
        "resource \"aws_s3_bucket\" \"ok\" {\n  acl = \"private\"\n}\n",
    )
    .unwrap();

    let loaded = load_terraform_project(&dir).unwrap();
    let paths: BTreeSet<&str> = loaded
        .project
        .unread
        .iter()
        .map(|u| u.path.as_str())
        .collect();
    assert!(paths.contains("main.tf"), "{paths:?}");
    assert!(paths.contains("extra.tf.json"), "{paths:?}");

    let analysis = TfAnalysis::run(&loaded.project, "", loaded.bindings);
    assert_eq!(
        outcome(&analysis, "aws_s3_bucket.ok", "acl"),
        &Outcome::Satisfied,
        "the readable file is still analysed"
    );
    assert!(!analysis.coverage().is_complete());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_tfvars_value_that_is_not_a_constant_is_not_guessed() {
    let dir = tempdir("tfvars");
    std::fs::write(
        dir.join("main.tf"),
        "variable \"cidr\" {\n  type = string\n}\nresource \"aws_security_group\" \"g\" {\n  ingress {\n    from_port = 22\n    to_port = 22\n    protocol = \"tcp\"\n    cidr_blocks = [var.cidr]\n  }\n}\n",
    )
    .unwrap();
    std::fs::write(dir.join("terraform.tfvars"), "cidr = file(\"cidr.txt\")\n").unwrap();

    let loaded = load_terraform_project(&dir).unwrap();
    assert!(
        loaded
            .project
            .unread
            .iter()
            .any(|u| u.path == "terraform.tfvars")
    );
    let analysis = TfAnalysis::run(&loaded.project, "", loaded.bindings);
    assert!(matches!(
        outcome(&analysis, "aws_security_group.g", "ingress[0].cidr_blocks"),
        Outcome::Unverified { .. }
    ));
    std::fs::remove_dir_all(&dir).ok();
}

fn tempdir(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "cognicode-terraform-ir-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}
