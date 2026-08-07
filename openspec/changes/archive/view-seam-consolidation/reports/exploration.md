# Exploration: view-seam-consolidation

**Change:** view-seam-consolidation
**Date:** 2026-06-13
**Artifact store:** OpenSpec + Engram
**Status:** success
**Skill resolution:** paths-injected (sdd-explore, cognicode-sdd, entropy-sdd, rust-patterns)

---

## Executive Summary

The current view seam in `cognicode-explorer` has a **split-brain architecture**: `ViewDescriptorProvider` (in `registry.rs`) provides metadata-only discovery via `inventory::submit!`, while `ExplorerService` (`service.rs`) holds a parallel set of `match view_id` arms that actually build `ContextualView`s. Five free-function `*_descriptor_list()` helpers in `service.rs` duplicate the knowledge already encoded in the four `inventory`-registered providers. All five grilled decisions (ISP traits, pre-resolved `ViewContext`, single-capability-per-concept, `build()`- validates-applicability, all-at-once migration) are **feasible against the real codebase** with no architectural blockers. The migration touches 4 production files (`service.rs`, `registry.rs`, `domain/views.rs`, `dto.rs`) and breaks **6 existing tests** that assert on specific `available_views` lists. The biggest risk is a **semantic drift between descriptor listings and `build()` capability** during the transition window — mitigated by making `applies_to()` and `build()` pattern-match the same enum.

---

## Current State

The view discovery + execution pipeline has **three parallel sources of truth**:

1. **`ExplorerService` match arms** (`service.rs:1247-1681`) — the execution path. Five private methods (`contextual_view_{symbol,file,scope,issue,rule}`) each hold a `match view_id { ... }` that calls `views::build_*` functions.

2. **Five `*_descriptor_list()` free functions** (`service.rs:1858-1947`) — hardcoded `Vec<ViewDescriptor>` lists returned by `inspect_object()` and `spotter_search()`. These are **independent** from both the match arms and the registry.

3. **`ViewRegistry` + `ViewDescriptorProvider`** (`registry.rs` + `domain/views.rs:1213-1276`) — four `inventory::submit!` registrations (OverviewProvider, CallGraphProvider, SourceProvider, QualityProvider). Used by `available_views()` and `available_views_for_workspace()`.

The three sources **do not agree**:
- `symbol_descriptor_list()` returns `[overview, call-graph, source, evidence, quality]` — 5 views.
- `ViewRegistry::list_for(Symbol)` returns `[call-graph, overview, quality, source]` — 4 views (no `evidence` provider registered).
- `file_descriptor_list()` returns `[overview, symbols, quality]` — but `symbols` and `dependencies`/`hotspots` have **no registered providers** at all.

This is the split-brain the consolidation eliminates.

---

## View Map: view_id → object_type → builder → ports

| view_id | Object Types (match arms) | Builder Function | Ports Needed | Registered Provider? |
|---------|--------------------------|------------------|--------------|---------------------|
| `overview` | Symbol, File, Scope, Issue, Rule | `build_overview` (Symbol), `build_file_overview` (File), `build_scope_overview` (Scope), `build_issue_detail` (Issue), `build_rule_detail` (Rule) | Symbol: `SymbolRepository`; File: `SymbolRepository` + `SourceReader`; Scope: `SymbolRepository`; Issue: `QualityRepository`; Rule: `QualityRepository` | ✅ OverviewProvider (Symbol, File, Scope only — **missing Issue, Rule**) |
| `call-graph` | Symbol | `build_callgraph` | `SymbolRepository` (+ `as_metadata_aware`) | ✅ CallGraphProvider (Symbol) |
| `source` | Symbol | `build_source` | `SourceReader` | ✅ SourceProvider (Symbol) |
| `evidence` | Symbol | `build_evidence_view` (private fn in `service.rs:1770`) | `SymbolRepository` + `SourceReader` | ❌ **No provider** — exists only in `symbol_descriptor_list()` |
| `quality` | Symbol, File, Scope | `build_symbol_quality_view`, `build_file_quality_view`, `build_scope_quality_view` | `QualityRepository` (optional) | ✅ QualityProvider (Symbol, File, Scope, Issue, Rule) |
| `symbols` | File | `build_file_symbols` | `SymbolRepository` (symbols slice) | ❌ **No provider** — exists only in `file_descriptor_list()` |
| `dependencies` | Scope | `build_scope_dependencies` | `SymbolRepository` (+ `as_metadata_aware`) | ❌ **No provider** — exists only in `scope_descriptor_list()` |
| `hotspots` | Scope | `build_scope_hotspots` | Pre-sorted symbols (service computes via `top_hotspots`) | ❌ **No provider** — exists only in `scope_descriptor_list()` |

