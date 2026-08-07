# Design: ViewSpec Authoring V2 — Slice 1

## Technical Approach

Add a client-side JSONata sandbox, live preview, and edit/draft flow to the existing ViewSpecWizard while keeping all current behavior intact. The backend already exposes full CRUD and execute endpoints; this slice is purely frontend work.

- **JSONata sandbox**: Dedicated Web Worker with a 100ms timeout and 1MB input cap.
- **Live preview**: Debounced (300ms) auto-execution in the Transform step so users see input/output side-by-side.
- **Edit flow**: `editSpec` prop pre-fills the wizard; save switches to `PUT /api/viewspecs/:id`.
- **Draft persistence**: Auto-save to `localStorage` per inspected object, restore on reopen, clear on explicit save/cancel.

## Architecture Decisions

| Decision | Option | Tradeoff | Choice |
|----------|--------|----------|--------|
| JSONata execution | Web Worker + `terminate()` | ~150KB gzipped, lazy-loaded | Worker — prevents main-thread blocking and enables safe timeout |
| Preview trigger | 300ms debounce | Slight delay vs. hammering | 300ms — aligns with 100ms execution budget |
| Draft scoping | Per-object key `viewspec-draft-{objectId}` | 20-draft cap, evict oldest | Per-object — matches accepted auto-grill recommendation |
| Edit mode trigger | `editSpec` prop on wizard | Parent must supply spec | Minimal API change; wizard stays self-contained |
| TransformStep | Extract to own file | Extra import vs. monolith | Extract — reduces ViewSpecWizard size and matches proposal file list |

## Data Flow

```
User types JSONata expression
  ↓ 300ms debounce
useJsonataPreview
  ↓ 1MB cap check
jsonata.worker.ts
  ↓ 100ms timeout
{ok, output?, error?, duration_ms}
  ↓
TransformStep renders input/output side-by-side
```

## Sequence Diagram — Wizard Save / Edit

```
User clicks Save
  ViewSpecWizard.runSave()
    if editSpec → apiPut /viewspecs/:id
    else      → apiPost /viewspecs
  onSaved callback
  useWizardDraft.clear()
  onClose
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `apps/explorer-ui/src/workers/jsonata.worker.ts` | Create | Sandboxed JSONata execution with timeout and size guard |
| `apps/explorer-ui/src/hooks/useJsonataPreview.ts` | Create | Debounced hook wrapping the worker; handles race cancellation |
| `apps/explorer-ui/src/hooks/useWizardDraft.ts` | Create | localStorage auto-save/restore per object; clear on save/cancel |
| `apps/explorer-ui/src/components/ObjectInspector/TransformStep.tsx` | Create | Extracted from wizard; inline JSONata preview with input/output panels |
| `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx` | Modify | Add `editSpec` prop, draft restore, auto-preview trigger on step 4 |
| `apps/explorer-ui/package.json` | Modify | Add `jsonata` dependency |

## Interfaces / Contracts

```typescript
// jsonata.worker.ts
interface JsonataRequest {
  expression: string;
  input: unknown;
}
interface JsonataResponse {
  ok: boolean;
  output?: unknown;
  error?: string;
  duration_ms: number;
}

// useJsonataPreview.ts
function useJsonataPreview(
  input: unknown,
  expression: string | null,
): { output: unknown | null; error: string | null; loading: boolean };

// useWizardDraft.ts
function useWizardDraft(
  objectId: string,
  editSpec?: ViewSpec,
): { state: WizardState; dispatch: WizardDispatch; clear: () => void };

// ViewSpecWizard.tsx
interface ViewSpecWizardProps {
  // ... existing props ...
  editSpec?: ViewSpec; // pre-fills all fields; save calls PUT
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Worker timeout, error handling, 1MB cap | Vitest with `msw` + inline worker mock |
| Unit | Hook debounce and race cancellation | `@testing-library/react` + fake timers |
| Unit | Draft save/restore/clear | `localStorage` mock + `renderHook` |
| Unit | Wizard edit mode pre-fill | Component test with `editSpec` prop |
| Integration | Full wizard create → save → edit → save | Playwright E2E |

## Migration / Rollout

No migration required. All changes are frontend-only and additive. No database or backend changes for this slice.

## Open Questions

- None
