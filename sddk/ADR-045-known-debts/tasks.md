# Kernel Tasks: sddk/ADR-045-known-debts

## Router Context Used
- **Knowledge Coverage**: sufficient — proposal/spec/design triad complete (engram #2731/2732/2733); all 3 debt locations code-verified at current `main @ 10ef35a`.
- **Context Quality**: C2 (verified facts, code-pinned locations).
- **Taxonomy**: dominant axes = persistence-shape dual-model (Debt 2) + durability gap (Debt 3) + dead-route seam (Debt 1).
- **Invariants Driving Tasks**:
  - INV-1: ADR-045 must explicitly surface the live `columns` / `default_navigation_mode="column"` contradiction with ADR-039 Decision 1 (verified at `dto.rs:447` + `ExplorationPath.columns`).
  - INV-2: Ordering constraint Debt 2 → Debt 3 must be documented (do not entrench legacy `ExplorationPath` in SQL).
  - INV-3: Doc-only scope — no code, no tags, no amendments to ADR-039/040/044.
- **Recommended Effort**: **verify** — narrow, mechanical, fully reversible.

## Review Budget Forecast
- **Estimated changed lines**: ~95 (T1.1: ~92 / T1.2: ~3)
- **400-line budget risk**: **Low** — single commit, well under threshold.
- **Chained PRs recommended**: **No** — single PR/single commit is appropriate for a doc-only change.
- **Decision needed before apply**: **No** — all adopted decisions captured (single ADR + 3 sections + ADR-039 contradiction + Debt 2→3 ordering + Defer all 3 + doc-only).

## Knowledge Traceability
- **Work item source artifacts**: engram #2731 (proposal), #2732 (spec), #2733 (design).
- **Ownership source**: engram #2733 (design) governs file structure and KNOWN-DEBT cross-link edit pattern.
- **Open knowledge gaps affecting execution**: **None**. All file paths verified at `main @ 10ef35a`:
  - `docs/adr/ADR-045-exploration-debts.md` — does not exist (will be created).
  - `crates/cognicode-explorer/src/facades/persistence.rs:209-224` — confirmed 3-bullet KNOWN-DEBT block.
  - `crates/cognicode-explorer/src/dto.rs:362-477` + `dto.rs:447` — confirmed.
  - `crates/cognicode-explorer/src/facades/persistence.rs:27,30` — confirmed (two `Mutex<HashMap>` type aliases).

## Commit Plan (single commit, 2 sub-tasks)

Both sub-tasks land together because T1.2's rustdoc cross-link refers to the ADR created by T1.1. Splitting would produce a dangling reference in the intermediate state.

**Conventional commit message** (use verbatim in `sddk-apply`):
```
docs(adr): add ADR-045 known exploration-persistence debts

Promote the 3 KNOWN-DEBT items already recorded in the E4.5 LIST-endpoint
rustdoc (persistence.rs:209-224) to a single canonical, reviewed ADR.

- Debt 1 — get_exploration mis-wire (api.rs:776-788): defer + plan.
  Recommend remove orphaned route (zero consumers verified: no
  load_exploration_path trait method; frontend uses /explorations/:id/
  artifacts or in-hand LIST data).
- Debt 2 — ExplorationPath vs ExplorationSession dual model
  (dto.rs:362-477): defer + timeline. Unify onto ExplorationSession.
- Debt 3 — in-memory paths + sessions HashMaps (persistence.rs:27,30):
  defer + timeline. Postgres persistence for the UNIFIED model,
  blocked on Debt 2 (must precede to avoid entrenching legacy shape).

Surfaced (not amended) the ADR-039 contradiction: backend still writes
columns on ExplorationPath and defaults navigation_mode="column"
(dto.rs:447) while ADR-039 Decision 1 mandates a hard cut to the
pane-stack model. Reconciliation is a separate future ADR.

Doc-only / fully reversible: delete the new file + revert the rustdoc
hunk. No code, no tags, no amendments to ADR-039/040/044.
```

---

## Tasks

### T1.1 — Create `docs/adr/ADR-045-exploration-debts.md`
- **Files**: `docs/adr/ADR-045-exploration-debts.md` (new)
- **LOC delta**: +92 (target ~80–100 per proposal; meets acceptance)
- **Depends on**: none
- **Concrete structure** (level-2 sections, in order):
  1. **Header** — `Title: ADR-045 — Known Exploration-Persistence Debts`, `Status: Accepted`, `Date: 2026-06-23`, supersedes nothing, related: `ADR-039`, `ADR-040`, `ADR-044`. One-paragraph Context + Status summary.
  2. **Debt 1 — `get_exploration` mis-wire**
     - Description: route `GET /api/explorations/:id` is doc-claimed as "return a previously saved exploration path" but calls `load_exploration_session` and returns an `ExplorationSession`.
     - Location: `crates/cognicode-explorer/src/api.rs:776-788`.
     - Disposition: **Defer + plan**. Recommend **remove orphaned route** (zero consumers verified: no `load_exploration_path` trait method; frontend uses `/explorations/:id/artifacts` or in-hand LIST data).
     - Rationale: latent only — no consumer; adding `load_exploration_path` would entrench the legacy shape.
  3. **Debt 2 — Dual model (`ExplorationPath` vs `ExplorationSession`)**
     - Description: `ExplorationPath` (legacy `columns` model, `dto.rs:362`) and `ExplorationSession` (pane-stack model per ADR-040 Wave 3, `dto.rs:432`) coexist with parallel save/list/restore paths.
     - Location: `crates/cognicode-explorer/src/dto.rs:362-477` + ~8 frontend files in `apps/explorer-ui/` using `ExplorationPath`.
     - Disposition: **Defer + timeline** — unify onto `ExplorationSession` (ADR-039-aligned).
     - Rationale: must precede Debt 3 to avoid entrenching the legacy `ExplorationPath` shape in SQL.
  4. **Debt 3 — In-memory store lifetime**
     - Description: `paths` + `sessions` HashMaps are process-lifetime only — server restart loses all rows.
     - Location: `crates/cognicode-explorer/src/facades/persistence.rs:27,30`.
     - Disposition: **Defer + timeline** — Postgres persistence for the **unified** model (blocked on Debt 2). No PG exploration table exists today.
     - Rationale: highest user impact (restart loses state); blocked on Debt 2 ordering.
  5. **ADR-039 contradiction** — state explicitly: backend still writes `columns` on `ExplorationPath` and `default_navigation_mode()` returns `"column"` (`dto.rs:447`) while ADR-039 Decision 1 mandates a hard cut to pane-stack navigation. State scope: ADR-045 surfaces but does not amend; reconciliation is a separate future ADR.
  6. **Ordering constraint: Debt 2 → Debt 3** — state explicitly: Debt 3 is **blocked on Debt 2**. Rationale: persisting first would entrench the legacy `ExplorationPath` columns model in a new SQL table.
  7. **Open Question** — Debt 1 final fix shape: **remove orphaned route** (recommended, zero consumers verified) vs **add `load_exploration_path`** trait method. The implementation ADR for Debt 1 must confirm before any code lands.
- **Verification**:
  - `test -f docs/adr/ADR-045-exploration-debts.md` → exit 0.
  - `rg -c '^## Debt [123]' docs/adr/ADR-045-exploration-debts.md` → outputs `3` (exactly three debt sections).
  - `rg -c '^## ADR-039' docs/adr/ADR-045-exploration-debts.md` → outputs `1` (contradiction subsection present).
  - `rg -n 'defer|Defer' docs/adr/ADR-045-exploration-debts.md` → returns ≥ 3 matches (one disposition per debt).
  - `rg -n 'api.rs:776-788|dto.rs:362-477|dto.rs:447|persistence.rs:27,30' docs/adr/ADR-045-exploration-debts.md` → returns matches in the expected Location fields.
  - `rg -n 'blocked on Debt 2|Debt 2.*Debt 3' docs/adr/ADR-045-exploration-debts.md` → at least 1 match (ordering constraint).
- **Risk**: **Low** — markdown only; no executable change.
- **Rollback**: `rm docs/adr/ADR-045-exploration-debts.md` (or rely on the atomic commit revert in T1.2).

### T1.2 — Append ` See ADR-045 for the disposition.` to each KNOWN-DEBT bullet in `persistence.rs:209-224`
- **Files**: `crates/cognicode-explorer/src/facades/persistence.rs` (lines 209-224, text-only edit)
- **LOC delta**: +3 (one appended clause per existing bullet; no new bullets, no rewrites)
- **Depends on**: T1.1 (ADR-045 file must exist before the cross-link resolves).
- **Concrete change** — apply uniformly to each of the three existing KNOWN-DEBT bullets (lines 211-215, 216-221, 222-224):
  - Append ` See ADR-045 for the disposition.` at the end of each bullet's last text line, before the next bullet or the rustdoc block's closing `///`.
  - The three-bullet shape must be preserved (no bullets added, removed, or reordered).
- **Verification**:
  - `rg -c 'See ADR-045 for the disposition' crates/cognicode-explorer/src/facades/persistence.rs` → outputs `3` (one per bullet).
  - `rg -n 'KNOWN-DEBT' crates/cognicode-explorer/src/facades/persistence.rs` → outputs `3` (bullet count preserved).
  - `cargo check -p cognicode-explorer` → must finish without errors. Warnings tolerated. Tolerate pre-existing failures per project policy.
  - `cargo doc -p cognicode-explorer --no-deps` → must parse the rustdoc block successfully (cheap smoke check that the comment still terminates cleanly).
- **Risk**: **Low** — text-only append; no code path changes; rustdoc block already terminates with `///` on line 224.
- **Rollback**: covered by the same `git revert HEAD` (T1.1 + T1.2 in one commit).

---

## Verification (whole-change, run in order)
1. `test -f docs/adr/ADR-045-exploration-debts.md` — ADR-045 exists.
2. `rg -c '^## Debt [123]' docs/adr/ADR-045-exploration-debts.md` — exactly `3` debt sections.
3. `rg -c '^## ADR-039' docs/adr/ADR-045-exploration-debts.md` — contradiction subsection present.
4. `rg -c 'See ADR-045 for the disposition' crates/cognicode-explorer/src/facades/persistence.rs` — exactly `3` cross-links (one per KNOWN-DEBT bullet).
5. `rg -c 'KNOWN-DEBT' crates/cognicode-explorer/src/facades/persistence.rs` — exactly `3` (bullet structure preserved).
6. `git diff --stat` against the change base — exactly two paths changed: one new `.md` and one modified `.rs`.
7. `git diff <base>..HEAD -- docs/adr/ADR-039-explorer-navigation-model.md docs/adr/ADR-040-graph-view-renderer.md docs/adr/ADR-044-mcp-viewspec-followup.md` — empty (no amendment of existing ADRs).
8. `cargo check -p cognicode-explorer` — finishes without errors. Tolerate pre-existing failures elsewhere per project policy.

## Rollback Notes (whole-change)
- **Atomic**: `git revert HEAD` — one revert commit restores the rustdoc block and removes the new ADR file.
- **Selective (if needed during dev)**: `git restore --source=HEAD~1 crates/cognicode-explorer/src/facades/persistence.rs && rm docs/adr/ADR-045-exploration-debts.md` — only valid if T1.1 + T1.2 were committed as a separate, un-pushed commit.
- **Do not** cherry-revert T1.2 without T1.1: the cross-link would dangle.

## Out of Scope (explicit, per proposal)
- Any code change (Debt 1 implementation, Debt 2 unification, Debt 3 PG migration).
- New git tags.
- Amendments to ADR-039, ADR-040, or ADR-044.
- New tests or spec deltas.
- Implementation ADR for Debt 1 (separate future cycle).
- ADR-039 contradiction reconciliation (separate future ADR).
