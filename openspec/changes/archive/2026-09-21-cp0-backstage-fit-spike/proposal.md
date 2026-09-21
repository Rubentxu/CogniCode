# CP0 — Backstage Fit Spike

## Intent

Decide whether **Backstage** is an adequate **optional host shell** for a future
CogniCode outer-loop Control Plane, while CogniCode remains headless and
authoritative and the Explorer remains the specialized inner-loop product.

This is a **spike**. A negative outcome (`NOT_FIT`) is a legitimate result.

## Non-goals (explicitly out of scope)

- No `ControlQueryService`, `AttentionItem`, `CaseView`, Investigation or approval UI.
- No fixing of the `check_architecture` MCP tool.
- No e78 (packs) work.
- No CP1.
- No domain logic in TypeScript; no Rust-in-Node.
- No canonical CogniCode state in the Backstage backend plugin.

## Constraints carried from the LSI mandate

- **Authority invariants hold unchanged.** Trial/PolicyGate/Evaluation are
  technical; external human approval is authority; `PromotionPermit` is
  capability. Nothing in this spike may weaken that.
- Backstage is a **shell**. It may display and navigate. It may not authorize.
- The result must be reachable as a **bounded, disposable subtree**.

## Deliverable

Exactly one outcome — `BACKSTAGE_FIT`, `FIT_WITH_CONSTRAINTS`, or `NOT_FIT` —
supported by the evidence in `verification-report.md`.
