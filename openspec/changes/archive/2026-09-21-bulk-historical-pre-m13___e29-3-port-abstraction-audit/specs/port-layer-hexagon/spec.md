# Port Layer Hexagon Specification

> Capability: `port-layer-hexagon` · Change: `e29-3-port-abstraction-audit`
> Branch: `main` @ `10f27d59` · Strict TDD: ACTIVE · Test runner:
> `cargo test -p cognicode-ladybug / -p cognicode-runtime / -p cognicode-explorer --no-default-features`

## Purpose

Codify the hexagonal dependency direction in `cognicode-core`: domain
(`src/domain/`) and application (`src/application/`) production code
MUST NOT depend on `crate::infrastructure::*`. The petgraph-backed
projection that analytics descriptors and services share becomes a
domain port (`CallGraphProjectionPort`) so adapters stay out of
domain code. The port catalog in `domain/ports/mod.rs` is reconciled
to the post-change set, and stale `PostgresRepository`/`Postgres*`
doc references are purged from the port surface.

## ADDED Requirements

### Requirement: Hexagon Domain Direction

`crates/cognicode-core/src/domain/` and production code under
`crates/cognicode-core/src/application/services/` and
`crates/cognicode-core/src/application/dto/` MUST NOT import
`crate::infrastructure::*`. Test modules (`#[cfg(test)] mod tests`)
are exempt; they may import infrastructure stubs as needed.

#### Scenario: zero `crate::infrastructure::` imports in domain and application production code

- GIVEN `grep -rn "use crate::infrastructure::" crates/cognicode-core/src/domain/ crates/cognicode-core/src/application/`
- WHEN the search runs against production (non-`mod tests`) modules
- THEN the result count MUST be 0
- AND the 14 migrated files (11 in `domain/analytics/*_descriptor.rs`, 3 in `application/services/{graph_analytics,graph_insights,impact_analysis}.rs`) MUST depend on `&dyn CallGraphProjectionPort` (or `Arc<dyn CallGraphProjectionPort>`) instead of the concrete petgraph struct

> **Amendment (apply, Phase 2)**: `search_ranker` and `community_detector`
> were relocated from `application/services/` to
> `infrastructure/graph/analytics/` (they consume the concrete petgraph
> projection directly). They are exempt from the domain/application
> dependency rule **by location** — the production-code grep above no
> longer sees them, and the migrated-file count is 14, not 16.

#### Scenario: e28 analytics conformance still passes after projection migration

- GIVEN cohort-1/cohort-2 analytics descriptors whose `AlgorithmExecute::execute` builds a projection internally
- WHEN `cargo test -p cognicode-core --test analytics_registry_cohort_1 --test analytics_registry_cohort_2 --test analytics_bounded_paths --test analytics_registry_admission` runs
- THEN every conformance-fixture test MUST pass (behavior preserved; only the type the descriptor depends on changed)

### Requirement: `CallGraphProjectionPort` is the canonical domain port for call-graph projections

`crates/cognicode-core/src/domain/ports/call_graph_projection.rs`
MUST declare `CallGraphProjectionPort`. The trait abstracts the
projection operations needed by analytics descriptors and services
(`build_adjacency`, `node_count`, `symbol_index`, and every other
method actually called by the 14 migrated call sites; see the
descriptor source for the live list).

> **Amendment (apply, Phase 1)**: construction is NOT a trait method —
> a no-receiver associated fn would break `dyn` compatibility. The port
> module exposes the object-safe free factory
> `project_call_graph(&CallGraph) -> Arc<dyn CallGraphProjectionPort>`
> (construction logic stays in the infra adapter), and the
> `id_to_index` accessor is surfaced as `symbol_index` on the port.

#### Scenario: trait declaration compiles and the infra impl satisfies it

- GIVEN the new trait in `domain/ports/call_graph_projection.rs`
- WHEN `cargo check -p cognicode-core` runs under default features
- THEN the trait declaration MUST compile
- AND `infrastructure::graph::CallGraphProjection` (petgraph-backed) MUST `impl CallGraphProjectionPort for CallGraphProjection`

#### Scenario: 16 call sites drop the concrete `infrastructure::graph::CallGraphProjection` import

- GIVEN the 16 migrated files enumerated above
- WHEN `grep -rn "use crate::infrastructure::graph::CallGraphProjection" crates/cognicode-core/src/domain/analytics/*.rs crates/cognicode-core/src/application/services/{graph_analytics,graph_insights,impact_analysis,search_ranker,community_detector}.rs` runs
- THEN result count MUST be 0 (each file imports `crate::domain::ports::CallGraphProjectionPort` instead)

### Requirement: Port catalog is reconciled to the post-change set

`crates/cognicode-core/src/domain/ports/mod.rs` MUST enumerate
exactly 13 modules in its doc table. `graph_error` is listed as a
utility (not a port). Two modules (`graph_write_port`,
`named_view_store`) MUST be deleted; one (`call_graph_projection`)
MUST be added; one (`node_property_reader`) is renamed to
`node_property_repository`. The catalog MUST reflect this.

#### Scenario: `mod.rs` table lists exactly the active ports

- GIVEN the post-change `crates/cognicode-core/src/domain/ports/mod.rs`
- WHEN the module-level doc table is parsed and counted
- THEN the count of port entries MUST be 12 active ports + 1 utility (`graph_error`), totalling 13
- AND it MUST list: `call_graph_store`, `call_graph_projection`, `federation_store` (cfg multimodal), `graph_repository`, `ingest_commit` (cfg multimodal), `manifest_store`, `node_property_repository`, `quality_store`, `report_store`, `revision_store`, `session_store`, `view_spec_store`
- AND it MUST NOT list `named_view_store` or `graph_write_port`
- [needs-multimodal] — the cfg-gated modules (`federation_store`, `ingest_commit`) appear only under `--features multimodal`; the table header notes which entries are cfg-gated

#### Scenario: deleted port symbols compile to zero

- GIVEN `grep -rn "NamedViewStore\|GraphWritePort" crates/*/src`
- WHEN the search runs
- THEN result count MUST be 0 (both the trait `NamedViewStore`, the type `NamedView`, the error `NamedViewError`, the trait `GraphWritePort` are gone)
- [needs-multimodal] — must hold under `--features multimodal` (the deleted files were cfg-gated in the base state)

### Requirement: Port doc vocabulary is backend-neutral

Doc comments inside `crates/cognicode-core/src/domain/ports/` MUST
NOT mention PostgreSQL primitives (`PostgresRepository`,
`PostgresQualityStore`, `PostgresViewSpecStore`). The port catalog
is DB-agnostic; PG specifics belong inside adapters.

#### Scenario: PG-leak grep returns zero in port surface

- GIVEN `grep -rn "PostgresRepository\|PostgresQualityStore\|PostgresViewSpecStore" crates/cognicode-core/src/domain/ports/`
- WHEN the search runs
- THEN result count MUST be 0
- [needs-multimodal] — must hold under `--features multimodal`
