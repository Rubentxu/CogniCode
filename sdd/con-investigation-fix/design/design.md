# Design: Fix CON Dimension Scoring

## Technical Approach

Fix the single-run consistency heuristic that inflates CON scores to 40-60 for all tools by:
1. Adding a manifest-settable `consistency_baseline_ms` override in `ConsistencyMetrics`
2. Calibrating per-tool-class baseline expectations (in-memory vs indexing vs graph tools)
3. Keeping the multi-run CV path unchanged

## Architecture Decisions

### Decision: Tool-Class Baseline Calibration

**Choice**: Classify tools into three consistency tiers with calibrated baselines instead of uniform 0.1ms/KB

| Tool Class | Examples | Baseline (ms) | Rationale |
|------------|----------|---------------|-----------|
| `in_memory` | symbols, outline, search | 1-5 | Sub-ms operations on cached data |
| `indexing` | build_lightweight_index | 15-30 | Must scan and index |
| `graph` | build_graph, trace_path, analyze_impact | 10-50 | Graph traversal complexity |

**Alternatives rejected**: Universal multiplier adjustment (too blunt), removing single-run scoring entirely (breaks single-run evaluations)

### Decision: Manifest Override Path

**Choice**: Add `consistency_baseline_ms: u64` to `ConsistencyMetrics` struct, parsed from manifest YAML

**Rationale**: Allows per-scenario calibration without code changes; aligns with existing manifest-based metrics pattern (latency target/max_ms already use this path)

## Data Flow

```
manifest.metrics.consistency_baseline_ms
         OR
tool_class_baseline(tool_name)
         OR
workspace_size_kb * 0.1  (current fallback)
         │
         ▼
compute_consistency_score()
         │
         ▼
CON score (0-100)
```

Multi-run path unchanged: `latency_samples_ms.len() >= 2` → `compute_consistency_from_cv()` (line 1133)

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/sandbox_core/scoring.rs` | Modify | Add `consistency_baseline_ms` to `ConsistencyMetrics` (line ~80); add tool-class calibration fn; update `compute_consistency_score` (lines 1136-1153) |
| `src/sandbox_core/manifest.rs` | Modify | Add `consistency_baseline_ms` to `ConsistencyMetrics` serde deserialization (YAML) |

## Interfaces / Contracts

### New field in `ConsistencyMetrics` (scoring.rs:77-82)
```rust
pub struct ConsistencyMetrics {
    /// Expected variance threshold
    pub variance_threshold: Option<f64>,
    /// Baseline expected latency (ms) for single-run consistency scoring.
    /// If set, overrides tool-class heuristic. For in-memory tools targeting ≥88 CON.
    pub consistency_baseline_ms: Option<u64>,
}
```

### Tool-class calibration function
```rust
/// Returns expected baseline latency (ms) for single-run consistency scoring.
/// Falls back to 0.1ms/KB heuristic if no class matches.
fn tool_class_baseline(tool_name: &str) -> Option<u64> {
    match classify_tool_for_consistency(tool_name) {
        ToolConsistencyClass::InMemory => Some(3),   // ~3ms for cached lookups
        ToolConsistencyClass::Indexing => Some(20),   // 15-25ms for indexing
        ToolConsistencyClass::Graph => Some(30),      // 20-50ms for graph ops
        ToolConsistencyClass::Unknown => None,         // Use 0.1ms/KB fallback
    }
}
```

### Modified single-run branch in `compute_consistency_score` (lines 1136-1153)
```rust
// Priority: metrics override > tool-class baseline > 0.1ms/KB
let baseline_ms = if let Some(baseline) = metrics_override {
    baseline as f64
} else if let Some(baseline) = tool_class_baseline(tool_name) {
    baseline as f64
} else {
    workspace_size_kb as f64 * 0.1  // current heuristic
};

let ratio = latency_ms as f64 / baseline_ms;
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Tool classification | `classify_tool_for_consistency` returns correct class for known tools |
| Unit | Baseline override | `consistency_baseline_ms` from manifest takes precedence |
| Unit | Score boundaries | ratio ≤1 → 90, ≤2 → 80, ≤5 → 60, >5 → 40 |
| Integration | In-memory tool CON ≥88 | manifest with `consistency_baseline_ms: 3` + latency 2ms → ratio 0.67 → 90 |
| Integration | Multi-run unchanged | latencies [5,5,5] → CV=0 → 95 (existing behavior preserved) |

## Migration / Rollout

No migration required. New field is `Option<u64>` with `None` fallback to existing 0.1ms/KB heuristic.

## Open Questions

- [ ] What tool_class should `get_file_symbols` map to? (likely `InMemory` but verify with benchmarks)
- [ ] Should `consistency_baseline_ms` be settable per-variant in the manifest, or only per-scenario?
