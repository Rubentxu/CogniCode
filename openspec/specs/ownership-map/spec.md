# Spec: OwnershipMap ViewExecutor (Quality-Ownership)

## Purpose

Wire the `OwnershipMap` ViewExecutor so it becomes reachable from the
Explorer inspector pane for `Symbol`, `Scope`, and `Issue` objects.

`OwnershipMap` answers: "who owns this code or quality issue?" In v1 the
only reachable ownership data is the **code quality ownership** encoded in
`QualityIssue` structs: `rule_id` (the rule that detected the problem,
acting as a proxy for ownership), `severity`, `file_path`, and `status`.

Symbols, files, and scopes have **no** ownership data in v1 — the
executor shows an explicit `"ownership unavailable"` placeholder.
CODEOWNERS parsing and `git blame` are deferred to e12f-2.

`ViewKind::OwnershipMap` is already catalogued (`dto.rs`) and listed
in the frontend `ViewSpecWizard`. This spec wires the backend
executor only.

---

## ADDED Requirements

### Requirement: 1. OwnershipMapExecutor — Scope

The `OwnershipMapExecutor` MUST be added to the `ViewRegistry` and
MUST apply to `InspectionTarget::Symbol`, `InspectionTarget::Scope`,
and `InspectionTarget::Issue`. The executor MUST NOT panic or error
on `InspectionTarget::File` or `InspectionTarget::Rule`; it MUST
render the degraded placeholder instead.

### Requirement: 2. build_ownership_map Function

The `build_ownership_map` function in
`crates/cognicode-explorer/src/domain/views.rs` MUST return a
`ContextualView` with a single `ViewBlock`:

- `view_id`: `"ownership-map"`, `view_kind`: `ViewKind::OwnershipMap`,
  `renderer_kind`: `RendererKind::Table`.
- `block.id`: `"ownership"`, `block.title`: `"Ownership"`.
- Table columns: `["node", "file", "severity", "rule_id", "status"]`.

**QualityIssue semantics for ownership**:
- `node`: the issue identifier (`"Issue #<id>"`)
- `file`: `issue.file_path` — the file where the issue was detected
- `severity`: `issue.severity` — urgency indicator (blocker, critical, major, minor, info)
- `rule_id`: `issue.rule_id` — the rule that detected this, acting as ownership proxy (who wrote the rule is responsible for its quality)
- `status`: `issue.status` — resolution state

**Placeholder** (Symbol, Scope, File, Rule, or when graph data is unavailable):
- Single row with `"ownership unavailable"` in the `node` column
- All other columns empty
- The placeholder cell MUST NOT be blank, empty, or `null`.

#### Scenario: Issue with quality ownership data

- GIVEN `InspectionTarget::Issue(QualityIssue { id: 42, rule_id: "S2583", severity: "major", file_path: "src/auth.rs", status: "open", .. })`
- WHEN `build_ownership_map` is called
- THEN the block has 1 row with `node = "Issue #42"`, `file = "src/auth.rs"`, `severity = "major"`, `rule_id = "S2583"`, `status = "open"`

#### Scenario: Symbol with no ownership data

- GIVEN `InspectionTarget::Symbol(UserService::create_user)` with no ownership graph-node properties
- WHEN `build_ownership_map` is called
- THEN the block has 1 row with `node = "ownership unavailable"` and all other columns empty

#### Scenario: Scope with no ownership data

- GIVEN `InspectionTarget::Scope { path: "src/auth", .. }`
- WHEN `build_ownership_map` is called
- THEN the block has 1 row with `node = "ownership unavailable"` and all other columns empty

#### Scenario: File or Rule target — placeholder only

- GIVEN `InspectionTarget::File` or `InspectionTarget::Rule`
- WHEN `build_ownership_map` is called
- THEN the block renders the placeholder row and no error is returned

#### Scenario: GraphQueryPort unavailable

- GIVEN `graph_query: None` in `ViewContext`
- WHEN `build_ownership_map` is called
- THEN the block renders the placeholder row and no error is returned

### Requirement: 3. Registration

The executor MUST be registered in `registry.rs` `REAL_EXECUTORS`
map with key `"ownership-map"`. After registration the registry MUST
expose 15 real executors (the 14 existing plus `ownership-map`), and
`get_executor("ownership-map")` MUST return
`Some(&'static dyn ViewExecutor)`.

#### Scenario: ownership-map appears in REAL_EXECUTORS

- GIVEN the 14 existing executors are registered
- WHEN `ViewRegistry::get_executor("ownership-map")` runs after the change is applied
- THEN it returns `Some(&'static dyn ViewExecutor)` and the count of real executors is 15

### Requirement: 4. Frontend View Selector

The inspector pane's view selector already lists `"ownership_map"`
in `ViewSpecWizard`. The view MUST be selectable from the inspector
for Symbol, Scope, and Issue objects after the executor is registered.

#### Scenario: ownership_map selectable from inspector

- GIVEN a registered `OwnershipMapExecutor` and an inspected Symbol
- WHEN the inspector pane renders the view selector
- THEN `"ownership_map"` appears in the available views list

---

## UNCHANGED Requirements

- The 14 existing executors continue to work unchanged.
- `ViewRegistry::known_view_kinds()` continues to list `OwnershipMap` (already present).
- `QualityIssue` struct is not modified.
- No new schema columns, no new database migrations.

## Out of Scope (deferred to e12f-2)

- `gix`/`git2` blame pipeline; `.github/CODEOWNERS` parsing in ingest
- `owner`/`author` columns on `symbols` table or `scan_manifest`
- Symbol/file ownership from language analysis
- Configurable placeholder text via runtime config (v1 hardcodes the string)

## Model Notes

**Why `rule_id` as ownership proxy?** A SonarQube/SonarJS rule like `S2583` (抽像) has a known author/team at SonarSource — the rule itself encodes ownership. Similarly, a project-specific rule carries the author's intent. `rule_id` is the best available proxy for "who is responsible for this code quality problem" in v1.

**Why not GitHub Issue `author`/`assignee`?** `InspectionTarget::Issue(QualityIssue)` carries a `QualityIssue` struct from the `issues` table. This struct has **no** `author`/`assignee` fields — those live on a separate GitHub Issues system (graph nodes with `properties.author`/`properties.assignee`) that is not reachable from `InspectionTarget::Issue`. See ADR-046 §7 on system boundary.

## Coverage

- **Happy paths**: covered (QualityIssue renders as quality-ownership table)
- **Edge cases**: covered (Symbol, Scope, File, Rule, missing graph node, GraphQueryPort None)
- **Error states**: covered (no panic on degraded path; placeholder never blank)
