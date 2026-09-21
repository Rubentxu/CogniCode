# Verification Report — cycle e64 — execution identity + behavior authority (M7.2/M7.3, U52)

> Cycle: A-lite | Milestone: M7 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e64 — execution identity + behavior authority |
| Path | A-lite |
| Base HEAD | `9a70c133` (e63 archive) |
| Commits | `5f906d58` (WU0), `b483b6b5` (WU1+WU2), `d6755c68` (WU3–WU7) |
| Verify verdict | **PASS** |
| U52 | **CLOSED** |

## Verification

### V-1 — Suite counts

| Suite | Result |
|-------|--------|
| `domain::findings` | 112 passed |
| `domain::execution` | 10 passed |
| `domain::behaviors` | 13 passed |
| `domain::naming` / `domain::trust` (moved tests) | 6 passed |
| event log (domain + infra + application) | 32 passed |
| `findings_ast_e2e` / `findings_graph_e2e` | 8 / 4 passed |
| `findings_dataflow_e2e` / `findings_axiom_import_e2e` | 7 / 3 passed |
| `findings_canonical_grounding_e2e` (gated) | 10 passed |
| `intelligence_event_log_e2e` (gated) | 4 passed |
| **`behavior_authority_e2e`** (gated, **acceptance**) | **6 passed** |
| `cargo check --workspace --all-targets` | 0 errors (gated and ungated) |
| `cargo fmt --all --check` | clean |
| `scripts/check_known_failures.py` | exit 0 — 41 entries, unchanged |

`domain::findings` moved 114 → 112 because the three `AnalysisScope` tests moved
with the type to `domain::execution` (which now holds 10). No test was lost.

### V-2 — The U52 exit gate

`tests/behavior_authority_e2e.rs`, over a real kernel fact store:

| Assertion | Evidence |
|-----------|----------|
| the refusal is about the class, not the payload | the attempted fact draft is one a `PureDerivation` may commit, and the positive control accepts and **actually commits** it |
| `effective_class == AgentBehavior` | admission downgraded `declared = PureDerivation` from an `AiGenerated` source |
| `FactStore before == after` | the kernel fact store is byte-identical before and after the run |
| the effect never reached an adapter | the sink recorded nothing, so nothing had to be undone |
| `causal_chain(rejection) == [trigger, behavior.started, policy.behavior_output_rejected]` | kinds compared in order, with every `caused_by` edge checked and the trigger id compared to the recorded root |
| the violation is navigable | both events are attributed to `behavior:derivation.symbol_index`; the payload carries behavior_id, class, effect, reason and execution_id |
| one correlation end to end | `by_correlation` returns exactly trigger, started and rejected |
| the claim is recorded but not acted on | `behavior.started` carries both `class = agent_behavior` and `declared_class = pure_derivation` |
| a downgraded behavior keeps what it may still do | the same `AiGenerated` source accepts `ProposeChange` |
| asking for an analysis grants nothing | `ExecuteAdmittedAnalysis` carries a detector *name*; an `ExecutionPermit` still cannot be minted by a behavior |
| authority cannot be restored | the permit has no public constructor and no serde (compile-time) |

### V-3 — WU2 changed no detector semantics

`ExecutionError::InputScopeMismatch` is new and tested
(`an_input_scope_that_disagrees_with_the_execution_is_refused`), and the four
findings E2E suites, the M6 canonical grounding acceptance test and the U50
event-log acceptance test all still pass unchanged after
`DetectorExecutionRef` was restructured around `ExecutionContext`.

### V-4 — WU0 corrections are regression-tested

| Hole | Test |
|------|------|
| an event claiming another workspace's scope | `an_event_may_not_claim_another_workspaces_scope` (append refused **and** nothing written; the same shape still works in its own workspace) |
| an unpinned scope | `an_unpinned_scope_is_refused` (store) and `an_unpinned_scope_is_rejected` (domain) |
| a lying accessor | `depth()` no longer exists |

V-4's first test found the correction's real consequence: with per-workspace id
numbering, "this cause is another tenant's event" is undetectable — which is why
e63 chose one global sequence, and why the test can now assert the specific
error rather than a generic failure.

### V-5 — Deviations from the review, and why

- **`BehaviorEffect::CommitCanonicalFact` carries a `FactDraft`, not a kernel
  `Fact`.** A behavior does not own canonical truth, and carrying the gated
  kernel type into the behavior domain would have put the feature gate in the
  middle of the authority model. The draft is what the policy needs to decide.
- **`BehaviorDefinition` refuses a self-contradicting definition** (a declared
  class that forbids its own declared effects) instead of silently downgrading
  it. Admission would downgrade it anyway; the refusal is about not surprising
  the author, not about safety.
- **`behavior.completed` is emitted only when nothing was refused.** A run with a
  violation ends at the rejection event, so the chain of a refused run has no
  misleading "completed" at the end.

## Deliberately not executed

- Full-workspace test run beyond the known-failure baseline: the change adds
  three ungated modules, one restructured type and two gated acceptance tests,
  and the baseline covers the rest of the workspace by name.
- Budget and read-set tests: those features do not exist yet.

## Unknown impact

- **`BehaviorRuntime` has exactly one consumer shape**: a deterministic,
  synchronous `Behavior` returning `Vec<BehaviorEffect>`. The authority boundary
  is proved; the production runtime that would match events to behaviors and
  decide when to run them does not exist and is not implied by this cycle.
- The `behavior.*` / `policy.*` event kinds are now partly reachable
  (`behavior.started`, `behavior.completed`, `policy.behavior_output_rejected`)
  but only through this runtime; `behavior.budget_exhausted` remains reserved
  vocabulary for e65.

## Conclusion

An execution now has an identity that says where it sits in the causal history,
and authority that says what it was allowed to do — and the two cannot be
confused, because the FQ52 fixture makes a **human** actor run an
`AgentBehavior` and the refusal comes from the class rather than from the actor.
**PASS — U52 closed; M7.2 and M7.3 complete.**
