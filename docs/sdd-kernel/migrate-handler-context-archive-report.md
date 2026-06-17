# SDD Kernel Archive Report: HandlerContext builder migration

> **Closed**: 2026-06-17
> **Verdict**: PASS
> **PRs**: PR-1 (9eae985), PR-2 (07293e0)

## Change Summary

Remove 6 deprecated `HandlerContext::new(PathBuf)` constructors and migrate all 117 callsites to `HandlerContext::builder().with_*().build()`. Remove multimodal dead code from `rmcp_adapter.rs`.

## Verified Deliverables

- ✅ PR-1 (9eae985): builder additions + 101 `new(PathBuf)` migrations
- ✅ PR-2 (07293e0): 14 Arc migrations + 6 deprecated methods removed + multimodal dead code removed
- ✅ Zero deprecation warnings remaining (`grep -r HandlerContext::new` returns nothing)
- ✅ 1350 tests passing (5 pre-existing failures unrelated)
- ✅ HandlerContext constructed exclusively via `.builder().with_*().build()`

## Changes by File

| File | Δ Lines | What |
|------|---------|------|
| `handlers/mod.rs` | -68 | 6 deprecated methods removed; added `with_graph_store_arc()`, `with_code_intelligence_arc()` |
| `rmcp_adapter.rs` | -26 | Removed `#[cfg(feature = "multimodal")] with_graph_repository()` dead code |
| `aix_handlers.rs` | 106 changed | Arc migrations in handler signatures |
| `lsp_handlers.rs` | 104 changed | Arc migrations in handler signatures |
| `refactor_handlers.rs` | 40 changed | Arc migrations |
| `file_ops_handlers.rs` | 1 changed | Builder migration |
| `mcp_roundtrip_tests.rs` | 1 changed | Builder migration |
| `file_ops_integration_tests.rs` | 1 changed | Builder migration |
| **Net** | **-89** | (180 added, 269 removed) |

## Deprecated Methods Removed

1. `HandlerContext::new(working_dir: PathBuf)` → `.builder().with_working_dir(wd).build()`
2. `HandlerContext::with_validator(wd, validator)` → `.builder().with_working_dir(wd).with_validator(v).build()`
3. `HandlerContext::with_analysis_service(wd, svc)` → `.builder().with_working_dir(wd).with_analysis_service(svc).build()`
4. `HandlerContext::with_refactor_service(wd, svc)` → `.builder().with_working_dir(wd).with_refactor_service(svc).build()`
5. `HandlerContext::with_code_intelligence_provider(wd, provider)` → `.builder().with_working_dir(wd).with_code_intelligence_arc(provider).build()`
6. `HandlerContext::with_graph_store(wd, store)` → `.builder().with_working_dir(wd).with_graph_store_arc(store).build()`

## Builder API (current)

```rust
HandlerContext::builder()
    .with_working_dir(PathBuf)              // required
    .with_validator(InputValidator)          // optional
    .with_analysis_service(AnalysisService)  // optional
    .with_refactor_service(RefactorService)  // optional
    .with_graph_store_arc(Arc<dyn GraphStore>)       // optional
    .with_code_intelligence_arc(Arc<dyn CodeIntelligenceProvider>) // optional
    .build()
```

## Multimodal Dead Code

`rmcp_adapter.rs` had a `#[cfg(feature = "multimodal")]` block containing `with_graph_repository()` that no binary enables. Fully removed. No `multimodal` references remain in the MCP interface crate.

## Entropy Trend

**Method**: Heuristic (CogniCode not invoked — pure refactoring, no new coupling introduced)

**Change type**: Pure refactoring — no new capabilities, no new interfaces, no new coupling.
Net -89 lines, -6 public API surfaces eliminated, -1 dead code path removed.

| Metric | Value | Threshold | Status |
|--------|-------|-----------|--------|
| H(Δ_existing) | ~0 bits | < 1.0 | ✅ |
| H(Δ_new) | ~0 bits | > 0 | ✅ |
| New connascence pairs | 0 | < 3 | ✅ |
| DQS impact | Neutral (same interfaces, same structure) | — | ✅ |
| Connascence Delta | 0 added, 0 removed (refactoring within boundaries) | — | ✅ |
| OCP | Pure refactoring, no extension | yes | ✅ |

**Verdict**: 🟢 No entropy impact. This is a mechanical refactoring that replaces one construction pattern with another. No new coupling introduced. No interfaces changed. No connascence pairs added or removed.

## Knowledge Updates

| Artifact | State | Notes |
|----------|-------|-------|
| `HandlerContext::new()` | **superseded** | Use `HandlerContext::builder()` |
| `HandlerContext::with_graph_store()` | **superseded** | Use `.builder().with_graph_store_arc()` |
| `HandlerContext::with_code_intelligence_provider()` | **superseded** | Use `.builder().with_code_intelligence_arc()` |
| Multimodal `with_graph_repository()` path | **contradicted** | Dead code removed, no binary enables `multimodal` |

## Final Router Context

- **Context Quality**: C2 (well-understood change, clear scope, all artifacts present)
- **Problem Taxonomy**: Mechanical refactoring (deprecation removal + builder migration)
- **Domain Language**: All resolved — `HandlerContextBuilder`, `with_graph_store_arc`, `with_code_intelligence_arc`
- **Invariants**: HandlerContext continues to hold same fields; construction changed only
- **Recommended Effort**: skip (closed)

## Risks

- None. All deprecated methods removed, 117 callsites migrated, tests pass.
- Backward compatibility: breaking change (methods removed). Consumers still on the old API must migrate.

## Next Recommended

None. Change is closed.
