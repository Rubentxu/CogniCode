# Port Naming Convention Specification

> Capability: `port-naming-convention` · Change: `e29-3-port-abstraction-audit`
> Branch: `main` @ `10f27d59` · Strict TDD: ACTIVE · Test runner:
> `cargo test -p cognicode-ladybug / -p cognicode-runtime / -p cognicode-explorer --no-default-features`

## Purpose

Enforce the ADR-028 port-naming taxonomy at the workspace level so
port-trait names signal their shape: `*Repository` for
read-only identity/resolution (e.g. `GraphRepository`,
`NodePropertyRepository`), `*Store` for CRUD persistence
(e.g. `QualityStore`, `ViewSpecStore`, `CallGraphStore`), `*Port`
for generic domain ports. `*Reader` is collapsed into `*Repository`
for traits that resolve identity rather than persist state — the
single offender, `NodePropertyReader`, MUST be renamed.

## ADDED Requirements

### Requirement: `NodePropertyReader` is renamed to `NodePropertyRepository`

The file
`crates/cognicode-core/src/domain/ports/node_property_reader.rs`
MUST be renamed on disk to
`crates/cognicode-core/src/domain/ports/node_property_repository.rs`
(file body + module name in `mod.rs` + the re-export line). Every
reference to `NodePropertyReader` in the workspace MUST be updated
to `NodePropertyRepository`.

#### Scenario: zero references to the old trait name

- GIVEN `grep -rn "NodePropertyReader" crates/*/src`
- WHEN the search runs
- THEN result count MUST be 0 (no `pub trait NodePropertyReader`, no `impl NodePropertyReader for X`, no `cognicode_core::domain::ports::NodePropertyReader` imports anywhere)
- AND `use cognicode_core::domain::ports::NodePropertyRepository;` MUST appear in `crates/cognicode-explorer/src/adapters/call_graph_repository.rs`
- AND `impl NodePropertyRepository for CallGraphRepository` MUST be the impl block on that adapter
- AND `crates/cognicode-explorer/src/dto.rs` line 346 (`pub node_property_reader: Option<&'a dyn ... NodePropertyReader>`) MUST be retyped to `pub node_property_repository: Option<&'a dyn NodePropertyRepository>`

[needs-multimodal] — must hold under `--features multimodal` (the
trait is not cfg-gated, but the search runs against both builds).

### Requirement: explorer `::ports::QualityStore` re-export shim is deleted

The file
`crates/cognicode-explorer/src/ports/quality_repository.rs` MUST NOT
exist. All callers that previously routed through the shim MUST
import the trait and DTOs directly from
`cognicode_core::domain::ports` (specifically:
`QualityStore`, `NewIssue`, `UpsertSummary`, `QualityError`,
`QualityIssue`, `QualityGateSummary`, `RuleSummary`, `IssueFilter`).

`GraphRepository` and other explorer-reserved ports (e.g.
`SymbolRepository`, `SourceReader`) remain in `cognicode_explorer::ports`
because their adapters live there; only the `QualityStore` shim is
deleted.

#### Scenario: explorer ports shim is gone and runtime imports from core

- GIVEN `find crates/cognicode-explorer/src/ports/quality_repository.rs`
- WHEN the lookup runs
- THEN it MUST NOT exist
- AND `grep -rn "cognicode_explorer::ports::QualityStore\|cognicode_explorer::ports::QualityError\|cognicode_explorer::ports::NewIssue" crates/*/src` MUST return 0 hits
- AND `grep -rn "cognicode_explorer::ports::GraphRepository" crates/*/src` MAY still return hits (this port stays in explorer because its canonical adapter is explorer-resident)

[needs-multimodal] — must hold under `--features multimodal`.

### Requirement: every port-trait name follows the ADR-028 taxonomy

Every `pub trait` declared in
`crates/cognicode-core/src/domain/ports/` MUST end with `Store`,
`Repository`, or `Port`. The taxonomy is the SINGLE source of truth
for what a port looks like; reviewers can grep one pattern to find
every port.

#### Scenario: no `*Reader`, no other port-noun

- GIVEN `grep -rn "^pub trait " crates/cognicode-core/src/domain/ports/`
- WHEN the matches are inspected
- THEN every trait name MUST end with `Store`, `Repository`, or `Port`
- AND no trait MUST end with `Reader`, `Manager`, `Engine`, `Facade`, `Service`, or any other port-noun
- AND the matches MUST equal the 13-module post-change port catalog (excluding `graph_error`, which contains types not traits)

[needs-multimodal] — applies under both `--features multimodal`
(multimodal-gated port files must still comply).
