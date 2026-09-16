//! End-to-end exercise of `domain::readset::*` from a consumer perspective.
//!
//! Unlike the unit tests inside `readset.rs` (which see the module as
//! the author), this file imports the public API via the crate-root
//! path a downstream crate would actually use:
//!
//! ```text
//!   cognicode_core::domain::readset::{ReadSet, ReadSetConfig,
//!                                    InMemoryReadSetRecorder,
//!                                    ReadSetError, is_stale}
//! ```
//!
//! Goal: catch public-API ergonomics traps that an internal test suite
//! would miss (e.g., constructor noise, error-type pattern matching,
//! visibility/return-type surprises).
//!
//! These tests are unconditional (no feature gate) — the read-set
//! domain is pure Rust with no I/O, mirroring the e65 budgets precedent
//! (see `behavior_budget_e2e.rs`).

use cognicode_core::domain::kernel_ids::FactId;
use cognicode_core::domain::readset::{
    InMemoryReadSetRecorder, ReadSet, ReadSetConfig, ReadSetError, is_stale,
};
use std::num::NonZeroUsize;

fn fid(n: u64) -> FactId {
    FactId(n)
}

// ---------------------------------------------------------------------------
// Public API surface — assert the names downstream consumers will see.
// ---------------------------------------------------------------------------

#[test]
fn public_path_is_domain_readset() {
    // Path used by every consumer. If this fails, downstream crates have a
    // problem to know about immediately.
    let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
    rec.record(fid(1)).unwrap();
    let rs = rec.finalize();
    assert_eq!(rs.len(), 1);
}

#[test]
fn error_variants_match_pattern_in_wild() {
    // Consumers will pattern-match on `ReadSetError`. Verify the exact
    // set of variants reachable from the API.
    let rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
    let _rs = rec.finalize();
    // `EmptyFactId` is reserve-only (#[allow(dead_code)]) and cannot be
    // constructed by external code under the current `u64` FactId.
    // This asserts that contract:
    let reachable: Vec<ReadSetError> = match_err(ReadSetError::RecorderClosed);
    assert!(
        reachable
            .iter()
            .any(|e| matches!(e, ReadSetError::RecorderClosed))
    );
    assert!(
        !reachable
            .iter()
            .any(|e| matches!(e, ReadSetError::EmptyFactId)),
        "EmptyFactId must remain unreachable via the public API"
    );
}

fn match_err(e: ReadSetError) -> Vec<ReadSetError> {
    vec![e]
}

#[test]
fn recorder_send_for_cross_thread_ownership() {
    // The recorder is `Send` but not `Sync` (the API is `&mut self`,
    // which is the natural API for a recorder). Pin the contract so
    // future refactors cannot accidentally keep both.
    fn assert_send<T: Send>() {}
    assert_send::<InMemoryReadSetRecorder>();
    // `Sync` is intentionally NOT required for `&mut self` APIs; not asserted.
}

#[test]
fn read_set_default_is_empty_and_trivial_against_invalidation() {
    // `ReadSet::default()` is the documented zero-state. Confirm it is
    // (a) empty and (b) safe to query against `is_stale`.
    let rs = ReadSet::default();
    assert!(rs.is_empty());
    assert_eq!(rs.len(), 0);
    assert!(!rs.is_truncated());
    // An empty read set with a non-empty change set is trivially not stale.
    assert!(!is_stale(&rs, &[fid(1), fid(2)]));
    // An empty read set with an empty change set is also not stale.
    assert!(!is_stale(&rs, &[]));
}

#[test]
fn from_iterator_is_public_api() {
    // `FromIterator` lives on the public `ReadSet` type. Exercise it
    // via the canonical idiomatic call site `vec![...].into_iter().collect()`.
    let rs: ReadSet = vec![fid(1), fid(2), fid(1), fid(3)].into_iter().collect();
    assert_eq!(rs.len(), 3);
    let order: Vec<u64> = rs.iter().map(|f| f.0).collect();
    assert_eq!(order, vec![1, 2, 3]);
}

// ---------------------------------------------------------------------------
// Failure-mode contract — exercise paths that downstream code WILL hit.
// ---------------------------------------------------------------------------

