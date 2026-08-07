# Delta for Repository Trait Bridge (explorer + core)

> Companion to the engram-stored full spec (`sdd/explorer-graph-repository-bridge/spec`).
> This file mirrors the requirements in LogSeq `Spec: explorer-graph-repository-bridge` and the engram observation.

## Status

Draft. Awaiting `sdd-design`.

## Findings Reshaping Scope

The proposal claimed `cognicode-store-traits` is "dead, zero dependents, identical to `GraphStore` in core". **VERIFIED FALSE**:

- `cognicode-core/Cargo.toml` line 90: `cognicode-store-traits = { workspace = true }`
- `cognicode-db/Cargo.toml` line 15: `cognicode-store-traits.workspace = true`
- `cognicode-store-traits` defines its own `CallGraph`, `Symbol`, `GraphStore`, `StoreError`, `FileManifest` (no `Provenance` metadata)
- The shape of `GraphStore` IS identical between the two crates; the types referenced by it are NOT

**Reshape**: deprecate `cognicode-store-traits` in this slice. Removal is deferred to a follow-up slice once the PostgreSQL adapter is delivered and the workspace can be migrated atomically.

## ADDED Requirements

### Requirement: MetadataAwareRepository sub-trait

The system SHALL expose a `MetadataAwareRepository: SymbolRepository` sub-trait in `cognicode-explorer::ports::symbol_repository` with three methods: `callees_with_metadata`, `dependencies_with_metadata`, `edges_with_metadata`. The sub-trait SHALL be opt-in. Implementations SHALL route to `CallGraph::callees_with_metadata` and `CallGraph::edges_with_metadata` (Phase 1).

#### Scenario: Opt-in sub-trait on CallGraphRepository
- GIVEN a `CallGraphRepository` and a graph seeded with mixed-provenance edges
- WHEN `callees_with_metadata(&id)` is called
- THEN every returned entry MUST carry the exact `(provenance, confidence)` assigned by `ConfidenceRules`

#### Scenario: Sub-trait not required for base consumers
- GIVEN a mock implementor of `SymbolRepository` that does NOT implement `MetadataAwareRepository`
- WHEN the mock is passed where `dyn SymbolRepository` is expected
- THEN the call MUST succeed and metadata-aware methods MUST NOT be reachable

### Requirement: Optional metadata on RelationTarget
The system SHALL add `pub provenance: Option<Provenance>` and `pub confidence: Option<f64>` to `RelationTarget`. `From<&ResolvedSymbol>` MUST set both to `None`. The base `SymbolRepository::callees` signature is unchanged.

#### Scenario: Backward compatibility
- GIVEN a `ResolvedSymbol` and `RelationTarget::from(&resolved)`
- WHEN the conversion runs
- THEN `provenance = None, confidence = None`
- AND the 295 existing tests MUST pass unmodified

### Requirement: Async-ready Repository trait in core
The system SHALL define a new `Repository` trait in `cognicode-core::domain::traits::repository` using `#[async_trait]`, extending the synchronous `GraphStore` with two async read methods. No concrete impl is added in this slice.

#### Scenario: Trait compiles and is dyn-compatible
- GIVEN the `Repository` trait annotated with `#[async_trait]`
- WHEN `cargo check -p cognicode-core` runs
- THEN compilation MUST succeed and `Box<dyn Repository>` MUST be `Send + Sync`

### Requirement: cognicode-store-traits deprecation
The system SHALL mark `cognicode-store-traits` as deprecated in `Cargo.toml` description AND in `lib.rs` top-level doc. The crate SHALL remain in the workspace and SHALL keep compiling. Removal is out of scope.

#### Scenario: Crate still compiles
- GIVEN `cognicode-store-traits` present
- WHEN this slice lands
- THEN `cargo check --workspace` MUST pass

### Requirement: Contract tests for MetadataAwareRepository
The system SHALL provide `cognicode-explorer/tests/metadata_aware_repository.rs` with golden, invariant, backward-compat, and polymorphism tests.

#### Scenario: Golden triples
- GIVEN a fixture with `DirectExtraction`, `Heuristic{0.7}`, `Heuristic{0.4}` edges
- WHEN `callees_with_metadata()` is called
- THEN the entries MUST be `(Extracted, 1.0)`, `(Inferred, 0.7)`, `(Ambiguous, 0.3)` (f64 exact)

### Requirement: Migration and rollout safety
Single PR ≤ 400 lines. Land in order: sub-trait + RelationTarget fields → impl on CallGraphRepository → contract tests → core Repository trait → deprecation notice. Reversible via single `git revert`.

#### Scenario: PR size budget
- GIVEN the planned changes
- THEN `additions + deletions` MUST be ≤ 400

## MODIFIED Requirements

### Requirement: cognicode-explorer SymbolRepository is the canonical explorer port
`SymbolRepository` remains the canonical explorer port. `MetadataAwareRepository` extends it. No method on `SymbolRepository` itself changes signature. (Previously: `SymbolRepository` was the only port; metadata-aware access required an adapter-specific method outside the trait.)

### Requirement: cognicode-core domain::traits is the canonical core port surface
`cognicode_core::domain::traits` is the canonical home for cross-cutting ports. The new `Repository` trait is added there. `cognicode-store-traits` is marked deprecated. (Previously: `cognicode-store-traits` claimed to be canonical; Phase 1 already moved the canonical `CallGraph` into `cognicode-core`.)

## REMOVED Requirements
None.

## Out of Scope (locked)
- PostgreSQL adapter, sqlx, schema DDL
- New node kinds, new edge kinds
- MCP envelope, ExplorerQL, Explorer UI
- `CallGraphV1` removal, JSON snapshots, bincode wire-format changes
- Removal of `cognicode-store-traits` (deferred)

## Coverage
- Happy paths: covered
- Edge cases: covered
- Error states: covered

## Open Questions
None blocking.
