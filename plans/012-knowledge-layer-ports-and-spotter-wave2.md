# Plan 012: Establish the knowledge layer and ship universal Spotter wave 2

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the STOP conditions section occurs, stop and
> report — do not improvise.
>
> **Drift check (run first)**: `git diff --stat a130d53b..HEAD -- \
> crates/cognicode-explorer/src apps/explorer-ui/src docs/ROADMAP.md docs/adr`

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `a130d53b`, 2026-07-22

## Why this matters

CogniCode already explores code well, but it still lacks first-class discovery
for docs, ADRs, and evidence. That keeps Explorer from behaving like a real
knowledge environment. This plan unblocks the highest-leverage foundation for
architecture understanding and all later decision-support features.

## Current state

- `docs/ROADMAP.md:301-302` marks `e13-wave2-universal-spotter` blocked by
  missing `DocRepository`, ADR index, and evidence store ports.
- `crates/cognicode-explorer/src/lib.rs` already has Spotter, views, MCP,
  investigations, and affordances wired as first-class concepts.
- `apps/explorer-ui/src/components/Spotter.tsx` and
  `apps/explorer-ui/src/hooks/useSpotter.ts` provide the visible discovery UX.
- `docs/adr/ADR-009-knowledge-layer-ports-and-universal-spotter.md` defines the
  product contract: backend port + UI discoverability + inspectability.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cargo test -p cognicode-explorer --lib` | exit 0 |
| Rust check | `cargo check -p cognicode-explorer` | exit 0 |
| UI unit tests | `npm --prefix apps/explorer-ui test -- Spotter` | exit 0 |
| UI build | `npm --prefix apps/explorer-ui build` | exit 0 |

## Scope

**In scope**:
- `crates/cognicode-explorer/src/ports/*`
- `crates/cognicode-explorer/src/facades/search.rs`
- `crates/cognicode-explorer/src/mcp/handler/search.rs`
- `apps/explorer-ui/src/components/Spotter.tsx`
- `apps/explorer-ui/src/hooks/useSpotter.ts`
- `apps/explorer-ui/src/api/schemas.ts`

**Out of scope**:
- diagram export logic
- ProjectDiary / ExampleObject runtime
- architecture decision support pack composition

## Steps

### Step 1: Introduce typed knowledge ports

Add `DocRepository`, ADR index access, and `EvidenceStore` ports to Explorer's
seams. Keep interfaces read-focused and inspectable-object oriented.

**Verify**: `cargo check -p cognicode-explorer` → exit 0.

### Step 2: Extend Spotter families and DTOs

Add `doc`, `adr`, and `evidence` families to search results, DTO schemas, and
frontend discriminated unions.

**Verify**: `cargo test -p cognicode-explorer --lib` → exit 0.

### Step 3: Expose visible Explorer entry paths

Update Spotter UI and result actions so a user can open each family into a
pane and see at least one default useful view.

**Verify**: `npm --prefix apps/explorer-ui test -- Spotter` → exit 0.

### Step 4: Add interaction validation

Add at least one UI interaction test proving the happy path for doc/ADR/evidence
discovery and pane opening.

**Verify**: `npm --prefix apps/explorer-ui build` → exit 0.

## Test plan

- Add Rust tests for family dispatch and DTO serialization.
- Add frontend tests around Spotter family rendering and selection.
- Model after existing Spotter tests in `apps/explorer-ui/src/components/Spotter.test.tsx`.

## Done criteria

- [ ] `DocRepository`, ADR index, and evidence seams exist
- [ ] Spotter returns `doc`, `adr`, and `evidence` families
- [ ] Explorer UI can open those results into panes
- [ ] Rust and UI verification commands exit 0

## STOP conditions

- Search results cannot carry stable identity for the new families
- The existing pane model cannot render one of the object families without a
  broader inspector refactor
- The port design requires mutating unrelated investigation or graph contracts

## Maintenance notes

- This plan is the dependency root for ConceptMap, DocCodeAlignment, and richer
  DecisionGraph/Decision support work.
