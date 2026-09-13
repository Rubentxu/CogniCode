//! Tiered deterministic continuity matcher (E38 WU-3, design D3/D6) —
//! threads [`StableEntityId`]s across ONE snapshot pair (N → N+1).
//!
//! ## Pipeline (design D3)
//!
//! Over the per-snapshot [`SnapshotEntityView`]s, four ordered tiers run
//! TIER-MAJOR (all T0 claims before any T1, …), each pass walking the
//! unresolved after-entities in canonical `(fqn, entity)` order:
//!
//! - **T0 [`MatchTier::ExactIdentity`]** — the identity string (FQN) is
//!   present on both sides.
//! - **T1 [`MatchTier::PathNameKind`]** — same `(path, name, kind)`; the
//!   line segment may differ ("Line shift keeps stable identity").
//! - **T2 [`MatchTier::RenameEvidence`]** — a declared file rename/move
//!   `old_path → new_path` with `similarity ≥ rename_similarity_floor`
//!   ties the before-file to the after-file, plus equal `(name, kind)`
//!   ("File move keeps stable identity"). Evidence arrives as plain DATA
//!   ([`FileRename`] — design D2); the matcher never resolves the port.
//! - **T3 [`MatchTier::Fingerprint`]** — tagged-multiset Jaccard ≥
//!   `jaccard_match_threshold` with the kind hard pre-filter (design D4).
//!   Bodyless entities score 0.0 (fail-closed) and are never T3-matched.
//!
//! ## Ambiguity is fail-closed and terminal (ADR-038)
//!
//! With MORE than one candidate at a tier, the candidates are separated by
//! their tier score: the best claims the after-entity only when it exceeds
//! the runner-up by MORE than `ambiguity_epsilon`; a within-epsilon tie —
//! exact ties included, which is all T0/T1 can produce — yields
//! [`ContinuityStatus::Ambiguous`] listing every candidate FQN with NO
//! [`StableEntityId`]. Candidate ordering `(score desc, name-equal, fqn
//! asc)` only enumerates deterministically; it never rescues a
//! within-epsilon tie. An `Ambiguous` outcome is TERMINAL for the pair: it
//! never falls through to a lower tier and is never retroactively
//! force-matched. Its candidate before-entities stay in the pool (they may
//! still be claimed by other after-entities on clearly better evidence);
//! anything unclaimed ends [`ContinuityStatus::Terminated`], retaining its
//! pool id.
//!
//! ## Identity pool, New and no-resurrection (A1/A2)
//!
//! Matching is strictly pairwise N → N+1 with no history table: the before
//! pool receives `StableEntityId(1..K)` in sorted-FQN order, matches
//! inherit, after-only entities are [`ContinuityStatus::New`] with fresh
//! ids `K+1..` in sorted-FQN order. A symbol reintroduced after a gap is
//! therefore NEW again (no resurrection); stitching across ≥3 snapshots is
//! the caller's composition duty.
//!
//! ## Determinism (spec "Tiered deterministic continuity matching")
//!
//! The matcher sorts facts itself (via [`SnapshotEntityView::from_facts`]),
//! enumerates in canonical order, and indexes renames order-freely, so
//! identical fact/rename sets yield identical [`ContinuityResult`]s
//! regardless of commit or input order (permutation-invariance tests
//! below).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use super::fingerprint::{SemanticFingerprint, fingerprint, similarity};
use super::view::{EntityFacts, SnapshotEntityView};
use crate::domain::evidence_kernel::fact::Fact;
use crate::domain::evidence_kernel::ids::{EntityId, OccurrenceId, SnapshotId, StableEntityId};
use crate::domain::evidence_kernel::ports::FileRename;

// ============================================================================
// Pinned thresholds (design D6)
// ============================================================================

/// Pinned T3 gate: minimum tagged-multiset Jaccard for a fingerprint match
/// (design D6). A pure rename costs exactly the two `name:` elements, so
/// bodies must carry the structural evidence — the benchmark fixtures are
/// sized accordingly (WU-4 tuning note).
pub const PINNED_JACCARD_MATCH_THRESHOLD: f64 = 0.6;

/// Pinned ambiguity margin (design D6): candidates whose scores differ by
/// this much or less are tied ("within the configured margin", spec
/// "Ambiguous continuity fails closed") and MUST yield `Ambiguous`.
pub const PINNED_AMBIGUITY_EPSILON: f64 = 0.05;

/// Pinned T2 floor (design D6): the declared rename/move similarity must
/// reach this before file-move evidence may match at all.
pub const PINNED_RENAME_SIMILARITY_FLOOR: f64 = 0.5;

/// The matcher's pinned thresholds (design D6). Defaults ARE the pin;
/// constructing different values is possible for tests but any change to
/// the pinned constants changes the benchmark's `MATCHER_CONVENTION`
/// digest and fails the run until explicitly re-pinned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatcherThresholds {
    /// Minimum T3 Jaccard score (`PINNED_JACCARD_MATCH_THRESHOLD`).
    pub jaccard_match_threshold: f64,
    /// Within-epsilon tie margin (`PINNED_AMBIGUITY_EPSILON`).
    pub ambiguity_epsilon: f64,
    /// Minimum T2 rename similarity (`PINNED_RENAME_SIMILARITY_FLOOR`).
    pub rename_similarity_floor: f64,
}

impl Default for MatcherThresholds {
    fn default() -> Self {
        Self {
            jaccard_match_threshold: PINNED_JACCARD_MATCH_THRESHOLD,
            ambiguity_epsilon: PINNED_AMBIGUITY_EPSILON,
            rename_similarity_floor: PINNED_RENAME_SIMILARITY_FLOOR,
        }
    }
}

// ============================================================================
// Outcome types (design D1/D3)
// ============================================================================

/// The evidence tier that produced a [`ContinuityStatus::Matched`].
#[derive(Debug, Clone, PartialEq)]
pub enum MatchTier {
    /// T0 — identical identity string on both sides.
    ExactIdentity,
    /// T1 — same `(path, name, kind)`; the line segment shifted.
    PathNameKind,
    /// T2 — declared file rename/move at or above the similarity floor.
    RenameEvidence {
        /// Repo-relative path of the file before the move.
        old_path: String,
        /// Repo-relative path of the file after the move.
        new_path: String,
        /// The declared similarity (≥ the pinned floor).
        similarity: f64,
    },
    /// T3 — tagged-multiset Jaccard at or above the pinned threshold.
    Fingerprint {
        /// The measured Jaccard score.
        score: f64,
    },
}

