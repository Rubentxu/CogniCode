use cognicode_core::application::architecture::evolution::{
    EvolutionStatus, query_architecture_evolution,
};
use cognicode_core::domain::scm::fake::FakeSource;
use cognicode_core::domain::scm::{ProviderCapabilities, RevisionReader, SourceRef};

#[test]
fn a_real_architecture_violation_survives_line_and_module_moves_without_new_regression() {
    let provider = FakeSource::new("fake", ProviderCapabilities::NONE)
        .with_revision(
            "base",
            &[],
            &[(
                "crates/service/src/domain/service.rs",
                b"use crate::infrastructure::db::Db;\npub fn connect() -> Db { unimplemented!() }\n",
            )],
        )
        .with_revision(
            "candidate",
            &["base"],
            &[(
                "crates/service/src/domain/service/mod.rs",
                b"\nuse crate::infrastructure::db::Db;\npub fn connect() -> Db { unimplemented!() }\n",
            )],
        );
    let source = SourceRef::new("fake", "workspace").expect("source ref");
    let base = provider.resolve(&source, "base").expect("base revision");
    let candidate = provider
        .resolve(&source, "candidate")
        .expect("candidate revision");

    let answer = query_architecture_evolution(&provider, &base, &candidate);

    assert_eq!(
        answer.producer,
        "cognicode.application.architecture.ControlQueryService"
    );
    assert_eq!(answer.base.revision_ref, "base");
    assert_eq!(answer.candidate.revision_ref, "candidate");
    assert_eq!(answer.base.coverage, "full");
    assert_eq!(answer.candidate.coverage, "full");
    assert_eq!(answer.base.evaluated_source_files, 1);
    assert_eq!(answer.candidate.evaluated_source_files, 1);
    assert_ne!(answer.base.content_digest, answer.candidate.content_digest);
    assert_eq!(answer.evolution.status, EvolutionStatus::Evaluated);
    assert!(answer.evolution.added_violations.is_empty());
    assert!(answer.evolution.removed_violations.is_empty());
    assert_eq!(
        answer.metric_evolution.producer,
        "cognicode.infrastructure.native_metrics"
    );
    assert_eq!(answer.metric_evolution.metric, "cyclomatic_complexity_sum");
    assert_eq!(answer.metric_evolution.unit, "paths");
    assert_eq!(answer.metric_evolution.status, EvolutionStatus::Evaluated);
    assert_eq!(answer.metric_evolution.base.cyclomatic_paths, Some(1));
    assert_eq!(answer.metric_evolution.candidate.cyclomatic_paths, Some(1));
    assert_eq!(answer.metric_evolution.delta, Some(0));
}

#[test]
fn a_new_branch_increases_the_existing_cyclomatic_metric() {
    let provider = FakeSource::new("fake", ProviderCapabilities::NONE)
        .with_revision("base", &[], &[("src/lib.rs", b"pub fn run() {}\n")])
        .with_revision(
            "candidate",
            &["base"],
            &[("src/lib.rs", b"pub fn run() { if ready() {} }\n")],
        );
    let source = SourceRef::new("fake", "workspace").expect("source ref");
    let base = provider.resolve(&source, "base").expect("base revision");
    let candidate = provider
        .resolve(&source, "candidate")
        .expect("candidate revision");

    let answer = query_architecture_evolution(&provider, &base, &candidate);

    assert_eq!(answer.metric_evolution.status, EvolutionStatus::Evaluated);
    assert_eq!(answer.metric_evolution.base.cyclomatic_paths, Some(1));
    assert_eq!(answer.metric_evolution.candidate.cyclomatic_paths, Some(2));
    assert_eq!(answer.metric_evolution.delta, Some(1));
}

#[test]
fn unreadable_revision_content_keeps_evolution_unknown() {
    let provider = FakeSource::new("fake", ProviderCapabilities::NONE)
        .with_revision(
            "base",
            &[],
            &[("src/lib.rs", b"use crate::domain::service;\n")],
        )
        .with_revision("candidate", &["base"], &[("src/lib.rs", &[0xff, 0xfe])]);
    let source = SourceRef::new("fake", "workspace").expect("source ref");
    let base = provider.resolve(&source, "base").expect("base revision");
    let candidate = provider
        .resolve(&source, "candidate")
        .expect("candidate revision");

    let answer = query_architecture_evolution(&provider, &base, &candidate);

    assert_eq!(answer.evolution.status, EvolutionStatus::Unknown);
    assert_eq!(answer.candidate.coverage, "partial");
    assert_eq!(answer.metric_evolution.status, EvolutionStatus::Unknown);
    assert_eq!(answer.metric_evolution.delta, None);
    assert!(
        answer
            .candidate
            .gaps
            .iter()
            .any(|gap| gap.contains("UTF-8"))
    );
    assert!(answer.evolution.added_violations.is_empty());
}

#[test]
fn oversized_revision_is_partial_and_does_not_publish_source_bytes() {
    const MAX_FILE_BYTES: usize = 256 * 1024;
    let private_canary = "private-source-canary";
    let mut oversized = private_canary.as_bytes().to_vec();
    oversized.resize(MAX_FILE_BYTES + 1, b'x');
    let provider = FakeSource::new("fake", ProviderCapabilities::NONE)
        .with_revision("base", &[], &[("src/lib.rs", b"pub fn run() {}\n")])
        .with_revision(
            "candidate",
            &["base"],
            &[("src/lib.rs", oversized.as_slice())],
        );
    let source = SourceRef::new("fake", "workspace").expect("source ref");
    let base = provider.resolve(&source, "base").expect("base revision");
    let candidate = provider
        .resolve(&source, "candidate")
        .expect("candidate revision");

    let answer = query_architecture_evolution(&provider, &base, &candidate);

    assert_eq!(answer.candidate.coverage, "partial");
    assert_eq!(answer.evolution.status, EvolutionStatus::Unknown);
    assert_eq!(answer.metric_evolution.status, EvolutionStatus::Unknown);
    assert!(
        answer
            .candidate
            .gaps
            .iter()
            .any(|gap| gap.contains("per-file limit"))
    );
    let serialized = serde_json::to_string(&answer).expect("serialize bounded result");
    assert!(!serialized.contains(private_canary));
}
