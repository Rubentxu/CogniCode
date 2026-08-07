# Tasks: brain-federation

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~965 (new) + ~501 (modified) ≈ 1466 |
| 400-line budget risk | High |
| Chained PRs recommended | **Yes** — split into 3 chained PRs (one per batch) |
| Suggested split | Batch 1 → PR-1 (foundation, ~330 LOC). Batch 2 → PR-2 (federation + merge detection, ~580 LOC). Batch 3 → PR-3 (MCP tools + integration, ~556 LOC). |
| Delivery strategy | chained-prs |
| Chain strategy | review-slice |

Each batch is one PR. PRs are stacked: `brain-federation#1` (foundation) → `brain-federation#2` (federation service) → `brain-federation#3` (tools + integration). Reviewer can land PR-1 independently; PR-2 builds on PR-1's branch; PR-3 closes the chain. Rollback is per-PR.

### Suggested Work Units per PR

| PR | Batch | Work Units | Approx LOC | Files |
|----|-------|-----------|------------|-------|
| PR-1 | Batch 1 | 1.1 → 1.5 | ~330 | 4 new + 2 modified |
| PR-2 | Batch 2 | 2.1 → 2.6 | ~580 | 5 new + 2 modified |
| PR-3 | Batch 3 | 3.1 → 3.6 | ~556 | 3 new + 3 modified |

Each work unit is one commit. Apply phase MUST commit in this order; tests live alongside their unit (TDD RED→GREEN pairs are not split across commits).

---

## Batch 1: Foundation — Space model + FederatedNodeId + PG schema (TDD RED→GREEN)

> Goal: Define `Space`, `SpaceId`, `SpaceKind`, `FederatedNodeId`, the PG `spaces` migration, and the `SpaceRegistry`. No brain tools, no federation queries — pure data + DB. This is the dependency floor for everything else.

### Task 1.1 — SpaceId + SpaceKind + Space value objects (RED→GREEN)

- **Batch**: 1
- **Deps**: none
- **Spec reqs**: `federated-spaces` §`SpaceId Non-Empty and Opaque`, §`SpaceKind Exactly 3 Variants`, §`Space Value Object`
- **RED gate**: `cargo test -p cognicode-core space::space_id` fails (no `space_id.rs` exists)
- **Files**: `crates/cognicode-core/src/domain/value_objects/space_id.rs` (create, ~50 LOC), `crates/cognicode-core/src/domain/value_objects/space.rs` (create, ~80 LOC), `crates/cognicode-core/src/domain/value_objects/mod.rs` (modify, +2)
- **LOC est.**: ~135
- **Test cases** (RED): `space_id_try_new_empty_returns_err`, `space_id_try_new_non_empty_succeeds`, `space_id_default_constant`, `space_kind_repo_docs_issues_roundtrip`, `space_try_new_with_name_and_kind`, `space_try_new_empty_name_returns_err`, `space_try_new_defaults_config_to_empty_object`
- **GREEN**: implement `SpaceId::try_new`, `Space::try_new`, `SpaceKind` enum (3 variants), `SpaceError` enum
- **Validation**: `cargo test -p cognicode-core --features multimodal` passes; `cargo build -p cognicode-core --no-default-features` clean

### Task 1.2 — spaces PG migration (RED→GREEN)

- **Batch**: 1
- **Deps**: 1.1
- **Spec reqs**: `federated-spaces` §`spaces PG Table`
- **RED gate**: PG test fails because `spaces` table doesn't exist; assert `SELECT * FROM spaces` returns 0 rows
- **Files**: `crates/cognicode-core/src/infrastructure/persistence/spaces_migration.rs` (create, ~60 LOC), `crates/cognicode-core/src/infrastructure/persistence/mod.rs` (modify, +1), `schema_postgres.sql` (modify, +15)
- **LOC est.**: ~80
- **Test cases**: `spaces_table_creation_succeeds`, `default_space_row_is_seeded`, `spaces_kind_check_constraint_rejects_invalid_kind`
- **GREEN**: write the migration SQL; register it in the migration list; add a default-row INSERT with `ON CONFLICT DO NOTHING`
- **Validation**: integration test against a real PG instance creates the table and seeds the row

