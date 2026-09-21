# Decision Support Packs Specification

## Purpose

Wire the E25.1 slice: a backend fan-out `DecisionSupportPackExecutor` that
composes DecisionGraph + ArchitectureRationale + EvidencePack + RiskMap +
ChangeImpactStory into a coherent inspectable pack, exposed via REST and
rendered in the Explorer pane stack. Differentiate DecisionGraph from
ArchitectureRationale: graph topology via `GraphQueryPort::subgraph()` and
`RendererKind::Graph` (DecisionGraph) vs. Markdown narrative
(ArchitectureRationale). Strict E25.1 scope — no new tables, no new ports,
no ContextRail content (E27.3-owned).

Reference: proposal `e25-decision-support-packs` (Decisions A: Differentiate;
B: Backend fan-out — both locked). Plan 015 status → ACTIVE.

---

## ADDED Requirements — NEW capability `decision-support-packs`

### Requirement: DecisionSupportPackExecutor registration

The system MUST register a `DecisionSupportPackExecutor` in the ViewRegistry
that resolves via `ViewKind::DecisionSupportPack` and accepts
`InspectionTarget::Decision(DecisionArtifact)`. The executor's
`view_kind()` MUST return `ViewKind::DecisionSupportPack` and its
`applies_to()` MUST include `InspectableObjectType::DecisionArtifact`.

#### Scenario: Decision Support Pack is discoverable

- GIVEN the ViewRegistry is initialised with the pack executor registered
- WHEN a consumer queries available views for `DecisionArtifact`
- THEN a `decision-support-pack` descriptor with `view_kind = decision_support_pack` is present
- AND the descriptor's title is "Decision Support Pack"

#### Scenario: Non-Decision target returns ViewNotAvailable

- GIVEN a `Symbol` or `File` inspection target
- WHEN `DecisionSupportPackExecutor::build()` is called
- THEN the result is `Err(ViewNotAvailable { object_id, view_id: "decision-support-pack" })`
- AND the error is a typed `ExplorerError`, not a panic

### Requirement: Pack endpoint returns composable pack

The system MUST expose `GET /api/decisions/:id/support-pack` returning a
`DecisionSupportPack` JSON payload. The payload MUST include the focus
decision id, the pack's `view_kind`, a `status` field, and a `panes`
collection — one entry per sub-view with `{ pane_id, view_id, view_kind,
renderer_kind, view: ContextualView }`. The handler MUST respond `404`
if the decision id is unknown, `200` otherwise.

#### Scenario: Composed pack returns all sub-views

- GIVEN a decision that resolves to ArchitectureRationale, EvidencePack,
  RiskMap, and ChangeImpactStory sub-views
- WHEN client calls `GET /api/decisions/{id}/support-pack`
- THEN response status is `200`
- AND `payload.panes` contains entries for each sub-view with pane_id,
  view_id, view_kind, renderer_kind, and a populated `view`

#### Scenario: Unknown decision id

- GIVEN a decision id that does not exist in the repository
- WHEN client calls `GET /api/decisions/{id}/support-pack`
- THEN response status is `404`
- AND body is a Problem Details JSON with `error="decision_not_found"`

### Requirement: Parallel fan-out with failure-tolerant degradation

The pack executor MUST resolve the sub-view executors concurrently and
collect each `ContextualView` into the pack. When one sub-view build
fails, the pack MUST still return the remaining successful sub-views and
MUST mark the failed entry with `status: "degraded"` and an `error`
field; the overall response MUST remain `200`.

#### Scenario: All sub-views succeed

- GIVEN 5 sub-views all build successfully
- WHEN the pack executor runs
- THEN `payload.panes` contains 5 entries
- AND every entry has `status: "ok"` and no `error` field

#### Scenario: One sub-view fails

- GIVEN ArchitectureRationale sub-view build returns `Err(_)`
- WHEN the pack executor runs
- THEN `payload.panes` contains 4 `status: "ok"` entries
- AND 1 entry has `status: "degraded"` with `error` describing the failure
- AND response status is `200`

#### Scenario: Empty sub-view result is not failure

- GIVEN a sub-view returns `Ok(ContextualView)` with empty blocks
- WHEN the pack executor runs
- THEN that entry is `status: "ok"` (not `degraded`) and the empty body is preserved

### Requirement: Empty-decision graceful degradation

