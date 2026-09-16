# behavior-budgets Specification

## Purpose

Budget enforcement for the M7.4 Living Software Intelligence behavior runtime. Behaviors declare ceiling budgets (time, effect-count, fact-visits) at admission; the runtime enforces them before effects reach the sink. Exhaustion emits an observable `execution.budget_exhausted` event; the behavior continues with subsequent effects refused independently.

## Requirements

### Requirement: EffectCount budget limits effects per execution

A `PureDerivation` admitted with `EffectCount = N` may have at most N effects accepted; the (N+1)th effect is refused with `budget_exhausted` before the sink is reached.

#### Scenario: Three-effect behavior exhausts at the declared ceiling of 2

- GIVEN a `PureDerivation` admitted with `EffectCount = 2`
- AND a behavior that proposes 4 `RecordEvidence` effects
- WHEN the runtime executes the behavior
- THEN exactly 2 effects are accepted
- AND exactly 2 effects are refused with `budget_exhausted` reason
- AND the sink receives exactly 2 effects
- AND `FactStore` is unchanged by the refused effects

#### Scenario: Agent authority denial is checked before budget

- GIVEN an `AgentBehavior` admitted with `EffectCount = 10`
- AND a behavior that proposes a `CommitCanonicalFact` effect
- WHEN the runtime executes the behavior
- THEN the effect is refused by `BehaviorAuthorityPolicy` (not by the budget)
- AND no budget exhaustion event is emitted

### Requirement: Time budget limits wall-clock elapsed time

A behavior admitted with `Time = Tms` may run for at most Tms of wall-clock time; effects proposed after the ceiling is reached are refused with `budget_exhausted`.

#### Scenario: Effects are refused after elapsed time exceeds the ceiling

- GIVEN a `ReactiveAnalysis` admitted with `Time = 100ms`
- AND a clock advanced to t=120 after two effects
- WHEN the runtime proposes a third effect
- THEN the third effect is refused with `budget_exhausted`
- AND the `behavior.budget_exhausted` event carries `kind: "time"`, `remaining: 0`, `attempted: 1`

### Requirement: Exhaustion is observable via the causal event log

When a budget is exhausted, the runtime emits `behavior.budget_exhausted` caused by the `policy.behavior_output_rejected` event.

#### Scenario: Causal chain is ordered correctly

- GIVEN a behavior that exhausts its budget on the second effect
- WHEN the `causal_chain(budget_exhausted_event)` is retrieved
- THEN the chain reads `[trigger, behavior.started, policy.behavior_output_rejected, behavior.budget_exhausted]`

#### Scenario: AiGenerated behavior with unbounded declared budget is silently capped

- GIVEN an `AiGenerated` behavior declaring `PureDerivation` with no budget
- WHEN the behavior is admitted
- THEN the effective class is `AgentBehavior` (downgraded by admission)
- AND no new authority is gained

### Requirement: Snapshot integrity — exhaustion never touches the canonical store

A refused effect must not reach the `FactStore`.

#### Scenario: FactStore is byte-identical after exhaustion

- GIVEN a `FactStore` with N facts in the active snapshot
- AND a behavior admitted with `EffectCount = 1`
- WHEN the behavior proposes 3 effects and all 3 are refused
- THEN the `FactStore` still contains exactly N facts
- AND the event log contains the exhaustion events

## Design Notes

### Budget kinds

| Kind | What it limits | Check reference |
|---|---|---|
| `EffectCount` | Number of effect attempts | Committed spend counter |
| `Time` | Wall-clock elapsed ms | Last time checkpoint |
| `FactVisits` | Fact read operations | Committed spend counter |

### Execution order per effect

1. Authority check (`BehaviorAuthorityPolicy::allows`) — e64 seam, unchanged
2. Budget check (`BudgetAuthorizer::check`) — e65 seam
   - Refusal emits **both** `policy.behavior_output_rejected` and `behavior.budget_exhausted`
3. Sink dispatch — only reached when both pass
4. Budget commit — only on confirmed sink success

### Key invariant: refuse BEFORE the adapter

The budget check runs **before** `BehaviorEffectSink::apply`. There is no branch in which an effect is dispatched to the sink and then undone. The refusal path cannot reach `FactStore.append_batch`.

### Clock port

Time budgets use `application::behaviors::Clock::now_millis()`. The domain `BehaviorBudget` types carry declared ceilings only; no `tokio` or `std::time` leaks into the domain layer.
