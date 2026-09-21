# Tasks: e12g-risk-map

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~470–610 (adapter ~220, executor+wiring ~250, optional REST ~140) |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR 1 (adapter + compute_risk) → PR 2 (executor + registry) → PR 3 (REST, optional) |
| Delivery strategy | auto-chain |
| Chain strategy | stacked-to-main |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | QualityGraphRepository adapter + shared `compute_risk` helper | PR 1 | Foundational; no executor touches it yet |
| 2 | RiskMapExecutor + registry wiring + unit tests | PR 2 | Stacks on PR 1; graph-shaped view live |
| 3 | `GET /api/quality/hotspots` REST handler + integration tests | PR 3 | Optional per spec (MAY); keeps core PRs reviewable |

## Phase 1: Foundation (PR 1)

- [ ] 1.1 Create `crates/cognicode-explorer/src/adapters/quality_graph_repository.rs` with module skeleton + re-export in `adapters/mod.rs`.
- [ ] 1.2 In `crates/cognicode-explorer/src/domain/lenses/hotspots.rs`, expose `pub fn compute_risk(fan_in: u32, weighted_issue_count: f32) -> f32 = fan_in * 0.4 + weighted_issue_count * 0.6`; refactor `HotspotsLens::symbol_risk` to call it.
- [ ] 1.3 Implement `QualityGraphRepository::new(qr: Option<&dyn QualityRepository>, gq: &dyn GraphQueryPort)` with `rank_hotspots(target, limit) -> ExplorerResult<Vec<HotspotNode>>` (returns `ExplorerError::QualityUnavailable` when `qr.is_none()` and quality data requested) and `traverse_from(symbol_id, filter) -> ExplorerResult<Vec<RelEdge>>` preserving `provenance` + `confidence`.
- [ ] 1.4 RED→GREEN unit tests in same file: ranked-by-risk-desc, max 5, fan-in-only when no issues, missing quality record excluded, `QualityUnavailable` classified error, traversal edges retain provenance/confidence.

## Phase 2: Core View (PR 2)

- [ ] 2.1 In `crates/cognicode-explorer/src/domain/views.rs`, add `pub struct RiskMapExecutor;` with `ViewDescriptor` (`id="risk_map"`, `title="Risk Map"`, `applies_to=[Symbol,File,Scope]`, `view_kind=ViewKind::RiskMap`, `renderer_kind=RendererKind::Graph`).
- [ ] 2.2 Implement `ViewExecutor::build` for `RiskMapExecutor`: build target via existing inspection resolver, call `QualityGraphRepository::rank_hotspots(limit=5)`, emit each as graph node with `properties: {fan_in, weighted_issue_count, risk}`, attach edges from `traverse_from`, omit unresolvable edges with `ViewDiagnostic::RelationUnresolved`, return graph-shaped `ContextualView`.
- [ ] 2.3 RED→GREEN unit tests in `views.rs` `#[cfg(test)] mod risk_map`: ranked output, ≤5 desc, no-quality → fan-in only with non-zero risk, unresolvable relation → hotspot retained + diagnostic present.
- [ ] 2.4 Add `pub static RISK_MAP_EXECUTOR: RiskMapExecutor = RiskMapExecutor;` in `views.rs` and extend `ViewRegistry::get_executor` match in `crates/cognicode-explorer/src/registry.rs` to return `&RISK_MAP_EXECUTOR` for `ViewKind::RiskMap`.
- [ ] 2.5 Gate multimodal: in `RiskMapExecutor::build`, read feature flag (env `COGNICODE_MULTIMODAL_QUALITY` or config); when disabled, skip `weighted_issue_count` enrichment and use topology-only path; document the constant.

## Phase 3: REST API (PR 3, optional)

- [ ] 3.1 RED integration test at `crates/cognicode-explorer/tests/risk_map_api.rs` asserting `GET /api/quality/hotspots?scope=...&limit=N` returns ranked hotspot JSON; `limit=0` or unknown scope → 400.
- [ ] 3.2 Add `pub async fn risk_map_hotspots(...)` handler in `crates/cognicode-explorer/src/api.rs` reusing `QualityGraphRepository::rank_hotspots`; validate `limit ∈ [1,5]` and scope; return `ApiError::BadRequest` on invalid input without partial queries; register route in router builder.

## Phase 4: Verification

- [ ] 4.1 Run `cargo test --workspace` (unit + integration); confirm new tests pass and no regression in `HotspotsLens`/`HotspotsExecutor`.
- [ ] 4.2 Run `cargo clippy --workspace -- -D warnings` (warning tolerance per launch plan); fix any new lints.
- [ ] 4.3 Run `cargo build --release --workspace`; persist build log to artifact metadata.
- [ ] 4.4 Save `tasks.md` outcome to Engram under topic `sddk/e12g-risk-map/tasks` with PR unit summary + measured changed lines per PR (feeds `sddk-apply` and `sddk-verify`).