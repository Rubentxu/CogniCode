# DEBT-SDDK-006 — `interproc_summary` feature-gating drift in two known-failure entries

> **Working document** — local only, NEVER pushed to remote (ephemeral per AGENTS.md).
> Created 2026-09-17 during e79.1 (trust-baseline-hardening) verification.
> Discovered in e79.1; deliberately NOT repaired there or in e80a.

## Symptom

Two entries in `scripts/known_failures.yaml` are classified as
`environment_dependency`, but they are actually a **feature-gating
inconsistency**, not an environment issue:

```text
application::program_analysis::acceptance_evidence::public_dispatcher_accepts_every_m5_algorithm_id
  reason: cwd / fixture resolution            # actual: missing feature-gated fixture

interface::mcp::mcp_roundtrip_tests::tests::tool_surface_parity::test_allowlist_subset_of_listed
  reason: (allowlist drift)                   # actual: missing feature-gated tool
```

## Root cause

`interproc_summary` is a `program-analysis-server` capability. Its canonical
fixture and its tools/list entry are gated:

* `crates/cognicode-core/src/application/program_analysis/conformance.rs:175`
  — the `interproc_summary` `ConformanceFixture` is
  `#[cfg(feature = "program-analysis-server")]`.
* `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs:1185` — the
  `interproc_summary` tool is `#[cfg(feature = "program-analysis-server")]`.

But the two asserting tests are **ungated**:

* `application/program_analysis.rs` `REQUIRED_PUBLIC_ALGORITHMS` lists
  `interproc_summary` unconditionally.
* `interface/mcp/mcp_roundtrip_tests.rs` `dispatchable_tool_names()` lists
  `interproc_summary` unconditionally.

So on the default build (`cargo test -p cognicode-core --lib`, no feature) the
fixture and the tool are absent while the tests require them.

Note the sibling test `canonical_corpus_has_one_fixture_per_m5_algorithm`
already lists only the five "Core (non-feature-gated) algorithms", so the
codebase is internally inconsistent about this gate.

## Evidence

Verified during e79.1:

* `cargo test -p cognicode-core --lib public_dispatcher_accepts_every_m5_algorithm_id`
  — FAILS without the feature.
* `cargo test -p cognicode-core --lib --features program-analysis-server`
  `public_dispatcher_accepts_every_m5_algorithm_id` — PASSES.
* Same for `test_allowlist_subset_of_listed`.
* Adding `#[cfg(feature = "program-analysis-server")]` to the two gated
  entries makes both pass and yields `cargo test --lib` = 2064 passed / 0
  failed (with the feature) and 2039 passed / 0 failed (without, clean TMPDIR).

Independent of this drift, running the suite inside the agent session also
trips ~39 unrelated `environment_dependency` entries because `TMPDIR` sits
under a symlinked `/home` (`/home -> var/home`) and the file-ops symlink guard
rejects those paths. Those 39 are genuinely environmental; only these 2 are the
feature-gating drift.

## Proposed fix (NOT applied)

Add the matching `#[cfg]` to the two ungated entries, then
`python3 scripts/check_known_failures.py --update` and review the diff:

```rust
// program_analysis.rs, REQUIRED_PUBLIC_ALGORITHMS
#[cfg(feature = "program-analysis-server")]
"interproc_summary",

// mcp_roundtrip_tests.rs, dispatchable_tool_names()
#[cfg(feature = "program-analysis-server")]
"interproc_summary",
```

This makes the tests respect the product's real gating. It does **not**
weaken either test: both still run and pass when the feature is enabled.

Do NOT instead ungate the product fixture/tool without a decision: that is a
product change with wider reach than the test fix.

## Why it was not fixed in e79.1 or e80a

* e79.1 was bounded to DEBT-002/004/005. Fixing this would have required
  regenerating the maintained `known_failures.yaml` baseline, which the cycle
  directive explicitly forbade ("do not just update expected counts").
* e80a is authority-only. The directive said to record this as follow-up debt
  and keep it out of the authority work.

## Revisit trigger

Before the next full-suite housekeeping pass, or whenever
`check_known_failures.py` is regenerated for another reason. It is not on the
critical path for e80a/e80b/e81/e82.

## Closure semantics

Closed when the two `#[cfg]` gates are added, the baseline is regenerated,
and `python3 scripts/check_known_failures.py` reports the reduced set with no
new failures.

## CLOSED — 2026-09-18

Both `#[cfg(feature = "program-analysis-server")]` gates applied:
- `application/program_analysis.rs` `REQUIRED_PUBLIC_ALGORITHMS`
- `interface/mcp/mcp_roundtrip_tests.rs` `dispatchable_tool_names()`

Verified: both tests pass without the feature (fixture/tool correctly
excluded) and with the feature (still exercised). Baseline regenerated:
39 entries, exactness check OK. The two entries removed from
`scripts/known_failures.yaml`.