### Key finding: `evidence` view is built by a **private function in `service.rs`** (`build_evidence_view` at line 1770), NOT by a `views::build_*` function. This function calls `crate::domain::evidence::build_evidence_blocks` and wraps the result. The consolidated capability must move this logic into a `ViewExecutor::build()` implementation.

### Key finding: `hotspots` requires **service-level pre-processing** — `top_hotspots()` (service.rs:1975) sorts symbols by `fan_in` before passing them to `build_scope_hotspots`. The `ViewExecutor::build()` must either (a) do this sorting inside `build()`, or (b) receive pre-sorted data via `ViewContext`. Decision 2's `InspectionTarget::Scope` carries `symbols: Vec<ResolvedSymbol>` — the sorting can happen in `build()` since `ViewContext` has `&dyn SymbolRepository`.

---

## Descriptor List Functions (to be deleted)

| Function | Location | Returns | Used by |
|----------|----------|---------|---------|
| `symbol_descriptor_list()` | service.rs:1858 | `[overview, call-graph, source, evidence, quality]` | `inspect_symbol()`, `spotter_search()` |
| `file_descriptor_list()` | service.rs:1888 | `[overview, symbols, quality]` | `inspect_file()` |
| `scope_descriptor_list()` | service.rs:1908 | `[overview, dependencies, hotspots, quality]` | `inspect_scope()` |
| `issue_descriptor_list()` | service.rs:1933 | `[overview]` | `inspect_quality_issue()` |
| `rule_descriptor_list()` | service.rs:1941 | `[overview]` | `inspect_rule()` |

All five are private free functions. They produce `ViewDescriptor` DTOs with only `id` + `title` (no `view_kind`, no `renderer_kind`, no `applies_to`). After consolidation, the `InspectableObjectSummary.available_views` field should be populated by querying the registry (which stores `dyn ViewExecutor` that exposes full metadata).

---

## Consumers of ViewDescriptorProvider / ViewRegistry

### Production consumers (will break when registry changes):

| Consumer | File | What it calls | Impact |
|----------|------|--------------|--------|
| `ExplorerService::available_views()` | service.rs:630 | `view_registry.list_for(object_type)` | **Signature change** — `list_for` returns descriptors from `dyn ViewExecutor`, needs to extract metadata |
| `ExplorerService::available_views_for_workspace()` | service.rs:641 | `view_registry.list_for_with_store(...)` | Same as above, async variant |
| `ExplorerService` constructors | service.rs:132 | `ViewRegistry::new(None)` | May change if registry constructor signature changes |
| `api.rs::available_views` handler | api.rs:705 | Calls `service.available_views_for_workspace()` | Indirect — breaks only if service method signature changes |
| `api.rs::contextual_view` handler | api.rs:719 | Calls `service.contextual_view()` | **Major** — this is the execution path that moves to `ViewExecutor::build()` |
| `mcp.rs` TOOL_GET_VIEWS | mcp.rs:842 | `service.available_views()` | Indirect |
| `mcp.rs` TOOL_GET_VIEW | mcp.rs:863 | `service.contextual_view()` | **Major** — execution path |
| `ask/dispatch.rs::code_quality` | dispatch.rs:256 | `service.contextual_view(&resolved, "quality")` | **Major** — hardcoded view_id "quality" |
| `ask/dispatch.rs::generic_description` | dispatch.rs:358 | `service.contextual_view(&resolved, "overview")` | **Major** — hardcoded view_id "overview" |
| `load_view` (PG feature) | service.rs:821 | `self.contextual_view(&mvp_id, lens_to_view_id(...))` | **Major** — execution path, PG-gated |

### Test consumers (will break):

