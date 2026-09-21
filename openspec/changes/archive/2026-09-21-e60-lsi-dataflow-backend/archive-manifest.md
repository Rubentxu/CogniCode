# Archive Manifest — e60 — dataflow backend (M5 adapter)

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e60-lsi-dataflow-backend` |
| Path | A-lite |
| Final status | **ARCHIVED** |
| Base SHA | `b465b2e0` (post-e59.1) |
| Verify verdict | **PASS** |

## What was delivered

- Typed M5 facade `TaintFlowRequest`/`TaintFlowResult`; `dispatch(TAINT_FLOW)`
  delegates to it (one implementation, two presentations).
- Pure `DataflowInput` domain DTO (no M5 types) + `AnalysisInput.dataflow`.
- `TaintFlowRunner` (application-local seam) implemented by
  `ProgramAnalysisService`.
- `M5DataflowBackend`: `{Dataflow}`, ceiling `B`, IR→engine site mapping,
  `DataflowPath` evidence, `Source → Flow* → Sink` causal chain.
- `BackendError::Analysis`: engine errors never become "nothing found".
- IR reachability rule V10 (`FLOW`/`EXCLUDE` need `GraphQuery` **or** `Dataflow`).
- E2E with AST + Graph + Dataflow registered.

## Evidence

- `domain::findings` 97; `application::findings` 8; AST E2E 6; Graph E2E 4;
  Dataflow E2E 5.
- Counting-spy test proves M5 is really invoked with the expected sites.
- `cargo check --workspace --all-targets` 0 errors; fmt clean;
  known-failure baseline unchanged (41).

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| 7.1 Finding/Risk/EvidenceClass lifecycle | ✅ |
| 7.2 Detector IR schema/parser/validator | ✅ |
| 7.3 AST detector backend | ✅ |
| 7.4 graph-pattern backend | ✅ |
| **7.5 dataflow backend** | ✅ **done (this cycle)** |
| 7.6 QualityIssue compatibility projection | ✅ |
| 7.7 Axiom rule import tooling | pending |
| U40-U48 | U40/U41/U43/U47 pass; U42 open (kernel adapter) |

**Three paradigms, one core**: AST, Graph and Dataflow all traverse
`admission → planner → backend → outcome → evidence → assembler → verifier →
gate` with no special case.

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification commands

```
cargo test -p cognicode-core --lib domain::findings
cargo test -p cognicode-core --lib application::findings
cargo test -p cognicode-core --test findings_dataflow_e2e
```
