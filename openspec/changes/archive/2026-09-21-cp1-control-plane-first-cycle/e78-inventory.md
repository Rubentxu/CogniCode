# e78 Consumer Inventory — CP1.0 checkpoint re-applied (2026-09-19)

> Per user directive 2026-09-19 (session clover): the e78 gate threshold (≥2
> genuine product consumers) is unchanged. This inventory re-applies the gate
> at HEAD `949cae62`. It does **not** reinterpret the threshold and does
> **not** alter the historic archive closure (e78 = DEFERRED).
>
> **Updated accounting 2026-09-19T09:30Z**: the distinction between
> *implemented candidate* and *accredited operational consumer* is now
> explicit. CP1.0 WU4 provides one **implemented candidate** (the endpoint
> and its tests), but **zero accredited operational consumers** because
> no production runtime wires `ControlQueryService` (see closeout §13 and
> commit `674c3795`). e78 stays DEFERRED.

## Scope

Module under audit: `crates/cognicode-core/src/application/architecture/*`
(the e77 executable architecture surface).

A "genuine product consumer" is a non-test, non-proxy, non-legacy production
code path that drives the e77 module on behalf of a user-facing workflow.

## Inventory

| # | site | path / commit | uses ControlQueryService? | uses application::architecture (other)? | counts? |
|---|---|---|---|---|---|
| 1 | `GET /control-plane/workspaces/:id/architecture` | `crates/cognicode-explorer/src/api.rs:635` (commit `fed57b95`, WU4) | YES (`ApiState.control_query` injection + handler at line 1212) | YES | **YES — consumer #1** |
| 2 | `GET /api/workspaces/:id/architecture` | `crates/cognicode-explorer/src/api.rs:645` (legacy, pre-CP1) | NO | NO (uses `state.graph.build_architecture`) | NO (legacy explorer path, not Control Plane) |
| 3 | `GET /api/workspaces/:id/architecture/mermaid` | `crates/cognicode-explorer/src/api.rs:691` (E20 mermaid C4 export) | NO | NO (uses graph state) | NO (E20 export, not Control Plane) |
| 4 | `crates/cognicode-core/src/application/ai/semantic_miner.rs` | E40 era | NO | imports `domain::architecture::ArchitectureConstraintId` only (domain layer) | NO (domain layer, not the e77 ejecutor) |
| 5 | `crates/cognicode-core/src/domain/ai/{frame,hypothesis}.rs` | LSI | NO | imports `domain::architecture::ArchitectureConstraintId` (domain layer) | NO |
| 6 | `crates/cognicode-core/src/application/architecture/{admission,evaluator,grounding,registry}.rs` | e77 internal | n/a | internal modules of the same crate | NO (submodule of the module under audit; not a consumer) |
| 7 | `crates/cognicode-core/src/application/ai/boundary_tests.rs` | tests | NO | tests only | NO (tests) |

## Excluded categories (per directive)

- tests / property tests / unit tests
- documentation (`docs/`, READMEs, ADRs)
- proxies / re-exports / builder methods (e.g. `with_control_query`)
- the same operational need expressed through different routes (e.g. the
  control-plane endpoint + a hypothetical second client of the same service
  remain ONE consumer; the actual second consumer must express a different
  product question)

## Result

```text
genuine product consumers of application::architecture  = 1
threshold (carried unchanged)                           = 2
gate status                                              = 1 / 2 — UNMET
e78 outcome                                              = DEFERRED
historic archive                                         = NOT MODIFIED
```

## Notes

- `/api/workspaces/:id/architecture` legacy was NOT counted: it uses
  `state.graph.build_architecture`, not `ControlQueryService`. It does not
  drive the e77 executable architecture module; it drives the explorer's
  pre-CP1 internal graph build.
- A potential future second consumer must express a **distinct product
  question** than "what is the architecture state of W at S?". Possibilities
  include (but are not limited to): architecture diff over time, attention
  list filtered by architecture status, investigation seeded from a violation
  reference. None of these are pre-allocated; each must originate from a real
  need surfaced by the vertical in CP1.0.
