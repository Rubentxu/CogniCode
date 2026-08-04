# Tasks: fix-planhash-placeholder

> Idioma: español (prosa) · Test names: English · Commit messages: English Conventional Commits

## ⚠️ APPLY PIVOT — 2026-08-04

**Original plan**: migrate 74 call sites from `PlanHash::compute(&0u32)` to `compute_hash()`.

**What shipped**: The apply discovered that ALL placeholder sites are test fixtures OR intermediate throwaway plans. Production code already uses content-derived hashes via `plan_metadata_for(&plan)`. The pivot delivered:
1. `PlanMetadata::with_hash_computed` helper API
2. `populate_limits` helper function
3. Architectural documentation in production lowering code

**Status of original 10 tasks**: Superseded by pivot. Production code is correct; test fixtures remain with placeholders as an architectural constraint.

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~120 (helpers + docs, not 74-site migration) |
| 400-line budget risk | **Low** |
| Chained PRs recommended | **No** |
| Delivery strategy | `single-pr` |

---

## 1. Task Summary

| Task | File | Sites | Plan type | Test budget | Atomic commit |
|------|------|-------|-----------|-------------|---------------|
| T-001 | `cognicode-core/src/domain/plan/mold_plan.rs` | 16 | `MoldPlan` | 1–2 RED tests | yes |
| T-002 | `cognicode-core/src/domain/plan/graph_plan.rs` | 14 | `GraphPlan` | 1–2 RED tests | yes |
| T-003 | `cognicode-core/src/domain/plan/limits.rs` | 4 | `GraphPlan` | 1 RED test | yes |
| T-004 | `cognicode-core/src/domain/plan/executor.rs` | 1 | `GraphPlan` | 0 (trivial) | yes |
| T-005 | `cognicode-core/src/domain/plan/lower.rs` | 7 | `GraphPlan` | 1 RED test | yes |
| T-006 | `cognicode-core/src/infrastructure/graph/snapshot_graph_executor.rs` | 17 | `GraphPlan` | 0 (infra, no domain tests) | yes |
| T-007 | `cognicode-explorer/src/moldql/lower_plan.rs` | 6 | `MoldPlan` | 0 (uses existing helper) | yes |
| T-008 | `cognicode-explorer/src/moldql/lower_pattern_profile.rs` | 2 | `MoldPlan` | 0 (trivial) | yes |
| T-009 | `cognicode-explorer/src/moldql/executor.rs` | 5 | `GraphPlan`/`MoldPlan` | 0 (test constructs inline) | yes |
| T-010 | `cognicode-ladybug/src/lib.rs` | 2 | `GraphPlan` | 0 (trivial) | yes |

**Total**: 74 sites across 10 files, 10 commits.

---

## 2. Per-Task Detail

### T-001 — mold_plan.rs

- **Goal**: Replace 16 `PlanHash::compute(&0u32)` with `MoldPlan::compute_hash()` calls.
- **Files touched**: `crates/cognicode-core/src/domain/plan/mold_plan.rs`
- **Site count**: 16
- **Plan type**: `MoldPlan` (all sites)
- **Strict TDD sequence**:
  1. **RED**: Write `test_mold_plan_distinct_hashes_different_limits` in `crates/cognicode-core/src/domain/plan/mold_plan.rs` — constructs two `MoldPlan::Graph` instances with different `limit` values, asserts `compute_hash()` results differ.
  2. **Apply**: Replace all 16 `PlanHash::compute(&0u32)` with `compute_hash()` (verify `self` or local `plan` variable is in scope at each site; some sites in constructor patterns need local binding `let hash = self.compute_hash();`).
  3. **GREEN**: Test passes.
  4. **Refactor**: None expected.
- **Note**: Some sites at lines 275 and 518 use `&42u32` (error case seeds) — these are NOT placeholders, do NOT migrate.
- **Verification**: `cargo check -p cognicode-core`
- **Acceptance**: `grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/mold_plan.rs` → 0

---

### T-002 — graph_plan.rs

