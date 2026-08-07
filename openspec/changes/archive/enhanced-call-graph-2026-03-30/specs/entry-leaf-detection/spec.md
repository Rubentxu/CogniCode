# Entry Points and Leaf Functions Specification

## Purpose

Identify entry points (functions with no callers) and leaf functions (functions that make no calls) in the codebase.

## Requirements

### REQ-EL-001: Entry Points Detection

The system SHALL provide `get_entry_points` to return all symbols with zero incoming edges.

| Return Field | Type | Description |
|--------------|------|-------------|
| symbol | string | Fully qualified symbol name |
| file_path | string | Source file location |
| line | uint | Line number |

#### Scenario: Standard entry points

- GIVEN project with `main`, `#[test]` functions, and `pub fn` handlers
- WHEN `get_entry_points()`
- THEN result includes `main` function
- AND result includes all test entry functions
- AND result includes public HTTP handlers

#### Scenario: No entry points

- GIVEN a library crate with no `main` and all functions are internal
- WHEN `get_entry_points()`
- THEN result is empty array
- AND `count=0`

### REQ-EL-002: Leaf Functions Detection

The system SHALL provide `get_leaf_functions` to return all symbols with zero outgoing edges.

#### Scenario: Identify terminal functions

- GIVEN function `helper` calls no other functions
- WHEN `get_leaf_functions()`
- THEN result includes `helper`
- AND each result includes `file_path` and `line`

#### Scenario: Mixed call graph

- GIVEN `main` → `process` → `validate` (validate calls nothing)
- WHEN `get_leaf_functions()`
- THEN result contains only `validate`
- AND `count=1`

### REQ-EL-003: Filtered Detection

The system SHOULD support filtering by file path pattern.

#### Scenario: Filter by module

- GIVEN entry points in `src/api/` and `src/db/`
- WHEN `get_entry_points(filter="src/api/**")`
- THEN result includes only symbols from `src/api/`

### REQ-EL-004: Performance for Large Codebases

The system MUST complete detection within 5 seconds for projects up to 10,000 symbols.

#### Scenario: Large project performance

- GIVEN project with 10,000 functions
- WHEN `get_entry_points()`
- THEN response time < 5 seconds
- AND results are paginated if > 1000 entries
