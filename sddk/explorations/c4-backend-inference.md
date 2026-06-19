# Kernel Exploration: C4 Backend Inference (E6 / ADR-039 §8)

**Date:** 2026-06-19
**Triggered by:** E6 of ADR-039 — "C4 structure is inferred automatically from
what the backend already parses: Containers from `Cargo.toml`/`package.json`,
Components from directory structure and module boundaries, Code elements from
symbols and edges."

**Context level:** C1 (significant code read; no prior C4-inference kernel
exploration exists; the planned `cognicode-diagram` crate is docs-only and
unbuilt).

**Recommendation:** Skip `cognicode-diagram` for E6. Extend
`cognicode-explorer::GraphService` with a `root_path` and a small
`Cargo.toml`/`package.json` parser to produce real C1+C2+C3+C4 nodes
inline. Defer the full diagram crate to a follow-up sprint.

---

## 1. Current State

### 1.1 What the architecture endpoint returns today

**Endpoint:** `GET /api/workspaces/:workspace_id/architecture`
(`crates/cognicode-explorer/src/api.rs:667-678`).

**Implementation:** `GraphServiceImpl::build_architecture()`
(`crates/cognicode-explorer/src/facades/graph.rs:201-258`).

**Output:** A `SubgraphResponse` synthesised from `module_list()`:

```text
modules = ["crates/cognicode-core/src/domain/aggregates",
           "crates/cognicode-core/src/domain/value_objects",
           "crates/cognicode-explorer/src/facades",
           ...]
```

Each entry becomes a `GraphNode { id: "component:<path>", kind: "component",
style_class: "node-component" }`. If `<path>`'s parent is also in the
list, a `part_of` edge is added.

**Result:** the E5 toggle renders ~1,000 `node-component` nodes — one per
directory containing source code. There is no real C2 container boundary, no
C1 system node, and the C3 components are just directories, not module
boundaries. ADR-039 §8 explicitly says this is wrong: "C4 structure is
inferred automatically from what the backend already parses: **Containers
from Cargo.toml/package.json**…".

### 1.2 What is available TODAY to build containers (C2)

| Data source | Available? | Where | Used by `build_architecture`? |
|---|---|---|---|
| Workspace `Cargo.toml` members list | **File exists, not parsed** | `Cargo.toml:1-17` in the workspace root | No |
| Workspace `Cargo.toml` `[lib]` / `[[bin]]` per crate | **File exists per crate, not parsed** | `crates/*/Cargo.toml` | No |
| `package.json` per app | **File exists, not parsed** | `apps/*/package.json` | No |
| Module list (dirs) | ✅ In graph | `SymbolRepository::module_list()` | Yes — used |
| Symbols + edges | ✅ In graph | `CallGraph::symbol_ids()` / `edges` | No (only at C4 level, not yet wired) |
| Workspace root path | ✅ Held by `WorkspaceServiceImpl` | `WorkspaceServiceImpl { root_path: PathBuf }` | Not reachable from `GraphServiceImpl` |
| TOML parser | ❌ **Not in workspace** | `Cargo.lock` has `toml` as transitive of test-deps only; no `toml =` direct dep | n/a |
| `tree-sitter-toml` | ❌ **Not in workspace** | n/a | n/a |
| `cargo_metadata` crate | ❌ **Not in workspace** | n/a | n/a |

**The only blocking gap is "TOML is not parsed".** The crate metadata
itself is on disk and the backend knows where the workspace root is.

### 1.3 The `cognicode-diagram` crate is docs-only

`docs/planes/cognicode-diagram/` contains 10 planning documents
(ARQUITECTURA.md, INVESTIGACION.md, PLAN-FASE1..7.md, MCP-TOOLS.md,
ROADMAP.md) but no `Cargo.toml` and no source files. The folder is a
blueprint, not a crate. `cargo_metadata` is not pulled in. The plan
proposes `rust-sugiyama` (layout), `structurizr-rs` (DSL reference), and a
full inference pipeline (L1–L4) — all out of scope for E6.

### 1.4 The `cognicode-core` Cargo.toml

`crates/cognicode-core/Cargo.toml` has no TOML-parser dependency. It does
have:

