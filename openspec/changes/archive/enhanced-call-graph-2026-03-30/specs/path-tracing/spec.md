# Path Tracing Specification

## Purpose

Trace execution paths between two functions, showing all intermediate calls.

## Requirements

### REQ-PT-001: Basic Path Finding

The system SHALL provide `trace_execution_path(from, to)` returning all paths between two symbols.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| from | string | YES | Starting symbol |
| to | string | YES | Target symbol |
| max_paths | uint | NO | Limit paths (default: 10) |

#### Scenario: Single path exists

- GIVEN `main` → `process` → `validate`
- WHEN `trace_execution_path(from="main", to="validate")`
- THEN result contains one path: `["main", "process", "validate"]`
- AND `path_count=1`

#### Scenario: Multiple paths exist

- GIVEN `main` → `a` → `c` and `main` → `b` → `c`
- WHEN `trace_execution_path(from="main", to="c")`
- THEN result contains two paths
- AND each path is a complete call chain

### REQ-PT-002: No Path Exists

The system MUST handle cases where no path connects the symbols.

#### Scenario: Disconnected functions

- GIVEN `foo` and `bar` in separate call graphs
- WHEN `trace_execution_path(from="foo", to="bar")`
- THEN result contains `paths=[]`
- AND `path_count=0`
- AND message indicates "no path found"

### REQ-PT-003: Path Limit

The system MUST limit the number of paths returned to prevent exponential blowup.

#### Scenario: Exponential paths

- GIVEN diamond pattern with 100+ possible paths
- WHEN `trace_execution_path(from="start", to="end", max_paths=5)`
- THEN result contains exactly 5 paths
- AND `truncated=true` flag is set

### REQ-PT-004: Cycle Handling

The system MUST handle cycles without infinite loops.

#### Scenario: Cyclic call graph

- GIVEN `a` → `b` → `c` → `a` (cycle)
- WHEN `trace_execution_path(from="a", to="c")`
- THEN result contains path `["a", "b", "c"]`
- AND cycle is NOT included in path

### REQ-PT-005: Self-Reference

The system SHALL handle when `from` equals `to`.

#### Scenario: Same start and end

- GIVEN function `recursive` calls itself
- WHEN `trace_execution_path(from="recursive", to="recursive")`
- THEN result contains path `["recursive"]`
- AND `path_count=1`
