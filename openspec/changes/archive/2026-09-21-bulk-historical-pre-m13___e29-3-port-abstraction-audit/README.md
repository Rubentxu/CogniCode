# Delta Spec Index — e29-3-port-abstraction-audit

> Phase: 1.4 sdd-spec · Branch: `main` @ `10f27d59`
> Strict TDD: ACTIVE · All four capabilities are **NEW** (no prior spec).

## Capabilities

| Capability | Spec file | Tests in scope | Requirements | Scenarios |
|---|---|---|---|---|
| `port-layer-hexagon` | [spec.md](./specs/port-layer-hexagon/spec.md) | cargo check / test on `cognicode-core`, `cognicode-runtime`, `cognicode-explorer` | 4 | 7 |
| `port-naming-convention` | [spec.md](./specs/port-naming-convention/spec.md) | cargo test on all three crates | 3 | 3 |
| `runtime-bootstrap-contract` | [spec.md](./specs/runtime-bootstrap-contract/spec.md) | `cargo test -p cognicode-runtime --no-default-features` | 5 | 5 |
| `quality-store-backend` | [spec.md](./specs/quality-store-backend/spec.md) | `cargo test -p cognicode-ladybug` (10 new in-crate tests) | 2 | 7 |

## User-scenario coverage map

12 GWT scenarios from the proposal → where each is asserted:

| # | User scenario | Asserted in spec |
|---|---|---|
| 1 | `grep PgBackend\|LadybugPgBackend` → 0 hits | `runtime-bootstrap-contract` S1 |
| 2 | `bootstrap_with_backend(cwd, RuntimePorts)` populates 3 ports Some | `runtime-bootstrap-contract` S2 |
| 3 | `grep NamedViewStore\|GraphWritePort` → 0 hits | `port-layer-hexagon` S6 |
| 4 | domain+application import zero `crate::infrastructure::*` | `port-layer-hexagon` S1 |
| 5 | `CallGraphProjectionPort` trait + 16 migrated files | `port-layer-hexagon` S3, S4 |
| 6 | 10 `QualityStore` methods + 10 in-crate tests | `quality-store-backend` S1, S2 |
| 7 | `grep NodePropertyReader` → 0 hits, renamed | `port-naming-convention` S1 |
| 8 | `grep explorer::ports::QualityStore\|GraphRepository` → 0 hits | `runtime-bootstrap-contract` S3 + `port-naming-convention` S2 |
| 9 | `ports/mod.rs` documents exactly 13 ports | `port-layer-hexagon` S5 |
| 10 | e28 conformance tests still pass | `port-layer-hexagon` S2 |
| 11 | build matrix (default + multimodal) green | cross-cutting (every spec asserts cargo check/test under `--no-default-features` and `[needs-multimodal]` markers document the multimodal requirement) |
| 12 | `grep PostgresRepository\|…` in `domain/ports/` → 0 | `port-layer-hexagon` S7 |

## Multimodal-gated scenarios (must hold under `--features multimodal`)

`port-layer-hexagon`: S5 (cfg-gated ports), S6 (deleted cfg-gated port), S7 (PG-leak grep)
`port-naming-convention`: S1, S2, S3 (workspace-wide grep under both builds)
`runtime-bootstrap-contract`: S1, S2, S3, S4 (the runner must compile under multimodal too)

## Test migration plan (from proposal)

Tests that **die** (3 PgBackend self-justifying tests):
- `crates/cognicode-runtime/tests/bootstrap_with_backend_smoke.rs::bootstrap_with_backend_signature_compiles` (line 215)
- `crates/cognicode-runtime/tests/bootstrap_with_backend_smoke.rs::ladybug_pg_backend_implements_pg_backend_for_bootstrap_with_backend` (line 220)
- `crates/cognicode-runtime/tests/backend_compat.rs::pg_backend_trait_object_supports_ladybug_backend` (whole file)
- Plus the obsolete `tests/ui/r2_as_postgres_repo_absent.rs` UI compile-fail scaffold.

Tests that **migrate**:
- `tests/bootstrap_with_backend_smoke.rs::r3_r5_ports_populated_from_backend_with_identity` (line 249) — rewrite to build `RuntimePorts { ... }` instead of `LadybugPgBackend`.

Tests that are **new**:
- 10 in-crate `LadybugStore` QualityStore tests (one per method, in `cognicode-ladybug`).
- Domain `CallGraphProjectionPort` trait + impl unit tests in `cognicode-core`.
- Runtime `bootstrap_with_backend` integration test rewritten around `RuntimePorts`.

## Acceptance criteria mapping

Proposal criteria → spec requirement:

| Criterion | Spec requirement |
|---|---|
| domain+application import zero infrastructure symbols | `port-layer-hexagon` Req "Hexagon Domain Direction" |
| 10 QualityStore tests green | `quality-store-backend` Req "10 in-crate tests" |
| no PgBackend anywhere | `runtime-bootstrap-contract` Req "PgBackend trait and LadybugPgBackend adapter are deleted" |
| DQS ≥ 0.75 | measured by `sddk-entropy-sdd` post-apply; no spec requirement here (entropy gate is a design-phase measurement) |
| 13 ports documented | `port-layer-hexagon` Req "Port catalog is reconciled" |

## Next recommended phase

`sddk-design` — translate each requirement into concrete file-level changes
(editor wiring, trait method extraction, lbug schema registration) before
the `sddk-tasks` phase breaks them into per-task units. The `sddk-design`
phase should also produce the entropy/DQS measurement sketch.