- 30+ tree-sitter language grammars (no TOML grammar).
- `petgraph` for graphs.
- `serde_json`, `serde_yaml` (no `toml`).
- `walkdir`, `ignore`, `glob` for filesystem.
- No workspace-level parser. `Cargo.toml` is treated as a generic file
  (`file_type: Config` at `scan.rs:115`, with no special content
  extraction).

### 1.5 Module list semantics (what `module_list()` actually returns)

`CallGraph::module_from_file(file)` returns the parent directory of
`file` (`call_graph.rs:768-773`). For a Rust source like
`crates/cognicode-explorer/src/facades/graph.rs`, that is
`crates/cognicode-explorer/src/facades`.

So `module_list()` returns **the deepest directory of each source file**,
not the crate root. To build C2 containers we need to walk **up** the path
until we find a directory that contains a `Cargo.toml` — that IS the
container boundary.

### 1.6 The `GraphService` facade is what we'd extend

```rust
// crates/cognicode-explorer/src/facades/graph.rs:19-33
pub struct GraphServiceImpl {
    symbol_repo: Arc<dyn SymbolRepository>,
    graph_query: Option<Arc<dyn GraphQueryPort>>,
}

pub fn new(
    symbol_repo: Arc<dyn SymbolRepository>,
    graph_query: Option<Arc<dyn GraphQueryPort>>,
) -> Self
```

**`GraphServiceImpl` does not hold `root_path`.** Compare with
`WorkspaceServiceImpl { root_path: PathBuf }` (line 21 of
`facades/workspace.rs`). Adding `root_path: PathBuf` to
`GraphServiceImpl::new` is a small, localised change.

**Callers of `GraphServiceImpl::new`** (must all be updated together):
- `mcp/explorer.rs:329` — production wiring, has `cwd: PathBuf` in scope.
- `api_rationale_tests.rs:254` — test wiring, can pass `PathBuf::new()`.
- `api_graph_tests.rs:417`, `api_graph_tests.rs:1480` — test wiring.
- `facades/moldql.rs:98` — internal use.
- `domain/views.rs` — 14 test mock sites (line 2494…2905) that pass
  `graph_query: None` to a different constructor; need to verify they
  use a different ctor (they do — they construct `ViewServiceImpl`,
  not `GraphServiceImpl`).

So `GraphServiceImpl::new` has ~5 real call sites in the production code
plus 2 test sites. Cheap to migrate.

---

## 2. Context Quality

- **Level:** C1 — direct code reading only; no kernel exploration or
  design for E6 exists yet; `cognicode-diagram` is a planning surface
  only.
- **Evidence Present:**
  - ADR-039 §8 specifies inference rules (`Cargo.toml`/`package.json`
    for containers, modules for components, symbols+edges for code).
  - `docs/planes/cognicode-diagram/ARQUITECTURA.md:249-271` and
    `INVESTIGACION.md:15-19` document the same heuristics in
    detail.
  - `crates/cognicode-explorer/src/facades/graph.rs:201-258` shows the
    current directory-as-component implementation.
  - `crates/cognicode-core/src/domain/aggregates/call_graph.rs:768-781`
    shows `module_list()` semantics.
  - `crates/cognicode-explorer/src/facades/workspace.rs:21,25` shows
    the existing `root_path` plumbing pattern.
- **Missing Context:**
  - No prototype of TOML parsing (no `[dependencies.toml]` or
    `tree-sitter-toml` to test against).
  - No sample `package.json` test fixture for the JS app dir.
  - No agreed contract for how the frontend should render a
    4-level C4 hierarchy inside the existing `SubgraphResponse`
    shape (it is flat — no nesting primitive today).
- **Recommended Effort:** **Deepen** (focused verification of TOML
  parsing + a DTO shape decision) before going to proposal.

---

## 3. Knowledge Coverage

| Class | Status | Evidence | Gap Impact |
|------|--------|----------|------------|
| Roadmap/Backlog | present | `docs/adr/039 §8`, `docs/planes/cognicode-diagram/ROADMAP.md` (F2 = L2+L3) | Low — vision is clear, no ambiguity |
| Work Items | stale | `sddk/proposals/graph-landing-page.md` references E5, not E6; `sddk/explorations/graph-landing-page.md:213,327-339` flags the C4 perspective toggle as a known follow-up but does not call it E6 | Medium — no in-flight ticket for E6 |
| Architecture/ADRs | present | `ADR-039 §8` is the spec; `cognicode-diagram/ARQUITECTURA.md` is a longer form | Low |
| Ownership | missing | No `CODEOWNERS` entry for `cognicode-explorer::facades::graph`; no ticket assignee | Medium — blocks escalation if questions arise |
| Learnings | partial | `engram` has 5 E5-related observations; 0 E6-related | Medium — prior E5 C4-as-directories was accepted as good-enough, may bias reviewers away from full C4 |