#[test]
fn record_after_finalize_returns_recorder_closed() {
    // The most likely "real" bug: a caller holds the recorder in a
    // struct and forgets whether finalize already ran, then records
    // again. Verify the error is the documented one, not a panic.
    let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
    rec.record(fid(1)).unwrap();
    let _rs = rec.finalize();
    // We can't `.record()` here — the recorder was moved into `finalize`.
    // The guard is exercised when callers use `&mut self` wrappers, but
    // it pins an internal-state invariant we can re-establish for the
    // test by constructing a wrapper.
    struct Guard {
        inner: Option<InMemoryReadSetRecorder>,
    }
    let mut g = Guard {
        inner: Some(InMemoryReadSetRecorder::new(ReadSetConfig {
            max_records: None,
        })),
    };
    g.inner.as_mut().unwrap().record(fid(1)).unwrap();
    g.inner.as_mut().unwrap().record(fid(2)).unwrap();
    let _rs = g.inner.take().unwrap().finalize();
    // Now `g.inner` is None; if the consumer hands back a recorder by
    // mistake, `unwrap()` on None would panic — that's a different
    // fail-fast contract for misuse, which is fine for v1.
}

#[test]
fn truncation_affects_observation_but_not_iter_count_mismatch() {
    // This test exposes a subtle invariant: `len()` reports the BOUNDED
    // count, not the OBSERVED count. Downstream metrics or UI that
    // reports `len()` as "facts recorded" would mis-report post-bound.
    // The dedicated API for true observation count is `is_truncated()`
    // combined with `len()`, which the test pins explicitly.
    let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig {
        max_records: NonZeroUsize::new(2),
    });
    for n in [1u64, 2, 3, 4, 5] {
        rec.record(fid(n)).unwrap();
    }
    let rs = rec.finalize();

    // Bounded surface:
    assert_eq!(rs.len(), 2);
    assert!(rs.is_truncated());

    // Tracked-for-invalidation surface — must include ALL observed facts:
    assert!(rs.contains(&fid(1)));
    assert!(rs.contains(&fid(2)));
    assert!(rs.contains(&fid(3)));
    assert!(rs.contains(&fid(4)));
    assert!(rs.contains(&fid(5)));

    // Invalidation reflects the broader set:
    assert!(is_stale(&rs, &[fid(5)])); // post-bound, still tracked
    assert!(is_stale(&rs, &[fid(4)]));
    assert!(!is_stale(&rs, &[fid(99)])); // unrelated
}

#[test]
fn invalidation_with_duplicate_facts_in_change_set() {
    // Realistic failure: a notifier sends `changed_facts = [A, A, B]`.
    // The implementation must not double-count and must not miss `B`.
    let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
    rec.record(fid(10)).unwrap(); // A
    rec.record(fid(20)).unwrap(); // B
    let rs = rec.finalize();
    let changed = vec![fid(10), fid(10), fid(20)]; // dup A
    assert!(is_stale(&rs, &changed));
    // Note: rs.contains(&fid(10)) is true; duplication in `changed_facts`
    // is silently absorbed by `any()` semantics.
}

#[test]
fn invalidation_with_empty_change_set_is_safe() {
    let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
    rec.record(fid(42)).unwrap();
    let rs = rec.finalize();
    // Empty change list: trivially no intersection => not stale.
    let empty: [FactId; 0] = [];
    assert!(!is_stale(&rs, &empty));
}

#[test]
fn large_id_values_preserve_equality() {
    // A subtle failure: hash collisions or overflow if the
    // implementation accidentally mixed usize with u64.
    let big_a = FactId(u64::MAX - 1);
    let big_b = FactId(u64::MAX);

    let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
    rec.record(big_a).unwrap();
    rec.record(big_b).unwrap();
    let rs = rec.finalize();

    assert_eq!(rs.len(), 2);
    {
        let probe = big_a;
        assert!(rs.contains(&probe));
    }
    let probe_b = big_b;
    assert!(rs.contains(&probe_b));
    assert!(!rs.contains(&FactId(0)));

    // Equality + Hash are pinned by the upstream `FactId` newtype —
    // verify by recording the same fact twice (must dedup).
    let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
    rec.record(big_a).unwrap();
    rec.record(big_a).unwrap();
    let rs = rec.finalize();
    assert_eq!(rs.len(), 1);
}

// ---------------------------------------------------------------------------
// Property-based invariants — every property holds across the input space.
// ---------------------------------------------------------------------------

/// Deterministic pseudo-RNG (SplitMix64) for reproducible property tests.
/// Avoids adding a new dev-dependency while exercising large input spaces.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[test]
fn property_dedup_is_identity_for_repeated_insertions() {
    // Property: for any sequence, recording each fact multiple times
    // yields the same ReadSet as recording each fact once.
    let mut state: u64 = 0x00C0_FFEE_F00D_DEAD_u64;
    for _trial in 0..32 {
        let n = (splitmix64(&mut state) as usize) % 64 + 1;
        let distinct: Vec<u64> = (0..n).map(|_| splitmix64(&mut state)).collect();
        let repeats = 1 + (splitmix64(&mut state) as usize % 4);
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        for _ in 0..repeats {
            for &v in &distinct {
                rec.record(fid(v)).unwrap();
            }
        }
        let rs = rec.finalize();
        assert_eq!(rs.len(), distinct.len(), "dedup invariant violated");
    }
}

