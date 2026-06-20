# Kernel Exploration: C4 Architecture Drift Detection

**Date:** 2026-06-20
**Triggered by:** User question — compare E6's inferred C4 (from code) against
documented architecture (ADRs, CONTEXT.md) and surface divergences as a
drift report.
**Context level:** C2 (E6 is shipped; ADR-039 §8 is the spec; existing
detection machinery is limited but present).

---

## 1. Current State

### 1.1 E6 ships inferred C4 — that side is done

`GraphServiceImpl::build_architecture(root_path)`
(`crates/cognicode-explorer/src/facades/graph.rs:201-258`) parses
`Cargo.toml` (workspace members + per-crate `[lib]`/`[[bin]]`) and
`apps/*/package.json` to synthesise a `SubgraphResponse` with C1
(system node), C2 (containers), C3 (components = modules), and a
capped C4 layer (≤200 symbols).

Endpoint: `GET /api/workspaces/:workspace_id/architecture`
(`crates/cognicode-explorer/src/api.rs:669-684`). The response is a
flat `SubgraphResponse { nodes, edges }` with
`style_class ∈ { node-system, node-container, node-component, node-code }`.

Tests: `build_architecture_*` (5 tests in `graph.rs:480-678`) cover
system node, containers, JS apps, components, 200-cap, `part_of`
edges. Solid.

### 1.2 The `architecture_drift` ViewKind is a placeholder

- Declared in `ViewKind` enum (`crates/cognicode-explorer/src/dto.rs:1122`).
- Wired through serde (`dto.rs:1172`, `dto.rs:1221`).
- Listed in the known-kinds allow-list
  (`crates/cognicode-explorer/src/registry.rs:479`).
- Frontend references it: `apps/explorer-ui/src/api/schemas.ts:610`,
  `apps/explorer-ui/src/api/schemas.test.ts:529`,
  `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx:139`
  (UI label "Architecture Drift" + default `renderer_kind: "table"`).
- **No `ViewDescriptorProvider` / `ViewExecutor` for it.** Same gap as
  `ArchitectureRationale`, `BoundaryMap`, `RiskMap`, etc. — the catalog
  entry is reserved but the implementation is missing.

### 1.3 The existing `detect_drift` MCP tool is a DIFFERENT thing

`handle_detect_drift` (`crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs:2005`)
analyses **a single source file** for intent drift / AVC violations /
obsolete patterns / forbidden terms (rule IDs S7000-S7003). Tree-sitter
walks function nodes; findings have `function_name`, `drift_score`,
`rule_id`, `severity`, `line`, `message`. 16+ unit tests in
`mcp_roundtrip_tests.rs:3789-4502`.

**This is per-file code-level drift, NOT architecture-level drift.**
The naming collision is unfortunate; they are independent concepts.

### 1.4 The closest existing concept is `ArchitectureLens`

`crates/cognicode-explorer/src/domain/lenses/architecture.rs:26`
(`ArchitectureLens`) — applies to `Symbol`/`File`/`Scope`. Detects:
- **Dependency cycles between scopes** (Critical finding).
- **God modules** with `> 20` foreign incoming scopes (Warning).
- Per-symbol "boundary touch" findings (Info).

Pattern: walks `SymbolRepository` + `GraphQueryPort`, emits
`DesignFinding`s with `severity` and `confidence`. Already has 5 unit
tests proving the cycle detection works on mocked repos. **However:
this is scope-level call-graph analysis, not C4-vs-ADR comparison.**
It does not know about ADR boundaries or documented expectations.

### 1.5 `check_boundaries` is a STUB

`cognicode-axiom/src/mcp/tools.rs:340` (`handle_check_boundaries`)
takes a `boundaries_config` of `{name, path_patterns,
allowed_dependencies}` per boundary, but the actual `BoundaryChecker`
call is replaced with a placeholder returning `"Boundary checking
requires CallGraph build from cognicode-core"`. The DTOs and default
DDD templates (`BoundaryDefinition`, `BoundaryChecker::with_ddd_defaults`)
exist; the wiring is incomplete.

### 1.6 ADR-039 §8 does NOT mention drift detection

