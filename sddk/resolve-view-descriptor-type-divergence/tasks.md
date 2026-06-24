# Kernel Tasks: Resolve ViewDescriptor Type Divergence

> **Change:** `sddk/resolve-view-descriptor-type-divergence`
> **Branch:** `refactor/resolve-view-descriptor-type-divergence`
> **Tag (post-archive):** `v0.12.3` (MINOR — feature change, not cleanup)
> **Mode:** auto · **Store:** engram · **Project:** cognicode
> **base HEAD:** `2f0b7f0` (v0.12.2)

## Router Context Used

- **Knowledge Coverage:** sufficient (verified). Roadmap/ADR sources consumed: ADR-008 (MCP ViewSpec tools — trigger), ADR-044 (data-sharing bridge — partial, deferred the type divergence), CONTEXT.md (ViewDescriptor/ViewExecutor trait terminology). Work-item scope bounded by proposal #2807 (Option B + C adopted). Ownership confirmed: explorer owns typed DTOs; core owns MCP schemas.
- **Context Quality:** **C1** (codebase-confirmed). All file paths, line numbers, callers, and dead-code status verified against the working tree at HEAD `2f0b7f0`. File list (10 files) cross-checked with grep of `\bViewDescriptor\b` in `crates/cognicode-explorer/src` (9 files with bare struct refs + lib.rs only needs `pub mod boundary;`).
- **Taxonomy (dominant axes):**
  - **naming-collision** (trait `domain::views::ViewDescriptor` at L1227 vs struct `dto::ViewDescriptor` at L91) — resolved by `Dto` suffix.
  - **type-duplication** (`ViewDescriptor` + `ViewSpec` mirrored across explorer and core) — resolved by 4 From impls in new `boundary.rs`.
  - **connascence-of-algorithm** (registry::raw_to_view_descriptor L38 mirrors `core::BuiltinDescriptorRaw::to_view_descriptor`) — resolved by L38 refactor.
  - **dead-code** (`list_all_builtin_descriptors` at registry.rs:310, zero callers) — removed.
- **Invariants Driving Tasks:**
  - HTTP API wire format unchanged (serde repr identical post-rename).
  - Dependency direction explorer→core preserved (core never imports explorer).
  - `ViewExecutor` trait (domain/views.rs:1239) name unchanged — distinct from renamed struct.
  - `ViewSpecSummary` preserved (consumed by Spotter search results — NOT dead).
  - All enum `From` impls are infallible because every `ViewKind`/`RendererKind`/`HierarchyKind`/`DataSource`/`Transform` has `Custom(String)` or `#[serde(other)]` forward-compat arms.
  - L1339 (`consolidated_handlers.rs`) is OUT OF SCOPE (persistence→schema, not explorer→core; lives in core which cannot import explorer by dep direction).