- **Goal**: Replace 14 `PlanHash::compute(&0u32)` with `GraphPlan::compute_hash()` calls.
- **Files touched**: `crates/cognicode-core/src/domain/plan/graph_plan.rs`
- **Site count**: 14
- **Plan type**: `GraphPlan` (all sites)
- **Strict TDD sequence**:
  1. **RED**: Write `test_graph_plan_distinct_hashes_different_quantifiers` in `crates/cognicode-core/src/domain/plan/graph_plan.rs` — constructs two `GraphPlan::Path` with different `max_hops`, asserts hashes differ.
  2. **Apply**: Replace all 14 `PlanHash::compute(&0u32)` with `compute_hash()`. Note: some sites are in `GraphPlan::Path`, others in `GraphPlan::Neighbors` or `GraphPlan::Subgraph` variants — each arm has `plan` in scope.
  3. **GREEN**: Test passes.
  4. **Refactor**: None expected.
- **Verification**: `cargo check -p cognicode-core`
- **Acceptance**: `grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/graph_plan.rs` → 0

---

### T-003 — limits.rs

- **Goal**: Replace 4 `PlanHash::compute(&0u32)` with `GraphPlan::compute_hash()` calls in test fixtures.
- **Files touched**: `crates/cognicode-core/src/domain/plan/limits.rs`
- **Site count**: 4
- **Plan type**: `GraphPlan` (all are inline test plan constructions)
- **Strict TDD sequence**:
  1. **RED**: Write `test_limits_validate_plan_hash_is_content_derived` — constructs two `GraphPlan::Subgraph` with different `max_depth`, asserts `metadata().hash()` differs.
  2. **Apply**: Replace all 4 `PlanHash::compute(&0u32)` in test fixtures with `compute_hash()`.
  3. **GREEN**: Test passes.
  4. **Refactor**: None expected.
- **Verification**: `cargo check -p cognicode-core`
- **Acceptance**: `grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/limits.rs` → 0

---

### T-004 — executor.rs (domain)

- **Goal**: Replace 1 `PlanHash::compute(&0u32)` with `GraphPlan::compute_hash()`.
- **Files touched**: `crates/cognicode-core/src/domain/plan/executor.rs`
- **Site count**: 1 (line ~151)
- **Plan type**: `GraphPlan`
- **Strict TDD sequence**: Trivial (1 site); write brief inline comment confirming hash derivation intent, then migrate.
- **Apply**: Replace `PlanHash::compute(&0u32)` with `plan.compute_hash()` — the `plan` variable is already in scope at the call site.
- **Verification**: `cargo check -p cognicode-core`
- **Acceptance**: `grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/executor.rs` → 0

---

### T-005 — lower.rs

- **Goal**: Replace 7 `PlanHash::compute(&0u32)` with `GraphPlan::compute_hash()` calls.
- **Files touched**: `crates/cognicode-core/src/domain/plan/lower.rs`
- **Site count**: 7
- **Plan type**: `GraphPlan` (all sites produce `GraphPlan` variants)
- **Risk**: Some sites may have the plan being constructed in a scope where a local binding is needed before calling `compute_hash()`. Fallback: add `let plan = GraphPlan::Path { ... }; let hash = plan.compute_hash();` pattern.
- **Strict TDD sequence**:
  1. **RED**: Write `test_lower_graph_plan_hash_reflects_bounds` — lower a query with two different `max_hops` values, compare resulting `GraphPlan` hashes.
  2. **Apply**: Migrate each `PlanHash::compute(&0u32)` site. Where `plan` is not yet bound, create local binding before hash computation.
  3. **GREEN**: Test passes.
  4. **Refactor**: None expected.
- **Verification**: `cargo check -p cognicode-core`
- **Acceptance**: `grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/lower.rs` → 0

---

### T-006 — snapshot_graph_executor.rs (infra)