### Task 1.3 — graph_nodes.space_id additive column (RED→GREEN)

- **Batch**: 1
- **Deps**: 1.2
- **Spec reqs**: `federated-spaces` §`spaces PG Table` (additive column) + entropy budget
- **RED gate**: PG test fails because `graph_nodes.space_id` column does not exist
- **Files**: `crates/cognicode-core/src/infrastructure/persistence/graph_nodes_space_id_migration.rs` (create, ~50 LOC), `schema_postgres.sql` (modify, +3)
- **LOC est.**: ~55
- **Test cases**: `graph_nodes_space_id_column_exists_with_default`, `existing_rows_backfilled_to_default`, `idx_graph_nodes_space_id_index_created`
- **GREEN**: `ALTER TABLE graph_nodes ADD COLUMN space_id TEXT NOT NULL DEFAULT 'default'; CREATE INDEX idx_graph_nodes_space_id ON graph_nodes(space_id);`
- **Validation**: integration test confirms the column exists, has the default, and the index is created

### Task 1.4 — FederatedNodeId newtype (RED→GREEN)

- **Batch**: 1
- **Deps**: 1.1
- **Spec reqs**: `federated-graph-service` §`FederatedNodeId Format`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal federation::federated_node_id` fails (module missing)
- **Files**: `crates/cognicode-explorer/src/federation/mod.rs` (create, ~15 LOC), `crates/cognicode-explorer/src/federation/federated_node_id.rs` (create, ~120 LOC)
- **LOC est.**: ~135
- **Test cases**: `federated_node_id_try_new_valid_format_succeeds`, `federated_node_id_try_new_missing_separator_returns_err`, `federated_node_id_try_new_empty_space_segment_returns_err`, `federated_node_id_try_new_empty_local_segment_returns_err`, `federated_node_id_space_id_returns_left_of_separator`, `federated_node_id_local_id_returns_right_of_separator`, `federated_node_id_display_prints_inner_string`
- **GREEN**: implement `FederatedNodeId::try_new` (parses `::` once), `space_id()`, `local_id()`, `Display`. Reject empty segments and the `::` substring in either half.
- **Validation**: `cargo test -p cognicode-explorer --features multimodal federation::federated_node_id` passes; `cargo build -p cognicode-explorer --no-default-features` clean (no federation module)

### Task 1.5 — SpaceRegistry in-memory CRUD (RED→GREEN)

- **Batch**: 1
- **Deps**: 1.1
- **Spec reqs**: `federated-spaces` §`SpaceRegistry In-Memory CRUD`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal federation::space_registry` fails
- **Files**: `crates/cognicode-explorer/src/federation/space_registry.rs` (create, ~120 LOC), `crates/cognicode-explorer/src/federation/mod.rs` (modify, +1)
- **LOC est.**: ~125
- **Test cases**: `space_registry_register_and_get`, `space_registry_register_duplicate_returns_err`, `space_registry_get_unknown_returns_none`, `space_registry_list_preserves_insertion_order`, `space_registry_unregister_existing_returns_true`, `space_registry_unregister_unknown_returns_false`, `space_registry_sequential_registers_in_order`
- **GREEN**: implement `SpaceRegistry` with `register(Space) -> Result<SpaceId, SpaceError>`, `get(SpaceId) -> Option<Space>`, `list() -> Vec<Space>` (insertion order), `unregister(SpaceId) -> bool`
- **Validation**: `cargo test -p cognicode-explorer --features multimodal federation::space_registry` passes all 7 tests; `cargo build -p cognicode-explorer --features multimodal` clean

---

## Batch 2: Federation service + merge detection + session extension (TDD RED→GREEN)

> Goal: `FederatedGraphService` (multiplexes per-space repos), `MergeDetector` (heuristic + scoring), `BrainSessionState.spaces` field, `BrainSessionService.add_space/remove_space`, and the routing in `ask_with_session`. No MCP tools yet — that's Batch 3.

