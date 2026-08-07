# Proposal: ViewSpec Authoring V2 — Finishing the Moldable Flow

## Intent

The `moldable-view-runtime-v1` change shipped the foundation: domain vocabulary, backend ViewRegistry, PostgresViewSpecStore, frontend RendererRegistry skeleton, and a 5-step ViewSpecWizard. However, the wizard is a skeleton — JSONata transforms are display-only, there's no live preview, no draft persistence, no edit flow, and no integration with Explorer panes or Spotter/Search. Users can technically create a ViewSpec, but the experience is incomplete and fragile.

This change finishes the user-facing moldable authoring flow so that creating, editing, previewing, and discovering custom views feels like a first-class Explorer capability.

## Scope

### In Scope
- JSONata sandbox: Web Worker with 100ms budget, 1MB input cap, error reporting
- Live preview: auto-preview on step changes, debounced re-render, inline error display
- Draft/auto-save: persist wizard state to localStorage, restore on reopen
- Edit flow: pre-fill wizard from saved ViewSpec, PUT on save
- Explorer pane integration: saved ViewSpecs appear in ViewTabs, open in new pane
- Spotter/Search: saved ViewSpecs indexed and searchable

### Out of Scope
- JSONata Rust-side execution (backend just stores expression)
- Bulk ViewBlock migration / richer renderer wiring beyond current set
- ViewSpec versioning/history
- Template gallery / curated starters
- Live-collaborative authoring
- Remote/plugin renderers

## Capabilities

### New Capabilities
- `viewspec-jsonata-sandbox`: Client-side JSONata execution in a sandboxed Web Worker with timeout budget, input size cap, and structured error reporting
- `viewspec-draft-persistence`: Auto-save wizard state to localStorage; restore on reopen; clear on explicit save/cancel

### Modified Capabilities
- `viewspec-authoring-flow`: Adding live preview (auto-trigger on step changes), edit-existing-ViewSpec pre-fill, and wizard UX polish
- `renderer-registry-frontend`: Saved runtime ViewSpecs must appear in ViewTabs alongside built-in views; RendererRegistry lookup must handle runtime `renderer_kind` values

## Approach

### Slice 1 (first reviewable PR — visible user value fast)
1. **JSONata Web Worker** (`apps/explorer-ui/src/workers/jsonata.worker.ts`)
   - Load `jsonata` npm package in a dedicated Web Worker
   - 100ms evaluation timeout via `setTimeout` + `worker.terminate()`
   - 1MB input size check before sending to worker
   - Structured result: `{ ok, output?, error?, duration_ms }`

2. **Live preview hook** (`apps/explorer-ui/src/hooks/useJsonataPreview.ts`)
   - Debounced (300ms) auto-execution when expression or input changes
   - Returns `{ output, error, loading }`
   - Integrates with the TransformStep component

3. **Wizard UX polish**
   - Auto-preview on step 4 (Transform) when expression is non-empty
   - Inline JSONata error with line/column from parser
   - Preview panel shows input/output side-by-side in TransformStep

### Slice 2 (edit flow + persistence)
4. **Edit flow** (`ViewSpecWizard` prop: `editSpec?: ViewSpec`)
   - Pre-fills all wizard fields from existing ViewSpec
   - Save calls `PUT /api/viewspecs/:id` instead of `POST`
   - Ownership check: edit button hidden if `owner !== current_user`

5. **Draft persistence** (`apps/explorer-ui/src/hooks/useWizardDraft.ts`)
   - `localStorage` key: `viewspec-draft-{objectId}`
   - Auto-save on every wizard state change (debounced 1s)
   - Restore on wizard open if draft exists
   - Clear on explicit save or cancel

### Slice 3 (Explorer + Spotter integration)
6. **Explorer pane integration**
   - `useAvailableViews` merges runtime ViewSpecs from `listViewSpecs` API
   - Runtime views appear in ViewTabs with a "custom" badge
   - Clicking a runtime view calls `executeViewSpec` and renders via RendererRegistry

7. **Spotter/Search integration**
   - `POST /api/spotter` index includes saved ViewSpecs
   - Search results include ViewSpec matches with `kind: "viewspec"`
   - Clicking a ViewSpec result opens it in the Explorer