- **Goal**: Replace 17 `PlanHash::compute(&0u32)` with `GraphPlan::compute_hash()` calls.
- **Files touched**: `crates/cognicode-core/src/infrastructure/graph/snapshot_graph_executor.rs`
- **Site count**: 17
- **Plan type**: `GraphPlan` (all sites build `GraphPlan` variants in infra layer)
- **Risk**: Medium — infra layer constructs plans; some may need `plan.clone()` before calling `compute_hash()` if ownership rules conflict. The `compute_hash()` takes `&self`, so cloning is only needed if the plan is moved after.
- **Strict TDD sequence**: No domain-level TDD (infra layer); rely on S5 (existing test suite green) and grep acceptance.
- **Apply**: Migrate all 17 sites. If ownership conflict arises at any site, use `let hash = plan.compute_hash();` after plan construction.
- **Verification**: `cargo check -p cognicode-core`
- **Acceptance**: `grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/infrastructure/graph/snapshot_graph_executor.rs` → 0

---

### T-007 — lower_plan.rs (explorer)

- **Goal**: Replace 6 `PlanHash::compute(&0u32)` with `MoldPlan::compute_hash()` calls.
- **Files touched**: `crates/cognicode-explorer/src/moldql/lower_plan.rs`
- **Site count**: 6
- **Plan type**: `MoldPlan` (all sites)
- **Note**: `lower_plan.rs:46` already uses `plan.compute_hash()` correctly in `plan_metadata_for()`. The 6 placeholder sites are in the actual lowering methods.
- **Strict TDD sequence**:
  1. **RED**: Write `test_lower_mold_plan_hash_derives_from_ast` — lower the same AST with two different limit values, assert hashes differ.
  2. **Apply**: Migrate all 6 sites to `plan.compute_hash()`. The `self.plan_metadata_for(plan.clone())` helper pattern already exists and already calls `compute_hash()` correctly — ensure no duplicate hashing.
  3. **GREEN**: Test passes.
  4. **Refactor**: None expected.
- **Verification**: `cargo check -p cognicode-explorer`
- **Acceptance**: `grep -n "PlanHash::compute(&0u32)" crates/cognicode-explorer/src/moldql/lower_plan.rs` → 0

---

### T-008 — lower_pattern_profile.rs (explorer)

- **Goal**: Replace 2 `PlanHash::compute(&0u32)` with `MoldPlan::compute_hash()` calls.
- **Files touched**: `crates/cognicode-explorer/src/moldql/lower_pattern_profile.rs`
- **Site count**: 2 (lines ~111, ~146)
- **Plan type**: `MoldPlan`
- **Strict TDD sequence**: Trivial (2 sites); grep acceptance sufficient.
- **Apply**: Replace both `PlanHash::compute(&0u32)` with `profile.compute_hash()` (the `MoldPlan` variant is `PatternProfile`).
- **Verification**: `cargo check -p cognicode-explorer`
- **Acceptance**: `grep -n "PlanHash::compute(&0u32)" crates/cognicode-explorer/src/moldql/lower_pattern_profile.rs` → 0

---

### T-009 — executor.rs (explorer)

- **Goal**: Replace 5 `PlanHash::compute(&())` with appropriate `compute_hash()` calls.
- **Files touched**: `crates/cognicode-explorer/src/moldql/executor.rs`
- **Site count**: 5 (lines ~1499, ~1543, ~1581, ~1622, ~1665)
- **Plan type**: Mixed — test code constructs plans inline; determine variant per call site:
  - Lines 1499, 1543: `GraphPlan::Path`
  - Lines 1581, 1622: `GraphPlan::Neighbors`
  - Line 1665: `GraphPlan::Subgraph`
- **Risk**: Medium — `&()` (unit type) is used instead of `&0u32`; this is clearly a placeholder. Each test constructs a plan inline and immediately passes it to `execute()`. Migrate each to `plan.compute_hash()` where `plan` is the local variable.
- **Strict TDD sequence**: Trivial (inline test plans); grep acceptance sufficient. Each plan is built with specific parameters; the hash will correctly reflect content.
- **Apply**: For each of the 5 sites, replace `PlanHash::compute(&())` with the appropriate `plan.compute_hash()` on the local plan variable.
- **Verification**: `cargo check -p cognicode-explorer`
- **Acceptance**: `grep -n "PlanHash::compute(&())" crates/cognicode-explorer/src/moldql/executor.rs` → 0

---

