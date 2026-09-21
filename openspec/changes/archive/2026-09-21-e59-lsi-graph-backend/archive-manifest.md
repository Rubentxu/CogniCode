# Archive Manifest — e59 — graph detector backend

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e59-lsi-graph-backend` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** — second paradigm through the same seam |
| Path | A-lite |
| Final status | **ARCHIVED** |
| Base SHA | `433e3397` (post-e58.2) |
| Verify verdict | **PASS** |

## What was delivered

**WU-0** — promotion bound to the exact permit: `PromotionTarget {detector_id,
version, semantic_digest, source}` derived from the permit;
`PromotionRequest::for_permit` is the only public constructor; `promote`
compares the full target. A verified approval cannot be re-aimed at another
source, version, logic or policy; a display rename does not invalidate it.

**WU-1..WU-6** — `GraphBackend` (`{GraphQuery}`, ceiling `B`):
- `GraphInput` added to `AnalysisInput`.
- `FLOW` ⇒ deterministic bounded BFS; `EXCLUDE` drops sanitized paths.
- `GraphPath` evidence (class B) + `Source → Flow → Sink` causal chain.
- The backend builds no `Finding`; no branch was added to the executor,
  assembler or verifier.

## Evidence

- `domain::findings`: **95 tests**; AST E2E **6**; graph E2E **4**.
- `cargo check --workspace --all-targets` 0 errors; fmt clean; domain pure;
  gated kernel 82 green; known-failure checker exit 0.

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| 7.1 Finding/Risk/EvidenceClass lifecycle | ✅ |
| 7.2 Detector IR schema/parser/validator | ✅ |
| 7.3 AST detector backend | ✅ |
| **7.4 graph-pattern backend** | ✅ **done (this cycle)** |
| 7.5 dataflow backend | next (adapter over `ProgramAnalysisService`, no second taint engine) |
| 7.6 QualityIssue compatibility projection | ✅ |
| 7.7 Axiom rule import tooling | pending |
| U40-U48 | U40/U41/U43/U47 pass; U42 open (kernel adapter) |

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification commands

```
cargo test -p cognicode-core --lib domain::findings
cargo test -p cognicode-core --test findings_ast_e2e
cargo test -p cognicode-core --test findings_graph_e2e
```

Return `95`, `6` and `4` passed. ✅
