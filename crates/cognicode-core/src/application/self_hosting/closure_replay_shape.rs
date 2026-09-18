//! Closure gate tests for `HistoricalReplay` (e76 WU4 cross-WS).
//!
//! `HistoricalReplay` is the bridge between WU1 (baseline) / WU2
//! (prediction) and the user-driven closure follow-up. Its API
//! must:
//!
//! - Accept a snapshot digest (conventionally a baseline hex).
//! - Surface observations through the public field.
//! - Optionally carry a sealed prediction — and the
//!   presence/absence must be a first-class shape, not a sentinel
//!   "empty" value.
//! - Preserve the sealed prediction's content binding across
//!   construction.
//!
//! These tests do NOT assert runtime scoring behaviour —
//! `compare_replay` is `todo!()` by design. They assert SHAPE: that
//! the public shape is sufficient for a follow-up cycle to extend
//! without a breaking change.

#![cfg(feature = "evidence-kernel")]

use crate::application::self_hosting::platform_equivalence::{
    HistoricalReplay, PlatformKind, PlatformObservation,
};
use crate::application::self_hosting::prediction::{ExpectedObservation, SealedPrediction};

#[test]
fn closure_replay_observation_only_carries_no_sealed_prediction() {
    let replay = HistoricalReplay::observation_only("a".repeat(128), Vec::new());
    assert_eq!(replay.snapshot_digest.len(), 128);
    assert!(
        replay
            .snapshot_digest
            .chars()
            .all(|c| c.is_ascii_hexdigit()),
        "snapshot_digest must be hex"
    );
    assert!(
        replay.sealed_prediction.is_none(),
        "observation_only() must carry no sealed prediction"
    );
    assert_eq!(replay.observations.len(), 0);
}

#[test]
fn closure_replay_with_sealed_prediction_distinguishes_from_observation_only() {
    let only_obs = HistoricalReplay::observation_only("b".repeat(128), Vec::new());
    let pred = SealedPrediction::seal(
        "e76-replay-smoke",
        vec![ExpectedObservation {
            id: "oracle.scope.divergence_linux".into(),
            expected_present: true,
        }],
    );
    let with_pred = HistoricalReplay::with_sealed_prediction("c".repeat(128), Vec::new(), pred);

    assert!(only_obs.sealed_prediction.is_none());
    assert!(with_pred.sealed_prediction.is_some());
}

#[test]
fn closure_replay_preserves_sealed_label_after_round_trip() {
    let pred = SealedPrediction::seal(
        "e76-replay-smoke",
        vec![ExpectedObservation {
            id: "oracle.scope.divergence_linux".into(),
            expected_present: true,
        }],
    );
    let replay = HistoricalReplay::with_sealed_prediction("d".repeat(128), Vec::new(), pred);
    let carried = replay
        .sealed_prediction
        .as_ref()
        .expect("replay must carry the sealed prediction we attached");
    assert_eq!(carried.label, "e76-replay-smoke");
    // The seal must still equal the content_digest of the
    // canonical encoding (sorted "id|present" lines joined with
    // '\n'). The replay ctor does NOT mutate the sealed prediction
    // — proof of preservation. We compute via the real digest
    // function the seal uses, so the test stays in lock-step with
    // the algorithm even if the digest implementation changes.
    use crate::application::portable_execution::content_digest;
    let canonical = "oracle.scope.divergence_linux|true";
    let expected_seal = content_digest(canonical.as_bytes()).as_str().to_string();
    assert_eq!(carried.seal_digest, expected_seal);
}

#[test]
fn closure_replay_holds_observations_for_later_replay() {
    // The replay shape must accept an observation list — so a
    // follow-up cycle can attach observations offline and then
    // run the (currently `todo!()`) `compare_replay` against them.
    // Note: observations here are `PlatformObservation` (raw
    // bytes per platform), NOT the prediction-scoped `Observation`.
    let pred = SealedPrediction::seal(
        "smoke",
        vec![ExpectedObservation {
            id: "oracle.scope.smoke".into(),
            expected_present: false, // we expect this NOT to fire
        }],
    );
    let obs = PlatformObservation {
        platform: PlatformKind::Linux,
        id: "round.trip".into(),
        raw: b"observed".to_vec(),
    };
    let replay = HistoricalReplay::with_sealed_prediction("e".repeat(128), vec![obs], pred);
    assert_eq!(replay.observations.len(), 1);
    assert_eq!(replay.observations[0].id, "round.trip");
    assert_eq!(replay.observations[0].platform, PlatformKind::Linux);
}