/// Continuity status of one occurrence (spec "Ambiguous continuity fails
/// closed"): exactly `Matched`, `New`, `Terminated`, or `Ambiguous`.
#[derive(Debug, Clone, PartialEq)]
pub enum ContinuityStatus {
    /// Matched across the pair at some tier with the tier's confidence
    /// (1.0 for exact tiers T0/T1, the rename similarity for T2, the
    /// Jaccard score for T3).
    Matched {
        /// The tier that decided the match.
        tier: MatchTier,
        /// Evidence strength.
        confidence: f64,
    },
    /// After-only: fresh stable id (no history — A1).
    New,
    /// Before-only: stable id retained, never resurrected.
    Terminated,
    /// Fail-closed: candidates listed, NO stable id, terminal per pair
    /// (ADR-038; identities are never merged).
    Ambiguous {
        /// Every candidate's FQN, sorted ascending.
        candidates: Vec<String>,
    },
}

/// One occurrence's continuity verdict (design D1). `stable_id` is `None`
/// ONLY for `Ambiguous` — every other status carries an id.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuityOutcome {
    /// The occurrence's identity string (FQN).
    pub fqn: String,
    /// The snapshot-scoped occurrence key.
    pub occurrence: OccurrenceId,
    /// The snapshot the occurrence lives in (the input facts' pin).
    pub snapshot: SnapshotId,
    /// The threaded stable id (`None` iff `Ambiguous`).
    pub stable_id: Option<StableEntityId>,
    /// The verdict.
    pub status: ContinuityStatus,
}

/// The full continuity mapping of one snapshot pair (design D1): every
/// after-entity (Matched/New/Ambiguous) and every before-only entity
/// (Terminated), sorted by `(snapshot, fqn, occurrence)`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuityResult {
    /// One outcome per occurrence, canonically sorted.
    pub outcomes: Vec<ContinuityOutcome>,
}

// ============================================================================
// Matching (design D2/D3)
// ============================================================================

/// Resolves continuity across one snapshot pair (design D2).
///
/// `before`/`after` are `facts_in_snapshot` slices (each pinned to its own
/// snapshot by the store read contract; the outcome snapshots are derived
/// from the facts' own pins — the minimum pin, deterministic even for
/// mispinned input). `renames` is the caller-resolved [`FileRename`]
/// evidence consumed as DATA. Sorting is internal: identical inputs yield
/// identical results regardless of order.
pub fn match_snapshots(
    before: &[Fact],
    after: &[Fact],
    renames: &[FileRename],
    thresholds: &MatcherThresholds,
) -> ContinuityResult {
    let before_snapshot = pinned_snapshot(before);
    let after_snapshot = pinned_snapshot(after);
    let before_view = SnapshotEntityView::from_facts(before, before_snapshot);
    let after_view = SnapshotEntityView::from_facts(after, after_snapshot);

    let pool = canonical_order(&before_view);
    let after_order = canonical_order(&after_view);

    // Stable-id pool: before entities in sorted-FQN order → 1..K (design D3).
    let before_stable: BTreeMap<EntityId, StableEntityId> = pool
        .iter()
        .enumerate()
        .map(|(i, e)| (e.entity, StableEntityId::new(i as u64 + 1)))
        .collect();
    let pool_max = pool.len() as u64;

    // Rename evidence indexed by destination file, order-free: duplicate
    // rows for one (old, new) pair collapse to their MAX similarity, rows
    // sort by (similarity desc, old path asc) so candidate enumeration is
    // deterministic under shuffled evidence input.
    let mut renames_by_new: BTreeMap<String, Vec<(String, f64)>> = BTreeMap::new();
    for rename in renames {
        let rows = renames_by_new.entry(rename.new_path.clone()).or_default();
        match rows.iter_mut().find(|(old, _)| *old == rename.old_path) {
            Some(entry) => entry.1 = entry.1.max(rename.similarity),
            None => rows.push((rename.old_path.clone(), rename.similarity)),
        }
    }
    for rows in renames_by_new.values_mut() {
        rows.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
    }

    // Fingerprints are computed once per occurrence (pure, fact-derived).
    let fps_before: BTreeMap<EntityId, SemanticFingerprint> =
        pool.iter().map(|e| (e.entity, fingerprint(e))).collect();
    let fps_after: BTreeMap<EntityId, SemanticFingerprint> = after_order
        .iter()
        .map(|e| (e.entity, fingerprint(e)))
        .collect();

    let mut claimed: BTreeSet<EntityId> = BTreeSet::new();
    let mut matched: BTreeMap<EntityId, (EntityId, MatchTier, f64)> = BTreeMap::new();
    let mut ambiguous: BTreeMap<EntityId, Vec<String>> = BTreeMap::new();
    let mut resolved: BTreeSet<EntityId> = BTreeSet::new();

    // Tier-major pipeline: every T0 claim before any T1, … (design D3).
    for a in after_order.iter().copied() {
        if resolved.contains(&a.entity) {
            continue;
        }
        if let Some(decision) = resolve(candidates_t0(a, &pool, &claimed), a, thresholds) {
            apply(
                decision,
                a,
                &mut claimed,
                &mut matched,
                &mut ambiguous,
                &mut resolved,
            );
        }
    }
    for a in after_order.iter().copied() {
        if resolved.contains(&a.entity) {
            continue;
        }
        if let Some(decision) = resolve(candidates_t1(a, &pool, &claimed), a, thresholds) {
            apply(
                decision,
                a,
                &mut claimed,
                &mut matched,
                &mut ambiguous,
                &mut resolved,
            );
        }
    }
    for a in after_order.iter().copied() {
        if resolved.contains(&a.entity) {
            continue;
        }
        if let Some(decision) = resolve(
            candidates_t2(a, &pool, &claimed, &renames_by_new, thresholds),
            a,
            thresholds,
        ) {
            apply(
                decision,
                a,
                &mut claimed,
                &mut matched,
                &mut ambiguous,
                &mut resolved,
            );
        }
    }
    for a in after_order.iter().copied() {
        if resolved.contains(&a.entity) {
            continue;
        }
        if let Some(decision) = resolve(
            candidates_t3(
                a,
                &pool,
                &claimed,
                &fps_before,
                &fps_after[&a.entity],
                thresholds,
            ),
            a,
            thresholds,
        ) {
            apply(
                decision,
                a,
                &mut claimed,
                &mut matched,
                &mut ambiguous,
                &mut resolved,
            );
        }
    }

    // New entities: fresh ids K+1.. in sorted-FQN order (design D3).
    let mut next = pool_max;
    let mut fresh: BTreeMap<EntityId, StableEntityId> = BTreeMap::new();
    for a in after_order.iter().copied() {
        if resolved.contains(&a.entity) {
            continue; // matched or ambiguous — neither is New
        }
        next += 1;
        fresh.insert(a.entity, StableEntityId::new(next));
    }

    // Outcomes: every after-entity, then every unclaimed (Terminated)
    // before-entity, canonically sorted.
    let mut outcomes = Vec::with_capacity(after_order.len() + pool.len());
    for a in after_order.iter().copied() {
        let (stable_id, status) = if let Some((before, tier, confidence)) = matched.get(&a.entity) {
            (
                Some(before_stable[before]),
                ContinuityStatus::Matched {
                    tier: tier.clone(),
                    confidence: *confidence,
                },
            )
        } else if let Some(candidates) = ambiguous.get(&a.entity) {
            // Ambiguous NEVER carries a stable id (design D1).
            (
                None,
                ContinuityStatus::Ambiguous {
                    candidates: candidates.clone(),
                },
            )
        } else {
            (Some(fresh[&a.entity]), ContinuityStatus::New)
        };
        outcomes.push(ContinuityOutcome {
            fqn: a.fqn.clone(),
            occurrence: OccurrenceId::from_entity(a.entity),
            snapshot: after_snapshot,
            stable_id,
            status,
        });
    }
    for b in pool.iter().copied() {
        if claimed.contains(&b.entity) {
            continue;
        }
        outcomes.push(ContinuityOutcome {
            fqn: b.fqn.clone(),
            occurrence: OccurrenceId::from_entity(b.entity),
            snapshot: before_snapshot,
            stable_id: Some(before_stable[&b.entity]),
            status: ContinuityStatus::Terminated,
        });
    }
    outcomes.sort_by(|x, y| {
        x.snapshot
            .cmp(&y.snapshot)
            .then_with(|| x.fqn.cmp(&y.fqn))
            .then_with(|| x.occurrence.cmp(&y.occurrence))
    });
    ContinuityResult { outcomes }
}

