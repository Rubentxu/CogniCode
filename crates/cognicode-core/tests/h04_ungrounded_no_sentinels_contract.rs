//! H0.4 (delivery-first-radical, package integration) — the seam
//! must not fabricate provenance it does not have.
//!
//! Claim: when `to_guard_observation` receives a
//! `ProjectionError::Inconclusive(Vec<Gap>)`, the input carries only
//! gaps — no `SnapshotId`, no `DetectorExecutionRef`. The output MUST
//! reflect that absence in the type system (a `GuardObservation`
//! variant that does not require those fields). Fabricating
//! `SnapshotId(0)` and a `synthetic_detector()` to fill the
//! non-optional fields of a `GuardObservation::Inconclusive` is the
//! exact "Ungrounded representa causalidad verificada" bug that H0.4
//! calls out: a downstream consumer (Guard) cannot pattern-match on
//! the discriminant alone to know "this is not a real observation",
//! because the shape looks the same as a regular `Inconclusive`.
//!
//! H0.5 acceptance ("`Ungrounded` jamas implica admision") is
//! enforceable only when the type system distinguishes grounded
//! from ungrounded. The tests below pin the structural property at
//! the seam.
//!
//! Test strategy:
//!   - Build `Err(ProjectionError::Inconclusive(vec![...]))` directly
//!     (no need to drive the full pipeline; the seam is pure).
//!   - Assert the discriminant is the new `Ungrounded` variant.
//!   - Assert the variant does not carry `snapshot_id` or
//!     `detector` at all (the test is exhaustive over fields).
//!   - Regression: `Ok(Inconclusive { snapshot_id, detector, gaps })`
//!     still maps to the full-provenance `GuardObservation::Inconclusive`.

use cognicode_core::domain::findings::EvidenceClass;
use cognicode_core::domain::findings::guard_transfer::{GuardObservation, to_guard_observation};
use cognicode_core::domain::findings::projection::{Gap, ProjectionError};
use cognicode_core::domain::kernel_ids::SnapshotId;

fn sample_gaps() -> Vec<Gap> {
    vec![Gap::EvidenceClassBelowThreshold {
        class: EvidenceClass::D,
    }]
}

#[test]
fn err_inconclusive_maps_to_ungrounded_without_synthetic_provenance() {
    // RED: there is no `Ungrounded` variant on `GuardObservation` yet.
    // Today, the seam returns `GuardObservation::Inconclusive` with
    // `SnapshotId(0)` and a synthetic detector — those fields are
    // fabricated; the type does not document the absence.
    let result: Result<
        cognicode_core::domain::findings::projection::FindingProjection,
        ProjectionError,
    > = Err(ProjectionError::Inconclusive(sample_gaps()));
    let observation = to_guard_observation(&result);
    assert!(
        matches!(observation, GuardObservation::Ungrounded { .. }),
        "Err(ProjectionError::Inconclusive(_)) must map to \
         GuardObservation::Ungrounded, got {observation:?}; the current \
         shape fabricates SnapshotId(0) and a synthetic detector (see \
         guard_transfer.rs:128-138), which a downstream consumer cannot \
         distinguish from a real Inconclusive."
    );
}

#[test]
fn ungrounded_carries_gaps_and_nothing_else() {
    // The variant must carry the gaps intact. We assert on the field
    // we know exists, leaving the rest of the variant untestable in
    // a way that proves "there is no snapshot_id, no detector" —
    // the structural test is the discriminant check above; this
    // checks data preservation.
    let result: Result<
        cognicode_core::domain::findings::projection::FindingProjection,
        ProjectionError,
    > = Err(ProjectionError::Inconclusive(sample_gaps()));
    let observation = to_guard_observation(&result);
    match observation {
        GuardObservation::Ungrounded { gaps } => {
            assert_eq!(gaps.len(), 1, "the seam must carry the gaps");
            assert_eq!(
                gaps[0],
                Gap::EvidenceClassBelowThreshold {
                    class: EvidenceClass::D
                }
            );
        }
        other => panic!("expected Ungrounded, got {other:?}"),
    }
}

#[test]
fn ungrounded_cannot_be_misread_as_verifiable() {
    // The strongest property: an Ungrounded must NOT pattern-match as
    // Verifiable. This is the whole point of the variant: the
    // discriminant alone is enough for Guard to refuse to promote.
    let result: Result<
        cognicode_core::domain::findings::projection::FindingProjection,
        ProjectionError,
    > = Err(ProjectionError::Inconclusive(sample_gaps()));
    let observation = to_guard_observation(&result);
    if let GuardObservation::Verifiable { .. } = &observation {
        panic!(
            "Ungrounded must never be confusable with Verifiable; \
             a Guard consumer that pattern-matched on Verifiable would \
             promote evidence the seam could not provenance. Got: {observation:?}"
        );
    }
    if let GuardObservation::Advisory { .. } = &observation {
        panic!("Ungrounded must never be confusable with Advisory; got: {observation:?}");
    }
    if let GuardObservation::Inconclusive { .. } = &observation {
        panic!(
            "Ungrounded must be its own discriminant, distinct from \
             Inconclusive (which carries full provenance). Today the \
             seam fabricates a snapshot_id and a detector to fit \
             Inconclusive, hiding the absence. Got: {observation:?}"
        );
    }
}

