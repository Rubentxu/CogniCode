# Kernel Tasks: sddk/E6-c4-backend-inference

## Router Context Used
- **Knowledge Coverage**: sufficient — proposal/spec/design triad complete (engram #2717/2718/2719); roadmap target file mapped; production site pinned at `graph.rs:201-216`; test sites enumerated (`620`, `665`, `804`).
- **Context Quality**: C2 (durable artifacts + concrete code locations). Divergence between spec and design on label casing is **resolved** by design authority (engram #2719 wins): `id` lowercase, `label` original case.
- **Taxonomy**: dominant axis = **correctness of C4 inference contract** (REQ-E6-1); secondary axis = **documentation honesty** (roadmap mismatch).
- **Invariants Driving Tasks**:
  - INV-1: `system_id = format!("system:{}", workspace_name.to_lowercase())` — stable, edge-safe.
  - INV-2: `system_label = workspace_name` — preserves UX branding (CogniCode, MyApp, etc.).
  - INV-3: design overrides spec on label casing; spec's `to_lowercase()` on label is **rejected**.
  - INV-4: T1.1 prod fix and T1.2 test updates are coupled (split commits break the suite).
- **Recommended Effort**: **verify** — narrow, well-scoped, mechanical fix + doc reconciliation. No new abstractions, no persistence, no new crate.

## Review Budget Forecast
- **Estimated changed lines**: ~46 (T1.1: ~10 / T1.2: ~6 / T1.3: ~15 / T1.4: ~15)
- **400-line budget risk**: **Low** — single commit, well under threshold.
- **Chained PRs recommended**: **No** — single PR/single commit is appropriate per Option A.
- **Decision needed before apply**: **No** — design decision (label casing) already adjudicated in engram #2719.

## Knowledge Traceability
- **Work item source artifacts**: engram #2717 (proposal), #2718 (spec), #2719 (design).
- **Ownership source**: engram #2719 (design) explicitly overrides spec on label casing → authoritative.
- **Open knowledge gaps affecting execution**: **None**. All test sites located and verified against current `main @ 2b5dac4`. `tempfile::Builder` API confirmed available (`Cargo.toml:68`).

## Commit Plan (single commit, 4 sub-tasks)
All four sub-tasks land together because:
1. T1.1 (prod fix) and T1.2 (3 test updates) MUST be coupled — splitting breaks the build.
2. T1.3 (regression test) is the proof that T1.1 actually works.
3. T1.4 (roadmap) is the truthful reconciliation that closes the loop with the user.

**Conventional commit message** (use verbatim in `sddk-apply`):
```
fix(explorer): derive C4 system name from workspace root

Replace hardcoded "system:cognicode" in build_architecture so the C4
view works for any workspace, not only CogniCode itself.

- Derive workspace_name from Path::new(root_path).file_name()
- id   = "system:" + workspace_name.to_lowercase()  (stable for edges)
- label = workspace_name                            (preserve original case)
- Update 3 test assertions to compute expected id dynamically
- Add regression test build_architecture_derives_system_name_from_workspace
- Reconcile docs/explorer-roadmap.md: E6.1-E6.4 done, E6.5 partial
```

---

## Tasks

### T1.1 — Fix hardcoded "CogniCode" in `build_architecture`
- **Files**: `crates/cognicode-explorer/src/facades/graph.rs` (lines 201-216 + line 222 hoist)
- **LOC delta**: +10 / -3 (net ~+7)
- **Depends on**: none
- **Concrete change**:
  1. Hoist `let root = Path::new(root_path);` from line 222 to **above** the C1 block (after the two `use` statements on 202-203).
  2. Replace lines 208-211 with:
     ```rust
     let workspace_name = root
         .file_name()
         .map(|s| s.to_string_lossy().to_string())
         .unwrap_or_else(|| "system".to_string());
     let system_id = format!("system:{}", workspace_name.to_lowercase());
     let system_label = workspace_name;
     ```
  3. Update the `GraphNode` constructor to use `id: system_id.clone(), label: system_label`.
  4. Delete the now-redundant `let root = Path::new(root_path);` at line 222.
- **Verification**:
  - `cargo check -p cognicode-explorer` → must finish with `Finished ... [unoptimized + debuginfo] target(s)` and **no errors**. Warnings tolerated.
  - `rg -n '"CogniCode"' crates/cognicode-explorer/src/facades/graph.rs` → must return **zero** matches inside `build_architecture` body.
  - `rg -n '"system:cognicode"' crates/cognicode-explorer/src/facades/graph.rs` → must return **zero** matches inside `build_architecture` body (test assertions are updated by T1.2).
- **Risk**: **Low** — pure local rename; no signature change; no behavioral change for CogniCode itself (lowercased basename = `cognicode`).
- **Rollback**: `git revert HEAD -- crates/cognicode-explorer/src/facades/graph.rs` (single-commit revert, restores hardcoded values). **Do not** attempt surgical rollback mid-apply; rely on the atomic revert.

### T1.2 — Update 3 test assertions to compute expected id dynamically
- **Files**: `crates/cognicode-explorer/src/facades/graph.rs` (lines 620, 665, 804)
- **LOC delta**: +6 / -3 (net ~+3)
- **Depends on**: T1.1 (must be in the same commit)
- **Concrete change** (apply uniformly to all three sites):
  - At the top of each test function (just after `let tmp = TempDir::new().unwrap();`), insert:
    ```rust
    let workspace_name = tmp.path().file_name().unwrap().to_string_lossy().to_string();
    let expected_system_id = format!("system:{}", workspace_name.to_lowercase());
    ```
  - Replace `"system:cognicode"` with `expected_system_id.as_str()` at all three sites.
- **Verification**:
  - `cargo test -p cognicode-explorer --lib facades::graph::tests::build_architecture_returns_system_node` → must pass.
  - `cargo test -p cognicode-explorer --lib facades::graph::tests::build_architecture_includes_workspace_containers` → must pass.
  - `cargo test -p cognicode-explorer --lib facades::graph::tests::build_architecture_creates_part_of_edges` → must pass.
  - `rg -n '"system:cognicode"' crates/cognicode-explorer/src/facades/graph.rs` → must return **zero** matches.
- **Risk**: **Low** — mechanical; the new `expected_system_id` uses the same algorithm as T1.1.
- **Rollback**: covered by the same `git revert HEAD` (T1.1 + T1.2 are in one commit).

### T1.3 — Add regression test `build_architecture_derives_system_name_from_workspace`
- **Files**: `crates/cognicode-explorer/src/facades/graph.rs` (append inside `mod tests`, before closing `}` at line 805)
- **LOC delta**: +15
- **Depends on**: T1.1, T1.2
- **Concrete change**: append this test (uses qualified path `tempfile::Builder` so the diff stays self-contained without touching the shared `use` block):
  ```rust
  #[tokio::test]
  async fn build_architecture_derives_system_name_from_workspace() {
      let tmp = tempfile::Builder::new().prefix("my-app").tempdir().unwrap();
      let repo = make_mock_repo(vec![], vec![]);
      let service = GraphServiceImpl::new(repo, None);

      let response = service
          .build_architecture(tmp.path().to_str().unwrap())
          .await
          .unwrap();

      let system_nodes: Vec<_> = response
          .nodes
          .iter()
          .filter(|n| n.kind == "system")
          .collect();
      assert_eq!(system_nodes.len(), 1);

      let workspace_name = tmp
          .path()
          .file_name()
          .unwrap()
          .to_string_lossy()
          .to_string();
      assert_eq!(system_nodes[0].id, format!("system:{}", workspace_name.to_lowercase()));
      assert_eq!(system_nodes[0].label, workspace_name);
      assert_ne!(system_nodes[0].id, "system:cognicode");
  }
  ```
- **Verification**:
  - `cargo test -p cognicode-explorer --lib facades::graph::tests::build_architecture_derives_system_name_from_workspace` → must pass with output `1 passed; 0 failed`.
  - `cargo test -p cognicode-explorer --lib facades::graph::tests` → all 7 tests must pass (6 existing + 1 new).
- **Risk**: **Low** — purely additive, no production code touched.
- **Rollback**: covered by `git revert HEAD`. To isolate during dev, comment out the test temporarily; it does not gate compilation.

### T1.4 — Reconcile `docs/explorer-roadmap.md` (Sprint E6 status)
- **Files**: `docs/explorer-roadmap.md` (lines 134-153)
- **LOC delta**: +9 / -5 (net ~+4 in changed tokens; ~15 touched lines including table rewrites)
- **Depends on**: T1.1, T1.2, T1.3 (must land in same commit for the roadmap to be truthful)
- **Concrete change** (line-by-line):
  - Line 138: `**Status:** ❌ Not started — \`cognicode-diagram\` crate does not exist`
    → `**Status:** ⚠️ Partial — inference complete in `cognicode-explorer::build_architecture`; type-safety and crate extraction deferred`
  - Line 142 (E6.1): `❌ | Crate does not exist` → `✅ Done | Inference lives in `cognicode-explorer::GraphServiceImpl::build_architecture` (graph.rs:201) — crate extraction not required`
  - Line 143 (E6.2): `❌ | —` → `✅ Done | Cargo.toml members + package.json apps → `container:` nodes (graph.rs:222+)`
  - Line 144 (E6.3): `❌ | —` → `✅ Done | `src/` directory inference → `component:` nodes`
  - Line 145 (E6.4): `❌ | —` → `✅ Done | Served via `build_architecture` subgraph query — no dedicated `/c4` endpoint needed (Option A)`
  - Line 146 (E6.5): `❌ | C4 kinds exist in domain but no inference engine` → `⚠️ Partial | C4 kinds registered in domain; inference emits `system`/`container`/`component` as string `kind`; full `NodeKind`/`EdgeKind` enum wiring deferred`
- **Verification**:
  - `rg -n 'E6' docs/explorer-roadmap.md` → returns the 5 rows with updated status icons (4 ✅ + 1 ⚠️).
  - `rg -n '❌' docs/explorer-roadmap.md` in the E6 block (lines 134-153) → must return **zero** matches.
  - `sed -n '134,153p' docs/explorer-roadmap.md` → visual inspection confirms truthful status table.
- **Risk**: **Low** — documentation only; no code behavior change.
- **Rollback**: covered by `git revert HEAD`.

---

## Verification (whole-change, run in order)
1. `cargo check -p cognicode-explorer` — must finish without errors.
2. `cargo test -p cognicode-explorer --lib facades::graph::tests` — all 7 tests must pass. Tolerate unrelated pre-existing failures elsewhere in the workspace per project policy.
3. `rg -n '"system:cognicode"' crates/cognicode-explorer/src/facades/graph.rs` — zero matches.
4. `rg -n '"CogniCode"' crates/cognicode-explorer/src/facades/graph.rs` — zero matches inside `build_architecture` body (test assertion replaced; only legitimate matches should be in unrelated code, which we verified there are none in the prod body).
5. `sed -n '134,153p' docs/explorer-roadmap.md` — visual confirmation of truthful Sprint E6 table.

## Rollback Notes (whole-change)
- **Atomic**: `git revert HEAD` — produces one revert commit restoring hardcoded values and old roadmap text.
- **Selective (if needed during dev)**: `git restore --source=HEAD~1 crates/cognicode-explorer/src/facades/graph.rs docs/explorer-roadmap.md` — but only valid if T1.1-T1.4 were committed as a separate, un-pushed commit.
- **Do not** cherry-revert individual sub-tasks: T1.2 without T1.1 leaves tests asserting against a missing value; T1.3 without T1.1 produces a test that asserts on a value the code does not emit.

## Out of Scope (explicit, per Option A decisions)
- New `cognicode-diagram` crate (rejected — inference stays in `cognicode-explorer`).
- Type-safety refactor to `NodeKind`/`EdgeKind` enums (deferred — current `String` `kind` is sufficient).
- Persistence of inferred C4 structure (rejected — derived on each request).
- Dedicated `GET /api/graph/c4` endpoint (rejected — `build_architecture` already serves the data).
