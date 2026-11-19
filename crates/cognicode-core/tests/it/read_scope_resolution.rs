//! Published read scopes agree with the one affected-work planner and
//! survive their wire form: a scope never says a read is still valid where
//! the planner proves it stale, and a scope that cannot be trusted forces
//! revalidation instead of being extrapolated.

use std::collections::BTreeMap;

use cognicode_core::application::change_tracking::planner::{
    ExecutionDependency, WorkDisposition, WorkId, plan_affected_work,
};
use cognicode_core::application::change_tracking::read_scope::{
    ChangedFacts, READ_SCOPE_SCHEMA, ReadBreadth, ReadReason, ReadScope, ReadVerdict, resolve_reads,
};
use cognicode_core::application::change_tracking::scm_delta::RevisionFacts;
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::evidence_kernel::semantic_diff::compute_fact_delta;
use cognicode_core::domain::kernel_ids::ExecutionId;
use cognicode_core::domain::naming::NamespacedName;
use cognicode_core::domain::readset::ReadSet;
use cognicode_core::domain::scm::PathKey;
use cognicode_core::domain::value_objects::Provenance;

/// Symbols of the fixture: `(file, name, subject entity)`.
const SYMBOLS: [(&str, &str, u64); 3] = [
    ("src/a.rs", "alpha", 10),
    ("src/a.rs", "beta", 20),
    ("src/b.rs", "gamma", 30),
];

fn fact(id: u64, subject: u64, predicate: &str, object: &str, snapshot: u64) -> Fact {
    Fact::new(
        FactId(id),
        EntityId(subject),
        RelationKind::try_new(predicate).expect("predicate"),
        FactValue::Text(object.to_string()),
        SnapshotId(snapshot),
        ProvenanceRecord::new(
            Provenance::Extracted,
            ProducerKind::DeterministicAnalyzer,
            None,
        ),
    )
    .expect("fact")
}

/// One revision: every symbol is defined and calls a callee. The callee of
/// the symbol named by `edited` carries a marker, so the head revision
/// differs from the base in exactly that symbol's call fact.
fn revision(snapshot: u64, edited: Option<&str>) -> RevisionFacts {
    let mut facts = Vec::new();
    let mut path_facts: BTreeMap<PathKey, Vec<FactId>> = BTreeMap::new();
    for (index, (file, name, subject)) in SYMBOLS.iter().enumerate() {
        let defines = 2 * index as u64 + 1;
        let callee = if edited == Some(*name) {
            "callee_v2"
        } else {
            "callee_v1"
        };
        facts.push(fact(
            defines,
            *subject,
            "core:defines",
            &format!("{file}:{name}:{}", index + 1),
            snapshot,
        ));
        facts.push(fact(defines + 1, *subject, "core:calls", callee, snapshot));
        path_facts
            .entry(PathKey::new(file).expect("path"))
            .or_default()
            .extend([FactId(defines), FactId(defines + 1)]);
    }
    RevisionFacts {
        snapshot: SnapshotId(snapshot),
        facts,
        path_facts,
    }
}

fn call_fact_of(name: &str) -> FactId {
    let index = SYMBOLS
        .iter()
        .position(|(_, n, _)| *n == name)
        .expect("symbol");
    FactId(2 * index as u64 + 2)
}

#[test]
fn a_scope_never_says_valid_where_the_planner_proves_the_read_stale() {
    let base = revision(1, None);
    for edited in SYMBOLS.map(|(_, name, _)| name) {
        let head = revision(2, Some(edited));
        let delta = compute_fact_delta(
            base.snapshot,
            base.facts.iter().cloned(),
            head.snapshot,
            head.facts.iter().cloned(),
        )
        .expect("delta");
        let changed = ChangedFacts::from_delta(&delta, &base, &head);

        for read in SYMBOLS.map(|(_, name, _)| name) {
            let fact = call_fact_of(read);
            let dependency = ExecutionDependency {
                work: WorkId::new(NamespacedName::new("detector.reader").expect("name")),
                execution: ExecutionId(0),
                read_set: [fact].into_iter().collect::<ReadSet>(),
                read_scope: None,
            };
            let planned = plan_affected_work(&delta, &[dependency]).decisions[0].disposition;
            let scope = ReadScope::from_reads([fact], false, &base, ReadBreadth::Pointwise);

            let resolution = resolve_reads(Some(&scope), &changed);

            if planned == WorkDisposition::Affected {
                assert_eq!(
                    resolution.verdict,
                    ReadVerdict::Invalidated,
                    "edited {edited}, read {read}"
                );
            } else {
                assert_ne!(
                    resolution.verdict,
                    ReadVerdict::Invalidated,
                    "edited {edited}, read {read}: the planner did not call it stale"
                );
            }
            assert_eq!(
                resolution.verdict == ReadVerdict::Invalidated,
                edited == read,
                "edited {edited}, read {read}"
            );
        }
    }
}