/// The snapshot pin of a fact slice: the minimum pin, deterministic under
/// input shuffling even for mispinned input. The store read contract pins
/// every fact of a slice to one snapshot; an empty side (no outcomes can
/// arise from it) degrades to the `NONE` sentinel.
fn pinned_snapshot(facts: &[Fact]) -> SnapshotId {
    facts
        .iter()
        .map(|f| f.snapshot)
        .min()
        .unwrap_or(SnapshotId::NONE)
}

/// Canonical per-side entity order: `(fqn, entity id)` — total and free of
/// input order. The `BTreeMap` iteration already orders by entity id, so
/// the stable sort on FQN is deterministic even for duplicate FQNs.
fn canonical_order(view: &SnapshotEntityView) -> Vec<&EntityFacts> {
    let mut entities: Vec<&EntityFacts> = view.entities.values().collect();
    entities.sort_by(|a, b| a.fqn.cmp(&b.fqn).then_with(|| a.entity.cmp(&b.entity)));
    entities
}

/// The file-path segment of an identity string: everything before the
/// `name:line` tail (the extractor grammar is `{file}:{name}:{line}`,
/// e37 D3). Identity strings with fewer than three segments carry no
/// recoverable path and degrade to the empty string — mirroring
/// `view::symbol_name_from_fqn`'s fallback discipline.
fn path_of(fqn: &str) -> String {
    let segments: Vec<&str> = fqn.split(':').collect();
    if segments.len() >= 3 {
        segments[..segments.len() - 2].join(":")
    } else {
        String::new()
    }
}

/// One tier candidate: a before-entity, its tier score, and the tier it
/// would produce. Scores are 1.0 for the exact tiers (T0/T1), the rename
/// similarity for T2, and the Jaccard score for T3.
struct Candidate<'a> {
    before: &'a EntityFacts,
    score: f64,
    tier: MatchTier,
}

/// One tier's decision for one after-entity.
enum TierDecision {
    /// A single clearly-best candidate claims the after-entity.
    Claim(EntityId, MatchTier, f64),
    /// Within-epsilon tie (or unresolvable multiplicity): fail closed with
    /// every candidate FQN, sorted.
    Ambiguous(Vec<String>),
}

/// Applies a tier decision, recording claims and terminal ambiguity.
fn apply(
    decision: TierDecision,
    after: &EntityFacts,
    claimed: &mut BTreeSet<EntityId>,
    matched: &mut BTreeMap<EntityId, (EntityId, MatchTier, f64)>,
    ambiguous: &mut BTreeMap<EntityId, Vec<String>>,
    resolved: &mut BTreeSet<EntityId>,
) {
    resolved.insert(after.entity);
    match decision {
        TierDecision::Claim(before, tier, confidence) => {
            claimed.insert(before);
            matched.insert(after.entity, (before, tier, confidence));
        }
        TierDecision::Ambiguous(candidates) => {
            // Terminal per pair (A2): recorded once, never revisited by a
            // lower tier, never force-matched (ADR-038).
            ambiguous.insert(after.entity, candidates);
        }
    }
}

/// Uniform per-tier resolution (design D3): zero candidates → fall through;
/// one candidate → claim; several → the best claims only when it exceeds
/// the runner-up by MORE than the epsilon margin, otherwise the tier fails
/// closed with every candidate listed.
fn resolve(
    candidates: Vec<Candidate<'_>>,
    after: &EntityFacts,
    thresholds: &MatcherThresholds,
) -> Option<TierDecision> {
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        let only = candidates.into_iter().next().expect("len == 1");
        return Some(TierDecision::Claim(
            only.before.entity,
            only.tier,
            only.score,
        ));
    }

    // Deterministic enumeration (score desc, name-equal first, fqn asc,
    // entity id asc) — ordering NEVER rescues a within-epsilon tie.
    let mut ordered = candidates;
    ordered.sort_by(|x, y| {
        y.score
            .partial_cmp(&x.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| (y.before.name == after.name).cmp(&(x.before.name == after.name)))
            .then_with(|| x.before.fqn.cmp(&y.before.fqn))
            .then_with(|| x.before.entity.cmp(&y.before.entity))
    });
    let best = &ordered[0];
    let second = &ordered[1];
    if best.score - second.score > thresholds.ambiguity_epsilon {
        let winner = ordered.into_iter().next().expect("non-empty");
        Some(TierDecision::Claim(
            winner.before.entity,
            winner.tier,
            winner.score,
        ))
    } else {
        let mut listed: Vec<String> = ordered.iter().map(|c| c.before.fqn.clone()).collect();
        listed.sort();
        listed.dedup();
        Some(TierDecision::Ambiguous(listed))
    }
}

/// T0 — the identity string is present on both sides (design D3).
fn candidates_t0<'a>(
    after: &EntityFacts,
    pool: &[&'a EntityFacts],
    claimed: &BTreeSet<EntityId>,
) -> Vec<Candidate<'a>> {
    pool.iter()
        .copied()
        .filter(|b| !claimed.contains(&b.entity) && b.fqn == after.fqn)
        .map(|b| Candidate {
            before: b,
            score: 1.0,
            tier: MatchTier::ExactIdentity,
        })
        .collect()
}

/// T1 — same `(path, name, kind)`; the line segment may differ (design D3,
/// spec scenario "Line shift keeps stable identity").
fn candidates_t1<'a>(
    after: &EntityFacts,
    pool: &[&'a EntityFacts],
    claimed: &BTreeSet<EntityId>,
) -> Vec<Candidate<'a>> {
    let after_path = path_of(&after.fqn);
    pool.iter()
        .copied()
        .filter(|b| !claimed.contains(&b.entity))
        .filter(|b| path_of(&b.fqn) == after_path && b.name == after.name && b.kind == after.kind)
        .map(|b| Candidate {
            before: b,
            score: 1.0,
            tier: MatchTier::PathNameKind,
        })
        .collect()
}

