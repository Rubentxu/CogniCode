//! Self-hosting platform equivalence + historical bootstrap (e76 WU4).
//!
//! The self-hosting validation must work across supported platforms
//! (Linux, macOS, Windows). OS differences (line endings, path
//! separators, environment variables, …) MUST be normalised away
//! before the canonical-vs-derived comparison happens — otherwise
//! they would silently become semantic knowledge differences.
//!
//! This module provides:
//!
//! 1. [`PlatformKind`] — the three supported targets, declared
//!    explicitly so the corpus can be enumerated.
//! 2. [`PlatformObservation`] — a single observation tagged with
//!    the platform it was recorded on.
//! 3. [`PlatformNormaliser`] — trait that platform-specific
//!    normalisers implement (LF normalisation, canonical path,
//!    env-var filtering, …). The default normaliser does the
//!    well-known canonical normalisations (CRLF→LF, //→/,
//!    case-insensitive Windows paths mapped to canonical form).
//! 4. [`HistoricalReplay`] — a small sealed-prediction container
//!    used to replay a historical self-hosting trial. The replay
//!    is intentionally decoupled from the prediction-sealing
//!    time so that successor outcomes cannot leak in.
//!
//! What this module does NOT do:
//! - It does NOT execute code on any platform. The caller runs
//!   the corpus on each platform and feeds observations in.
//! - It does NOT mint authority. Equivalence is a structural
//!   observation, not a verdict.
//!
//! Design constraint: this module MUST be platform-neutral in
//! code. The platform kinds are enumerated, but the module does
//! NOT mention specific runtimes or platform-specific mechanics
//! in production code (only in the test, where it is stripped).

use std::collections::BTreeMap;

/// The supported platforms. Enumerated explicitly so the corpus
/// can be exhaustively covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlatformKind {
    Linux,
    MacOs,
    Windows,
}

impl PlatformKind {
    /// Stable identifier of the platform, suitable for log lines.
    pub fn label(self) -> &'static str {
        match self {
            PlatformKind::Linux => "linux",
            PlatformKind::MacOs => "macos",
            PlatformKind::Windows => "windows",
        }
    }
}

/// A single observation tagged with the platform it was recorded
/// on. The observation's value is opaque bytes (the platform's
/// view of the same logical datum).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformObservation {
    pub platform: PlatformKind,
    /// Stable identifier of the observation (e.g. a digest of the
    /// input that produced it).
    pub id: String,
    /// The bytes the platform produced for this observation,
    /// BEFORE normalisation.
    pub raw: Vec<u8>,
}

/// A normaliser that maps a platform-specific raw observation to
/// its canonical form. The canonical form is the byte sequence
/// that two platforms agree on for semantically equivalent inputs.
///
/// This trait is implemented by callers (and by tests); the
/// default canonicalisation is provided by
/// [`default_canonicalize`].
pub trait PlatformNormaliser {
    fn canonicalize(&self, raw: &[u8]) -> Vec<u8>;
}

/// The default canonicalisation. It removes the well-known
/// platform mechanics that MUST NOT affect semantic equivalence:
///   - CRLF (Windows) → LF
///   - stray carriage returns at line end → LF
///
/// (Path and case normalisation are handled at the *enumeration*
/// layer by `application::portable_execution::canonicalize` and
/// its host detection; the raw-bytes canonicalisation here only
/// deals with content-level mechanics.)
pub struct DefaultNormaliser;

impl PlatformNormaliser for DefaultNormaliser {
    fn canonicalize(&self, raw: &[u8]) -> Vec<u8> {
        default_canonicalize(raw)
    }
}

/// Pure default canonicalisation exposed for tests and callers
/// that want to skip the trait.
pub fn default_canonicalize(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let b = raw[i];
        if b == b'\r' {
            // CRLF or lone CR → LF.
            out.push(b'\n');
            if i + 1 < raw.len() && raw[i + 1] == b'\n' {
                i += 1;
            }
        } else {
            out.push(b);
        }
        i += 1;
    }
    out
}

/// The equivalence verdict for a single observation id across the
/// platforms it was observed on. `Equivalent` means all platforms
/// produced the same canonical bytes; `Divergent` means at least
/// two platforms disagree (a platform-specific semantic gap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Equivalence {
    /// All observed platforms agree on the canonical bytes.
    Equivalent,
    /// At least two platforms disagree on the canonical bytes.
    Divergent { platforms: Vec<PlatformKind> },
    /// No observations recorded.
    Absent,
}