---

## 4. Problem Taxonomy

| Axis | Applies | Evidence |
|------|---------|----------|
| Domain modeling | **Yes** | "Container" is a new concept not in the core graph; the boundary between C2 (crate) and C3 (module) is the principal design decision. The directory-as-component is the wrong model. |
| Boundary/seam | **Yes** | The new C2 inference lives in `cognicode-explorer` (NOT `cognicode-core`) because: (a) it reads from the filesystem using a path the core does not hold, (b) the result is a UI-flavoured `SubgraphResponse`, not a graph mutation. |
| Coupling/connascence | **Yes** | `GraphServiceImpl` will grow a `root_path: PathBuf` field. `WorkspaceServiceImpl` already has one. We should extract a small `WorkspaceRoot` newtype to prevent connascence of identity on a raw `PathBuf`. |
| API contract | **Yes** | `SubgraphResponse` is flat (no nesting). Returning a C1→C2→C3→C4 hierarchy either flattens it (loses semantics) or extends the response shape (breaking change). Decision needed before proposal. |
| Refactor/legacy | **No** | The directory-as-component code is recent (E5) and clean. We are replacing it, not refactoring around it. |
| Event/CQRS | **No** | This is a read-time inference, not an event-sourced projection. |
| Testing | **Yes** | `api_graph_tests.rs` already has 4 `module_list` mocks (lines 828, 1079, 1274, 1385) but no `build_architecture` integration test. New tests must cover: workspace with 1 crate, with N crates, with JS `package.json` apps, with no `Cargo.toml` (degrade gracefully). |
| Security/operations | **No** | C2 inference reads public metadata; no PII, no auth path. |

---

## 5. Domain Language And Invariants

### 5.1 Domain Language

| Term | Resolved meaning | Unresolved ambiguity |
|---|---|---|
| **C1 System** | "CogniCode" (the indexed workspace as a single software system) | None — single hardcoded value. |
| **C2 Container** | A unit of deployment/runtime. In Rust: a crate with `[lib]` or `[[bin]]`. In JS: an `apps/*` with `package.json`. | Mixed-language workspaces: should a Rust `cognicode-core` and a JS `apps/explorer-ui` count as 2 containers in the same diagram? **Current belief: yes, with a `technology` discriminator.** |
| **C3 Component** | A logical grouping of code inside a container. In Rust: a top-level module (a path with ≥ 2 levels under `src/`). In JS: a top-level `src/` subdirectory. | The C3 boundary is fuzzy. We currently use **deepest parent dir of source files** (`module_from_file`). The cleaner definition is "path with at least one `mod.rs`/`index.ts` above it" — but that requires walking. |
| **C4 Code** | A `Symbol` (function, struct, trait, etc.) inside a C3 component. | None — 22-kinds `SymbolKind` enum is well-defined. |
| **`part_of`** | Edge from child → parent in the C4 hierarchy. | Should be typed differently per level? (`c4_part_of` vs `c3_part_of`?) or a single `part_of` with `style_class` discriminant. Current code uses one relation with one `edge-part-of` style class. |

### 5.2 Invariants

1. The `architecture` endpoint must return *at minimum* one C1 node,
   never an empty graph.
2. The hierarchy must be acyclic (containers do not contain themselves).
3. Components in different containers never share a `part_of` parent.
4. If `Cargo.toml` parsing fails (malformed, missing), the endpoint must
   **degrade** to the current directory-as-component behaviour, not
   500.
5. `build_architecture()` must remain synchronous-feeling (no
   filesystem-walk that blocks the runtime). A single `read_to_string`
   of the workspace `Cargo.toml` is acceptable; a recursive directory
   walk on every call is not.

---

## 6. Knowledge Gaps

