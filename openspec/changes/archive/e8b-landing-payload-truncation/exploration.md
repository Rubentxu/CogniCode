# Exploration: e8b-landing-payload-truncation

## Current State

Cycle `e8-graphlanding-affordances` (v0.24.1, merged 2026-06-25) shipped the
frontend side of the landing-page truncation banner
(`openspec/specs/graphlanding-affordances/spec.md` Requirement 1). The banner
renders when `LandingPayload.truncated === true`, with
`LandingPayload.truncated_reason` shown in parentheses. The schema and the
banner code are in place.

What was NOT shipped in E8 (and was explicitly deferred as W-1 in the
verify-report):

> The banner code (`GraphLanding.tsx:216-231`) and schema
> (`schemas.ts:1229-1230`) are wired correctly, but the backend
> `LandingPayload` (`crates/cognicode-explorer/src/dto.rs:782-799`) does not
> return `truncated` / `truncated_reason`. The banner is therefore
> invisible in production today.

This cycle closes that gap.

### Drift: backend `LandingPayload` doesn't expose truncation

`crates/cognicode-explorer/src/dto.rs:782-799`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandingPayload {
    pub workspace: WorkspaceSummary,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub entry_points: Vec<InspectableObjectSummary>,
    pub hot_paths: Vec<InspectableObjectSummary>,
    pub god_nodes: Vec<GodNodeEntry>,
    pub suggested_questions: Vec<String>,
    pub graph_status: GraphStatus,
    // ← no truncated
    // ← no truncated_reason
}
```

The frontend already accepts these fields as `.optional()` (E8 PR-1), so
older backends parse cleanly. The cycle just adds them to the backend so
the banner can light up.

### Drift: handler returns empty stubs (NOT in scope for e8b)

`crates/cognicode-explorer/src/api.rs:669-688`:

```rust
// Build the landing payload with empty stubs for now.
// TODO: Wire get_entry_points, get_hot_paths, graph_insights from the
// analysis service once those methods are available on a facade.
let payload = LandingPayload {
    // ...
    nodes: Vec::new(),
    edges: Vec::new(),
    entry_points: Vec::new(),
    hot_paths: Vec::new(),
    god_nodes: Vec::new(),
    suggested_questions: Vec::new(),
    // ...
};
```

This is a SEPARATE problem (stale TODO since at least 2026-06-22 per
`feat/e7-renderer-scale-evaluation` commit `d4438b3`). `get_entry_points`
and `get_hot_paths` already exist in
`crates/cognicode-core/src/application/workspace_session.rs:1198,1443`,
but they are NOT exposed through the `Graph` facade in
`crates/cognicode-explorer/src/facades/graph.rs`. The facade currently
exposes only `build_subgraph`, `build_architecture`, `compare_architecture`.

Wiring these methods through the facade is a separate, larger cycle
(probably `e10-landing-real-data` or similar). This cycle focuses on the
DTO contract + truncation hook only.

### Reference: how `SubgraphResponse` already does it

`crates/cognicode-explorer/src/facades/graph.rs:107-194` already
implements the same truncation pattern that `LandingPayload` should
follow:

```rust
let mut truncated = false;

while let Some((current_id, current_depth)) = queue.first().cloned() {
    // ...
    if nodes.len() >= max_nodes_usize {
        truncated = true;
        break;
    }
    // ...
}

Ok(SubgraphResponse {
    // ...
    truncated,
    truncated_reason: if truncated {
        Some("node_cap".to_string())
    } else {
        None
    },
    // ...
})
```

The cap value comes from `max_nodes_usize` (query parameter, default
200). For landing we'll use a fixed cap (no query parameter on the
landing endpoint) — say `LANDING_NODE_CAP = 50` (matches the
landing-page UX: a workspace graph should fit in one viewport).

### Reference: how `ContextualGraphResponse` does it (almost)

`crates/cognicode-explorer/src/facades/view.rs:268-284` uses a
different field name (`truncation_reason`, not `truncated_reason`).
This is an existing inconsistency in the API surface that should NOT
be replicated for the landing — landing uses `truncated_reason` to
match `SubgraphResponse` (which the landing's existing schema in E8
already aligns with).

## What this cycle changes

| File | Change | Why |
|---|---|---|
| `crates/cognicode-explorer/src/dto.rs:782-799` | +2 fields on `LandingPayload` | Match the frontend schema (E8 PR-1) |
| `crates/cognicode-explorer/src/api.rs:646-691` | Construct `truncated: false, truncated_reason: None` for now | Handler stub today; cap logic prepared for future |
| `crates/cognicode-explorer/src/dto.rs` (new constant) | `pub const LANDING_NODE_CAP: usize = 50` | Document the cap; future cycles that wire real data will consult it |
| `crates/cognicode-explorer/src/api.rs` (helper) | `fn apply_landing_cap(total: usize) -> (truncated, truncated_reason)` | Pure function that's a no-op today (always `false`) but ready for use once the handler returns real `nodes`/`entry_points` |
| `crates/cognicode-explorer/tests/api_landing_truncation.rs` (new) | RED-then-GREEN tests for: DTO shape, `apply_landing_cap` behavior, handler returns 200 with `truncated=false` | Strict TDD: tests fail before implementation |
| `apps/explorer-ui/src/mocks/landingFixtures.ts` | No change (already includes `truncated: false, truncated_reason: null` from E8) | E8 already covered |
| `openspec/specs/graphlanding-affordances/spec.md` | Add Requirement 9 (backend contract) + delta to Requirement 2 (no longer optional — strongly recommended but still optional for backwards compat with sub-v0.24.1 servers) | Document the new backend contract |

## Affected areas

```
crates/cognicode-explorer/src/
├── dto.rs                            ← +2 fields on LandingPayload, +1 constant
├── api.rs                            ← handler sets truncated=false, helper apply_landing_cap
├── facades/graph.rs                  ← UNCHANGED (already correct pattern)
└── tests/api_landing_truncation.rs   ← NEW (RED → GREEN tests)