### Task 2.1 — FederatedNode wrapper (RED→GREEN)

- **Batch**: 2
- **Deps**: 1.4
- **Spec reqs**: `federated-graph-service` §`FederatedNode Wrapper`, §`Per-Space Namespace Isolation`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal federation::federated_node` fails
- **Files**: `crates/cognicode-explorer/src/federation/federated_node.rs` (create, ~80 LOC), `crates/cognicode-explorer/src/federation/mod.rs` (modify, +1)
- **LOC est.**: ~85
- **Test cases**: `federated_node_constructs_with_node_and_space_id`, `federated_node_federated_id_joins_space_id_and_local_id_with_separator`, `federated_node_display_prints_federated_id`, `federated_node_local_id_is_unprefixed` (regression: `node.node.id.0` MUST NOT contain `::`)
- **GREEN**: implement `FederatedNode::new(node, space_id)`, `federated_id()` (joins with `"::"`), `Display` impl
- **Validation**: `cargo test -p cognicode-explorer --features multimodal federation::federated_node` passes

### Task 2.2 — FederatedGraphService construction + add_space (RED→GREEN)

- **Batch**: 2
- **Deps**: 2.1
- **Spec reqs**: `federated-graph-service` §`FederatedGraphService Construction`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal federation::federated_graph_service::construction` fails
- **Files**: `crates/cognicode-explorer/src/federation/federated_graph_service.rs` (create, ~150 LOC first cut), `crates/cognicode-explorer/src/federation/mod.rs` (modify, +1)
- **LOC est.**: ~155
- **Test cases**: `federated_graph_service_new_is_empty`, `federated_graph_service_add_space_registers_id`, `federated_graph_service_add_space_is_idempotent`, `federated_graph_service_spaces_preserves_insertion_order`, `federated_graph_service_spaces_returns_cloned_vec`
- **GREEN**: implement `FederatedGraphService::new`, `add_space`, `spaces()`. Internally a `HashMap<SpaceId, Arc<dyn GraphRepository>>` plus a `Vec<SpaceId>` for insertion order.
- **Validation**: passes all 5 tests; `cargo build -p cognicode-explorer --features multimodal` clean

### Task 2.3 — FederatedGraphService::federated_search + get_node + find_outgoing_edges (RED→GREEN)

- **Batch**: 2
- **Deps**: 2.2
- **Spec reqs**: `federated-graph-service` §`Federated Search`, §`Federated Node Lookup`, §`Per-Space Namespace Isolation`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal federation::federated_graph_service::search` fails
- **Files**: `crates/cognicode-explorer/src/federation/federated_graph_service.rs` (modify, +~130 LOC)
- **LOC est.**: ~135
- **Test cases**: `federated_search_merges_results_from_two_spaces`, `federated_search_empty_service_returns_empty_page`, `federated_search_tags_every_item_with_space_id`, `federated_search_fans_out_in_parallel`, `federated_get_node_routes_to_correct_space`, `federated_get_node_unknown_space_returns_none`, `federated_get_node_unknown_local_id_returns_none`, `federated_find_outgoing_edges_uses_correct_space`
- **GREEN**: implement async `federated_search` (uses `tokio::join_all` over the repos), `get_node` (parses `FederatedNodeId` → routes), `find_outgoing_edges`. Use a mock `GraphRepository` in tests.
- **Validation**: passes all 8 tests; integration test with 2 mock repos returns merged results

### Task 2.4 — MergeCandidate + MergeReason types (RED→GREEN)

- **Batch**: 2
- **Deps**: 2.1
- **Spec reqs**: `merge-candidate-detection` §`Reason Trace`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal federation::merge_candidate` fails
- **Files**: `crates/cognicode-explorer/src/federation/merge_candidate.rs` (create, ~70 LOC), `crates/cognicode-explorer/src/federation/mod.rs` (modify, +1)
- **LOC est.**: ~75
- **Test cases**: `merge_candidate_constructs_with_left_right_confidence_reasons`, `merge_reason_label_match_kind_match_property_overlap_variants_construct`, `merge_candidate_confidence_clamps_to_1_when_components_exceed`
- **GREEN**: implement `MergeCandidate { left, right, confidence, reasons }`, `MergeReason` (3 variants, `#[non_exhaustive]`)
- **Validation**: passes all 3 tests