Section 8 (`docs/adr/ADR-039-explorer-navigation-model.md:87-97`)
defines the inference rules for C4 — that's it. The phrase
"architecture_drift" appears only in:
- `CONTEXT.md:143` (catalog entry).
- `docs/adr/ADR-008-moldable-view-runtime.md:97` (catalog entry).

Both are descriptions of what the view should show, not the spec for
how to implement it. **No ADR yet covers drift detection semantics,
data sources, or comparison algorithm.**

### 1.7 No expected/declared architecture data structure exists

Searched: `expected_architecture`, `declared_boundaries`,
`intended_boundaries`, `adr_dependencies`, `allowed_dependencies`
(w/ context), `forbidden_dependencies`. The only `allowed_dependencies`
field that exists is on the stub `check_boundaries` config DTO. **No
persisted, queryable "documented architecture"** — neither in PG, nor
in a JSON file, nor in any parsed-and-stored form of ADRs.

ADRs live in `docs/adr/ADR-*.md` as Markdown. They are NOT parsed
into the graph. The only ADR-derived data the system has is via the
`Decision`/`Doc`/`Evidence` graph nodes (see `dto.rs:440` for
`DecisionArtifactSummary`), and that pipeline ingests them as
references, not as boundary rules.

### 1.8 What does the user actually want?

A drift report with three layers of signal, in increasing order of
effort:

1. **Structural drift** — does inferred C2/C3 match what ADR-039 §8 /
   CONTEXT.md describes? (E.g., CONTEXT.md says "the composition root
   is `cognicode-runtime`"; inferred graph must show a `cognicode-runtime`
   container.)
2. **Boundary drift** — does inferred edge traffic cross boundaries
   that ADRs forbid? (Uses `ArchitectureLens`-style call-graph walks
   against an ADR-derived rule list.)
3. **Document drift** — do ADRs/ADRs-cited claims no longer match the
   code? (E.g., "all ports are async-trait" — inferred: `McpHandler`
   is `async fn` in 95% of crates, but 2 crates use `async fn`
   directly without `async_trait`.)

---

## 2. Context Quality

- **Level:** C2 — E6 ships, ArchitectureLens works, all the building
  blocks are visible; the work is composition + a comparison spec.
- **Evidence Present:**
  - `build_architecture()` (graph.rs:201) with full test coverage.
  - `ArchitectureLens` (architecture.rs:26) with 5 unit tests on
    cycle detection.
  - `architecture_drift` ViewKind enum + frontend UI string
    (`ViewSpecWizard.tsx:139,199`).
  - `DecisionArtifactSummary` DTO + `Decision` graph node type
    (suggesting the persistence layer for ADR-derived data exists).
  - CONTEXT.md + ADR-008 catalog definition for the view.
  - `cognicode-diagram/PLAN-FASE4.md` (docs-only plan, no code) —
    sketches container-level boundary validation.
- **Missing Context:**
  - **No parser for ADR markdown** → boundary rules. (PLAN-IMPLEMENTACION
    T15 mentions `boundary.rs` but it lives in `docs/plan/cognicode-axioms/`
    — docs-only, not the `cognicode-axiom` crate which is a different
    concern.)
  - **No agreed vocabulary** for what "drift" means at the C4 level
    (missing container? extra container? boundary crossing? scope
    touching > N foreign containers?).
  - **No persistence story** — does drift produce a `DesignFinding`
    (lens-style) or a top-level report (DTO-style)?
- **Recommended Effort:** **Deepen** — verify ADR-parser feasibility +
  decide comparison semantics before proposal.

---

## 3. Knowledge Coverage

| Class | Status | Evidence | Gap Impact |
|------|--------|----------|------------|
| Roadmap/Backlog | partial | CONTEXT.md:143 + ADR-008:97 catalog the view; no ADR on implementation | Medium — vision clear, no in-flight ticket |
| Work Items | missing | No `sddk/proposals/*drift*` file; no ticket | Medium — must spin up new proposal |
| Architecture/ADRs | partial | ADR-039 §8 covers inference; **no ADR on comparison algorithm** | High — blocks the "what to compare" decision |
| Ownership | missing | No `CODEOWNERS` entry for `cognicode-explorer::facades::graph` or `::domain::lenses::architecture` | Medium — blocks escalation if questions arise |
| Learnings | partial | engram has 1 obs on E6 (`sddk/explorations/c4-backend-inference.md`); no learning on ArchitectureLens | Low |

