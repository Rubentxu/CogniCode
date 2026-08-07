# Tasks: ask-router

> new `src/ask/` (5 files) + `mcp.rs` extension · Strict TDD · 3 chained PRs · ~485 net

## Review Workload Forecast

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: feature-branch-chain
400-line budget risk: High

| Unit | Goal | PR | Base | ~Lines |
|------|------|----|------|--------|
| 1 | Types + Patterns + classify | PR 1 | `feature/ask-router` | +160 |
| 2 | entity + followups + dispatch | PR 2 | PR 1 branch | +240 |
| 3 | mcp.rs wiring + integration | PR 3 | PR 2 branch | +85 |

## Phase 1 — Skeleton + Types (RED → GREEN)

- [ ] 1.1 RED 6 tests in `ask::tests`: exports, 8-variant `QuestionCategory`, `PATTERNS.len()==8`, `QuestionPattern` fields, `ClassifiedQuestion` constructs. Validate: compile error.
- [ ] 1.2 RED 4 tests: `AskRouter::classify` returns priorities 1/2/3 + fallback. Validate: compile error.
- [ ] 1.3 GREEN: create `src/ask/mod.rs` + `patterns.rs` (8-variant enum, `QuestionPattern`, `PATTERNS` const, `AskRouter::classify` lowercased + score 1.0/0.7/0.5). Add `pub mod ask;` to `lib.rs`. 10 pass.
- [ ] 1.4 Commit `feat(ask): module skeleton + classify`.

## Phase 2 — Pattern Calibration (RED → GREEN)

- [ ] 2.1 RED 11 tests: one per canonical pattern input + overlap (path-between wins) + full=1.0 + partial=0.7.
- [ ] 2.2 GREEN: refine `classify()` scoring + `PATTERNS` regex from spec §Pattern Specs. 11 pass.
- [ ] 2.3 Commit `feat(ask): calibrate 8 patterns + scoring`.

## Phase 3 — Entity Extraction (RED → GREEN)

- [ ] 3.1 RED 5 tests in `ask::entity::tests`: backtick parsing 0/1/many + ambiguous spotter→top-3 follow-up + zero-match→`no_entity_match`.
- [ ] 3.2 GREEN: create `src/ask/entity.rs` with `ExtractedEntity` + `pub async fn extract_entities(question, &Arc<ExplorerService>) -> (Vec<ExtractedEntity>, Vec<FollowUp>)` (regex `` `([^`]+)` `` + `spotter_search` with 0.6 threshold). 5 pass.
- [ ] 3.3 Commit `feat(ask): extract_entities with disambiguation`.

## Phase 4 — Follow-Ups (RED → GREEN)

- [ ] 4.1 RED 6 tests: per-category required follow-ups (path→dependency, forward→inverse, backward→inverse, quality→inspect, fallback→`no_pattern_match`) + determinism.
- [ ] 4.2 GREEN: create `src/ask/followups.rs` with `pub fn generate_follow_ups(category, &[String], &Value) -> Vec<FollowUp>` (static table; extend `FollowUp` in `mcp.rs` with `kind: Option<String>`). 6 pass.
- [ ] 4.3 Commit `feat(ask): deterministic follow-up generation`.

## Phase 5 — Dispatcher (RED → GREEN)

- [ ] 5.1 RED 10 tests in `ask::dispatch::tests`: one per category + 2 failures (graph_unavailable envelope for graph-dep; non-graph question works without graph).
- [ ] 5.2 GREEN: create `src/ask/dispatch.rs` with `pub async fn dispatch_ask(classified, &Arc<ExplorerService>, &Option<Arc<CallGraph>>) -> McpResultEnvelope<serde_json::Value>`. Pre-dispatch graph check returns `graph_unavailable` envelope listing patterns 4, 8. Match on category → call service directly → build `{primary_result, supporting}` + `ProvenanceMetadata::new(confidence, Some("ask-router"))`. 10 pass.
- [ ] 5.3 Commit `feat(ask): dispatch_ask with graph gating`.

## Phase 6 — MCP Wiring (RED → GREEN, atomic)

- [ ] 6.1 RED 6 tests: tool count 17→18, `TOOL_ASK` in `TOOL_NAMES`, schema has `question` (required) + `context` (optional), missing `question`→validation error, full dispatch envelope has `provenance.source = "ask-router"`.
- [ ] 6.2 GREEN same commit: in `mcp.rs` add `pub const TOOL_ASK`, append to `TOOL_NAMES`, add `AskArgs`, add `TOOL_ASK` arm in `dispatch()` (`AskRouter::classify` → `ask::dispatch::dispatch_ask` → `envelope_ok(TOOL_ASK, &Ok(env), None)`), add schema in `build_tool_schemas()`. Add `regex.workspace = true` to `Cargo.toml`. Update count assertion 17→18. 6 pass; 32 new + 71 prior = 103 total.
- [ ] 6.3 Commit `feat(mcp): register cognicode_ask + wire AskRouter`.

## Phase 7 — Verification Gate

- [ ] 7.1 `cargo build --workspace --all-targets` — no new warnings.
- [ ] 7.2 `cargo test --workspace` — 0 regressions.
- [ ] 7.3 `cargo fmt --check && cargo clippy -p cognicode-explorer --all-targets -- -D warnings`.
- [ ] 7.4 Count ask-router tests → expect ≥ 42.
- [ ] 7.5 Trace 36 scenarios in `specs/ask-router/spec.md` (sdd-verify).
- [ ] 7.6 Rustdoc one-liner per new `pub` type in `ask/`.

## Atomic-Commit Constraints

- **6.1 + 6.2** = single commit (constant + arm + schema + count assert ship together).
- **2.1 + 2.2** = single commit (regex calibration only valid with all 8 patterns).
- Splitting any phase breaks compile or the chained-PR base.

## Dependencies

1.1 → 1.2 → 1.3 → 1.4 → 2.1 → 2.2 → 2.3 → 3.1 → 3.2 → 3.3 → 4.1 → 4.2 → 4.3 → 5.1 → 5.2 → 5.3 → 6.1+6.2 → 6.3 → 7.x

## Estimates

| Phase | Tasks | + | − | Net |
|-------|-------|---|---|
| 1 | 4 | 160 | 0 | +160 |
| 2 | 3 | 30 | 5 | +25 |
| 3 | 3 | 70 | 0 | +70 |
| 4 | 3 | 50 | 0 | +50 |
| 5 | 3 | 110 | 0 | +110 |
| 6 | 3 | 90 | 25 | +65 |
| 7 | 6 | 5 | 0 | +5 |
| **Total** | **25** | **515** | **30** | **+485** |
