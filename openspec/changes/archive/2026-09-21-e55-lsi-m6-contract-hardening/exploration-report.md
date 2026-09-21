# Exploration Report — cycle e55 — M6 contract hardening

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

External review of e52–e54 (Detector IR, Finding model, QualityIssue
projection) identified contract gaps that must be closed **before** any
detector backend (tasks 7.3–7.5) is written, so every backend shares one
coherent contract rather than inventing its own interpretation.

## Findings from the review (adopted)

1. **P0 — authority is lost between detector and finding.** `Finding` kept
   only `DetectorRef{id,version}`; `Finding::can_block()` checked evidence
   class and risk but *not* detector authority. A future backend could emit
   a blocking finding from a `Candidate` detector. Authority must be
   captured **at execution time** (a later promotion to `Gated` must not
   retroactively grant authority).
2. **`AnalysisLevel` conflates capability and cost.** The linear ladder
   made `LLM_CONTEXT >= SYMBOLIC` by `Ord`, so an LLM-only detector could
   satisfy `VERIFY feasible_path`. Capability (a set) and escalation cost
   (a tier) are different concepts.
3. **`DetectorId` was only "non-empty"**, not namespaced, exposing
   pack/federation collisions.
4. **`EvidenceClass` doc bug**: the comment said `satisfies(A) is true for
   every class`; it is `satisfies(D)`.
5. **`FindingStatus` collapsed `false_positive`/`wontfix`/`suppressed`/
   `ignored`** into `FalsePositive`, losing governance semantics.
6. **Legacy origin was lost**: the projection dropped `issue_id`/`rule_id`
   unless the message was empty.
7. **Three copies of the `ns.name` validator** would drift.
8. **Governance ambiguity**: umbrella `tasks.md` checkboxes and
   `state.yaml` disagreed on completion.

## Deferred (deliberately, to keep this slice small)

- **Kernel id extraction + `EvidenceRef = EvidenceId`** and **structured
  `CausalStep`** (EntityId/FactId/EvidenceId refs). Both require touching
  the feature-gated `evidence_kernel`; they are isolated into cycle **e56**
  so this cycle stays low-risk and bounded.

## Strategy

M6-local hardening only (no kernel changes):

- new `findings::namespaced` (single `ns.name` validator);
- `AnalysisCapability` (set) + `EscalationTier` (cost) replacing
  `AnalysisLevel`;
- `DetectorExecutionRef{id,version,authority_at_execution,detector_digest,
  execution_id}` and `Finding.detector` of that type; `can_block()` now
  also requires `authority_at_execution.can_block()`;
- `DetectorIr::digest()` (fnv1a64 content digest, house convention);
- `DetectorId` namespaced;
- `FindingStatus` += `RiskAccepted`, `Suppressed`;
- `FindingOrigin` + `Finding.origin`;
- QualityIssue projection updated (origin, distinct statuses, Candidate
  authority at execution);
- doc fix; governance fix in the umbrella.

## Impact model

| Artifact | Impact | Confidence |
|----------|--------|-----------|
| `domain/findings/*` | M6-local API change (no consumers yet) | KNOWN |
| `evidence_kernel` | untouched | KNOWN |
| other crates | none (no consumers) | LIKELY |