## Entropy Budget

**Method**: Heuristic (CogniCode not available for this analysis)

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) | ~1.5 | < 1.0 | ⚠️ MODERATE |
| H(Δ_new) | ~3.0 | > 0 | ✅ |
| New connascence pairs | ~4 | < 3 | ⚠️ |
| OCP compliant? | Mostly | yes | ⚠️ |

**Breaking Change Indicators**:
- H(Δ_existing) ≈ 1.5 bits: existing ViewTabs, TransformStep, and useAvailableViews must change
- New connascence: Worker ↔ main thread message protocol (Position connascence), localStorage key naming (Name connascence)
- KL ≈ 0 for existing ViewSpec DTO — no subtype behavior changes

**Verdict**: 🟡 YELLOW — moderate coupling introduction, but all changes are additive extensions to existing seams. No circular dependencies introduced.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `apps/explorer-ui/src/workers/jsonata.worker.ts` | New | Web Worker for sandboxed JSONata execution |
| `apps/explorer-ui/src/hooks/useJsonataPreview.ts` | New | Debounced JSONata preview hook |
| `apps/explorer-ui/src/hooks/useWizardDraft.ts` | New | localStorage draft persistence hook |
| `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx` | Modified | Live preview, edit mode, draft restore |
| `apps/explorer-ui/src/components/ObjectInspector/TransformStep.tsx` | Modified | Inline JSONata preview with input/output |
| `apps/explorer-ui/src/hooks/useViews.ts` | Modified | Merge runtime ViewSpecs into available views |
| `apps/explorer-ui/src/components/ObjectInspector/ViewTabs.tsx` | Modified | Show runtime views with custom badge |
| `apps/explorer-ui/src/api/schemas.ts` | Modified | Add viewspotter result schema |
| `apps/explorer-ui/package.json` | Modified | Add `jsonata` npm dependency |
| `crates/cognicode-explorer/src/api.rs` | Modified | Spotter endpoint includes ViewSpec results |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| JSONata npm package size bloat (~150KB gzipped) | Medium | Lazy-load worker; only import when TransformStep mounts |
| Web Worker termination leaves orphan processes | Low | Use `worker.terminate()` in cleanup; test in CI |
| localStorage quota exceeded on many drafts | Low | Cap at 20 drafts; evict oldest on overflow |
| Race condition: auto-preview fires while previous is still running | Medium | Cancel previous worker instance before starting new one |
| Edit flow ownership check bypass via API | Low | Backend already scopes by `(workspace_id, owner)` |

## Rollback Plan

1. **Slice 1 rollback**: Remove `jsonata.worker.ts`, `useJsonataPreview.ts`, and revert TransformStep to display-only. The wizard keeps working without JSONata preview.
2. **Slice 2 rollback**: Remove `editSpec` prop and `useWizardDraft.ts`. Wizard reverts to create-only mode.
3. **Slice 3 rollback**: Revert `useAvailableViews` to not merge runtime specs. ViewTabs hides runtime views. Spotter endpoint unchanged.
4. **Full rollback**: Delete `openspec/changes/viewspec-authoring-v2/`. No database migrations required — all changes are frontend-only except Spotter indexing.

## Dependencies

- `jsonata` npm package (MIT license, ~150KB gzipped)
- Existing `executeViewSpec` API endpoint (already implemented)
- Existing `listViewSpecs` API endpoint (already implemented)
- PostgresViewSpecStore (already implemented in Phase 2)

## Success Criteria

- [ ] JSONata expressions execute in Web Worker with 100ms timeout; violations surface as wizard errors
- [ ] Live preview auto-triggers on step changes; debounced to avoid hammering backend
- [ ] Edit flow: clicking "Edit" on a saved ViewSpec opens wizard pre-filled; save calls PUT
- [ ] Draft persistence: closing wizard mid-flow restores state on reopen
- [ ] Runtime ViewSpecs appear in ViewTabs alongside built-in views
- [ ] Spotter search returns ViewSpec matches
- [ ] All existing tests pass; new tests for Worker, hooks, and edit flow
