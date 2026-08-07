# Relation Candidates v1

> Source: SDDK cycle `relation-candidates-v1` — archived `YYYY-MM-DD`
> Branch: `feat/relation-candidates-v1` @ `e0de336`

## Capability: `relation-candidates`

Suggest potential callers for symbols that have no incoming `Calls` edges (dead or orphaned symbols).

---

## Requirements

### Requirement: Gated on Zero Incoming Calls

Symbols with at least one incoming `Calls` edge SHALL NOT produce relation candidates.

**Rationale**: A symbol already called is not a candidate for dead-code investigation.

---

### Requirement: Three Confidence Heuristics

The system SHALL rank candidate callers across three confidence tiers:

| Heuristic | Confidence | Condition |
|-----------|------------|-----------|
| `same_file` | 0.7 | Candidate shares the dead symbol's source file |
| `same_community` | 0.5 | Candidate shares the dead symbol's community (Label Propagation) |
| `name_match` | 0.3 | Candidate's name tokens intersect dead symbol's name tokens (token len ≥ 3) |

**Rationale**: Spatial and lexical locality provide probabilistic signal for missing call edges.

---

### Requirement: Deduplicated by Symbol ID

For a given dead symbol, the output SHALL contain at most one `RelationCandidate` per `SymbolId`. When multiple heuristics suggest the same symbol, the highest confidence wins.

**Rationale**: Ambiguous suggestions reduce trust; dedup surfaces the strongest signal.

---

### Requirement: Output Shape

Each `RelationCandidate` SHALL contain:

| Field | Type | Description |
|-------|------|-------------|
| `symbol_id` | `String` | Resolved symbol identifier |
| `confidence` | `f32` | 0.7 \| 0.5 \| 0.3 |
| `reason` | `String` | `"same_file"` \| `"same_community"` \| `"name_similarity"` |
| `direction` | `String` | Fixed `"incoming"` (v1) |

**Rationale**: Direction field reserved for forward-compatibility when outgoing candidates are supported.

---

## Scenarios

### Scenario: Same-File Heuristic

**Given** a dead symbol `foo` at `src/utils.rs:12`
**And** `bar` and `baz` are other functions in `src/utils.rs`
**When** `suggest_relation_candidates(foo)` is called
**Then** the result SHALL include `bar` and `baz` with `confidence = 0.7` and `reason = "same_file"`

---

### Scenario: Same-Community Heuristic

**Given** a dead symbol `create_user` in community `42`
**And** `user_service` and `auth_handler` are in community `42`
**When** `suggest_relation_candidates(create_user)` is called
**Then** the result SHALL include `user_service` and `auth_handler` with `confidence = 0.5` and `reason = "same_community"`

---

### Scenario: Name-Match Heuristic

**Given** a dead symbol `create_user`
**And** `user_repository` and `get_user` exist in the graph
**When** `suggest_relation_candidates(create_user)` is called
**Then** the result SHALL include candidates whose tokenized names share at least one token ≥ 3 chars with `create_user`'s tokens

---

### Scenario: Multi-Heuristic Dedup

**Given** a dead symbol `process_order` at `src/biz.rs:5` in community `7`
**And** `order_handler` is in `src/biz.rs` AND in community `7`
**When** `suggest_relation_candidates(process_order)` is called
**Then** the result SHALL contain exactly one entry for `order_handler` with `confidence = 0.7` (same_file wins over same_community)

---

### Scenario: Dead Symbol With Callers Returns Empty

**Given** a symbol `alive` that has at least one incoming `Calls` edge
**When** `suggest_relation_candidates(alive)` is called
**Then** the result SHALL be empty

---

### Scenario: Unknown Symbol Returns Empty

**Given** a `symbol_id` that does not exist in the graph
**When** `suggest_relation_candidates(unknown_id)` is called
**Then** the result SHALL be empty

---

### Scenario: Results Sorted by Confidence Descending

**Given** a dead symbol `orphaned` with same_file, same_community, and name_match candidates
**When** `suggest_relation_candidates(orphaned)` is called
**Then** results SHALL be sorted by `confidence` descending (0.7 before 0.5 before 0.3)

---

### Scenario: Dead Symbol Excluded From Results

**Given** a dead symbol `dead_fn`
**When** `suggest_relation_candidates(dead_fn)` is called
**Then** `dead_fn` SHALL NOT appear in the result set

---

### Scenario: Tokenization Handles Mixed Case

**Given** a dead symbol `createUserAccount`
**And** candidate `create_user_record` in the graph
**When** `suggest_relation_candidates(createUserAccount)` is called
**Then** tokenization SHALL split on camelCase AND snake_case boundaries
**And** `create_user_record` SHALL be included as a name_match candidate if tokens intersect

---

### Scenario: Short Tokens (< 3 chars) Ignored

**Given** a dead symbol `get_by_id`
**And** a candidate with token `id`
**When** `suggest_relation_candidates(get_by_id)` is called
**Then** token `id` (length 2) SHALL be excluded from name matching
**And** token `get` (length 3) SHALL be included

---

## Constants

| Name | Value | Description |
|------|-------|-------------|
| `CONFIDENCE_SAME_FILE` | `0.7` | Same-file heuristic confidence |
| `CONFIDENCE_SAME_COMMUNITY` | `0.5` | Same-community heuristic confidence |
| `CONFIDENCE_NAME_MATCH` | `0.3` | Name-match heuristic confidence |
| `MIN_TOKEN_LEN` | `3` | Minimum token length for name matching |

---

## Architecture Notes

- **Service**: `RelationCandidateService` (stateless, `&CallGraph` in, `Vec<RelationCandidate>` out)
- **Facade**: `AnalysisService::suggest_relation_candidates(&self, symbol_id: &str)`
- **Community**: Recomputed once per call via `CommunityDetector::detect()` (not persisted)
- **Dedup**: `HashMap<SymbolId, RelationCandidate>` with max-confidence-wins semantics

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|----------|
| Service module vs inline | Separate `relation_candidates.rs` | Matches `CommunityDetector` pattern; testable |
| Community recompute | Once per call | No schema change; bounded O(V+E) |
| Name tokenization | snake + camelCase split | Catches `create_user` ↔ `user_service` |
| Token min length | 3 chars | Filters `get`, `set`, `to` noise |
| Dedup strategy | Keep max confidence | Highest signal wins per symbol |

---

## Future Extensibility

- `direction: "outgoing"` for suggesting callees of an isolated live symbol
- Persist `community_id` on `Symbol` to avoid recompute
- Configurable confidence weights
- MCP tool exposure via `suggest_relation_candidates` handler
