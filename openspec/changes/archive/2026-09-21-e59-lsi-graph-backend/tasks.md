# Tasks — cycle e59 — graph detector backend

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Work units

### WU-0 — Bind the promotion to the exact permit
- `PromotionTarget { detector_id, version, semantic_digest, source }`.
- `ExecutionPermit::promotion_target()`.
- `PromotionRequest::for_permit(&permit, approver)` (only public constructor);
  `from_record` for restore.
- `promote` compares the full target.
- `EligibleSourceVerifier` reads `request.target.source`.

### WU-1 — Graph view
- `GraphNode`, `GraphEdge`, `GraphInput`; `AnalysisInput.graph`.

### WU-2 — GraphBackend
- `capabilities = {GraphQuery}`, `evidence_ceiling = B`.

### WU-3 — FLOW semantics
- Bounded BFS, deterministic ordering, `GraphPath` evidence.

### WU-4 — EXCLUDE semantics
- Drop paths containing an excluded subject; `path_excluded` diagnostic.

### WU-5 — Causal chain
- `Source → Flow → Sink`, each step attributed to the path evidence.

### WU-6 — E2E (U41)
- `tests/findings_graph_e2e.rs`: MATCH/FLOW/EXCLUDE/PRODUCE through the
  unchanged seam; planner picks `graph`; class B; promotion ⇒ blocks;
  excluded path ⇒ no finding; multi-capability detector fails loud.

## Acceptance gate
- `cargo test -p cognicode-core --lib domain::findings` (95).
- `cargo test -p cognicode-core --test findings_ast_e2e` (6).
- `cargo test -p cognicode-core --test findings_graph_e2e` (4).
- `cargo check --workspace --all-targets` 0 errors; fmt clean; domain pure.
- gated kernel green; known-failure checker exit 0.