- **Recommended Effort:** **deepen** (already deepened via 3-option fork in proposal #2807; Option B + C adopted). No further lens work needed.

## Review Budget Forecast

- **Estimated changed lines:** ~210 (≈30 rename across 9 files + ~80 new boundary.rs + ~10 lib.rs/registry.rs refactor + ~15 deletion + ~100 ADR-046 prose). Well under 400.
- **400-line budget risk:** **Low**.
- **Chained PRs recommended:** **No** (single PR, 3 commits, each independently revertible and buildable).
- **Decision needed before apply:** **No** (coherence 88/100 PASS; spec AMENDED rev 2; design 4 corrections applied; L1339 explicitly OUT OF SCOPE; L38 explicitly IN SCOPE).

## Knowledge Traceability

- **Work item source artifacts:**
  - Proposal: engram **#2807** (Option B + C adopted, 3-option fork evaluated)
  - Spec: engram **#2809** (AMENDED rev 2, 27 scenarios = 22 ADDED + 3 MODIFIED + 2 NEW [23, 24]; 8 ADDED capabilities + 2 MODIFIED capabilities; 10-file rename list verified)
  - Design: engram **#2811** (4 corrections to proposal applied; From impls are PREVENTIVE ACL; 10-file list verified; ViewSpec From is infallible)
  - Coherence: engram **#2813** (88/100 PASS, score lift from 45→88 post-amendment)
  - Artifact registry: `b2f21857…` (coherence PASS, 88/100)
- **Ownership source:** design #2811 Architecture Decisions — *"explorer crate owns typed DTOs; core owns MCP schemas"* (also documented in CONTEXT.md).
- **Open knowledge gaps affecting execution:** **None blocking.** One optional follow-up mentioned in design (#2811 Open Questions): derive macro for field-parity enforcement, deferred to post-merge.

## Tasks

> **Structure:** 1 PR · 3 commits · 16 sub-tasks total.
> **Commit ordering rationale:** rename first (mechanical, compiler-enforced), then add the ACL (additive, no behavior change), then delete dead code + document (cleanest review surface).

### 1. `refactor(explorer): rename dto::ViewDescriptor → ViewDescriptorDto`

> **Scope:** 9 files receive the rename; lib.rs (10th file in spec list) is verified to NOT need rename work in this commit (it only gets `pub mod boundary;` in commit 2).
> **Risk:** mechanical — `cargo build` is the gate. No new files. No behavior change.
> **Touches:** `dto.rs` L91 (struct decl) + L69 (field type) + L201 (stale comment); `registry.rs` L32 (use) + ~12 sites L38-522; `facades/mod.rs` L33 + L137; `facades/view.rs` L13 + L51 + L270; `api_rationale_tests.rs` L184; `api_graph_tests.rs` L96 + L387; `session/service.rs` L302 + L336; `session/registry.rs` L300 + L335; `ask/dispatch.rs` L459 + L498.

- [ ] **1.1** `crates/cognicode-explorer/src/dto.rs`: rename `pub struct ViewDescriptor` → `pub struct ViewDescriptorDto` at L91; update `pub available_views: Vec<ViewDescriptor>` at L69 → `Vec<ViewDescriptorDto>`; update stale comment at L201.
- [ ] **1.2** `crates/cognicode-explorer/src/registry.rs`: update `use` statement at L32 to import `ViewDescriptorDto`; update ~12 reference sites (L39, L189, L261, L265, L270, L276, L286, L310-311, L377-378, L389, L408, L412, L422, L430, L508, L522). **Trait impls at L214 remain untouched** (`impl crate::domain::views::ViewDescriptor for ...` is fully-qualified — refers to the trait, not the renamed struct).
- [ ] **1.3** `crates/cognicode-explorer/src/facades/mod.rs`: update `use` at L33; update return type at L137.
- [ ] **1.4** `crates/cognicode-explorer/src/facades/view.rs`: update `use` at L13; update return types at L51 + L270.
- [ ] **1.5** `crates/cognicode-explorer/src/api_rationale_tests.rs`: update mock return type at L184.
- [ ] **1.6** `crates/cognicode-explorer/src/api_graph_tests.rs`: update mock return types at L96 + L387.
- [ ] **1.7** `crates/cognicode-explorer/src/session/service.rs`: update `use` at L302; update mock return type at L336.
- [ ] **1.8** `crates/cognicode-explorer/src/session/registry.rs`: update `use` at L300; update mock return type at L335.
- [ ] **1.9** `crates/cognicode-explorer/src/ask/dispatch.rs`: update `use` at L459; update mock return type at L498.
- [ ] **1.10** `crates/cognicode-explorer/src/lib.rs`: **verify** that no `dto::ViewDescriptor` references exist (this file only gets `pub mod boundary;` in commit 2; rename work skipped intentionally).
- [ ] **1.11** Build green gate: `cargo build -p cognicode-explorer` exits 0 with no warnings.
- [ ] **1.12** Commit: `refactor(explorer): rename dto::ViewDescriptor → ViewDescriptorDto` (one logical change; include all 9 files + lib.rs verification).

### 2. `feat(boundary): add From impls for ViewDescriptor/ViewSpec ACL`

> **Scope:** new module + new module declaration + L38 refactor + 4 From impls + round-trip tests.
> **Risk:** additive only — no existing code changes behavior. Orphan-rule compliant (explorer depends on core, so explorer owns the impls).
> **Touches:** `crates/cognicode-explorer/src/boundary.rs` (NEW, ~50 lines + tests); `crates/cognicode-explorer/src/lib.rs` (`pub mod boundary;`); `crates/cognicode-explorer/src/registry.rs` (L38 refactor: function body becomes one-line `From` call).

- [ ] **2.1** Create `crates/cognicode-explorer/src/boundary.rs` with:
  - Module-level rustdoc explaining the anti-corruption layer pattern (orphan-rule: explorer depends on core → explorer owns all 4 impls).
  - 4 From impls (verbatim from design #2811 Interfaces/Contracts):
    - `From<core::interface::mcp::schemas::ViewDescriptor> for ViewDescriptorDto` (lossless, pure copy of 4 fields).
    - `From<ViewDescriptorDto> for core::interface::mcp::schemas::ViewDescriptor` (mirror).
    - `From<dto::ViewSpec> for core::interface::mcp::schemas::ViewSpec` (lossless via serde for enum fields; `DataSource`/`Transform` via `serde_json::to_value`).
    - `From<core::interface::mcp::schemas::ViewSpec> for dto::ViewSpec` (lossless + infallible via `Custom(String)` / `#[serde(other)]` forward-compat arms on `ViewKind`/`RendererKind`/`HierarchyKind`/`DataSource`/`Transform`; `transform` uses `.and_then(|t| serde_json::from_value(t).ok())` for `Option<Value>` null edge case).
- [ ] **2.2** Add `pub mod boundary;` to `crates/cognicode-explorer/src/lib.rs` (module-level declaration).
- [ ] **2.3** Refactor `crates/cognicode-explorer/src/registry.rs::raw_to_view_descriptor` (L38) to use the new `From` impl:
  ```rust
  fn raw_to_view_descriptor(raw: &cognicode_core::schemas::BuiltinDescriptorRaw) -> ViewDescriptorDto {
      ViewDescriptorDto::from(crate::boundary::BuiltinDescriptorRawConversion::from(raw))
  }
  ```
  Or simpler — add a `From<&BuiltinDescriptorRaw>` for `ViewDescriptorDto` directly in `boundary.rs` and have L38 be a one-line call. **Verify:** `cargo build -p cognicode-explorer` still passes; function body ≤ 5 lines excluding signature.
- [ ] **2.4** Add round-trip unit tests in `boundary.rs`:
  - `view_descriptor_roundtrip`: `let s: core_schema = d.clone().into(); let d2: ViewDescriptorDto = s.into(); assert_eq!(d, d2);` for a fixture with `id="v1"`, `title="Call Graph"`, `is_builtin=true`, `source=None`.
  - `view_spec_roundtrip`: same shape, exercises both `ViewKind::VerticalSlice` ↔ `"vertical_slice"` and `ViewKind::CallGraph` ↔ `"call_graph"` mappings + `RendererKind::Graph` ↔ `"graph"` + `transform=None` edge case.
  - `view_spec_infallibility_smoke`: serialize-then-deserialize every variant of `ViewKind` and `RendererKind` via the From pair to confirm zero `unreachable!()` / panic paths.
- [ ] **2.5** Verify gates: `cargo build -p cognicode-explorer` exits 0; `cargo test -p cognicode-explorer boundary` passes (new round-trip tests + no regressions).
- [ ] **2.6** Commit: `feat(boundary): add From impls for ViewDescriptor/ViewSpec ACL` (boundary.rs + lib.rs + registry.rs L38 + tests as one logical change).

### 3. `chore(explorer): remove dead list_all_builtin_descriptors + ADR-046`

> **Scope:** dead-code removal + boundary contract documentation + stale comment update in core.
> **Risk:** deletion is safe (zero callers verified); ADR is docs-only.
> **Touches:** `crates/cognicode-explorer/src/registry.rs` L305-320 (delete); `docs/adr/ADR-046-view-descriptor-boundary.md` (NEW, ~100 lines); `crates/cognicode-core/src/interface/mcp/schemas.rs` L2452 (comment update).

- [ ] **3.1** Delete `crates/cognicode-explorer/src/registry.rs::list_all_builtin_descriptors` (L310-320) AND the stale doc comment immediately above (L305-309) that falsely claims "This is used by the MCP `list_view_specs` handler". Verify: `grep -rn "list_all_builtin_descriptors" .` returns zero outside `target/` and `.git/`.
- [ ] **3.2** Create `docs/adr/ADR-046-view-descriptor-boundary.md` capturing the boundary contract:
  1. **Field-parity invariant** — every field present in one side of the boundary pair must be present in the other, OR an explicit divergence is recorded in the ADR.
  2. **`From` impl ownership rule** — the explorer crate is the only crate that implements conversions because it is the only crate that imports both sides (orphan-rule + dep direction explorer→core).
  3. **`ViewSpecSummary` decision** — preserved because it is consumed by Spotter search results (NOT dead; verified by explore #2797).
  4. **Wire-format stability** — HTTP JSON shape unchanged across the change (serde derives identical, struct rename is internal).
  5. **Preventive ACL framing** — the 4 From impls formalize a boundary already documented in ADR-044, NOT a replacement of existing conversion code (per design #2811 Correction 2).
  6. **`raw_to_view_descriptor` (L38) consolidation** — the in-scope duplicate the From impls make redundant.
  7. **`consolidated_handlers.rs:1339` deferred** — persistence→schema is out of scope; belongs to a future change (different architectural concern: persistence row → schema, not explorer DTO → core schema).
  8. **References** — cite ADR-008 (MCP ViewSpec tool surface), ADR-044 (data-sharing bridge), CONTEXT.md (`ViewDescriptor`/`ViewExecutor` trait terminology), engram #2807/2809/2811.
- [ ] **3.3** Update stale comment at `crates/cognicode-core/src/interface/mcp/schemas.rs` L2452: change "Mirrors cognicode_explorer::dto::ViewDescriptor" → "Mirrors cognicode_explorer::dto::ViewDescriptorDto".
- [ ] **3.4** Verify gates: `cargo build` (workspace) exits 0; `cargo test` (workspace) passes; ADR renders in `cargo doc` if applicable.
- [ ] **3.5** Commit: `chore(explorer): remove dead list_all_builtin_descriptors + ADR-046` (dead-code deletion + ADR creation + comment fix as one logical change).

## Verification

### Mechanical (compiler-enforced)

| # | Gate | Expected |
|---|------|----------|
| 1 | `grep "pub struct ViewDescriptor " crates/cognicode-explorer/src/dto.rs` | empty (renamed) |
| 2 | `grep -rn "list_all_builtin_descriptors" .` | empty (deleted) |
| 3 | `grep -rn "dto::ViewDescriptor[^D]" crates/` | empty (only `ViewDescriptorDto` remains) |
| 4 | `cargo build -p cognicode-explorer` | exit 0, no warnings |
| 5 | `cargo test -p cognicode-explorer` | all pass (incl. new boundary round-trip tests) |
| 6 | `cargo build` (workspace) | exit 0 |
| 7 | `npm run build` (frontend, `apps/explorer-ui`) | exit 0 (wire format unchanged) |
| 8 | `npm run lint` (frontend) | no new warnings |

### Architectural (semantic)

| # | Gate | Expected |
|---|------|----------|
| 9 | `grep -rn "pub struct ViewDescriptorDto\b" crates/cognicode-explorer/src/dto.rs` | exactly 1 match |
| 10 | `grep -rn "pub trait ViewDescriptor\b" crates/cognicode-explorer/src/domain/views.rs` | exactly 1 match (unchanged) |
| 11 | `grep -rn "pub trait ViewExecutor\b" crates/cognicode-explorer/src/domain/views.rs` | exactly 1 match (unchanged) |
| 12 | `grep -rn "pub mod boundary" crates/cognicode-explorer/src/lib.rs` | exactly 1 match |
| 13 | `grep -rn "impl From.*ViewDescriptorDto" crates/cognicode-explorer/src/boundary.rs` | ≥ 2 matches (forward + inverse) |
| 14 | `grep -rn "impl From.*ViewSpec" crates/cognicode-explorer/src/boundary.rs` | ≥ 2 matches (forward + inverse) |
| 15 | `grep -rn "fn raw_to_view_descriptor" crates/cognicode-explorer/src/registry.rs` | 0 or 1 match (refactored to one-liner or deleted) |
| 16 | `grep -rn "ViewSpecSummary" crates/cognicode-explorer/src/` | preserved (Spotter consumer intact) |
| 17 | `ls docs/adr/ADR-046-view-descriptor-boundary.md` | exists |
| 18 | `cargo tree -p cognicode-explorer -i cognicode-core` | shows explorer→core only (core never imports explorer) |

### Behavior

| # | Gate | Expected |
|---|------|----------|
| 19 | Existing 648 vitest tests in `apps/explorer-ui` | pass (wire format unchanged) |
| 20 | `cd apps/explorer-ui && npx tsc --noEmit` | exit 0 |
| 21 | HTTP `GET /api/views/...` JSON byte-identical pre/post change | confirmed by existing snapshot/vitest |

## Rollback Notes

- **Commit 1 (rename)** — pure mechanical rename; rollback is `git revert <sha>`. No DB migration, no API break, no consumer impact beyond the rename.
- **Commit 2 (From impls)** — additive only (new module + new module declaration + L38 refactor that preserves behavior). Rollback is `git revert <sha>`. The 4 From impls become unused but are dead code, not load-bearing.
- **Commit 3 (delete dead code + ADR)** — safe rollback individually:
  - Deletion of `list_all_builtin_descriptors` has zero callers (verified by design #2811 Correction 1).
  - ADR-046 deletion is docs-only.
  - Stale comment update on `core/schemas.rs` L2452 is a one-line doc change.
- **No commit is a hotspot** — each is independently revertible. Worst-case rollback is three `git revert`s in reverse order.
- **No DB migration required.** No API break. No frontend impact.

---

## Reference: Spec Capability → Task Coverage

| Spec Capability (engram #2809) | Covered by tasks |
|---------------------------------|------------------|
| `view-descriptor-type-collision` → `view-descriptor-struct-renamed` | 1.1-1.10 |
| `view-descriptor-type-collision` → `view-descriptor-trait-canonical` | 1.2 (negative — trait ref `impl crate::domain::views::ViewDescriptor for ...` untouched) |
| `view-descriptor-boundary-mapping` → `view-descriptor-forward-conversion` + inverse | 2.1 |
| `view-spec-boundary-mapping` → `view-spec-forward-conversion` + inverse | 2.1 |
| `dead-code-removal` → `list-all-builtin-descriptors-deleted` | 3.1 |
| `dead-code-removal` → `raw-to-view-descriptor-refactored-or-deleted` | 2.3 |
| `mcp-handler-routes-through-conversion` → `raw-to-view-descriptor-routes-through-from-impl` | 2.3 |
| `boundary-contract-documentation` → `adr-046-documents-field-parity` | 3.2 |
| `boundary-module-declaration` → `boundary-module-added-to-explorer` | 2.1 + 2.2 |
| `view-spec-summary-preserved` → `view-spec-summary-not-deleted` | (negative — no task touches `ViewSpecSummary`; verification gate #16) |
| `explorer-dto-types` → `dto-references-updated-across-crate` | 1.1-1.9 |
| `workspace-verification` → `all-gates-pass-post-change` | All 21 verification gates |

## Reference: Design Invariant → Task Coverage

| Invariant (design #2811) | Enforced by |
|--------------------------|-------------|
| Field-parity: ViewDescriptorDto ↔ core::schemas::ViewDescriptor (4 fields) | 2.1 (From impl) + 2.4 (round-trip test) + ADR-046 step 1 |
| Field-parity: ViewSpec ↔ core::schemas::ViewSpec (11 fields) | 2.1 (From impl) + 2.4 (round-trip test) + ADR-046 step 1 |
| JSON wire format unchanged | 1.1 (no serde attr changes) + gate #7 (npm build) + gate #21 (HTTP snapshot) |
| Trait canonical: `domain::views::ViewDescriptor` | (negative) — no task touches domain/views.rs; gate #10 |
| `ViewSpecSummary` preserved | (negative) — no task touches it; gate #16 |
| L1339 deferred | (negative) — no task touches consolidated_handlers.rs; ADR-046 step 7 documents |
| Infallible enum conversions | 2.1 (From impls use `.unwrap()` only after Custom/Other arms guarantee success) |