### Task 2.5 — MergeDetector::detect (RED→GREEN)

- **Batch**: 2
- **Deps**: 2.4
- **Spec reqs**: `merge-candidate-detection` §`Label Normalization`, §`Heuristic Scoring`, §`Threshold Filter`, §`O(N²) Brute-Force Detection`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal federation::merge_detector` fails
- **Files**: `crates/cognicode-explorer/src/federation/merge_detector.rs` (create, ~150 LOC), `crates/cognicode-explorer/src/federation/mod.rs` (modify, +1)
- **LOC est.**: ~155
- **Test cases**: `label_normalization_lowercases_and_strips_whitespace`, `label_normalization_preserves_hyphens`, `scoring_base_only_returns_0_5`, `scoring_label_only_returns_0_8`, `scoring_kind_only_returns_0_7`, `scoring_full_match_returns_1_0_capped`, `scoring_caps_at_1_0_when_property_overlap_fires`, `same_space_pair_filtered_out`, `below_threshold_excluded`, `empty_input_returns_empty_vec`, `reasons_populated_for_label_only_match`, `reasons_populated_for_full_match`, `three_space_cluster_produces_three_pairs`
- **GREEN**: implement `MergeDetector::detect(&[FederatedNode]) -> Vec<MergeCandidate>` with the scoring table (base 0.5 + label 0.3 + kind 0.2 + property 0.1 cap 1.0), threshold filter at 0.8, label normalization (lowercase, trim, collapse whitespace, strip surrounding punctuation), and the same-space filter
- **Validation**: passes all 13 tests; small-N timing test (100 nodes < 50ms)

### Task 2.6 — BrainSessionState.spaces + BrainSessionService federation methods (RED→GREEN)

- **Batch**: 2
- **Deps**: 1.1, 2.2
- **Spec reqs**: `brain-session` §MODIFIED §`Session State Model`, §MODIFIED §`Session Service`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal session::state` and `session::service` fail
- **Files**: `crates/cognicode-explorer/src/session/state.rs` (modify, +~15 LOC), `crates/cognicode-explorer/src/session/service.rs` (modify, +~80 LOC)
- **LOC est.**: ~100
- **Test cases**: `brain_session_state_spaces_defaults_to_empty_vec`, `brain_session_state_spaces_roundtrips_via_serde`, `brain_session_state_spaces_empty_serializes_as_not_null`, `service_add_space_stores_space`, `service_add_space_rejects_duplicate_id`, `service_remove_space_existing_returns_true`, `service_remove_space_unknown_returns_false`, `service_spaces_returns_registered_spaces_in_order`, `service_federated_graph_returns_arc_clone`
- **GREEN**: add `spaces: Vec<SpaceId>` field to `BrainSessionState` (with `#[serde(default)]`); modify `BrainSessionService` to hold `Arc<FederatedGraphService>` (constructed in `new`); add `add_space(Space) -> Result<(), SpaceError>`, `remove_space(SpaceId) -> bool`, `spaces() -> Vec<Space>`, `federated_graph() -> Arc<FederatedGraphService>`
- **Validation**: all 9 tests pass; `cargo test -p cognicode-explorer --features multimodal session::` clean

---

## Batch 3: Brain tools + routing + integration (TDD RED→GREEN)

> Goal: 3 new MCP tools, `brain_open` extension for `spaces`, `brain_status` extension, `ask_with_session` federation routing, end-to-end integration tests, regression sweep.

### Task 3.1 — brain_add_space tool constants + arg struct + dispatch arm (RED→GREEN)