- **Cargo.toml parsing library decision** — the workspace has no `toml`
  crate and no `tree-sitter-toml`. Choices: (a) add `toml = "0.8"` to
  workspace deps (industry standard, ~1 MB compiled), (b) use
  `toml_edit = "0.22"` (preserves formatting, useful if we ever
  *write* Cargo.toml — we don't), (c) hand-roll a tiny line-based
  parser (no — fragile). **Decision needed.**

- **DTO shape for hierarchical C4** — the existing `SubgraphResponse`
  is flat. Three options:
  1. **Flatten** everything to 4 levels of nodes (current shape),
     losing hierarchy in the wire format. Frontend re-derives hierarchy
     from `style_class`.
  2. **Add an optional `children: Vec<String>`** field on
     `GraphNode` (additive, backwards-compatible). Frontend reads
     `children` to build a tree view.
  3. **Wrap in a new `ArchitectureResponse { c1, containers[], components[], code[] }`**
     DTO. Cleanest contract, **breaking** on the existing E5 endpoint.

- **`package.json` app boundary** — `apps/explorer-ui` is a JS app and
  is NOT a Rust crate. Should the C2 inference cover JS apps, or
  scope E6 to Rust only? ADR-039 §8 says both. Minimal effort to
  cover both.

- **Container metadata fields** — Cargo.toml gives us
  `{ name, version, lib?, bin[], dependencies[] }`. Which fields
  belong in `GraphNode` (label, subtitle, file)? `style_class =
  "node-container"` is enough for the toggle; richer fields
  (technology, runtime) come later.

- **CodePoint for C4 (symbols)** — E5 ships a single-level toggle
  with no symbols. Should E6 also wire symbols as C4 nodes, or
  leave the "Code" layer for E7+? ADR-039 §8 explicitly says
  "Code elements from symbols and edges" — E6 should at least have
  the symbol->component edge wiring in place even if rendering is
  deferred.

---

## 7. Affected Areas

| File | Why |
|------|-----|
| `Cargo.toml` (workspace root) | Add `toml = "0.8"` to `[workspace.dependencies]` |
| `crates/cognicode-explorer/Cargo.toml` | Add `toml.workspace = true` |
| `crates/cognicode-explorer/src/facades/graph.rs` | Extend `GraphServiceImpl` with `root_path: PathBuf`; rewrite `build_architecture()` to return C1+C2+C3+C4 nodes |
| `crates/cognicode-explorer/src/facades/mod.rs` | Update `GraphServiceImpl::new` signature; document the change |
| `crates/cognicode-explorer/src/mcp/explorer.rs:329` | Pass `cwd` to `GraphServiceImpl::new` (it is already in scope as `cwd: PathBuf`) |
| `crates/cognicode-explorer/src/api_graph_tests.rs:417, 1480` | Pass `PathBuf::new()` or a fixture path in tests |
| `crates/cognicode-explorer/src/api_rationale_tests.rs:254` | Pass `PathBuf::new()` in test mock |
| `crates/cognicode-explorer/src/dto.rs:716` | **DECISION:** extend `SubgraphResponse` with optional `hierarchy: Option<ArchitectureHierarchy>` OR keep flat |
| `crates/cognicode-explorer/src/dto.rs:669` | Possibly add `sub_kind: Option<String>` to `GraphNode` (e.g. "library", "binary", "external") |
| `crates/cognicode-explorer/src/api.rs:672-678` | Handler unchanged if response shape unchanged |
| `apps/explorer-ui/src/components/InteractiveGraph/...` | Render new `node-container`/`node-system` style classes — owned by a follow-up UI ticket, not E6 |
| `docs/adr/ADR-039-explorer-navigation-model.md §8` | No change — already accurate |

---

## 8. Options

| Option | Pros | Cons | Effort |
|--------|------|------|--------|
| **A. Inline in `cognicode-explorer` (no new crate)** | (1) Unblocks E6 in 1 PR; (2) no new `Cargo.toml`; (3) reuses existing `SubgraphResponse` and `node-component`/`node-container` style classes; (4) `WorkspaceServiceImpl` already proves the `root_path` plumbing works | (1) Couples filesystem reading to the Explorer facade (mild boundary smell); (2) `cognicode-diagram` is now redundant for E6 — needs a re-scope conversation | S (1–2 days) |
| **B. New `cognicode-diagram` crate, deferred** | (1) Honors the planned crate; (2) cleaner separation | (1) 7 phases of planning (F1–F7) before anything ships; (2) way more than E6 needs; (3) re-implements `module_list` logic, splits the source of truth | XL (multi-week) |
| **C. New thin `cognicode-workspace-meta` crate** | (1) Reusable for E7+, M3 SLO work, future IaC tools; (2) clean boundary | (1) New crate, new CI target; (2) overkill for E6 alone | M (3–4 days) |

**Recommended:** **A.** E6 is small enough that the boundary tax of a new
crate exceeds the architectural benefit. Revisit B/C when E7+
("C4 with full layout", Structurizr DSL export) starts.

---

## 9. Entropy Envelope

- **Method:** heuristic (no CogniCode graph to query, but the analysis is
  about adding nodes so call-graph entropy is not the relevant metric;
  we are estimating the **connascence / OCP risk**).
- **Coupling risk:** **medium** — `GraphServiceImpl` gains a
  `root_path: PathBuf` field and a 3rd constructor argument. The
  `Build architecture` body grows from ~55 lines (directories only) to
  ~150 lines (C1+C2+C3+C4 with Cargo.toml parsing). Risk of growth
  in `build_architecture`; mitigate by extracting a private
  `infer_containers(root_path) -> Vec<Container>` helper.
- **OCP risk:** **low** — the function signature does not change; the
  body becomes richer. The frontend already supports
  `style_class = "node-component"`; adding `"node-container"` and
  `"node-system"` is a Cytoscape stylesheet addition, not a Rust
  change.
- **Connascence:** **medium** — `GraphServiceImpl` and
  `WorkspaceServiceImpl` both depend on the raw `PathBuf`. Extracting
  a `WorkspaceRoot(PathBuf)` newtype would prevent two-path-treats-the-
  same-file bugs and is a small additional change.

---

## 10. Recommendation

**Skip the new `cognicode-diagram` crate for E6.** Extend
`cognicode-explorer::GraphService::build_architecture()` to:

1. **C1:** add a single `node-system` node with
   `id: "system:cognicode"`, `label: workspace.name`, no parent.
2. **C2:** parse the workspace `Cargo.toml` once (lazily, on first
   call, then cache) to extract `members`. For each member, read
   its `Cargo.toml` and emit a `node-container` with
   `sub_kind: "library"|"binary"`. For JS apps in `apps/*`, parse
   `package.json` and emit a `node-container` with
   `sub_kind: "node-app"`. Each container gets a `part_of` edge to
   the system node.
3. **C3:** for every entry in `module_list()`, walk up to find the
   enclosing container (the deepest ancestor that contains a
   `Cargo.toml` or `package.json`). Emit a `node-component` per
   module, with `part_of` to its container. Modules that walk up to
   a path with no manifest become direct children of the system
   (degraded mode).
4. **C4 (light):** for each `ResolvedSymbol` whose file's parent
   matches a known C3 module, emit a `node-code` with `part_of` to
   the module. Cap total C4 node count at e.g. 200 to avoid blowing
   up the response.
5. **Defer to follow-ups:** Structurizr DSL export, real layout
   (Sugiyama/ELK), UmlRelationKind inference, mermaid rendering of
   the 4-level view. These are the `cognicode-diagram` F2+ items and
   do not block E6.

**Minimal dependencies to add:**

```toml
# workspace Cargo.toml [workspace.dependencies]
toml = "0.8"

# crates/cognicode-explorer/Cargo.toml [dependencies]
toml.workspace = true
```

Both are stable, widely-used, and ~1 MB compiled.

**Open questions for the user** (must answer before proposal):

1. **DTO shape** — keep `SubgraphResponse` flat (current shape) or
   add an optional `hierarchy` field? My recommendation: **flat**
   for E6 (E5 frontend already works with it), revisit when E7+
   needs real nesting.
2. **JS `package.json` scope** — include `apps/*` in E6 or Rust-only
   for the first PR? My recommendation: **include apps***, it costs
   ~30 lines of code and matches ADR-039 §8 verbatim.
3. **C4 symbols in E6** — wire symbols as C4 nodes (cheaper, 200-cap)
   or defer to E7? My recommendation: **wire them with a cap**, so
   the toggle can drill into a container and see real symbols.

---

## 11. Ready For Proposal

**Partially.** With the three open questions answered, the change is
small enough to skip a full `sddk-design` and go straight from
`sddk-propose` → `sddk-tasks` → `sddk-apply`.

**Status:** `blocked` on the three questions in §10.
