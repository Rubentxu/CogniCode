# ADR-RX-005-behavior-budgets — Behavior Budgets: Bounded Execution for M7.4

**Status:** PROPOSED

## Decision

Add budget enforcement to the M7.4 behavior runtime. A behavior's permit carries a `BudgetDeclaration` (per-kind ceilings). On `BehaviorRuntime::run`, a `BudgetAuthorizer` consults the budget state against the proposed effect **before** the effect reaches the sink. If a ceiling would be exceeded, the authorizer refuses the effect, the runtime emits `execution.budget_exhausted` (and `policy.behavior_output_rejected`), and the behavior continues without that effect. Time budgets use a `Clock` port (application boundary, no `tokio` in domain).

## Rationale

M7 milestone exit gates require "behavior budgets enforced" and "bounded drains." The budget vocabulary (`BudgetKind`, `BudgetDeclaration`, `BudgetState`, `BudgetAuthorizer`) is pure domain. The runtime seam (`BehaviorRuntime::run`) extends the e64 authority seam with budget enforcement as a second gate. The event vocabulary follows the e63 pattern (namespaced kind, structured payload).

## Constraint

- Budget does NOT derive authority: `BehaviorAuthorityPolicy` table is unchanged; budget operates after authority.
- A declaration never escalates: `BudgetDeclaration` has no `BehaviorClass` field; an `AiGenerated` behavior declaring `PureDerivation` runs as `AgentBehavior`.
- Refuse BEFORE the adapter: `BudgetAuthorizer::check` is read-only; only `commit` mutates state, and it is called only after the sink confirms success.
- No `block_on` in domain: `Clock` port is in `application::behaviors::clock`.
- Store-allocated ids, public fields make checks decorative: `BudgetState` has private fields; only `BudgetAuthorizer` can mutate it.

## Validation before ACCEPTED

1. All 6 UAT-U52 scenarios pass (`behavior_budget_e2e.rs`).
2. All 6 e64 regression scenarios pass (`behavior_authority_e2e.rs`).
3. All 4 e63 regression scenarios pass (`intelligence_event_log_e2e.rs`).
4. All 10 e62.4 regression scenarios pass (`findings_canonical_grounding_e2e.rs`).
5. `BehaviorAuthorityPolicy` table is byte-identical to e64.
6. `FactStore.count_facts()` is unchanged after exhaustion (snapshot integrity).
