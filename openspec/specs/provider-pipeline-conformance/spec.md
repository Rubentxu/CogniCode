# provider-pipeline-conformance Specification

## Purpose

Conformance fixtures and declared precision targets for the semantic provider pipeline: fixtures declare, before running, the expected serving tier and outcome per query and language, a run fails on any tier contradiction naming query/declared/observed, and per-language coverage gates on server availability — Rust and TypeScript verify their declared non-LSP tiers unconditionally without servers, Java asserts the declared-unavailable path without failing when its language server is absent. Tier and pipeline semantics live in main spec `provider-tier-provenance`; fixtures follow the `sandbox/fixtures` convention.

## Requirements

### Requirement: Declared per-tier precision targets

Fixtures MUST declare, before running, the expected serving tier and outcome per query and language. A run MUST verify each observation's declared tier against its declaration, failing on contradiction; matches pass without numeric thresholds.

#### Scenario: Contradicting tier fails the run

- GIVEN a fixture declaring the LSP tier for a query
- WHEN the run observes a tree-sitter-declared result
- THEN the run fails, naming query, declared tier, and observed tier

### Requirement: Availability-gated language coverage

Rust and TypeScript suites MUST verify declared non-LSP tiers unconditionally, without servers. Java LSP coverage MUST gate on server availability: when unavailable, the declared-unavailable path MUST assert no result declares a higher tier than available, with an unavailable diagnostic recorded; it MUST NOT fail.

#### Scenario: Java without its server degrades by declaration

- GIVEN the Java language server is unavailable
- WHEN the Java suite runs
- THEN the LSP scenario asserts declared unavailability (diagnostic, no LSP-tier result) and the suite passes

#### Scenario: Matching declarations pass without servers

- GIVEN no live language servers
- WHEN the Rust and TypeScript suites run
- THEN their declared non-LSP targets verify and pass, honoring declarations without thresholds