---

## 4. Problem Taxonomy

| Axis | Applies | Evidence |
|------|---------|----------|
| Domain modeling | **Yes** | The drift concept has no agreed shape. Is it `{missing_container, extra_container, boundary_crossing}` per ADR? Or `{expected, actual, diff}`? Need to pin down before coding. |
| Boundary/seam | **Yes** | Drift logic could live in (a) `cognicode-explorer::facades::graph` (next to `build_architecture`), (b) `cognicode-explorer::domain::lenses::architecture` (next to `ArchitectureLens`), (c) a new `cognicode-explorer::domain::drift` module. ADR needed. |
| Coupling/connascence | **Yes** | Drift needs `root_path: PathBuf` (to re-parse `Cargo.toml`/`package.json` for `actual`), and a parsed ADR boundary list (to get `expected`). Both new dependencies. |
| API contract | **Yes** | New DTO `DriftReport { findings: Vec<DriftFinding>, summary, ... }` and new endpoint `GET /api/workspaces/:id/drift` (or a new MCP tool `detect_architecture_drift`). Need to decide shape. |
| Refactor/legacy | **No** | This is additive; nothing to refactor. |
| Event/CQRS | **No** | Drift is a read-time diff, not an event-sourced projection. |
| Testing | **Yes** | Existing E6 tests cover `build_architecture`; new tests must cover: matching architecture (zero drift), missing container, extra container, boundary crossing. Use `MockRepo` pattern from `architecture.rs:342-547`. |
| Security/operations | **No** | Drift reads public metadata + ADRs; no PII, no auth path. |

---

## 5. Domain Language And Invariants

### 5.1 Domain Language

| Term | Resolved meaning | Unresolved ambiguity |
|---|---|---|
| **Drift** | A divergence between documented architecture and inferred architecture | Need to pick: scope (container? component? edge?), severity (info/warning/critical), and what counts as "documented" (ADRs only? CONTEXT.md? both?) |
| **Expected boundary** | A rule derived from an ADR or CONTEXT.md prose: "container X may depend on Y" | No parser today. Either (a) hand-curated YAML rules (like `check_boundaries` config), or (b) parsed from ADR markdown front-matter, or (c) implicit from CONTEXT.md prose via LLM |
| **Inferred boundary** | An edge between two inferred C2 containers that exists in the call graph | Source: `GraphQueryPort` call edges, mapped to C2 via `part_of` |
| **DriftFinding** | `{ kind, expected, actual, severity, evidence }` | Could reuse `DesignFinding` (lens style) or invent `DriftFinding` (DTO style) |

### 5.2 Invariants

1. Drift detection must NOT modify the graph or any persisted state.
2. Drift findings must include provenance: which ADR / CONTEXT.md
   section / file was the "expected" derived from.
3. Drift findings must include the inferred counterpart: which node
   or edge is the "actual" divergence.
4. Drift endpoint must degrade gracefully: if `Cargo.toml` parsing
   fails, return the ADRs-derived drift only (skip the structural
   comparison) — never 500.
5. Drift must be re-computable on demand (no stale cache required
   for v1). `build_architecture()` already has no cache, so this is
   free.

---

## 6. Knowledge Gaps

- **ADR-parser feasibility** — parsing `docs/adr/ADR-NNN-*.md` for
  boundary rules requires either (a) front-matter `boundaries:` YAML,
  or (b) regex over prose. Front-matter is cleaner but requires
  changing every ADR. Prose is free but unreliable.
- **Comparison algorithm** — set-difference on container names?
  Subgraph isomorphism? Heuristic name match (e.g.,
  `cognicode-runtime` ↔ `composition_root`)? Need a concrete answer.
- **Severity scale** — when is drift "info" (cosmetic), "warning"
  (likely unintentional), "critical" (clear architectural violation)?
  Could mirror `ArchitectureLens` (Critical = cycle, Warning = god
  module, Info = boundary touch).
- **UI shape** — frontend says default `renderer_kind: "table"`. Is
  a table the right rendering? Or `graph + table` so the user can
  see the inferred-vs-expected diff visually?