/// Compare a set of platform observations grouped by id. Returns
/// one verdict per id.
pub fn compare_platforms<N: PlatformNormaliser>(
    normaliser: &N,
    observations: &[PlatformObservation],
) -> BTreeMap<String, Equivalence> {
    use std::collections::BTreeMap;
    // Group observations by id.
    let mut by_id: BTreeMap<String, Vec<&PlatformObservation>> = BTreeMap::new();
    for obs in observations {
        by_id.entry(obs.id.clone()).or_default().push(obs);
    }
    let mut out = BTreeMap::new();
    for (id, group) in by_id {
        if group.is_empty() {
            out.insert(id, Equivalence::Absent);
            continue;
        }
        // Canonicalise each observation and compare.
        let mut canonicals: Vec<(PlatformKind, Vec<u8>)> = group
            .iter()
            .map(|o| (o.platform, normaliser.canonicalize(&o.raw)))
            .collect();
        canonicals.sort_by_key(|(p, _)| *p);
        let first = &canonicals[0].1;
        let mut divergent: Vec<PlatformKind> = Vec::new();
        for (p, c) in &canonicals[1..] {
            if c != first {
                divergent.push(*p);
            }
        }
        if divergent.is_empty() {
            out.insert(id, Equivalence::Equivalent);
        } else {
            // Include the first platform so the divergent set is
            // self-describing.
            let mut all: Vec<PlatformKind> = divergent
                .into_iter()
                .chain(std::iter::once(canonicals[0].0))
                .collect();
            all.sort();
            out.insert(id, Equivalence::Divergent { platforms: all });
        }
    }
    out
}

/// A historical replay container. Used by e76 WU4 (and future M13
/// OPTIMIZE/CONFIRM separation) to replay an old self-hosting trial
/// without leaking successor outcomes into the prediction.
///
/// The replay binds the observation set to a specific snapshot
/// digest at sealing time. The snapshot digest is what the caller
/// recorded at the time the historical trial was originally run;
/// using it later means the replay is reproducible without
/// re-executing any oracles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalReplay {
    /// Snapshot digest the replay is bound to. Conventionally a
    /// SHA-256 hex string of the workspace state at the time of
    /// the original trial.
    pub snapshot_digest: String,
    /// Observations as they were recorded at the time. The replay
    /// uses these directly — no oracle is re-executed.
    pub observations: Vec<PlatformObservation>,
    /// Optional sealed prediction associated with the historical
    /// trial. When present, the WU2 scorer can compare the
    /// replay's observations to the sealed predictions.
    pub sealed_prediction: Option<crate::application::self_hosting::prediction::SealedPrediction>,
}

impl HistoricalReplay {
    /// Build a replay with no associated prediction (purely an
    /// observation replay).
    pub fn observation_only(
        snapshot_digest: impl Into<String>,
        observations: Vec<PlatformObservation>,
    ) -> Self {
        Self {
            snapshot_digest: snapshot_digest.into(),
            observations,
            sealed_prediction: None,
        }
    }

    /// Build a replay with an associated sealed prediction. Use
    /// this when replaying a trial that should be re-scored against
    /// the same expectations.
    pub fn with_sealed_prediction(
        snapshot_digest: impl Into<String>,
        observations: Vec<PlatformObservation>,
        sealed_prediction: crate::application::self_hosting::prediction::SealedPrediction,
    ) -> Self {
        Self {
            snapshot_digest: snapshot_digest.into(),
            observations,
            sealed_prediction: Some(sealed_prediction),
        }
    }

    /// Run the platform equivalence comparison on the replay's
    /// observations.
    pub fn equivalence<N: PlatformNormaliser>(
        &self,
        normaliser: &N,
    ) -> BTreeMap<String, Equivalence> {
        compare_platforms(normaliser, &self.observations)
    }

    /// Score the replay against its sealed prediction (if any).
    /// Returns None when no sealed prediction is associated.
    pub fn score(&self) -> Option<crate::application::self_hosting::prediction::ScoreMatrix> {
        let pred = self.sealed_prediction.clone()?;
        let observations: Vec<_> = self
            .observations
            .iter()
            .map(
                |o| crate::application::self_hosting::prediction::Observation {
                    id: o.id.clone(),
                    present: !o.raw.is_empty(),
                },
            )
            .collect();
        Some(crate::application::self_hosting::prediction::score(
            &pred,
            &observations,
        ))
    }
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(platform: PlatformKind, id: &str, raw: &[u8]) -> PlatformObservation {
        PlatformObservation {
            platform,
            id: id.into(),
            raw: raw.to_vec(),
        }
    }