| Test | File | What it asserts |
|------|------|----------------|
| `available_views_dispatches_per_variant` | service.rs:2789 | Asserts exact `available_views` lists per object type |
| `contextual_view_dispatches_file_to_correct_builder` | service.rs:2836 | Asserts file overview + symbols views work |
| `contextual_view_dispatches_scope_to_correct_builder` | service.rs:2860 | Asserts scope overview + dependencies + hotspots work |
| `contextual_view_rejects_unknown_view_id_per_variant` | service.rs:2884 | Asserts `ViewNotAvailable` error for mismatched view_id/type |
| `inspect_file_returns_file_summary` | service.rs:2693 | Asserts `available_views == [overview, symbols, quality]` |
| `inspect_scope_returns_scope_summary` | service.rs:2734 | Asserts `available_views == [overview, dependencies, hotspots, quality]` |
| `built_in_providers_are_accessible` | registry.rs:429 | Asserts 4 providers are registered via inventory |
| `view_descriptor_provider_is_object_safe` | registry.rs:359 | Asserts trait object safety |
| `view_descriptor_from_provider_extracts_metadata` | registry.rs:367 | Asserts `From<&dyn ViewDescriptorProvider>` impl |
| `descriptor_lists_three_object_types` | lenses/hotspots.rs:532 | References descriptor lists (needs investigation) |
| MCP test at mcp.rs:3622 | mcp.rs:3622 | Asserts `available_views` is non-empty |
| MCP test at mcp.rs:3630 | mcp.rs:3630 | Calls `available_views("symbol:...")` |

### Hidden consumers (view builders called outside service match arms):

| Consumer | Function called | Risk |
|----------|----------------|------|
| `ask/dispatch.rs::code_quality` | `contextual_view(_, "quality")` | Uses service method — safe if method signature preserved |
| `ask/dispatch.rs::generic_description` | `contextual_view(_, "overview")` | Same |
| `load_view` (PG feature) | `contextual_view(_, lens_to_view_id(...))` | Uses `lens_to_view_id` to translate stored lens → view_id — this mapping function may become unnecessary |
| **No direct calls to `views::build_*`** outside `service.rs` | — | The builder functions are `pub` but only called from service match arms. Safe to move into capability `build()` methods. |

---

## Entropy Analysis (Connascence Landscape)

**Method:** Heuristic (CogniCode MCP unavailable in this session)

### Connascence pairs

| Component A | Component B | Connascence Type | I(bits) | Severity | Hidden? |
|-------------|-------------|------------------|---------|----------|---------|
| `service.rs` match arms | `domain/views.rs` builder fns | **Name** | log2(13) ≈ 3.70 | ❌ **HIGH** | No — 13 `build_*` functions called by name |
| `service.rs` `*_descriptor_list()` | `service.rs` match arms | **Meaning** | ~2.0 | ⚠️ **MEDIUM** | **YES** — both encode "which views exist for type X" but in different formats with no shared source of truth |
| `registry.rs` providers | `service.rs` match arms | **Meaning** | ~2.5 | ⚠️ **MEDIUM** | **YES** — `OverviewProvider.applies_to()` says `[Symbol, File, Scope]` but the match arm also handles Issue + Rule overview. Registry says 4 providers, descriptor lists say 5 view_ids for Symbol. |
| `views.rs` `build_callgraph` | `SymbolRepository::as_metadata_aware()` | **Algorithm** | ~1.5 | ⚠️ Low | No — documented escape hatch |
| `service.rs` `lens_to_view_id` | match arm view_ids | **Name** | ~1.0 | ⚠️ Low | No — string mapping function |

### SOLID-Entropy Violations

| Principle | Violation | Evidence |
|-----------|-----------|----------|
| **ISP** | `ViewDescriptorProvider` forces listing consumers to depend on a trait that cannot build views, while the service has a separate execution path that listing consumers cannot see. H(view) >> H(needs). | 3 separate sources of truth for "what views exist" |
| **OCP** | Adding a new view requires editing: (1) a `*_descriptor_list()` function, (2) a match arm in `contextual_view_*`, (3) a `build_*` function in `views.rs`, (4) optionally an `inventory::submit!`. H(Δ_existing) ≈ 2.0 bits. | 4 edit points for 1 new view |
| **DRY** | `evidence` view_id appears in `symbol_descriptor_list()` but has no registered provider and no match arm coverage in `available_views()`. | Descriptor list says it exists, registry says it doesn't |

### Coupling Score
- **H_external ≈ 3.7 bits** (HIGH) — service.rs ↔ views.rs name coupling alone is at the "refactor before adding features" threshold.
- The consolidation **reduces** this to ~1.0 bit (one trait method `build()` dispatches internally).

