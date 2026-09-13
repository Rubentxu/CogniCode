# ADR Review Record — e36 LSI Evidence Kernel Foundation

Review trail for **ADR-037..ADR-051** (umbrella task 1.5: "review and number proposed ADRs").
Numbering was assigned during the LSI kickoff (`docs/adr/README.md` rows 033–051, incorporating
the `CogniCode_Living_Software_Intelligence` package); this record completes the review trail
for the e36 slice. Per the package incorporation rule, every ADR below stays **PROPOSED** until
its spike dependency validates the decision — e36 provides supporting evidence, not acceptance.

## Deep review — kernel ADRs exercised by e36 (ADR-037..040)

| ADR | Title | Related e36/LSI surface | Spike dependency to validate before ACCEPTED | Evidenced by | Status |
|---|---|---|---|---|---|
| ADR-037 | Facts over Graphs as canonical knowledge | `domain/evidence_kernel::fact`/`relation` (`Fact`, `FactValue`, `RelationKind`); M0 goldens freeze graph-consumer outputs before the kernel replaces them | Producer pipeline emitting `core:calls` facts for a fixture repo with structural equivalence to legacy graph queries (M2 bridge, UAT-U10) | D1 (kernel module layout), D3 (facts as typed assertions), D6 (registry-governed vocabulary), D8 (M0 freeze first) | PROPOSED |
| ADR-038 | Stable Entity Identity separated from Occurrence | `domain/evidence_kernel::ids` (`EntityId` vs `OccurrenceId`) | M3 continuity spike across comment-shift / file-move / rename fixtures (UAT-U20..U22) | D1 (`ids` module) | PROPOSED |
| ADR-039 | Snapshots as reproducible analysis experiments | `SnapshotId`↔`RevisionId` bijection, `SnapshotDescriptor`, `SnapshotStore::from_revision`, pinned reads | Replaying one revision reproduces an identical descriptor (`source_state` + `config_digest`) through the bench/golden harness (UAT-U02/U06) | D4 (facade over revision model), D5 (pinned reads) | PROPOSED |
| ADR-040 | Separate Fact, Evidence and Hypothesis | `Fact::new` rejects `ProducerKind::LlmAgent`; `Evidence`/`EvidenceGrade`; `ProvenanceRecord` wraps legacy `Provenance`; kernel-namespaced `EvidenceStore` port | Explicit hypothesis→fact promotion flow driven by a deterministic verifier (UAT-U90) | D2 (kernel-namespaced `EvidenceStore`, legacy port untouched), D3 (provenance composition + LlmAgent rejection) | PROPOSED |

## Shallow review — ADRs with no e36 runtime surface in this slice

| ADR | Title | Related LSI surface (milestone) | Spike dependency to validate before ACCEPTED | Status |
|---|---|---|---|---|
| ADR-041 | Storage backend separated from incremental compute | M2 projection bridge / store migration (Ladybug adapter deliberately deferred — D7) | Ladybug `FactStore` adapter (ADR-028 pattern) passing in-memory oracle conformance | PROPOSED |
| ADR-042 | Detector IR with cost-aware escalation | M5 detectors/findings (UAT-U40..U42) | Detector IR prototype escalating one analyzer on a sandbox fixture | PROPOSED |
| ADR-043 | Intelligence Event Log for causal operational history | M7 reactive runtime (UAT-U50 causal trace) | Event-log replay reproducing a source-delta→finding causal chain | PROPOSED |
| ADR-044 | Three behavior classes with different authority | M7 behavior classes (UAT-U52) | Policy enforcement distinguishing the three classes on one sandbox scenario | PROPOSED |
| ADR-045 | Execution read sets as first-class lineage | M7 CI planning (UAT-U61) | Read-set capture on a sandbox scenario driving test selection | PROPOSED |
| ADR-046 | Software World Fork, Trial, Diff and Promote | M9 fork/promote (UAT-U70..U72) | Fork→mutate→promote cycle with audited conflict rejection | PROPOSED |
| ADR-047 | Evidence Bundle as CI promotion unit | M8 semantic PR diff / CI (UAT-U60..U63) | Evidence bundle gating a real PR pipeline | PROPOSED |
| ADR-048 | Packs as extension and governance unit | M10 packs (UAT-U80) | Pack manifest with OBSERVE authority proven unable to gate | PROPOSED |
| ADR-049 | Architecture knowledge as versioned executable constraints | M10 executable ADR drift (UAT-U81) | Boundary violation detected and linked as a drift finding | PROPOSED |
| ADR-050 | Code authorship without authority for AI and packs | M11 agents (UAT-U72, UAT-U91) | Agent SourcePatch rejected without trial/gate; prompt-injection containment | PROPOSED |
| ADR-051 | Historical replay and held-out promotion | M13 continuous improvement (UAT-U110..U112) | Optimize/confirm dataset separation enforced; governed promotion audited end-to-end | PROPOSED |

## Review verdicts (e36 scope)

- ADR-037 / 039 / 040: design decisions D1–D8 hold under the e36 suite (41 kernel tests green;
  `cargo check -p cognicode-core` identical with the feature off and on; goldens byte-stable).
  No design defect found; the listed spikes remain the gate to ACCEPTED.
- ADR-038: the EntityId/OccurrenceId split is in place as designed, but no continuity behavior
  exists yet in e36 — the spike is entirely M3; nothing in this slice contradicts the ADR.
- ADR-041: e36 confirms the separation is viable (in-memory adapters only, D7 deferral), which is
  supporting evidence, not validation.
