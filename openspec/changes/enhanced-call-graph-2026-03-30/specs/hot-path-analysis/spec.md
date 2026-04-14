# Hot Path Analysis Specification

## Purpose

Identify the most frequently called functions (hot paths) in the codebase based on fan-in metrics.

## Requirements

### REQ-HP-001: Hot Functions Detection

The system SHALL provide `get_hot_paths()` returning functions ranked by call frequency.

| Return Field | Type | Description |
|--------------|------|-------------|
| symbol | string | Function name |
| fan_in | uint | Number of unique callers |
| callers | [string] | List of caller symbols |

#### Scenario: Rank by fan-in

- GIVEN `helper` called by 10 functions, `process` called by 3
- WHEN `get_hot_paths()`
- THEN `helper` appears before `process` in results
- AND `helper.fan_in=10`

#### Scenario: Top N results

- GIVEN 50 callable functions
- WHEN `get_hot_paths(limit=10)`
- THEN result contains exactly 10 functions
- AND results are sorted by fan-in descending

### REQ-HP-002: Threshold Filtering

The system SHOULD support minimum threshold filtering.

#### Scenario: Filter by minimum calls

- GIVEN functions with fan-in ranging 1-20
- WHEN `get_hot_paths(min_calls=5)`
- THEN result excludes functions with fan_in < 5

### REQ-HP-003: Module Scope

The system SHALL support limiting analysis to specific modules.

#### Scenario: Scope to module

- GIVEN hot functions across `src/api/` and `src/core/`
- WHEN `get_hot_paths(scope="src/core/**")`
- THEN result includes only functions in `src/core/`
- AND callers from outside scope are counted but not listed

### REQ-HP-004: Call Frequency Types

The system MUST distinguish between static and dynamic hot paths.

#### Scenario: Static analysis only

- GIVEN `get_hot_paths(analysis_type="static")`
- THEN fan-in reflects static call graph edges
- AND dynamic profiling data is NOT used

### REQ-HP-005: Empty Result Handling

The system MUST return valid empty results.

#### Scenario: No calls in codebase

- GIVEN project with only entry points (no internal calls)
- WHEN `get_hot_paths()`
- THEN result is `[]`
- AND `total_count=0`
