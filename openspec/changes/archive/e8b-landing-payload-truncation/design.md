# Design: e8b-landing-payload-truncation

## Technical Approach

A single Rust-only change that closes the backend truncation contract for
`LandingPayload`. The frontend is already prepared (E8 v0.24.1). The data
wiring is deferred to a future cycle (`e10-landing-real-data`); this cycle
only establishes the contract and a hook (`apply_landing_cap`) for the
future cycle to consume.

Branch: `feat/e8b-landing-payload-truncation` off `main`. PATCH semver
(`v0.24.2`).

## Architecture Decisions

### Decision: D-1 Field naming — `truncated_reason` (not `truncation_reason`)

**Choice**: Use the field name `truncated_reason` (matching
`SubgraphResponse`).
**Alternatives considered**: Use `truncation_reason` (matching
`ContextualGraphResponse`).
**Rationale**: Two existing endpoints use these two different names:

| Endpoint | Field name |
|---|---|
| `GET /api/graph/:id/subgraph` | `truncated_reason` |
| `GET /api/graph/:id/contextual` | `truncation_reason` (note: extra 'i') |

The inconsistency is real and pre-existing. For the landing, the E8
frontend schema already declared `truncated_reason` (matching
`SubgraphResponse`). Widening this inconsistency to a third field name
would compound the problem. Aligning with `SubgraphResponse` is the
minimum-friction choice and is consistent with the existing
`openspec/specs/graphlanding-affordances/spec.md` Requirement 2.

A separate refactor (cycle `e11-context-response-field-naming`) should
harmonise the two existing endpoints. Out of scope for this cycle.

### Decision: D-2 Cap value — `LANDING_NODE_CAP = 50` (constant, not configurable)

**Choice**: Hard-code the cap as a `pub const` in `dto.rs`.
**Alternatives considered**: Add the cap to the `Config` struct (or
similar runtime-configurable location).
**Rationale**: The landing UX is a "show me a glance of the workspace"
page. The cap is a UX choice (how many nodes fit on one screen at the
default zoom), not a performance knob. Hard-coding it makes the
contract explicit and the helper testable. If the cap becomes
configurable later, the `pub const` can become `pub fn` reading from
`Config` without breaking callers.

### Decision: D-3 Helper signature — `apply_landing_cap(total: usize) -> (bool, Option<String>)`

**Choice**: Pure function, single `total` parameter, returns a tuple.
**Alternatives considered**: Method on `LandingPayload` struct
(`impl LandingPayload { fn compute_truncation(&self) -> ... }`);
struct with named fields (`TruncationInfo { truncated, reason }`).
**Rationale**:
- A method on the struct couples the data shape to the policy. The
  helper today operates on a single number; future iterations may need
  more inputs (e.g., separate caps per collection). Keeping it as a
  free function makes extension easy.
- A struct would be cleaner but adds a new DTO surface for a single
  use site. KISS for now.
- A tuple is acceptable because the two values are conceptually a
  pair (the reason is meaningful only when `truncated` is `true`).

### Decision: D-4 Handler today calls helper with `0`

**Choice**: `landing_handler` calls `apply_landing_cap(0)` and assigns
the result to `LandingPayload.truncated` / `truncated_reason`.
**Alternatives considered**: Inline `false` / `None` literals; gate
the helper call behind a `cfg(feature = "real-landing-data")`.
**Rationale**: Inline literals drift from the spec (the spec says
"single source of truth"). The `cfg` gate adds a feature flag we
don't otherwise use. Calling the helper with `0` is a one-line change
that documents the contract and is trivially correct today
(`apply_landing_cap(0)` returns `(false, None)`).

### Decision: D-5 Strict TDD — tests land first

