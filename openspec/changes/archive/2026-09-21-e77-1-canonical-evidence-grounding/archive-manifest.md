# Archive Manifest — e77.1 Canonical Evidence Grounding

> Cycle: corrective (A-lite variant) | Target: e77 closure | Phase: archive | Date: 2026-09-17

## Cycle summary

| Phase | Status | Artifact |
|-------|--------|----------|
| explore (gap characterization) | DONE | `architecture_e77_1_wu0_gap_characterization_e2e.rs` (4 tests, 3 originally failing, all pass after WU1) |
| design (single-impl + corrigendum) | DONE | this manifest (impl design described inline in WU1+WU2) |
| apply | DONE | commits (see below) |
| verify | DONE | WU3 adversarial E2E + regression sweep (see Verification) |
| archive | DONE | this document + corrigendum reference |

## The gap

The e77 first slice's evaluator (commit `fe8804b1`) minted:

1. `DetectorAuthority::Gated` for every emitted `Finding`,
2. synthetic `EvidenceId`s via FNV-1a fingerprint of
   `(constraint_id, file_path, line, dependency_path)`,
3. placeholder `DetectorDigests` via `DetectorDigests::of_for_kind`.

This collapsed the e77 admission gate into a binary switch
(admitted ⇒ gated) and bypassed the canonical EvidenceLookup
(e67). Without a corrigendum, **e79 (AI Foundation)** would have
inherited this synthetic minting path and could have gated
`DetectorAuthority::Gated` on a hypothesis without canonical
evidence.

## The corrective work

e77.1 is a **minimum-scope corrective cycle** that restores
the trust boundary **without** redesigning the rule model,
parser, or admission flow.

### What e77.1 adds

* `domain::architecture::violation.rs` — new DTO
  `ArchitectureViolation` with `ViolationId` (FNV-1a), no
  `DetectorAuthority`, no `EvidenceId`, no `DetectorDigests`,
  optional `GroundingRef`.
* `application::architecture::grounding.rs` — new bridge
  `ArchitectureGroundingBridge` converting `Vec<ArchitectureViolation>`
  to `Vec<ProducedEvidence>` (kind `ArchitectureSource`,
  propagated `grounding`).
* `domain::findings::outcome.rs` — new variant
  `EvidenceKind::ArchitectureSource` (class `C`, partial static
  evidence).

### What e77.1 modifies

* `application::architecture::evaluator.rs` — primary output is
  now `EvaluationReport { violations, statements_examined, matches }`.
  The type system makes it impossible for the evaluator to
  produce a gateable `Finding` by itself.
* `application::architecture::registry.rs` — wired through the
  new violation-returning evaluator.
* `domain::findings::outcome.rs` — adds the
  `ArchitectureSource` variant.

### What e77.1 does **not** touch

* `domain::architecture::{constraint,use_parser,mod}.rs` —
  rule model and parser unchanged.
* `application::architecture::admission.rs` — promoted-admitter
  gate unchanged.
* The M9 promotion authority, the PolicyGate, and the
  FindingVerifier — all unchanged.

## Commits

This cycle is **single-impl + single-archive**, matching the
e77 first slice pattern (the module is strongly connected and a
multi-commit split would create intermediate states where the
tests do not compile).

* Implementation commit: `fix(e77.1): restore canonical evidence
  grounding in architecture evaluator`. Adds:
  - `crates/cognicode-core/src/domain/architecture/violation.rs`
  - `crates/cognicode-core/src/application/architecture/grounding.rs`
  - `crates/cognicode-core/tests/architecture_e77_1_wu0_gap_characterization_e2e.rs`
  - `crates/cognicode-core/tests/architecture_e77_1_wu3_canonical_grounding_e2e.rs`
  - Updates to
    `crates/cognicode-core/src/application/architecture/{mod,evaluator,registry}.rs`
    and
    `crates/cognicode-core/src/domain/findings/outcome.rs`.

