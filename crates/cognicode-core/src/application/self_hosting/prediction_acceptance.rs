//! End-to-end acceptance tests for prediction vs observation (e76 WU2).
//!
//! Exercises the REAL public API `SealedPrediction::seal` and
//! `score` with concrete predictions and observations, asserting
//! the TP/FP/FN/TN accounting matches expectations.

#![cfg(feature = "evidence-kernel")]

use crate::application::self_hosting::prediction::{
    ExpectedObservation, Observation, SealedPrediction, score,
};

#[test]
fn acceptance_prediction_seal_is_content_addressed() {
    let a = SealedPrediction::seal(
        "acceptance-pred-v1",
        vec![
            ExpectedObservation {
                id: "alpha".into(),
                expected_present: true,
            },
            ExpectedObservation {
                id: "beta".into(),
                expected_present: false,
            },
        ],
    );
    let b = SealedPrediction::seal(
        "acceptance-pred-v1",
        vec![
            ExpectedObservation {
                id: "beta".into(),
                expected_present: false,
            },
            ExpectedObservation {
                id: "alpha".into(),
                expected_present: true,
            },
        ],
    );
    assert_eq!(
        a.seal_digest, b.seal_digest,
        "seal must be content-addressed, order-invariant"
    );
    assert!(!a.seal_digest.is_empty());
}

#[test]
fn acceptance_score_perfect_match_returns_zero_fp_zero_fn() {
    let prediction = SealedPrediction::seal(
        "acceptance-perfect",
        vec![
            ExpectedObservation {
                id: "present_a".into(),
                expected_present: true,
            },
            ExpectedObservation {
                id: "absent_b".into(),
                expected_present: false,
            },
            ExpectedObservation {
                id: "present_c".into(),
                expected_present: true,
            },
        ],
    );
    let observations = vec![
        Observation {
            id: "present_a".into(),
            present: true,
        },
        Observation {
            id: "absent_b".into(),
            present: false,
        },
        Observation {
            id: "present_c".into(),
            present: true,
        },
    ];
    let m = score(&prediction, &observations);
    assert_eq!(m.true_positive, 2);
    assert_eq!(m.false_positive, 0);
    assert_eq!(m.false_negative, 0);
    assert_eq!(m.true_negative, 1);
    assert_eq!(m.precision(), Some(1.0));
    assert_eq!(m.recall(), Some(1.0));
}

#[test]
fn acceptance_score_surfaces_false_negatives() {
    let prediction = SealedPrediction::seal(
        "acceptance-fn",
        vec![ExpectedObservation {
            id: "must_observe".into(),
            expected_present: true,
        }],
    );
    let observations = vec![Observation {
        id: "must_observe".into(),
        present: false,
    }];
    let m = score(&prediction, &observations);
    assert_eq!(
        m.false_negative, 1,
        "missed must_observe MUST surface as FN"
    );
    assert_eq!(m.recall(), Some(0.0));
    // precision is None (no observations of present type counted)
    // — this is the contract we documented in WU2.
}

#[test]
fn acceptance_score_surfaces_false_positives() {
    let prediction = SealedPrediction::seal("acceptance-fp", Vec::new());
    let observations = vec![
        Observation {
            id: "surprise_signal".into(),
            present: true,
        },
        Observation {
            id: "noise".into(),
            present: false,
        },
    ];
    let m = score(&prediction, &observations);
    // Empty prediction: every observation is "unexpected". The
    // present one is FP, the absent one is TN.
    assert_eq!(m.false_positive, 1);
    assert_eq!(m.true_negative, 1);
}