#[test]
fn snapshot_id_zero_must_not_appear_anywhere_in_the_seam_output() {
    // Belt and suspenders: even if the discriminant shape were ever
    // refactored, the literal `SnapshotId(0)` sentinel must not leak
    // into a GuardObservation. It is the magic number that hides
    // "I don't have a snapshot".
    let result: Result<
        cognicode_core::domain::findings::projection::FindingProjection,
        ProjectionError,
    > = Err(ProjectionError::Inconclusive(sample_gaps()));
    let observation = to_guard_observation(&result);
    if let GuardObservation::Inconclusive { snapshot_id, .. } = &observation {
        assert_ne!(
            *snapshot_id,
            SnapshotId(0),
            "SnapshotId(0) is the sentinel that must be removed in H0.4"
        );
    }
}

#[test]
fn synthetic_detector_marker_must_not_appear_anywhere_in_the_seam_output() {
    // The other sentinel: the seam fabricates a DetectorExecutionRef
    // with id "synthetic@0.0.0" and an empty SHA-256 digest to fill
    // the non-optional `detector` field of `Inconclusive`. The fix
    // removes this constructor entirely; this test pins the
    // invariant at the observable level (no detector with that id
    // or that digest survives in the seam output).
    let result: Result<
        cognicode_core::domain::findings::projection::FindingProjection,
        ProjectionError,
    > = Err(ProjectionError::Inconclusive(sample_gaps()));
    let observation = to_guard_observation(&result);
    if let GuardObservation::Inconclusive { detector, .. } = &observation {
        let id_str = detector.id.to_string();
        assert!(
            !id_str.starts_with("synthetic@"),
            "synthetic detector id `{id_str}` must not appear in the seam output (H0.4)"
        );
        let zero_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let digests_str = format!("{:?}", detector.digests);
        assert!(
            !digests_str.contains(zero_digest) || detector.context.is_some(),
            "a detector with all-zero digests and no context is the \
             synthetic fallback; it must not appear in seam output (H0.4). \
             Detector: {detector:?}"
        );
    }
}

#[test]
fn ok_inconclusive_with_full_provenance_still_maps_to_inconclusive() {
    // Regression: a real Inconclusive (with snapshot_id + detector)
    // must still land in `GuardObservation::Inconclusive` — not in
    // the new Ungrounded variant. Ungrounded is reserved for the
    // "we lost provenance" case.
    let snapshot = SnapshotId(7);
    let detector = cognicode_core::domain::findings::DetectorExecutionRef {
        id: cognicode_core::domain::findings::DetectorId::new("cyclomatic@1.0.0")
            .expect("valid detector id"),
        version: "1.0".to_string(),
        authority_at_execution: cognicode_core::domain::findings::DetectorAuthority::Gated,
        digests: cognicode_core::domain::findings::DetectorDigests {
            logic: cognicode_core::domain::findings::DetectorDigest::new(
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            )
            .expect("valid digest"),
            policy: cognicode_core::domain::findings::DetectorDigest::new(
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            )
            .expect("valid digest"),
            semantic: cognicode_core::domain::findings::DetectorDigest::new(
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            )
            .expect("valid digest"),
            instance: cognicode_core::domain::findings::DetectorDigest::new(
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            )
            .expect("valid digest"),
        },
        context: None,
    };
    let result: Result<
        cognicode_core::domain::findings::projection::FindingProjection,
        ProjectionError,
    > = Ok(
        cognicode_core::domain::findings::projection::FindingProjection::Inconclusive {
            snapshot_id: snapshot,
            detector: detector.clone(),
            gaps: sample_gaps(),
        },
    );
    let observation = to_guard_observation(&result);
    match observation {
        GuardObservation::Inconclusive {
            snapshot_id,
            detector: got,
            gaps,
        } => {
            assert_eq!(snapshot_id, snapshot, "snapshot must round-trip");
            assert_eq!(got.id, detector.id, "detector must round-trip");
            assert_eq!(gaps.len(), 1);
        }
        other => panic!(
            "Ok(Inconclusive {{ snapshot_id, detector, gaps }}) must map to \
             GuardObservation::Inconclusive (full provenance), got {other:?}"
        ),
    }
}
