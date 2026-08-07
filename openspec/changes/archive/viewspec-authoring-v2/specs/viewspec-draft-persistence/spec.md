# ViewSpec Draft Persistence Specification

## Purpose

Define the per-object auto-save and restore behaviour for the `ViewSpecWizard` so that closing the wizard mid-flow does not lose a user's work, and resuming authoring from the same inspected object re-opens the wizard with the same in-progress state.

## Requirements

### Requirement: 1. Per-object localStorage key

Drafts MUST be stored under `viewspec-draft-{objectId}` where `{objectId}` is the canonical id of the focused object (e.g. `symbol:src/foo.rs:bar:42`). The `viewspec-draft-` prefix prevents collision with other explorer features sharing the same `localStorage` origin.

#### Scenario: Draft for symbol A does not collide with B

- GIVEN the user is authoring a view for symbol `A` and has typed a title
- WHEN they navigate to symbol `B` and open the wizard
- THEN the wizard for `B` does NOT show `A`'s title
- AND no key is read or written under the wrong object

#### Scenario: Key is namespaced

- GIVEN the user has a draft
- WHEN `localStorage.keys()` is inspected
- THEN every draft key starts with `viewspec-draft-`

### Requirement: 2. Debounced auto-save

The hook MUST auto-save the wizard state 1 000 ms after the last state change. The state MUST include every field (step index, view_kind, renderer_kind, title, applies_to, data_source, transform, props).

#### Scenario: Typing triggers auto-save after debounce

- GIVEN the user types into the title field at `t=0`, `t=400ms`, `t=800ms`
- WHEN the debounce window opens at `t=1800ms`
- THEN exactly one `setItem` call occurs with the final state

#### Scenario: Save skipped when state is empty

- GIVEN the user opens the wizard but changes no field
- WHEN the debounce window opens
- THEN no `setItem` call is made (the prior draft is preserved)

### Requirement: 3. Restore on wizard open

When the wizard opens for an object with a stored draft, the hook MUST hydrate the state from `localStorage` before first paint. The restore MUST be silent — no toast, no modal.

#### Scenario: Closing and reopening restores the draft

- GIVEN the user typed `My Hot Symbols` into a Symbol's wizard, then closed the drawer
- WHEN they reopen the wizard for the same Symbol
- THEN the title field shows `My Hot Symbols` on first render

#### Scenario: No draft means a fresh wizard

- GIVEN the user opens the wizard for a Symbol with no stored draft
- WHEN the wizard renders
- THEN the state is the default initial state

### Requirement: 4. Clear on explicit save or cancel

`useWizardDraft.clear()` MUST remove the `viewspec-draft-{objectId}` key. The wizard MUST call it on the success path of `POST /api/viewspecs`, on the success path of `PUT /api/viewspecs/:id` (edit mode), and on the user's explicit Cancel action.

#### Scenario: Successful save clears the draft

- GIVEN a draft exists for object `X`
- WHEN the user clicks Save and the POST returns 200
- THEN `viewspec-draft-X` is removed
- AND reopening the wizard shows a fresh state

#### Scenario: Cancel clears the draft

- GIVEN a draft exists for object `X`
- WHEN the user clicks Cancel
- THEN `viewspec-draft-X` is removed

#### Scenario: Edit mode never persists a draft

- GIVEN the wizard opens with `editSpec={id: "V", title: "Hot Symbols", ...}`
- WHEN the user clicks Save and PUT returns 200
- THEN `useWizardDraft.clear()` is invoked and no draft is written

### Requirement: 5. 20-draft cap with LRU eviction

The hook MUST enforce a soft cap of 20 draft keys. When a write would push the count above 20, the hook MUST delete the oldest draft by `updated_at` and proceed.

#### Scenario: 21st draft evicts the oldest

- GIVEN 20 draft keys exist, the oldest is for `A`
- WHEN a 21st draft is written for `Z`
- THEN `viewspec-draft-A` is removed
- AND the total count is exactly 20

## Out of Scope

- Cross-device draft sync (no server-side draft storage in v1)
- Conflicts between an in-flight draft and a freshly-saved spec (last-write-wins)

## Coverage

- **Happy paths**: covered (per-object key, debounced save, restore, clear, cap)
- **Edge cases**: covered (no-draft fresh state, edit mode bypass, cancel clear)
- **Error states**: covered (localStorage quota overflow → evict oldest, JSON parse failure on a corrupt draft → ignore and start fresh)