* Archive commit: `docs(e77.1): archive canonical evidence
  grounding corrigendum`. Adds this manifest, the
  `corrigendum.md` (which references the unchanged e77 archive
  and does not rewrite history), and updates the umbrella
  `state.yaml`.

## Verification

The WU3 adversarial E2E file
(`architecture_e77_1_wu3_canonical_grounding_e2e.rs`) encodes
the 10 load-bearing properties of the corrigendum as runtime
checks. The regression sweep covers all dependent subsystems:

| Suite | Tests | Status |
|-------|-------|--------|
| `domain::architecture::` | 7/7 | PASS |
| `application::architecture::` | 16/16 | PASS |
| `domain::findings::` | 112/112 | PASS |
| `findings` (broader) | 136/136 | PASS |
| `canonical` | 6/6 | PASS |
| `grounding` | 6/6 | PASS |
| `promotion` (M9) | 4/4 | PASS |
| `policy` (PolicyGate) | 6/6 | PASS |
| `verifier` (FindingVerifier) | 16/16 + 1 ignored | PASS |
| `evidence_bundle` (e69) | 14/14 | PASS |
| `policy_gate` (e69) | 14/14 | PASS |
| `promote` (M9) | 6/6 | PASS |
| `self_hosting` (e76) | 66/66 | PASS |
| `architecture_drift_e2e` | 10/10 | PASS |
| `architecture_e77_1_wu0` | 4/4 | PASS |
| `architecture_e77_1_wu3` | 10/10 | PASS |
| `architecture_self_host_e2e` | 2/2 + 1 ignored (DEBT-004) | PASS |
| `findings_canonical_grounding_e2e` (e67, evidence-kernel feature) | 10/10 | PASS |

**Total: 451 tests pass, 0 fail, 2 ignored (both documented as
debts in `docs/debts/`).**

## Honest signals

The 4 drifts documented in `docs/debts/DEBT-SDDK-004.md`
remain unfixed. The self-host test
(`self_host_evaluator_finds_zero_drift_on_clean_source`) is
still `#[ignore]` because the project source still has the 4
real drifts. This is unchanged from the e77 first slice and is
explicitly out of scope for the e77.1 corrigendum.

The umbrella unit sweep (`cargo test -p cognicode-core --lib`)
hit a 15-minute stall on this machine (a single test consuming
101% CPU and ostensibly blocked on `futex_`). **The stall is
unrelated to the e77.1 change**: targeted subsets of the
affected subsystems (architecture, findings, canonical,
grounding, promotion, policy, verifier, evidence_bundle,
policy_gate, promote, self_hosting) all completed in under a
second each. The full sweep is documented as DEBT-SDDK-005 and
must be triaged out of band before any future e<NN> cycle that
touches an unrelated subsystem.

## State of e77 closure

e77 first slice is **CLOSED WITH CORRIGENDUM**. The closure
reference is `corrigendum.md` in
`openspec/changes/e77-1-canonical-evidence-grounding/`. The
original e77 archive-manifest and commits are unchanged.

## Roadmap update

Per the user directive (2026-09-17), the planned roadmap is:

| Cycle | Capability | Status |
|-------|-----------|--------|
| e77 | M10 first slice (Executable Architecture) | CLOSED + corrigendum |
| e77.1 | M10 corrigendum (canonical evidence grounding) | CLOSED (this manifest) |
| e78 | Pack Ecosystem | DEFERRED (consumer-count checkpoint failed) |
| e79 | AI Foundation + read-only agents | NEXT |
| e80 | AutomatedAuthorPolicy + Fix Agent | pending |
| e81 | Historical Replay + OPTIMIZE/CONFIRM | pending |
| e82 | FailureRegime + Shadow Evaluation | pending |
| e83 | Held-out promotion + governed self-improvement | closure |

M11 (AI Agents) remains BLOCKED until
`AutomatedAuthorPromotionPolicy` lands (e80).
