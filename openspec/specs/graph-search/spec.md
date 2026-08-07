# Graph Search

## Purpose

Read-path behavior of the `GraphRepository` port: paginated lookup by `NodeKind`, full-text `search` returning `SearchPage`, and graceful degradation when the `multimodal` Cargo feature is disabled.

## Requirements

### Requirement: Paginated Lookup By Kind

The system MUST return nodes whose `NodeKind` matches the requested kind, bounded by `limit` and advanced by an opaque `cursor`. Each returned item MUST expose `id`, `title`, `file_path`, and `kind`.

#### Scenario: First page returns kind-matching nodes

- GIVEN a workspace with 25 Doc nodes persisted in `graph_nodes`
- WHEN `find_nodes_by_kind(Doc, limit=10, cursor=None)` is called
- THEN at most 10 Doc nodes are returned
- AND each item exposes `id`, `title`, `file_path`, and `kind`

#### Scenario: Cursor advances to the next page

- GIVEN the first page returned a non-empty `next_cursor`
- WHEN `find_nodes_by_kind(Doc, limit=10, cursor=Some(<opaque>))` is called
- THEN the next page of Doc nodes is returned
- AND no node appears in both pages

#### Scenario: Empty result set is not an error

- GIVEN no Evidence nodes exist in the workspace
- WHEN `find_nodes_by_kind(Evidence, limit=10, cursor=None)` is called
- THEN an empty `Vec` is returned and no error is raised

### Requirement: Search Page With Rank And Cursor

The system MUST return full-text search results as `SearchPage { items, raw_total, next_cursor, raw_rank, item_ranks }` so pagination and ranking survive the port boundary. An empty query MUST return an empty page without error.

#### Scenario: Cursor pagination on full-text search

- GIVEN 30 Doc nodes match the query "auth"
- WHEN `search("auth", &[Doc], limit=10, cursor=None)` is called
- THEN `items.len() <= 10`, `raw_total == 30`, and `next_cursor` is `Some(...)`
- AND passing that cursor returns the next page with no overlap

#### Scenario: Empty query yields empty page

- GIVEN any workspace state
- WHEN `search("", &[Doc], limit=10, cursor=None)` is called
- THEN `items.is_empty()`, `raw_total == 0`, and no error is raised

### Requirement: Graceful Degradation Without Multimodal

When the `multimodal` Cargo feature is disabled, the system MUST degrade multimodal `find_nodes_by_kind` and `search` to empty results without compile errors. Symbol lookups MUST continue to work normally.

#### Scenario: Non-multimodal build compiles and returns empty

- GIVEN the crate is built with `--no-default-features` (multimodal off)
- WHEN any multimodal `find_nodes_by_kind` or `search` call is invoked
- THEN an empty result is returned
- AND Symbol lookups continue to work normally
