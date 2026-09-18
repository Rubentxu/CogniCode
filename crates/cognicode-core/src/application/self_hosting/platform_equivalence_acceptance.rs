//! End-to-end acceptance tests for platform equivalence (e76 WU4).
//!
//! Exercises the REAL public API `compare_platforms` and
//! `HistoricalReplay` against concrete observation data, not just
//! the synthetic bytes used by the unit tests in
//! `platform_equivalence.rs`.
//!
//! The acceptance criterion: when the same logical fact is observed
//! on Linux (LF) and Windows (CRLF), `compare_platforms` returns
//! `Equivalent` after the default canonicalisation. When the
//! observation is genuinely divergent, the verdict is `Divergent`
//! and the offending platforms are listed.

#![cfg(feature = "evidence-kernel")]

use crate::application::self_hosting::platform_equivalence::{
    DefaultNormaliser, Equivalence, HistoricalReplay, PlatformKind, PlatformObservation,
    compare_platforms,
};

#[test]
fn acceptance_platform_equivalence_agrees_across_lf_and_crlf() {
    let n = DefaultNormaliser;
    let observations = vec![
        PlatformObservation {
            platform: PlatformKind::Linux,
            id: "acceptance.lf_crlf".into(),
            raw: b"line one\nline two\nline three\n".to_vec(),
        },
        PlatformObservation {
            platform: PlatformKind::Windows,
            id: "acceptance.lf_crlf".into(),
            raw: b"line one\r\nline two\r\nline three\r\n".to_vec(),
        },
        PlatformObservation {
            platform: PlatformKind::MacOs,
            id: "acceptance.lf_crlf".into(),
            raw: b"line one\nline two\nline three\n".to_vec(),
        },
    ];
    let verdicts = compare_platforms(&n, &observations);
    assert_eq!(
        verdicts.get("acceptance.lf_crlf"),
        Some(&Equivalence::Equivalent),
        "LF and CRLF must agree after canonical normalisation"
    );
}

#[test]
fn acceptance_platform_equivalence_flags_genuine_divergence() {
    let n = DefaultNormaliser;
    let observations = vec![
        PlatformObservation {
            platform: PlatformKind::Linux,
            id: "acceptance.genuine_divergence".into(),
            raw: b"the linux answer is 42".to_vec(),
        },
        PlatformObservation {
            platform: PlatformKind::Windows,
            id: "acceptance.genuine_divergence".into(),
            raw: b"the windows answer is 17".to_vec(),
        },
        PlatformObservation {
            platform: PlatformKind::MacOs,
            id: "acceptance.genuine_divergence".into(),
            raw: b"the macos answer is 99".to_vec(),
        },
    ];
    let verdicts = compare_platforms(&n, &observations);
    match verdicts.get("acceptance.genuine_divergence") {
        Some(Equivalence::Divergent { platforms }) => {
            assert_eq!(platforms.len(), 3);
            assert!(platforms.contains(&PlatformKind::Linux));
            assert!(platforms.contains(&PlatformKind::Windows));
            assert!(platforms.contains(&PlatformKind::MacOs));
        }
        other => panic!("expected Divergent, got {:?}", other),
    }
}

#[test]
fn acceptance_historical_replay_equivalence_binds_snapshot() {
    let observations = vec![
        PlatformObservation {
            platform: PlatformKind::Linux,
            id: "replay.line".into(),
            raw: b"the quick brown fox\n".to_vec(),
        },
        PlatformObservation {
            platform: PlatformKind::Windows,
            id: "replay.line".into(),
            raw: b"the quick brown fox\r\n".to_vec(),
        },
    ];
    let replay = HistoricalReplay::observation_only("snapshot-digest-acceptance-v1", observations);
    assert_eq!(
        replay.snapshot_digest, "snapshot-digest-acceptance-v1",
        "snapshot digest binding must be preserved"
    );
    let verdicts = replay.equivalence(&DefaultNormaliser);
    assert_eq!(verdicts.get("replay.line"), Some(&Equivalence::Equivalent));
    assert!(
        replay.score().is_none(),
        "observation-only replay has no associated sealed prediction"
    );
}