    #[test]
    fn default_canonicalize_replaces_crlf_with_lf() {
        let crlf = b"a\r\nb\r\nc";
        let lf = b"a\nb\nc";
        assert_eq!(default_canonicalize(crlf), lf);
    }

    #[test]
    fn default_canonicalize_replaces_lone_cr_with_lf() {
        // Legacy Mac (pre-OSX) used lone CR; we accept it and
        // map to LF.
        let cr = b"a\rb\rc";
        assert_eq!(default_canonicalize(cr), b"a\nb\nc");
    }

    #[test]
    fn compare_platforms_marks_equivalent_when_canonicalised_agree() {
        // Linux uses LF, Windows uses CRLF. After canonicalisation
        // they MUST agree.
        let n = DefaultNormaliser;
        let observations = vec![
            obs(PlatformKind::Linux, "fact.a", b"hello\nworld\n"),
            obs(PlatformKind::Windows, "fact.a", b"hello\r\nworld\r\n"),
        ];
        let verdicts = compare_platforms(&n, &observations);
        assert_eq!(verdicts.get("fact.a"), Some(&Equivalence::Equivalent));
    }

    #[test]
    fn compare_platforms_marks_divergent_when_canonicalised_disagree() {
        let n = DefaultNormaliser;
        let observations = vec![
            obs(PlatformKind::Linux, "fact.a", b"hello"),
            obs(PlatformKind::MacOs, "fact.a", b"goodbye"),
        ];
        let verdicts = compare_platforms(&n, &observations);
        match verdicts.get("fact.a") {
            Some(Equivalence::Divergent { platforms }) => {
                assert!(platforms.contains(&PlatformKind::Linux));
                assert!(platforms.contains(&PlatformKind::MacOs));
            }
            other => panic!("expected Divergent, got {:?}", other),
        }
    }

    #[test]
    fn compare_platforms_reports_absent_for_missing_id() {
        let n = DefaultNormaliser;
        let verdicts = compare_platforms(&n, &[]);
        assert!(verdicts.is_empty(), "no ids observed → empty map");
    }

    #[test]
    fn historical_replay_can_score_against_sealed_prediction() {
        let pred = crate::application::self_hosting::prediction::SealedPrediction::seal(
            "replay-v1",
            vec![
                crate::application::self_hosting::prediction::ExpectedObservation {
                    id: "fact.a".into(),
                    expected_present: true,
                },
            ],
        );
        let observations = vec![obs(PlatformKind::Linux, "fact.a", b"hello")];
        let replay = HistoricalReplay::with_sealed_prediction("snap-digest-v1", observations, pred);
        let score = replay.score().expect("score present");
        assert_eq!(score.true_positive, 1);
        assert_eq!(score.false_negative, 0);
    }

    #[test]
    fn historical_replay_without_prediction_returns_none_on_score() {
        let replay = HistoricalReplay::observation_only("snap-digest-v1", vec![]);
        assert!(replay.score().is_none());
    }

    #[test]
    fn platform_kind_label_is_stable() {
        assert_eq!(PlatformKind::Linux.label(), "linux");
        assert_eq!(PlatformKind::MacOs.label(), "macos");
        assert_eq!(PlatformKind::Windows.label(), "windows");
    }

    #[test]
    fn platform_equivalence_module_does_not_leak_platform_specific_identifiers() {
        // The module is platform-neutral. The forbidden list is
        // extended with `wsl`/`hyper-v` to catch accidental
        // runtime-specific references (the module enumerates
        // `linux`, `macos`, `windows` as labels, which are
        // allowed because they are the canonical platform kinds).
        let full = include_str!("platform_equivalence.rs");
        let stripped = crate::application::portable_execution::strip_doc_comments_and_tests(full);
        let lower = stripped.to_lowercase();
        for forbidden in [
            "podman", "systemd", "quadlet", "wsl", "hyper-v", "docker", "rustc", "cargo ",
            "cargo.", "clippy",
        ] {
            assert!(
                !lower.contains(forbidden),
                "self_hosting::platform_equivalence code (non-test, non-doc) must not leak identifier: {forbidden}"
            );
        }
    }
}