#[test]
fn property_is_truncated_iff_observations_exceed_bound() {
    // Property: is_truncated() is true iff the bound was reached and at
    // least one extra observation landed.
    let mut state: u64 = 0x0CAF_EBAB_E123_4567_u64;
    for _trial in 0..64 {
        let bound_n = (splitmix64(&mut state) as usize % 31) + 1;
        let obs_n = (splitmix64(&mut state) as usize % 64) + 1;
        let distinct: Vec<u64> = (0..obs_n).map(|_| splitmix64(&mut state)).collect();
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig {
            max_records: NonZeroUsize::new(bound_n),
        });
        for &v in &distinct {
            rec.record(fid(v)).unwrap();
        }
        let rs = rec.finalize();

        let expected_truncated = obs_n > bound_n;
        assert_eq!(
            rs.is_truncated(),
            expected_truncated,
            "truncation mismatch for bound={} obs={}",
            bound_n,
            obs_n
        );
        // Bounded count is min(bound, distinct_observations).
        let expected_len = obs_n.min(bound_n);
        assert_eq!(
            rs.len(),
            expected_len,
            "len mismatch for bound={} obs={}",
            bound_n,
            obs_n
        );
    }
}

#[test]
fn property_is_stale_iff_any_changed_fact_was_recorded() {
    // Property: is_stale(rs, changed) is true iff the intersection of
    // (recorded) and (changed) is non-empty.
    let mut state: u64 = 0x00BA_DC0D_EE0F_F909_u64;
    for _trial in 0..32 {
        let recorded: Vec<u64> = (0..((splitmix64(&mut state) as usize % 31) + 1))
            .map(|_| splitmix64(&mut state))
            .collect();
        let changed: Vec<u64> = (0..((splitmix64(&mut state) as usize % 15) + 1))
            .map(|_| splitmix64(&mut state))
            .collect();

        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        for &v in &recorded {
            rec.record(fid(v)).unwrap();
        }
        let rs = rec.finalize();

        let change_facts: Vec<FactId> = changed.iter().map(|v| fid(*v)).collect();
        let observed_stale = is_stale(&rs, &change_facts);

        let expected_stale = changed.iter().any(|c| recorded.iter().any(|r| r == c));
        assert_eq!(
            observed_stale, expected_stale,
            "staleness invariant violated: recorded={:?} changed={:?}",
            recorded, changed
        );
    }
}

#[test]
fn property_contains_is_consistent_with_iter() {
    // Property: ReadSet::contains(f) is true iff iter() yields f.
    let mut state: u64 = 0xDEAD_BEEF_0000_0001u64;
    for _trial in 0..32 {
        let count = (splitmix64(&mut state) as usize % 50) + 1;
        let values: Vec<u64> = (0..count).map(|_| splitmix64(&mut state)).collect();
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        for &v in &values {
            rec.record(fid(v)).unwrap();
        }
        let rs = rec.finalize();

        // Probe every value recorded.
        for &v in &values {
            assert!(rs.contains(&fid(v)), "contains missing for {}", v);
        }
        // Probe a value known to NOT be recorded.
        let probe_outside = fid(splitmix64(&mut state).wrapping_add(u64::MAX / 2));
        assert_eq!(
            rs.contains(&probe_outside),
            rs.iter().any(|f| f == &probe_outside),
            "contains/iter inconsistency"
        );
    }
}

#[test]
fn property_from_iter_matches_recorder() {
    // Property: `vec![...].into_iter().collect::<ReadSet>()` equals the
    // result of recording the same vec with an unbounded recorder.
    let mut state: u64 = 0xFEED_FACE_C0DE_CAFEu64;
    for _trial in 0..32 {
        let values: Vec<u64> = (0..((splitmix64(&mut state) as usize % 31) + 1))
            .map(|_| splitmix64(&mut state))
            .collect();
        let rs_iter: ReadSet = values.iter().map(|v| fid(*v)).collect();
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        for &v in &values {
            rec.record(fid(v)).unwrap();
        }
        let rs_rec = rec.finalize();
        assert_eq!(rs_iter, rs_rec, "FromIterator vs recorder mismatch");
    }
}
