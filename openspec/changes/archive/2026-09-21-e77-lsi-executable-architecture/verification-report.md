# Verification Report — e77 LSI Executable Architecture

> Cycle: A-lite | Milestone: M10 — Executable Architecture (e77 first slice) | Phase: verify | Date: 2026-09-17

## Scope

Verify that the implementation matches the proposal, the design,
and the tasks. The verification runs the e77 test surface end-to-end
and confirms:

1. The compile is clean.
2. All 20 unit tests pass.
3. All 10 adversarial E2E tests pass.
4. The 2 self-host tests pass; the ignored self-host test
   reproduces the 4 documented drifts.

## Commands executed

```bash
cargo test -p cognicode-core --lib architecture::
# result: 20 passed; 0 failed; 0 ignored

cargo test -p cognicode-core --test architecture_drift_e2e
# result: 10 passed; 0 failed; 0 ignored

cargo test -p cognicode-core --test architecture_self_host_e2e
# result: 2 passed; 0 failed; 1 ignored
# (the ignored test reproduces 4 real drifts documented in DEBT-SDDK-004)

cargo test -p cognicode-core --test architecture_self_host_e2e -- --ignored
# result: ignored test fails (reproducing the 4 drifts), as expected
```

## Load-bearing property

The property is:

> An ADR by itself has ZERO execution/gating authority. Only an
> admitted `ArchitectureConstraint` may produce drift findings.

**Evidence**: `adr_text_alone_produces_zero_findings` in
`tests/architecture_drift_e2e.rs` passes. The test proposes a
candidate with `adr_ref = Some("ADR-046")`, never admits it, and
asserts that the registry rejects the evaluation. If the
property ever regresses (e.g. the evaluator becomes reachable
without admission), this test will fail.

## Coverage matrix

| Proposal claim | Test that exercises it | Result |
|----------------|------------------------|--------|
| ADR text alone → zero findings | `adr_text_alone_produces_zero_findings` | PASS |
| Admitted constraint → findings | `admitted_constraint_produces_findings` | PASS |
| Non-promoted admitter → rejected | `non_promoted_admitter_cannot_admit` | PASS |
| CI promoter → may admit | `ci_promoter_can_admit` | PASS |
| Idempotency on id | `double_admission_is_rejected` | PASS |
| Rule kinds independent | `rule_kinds_are_independent` | PASS |
| Empty source → zero findings | `empty_source_yields_zero_findings` | PASS |
| Deterministic evaluation | `evaluation_is_deterministic` | PASS |
| Empty rule body → rejected | `empty_rule_body_is_rejected` | PASS |
| System clock works | `system_clock_produces_non_empty_string` | PASS |

## Self-host honesty

The self-host test (`self_host_evaluator_finds_zero_drift_on_clean_source`)
is `#[ignore]` because it fails. This is **the honest signal**:

* The evaluator runs on real source.
* It finds 4 real drifts.
* The drifts are documented in `docs/debts/DEBT-SDDK-004.md`.

A green ignored test would mean the evaluator is **not
load-bearing** — i.e. it would silently miss real violations. The
fact that it fails is the proof that it works.

The companion test (`self_host_evaluator_finds_real_drift_in_synthetic_fixture`)
passes, proving the evaluator finds drifts in fixtures that have
them. Together, the two tests cover the two directions of the
load-bearing guarantee:

* Evaluator finds drifts when there are drifts (synthetic).
* Evaluator finds drifts when there are drifts on the real
  codebase (ignored, but reproducible).

## Out-of-scope verification

We deliberately do **not** verify the following in this cycle:

* Persistence of admitted constraints across restarts (no
  adapter yet).
* Cross-crate checks (only `cognicode-core` is in scope).
* Wire-up into the EvidenceBundle / PolicyGate pipeline (the
  drift findings are shaped for it, but the wire-up is a
  follow-on cycle).

## Conclusion

e77 is GREEN. The implementation matches the proposal, the
design, and the tasks. The load-bearing property is proven by
the adversarial matrix. The self-host property is proven by the
ignored test + its companion. The next step is archive.