- **Batch**: 3
- **Deps**: 2.6
- **Spec reqs**: `brain-space-tools` §`brain_add_space`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal mcp` fails
- **Files**: `crates/cognicode-explorer/src/mcp.rs` (modify, +~120 LOC)
- **LOC est.**: ~125
- **Test cases**: `brain_add_space_dispatches`, `brain_add_space_new_space_returns_space_count_after_1`, `brain_add_space_duplicate_returns_duplicate_space_error`, `brain_add_space_invalid_kind_returns_invalid_space_kind_error`, `brain_add_space_unknown_session_returns_session_not_found`, `brain_add_space_id_with_double_colon_returns_invalid_space_id`, `brain_add_space_provenance_source_is_brain_session`
- **GREEN**: add `TOOL_BRAIN_ADD_SPACE` constant, append to `TOOL_NAMES` (24→25), add `BrainAddSpaceArgs` struct, add dispatch arm that calls `service.add_space(space)`, add JSON-Schema in `build_tool_schemas()`
- **Validation**: passes all 7 tests; `TOOL_NAMES.len() == 25` regression test passes

### Task 3.2 — brain_remove_space + brain_spaces tools (RED→GREEN)

- **Batch**: 3
- **Deps**: 3.1
- **Spec reqs**: `brain-space-tools` §`brain_remove_space`, §`brain_spaces`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal mcp` fails on the 2 new tools
- **Files**: `crates/cognicode-explorer/src/mcp.rs` (modify, +~180 LOC)
- **LOC est.**: ~185
- **Test cases**: `brain_remove_space_dispatches`, `brain_remove_space_existing_returns_removed_true`, `brain_remove_space_unknown_returns_removed_false_not_error`, `brain_remove_space_unknown_session_returns_session_not_found`, `brain_remove_space_provenance_source_is_brain_session`, `brain_spaces_dispatches`, `brain_spaces_lists_registered_spaces`, `brain_spaces_empty_session_returns_empty_arrays_not_null`, `brain_spaces_unknown_session_returns_session_not_found`, `brain_spaces_returns_merge_candidates_when_two_spaces_have_matching_nodes`, `brain_spaces_single_space_returns_empty_merge_candidates`
- **GREEN**: add `TOOL_BRAIN_REMOVE_SPACE` and `TOOL_BRAIN_SPACES` constants, `TOOL_NAMES` 25→27, two more arg structs, two dispatch arms (`brain_remove_space` calls `service.remove_space`; `brain_spaces` calls `service.spaces()` AND `federated_graph.detect_merge_candidates()`), two JSON-Schemas
- **Validation**: passes all 11 tests; `TOOL_NAMES.len() == 27` regression test passes

### Task 3.3 — brain_open spaces extension (RED→GREEN)

- **Batch**: 3
- **Deps**: 3.1, 2.6
- **Spec reqs**: `brain-space-tools` §`brain_open Extension`, `brain-session` §ADDED §`brain_open spaces`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal mcp` fails on the extended open
- **Files**: `crates/cognicode-explorer/src/mcp.rs` (modify, +~30 LOC), `crates/cognicode-explorer/src/session/registry.rs` (modify, +~40 LOC)
- **LOC est.**: ~75
- **Test cases**: `brain_open_with_spaces_pre_registers_them`, `brain_open_without_spaces_preserves_existing_behavior_byte_for_byte`, `brain_open_with_empty_spaces_array_preserves_existing_behavior`, `brain_open_with_duplicate_space_id_returns_duplicate_space_error`
- **GREEN**: extend `BrainOpenArgs` with optional `spaces: Vec<SpaceSpec>`; the dispatch arm calls `service.add_space()` for each entry BEFORE returning; `SessionRegistry::open` gains an optional `Vec<SpaceSpec>` parameter (default empty)
- **Validation**: passes all 4 tests; backward-compat regression (all existing `brain_open` tests still pass) holds

### Task 3.4 — brain_status space summary extension (RED→GREEN)

- **Batch**: 3
- **Deps**: 3.3
- **Spec reqs**: `brain-space-tools` §`brain_status Extension`, `brain-session` §ADDED §`brain_status spaces`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal mcp` fails
- **Files**: `crates/cognicode-explorer/src/mcp.rs` (modify, +~15 LOC)
- **LOC est.**: ~20
- **Test cases**: `brain_status_includes_space_count_and_spaces_array`, `brain_status_empty_session_reports_zero_spaces`, `brain_status_existing_fields_unchanged` (regression: `session_id`, `history`, `focus_node` are still there)
- **GREEN**: extend the `TOOL_BRAIN_STATUS` dispatch arm to include `space_count: usize` and `spaces: [{id, name, kind}]` in the payload
- **Validation**: passes all 3 tests; `brain_status` backward-compat test (no `spaces` in state → `space_count: 0`) holds

