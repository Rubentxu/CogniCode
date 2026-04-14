# Complexity Metrics Specification

## Purpose

Calculate and expose code complexity metrics from the call graph: fan-in, fan-out, and nesting depth.

## Requirements

### REQ-CM-001: Fan-In Metric

The system SHALL calculate fan-in (number of unique callers) for each symbol.

| Metric | Formula | Range |
|--------|---------|-------|
| fan_in | count(incoming_edges) | 0 to N |

#### Scenario: Calculate fan-in

- GIVEN `helper` is called by `a`, `b`, `c`
- WHEN `get_complexity_metrics(symbol="helper")`
- THEN `fan_in=3`
- AND `callers=["a", "b", "c"]`

#### Scenario: Zero fan-in (entry point)

- GIVEN `main` has no callers
- WHEN `get_complexity_metrics(symbol="main")`
- THEN `fan_in=0`
- AND `is_entry_point=true`

### REQ-CM-002: Fan-Out Metric

The system SHALL calculate fan-out (number of unique callees) for each symbol.

#### Scenario: Calculate fan-out

- GIVEN `process` calls `validate`, `transform`, `save`
- WHEN `get_complexity_metrics(symbol="process")`
- THEN `fan_out=3`
- AND `callees=["validate", "transform", "save"]`

#### Scenario: Zero fan-out (leaf)

- GIVEN `validate` calls no other functions
- WHEN `get_complexity_metrics(symbol="validate")`
- THEN `fan_out=0`
- AND `is_leaf=true`

### REQ-CM-003: Nesting Depth Metric

The system SHALL calculate maximum nesting depth from call hierarchy.

| Metric | Formula |
|--------|---------|
| nesting_depth | max(depth) across all call paths from entry |

#### Scenario: Calculate nesting depth

- GIVEN `main` → `process` → `validate` → `check`
- WHEN `get_complexity_metrics(symbol="main", include_nesting=true)`
- THEN `max_nesting_depth=3`

#### Scenario: Multiple paths with different depths

- GIVEN `main` → `a` (depth 1) and `main` → `b` → `c` → `d` (depth 3)
- WHEN `get_complexity_metrics(symbol="main", include_nesting=true)`
- THEN `max_nesting_depth=3`
- AND `avg_nesting_depth` is calculated

### REQ-CM-004: Aggregated Metrics

The system SHALL provide project-wide metric summaries.

#### Scenario: Project summary

- WHEN `get_complexity_metrics(scope="project")`
- THEN result includes:
  - `total_functions`: count of all functions
  - `avg_fan_in`: mean fan-in across project
  - `avg_fan_out`: mean fan-out across project
  - `max_nesting_depth`: deepest call chain
  - `hot_functions`: top 10 by fan-in

### REQ-CM-005: Complexity Score

The system SHOULD calculate a composite complexity score.

#### Scenario: Calculate complexity score

- GIVEN function with `fan_in=5`, `fan_out=10`, `nesting_depth=4`
- WHEN `get_complexity_metrics(symbol="complex_func")`
- THEN `complexity_score` is calculated using weighted formula
- AND score formula is documented

### REQ-CM-006: Threshold Warnings

The system MAY flag functions exceeding complexity thresholds.

#### Scenario: High complexity warning

- GIVEN function with `fan_out > 10`
- WHEN `get_complexity_metrics(symbol="kitchen_sink")`
- THEN `warnings` includes "high fan-out detected"
- AND threshold defaults are documented