/// T2 — declared file rename/move (old file → the after-entity's file) at
/// or above the similarity floor, plus equal `(name, kind)` (design D3,
/// spec scenario "File move keeps stable identity").
fn candidates_t2<'a>(
    after: &EntityFacts,
    pool: &[&'a EntityFacts],
    claimed: &BTreeSet<EntityId>,
    renames_by_new: &BTreeMap<String, Vec<(String, f64)>>,
    thresholds: &MatcherThresholds,
) -> Vec<Candidate<'a>> {
    let after_path = path_of(&after.fqn);
    let Some(rows) = renames_by_new.get(&after_path) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for b in pool.iter().copied() {
        if claimed.contains(&b.entity) || b.name != after.name || b.kind != after.kind {
            continue;
        }
        let before_path = path_of(&b.fqn);
        // Best declared similarity from b's file into the after-file; rows
        // below the pinned floor are never evidence.
        let score = rows
            .iter()
            .filter(|(old, sim)| old == &before_path && *sim >= thresholds.rename_similarity_floor)
            .map(|(_, sim)| *sim)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        if let Some(score) = score {
            candidates.push(Candidate {
                before: b,
                score,
                tier: MatchTier::RenameEvidence {
                    old_path: before_path,
                    new_path: after_path.clone(),
                    similarity: score,
                },
            });
        }
    }
    candidates
}

