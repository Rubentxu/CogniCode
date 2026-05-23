# Performance-Memory Batch State
# 12 Rules for Performance and Memory Issue Detection
# Last Updated: 2026-05-14

## Batch Metadata
- batch: performance-memory
- rule_count: 12
- language: rust
- created: 2026-05-14
- phase: apply_complete

## Rules Status

| Rule ID | Concept | Severity | Status | LOC | Strategy |
|---------|---------|----------|--------|-----|----------|
| PERF_001 | Forgotten allocation | Critical | implemented | 160 | regex |
| PERF_002 | Unnecessary allocation | Critical | implemented | 155 | regex |
| PERF_003 | Clone in hot path | Critical | implemented | 137 | regex |
| PERF_004 | Vec push without reserve | Major | implemented | 130 | regex |
| PERF_005 | String concat loop | Major | implemented | 131 | regex |
| PERF_006 | N+1 query pattern | Major | implemented | 144 | regex |
| PERF_007 | Unnecessary async | Major | implemented | 134 | regex |
| PERF_008 | Sync in async | Critical | implemented | 143 | regex |
| PERF_009 | Large stack alloc | Major | implemented | 126 | regex |
| PERF_010 | Missing drop cleanup | Critical | implemented | 156 | regex |
| PERF_011 | Inefficient iterator | Minor | implemented | 118 | regex |
| PERF_012 | Box<Vec> indirection | Minor | implemented | 151 | regex |

## Phase Status
- [x] designs_complete: All rule designs finalized
- [x] fixtures_ready: All fixture matrices created
- [x] implementation_complete: All 12 rules implemented
- [x] tests_pass: 25 registration tests pass
- [ ] tests_implemented: Detection tests based on fixture-matrix pending
- [ ] integration_tested: Integration tests pending
- [ ] benchmarked: Performance benchmarks pending

## Implementation Notes
- All rules use `declare_rule!` macro (auto-submits via inventory)
- All rules have `agent_semantics` metadata
- Current tests only verify registration, not detection
- Rules use regex-based detection (not tree-sitter as originally planned)
- Registration tests: `test_perf_00X_registered` pass

## Files Created
- performance/mod.rs
- perf_001_forgotten_alloc.rs
- perf_002_unnecessary_alloc.rs
- perf_003_clone_hot_path.rs
- perf_004_vec_push_no_reserve.rs
- perf_005_string_concat_loop.rs
- perf_006_n_plus_one_query.rs
- perf_007_unnecessary_async.rs
- perf_008_sync_in_async.rs
- perf_009_large_stack_alloc.rs
- perf_010_missing_drop.rs
- perf_011_inefficient_iterator.rs
- perf_012_box_vec_indirection.rs

## Implementation Location
`crates/cognicode-axiom/src/rules/rules/rust/performance/`

## Artifacts
- designs: rules/performance-memory/designs/rule-designs.md
- fixtures: rules/performance-memory/fixture-matrix.md
- state: this file
