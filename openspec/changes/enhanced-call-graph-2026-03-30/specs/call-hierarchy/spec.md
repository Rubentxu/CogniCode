# Call Hierarchy Specification

## Purpose

Define recursive call hierarchy traversal with configurable depth and bidirectional support (outgoing calls and incoming callers).

## Requirements

### REQ-CH-001: Recursive Depth Traversal

The system SHALL support recursive call hierarchy traversal up to a configurable depth (default: 3, max: 10).

| Parameter | Type | Default | Constraints |
|-----------|------|---------|-------------|
| depth | uint | 3 | 1 ≤ depth ≤ 10 |

#### Scenario: Traverse with depth 2

- GIVEN function `main` calls `foo` which calls `bar`
- WHEN `get_call_hierarchy(symbol="main", depth=2, direction="outgoing")`
- THEN result includes `main` → `foo` → `bar`
- AND result stops at depth 2

#### Scenario: Max depth limit enforced

- GIVEN a call chain of 15 functions
- WHEN `get_call_hierarchy(symbol="root", depth=15)`
- THEN result is truncated at depth 10
- AND warning includes "depth capped to maximum 10"

### REQ-CH-002: Incoming Direction Support

The system SHALL support `direction="incoming"` to return callers of a symbol.

#### Scenario: Find callers with depth

- GIVEN `a` → `b` → `c` (b calls c)
- WHEN `get_call_hierarchy(symbol="c", depth=2, direction="incoming")`
- THEN result includes `c` ← `b` ← `a`

#### Scenario: No callers (entry point)

- GIVEN function `main` has no callers
- WHEN `get_call_hierarchy(symbol="main", direction="incoming")`
- THEN result contains only `main`
- AND `is_entry_point` flag is TRUE

### REQ-CH-003: Cycle Detection in Hierarchy

The system MUST detect and mark cycles during traversal to prevent infinite loops.

#### Scenario: Cycle in call graph

- GIVEN `a` → `b` → `c` → `a` (cycle)
- WHEN `get_call_hierarchy(symbol="a", depth=5)`
- THEN result marks `a` with `cycle=true` on second visit
- AND traversal continues to other branches

### REQ-CH-004: Symbol Not Found

The system MUST return a clear error when the target symbol does not exist.

#### Scenario: Unknown symbol

- GIVEN no function named `nonexistent`
- WHEN `get_call_hierarchy(symbol="nonexistent")`
- THEN result contains `error: symbol not found`
- AND HTTP status is 404 equivalent