/// T3 — tagged-multiset Jaccard at or above the pinned threshold, with the
/// kind hard pre-filter (design D3/D4). Bodyless entities score 0.0 and
/// are never candidates (fail-closed).
fn candidates_t3<'a>(
    after: &EntityFacts,
    pool: &[&'a EntityFacts],
    claimed: &BTreeSet<EntityId>,
    fps_before: &BTreeMap<EntityId, SemanticFingerprint>,
    fp_after: &SemanticFingerprint,
    thresholds: &MatcherThresholds,
) -> Vec<Candidate<'a>> {
    pool.iter()
        .copied()
        .filter(|b| !claimed.contains(&b.entity) && b.kind == after.kind)
        .filter_map(|b| {
            let score = similarity(&fps_before[&b.entity], fp_after);
            (score >= thresholds.jaccard_match_threshold).then_some(Candidate {
                before: b,
                score,
                tier: MatchTier::Fingerprint { score },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::evidence_kernel::fact::{FactValue, ProducerKind, ProvenanceRecord};
    use crate::domain::evidence_kernel::ids::FactId;
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::value_objects::Provenance;

    const BEFORE: SnapshotId = SnapshotId::new(1);
    const AFTER: SnapshotId = SnapshotId::new(2);

    // ---------------------------------------------------------------------
    // Fact builders (canonical grammar: FQN rides the defines OBJECT,
    // `kind=<K>` rides the detail — view.rs discipline).
    // ---------------------------------------------------------------------

    fn fact(
        snapshot: SnapshotId,
        id: u64,
        subject: u64,
        predicate: &str,
        object: &str,
        detail: Option<String>,
    ) -> Fact {
        Fact::new(
            FactId::new(id),
            EntityId::new(subject),
            RelationKind::try_new(predicate).expect("valid predicate"),
            FactValue::Text(object.to_string()),
            snapshot,
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                detail,
            ),
        )
        .expect("analyzer provenance is accepted")
    }

    fn defines(snapshot: SnapshotId, id: u64, subject: u64, fqn: &str, kind: &str) -> Fact {
        fact(
            snapshot,
            id,
            subject,
            "core:defines",
            fqn,
            Some(format!("kind={kind}")),
        )
    }

    fn calls(snapshot: SnapshotId, id: u64, subject: u64, callee: &str) -> Fact {
        fact(snapshot, id, subject, "core:calls", callee, None)
    }

    /// One defined function plus its call multiset (fact ids auto-spaced).
    fn fn_with_calls(snapshot: SnapshotId, subject: u64, fqn: &str, callees: &[&str]) -> Vec<Fact> {
        let mut facts = vec![defines(snapshot, subject, subject, fqn, "Function")];
        for (i, callee) in callees.iter().enumerate() {
            facts.push(calls(
                snapshot,
                subject * 100 + i as u64 + 1,
                subject,
                callee,
            ));
        }
        facts
    }

    fn rename(old_path: &str, new_path: &str, similarity: f64) -> FileRename {
        FileRename {
            old_path: old_path.to_string(),
            new_path: new_path.to_string(),
            similarity,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// The outcome of `fqn` on `snapshot`, if present.
    fn outcome<'r>(
        result: &'r ContinuityResult,
        snapshot: SnapshotId,
        fqn: &str,
    ) -> &'r ContinuityOutcome {
        result
            .outcomes
            .iter()
            .find(|o| o.snapshot == snapshot && o.fqn == fqn)
            .unwrap_or_else(|| panic!("no outcome for {fqn} on {snapshot}"))
    }

    fn assert_matched(outcome: &ContinuityOutcome, stable: u64) {
        assert!(
            matches!(outcome.status, ContinuityStatus::Matched { .. }),
            "expected Matched for {}, got {:?}",
            outcome.fqn,
            outcome.status
        );
        assert_eq!(
            outcome.stable_id,
            Some(StableEntityId::new(stable)),
            "{} must inherit stable:{stable}",
            outcome.fqn
        );
    }

    fn assert_ambiguous(outcome: &ContinuityOutcome, candidates: &[&str]) {
        let ContinuityStatus::Ambiguous { candidates: actual } = &outcome.status else {
            panic!(
                "expected Ambiguous for {}, got {:?}",
                outcome.fqn, outcome.status
            );
        };
        let expected: Vec<String> = candidates.iter().map(|c| c.to_string()).collect();
        assert_eq!(actual, &expected, "candidates must be listed sorted");
        assert_eq!(
            outcome.stable_id, None,
            "Ambiguous NEVER carries a stable id (design D1)"
        );
    }

    // ---------------------------------------------------------------------
    // Pinned thresholds (design D6).
    // ---------------------------------------------------------------------

    #[test]
    fn thresholds_default_to_the_pinned_constants() {
        let thresholds = MatcherThresholds::default();
        assert!(close(thresholds.jaccard_match_threshold, 0.6));
        assert!(close(thresholds.ambiguity_epsilon, 0.05));
        assert!(close(thresholds.rename_similarity_floor, 0.5));
        assert!(close(PINNED_JACCARD_MATCH_THRESHOLD, 0.6));
        assert!(close(PINNED_AMBIGUITY_EPSILON, 0.05));
        assert!(close(PINNED_RENAME_SIMILARITY_FLOOR, 0.5));
    }

    // ---------------------------------------------------------------------
    // T0 — exact identity.
    // ---------------------------------------------------------------------

    #[test]
    fn t0_exact_identity_matches_without_evidence() {
        let before = fn_with_calls(BEFORE, 10, "src/lib.rs:keep:1", &["alpha", "beta"]);
        let after = fn_with_calls(AFTER, 20, "src/lib.rs:keep:1", &["alpha", "beta"]);

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        let keep = outcome(&result, AFTER, "src/lib.rs:keep:1");
        assert_matched(keep, 1);
        let ContinuityStatus::Matched { tier, confidence } = &keep.status else {
            unreachable!("assert_matched guarantees Matched")
        };
        assert_eq!(*tier, MatchTier::ExactIdentity, "identical FQN is T0");
        assert!(close(*confidence, 1.0), "exact evidence is full confidence");
    }

    // ---------------------------------------------------------------------
    // T1 — path+name+kind (spec scenario "Line shift keeps stable identity").
    // ---------------------------------------------------------------------

    #[test]
    fn t1_line_shift_keeps_stable_identity() {
        let mut before = fn_with_calls(
            BEFORE,
            10,
            "src/engine.rs:tick:12",
            &["reset", "step", "check", "commit"],
        );
        let mut after = fn_with_calls(
            AFTER,
            20,
            "src/engine.rs:tick:40",
            &["reset", "step", "check", "commit"],
        );
        // Unrelated content inserted above the symbol shifts only the line
        // segment (the PAD constant is itself shifted and T1-matches too).
        before.insert(
            0,
            defines(BEFORE, 99, 11, "src/engine.rs:PAD:1", "Constant"),
        );
        after.insert(0, defines(AFTER, 98, 21, "src/engine.rs:PAD:2", "Constant"));

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        let tick = outcome(&result, AFTER, "src/engine.rs:tick:40");
        assert_matched(tick, 2); // pool order: PAD(1) < tick(2)
        let ContinuityStatus::Matched { tier, .. } = &tick.status else {
            unreachable!()
        };
        assert_eq!(*tier, MatchTier::PathNameKind, "a shifted line is T1");
    }

    #[test]
    fn t1_requires_kind_equality() {
        // Same path+name with bodies present: only the kind differs, so T1
        // must not fire and the kind pre-filter blocks T3 as well.
        let mut before = vec![defines(BEFORE, 10, 10, "src/lib.rs:widget:3", "Function")];
        before.push(calls(BEFORE, 101, 10, "draw"));
        before.push(calls(BEFORE, 102, 10, "measure"));
        let mut after = vec![defines(AFTER, 20, 20, "src/lib.rs:widget:9", "Struct")];
        after.push(calls(AFTER, 201, 20, "draw"));
        after.push(calls(AFTER, 202, 20, "measure"));

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        assert_eq!(
            outcome(&result, AFTER, "src/lib.rs:widget:9").status,
            ContinuityStatus::New
        );
        assert_eq!(
            outcome(&result, BEFORE, "src/lib.rs:widget:3").status,
            ContinuityStatus::Terminated
        );
    }

    // ---------------------------------------------------------------------
    // T2 — rename/move evidence (spec scenario "File move keeps stable
    // identity").
    // ---------------------------------------------------------------------

    #[test]
    fn t2_file_move_keeps_stable_identity() {
        let before = fn_with_calls(
            BEFORE,
            10,
            "src/config.rs:parse:5",
            &["read", "split", "merge"],
        );
        let after = fn_with_calls(
            AFTER,
            20,
            "src/parsing/config.rs:parse:5",
            &["read", "split", "merge"],
        );
        let renames = vec![rename("src/config.rs", "src/parsing/config.rs", 0.9)];

        let result = match_snapshots(&before, &after, &renames, &MatcherThresholds::default());

        let parse = outcome(&result, AFTER, "src/parsing/config.rs:parse:5");
        assert_matched(parse, 1);
        let ContinuityStatus::Matched { tier, confidence } = &parse.status else {
            unreachable!()
        };
        assert_eq!(
            *tier,
            MatchTier::RenameEvidence {
                old_path: "src/config.rs".to_string(),
                new_path: "src/parsing/config.rs".to_string(),
                similarity: 0.9,
            },
            "the declared move evidence is the tier"
        );
        assert!(
            close(*confidence, 0.9),
            "T2 confidence is the declared similarity"
        );
    }

    #[test]
    fn t2_requires_the_similarity_floor() {
        // 0.4 < 0.5 pinned floor: no T2 evidence. The bodies are disjoint so
        // T3 cannot rescue either — both sides fail closed to New/Terminated.
        let before = fn_with_calls(BEFORE, 10, "src/config.rs:parse:5", &["read"]);
        let after = fn_with_calls(AFTER, 20, "src/parsing/config.rs:parse:5", &["write"]);
        let renames = vec![rename("src/config.rs", "src/parsing/config.rs", 0.4)];

        let result = match_snapshots(&before, &after, &renames, &MatcherThresholds::default());

        assert_eq!(
            outcome(&result, AFTER, "src/parsing/config.rs:parse:5").status,
            ContinuityStatus::New,
            "below-floor evidence must not bless the move"
        );
    }

    #[test]
    fn t2_requires_matching_name_and_kind() {
        // The FILE moved, but the symbol was renamed too: T2's (name, kind)
        // equality fails. With disjoint bodies nothing else can match.
        let before = fn_with_calls(BEFORE, 10, "src/config.rs:parse:5", &["read"]);
        let after = fn_with_calls(AFTER, 20, "src/parsing/config.rs:load:5", &["write"]);
        let renames = vec![rename("src/config.rs", "src/parsing/config.rs", 0.95)];

        let result = match_snapshots(&before, &after, &renames, &MatcherThresholds::default());

        assert_eq!(
            outcome(&result, AFTER, "src/parsing/config.rs:load:5").status,
            ContinuityStatus::New
        );
    }

    #[test]
    fn t2_candidates_within_epsilon_fail_closed() {
        // Two renames land in the same file; both before-handles are equally
        // plausible (0.9 vs 0.87 — inside the 0.05 margin) → Ambiguous
        // (spec scenario "Colliding names fail closed").
        let mut before = fn_with_calls(BEFORE, 10, "src/a.rs:handle:1", &["h1", "h2"]);
        before.extend(fn_with_calls(
            BEFORE,
            11,
            "src/b.rs:handle:1",
            &["h1", "h2"],
        ));
        let after = fn_with_calls(AFTER, 20, "src/c.rs:handle:1", &["h1", "h2"]);
        let renames = vec![
            rename("src/a.rs", "src/c.rs", 0.9),
            rename("src/b.rs", "src/c.rs", 0.87),
        ];

        let result = match_snapshots(&before, &after, &renames, &MatcherThresholds::default());

        assert_ambiguous(
            outcome(&result, AFTER, "src/c.rs:handle:1"),
            &["src/a.rs:handle:1", "src/b.rs:handle:1"],
        );
        assert_eq!(
            outcome(&result, BEFORE, "src/a.rs:handle:1").status,
            ContinuityStatus::Terminated,
            "ambiguous candidates are never merged; unclaimed pool entities terminate"
        );
    }

    #[test]
    fn t2_margin_beyond_epsilon_resolves_to_the_best_candidate() {
        // 0.9 vs 0.7: the margin (0.2) exceeds epsilon → the stronger
        // evidence claims the after-entity deterministically.
        let mut before = fn_with_calls(BEFORE, 10, "src/a.rs:handle:1", &["h1"]);
        before.extend(fn_with_calls(BEFORE, 11, "src/b.rs:handle:1", &["h1"]));
        let after = fn_with_calls(AFTER, 20, "src/c.rs:handle:1", &["h1"]);
        let renames = vec![
            rename("src/a.rs", "src/c.rs", 0.9),
            rename("src/b.rs", "src/c.rs", 0.7),
        ];

        let result = match_snapshots(&before, &after, &renames, &MatcherThresholds::default());

        let handle = outcome(&result, AFTER, "src/c.rs:handle:1");
        assert_matched(handle, 1); // a.rs sorts first AND carries the best score
        let ContinuityStatus::Matched { tier, .. } = &handle.status else {
            unreachable!()
        };
        assert_eq!(
            *tier,
            MatchTier::RenameEvidence {
                old_path: "src/a.rs".to_string(),
                new_path: "src/c.rs".to_string(),
                similarity: 0.9,
            }
        );
    }

    // ---------------------------------------------------------------------
    // T3 — fingerprint similarity.
    // ---------------------------------------------------------------------

    #[test]
    fn t3_fingerprint_matches_a_pure_rename_above_threshold() {
        // Same body (4 structural elements), different name: Jaccard
        // 4/6 ≈ 0.667 ≥ 0.6 — the rename costs exactly the two `name:`
        // elements (design D4).
        let before = fn_with_calls(
            BEFORE,
            10,
            "src/lib.rs:fetch_total:1",
            &["log", "report", "audit", "export"],
        );
        let after = fn_with_calls(
            AFTER,
            20,
            "src/lib.rs:settle_total:1",
            &["log", "report", "audit", "export"],
        );

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        let settle = outcome(&result, AFTER, "src/lib.rs:settle_total:1");
        assert_matched(settle, 1);
        let ContinuityStatus::Matched { tier, confidence } = &settle.status else {
            unreachable!()
        };
        assert_eq!(
            *tier,
            MatchTier::Fingerprint { score: 4.0 / 6.0 },
            "the measured Jaccard rides the tier"
        );
        assert!(close(*confidence, 4.0 / 6.0));
    }

    #[test]
    fn t3_below_threshold_yields_new() {
        // Disjoint tiny bodies: Jaccard 0.0 < 0.6 → New/Terminated.
        let before = fn_with_calls(BEFORE, 10, "src/lib.rs:foo:1", &["a"]);
        let after = fn_with_calls(AFTER, 20, "src/lib.rs:bar:1", &["b"]);

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        assert_eq!(
            outcome(&result, AFTER, "src/lib.rs:bar:1").status,
            ContinuityStatus::New
        );
        assert_eq!(
            outcome(&result, BEFORE, "src/lib.rs:foo:1").status,
            ContinuityStatus::Terminated
        );
    }

    #[test]
    fn t3_kind_prefilter_blocks_cross_kind_fingerprint_match() {
        // Identical bodies, different kinds: similarity is 0.0 by the hard
        // pre-filter even though the multiset Jaccard would be high.
        let mut before = vec![defines(BEFORE, 10, 10, "src/lib.rs:widget:3", "Function")];
        before.push(calls(BEFORE, 101, 10, "draw"));
        before.push(calls(BEFORE, 102, 10, "measure"));
        let mut after = vec![defines(AFTER, 20, 20, "src/lib.rs:widget:9", "Struct")];
        after.push(calls(AFTER, 201, 20, "draw"));
        after.push(calls(AFTER, 202, 20, "measure"));

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        assert_eq!(
            outcome(&result, AFTER, "src/lib.rs:widget:9").status,
            ContinuityStatus::New
        );
    }

    #[test]
    fn t3_bodyless_entities_never_match() {
        // Different names (T0/T1 cannot fire) and NO calls/refs on either
        // side: no structural evidence → fail-closed New (fingerprint D4 —
        // the name element alone must never bless a T3 match).
        let before = vec![defines(BEFORE, 10, 10, "src/lib.rs:flag:1", "Function")];
        let after = vec![defines(AFTER, 20, 20, "src/lib.rs:banner:7", "Function")];

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        assert_eq!(
            outcome(&result, AFTER, "src/lib.rs:banner:7").status,
            ContinuityStatus::New
        );
    }

    #[test]
    fn t3_epsilon_tie_fails_closed() {
        // Identical bodies on both before-candidates: scores tie at 1.0 →
        // Ambiguous regardless of candidate order (design D3: ordering
        // never rescues a within-epsilon tie).
        let mut before = fn_with_calls(BEFORE, 10, "src/a.rs:dup:1", &["h1", "h2", "h3"]);
        before.extend(fn_with_calls(
            BEFORE,
            11,
            "src/b.rs:dup:1",
            &["h1", "h2", "h3"],
        ));
        let after = fn_with_calls(AFTER, 20, "src/c.rs:dup:1", &["h1", "h2", "h3"]);

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        assert_ambiguous(
            outcome(&result, AFTER, "src/c.rs:dup:1"),
            &["src/a.rs:dup:1", "src/b.rs:dup:1"],
        );
    }

    #[test]
    fn t3_margin_beyond_epsilon_prefers_the_higher_score_not_the_lower_fqn() {
        // b.rs scores 1.0 (identical body), a.rs scores 10/12 ≈ 0.833. The
        // margin (0.167) exceeds epsilon → b.rs wins EVEN THOUGH a.rs sorts
        // first — ordering enumerates, the score decides.
        let mut before = fn_with_calls(BEFORE, 10, "src/a.rs:dup:1", &["h1", "h2", "h3", "x"]);
        before.extend(fn_with_calls(
            BEFORE,
            11,
            "src/b.rs:dup:1",
            &["h1", "h2", "h3"],
        ));
        let after = fn_with_calls(AFTER, 20, "src/c.rs:dup:1", &["h1", "h2", "h3"]);

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        let dup = outcome(&result, AFTER, "src/c.rs:dup:1");
        assert_matched(dup, 2); // b.rs:dup:1 is pool position 2
        let ContinuityStatus::Matched { tier, .. } = &dup.status else {
            unreachable!()
        };
        assert_eq!(
            *tier,
            MatchTier::Fingerprint { score: 1.0 },
            "the identical body wins on score"
        );
    }

    // ---------------------------------------------------------------------
    // T1 ambiguity, terminality, double-claim protection.
    // ---------------------------------------------------------------------

    #[test]
    fn multiple_t1_candidates_fail_closed() {
        // Same-file overloads (same path+name+kind, different lines) that
        // all shift lines: perfectly tied T1 candidates → Ambiguous
        // (design open question, accepted fail-closed in v1).
        let mut before = vec![defines(BEFORE, 10, 10, "src/lib.rs:load:4", "Function")];
        before.push(defines(BEFORE, 11, 11, "src/lib.rs:load:9", "Function"));
        let after = vec![defines(AFTER, 20, 20, "src/lib.rs:load:15", "Function")];

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        assert_ambiguous(
            outcome(&result, AFTER, "src/lib.rs:load:15"),
            &["src/lib.rs:load:4", "src/lib.rs:load:9"],
        );
    }

    #[test]
    fn ambiguous_is_terminal_and_never_rescued_by_a_lower_tier() {
        // Two overloads tie at T1 even though ONE of them would T3-match the
        // after-entity with a clearly better body: Ambiguous is terminal
        // per pair — no fall-through, no retroactive force-match (A2).
        let mut before = vec![defines(BEFORE, 10, 10, "src/lib.rs:load:4", "Function")];
        before.extend(fn_with_calls(
            BEFORE,
            11,
            "src/lib.rs:load:9",
            &["h1", "h2", "h3"],
        ));
        let after = fn_with_calls(AFTER, 20, "src/lib.rs:load:15", &["h1", "h2", "h3"]);

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        assert_ambiguous(
            outcome(&result, AFTER, "src/lib.rs:load:15"),
            &["src/lib.rs:load:4", "src/lib.rs:load:9"],
        );
        // The body-identical overload stays UNCLAIMED → Terminated; the
        // fingerprint evidence must not retroactively rescue the pair.
        assert_eq!(
            outcome(&result, BEFORE, "src/lib.rs:load:9").status,
            ContinuityStatus::Terminated
        );
    }

    // ---------------------------------------------------------------------
    // Pool, New, Terminated, no-resurrection, ordering.
    // ---------------------------------------------------------------------

    #[test]
    fn stable_ids_are_pooled_in_sorted_fqn_order() {
        let mut before = vec![defines(BEFORE, 10, 10, "src/z.rs:last:1", "Function")];
        before.push(defines(BEFORE, 11, 11, "src/a.rs:first:1", "Function"));

        let result = match_snapshots(&before, &[], &[], &MatcherThresholds::default());

        assert_eq!(
            outcome(&result, BEFORE, "src/a.rs:first:1").stable_id,
            Some(StableEntityId::new(1)),
            "sorted-FQN order assigns the pool"
        );
        assert_eq!(
            outcome(&result, BEFORE, "src/z.rs:last:1").stable_id,
            Some(StableEntityId::new(2))
        );
    }

    #[test]
    fn new_entities_get_fresh_ids_after_the_pool_max_in_sorted_fqn_order() {
        let before = vec![defines(BEFORE, 10, 10, "src/lib.rs:keep:1", "Function")];
        let mut after = vec![defines(AFTER, 20, 20, "src/lib.rs:keep:1", "Function")];
        after.push(defines(AFTER, 21, 21, "src/zeta.rs:fresh:1", "Function"));
        after.push(defines(AFTER, 22, 22, "src/alpha.rs:fresh:1", "Function"));

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        assert_eq!(
            outcome(&result, AFTER, "src/lib.rs:keep:1").status,
            ContinuityStatus::Matched {
                tier: MatchTier::ExactIdentity,
                confidence: 1.0,
            }
        );
        // K = 1: fresh ids start at 2, assigned in sorted-FQN order.
        assert_eq!(
            outcome(&result, AFTER, "src/alpha.rs:fresh:1").stable_id,
            Some(StableEntityId::new(2))
        );
        assert_eq!(
            outcome(&result, AFTER, "src/zeta.rs:fresh:1").stable_id,
            Some(StableEntityId::new(3))
        );
    }

    #[test]
    fn terminated_entities_retain_their_stable_id() {
        let mut before = vec![defines(BEFORE, 10, 10, "src/lib.rs:keep:1", "Function")];
        before.push(defines(BEFORE, 11, 11, "src/lib.rs:gone:5", "Function"));
        let after = vec![defines(AFTER, 20, 20, "src/lib.rs:keep:1", "Function")];

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        let gone = outcome(&result, BEFORE, "src/lib.rs:gone:5");
        assert_eq!(gone.status, ContinuityStatus::Terminated);
        // Pool order is sorted-FQN: "gone" < "keep", so gone held stable:1.
        assert_eq!(gone.stable_id, Some(StableEntityId::new(1)), "id retained");
    }

    #[test]
    fn reintroduced_symbol_is_new_not_resurrected() {
        // A1 no-resurrection: the matcher is strictly pairwise N → N+1 with
        // no history table, so a symbol that disappears in N+1 and returns
        // in N+2 is NEW in the (N+1 → N+2) pair — the earlier run's stable
        // id is never revived across calls.
        let pair_one = match_snapshots(
            &[defines(BEFORE, 10, 10, "src/lib.rs:ghost:1", "Function")],
            &[],
            &[],
            &MatcherThresholds::default(),
        );
        assert_eq!(
            outcome(&pair_one, BEFORE, "src/lib.rs:ghost:1").status,
            ContinuityStatus::Terminated
        );

        let before_two = [defines(AFTER, 20, 20, "src/lib.rs:anchor:2", "Function")];
        let mut after_two = vec![defines(AFTER, 31, 31, "src/lib.rs:anchor:2", "Function")];
        after_two.push(defines(AFTER, 30, 30, "src/lib.rs:ghost:44", "Function"));
        let pair_two = match_snapshots(&before_two, &after_two, &[], &MatcherThresholds::default());

        let ghost = outcome(&pair_two, AFTER, "src/lib.rs:ghost:44");
        assert_eq!(
            ghost.status,
            ContinuityStatus::New,
            "no history: the reintroduced symbol is a fresh entity"
        );
        assert_eq!(
            ghost.stable_id,
            Some(StableEntityId::new(2)),
            "fresh id after the current pair's pool max (anchor=1)"
        );
    }

    #[test]
    fn outcomes_are_sorted_by_snapshot_then_fqn() {
        let mut before = vec![defines(BEFORE, 10, 10, "src/lib.rs:keep:1", "Function")];
        before.push(defines(BEFORE, 11, 11, "src/lib.rs:gone:5", "Function"));
        let mut after = vec![defines(AFTER, 20, 20, "src/lib.rs:keep:1", "Function")];
        after.push(defines(AFTER, 21, 21, "src/aaa.rs:fresh:1", "Function"));

        let result = match_snapshots(&before, &after, &[], &MatcherThresholds::default());

        assert_eq!(
            result.outcomes.len(),
            3,
            "2 after (matched + new) + 1 terminated"
        );
        let keys: Vec<(SnapshotId, &str)> = result
            .outcomes
            .iter()
            .map(|o| (o.snapshot, o.fqn.as_str()))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted, "outcomes must be canonically sorted");
    }

    #[test]
    fn empty_sides_degrade_to_new_and_terminated() {
        let thresholds = MatcherThresholds::default();

        let from_empty = match_snapshots(
            &[],
            &[defines(AFTER, 20, 20, "src/x.rs:a:1", "Function")],
            &[],
            &thresholds,
        );
        assert_eq!(from_empty.outcomes.len(), 1);
        assert_eq!(from_empty.outcomes[0].status, ContinuityStatus::New);
        assert_eq!(
            from_empty.outcomes[0].stable_id,
            Some(StableEntityId::new(1))
        );

        let to_empty = match_snapshots(
            &[defines(BEFORE, 10, 10, "src/x.rs:a:1", "Function")],
            &[],
            &[],
            &thresholds,
        );
        assert_eq!(to_empty.outcomes.len(), 1);
        assert_eq!(to_empty.outcomes[0].status, ContinuityStatus::Terminated);

        let both_empty = match_snapshots(&[], &[], &[], &thresholds);
        assert!(both_empty.outcomes.is_empty());
    }

    // ---------------------------------------------------------------------
    // Permutation invariance (spec "Tiered deterministic continuity
    // matching": identical fact sets MUST yield identical mappings
    // regardless of commit order).
    // ---------------------------------------------------------------------

    /// A rich scenario touching every tier and every status: T0, T1, T2,
    /// T3 matches; New; Terminated; and a within-margin T2 collision.
    fn rich_scenario() -> (Vec<Fact>, Vec<Fact>, Vec<FileRename>) {
        let mut before = fn_with_calls(BEFORE, 10, "src/lib.rs:keep:1", &["alpha", "beta"]);
        before.extend(fn_with_calls(
            BEFORE,
            11,
            "src/engine.rs:tick:12",
            &["reset", "step"],
        ));
        before.extend(fn_with_calls(
            BEFORE,
            12,
            "src/config.rs:parse:5",
            &["read", "split"],
        ));
        before.extend(fn_with_calls(
            BEFORE,
            13,
            "src/old.rs:legacy:2",
            &["x1", "x2"],
        ));
        before.extend(fn_with_calls(
            BEFORE,
            14,
            "src/a.rs:handle:1",
            &["h1", "h2"],
        ));
        before.extend(fn_with_calls(
            BEFORE,
            15,
            "src/b.rs:handle:1",
            &["h1", "h2"],
        ));

        let mut after = fn_with_calls(AFTER, 20, "src/lib.rs:keep:1", &["alpha", "beta"]);
        after.extend(fn_with_calls(
            AFTER,
            21,
            "src/engine.rs:tick:40",
            &["reset", "step"],
        ));
        after.extend(fn_with_calls(
            AFTER,
            22,
            "src/parsing/config.rs:parse:5",
            &["read", "split"],
        ));
        after.extend(fn_with_calls(AFTER, 23, "src/new.rs:fresh:1", &["n1"]));
        after.extend(fn_with_calls(AFTER, 24, "src/c.rs:handle:1", &["h1", "h2"]));
        after.extend(fn_with_calls(AFTER, 25, "src/gone.rs:vanish:1", &["v1"]));

        let renames = vec![
            rename("src/config.rs", "src/parsing/config.rs", 0.9),
            rename("src/a.rs", "src/c.rs", 0.9),
            rename("src/b.rs", "src/c.rs", 0.87),
        ];
        (before, after, renames)
    }

    /// Deterministic xorshift-seeded Fisher-Yates (view.rs test precedent;
    /// no rand dependency).
    fn shuffle<T>(items: &mut [T], seed: u64) {
        let mut state = seed;
        for i in (1..items.len()).rev() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let j = (state % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }

    #[test]
    fn result_is_identical_under_shuffled_fact_and_rename_input() {
        let (before, after, renames) = rich_scenario();
        let canonical = match_snapshots(&before, &after, &renames, &MatcherThresholds::default());

        // Sanity: the rich scenario really touches every tier and status.
        assert_eq!(
            outcome(&canonical, AFTER, "src/lib.rs:keep:1").status,
            ContinuityStatus::Matched {
                tier: MatchTier::ExactIdentity,
                confidence: 1.0,
            }
        );
        assert_eq!(
            outcome(&canonical, AFTER, "src/engine.rs:tick:40").status,
            ContinuityStatus::Matched {
                tier: MatchTier::PathNameKind,
                confidence: 1.0,
            }
        );
        assert!(matches!(
            &outcome(&canonical, AFTER, "src/parsing/config.rs:parse:5").status,
            ContinuityStatus::Matched {
                tier: MatchTier::RenameEvidence { .. },
                ..
            }
        ));
        assert!(matches!(
            &outcome(&canonical, AFTER, "src/c.rs:handle:1").status,
            ContinuityStatus::Ambiguous { .. }
        ));
        assert_eq!(
            outcome(&canonical, AFTER, "src/new.rs:fresh:1").status,
            ContinuityStatus::New
        );
        assert_eq!(
            outcome(&canonical, BEFORE, "src/old.rs:legacy:2").status,
            ContinuityStatus::Terminated
        );

        // Shuffled before, after, and rename input (independently) must
        // reproduce the canonical result EXACTLY.
        for seed in [1u64, 7, 42, 12345] {
            let mut shuffled_before = before.clone();
            shuffle(&mut shuffled_before, seed);
            let mut shuffled_after = after.clone();
            shuffle(&mut shuffled_after, seed.wrapping_mul(31));
            let mut shuffled_renames = renames.clone();
            shuffle(&mut shuffled_renames, seed.wrapping_add(5));

            let permuted = match_snapshots(
                &shuffled_before,
                &shuffled_after,
                &shuffled_renames,
                &MatcherThresholds::default(),
            );
            assert_eq!(
                permuted, canonical,
                "shuffled inputs (seed {seed}) must produce the identical ContinuityResult"
            );
        }
    }

    #[test]
    fn duplicated_rename_rows_collapse_to_max_regardless_of_order() {
        let (before, after, renames) = rich_scenario();
        let canonical = match_snapshots(&before, &after, &renames, &MatcherThresholds::default());

        // Duplicate rename rows collapse to MAX similarity, so the outcome
        // must be identical no matter the row order (the 0.5 duplicate for
        // src/a.rs → src/c.rs must never lower the 0.9 evidence).
        let mut doubled = vec![
            rename("src/b.rs", "src/c.rs", 0.87),
            rename("src/a.rs", "src/c.rs", 0.9),
            rename("src/a.rs", "src/c.rs", 0.5),
            rename("src/config.rs", "src/parsing/config.rs", 0.9),
        ];
        let merged = match_snapshots(&before, &after, &doubled, &MatcherThresholds::default());
        assert_eq!(
            merged, canonical,
            "duplicate rows collapse to max; the result is unchanged"
        );

        // …and permuting the doubled rows changes nothing either.
        for seed in [3u64, 99] {
            shuffle(&mut doubled, seed);
            let permuted =
                match_snapshots(&before, &after, &doubled, &MatcherThresholds::default());
            assert_eq!(
                permuted, canonical,
                "seed {seed} must not change the result"
            );
        }
    }
}
