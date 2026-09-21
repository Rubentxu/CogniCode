# Archive Manifest — cycle e69 — EvidenceBundle + PolicyGate

> Cycle: A-lite (degraded-but-governed) | Milestone: M8 — Scheduler + CI | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e69 |
| Milestone | M8 — Scheduler + CI (foundation: structured evidence → policy decision) |
| Requirement | feed e68's `AffectedWorkPlan` decisions with structured evidence; produce a gate verdict |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `02592ff4` (e68) |
| Spec delta | none (foundation cycle; spec lives in `proposal.md`) |

## Lifecycle note (honest)

**e69 was never instantiated as a formal SDDK cycle in the ledger.** Same pattern as e67+e68:
three work units authored with deterministic checkpoints, single commit `b39bb524` carrying
all three modules together (WU2 and WU3 depend on WU1 types). Verification report (already on
disk) is the durable record.

This archive commit closes the on-disk artifact gap (proposal + verification-report were
already committed; archive-manifest was missing).

## Commits

| Commit | Kind | Summary |
|--------|------|---------|
| `b39bb524` | `feat(e69)` | WU1+WU2+WU3 in a single commit |

## Delivered

### WU1 — EvidenceBundle + BundleEntry (`crates/cognicode-core/src/application/evidence_bundle/mod.rs`)

- `EvidenceBundle { entries: Vec<BundleEntry>, ... }` — ordered, deterministic.
- `BundleEntry` variants: `Evidence` / `Missing` / `Failed` / `Unknown` (the four terminal
  classifications of any work-result source).
- `ready_for_gate()` requires NO `Failed` / `Missing` / `Unknown` entries.
- 7/7 unit tests: empty bundle is well-formed; four kinds coexist without collapsing;
  `ready_for_gate` semantics; ordering stable across input reorder; bundle ids unique;
  slot lookup consistent.

### WU2 — EvidenceProducer (`crates/cognicode-core/src/application/evidence_bundle/producer.rs`)

- `EvidenceProducer` trait (one impl per source kind): `cargo test`, `just`, `cogh`,
  detector run, build result, analysis result.
- `pub fn produce(&self, ...) -> BundleEntry` — maps raw output to one of the four variants.
- No I/O policy dictated by this layer; each producer decides its own capture protocol.

### WU3 — PolicyGate (`crates/cognicode-core/src/application/policy_gate/mod.rs`)

- `PolicyGate::evaluate(bundle: &EvidenceBundle, policy: &GatePolicy) -> GateVerdict`.
- `GateVerdict`: `Pass | Warn | Block | InsufficientEvidence`.
- `GatePolicy` declares thresholds for each `EvidenceKind`; the gate aggregates.
- No scheduler; no retries; no remote CI.

## Architectural decisions (per proposal)

1. Bundle = the unit of truth passed to the gate (NOT a log; NOT a stream).
2. Four terminal classifications: `Evidence` / `Missing` / `Failed` / `Unknown`. Anything
   outside this taxonomy is a producer-side error.
3. `ready_for_gate` is the precondition; the gate itself never has to know.
4. Verdict semantics are local to the gate (no global "policy registry" yet — that is a
   separate follow-on cycle when multiple policies coexist).

## Non-goals (deferred to e70+)

- GitHub Actions remote.
- Jenkins / distributed workers.
- A scheduler with retries.
- A global policy registry (multiple coexisting policies).
- CLI surface (`why_decided`).
- EvidenceBundle persistence.

## Acceptance

The verification report enumerates the 7+evidence-bundle tests + 6+policy-gate adversarial
tests, all GREEN, plus the 6-case adversarial matrix (per the e67+e68 precedent). Verdict
semantics: `Pass` requires no `Failed` / `Missing` / `Unknown`; `Warn` allows non-fatal
`Unknown`; `Block` triggers on any `Failed`; `InsufficientEvidence` triggers on missing
required slots.

## Static gates (all clean)

- `cargo fmt --check -p cognicode-core` → clean.
- `cargo clippy -p cognicode-core --all-targets --features evidence-kernel -- -D warnings`
  → clean on touched paths.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — release route blocked; closure via direct push.
- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode.

## Related cycles

- **e68** (predecessor): semantic diff + affected work planner.
- **e70** (successor): local CI vertical (planner + executor + bundle + gate + why_*).
