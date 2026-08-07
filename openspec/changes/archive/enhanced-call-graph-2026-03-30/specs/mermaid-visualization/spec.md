# Mermaid Visualization Specification

## Purpose

Export call graphs as Mermaid diagrams for visualization in compatible tools.

## Requirements

### REQ-MV-001: Flowchart Export

The system SHALL provide `export_mermaid(call_graph)` returning valid Mermaid flowchart syntax.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| format | string | "flowchart" | Diagram type |
| theme | string | "default" | Mermaid theme |
| direction | string | "TD" | Layout direction |

#### Scenario: Basic flowchart generation

- GIVEN call graph: `main` → `process` → `validate`
- WHEN `export_mermaid(call_graph)`
- THEN output starts with `flowchart TD`
- AND contains `main --> process`
- AND contains `process --> validate`

#### Scenario: Valid Mermaid syntax

- GIVEN any call graph
- WHEN `export_mermaid(call_graph)`
- THEN output is parseable by Mermaid CLI
- AND passes Mermaid syntax validation

### REQ-MV-002: Node Styling

The system SHOULD apply visual styling based on node type.

#### Scenario: Style entry points

- GIVEN `main` is an entry point
- WHEN `export_mermaid(call_graph)`
- THEN `main` node has `classDef entryPoint` styling
- AND output includes style definitions

#### Scenario: Style hot paths

- GIVEN `helper` has fan_in > 5
- WHEN `export_mermaid(call_graph, highlight_hot=true)`
- THEN `helper` node has `classDef hotPath` styling

### REQ-MV-003: Subgraph Grouping

The system SHALL support grouping by module/file.

#### Scenario: Group by file

- GIVEN functions in `src/api/handlers.rs` and `src/db/queries.rs`
- WHEN `export_mermaid(call_graph, group_by="file")`
- THEN output contains `subgraph src/api/handlers.rs`
- AND cross-file edges connect subgraphs

### REQ-MV-004: Edge Labels

The system MAY include edge labels with call counts.

#### Scenario: Label edges with counts

- GIVEN `a` calls `b` 3 times
- WHEN `export_mermaid(call_graph, edge_labels=true)`
- THEN edge is `a -->|3| b`

### REQ-MV-005: Cycle Visualization

The system MUST visually indicate cycles.

#### Scenario: Cyclic dependency

- GIVEN `a` → `b` → `a` (cycle)
- WHEN `export_mermaid(call_graph)`
- THEN cycle edges have `classDef cycle` styling
- AND cycle is clearly distinguishable

### REQ-MV-006: Large Graph Handling

The system SHALL truncate or paginate large graphs.

#### Scenario: Graph exceeds limit

- GIVEN call graph with 500+ nodes
- WHEN `export_mermaid(call_graph, max_nodes=100)`
- THEN output contains at most 100 nodes
- AND `truncated=true` in metadata
- AND hottest nodes are prioritized
