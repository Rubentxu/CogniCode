# Runtime Bootstrap Contract Specification

> Capability: `runtime-bootstrap-contract` · Change: `e29-3-port-abstraction-audit`
> Branch: `main` @ `10f27d59` · Strict TDD: ACTIVE · Test runner:
> `cargo test -p cognicode-runtime --no-default-features`

## Purpose

The runtime composition seam collapses from a single-implementer
trait indirection (`PgBackend` + `LadybugPgBackend`) into a plain
`RuntimePorts` DTO carrying the three relocated
`Option<Arc<dyn *Port>>` slots. The `Runtime` struct exposes the
ports directly (no `backend` field). The 3 PgBackend-self-justifying
smoke tests die; the functional R3+R5 test migrates to the new
DTO.

## ADDED Requirements

### Requirement: `PgBackend` trait and `LadybugPgBackend` adapter are deleted

`crates/cognicode-runtime/src/lib.rs` MUST NOT declare `pub trait PgBackend`,
`pub struct LadybugPgBackend`, `impl PgBackend for LadybugPgBackend`,
or `pub backend: Option<Arc<dyn PgBackend>>`. The runtime no longer
carries a backend indirection.

#### Scenario: zero `PgBackend` / `LadybugPgBackend` references in the workspace

- GIVEN `grep -rn "PgBackend\|LadybugPgBackend" crates/*/src`
- WHEN the search runs
- THEN result count MUST be 0 (the trait name, the struct name, the impl block, every test reference, every docs reference — all gone)
- AND `cognicode_runtime` MUST NOT re-export either symbol

[needs-multimodal] — must hold under `--features multimodal`.

### Requirement: `bootstrap_with_backend` accepts a plain `RuntimePorts` DTO

`crates/cognicode-runtime/src/lib.rs` MUST declare:

```rust
pub struct RuntimePorts {
    pub quality_store: Option<Arc<dyn cognicode_core::domain::ports::QualityStore>>,
    pub view_spec_store: Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>,
    pub call_graph_store: Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>>,
}

pub async fn bootstrap_with_backend(
    cwd: PathBuf,
    ports: RuntimePorts,
) -> Result<Runtime, anyhow::Error>;
```

The signature replaces the previous
`(PathBuf, Arc<dyn PgBackend>) -> Result<Runtime, _>` form. No
backend indirection; ports live on the `Runtime` struct directly.

#### Scenario: bootstrap populates Runtime with Arc identity preserved

- GIVEN three test stub ports (`TestQualityStore`, `TestViewSpecStore`, `TestCallGraphStore`) each wrapped in `Arc<dyn _>`
- WHEN `bootstrap_with_backend(temp_dir(), RuntimePorts { quality_store: Some(q.clone()), view_spec_store: Some(v.clone()), call_graph_store: Some(c.clone()), /* ..other defaults for any added fields */ }).await`
- THEN the returned `Runtime.quality_store` MUST be `Arc::ptr_eq` to `q` (same allocation)
- AND `Runtime.view_spec_store` MUST be `Arc::ptr_eq` to `v`
- AND `Runtime.call_graph_store` MUST be `Arc::ptr_eq` to `c`
- AND `Runtime` MUST have no `backend` field

[needs-multimodal] — must hold under `--features multimodal`.

### Requirement: `Runtime.quality_store` is typed as the core port, not the explorer shim

The `Runtime::quality_store` field type MUST be
`Option<Arc<dyn cognicode_core::domain::ports::QualityStore>>`.
The runtime no longer types the field via the explorer shim.

#### Scenario: no explorer-shim path in the runtime field

- GIVEN `grep -rn "cognicode_explorer::ports::QualityStore" crates/cognicode-runtime/src/`
- WHEN the search runs
- THEN result count MUST be 0
- AND `cognicode_core::domain::ports::QualityStore` MUST appear at the field declaration site

[needs-multimodal] — must hold under `--features multimodal`.

### Requirement: smoke-test migration plan is explicit

The runtime integration-test directory
(`crates/cognicode-runtime/tests/`) MUST be reduced and rewritten
around `RuntimePorts`. The 3 PgBackend-self-justifying tests die;
the R3+R5 functional test migrates.

#### Scenario: 3 self-justifying tests die, 1 functional test migrates

- GIVEN the pre-change test files
- WHEN the migration is complete
- THEN `tests/bootstrap_with_backend_smoke.rs::bootstrap_with_backend_signature_compiles` (line 215) MUST be deleted (it asserts `Arc<dyn PgBackend>` compiles — meaningless under the new DTO)
- AND `tests/bootstrap_with_backend_smoke.rs::ladybug_pg_backend_implements_pg_backend_for_bootstrap_with_backend` (line 220) MUST be deleted (it asserts the impl-block exists — the trait is gone)
- AND `tests/backend_compat.rs::pg_backend_trait_object_supports_ladybug_backend` (full file) MUST be deleted (the trait-object check no longer applies)
- AND `tests/ui/r2_as_postgres_repo_absent.rs` (full file) MUST be deleted (trybuild compile-fail for a removed method)
- AND `tests/bootstrap_with_backend_smoke.rs::r3_r5_ports_populated_from_backend_with_identity` (line 249) MUST be rewritten — the test body builds a `RuntimePorts { quality_store: Some(q.clone()), view_spec_store: Some(v.clone()), call_graph_store: Some(c.clone()) }` and calls `bootstrap_with_backend(temp_dir(), ports)` instead of constructing a `LadybugPgBackend`

[needs-multimodal] — the test surface is not cfg-gated by multimodal,
but the runner must compile under `--features multimodal`.

### Requirement: minimal-features runtime build is green

`cargo test -p cognicode-runtime --no-default-features` MUST
succeed. The composition seam MUST NOT require the ladybug build
to type-check.

#### Scenario: --no-default-features build of runtime is green

- GIVEN `cargo test -p cognicode-runtime --no-default-features`
- WHEN the build + tests run
- THEN exit code MUST be 0
- AND the rewritten `bootstrap_with_backend` test (with `RuntimePorts`) MUST pass
- AND 0 PgBackend references remain in any test source
