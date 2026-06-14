# Working Summary — Architectural Deepening Candidates

## Session State
- **Pass:** 1 (complete)
- **Cycles completed:** 8
- **Questions answered:** 8

## Decisions Recorded

### Q001-P1: C1→C4 dependency — MODIFIED
C1 gated by C4 (3 preconditions OR C4 first). Boundary violations found: schemas.rs imports dto, handler DTO leakage, BuildGraphInput anomaly.

### Q002-P1: HandlerContext Builder — MODIFIED  
Split: Builder PR first (additive), then ContextGraphStore deletion + Box→Arc migration PR.

### Q003-P1: Rust macro technology — MODIFIED
Attribute proc macro (`#[aix_tool]`) extending cognicode-macros. build.rs codegen evaluated and rejected.

### Q004-P1: ReadMode dispatch — Proxy WINS
Enum with static dispatch. ReadMode already exists. Closed set of 4 variants. Trait objects rejected.

### Q005-P1: SKIP_DIRS placement — Proxy WINS, refined
`domain/value_objects/walk_filter.rs` with composed builder: `.with_security_blocklist()` + `.with_performance_skips()`. 9 duplicated blocks consolidated.

### Q006-P1: Mock visibility — Skeptic WINS
Separate `cognicode-core-mock` crate for cross-crate mocks. `#[cfg(test)]` for internal unit tests. No feature flag in production crate.

### Q007-P1: Dependency graph — MODIFIED
File-touch matrix augmented with runtime call-graph + trait coherence + feature-gate analysis. 5-wave execution order: C3+C6+C5 → C4 → C2 → C1.

### Q008-P1: Type boundary — MODIFIED
All 24 pairs use newtypes via declarative macro. No type aliases — 87.5% differ semantically. Derives chosen per-type, not standardized.

## Coverage Matrix

| Dimension | Status |
|-----------|--------|
| Goal clarity | ✅ |
| Code evidence | ✅ |
| Domain vocabulary | ✅ (WalkFilter, ReadMode, Schema/DTO boundary) |
| Implementation boundaries | ✅ |
| Migration path | ✅ (5-wave execution order) |
| Testing impact | ✅ (mock crate strategy, ReadMode testing) |
| Rollout strategy | ✅ (incremental, independent PRs) |
| ADR candidates | ✅ (3 candidates identified) |
| Risk assessment | ✅ (merge conflict analysis, blast radius) |
| Observability impact | ✅ (CI boundary checks) |
| Non-goals | ✅ (no domain layer changes, no public API breaks) |
| Backward compatibility | ✅ (serde roundtrip maintained, schema names frozen) |

## ADR Draft Candidates
| Slug | Status |
|------|--------|
| DRAFT-schema-dto-boundary | Identified (Q001-P1) |
| DRAFT-candidate-execution-order | Identified (Q007-P1) |
| DRAFT-newtype-declarative-macro | Identified (Q008-P1) |

## Current Candidate Status
| # | Candidate | Decision | Execution Order |
|---|-----------|----------|-----------------|
| 1 | Tool Registry | Attribute proc macro + register_tool! | Wave 5 (last) |
| 2 | HandlerContext Builder | Split: Builder PR + ContextGraphStore deletion | Waves 4 + cleanup |
| 3 | SKIP_DIRS Consolidation | WalkFilter with composed inputs | Wave 1 (parallel) |
| 4 | Schema/DTO Unification | Newtypes via declarative macro | Wave 3 (gates C1) |
| 5 | file_operations ReadMode | Enum dispatch | Wave 1 (parallel) |
| 6 | Mock relocation | Separate mock crate | Wave 1 (parallel) |