### T-010 — ladybug/lib.rs

- **Goal**: Replace 2 `PlanHash::compute(&0u32)` with `GraphPlan::compute_hash()` calls.
- **Files touched**: `crates/cognicode-ladybug/src/lib.rs`
- **Site count**: 2 (lines ~2925, ~3044)
- **Plan type**: `GraphPlan` (LadybugDB constructs graph plans)
- **Strict TDD sequence**: Trivial (2 sites); grep acceptance sufficient.
- **Apply**: Replace both `PlanHash::compute(&0u32)` with `plan.compute_hash()`. Inspect whether `plan` is owned or borrowed at each site.
- **Verification**: `cargo check -p cognicode-ladybug`
- **Acceptance**: `grep -n "PlanHash::compute(&0u32)" crates/cognicode-ladybug/src/lib.rs` → 0

---

## 3. Dependency Graph

```
T-001 (mold_plan.rs)          ─┐
T-002 (graph_plan.rs)         ─┤  Domain types — no cross-dependencies,
T-003 (limits.rs)             ─┤  can be migrated in any order within
T-004 (executor.rs, domain)   ─┤  the domain layer. T-004 is trivially
T-005 (lower.rs)              ─┘  independent but ordered last of domain
                                 for clarity.
         │
         ▼
T-006 (snapshot_graph_executor.rs)  ─┐  Infrastructure — depends on domain types
                                     │  being correct; runs after T-001..T-005.
         ┌───────────────────────────┘
         ▼
T-007 (lower_plan.rs)              ─┐
T-008 (lower_pattern_profile.rs)    ─┤  Application layer — depends on domain
T-009 (executor.rs, explorer)      ─┘  being stable; runs after T-006.
         │
         ▼
T-010 (ladybug/lib.rs)             ──  Final consumer; depends on all above.
```

**Linear enough**: domain → infra → explorer → ladybug. Within each phase, tasks are independent (no shared files).

---

## 4. Risk Per Task

| Task | Risk | Mitigation |
|------|------|-----------|
| T-001 mold_plan.rs | Low | 16 sites; `self.compute_hash()` straightforward |
| T-002 graph_plan.rs | Low | 14 sites; `self.compute_hash()` straightforward |
| T-003 limits.rs | Low | 4 test fixtures; `compute_hash()` straightforward |
| T-004 executor.rs | Very Low | 1 site; `plan` already in scope |
| T-005 lower.rs | **Medium** | 7 sites; some may need local plan binding before `compute_hash()` |
| T-006 snapshot_graph_executor.rs | **Medium** | 17 sites; ownership conflicts possible if plan moved after construction |
| T-007 lower_plan.rs | Low | 6 sites; existing `plan_metadata_for()` pattern already uses `compute_hash()` |
| T-008 lower_pattern_profile.rs | Very Low | 2 sites; trivial |
| T-009 executor.rs (explorer) | **Medium** | 5 sites with `&()` instead of `&0u32`; need to identify plan variant per site |
| T-010 ladybug/lib.rs | Low | 2 sites; straightforward |

**Fallback for blocked sites**: If a site cannot be migrated cleanly (e.g., plan not in scope, ownership conflict), add `// TODO(followup-cycle): migrate — placeholder hash` marker and `WARN` log, track in tasks.md comment. Do NOT leave the placeholder; the fallback is a tracked TODO, not a skip.

---

## 5. Strict TDD Test Inventory

| Test name | Task | File | What it asserts |
|-----------|------|------|-----------------|
| `test_mold_plan_distinct_hashes_different_limits` | T-001 | `mold_plan.rs` | `MoldPlan::Graph` with limit L1≠L2 → `compute_hash()` differs |
| `test_graph_plan_distinct_hashes_different_quantifiers` | T-002 | `graph_plan.rs` | `GraphPlan::Path` with max_hops N1≠N2 → hashes differ |
| `test_limits_validate_plan_hash_is_content_derived` | T-003 | `limits.rs` | `GraphPlan::Subgraph` with max_depth D1≠D2 → metadata hashes differ |
| `test_lower_graph_plan_hash_reflects_bounds` | T-005 | `lower.rs` | Lower same AST, different max_hops → resulting `GraphPlan` hashes differ |
| `test_lower_mold_plan_hash_derives_from_ast` | T-007 | `lower_plan.rs` | Lower same AST, different limits → resulting `MoldPlan` hashes differ |

