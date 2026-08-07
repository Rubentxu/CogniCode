# Tasks: ViewSpec Authoring V2

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 700–950 |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR 1 → PR 2 → PR 3 |
| Delivery strategy | auto-chain |
| Chain strategy | stacked-to-main |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | JSONata sandbox + live preview | PR 1 | Lazy-loaded worker, 300ms debounce, inline errors |
| 2 | Edit flow + draft persistence | PR 2 | Pre-fill, PUT, per-object localStorage, 20-draft cap |
| 3 | Explorer + Spotter integration | PR 3 | Runtime views in ViewTabs, backend registry merge |

## Phase 1: Foundation

- [x] 1.1 Add `jsonata` dep to `apps/explorer-ui/package.json`
- [x] 1.2 Create `workers/jsonata.worker.ts` with 100ms timeout, 1MB cap, structured error
- [x] 1.3 Create `hooks/useJsonataPreview.ts` with 300ms debounce and race cancellation

## Phase 2: Core Wizard UX

- [x] 2.1 Extract `components/ObjectInspector/TransformStep.tsx` with input/output panels
- [x] 2.2 Wire `TransformStep` into `ViewSpecWizard.tsx` step 4; auto-preview on expression change
- [x] 2.3 Render inline JSONata errors in red beneath the editor
- [x] 2.4 Test worker timeout, 1MB cap, parse error, lazy-load

## Phase 3: Edit & Draft

- [x] 3.1 Create `hooks/useWizardDraft.ts` with per-object localStorage, 1s debounce, 20-draft LRU cap
- [x] 3.2 Add `editSpec?: ViewSpec` prop to `ViewSpecWizard` with full pre-fill and ownership check
- [x] 3.3 Switch Save to `PUT /api/viewspecs/:id` in edit mode, `POST` in create mode
- [x] 3.4 Wire draft restore on open and clear on save/cancel
- [x] 3.5 Test draft save/restore/clear/LRU, wizard edit mode pre-fill

## Phase 4: Explorer + Backend

- [ ] 4.1 Modify `hooks/useViews.ts` to merge runtime specs from `listViewSpecs` with `source: "runtime"`
- [ ] 4.2 Modify `components/ObjectInspector/ViewTabs.tsx` to show runtime views with "custom" badge
- [ ] 4.3 Add ViewSpec schema to `api/schemas.ts`
- [ ] 4.4 Modify `crates/cognicode-explorer/src/api.rs` to index `ViewSpec`s in `POST /api/spotter`
- [ ] 4.5 Modify `crates/cognicode-explorer/src/registry.rs` `list_for` to merge runtime specs after built-ins
- [ ] 4.6 Modify `registry.rs` `get` to resolve runtime ids
- [ ] 4.7 Test backend registry merge, Spotter ViewSpec hits

## Phase 5: Verification

- [ ] 5.1 Verify `jsonata` is absent from main bundle via `vite build` inspect
- [ ] 5.2 Verify all existing `cognicode-explorer` tests pass byte-identical
- [ ] 5.3 E2E: wizard create → save → edit → save flow