- **MCP tool name** — `detect_drift` is taken (per-file). Need a
  distinct name. Candidates: `detect_architecture_drift`,
  `compare_architecture`, `architecture_drift_report`.

---

## 7. Affected Areas

| File | Why |
|------|-----|
| `crates/cognicode-explorer/src/dto.rs` | Add `DriftReport`, `DriftFinding`, `DriftKind` DTOs (next to `DecisionArtifactSummary`) |
| `crates/cognicode-explorer/src/facades/graph.rs` | Add `compare_architecture(root_path, expected) -> DriftReport` next to `build_architecture()` |
| `crates/cognicode-explorer/src/facades/mod.rs` | Extend `GraphService` trait with `compare_architecture(...)` |
| `crates/cognicode-explorer/src/api.rs` | Add `GET /api/workspaces/:id/drift` handler (next to `architecture_handler:669`) |
| `crates/cognicode-explorer/src/domain/lenses/architecture.rs` | **MAYBE** extend `ArchitectureLens` with boundary-crossing findings against an ADR-derived rule list |
| `crates/cognicode-explorer/src/domain/views.rs` | **MAYBE** add `ArchitectureDriftProvider` + `ArchitectureDriftExecutor` to fill the catalog gap (parallel to `OverviewProvider:1260`, `OverviewExecutor:1328`) |
| `crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs` | **MAYBE** add `detect_architecture_drift` MCP tool (parallel to `handle_detect_drift:2005`) |
| `crates/cognicode-core/src/interface/mcp/schemas.rs` | Add `DetectArchitectureDriftInput`/`Output` schemas (parallel to `DetectDriftInput:1857`) |
| `docs/adr/ADR-039-explorer-navigation-model.md` | **MAYBE** add §10 "Architecture drift detection" once approach is decided |
| `CONTEXT.md:143` | No change — catalog entry already accurate |

---

## 8. Options

| Option | Pros | Cons | Effort |
|--------|------|------|--------|
| **A. Structural-only diff** — compare `build_architecture()` output against a hand-curated `expected_containers` YAML. Report: missing/extra containers, no edge-level checks. | (1) Trivial to ship (1–2 days); (2) reuses `build_architecture()` end-to-end; (3) no ADR parser needed; (4) tests are easy (`MockRepo` mocks the inferred set) | (1) Requires manual YAML curation; (2) misses edge-level boundary drift; (3) won't satisfy "diverges from ADRs" wording | **S (1–2 days)** |
| **B. Structural + boundary-crossing** — A + extend `ArchitectureLens` with rule-based boundary checks (rule list from YAML). Report: structural + per-edge boundary violations. | (1) Covers CONTEXT.md "documented boundaries"; (2) reuses `ArchitectureLens` infrastructure; (3) rule list is decoupled from ADR parsing | (1) Still requires hand-curated rules; (2) 2–3 days; (3) does not parse ADR prose | **M (2–3 days)** |
| **C. ADR-parser + structural + edge diff** — B + a Markdown parser that extracts boundary claims from ADR front-matter / fenced YAML blocks. Report: provenance per finding. | (1) Honest "ADRs as source of truth"; (2) no manual curation; (3) provenance is real | (1) New ADR format requirement OR fragile prose parsing; (2) 4–6 days; (3) tests need to ship sample ADRs | **L (4–6 days)** |

**Recommended:** **A** for v1, with **B** queued as a follow-up. A
ships in one PR, gives the user a real drift report, and creates the
DTO and endpoint infrastructure that B/C will reuse. The hand-curated
YAML becomes the v1 "expected architecture" file; ADR-parser work
becomes a separate change when the format is agreed.

---

## 9. Entropy Envelope

- **Method:** heuristic (no CogniCode graph to query — we are adding
  read-only tooling on top of E6, not changing call-graph topology).
- **Coupling risk:** **low** — additive changes; `build_architecture`
  is untouched; new code lives in a sibling function.
- **OCP risk:** **low** — extending the `GraphService` trait with one
  new method is a single point of change; mock impls in
  `api_graph_tests.rs:92,385` and `api_rationale_tests.rs:183` need
  one new `async fn` each (~3 lines).