The pack executor MUST return a valid pack (focus decision id, `status:
"empty"`, no `panes`) when the underlying decision has no resolvable
sub-views. The response MUST NOT be `404` and MUST NOT panic.

#### Scenario: Decision with no linked sub-views

- GIVEN a decision whose rationale subgraph is empty AND no evidence/risk/impact data
- WHEN `GET /api/decisions/{id}/support-pack` runs
- THEN response status is `200`
- AND `payload.status` equals `"empty"`
- AND `payload.panes` is an empty array
- AND `payload.decision_id` reflects the requested id

### Requirement: Pane stack renders the pack sub-views

The Explorer pane stack MUST render pack sub-views as lateral panes. When
a pack response is loaded, the active pane MUST switch to the focus
pane and the remaining pack sub-views MUST open as additional lateral
panes (preserving the E27 pane-stack narrative). The ContextRail MUST
NOT consume pack content (E27.3-owned boundary).

#### Scenario: Pack opens one lateral pane per sub-view

- GIVEN the user activates a Decision Support Pack for a decision with 4 sub-views
- WHEN the response is rendered
- THEN the pane stack contains 5 panes — 1 focus + 4 lateral
- AND lateral panes are reachable via tabs in the existing pane stack
- AND `data-testid="context-rail"` does NOT contain any pack pane title

#### Scenario: Pack sub-views are inspectable like any pane

- GIVEN a lateral pane from the pack shows `view_kind = "evidence_pack"`
- WHEN the user clicks a view tab on that pane
- THEN the existing view-tab behaviour applies (no new pane, `activeViewId` updates)

## MODIFIED Requirements — `view-registry-backend`

### Requirement: DecisionGraph topology builder

The system MUST build a DecisionGraph view by constructing a decision
topology via `GraphQueryPort::subgraph()` over the decision node, bounded
by `max_depth` and `max_nodes`. The topology MUST traverse decision-level
relations (ADR → Code → Tests → Docs → Evidence) and MUST emit a
`ContextualView` whose blocks contain the topology's nodes and edges
suitable for a `Graph` renderer.

The executor MUST NOT delegate to `build_rationale_view()`. ArchitectureRationale
retains the rationale-subgraph path (Markdown narrative); DecisionGraph
and ArchitectureRationale MUST emit structurally distinct views.

(Previously: DecisionGraph delegated to `build_rationale_view()` and
differed only by title — Decision A locks the differentiation.)

#### Scenario: DecisionGraph emits Graph topology

- GIVEN a decision with decision-level relations (ADR → Code → Tests → Docs → Evidence)
- WHEN the DecisionGraphExecutor builds the view
- THEN the `ContextualView.renderer_kind` is `RendererKind::Graph`
- AND the blocks contain reachable nodes and edges from the topology traversal
- AND the decision focus node is present even when no edges leave it

#### Scenario: DecisionGraph differs from ArchitectureRationale

- GIVEN a decision and a fresh `ViewContext`
- WHEN the executor builds the view once with `ViewKind::DecisionGraph`
  and once with `ViewKind::ArchitectureRationale` over identical inputs
- THEN the block sets are NOT equal as JSON
- AND ArchitectureRationale uses `RendererKind::Markdown` while DecisionGraph uses `RendererKind::Graph`

#### Scenario: DecisionGraph topology empty gracefully

- GIVEN a decision whose `GraphQueryPort::subgraph()` returns an empty result
- WHEN the DecisionGraphExecutor builds the view
- THEN the result is `Ok(ContextualView)` with focus node and zero edges
- AND `truncated` equals `false`

## REMOVED Requirements

### Requirement: DecisionGraph delegates to build_rationale_view

(Reason: Decision A locks differentiation. DecisionGraph now emits a Graph
topology via `GraphQueryPort::subgraph()`; ArchitectureRationale keeps the
rationale-subgraph path. Sharing a single builder produced two views that
were indistinguishable except by title — defeating the purpose of the
distinction.)

## Coverage

- **Happy paths**: pack endpoint returns composed pack; sub-views succeed
- **Edge cases**: empty decision, empty sub-views, single sub-view failure
- **Error states**: unknown decision id (404), sub-view failure (degraded 200), unsupported target (ViewNotAvailable)
- **Differentiation**: DecisionGraph renderer is Graph, ArchitectureRationale is Markdown
- **Frontend**: pack renders as lateral panes (no ContextRail injection)