**Choice**: Write the test file `api_landing_truncation.rs` with all
8 scenarios, run `cargo test` to confirm RED, then implement the
DTO/helper/handler changes until GREEN.
**Rationale**: The cycle is small (≈50 LOC Rust + ≈100 LOC tests).
Strict TDD pays off here because:
- The tests document the contract.
- The tests are forward-compatible (they will catch the future
  `e10-landing-real-data` cycle's wiring regressions).
- The cycle is small enough that RED-then-GREEN is fast.

## Data Flow

```
[Future e10-landing-real-data]
state.graph.get_top_entry_points(LANDING_NODE_CAP)
  │
  ▼ Vec<InspectableObjectSummary> (count = total)
  │
  ▼ apply_landing_cap(total)
  │   if total <= LANDING_NODE_CAP: (false, None)
  │   else:                          (true, Some("node_cap"))
  │
  ▼ LandingPayload { ..., truncated, truncated_reason }
  │
  ▼ Json(payload).into_response()
```

```
[Today e8b]
landing_handler
  │
  ▼ apply_landing_cap(0)
  │   returns (false, None)
  │
  ▼ LandingPayload { ..., truncated: false, truncated_reason: None }
  │
  ▼ Json(payload).into_response()
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` (around `LandingPayload`) | Modify | +2 fields, +1 constant |
| `crates/cognicode-explorer/src/api.rs` (top-level + `landing_handler`) | Modify | +`apply_landing_cap` helper, handler uses it, TODO comment updated |
| `crates/cognicode-explorer/tests/api_landing_truncation.rs` | New | Strict TDD test suite (8 scenarios) |

## Interfaces / Contracts

```rust
// crates/cognicode-explorer/src/dto.rs
pub const LANDING_NODE_CAP: usize = 50;

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
    pub truncated: bool,                    // ← NEW
    pub truncated_reason: Option<String>,   // ← NEW
}
```

```rust
// crates/cognicode-explorer/src/api.rs
/// Pure helper. Single source of truth for the landing truncation policy.
pub(crate) fn apply_landing_cap(total: usize) -> (bool, Option<String>) {
    if total > LANDING_NODE_CAP {
        (true, Some("node_cap".to_string()))
    } else {
        (false, None)
    }
}

async fn landing_handler(
    State(state): State<ApiState>,
    Path(workspace_id): Path<String>,
) -> Result<Response, ApiError> {
    // ... (unchanged up to building the payload)

    let (truncated, truncated_reason) = apply_landing_cap(0);

    let payload = LandingPayload {
        // ... (unchanged fields)
        truncated,
        truncated_reason,
    };
    Ok(Json(payload).into_response())
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `apply_landing_cap` boundary at cap | Plain `#[test]` (sync) — pure function |
| Unit | `LandingPayload` serde — round-trip with `truncated: false` | Plain `#[test]` |
| Unit | `LandingPayload` serde — accepts `truncated: true, truncated_reason: Some("node_cap")` on deserialize | Plain `#[test]` |
| Unit | `LandingPayload` serde — accepts missing fields (legacy v0.24.1 server) via `.optional()` schema | Plain `#[test]` (only if there's a Rust schema validator; otherwise skip and rely on serde default behaviour) |
| Integration | Handler returns 200 with `truncated: false, truncated_reason: None` for an empty workspace | `#[tokio::test]` + `axum::Router` (matching `api_graph_tests.rs` pattern) |
| Integration | Handler test that ensures the JSON shape matches the frontend zod schema | Plain string comparison against expected JSON keys |
| Build | `cargo check --workspace --tests` | exit 0 |
| Lint | `cargo clippy --workspace --tests` | no new warnings vs baseline |
| E2E (frontend) | `just explorer-test`, `just explorer-build` | no regressions (no frontend change, but verify) |

## Migration / Rollout

No migration required. Pure additive Rust change. No DB schema, no
frontend change. Rollout:

1. PR lands → squash-merge to main.
2. Tag `v0.24.2` (PATCH).
3. The frontend banner remains dormant (handler still returns
   `truncated: false`). The future `e10-landing-real-data` cycle
   activates it.

## Open Questions

- [ ] Should the helper be `pub` (re-exported from the crate) or
      `pub(crate)` (private to `cognicode-explorer`)? Lean
      `pub(crate)` for now; promote to `pub` if `e10-landing-real-data`
      needs it from another crate. **Decision: `pub(crate)` for v0.24.2.**
- [ ] Should `LANDING_NODE_CAP` be a `u32` instead of `usize`? Lean
      `usize` to match `Vec::len()` return type. **Decision: `usize`.**
- [ ] The handler currently returns an empty stub. Is it worth wiring
      *just* `entry_points` in this cycle to actually exercise the
      cap? **Decision: No** — that is `e10-landing-real-data`, which
      needs an ADR about `InspectableObjectSummary` vs landing-specific
      summary struct. Keep this cycle to the contract only.