apps/explorer-ui/src/
└── (unchanged — E8 PR-1 already covered the schema)

openspec/specs/graphlanding-affordances/
└── spec.md                           ← MODIFIED Requirements 2 + NEW Requirement 9
```

## Approaches

### Approach A: Minimum viable (recommended for this cycle)

Add the fields to the DTO, add the cap constant, add a
`apply_landing_cap` helper that returns `(false, None)` today, write
the strict-TDD tests.

- **Pros**: Smallest PR (≈30 LOC Rust + tests). Mirrors `SubgraphResponse`
  exactly. Tests are forward-compatible — when the handler eventually
  returns real `nodes`, the helper will return `(true, "node_cap")` and
  the tests will verify both branches.
- **Cons**: The banner remains dormant in production today because the
  handler still returns `nodes: Vec::new()`. The cycle closes the
  contract gap, not the data gap.
- **Effort**: Low.

### Approach B: Wire real `entry_points` + activate banner

Add a new method to the `Graph` facade that returns top-N entry points
as `InspectableObjectSummary`. Wire it into the handler. If
`entry_points.len() > LANDING_NODE_CAP`, set `truncated = true`.

- **Pros**: Banner is live in production today.
- **Cons**: Adds a new method to the `Graph` facade (~80 LOC), plus
  handler wiring, plus integration tests. The data shape of
  `entry_points` is `InspectableObjectSummary` (for Spotter), but the
  landing UX might want different fields (file, line, kind). This is
  a design conversation that this cycle doesn't need to answer.
- **Effort**: Medium.

### Approach C: Wire both entry points and hot paths (with caps)

Same as B but also wires `get_hot_paths`. Probably also touches
`god_nodes` (which the E8 banner doesn't reference, but the landing
UX does).

- **Pros**: Real landing data end-to-end.
- **Cons**: Large. Probably 200+ LOC, 4-5 PRs. This is its own cycle
  and deserves its own spec/ADR.
- **Effort**: High.

## Recommendation

**Approach A**, with a clearly-tagged follow-up cycle `e10-landing-real-data`
for Approach B/C.

Rationale:
- Closes the contract gap that E8 verify-report W-1 flagged.
- Strict TDD tests document the contract and act as the spec.
- The `apply_landing_cap` helper is a hook for the future cycle; it
  doesn't add a runtime dependency.
- Banner remains dormant in production but is now ready to activate
  when the future cycle ships real data.
- Smallest blast radius: one Rust struct change + one handler line +
  tests.

The `e10-landing-real-data` follow-up (Approach B/C) should be planned
separately and likely needs an ADR about whether `entry_points` on the
landing endpoint should be `InspectableObjectSummary` or a leaner
landing-specific struct.

## Risks

- **Test runner parity**: The cognicode-explorer crate uses `cargo test`
  + `sqlx::test` for postgres-canonical tests. New tests should use the
  plain `#[tokio::test]` pattern (matching
  `crates/cognicode-explorer/src/api_graph_tests.rs`).
- **JSON wire compat**: Adding two optional fields to `LandingPayload`
  is a non-breaking change for clients (the frontend schema already
  declares them optional). For older clients that don't know about
  them, serde will skip them on deserialize.
- **Doc string drift**: The handler comment currently says "TODO: Wire
  get_entry_points...". After this cycle, the truncation aspect of
  the TODO is closed but the data aspect is not. Update the comment to
  reflect this.

## Ready for Proposal

**Yes.** Approach A is well-scoped, the pattern matches `SubgraphResponse`,
and the tests are clear.