### Design Quality Score (current): **~0.25** (NEEDS REFACTORING)
- Coupling: HIGH (3.7 bits)
- Cohesion: LOW (service.rs mixes 5 object types × 5 view types = 25 dispatch cells)
- LSP: N/A (no subtypes yet)
- Connascence: HIGH (meaning connascence between 3 sources of truth)

**Expected post-consolidation DQS: ~0.65** (ACCEPTABLE → approaching EXCELLENT)

---

## inventory::submit! Registration Changes

### Current registrations (domain/views.rs:1213-1276):

| Provider | id | applies_to | view_kind | renderer_kind |
|----------|----|-----------|-----------|--------------|
| `OverviewProvider` | `overview` | Symbol, File, Scope | VerticalSlice | Json |
| `CallGraphProvider` | `call-graph` | Symbol | CallGraph | Graph |
| `SourceProvider` | `source` | Symbol | SourceView | Code |
| `QualityProvider` | `quality` | Symbol, File, Scope, QualityIssue, Rule | QualityHotspots | Table |

### What changes:

1. **Each provider becomes a `ViewExecutor`** (adds `async build(&self, ctx: &ViewContext) -> ExplorerResult<ContextualView>`).
2. **`OverviewProvider` must handle 5 object types** (Symbol, File, Scope, Issue, Rule) — currently only declares 3. The `build()` method pattern-matches `ctx.target` and dispatches to the appropriate `build_overview`/`build_file_overview`/`build_scope_overview`/`build_issue_detail`/`build_rule_detail`.
3. **New providers needed** for currently-unregistered views:
   - `EvidenceProvider` (Symbol only) — wraps `build_evidence_view` logic
   - `SymbolsProvider` (File only) — wraps `build_file_symbols`
   - `DependenciesProvider` (Scope only) — wraps `build_scope_dependencies`
   - `HotspotsProvider` (Scope only) — wraps `build_scope_hotspots` (includes `top_hotspots` sorting)
4. **`QualityProvider` already covers all 5 types** but the match arms only build quality views for Symbol/File/Scope. The provider's `build()` must handle Issue/Rule gracefully (return error or empty view).
5. **`ProviderWrapper` changes** from holding `&'static dyn ViewDescriptorProvider` to `&'static dyn ViewExecutor`.

---

## Risk Assessment (ranked)

### 1. 🔴 HIGH — Semantic drift between `applies_to()` and `build()` coverage
**Risk:** A capability declares `applies_to: [Symbol, File]` but its `build()` only handles `Symbol`. The service no longer checks `applies_to` (Decision 4), so the user sees the view in the list but gets an error when they try to build it.

**Mitigation:** Each capability's `build()` must pattern-match **every** variant in `applies_to()` and return a rich error for unhandled types. Add a test that asserts: for every registered capability, for every type in `applies_to()`, `build()` does NOT return "unsupported type" error.

### 2. 🟠 MEDIUM — `hotspots` view requires service-level pre-processing
**Risk:** `build_scope_hotspots` expects pre-sorted symbols. Currently `top_hotspots()` (service.rs:1975) does this sorting using `repo.fan_in()`. Moving this into `ViewExecutor::build()` means the capability needs `&dyn SymbolRepository` to call `fan_in`.

**Mitigation:** `ViewContext` already carries `repo: &dyn SymbolRepository` (Decision 2). The `HotspotsProvider::build()` can call `repo.fan_in()` internally. The sorting logic moves from service.rs into the capability.

### 3. 🟠 MEDIUM — `evidence` view has no `views::build_*` function
**Risk:** `build_evidence_view` is a private function in `service.rs:1770`, not in `domain/views.rs`. The capability must move this function into the capability module or into `domain/views.rs`.

**Mitigation:** Move `build_evidence_view` into `domain/views.rs` as `pub fn build_evidence(...)` or inline it into `EvidenceProvider::build()`.

### 4. 🟡 LOW — `lens_to_view_id` translation function becomes partially redundant
**Risk:** `service.rs:2203` translates stored lens names ("callgraph" → "call-graph"). If capabilities are registered by their canonical id, this mapping may simplify but should not disappear (PG rows store the old format).

**Mitigation:** Keep `lens_to_view_id` as a compatibility shim. The `load_view` path still needs it to translate PG-stored lens values to capability ids.