### Task 3.5 — ask_with_session federation routing (RED→GREEN)

- **Batch**: 3
- **Deps**: 2.3, 2.6
- **Spec reqs**: `brain-session` §MODIFIED §`brain_ask (Federation)`
- **RED gate**: `cargo test -p cognicode-explorer --features multimodal session::service` fails
- **Files**: `crates/cognicode-explorer/src/session/service.rs` (modify, +~50 LOC)
- **LOC est.**: ~55
- **Test cases**: `ask_with_session_empty_spaces_uses_legacy_dispatch_path_byte_for_byte`, `ask_with_session_multi_space_tags_results_with_space_id`, `ask_with_session_multi_space_preserves_focus_injection`, `ask_with_session_history_appended_on_success_in_either_path`
- **GREEN**: when `state.spaces.is_empty()`, route through the existing `dispatch_ask(classified, &self.service, &self.graph, None).await` (byte-for-byte unchanged). When `state.spaces` is non-empty, route through `self.federated.federated_search(...)`, build a synthetic envelope, and tag each result with its `space_id`.
- **Validation**: passes all 4 tests; the byte-for-byte regression test holds (use a `serde_json::Value` comparison against a frozen baseline)

### Task 3.6 — Integration tests + multimodal frontend + final wiring (RED→GREEN)

- **Batch**: 3
- **Deps**: 3.1, 3.2, 3.3, 3.4, 3.5
- **Spec reqs**: end-to-end federation lifecycle, multimodal feature gate, regression sweep
- **RED gate**: integration test fails because the lifecycle doesn't work end-to-end
- **Files**: `crates/cognicode-explorer/tests/brain_federation_lifecycle.rs` (create, ~150 LOC), `crates/cognicode-explorer/src/lib.rs` (modify, +1 for `pub mod federation;`), `apps/explorer-ui/src/` (modify, +space badge component, ~80 LOC)
- **LOC est.**: ~235
- **Test cases**: `open_with_spaces_add_remove_spaces_lifecycle`, `brain_ask_across_two_spaces_returns_merged_results_with_space_id`, `merge_candidates_appear_in_brain_spaces_for_overlapping_labels`, `cargo_build_no_default_features_compiles_cleanly`, `cargo_build_with_multimodal_compiles_and_runs_all_federation_tests`
- **GREEN**: write the lifecycle integration test using the existing `call_tool_args` helper; add the `pub mod federation;` line (gated by `multimodal`); add a small "space badge" React/JSX component in the frontend that shows `kind` + `name` for each space in the current session
- **Validation**: full workspace test suite passes:
  - `cargo test --workspace --all-features` — zero regressions (18 + 6 + 3 = 27 brain tools, all green)
  - `cargo build --workspace --no-default-features` — clean (federation is hidden)
  - `cargo build --workspace --features multimodal` — clean
  - `cargo clippy --workspace --features multimodal -- -D warnings` — clean
  - `cargo fmt --all -- --check` — clean
  - `grep -c "TOOL_BRAIN" crates/cognicode-explorer/src/mcp.rs` — 9 brain constants (6 existing + 3 new)
  - frontend test: `pnpm test` in `apps/explorer-ui/` — space badge renders 2 badges for a 2-space session

---

## Validation Summary (run after each task)