#[test]
fn pointwise_scope_proves_exclusion_but_unscoped_planner_remains_unknown() {
    let base = revision(1, None);
    let mut head = revision(2, None);
    head.facts
        .push(fact(7, 40, "core:defines", "src/b.rs:delta:4", 2));
    head.path_facts
        .get_mut(&PathKey::new("src/b.rs").expect("path"))
        .expect("file")
        .push(FactId(7));
    let delta = compute_fact_delta(
        base.snapshot,
        base.facts.iter().cloned(),
        head.snapshot,
        head.facts.iter().cloned(),
    )
    .expect("delta");
    let changed = ChangedFacts::from_delta(&delta, &base, &head);
    let read = call_fact_of("alpha");
    let scope = ReadScope::from_reads([read], false, &base, ReadBreadth::Pointwise);
    assert_eq!(
        resolve_reads(Some(&scope), &changed).verdict,
        ReadVerdict::Valid
    );
    assert_eq!(
        resolve_reads(None, &changed).verdict,
        ReadVerdict::Revalidate
    );
    let dependency = ExecutionDependency {
        work: WorkId::new(NamespacedName::new("detector.reader").expect("name")),
        execution: ExecutionId(0),
        read_set: [read].into_iter().collect(),
        read_scope: None,
    };
    assert_eq!(
        plan_affected_work(&delta, &[dependency]).decisions[0].disposition,
        WorkDisposition::Unknown
    );
}

#[test]
fn an_untrustworthy_scope_forces_revalidation_over_an_empty_change() {
    let base = revision(1, None);
    let nothing_changed = ChangedFacts::default();
    let sound = ReadScope::from_reads(
        [call_fact_of("alpha")],
        false,
        &base,
        ReadBreadth::Pointwise,
    );

    let mut truncated = sound.clone();
    truncated.truncated = true;
    let mut unknown_schema = sound.clone();
    unknown_schema.schema = format!("{READ_SCOPE_SCHEMA}.next");

    assert_eq!(
        resolve_reads(Some(&sound), &nothing_changed).verdict,
        ReadVerdict::Valid
    );
    for untrusted in [None, Some(&truncated), Some(&unknown_schema)] {
        assert_eq!(
            resolve_reads(untrusted, &nothing_changed).verdict,
            ReadVerdict::Revalidate
        );
    }
}

#[test]
fn a_scope_survives_its_wire_form_and_resolves_the_same_after_it() {
    let base = revision(1, None);
    let head = revision(2, Some("alpha"));
    let delta = compute_fact_delta(
        base.snapshot,
        base.facts.iter().cloned(),
        head.snapshot,
        head.facts.iter().cloned(),
    )
    .expect("delta");
    let changed = ChangedFacts::from_delta(&delta, &base, &head);
    let scope = ReadScope::from_reads(
        [call_fact_of("alpha"), call_fact_of("gamma")],
        false,
        &base,
        ReadBreadth::Pointwise,
    );

    let wire = serde_json::to_string(&scope).expect("serialize");
    let received: ReadScope = serde_json::from_str(&wire).expect("deserialize");

    assert_eq!(received, scope);
    let resolution = resolve_reads(Some(&received), &changed);
    assert_eq!(resolution.verdict, ReadVerdict::Invalidated);
    assert_eq!(resolution.reasons.len(), 1, "{:?}", resolution.reasons);
    assert!(matches!(
        &resolution.reasons[0],
        ReadReason::ReadChanged { read } if read.path == "src/a.rs"
            && read.symbol.as_deref() == Some("alpha")
    ));
    let reasons = serde_json::to_value(&resolution).expect("reasons serialize");
    assert_eq!(reasons["verdict"], "invalidated");
    assert_eq!(reasons["reasons"][0]["reason"], "read_changed");
}
