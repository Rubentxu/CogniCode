# Proposal: brain-federation

> Full proposal content persisted in Engram topic `sdd/brain-federation/proposal` (observation #1541). This file mirrors the structural sections required by OpenSpec.

## Intent

The brain session system binds to a single workspace/graph. Federation enables multiple **spaces** (repos, docs sources, issue trackers) to appear as one navigable graph with merge candidate detection for cross-space entities.

## Capabilities

### New Capabilities
- `federated-spaces` — `Space` value object, `SpaceId` newtype, `SpaceKind` enum, PG `spaces` table
- `federated-graph-service` — `FederatedGraphService` merging per-space `GraphRepository` instances, `FederatedNode` wrapper, federated search/ask
- `merge-candidate-detection` — Cross-space same-label+kind heuristic, `MergeCandidate` entity, confidence scoring
- `brain-space-tools` — `brain_add_space`, `brain_remove_space`, `brain_spaces` MCP tools; `brain_open` / `brain_status` extensions

### Modified Capabilities
- `brain-session` — `BrainSessionState` gains `Vec<Space>`, `BrainSessionService` holds `FederatedGraphService` instead of single graph

## Approach

`Space` value object → `FederatedGraphService` over per-space repos → `FederatedNode { GraphNode, SpaceId }` wrapper → 3 new brain tools for runtime space management → "suggest, don't merge" UX for cross-space entity candidates. Federation layer lives in `cognicode-explorer/src/federation/`; no new crate.

## Out of Scope

Auto-federation, cross-space edge creation, space sync/refresh from remote sources, federated crate separation.

## Backward Compatibility

`brain_open` without `spaces[]` works exactly as today (default space). All 6 existing brain tools and 18 one-shot tools unchanged. PG migration is additive (nullable `space_id`, default `"default"`).

## Rollback Plan

Drop `spaces` table, drop `space_id` column from `graph_nodes`, revert `mcp.rs`/registry/state changes, delete `federation/` directory. All changes additive.

## Success Criteria

- [ ] `brain_open(spaces[])` creates a session with multiple spaces
- [ ] `brain_add_space` / `brain_remove_space` mutate a running session
- [ ] `brain_ask` returns results from all spaces with `space_id` tags
- [ ] Merge candidates detected and returned as suggestions
- [ ] `brain_open({})` behaves identically to current behavior
- [ ] PG migration non-destructive (existing data → default space)