- **Connascence:** **low** — new `DriftReport` and `DriftFinding`
  DTOs are independent of existing types; no shared mutable state.
- **Naming collision risk:** **medium** — `detect_drift` is taken for
  per-file code drift. The MCP tool for architecture drift MUST use a
  distinct name (`detect_architecture_drift` or
  `architecture_drift_report`). Document this prominently to avoid
  future LLM-confusion.

---

## 10. Recommendation

**Ship Option A (structural-only diff) as the v1 architecture drift
detection.** Concretely:

1. **New DTO** in `cognicode-explorer/src/dto.rs`:
   - `DriftFinding { kind: DriftKind, expected: Option<String>, actual: Option<String>, severity: Severity, evidence: Vec<String> }`.
   - `DriftKind` enum: `MissingContainer`, `ExtraContainer`, `WrongSubKind`.
   - `DriftReport { findings: Vec<DriftFinding>, summary: String }`.

2. **New file** `expected-architecture.yaml` at the workspace root:
   ```yaml
   containers:
     - { name: cognicode-core, sub_kind: library, purpose: domain primitives }
     - { name: cognicode-runtime, sub_kind: library, purpose: composition root }
     - { name: cognicode-explorer, sub_kind: library, purpose: explorer facade }
     - { name: cognicode-cli, sub_kind: binary, purpose: CLI entry }
     - { name: cognicode-mcp, sub_kind: binary, purpose: MCP server }
     - { name: cognicode-axiom, sub_kind: library, purpose: quality rules }
     - { name: cognicode-quality, sub_kind: library, purpose: quality crates }
   ```
   (Mirrors the workspace `Cargo.toml` `members`. One per workspace.)

3. **New method** `GraphService::compare_architecture(root_path,
   expected_path) -> ExplorerResult<DriftReport>`:
   - Reuse `build_architecture(root_path)` to get `actual`.
   - Parse `expected_path` as YAML (add `serde_yaml` — already a
     transitive dep of `cognicode-core` per E6 exploration, line 83).
   - Set-diff container names → `MissingContainer` / `ExtraContainer`.
   - Set-diff sub_kind → `WrongSubKind`.
   - Return `DriftReport`.

4. **New endpoint** `GET /api/workspaces/:id/drift` that calls
   `compare_architecture(root, expected_yaml_path)`. The
   `expected_yaml_path` defaults to `<root>/.cognicode/expected-architecture.yaml`.

5. **New ViewExecutor** `ArchitectureDriftExecutor` in
   `cognicode-explorer/src/domain/views.rs` that loads the
   `DriftReport` for the workspace scope and renders it as
   `renderer_kind = "table"` (matches the frontend default at
   `ViewSpecWizard.tsx:199`).

6. **MCP tool** (parallel to `handle_detect_drift`): add
   `detect_architecture_drift` in `aix_handlers.rs` that calls
   `compare_architecture` and serialises the `DriftReport`. Distinct
   from `detect_drift` to avoid name collision.

7. **Tests** (parallel to `build_architecture_*`):
   - `compare_architecture_returns_empty_when_matches`
   - `compare_architecture_detects_missing_container`
   - `compare_architecture_detects_extra_container`
   - `compare_architecture_detects_wrong_sub_kind`
   - `compare_architecture_handles_missing_yaml`

**Minimal dependencies:** none new (`serde_yaml` is already
transitive; if not direct, add `serde_yaml.workspace = true`).

**Open question for user:** should we ship A now, or wait for a
spec'd comparison algorithm (B/C)?

---

## 11. Ready For Proposal

**Yes** for Option A. The work is:
- ~200 lines of Rust (DTOs + service method + tests).
- 1 small YAML file (`expected-architecture.yaml`).
- 1 new endpoint + 1 new ViewExecutor + 1 new MCP tool.
- 1 doc update (mention the new tool in CONTEXT.md + add §10 to
  ADR-039).

Skip a full `sddk-design` and go straight from
`sddk-propose` → `sddk-tasks` → `sddk-apply`. The kernel exploration
above plus the E6 exploration (`sddk/explorations/c4-backend-inference.md`)
together are enough to write the proposal.

For Option B/C, return here after a design conversation about the
ADR-parser format.