### 5. 🟡 LOW — `inspect_object()` `available_views` field format changes
**Risk:** Currently `inspect_object()` returns `Vec<ViewDescriptor>` with only `id` + `title`. Post-consolidation, the descriptors come from `dyn ViewExecutor` which has richer metadata (`view_kind`, `renderer_kind`). The DTO `ViewDescriptor` may need to carry these fields, or a new DTO may be needed.

**Mitigation:** Extend `ViewDescriptor` DTO with optional `view_kind` and `renderer_kind` fields (backward compatible via `#[serde(default)]`). Or keep the DTO as-is and only use the richer metadata internally.

### 6. 🟢 MINIMAL — `inventory::collect!` type changes
**Risk:** `ProviderWrapper` changes from `&'static dyn ViewDescriptorProvider` to `&'static dyn ViewExecutor`. All `inventory::submit!` calls must be updated.

**Mitigation:** Mechanical change — update 4 `inventory::submit!` calls + add 4 new ones.

---

## Regression Tests Required Before Starting

Before touching any production code, these tests must exist and pass (they are the safety net):

1. **✅ EXISTS** — `available_views_dispatches_per_variant` (service.rs:2789) — verifies listing per type
2. **✅ EXISTS** — `contextual_view_dispatches_file_to_correct_builder` (service.rs:2836) — verifies File execution
3. **✅ EXISTS** — `contextual_view_dispatches_scope_to_correct_builder` (service.rs:2860) — verifies Scope execution
4. **✅ EXISTS** — `contextual_view_rejects_unknown_view_id_per_variant` (service.rs:2884) — verifies error paths
5. **⚠️ MISSING** — No test verifies that `available_views()` and `contextual_view()` agree (i.e., every listed view can actually be built). **This is the critical regression test to add before the migration.**
6. **⚠️ MISSING** — No test for the `evidence` view via `available_views()` (it only appears in `inspect_object()` via `symbol_descriptor_list()`).

---

## Recommendation

**Proceed to sdd-propose.** The five grilled decisions are all feasible. The migration is mechanical but wide — 4 production files, ~12 tests to update, 8 new capability structs to create. The all-at-once approach (Decision 5) is correct because the split-brain cannot be fixed incrementally without creating a temporary fourth source of truth.

**Critical sequencing for the proposal:**
1. Add the missing regression test (#5 above) FIRST — it validates the current behavior and will be the acceptance criterion.
2. Define `ViewDescriptor` + `ViewExecutor` traits in `registry.rs`.
3. Move all `build_*` functions (including `build_evidence_view`) into capabilities.
4. Replace `ProviderWrapper` to hold `dyn ViewExecutor`.
5. Replace `inspect_object()` `available_views` to come from the registry.
6. Replace `contextual_view()` to dispatch via `registry.get(view_id).build(ctx)`.
7. Delete all five `*_descriptor_list()` functions.
8. Update all 6+ tests.

---

## Affected Areas

- `crates/cognicode-explorer/src/service.rs` — delete 5 match-arm methods, 5 descriptor-list fns, 1 evidence-view fn; rewrite `contextual_view()` + `inspect_*()` + `available_views()`
- `crates/cognicode-explorer/src/registry.rs` — replace `ViewDescriptorProvider` with `ViewDescriptor` + `ViewExecutor`; update `ProviderWrapper`, `builtin_providers()`, `ViewRegistry`
- `crates/cognicode-explorer/src/domain/views.rs` — convert 4 providers to `ViewExecutor` impls; add 4 new providers (Evidence, Symbols, Dependencies, Hotspots)
- `crates/cognicode-explorer/src/dto.rs` — potentially extend `ViewDescriptor` with `view_kind` + `renderer_kind`
- `crates/cognicode-explorer/src/api.rs` — no signature changes expected (service methods preserved)
- `crates/cognicode-explorer/src/mcp.rs` — no signature changes expected
- `crates/cognicode-explorer/src/ask/dispatch.rs` — no changes expected (uses service method)
- 6+ test files — update assertions on `available_views` lists + `contextual_view` behavior

---

## Ready for Proposal

**Yes.** The orchestrator should tell the user: the exploration confirms feasibility of all five grilled decisions. The proposal should define the new trait shapes, the capability list (8 capabilities), the `ViewContext`/`InspectionTarget` types, and the migration sequence. The biggest open question for the proposal is whether to extend `ViewDescriptor` DTO with richer metadata or keep it minimal.
