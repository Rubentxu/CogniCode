# Proposal: CON Dimension Investigation & Fix

## Intent

CON (Consistency) is 78 ⚠️ and dragging the Health Score. Root cause is **confirmed**: the sandbox orchestrator always passes `&[]` (empty) for `latency_samples_ms`, so `compute_consistency_score` never reaches the multi-run CV path — it always falls back to the single-sample latency/size heuristic (`0.1ms per KB`). Scenarios with large workspaces (semantic_search, find_usages, complexity, size-variant indexing) get penalized for being "slower than expected", producing CON=40–60, not because the tool is actually inconsistent.

## Scope

### In Scope
- Fix the latency/size heuristic thresholds so they reflect real tool behavior
- Add per-scenario `con_runs` support to manifests for true multi-run CV scoring
- Recalibrate `compute_consistency_score` single-run path for sub-100ms tools
- Add ground-truth `consistency_tolerance` per scenario class (read_only, stateful)
- Re-run baselines and verify CON ≥ 88 across semantic_search, find_usages, complexity

### Out of Scope
- Changing how CON is weighted in the Health Score formula
- Full multi-run parallel execution infrastructure
- CON for robustness/error manifests (ROB domain)

## Capabilities

### New Capabilities
- None

### Modified Capabilities
- `consistency-scoring`: Fix single-run heuristic thresholds and add per-scenario calibration config

## Approach

**Two-track fix:**

1. **Threshold recalibration** (scoring.rs): The `0.1ms/KB` expected latency is too aggressive for in-memory tree-sitter/index tools. Adjust to realistic per-tool-class baselines (e.g. 2ms/KB for semantic search, 0.5ms/KB for lightweight index). This is the fast path — no manifest changes needed.

2. **Manifest-level override** (optional): Add a `consistency_baseline_ms` field to scenario metrics. When present, use it instead of the size heuristic. Allows per-scenario calibration without changing global constants.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/sandbox_core/scoring.rs` | Modified | Recalibrate heuristic thresholds; add baseline_ms param |
| `src/bin/sandbox_orchestrator.rs` | Modified | Pass per-scenario `consistency_baseline_ms` if present |
| `sandbox/manifests/code_intelligence/rust_semantic.yaml` | Modified | Add calibrated latency targets |
| `sandbox/manifests/code_intelligence/rust_find.yaml` | Modified | Add calibrated latency targets |
| `sandbox/manifests/code_intelligence/rust_complexity.yaml` | Modified | Add calibrated latency targets |
| `sandbox/manifests/scale/rust.yaml` | Modified | Verify size-variant CON after fix |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Threshold change breaks LAT scores | Low | LAT uses separate `compute_latency_score`; no shared state |
| Over-tuning heuristic to pass tests | Med | Validate thresholds against real measurements before hardcoding |
| CON ≥ 88 target infeasible for some tools | Low | Tools are deterministic; variance is measurement noise |

## Rollback Plan

Revert `scoring.rs` heuristic thresholds to previous values. One-file change, no DB migration. Previous Health Score: 90.7 (CON=78).

## Dependencies

- None — all changes are in scoring.rs and manifests

## Success Criteria

- [ ] CON global average ≥ 88 (up from 78)
- [ ] `rust_semantic_search_*` scenarios: CON ≥ 80
- [ ] `rust_find_usages_*` scenarios: CON ≥ 80
- [ ] `rust_complexity_*` scenarios: CON ≥ 80
- [ ] Health Score ≥ 92 (up from 90.7)
- [ ] All 226+ scenarios still pass (no regressions)