| Task | Command | Expected |
|------|---------|----------|
| 1.1 | `cargo test -p cognicode-core --features multimodal space::` | RED → GREEN |
| 1.2 | PG integration test (spaces table) | RED → GREEN |
| 1.3 | PG integration test (graph_nodes.space_id) | RED → GREEN |
| 1.4 | `cargo test -p cognicode-explorer --features multimodal federation::federated_node_id` | RED → GREEN |
| 1.5 | `cargo test -p cognicode-explorer --features multimodal federation::space_registry` | RED → GREEN |
| 2.1 | `cargo test -p cognicode-explorer --features multimodal federation::federated_node` | RED → GREEN |
| 2.2 | `cargo test -p cognicode-explorer --features multimodal federation::federated_graph_service::construction` | RED → GREEN |
| 2.3 | `cargo test -p cognicode-explorer --features multimodal federation::federated_graph_service` | RED → GREEN |
| 2.4 | `cargo test -p cognicode-explorer --features multimodal federation::merge_candidate` | RED → GREEN |
| 2.5 | `cargo test -p cognicode-explorer --features multimodal federation::merge_detector` | RED → GREEN |
| 2.6 | `cargo test -p cognicode-explorer --features multimodal session::` | RED → GREEN |
| 3.1 | `cargo test -p cognicode-explorer --features multimodal mcp::tool_names_has_twenty_five_entries` (was 24) | 25 |
| 3.2 | `cargo test -p cognicode-explorer --features multimodal mcp::tool_names_has_twenty_seven_entries` | 27 |
| 3.3 | `cargo test -p cognicode-explorer --features multimodal mcp` (brain_open with/without spaces) | All 4 tests pass; existing tests stay green |
| 3.4 | `cargo test -p cognicode-explorer --features multimodal mcp` (brain_status extension) | All 3 tests pass |
| 3.5 | `cargo test -p cognicode-explorer --features multimodal session::service` | All 4 tests pass; byte-for-byte regression holds |
| 3.6 | `cargo test --workspace --all-features` | Zero regressions |

## Dependency Graph

```
Batch 1:
  1.1 (Space types) ─→ 1.2 (PG migration) ─→ 1.3 (space_id column)
       └─→ 1.4 (FederatedNodeId) ─→ 1.5 (SpaceRegistry)
                                              │
Batch 2:                                     ▼
  1.4 ─→ 2.1 (FederatedNode) ─→ 2.2 (FederatedGraphService construction) ─→ 2.3 (search/get_node/edges)
                                    │                                              │
                                    └─→ 2.4 (MergeCandidate) ─→ 2.5 (MergeDetector)
                                                                                  │
  1.1, 2.2 ─→ 2.6 (BrainSessionState.spaces + service methods)                     │
                                                                                  │
Batch 3:                                                                           │
  2.6 ─→ 3.1 (brain_add_space) ─→ 3.2 (brain_remove_space + brain_spaces)         │
        2.6, 3.1 ─→ 3.3 (brain_open extension)                                     │
        3.3 ─→ 3.4 (brain_status extension)                                        │
        2.3, 2.6 ─→ 3.5 (ask_with_session federation routing)                     │
        3.1, 3.2, 3.3, 3.4, 3.5 ─→ 3.6 (integration + frontend + sweep)          │
```

Batch 1 tasks 1.4 and 1.5 can run in parallel with 1.2 / 1.3 (different files). Batch 2 starts only after Batch 1 finishes. Batch 3 starts only after Batch 2 finishes (the federation service must exist before the routing can be implemented).

## TDD Discipline

For every task tagged **RED**, the test MUST be written and the failure confirmed (`cargo test` exits non-zero) BEFORE the **GREEN** task begins. Do not combine RED+GREEN into one commit. The work-unit commits in the forecast table above are GREEN commits only — RED tests live in the same commit as the GREEN implementation but the diff is structured so reviewers see the failing test addition first.

## Out of Scope (locked by spec)

- Auto-federation
- Cross-space edge creation
- Federated crate separation
- Space persistence across sessions
- Streamed merge-candidate computation
- UI for in-session merge candidate review (only the `brain_spaces` payload is the v1 surface)
- Authorization / RBAC on spaces