**Total new tests**: 5 (all are RED-first, strict TDD)

All tests assert **hash derivation from content** (different content → different hash). Tests for **hash determinism** (same content → same hash) are implicitly covered by existing equality tests in `version.rs:266-364` (the `plan_version.rs` tests already verify deterministic serialization).

---

## 6. Branch Strategy

- **Branch name**: `fix/planhash-placeholder`
- **Base**: `c267fdca` (v0.80.1)
- **Commit strategy**: One atomic commit per task (10 commits total)
- **Commit message format**: `fix(plan): migrate <file> sites to compute_hash() — T-00<N>`

Examples:
```
fix(plan): migrate mold_plan.rs sites to compute_hash() — T-001
fix(plan): migrate graph_plan.rs sites to compute_hash() — T-002
...
```

- No `Co-Authored-By: AI`
- Single scope only (`fix(plan):`, not `fix(plan,scope):`)
- Squash NOT recommended — individual commits provide a clean audit trail of the migration

---

## 7. Verification Gates

### After T-001 (mold_plan.rs)
```bash
cargo check -p cognicode-core
cargo test -p cognicode-core --lib plan::mold_plan -- --nocapture 2>&1 | tail -20
grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/mold_plan.rs
# expect: 0
```

### After T-002 (graph_plan.rs)
```bash
cargo check -p cognicode-core
grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/graph_plan.rs
# expect: 0
```

### After T-003 (limits.rs)
```bash
cargo check -p cognicode-core
grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/limits.rs
# expect: 0
```

### After T-004 (executor.rs domain)
```bash
cargo check -p cognicode-core
grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/executor.rs
# expect: 0
```

### After T-005 (lower.rs)
```bash
cargo check -p cognicode-core
grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/domain/plan/lower.rs
# expect: 0
```

### After T-006 (snapshot_graph_executor.rs)
```bash
cargo check -p cognicode-core
grep -n "PlanHash::compute(&0u32)" crates/cognicode-core/src/infrastructure/graph/snapshot_graph_executor.rs
# expect: 0
```

### After T-007 (lower_plan.rs explorer)
```bash
cargo check -p cognicode-explorer
grep -n "PlanHash::compute(&0u32)" crates/cognicode-explorer/src/moldql/lower_plan.rs
# expect: 0
```

### After T-008 (lower_pattern_profile.rs)
```bash
cargo check -p cognicode-explorer
grep -n "PlanHash::compute(&0u32)" crates/cognicode-explorer/src/moldql/lower_pattern_profile.rs
# expect: 0
```

### After T-009 (executor.rs explorer)
```bash
cargo check -p cognicode-explorer
grep -n "PlanHash::compute(&())" crates/cognicode-explorer/src/moldql/executor.rs
# expect: 0
```

### After T-010 (ladybug)
```bash
cargo check -p cognicode-ladybug
grep -n "PlanHash::compute(&0u32)" crates/cognicode-ladybug/src/lib.rs
# expect: 0
```

### After ALL tasks (full workspace gate)
```bash
just test-unit
# must be GREEN

grep -rn "PlanHash::compute(&0u32)\|PlanHash::compute(&())" crates/
# must return 0 matches (doc comments in compute_hash() are OK)

cargo clippy --workspace --all-targets -- -D warnings
# must be GREEN — no new warnings
```

---

## 8. Rollback Plan

- **Mechanism**: `git revert <merge-commit>` or `git revert <commit-sha>` per task
- **Scope**: Single revert restores pre-migration state for that file
- **Data persistence**: No persisted `.lbdb` files are migrated; stale placeholder hashes in sandbox artifacts regenerate on next run — no data loss
- **Public API**: No API break; `PlanHash::compute()` remains available for non-plan content
- **Recovery time**: Minutes — revert is a single git command per affected task
