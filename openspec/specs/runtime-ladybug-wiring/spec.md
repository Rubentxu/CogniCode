# Runtime Ladybug Wiring Specification

**Version**: 0.1.0
**Date**: 2026-08-05
**Status**: ACTIVE (e29-6 cycle)
**Change**: `e29-6-ladybug-store-wiring`

## Overview

This specification defines the contract for wiring LadybugStore port implementations into the Runtime bootstrap. It covers the `RuntimePorts` DTO, the `bootstrap_ladybug` helper, and the `--db` CLI argument for both binary entry points.

## Requirement: Runtime Bootstrap Wires All LadybugStore Ports Through RuntimePorts

The system MUST accept all 10 LadybugStore port implementations (RevisionStore, FederationStore, ManifestStore, SessionStore, ReportStore, ViewSpecStore, CallGraphStore, IngestCommitPort, RunLineageStore, QualityStore) through the `RuntimePorts` DTO. `bootstrap_with_backend()` SHALL surface every provided port on the resulting `Runtime` struct, preserving Arc identity.

### Scenario: All 10 ports wired through bootstrap_with_backend

- GIVEN a `LadybugStore` instance implementing all 10 domain port traits
- WHEN `bootstrap_with_backend(cwd, RuntimePorts{...})` is called with all 10 `Option<Arc<dyn *Trait>>` fields set to `Some(store.clone())`
- THEN the returned `Runtime` SHALL expose every port via its corresponding public field
- AND each field's `Arc` SHALL be pointer-equal to the Arc passed into the DTO

### Scenario: Analytics lineage records persisted and queryable after bootstrap

- GIVEN a `LadybugStore` with an initialized lineage DDL schema
- WHEN `bootstrap_with_backend` completes with `analytics_lineage_store: Some(ladybug_store)` and an algorithm is executed via the constructed `AlgorithmRegistry`
- THEN lineage records inserted by the algorithm SHALL be queryable through the `RunLineageStore` port on the `Runtime`

## Requirement: LadybugStore Database Lifecycle

The system SHALL automatically create the LadybugDB file on first use, open existing files without data loss, apply schema DDL idempotently, and propagate errors for corrupt databases.

### Scenario: DB file created automatically when missing

- GIVEN no file exists at the target path `./cognicode.lbug`
- WHEN a binary calls `LadybugStore::open("./cognicode.lbug")`
- THEN a valid LadybugDB database file SHALL be created at that path
- AND `open()` SHALL return `Ok(LadybugStore)`

### Scenario: Existing DB file opened without data loss

- GIVEN a LadybugDB file exists at `./cognicode.lbug` containing persisted data
- WHEN `LadybugStore::open("./cognicode.lbug")` is called a second time
- THEN the store SHALL open the existing file
- AND previously persisted data SHALL remain intact and queryable

### Scenario: Schema DDL runs idempotently on cold start and subsequent starts

- GIVEN a freshly created LadybugDB with no tables
- WHEN `LadybugStore::open()` runs schema initialization
- THEN all node and relationship tables from all schema families SHALL be present
- AND calling `open()` again on the same file SHALL complete without error (all `IF NOT EXISTS` DDL)

### Scenario: Corrupt database file propagates error

- GIVEN a file at `./cognicode.lbug` that is not a valid LadybugDB database
- WHEN `LadybugStore::open("./cognicode.lbug")` is called
- THEN the method SHALL return `Err` with a descriptive error
- AND no partial store SHALL be constructed

## Requirement: CLI --db Flag Controls Database Path

Both binary entry points (`api.rs` and `mcp.rs`) MUST accept a `--db <PATH>` CLI argument. When provided, it SHALL override the default database path. When omitted, the binary SHOULD either degrade gracefully with no LadybugStore ports or exit with an error message that hints at the `--db` flag.

### Scenario: --db flag overrides default database path

- GIVEN a LadybugDB file exists at `/data/my-project.lbug`
- WHEN the API binary is invoked with `--db /data/my-project.lbug`
- THEN `LadybugStore::open()` SHALL be called with `/data/my-project.lbug`
- AND all ports SHALL be wired from that database instance

### Scenario: --db flag not provided — graceful degradation with hint

- GIVEN no `--db` flag is passed to the binary
- WHEN the binary starts up
- THEN the system SHALL either (a) log a warning and proceed with no LadybugStore ports wired, OR (b) exit with a non-zero code and a message suggesting the `--db` flag
- AND the binary SHALL NOT silently succeed with an empty port set without notification

---

## RuntimePorts Shape

```rust
pub struct RuntimePorts {
    // Original 4:
    pub quality_store: Option<Arc<dyn QualityStore>>,
    pub view_spec_store: Option<Arc<dyn ViewSpecStore>>,
    pub call_graph_store: Option<Arc<dyn CallGraphStore>>,
    pub analytics_lineage_store: Option<Arc<dyn RunLineageStore>>,
    // Added by e29-6:
    pub manifest_store: Option<Arc<dyn ManifestStore>>,
    pub session_store: Option<Arc<dyn SessionStore>>,
    pub report_store: Option<Arc<dyn ReportStore>>,
    pub revision_store: Option<Arc<dyn RevisionStore>>,
    #[cfg(feature = "multimodal")]
    pub federation_store: Option<Arc<dyn FederationStore>>,
    #[cfg(feature = "multimodal")]
    pub ingest_commit_port: Option<Arc<dyn IngestCommitPort>>,
}
```

---

## References

- [e29-6-ladybug-store-wiring proposal](../changes/e29-6-ladybug-store-wiring/proposal.md)
- [e29-6-ladybug-store-wiring design](../changes/e29-6-ladybug-store-wiring/design.md)
- [e29-6-ladybug-store-wiring tasks](../changes/e29-6-ladybug-store-wiring/tasks.md)
