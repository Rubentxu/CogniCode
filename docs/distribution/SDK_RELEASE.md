# SDK release guide

How the four library crates relate, what was measured to build, and how a
release is cut and rolled back. Which crates are libraries is declared in
`SDK_SURFACES.json`; what may change between versions is the
[semver policy](SEMVER_POLICY.md). Applications and internal crates are out of
scope and carry no Rust API promise.

## Dependency graph

Normal dependencies between the library crates, as `cargo metadata` reports
them. A contract compares this list with the manifests.

- `cognicode-core` -> `cognicode-graph-algos`, `cognicode-macros`
- `cognicode-graph-algos` ->
- `cognicode-macros` ->
- `cognicode-graph-wasm` -> `cognicode-graph-algos`

## Supported feature combinations

Each row was built with `cargo check --offline --locked -p <crate> <flags>`
on the host target. `checked` means that build succeeded; it says nothing
about the tests. `not measured` is a gap, not a promise. The `--no-default-features`
rows of a crate whose default set is empty build the same code as its default
row; they are listed so the claim is explicit.

| Crate | Flags | Status |
|---|---|---|
| `cognicode-macros` | `(default)` | checked |
| `cognicode-graph-wasm` | `(default)` | checked |
| `cognicode-graph-wasm` | `--no-default-features` | checked |
| `cognicode-graph-algos` | `(default)` | checked |
| `cognicode-graph-algos` | `--no-default-features` | checked |
| `cognicode-graph-algos` | `--no-default-features --features petgraph-adapter` | checked |
| `cognicode-core` | `(default)` | checked |
| `cognicode-core` | `--no-default-features` | checked |
| `cognicode-core` | `--no-default-features --features persistence` | checked |
| `cognicode-core` | `--no-default-features --features multimodal` | checked |
| `cognicode-core` | `--no-default-features --features ownership` | checked |
| `cognicode-core` | `--no-default-features --features evidence-kernel` | checked |
| `cognicode-core` | `--no-default-features --features program-analysis-server` | checked |
| `cognicode-core` | `--no-default-features --features contract-store` | checked |
| `cognicode-core` | `--no-default-features --features test-support` | checked |
| `cognicode-core` | `--no-default-features --features evidence-cli-ladybug` | checked |
| `cognicode-core` | `--all-features` | not measured |

Pairs and larger sets of `cognicode-core` features were not built either.
Targets: `cognicode-graph-algos` and `cognicode-graph-wasm` also build for
`wasm32-unknown-unknown`; `cognicode-core` does not (its dependency `mio` fails
to compile there), so it is a native-only library.

## Toolchain

The minimum supported Rust version is the `rust-version` of each manifest,
inherited from the workspace. The product publishes the same floor in
`manifest.toml`, and `rust-toolchain.toml` pins the toolchain the repository is
verified with; a contract keeps the manifests and `manifest.toml` equal. No
lower floor was measured, so none is claimed.

## Examples

There are no `macros` or `graph-wasm` programs under `examples/`, and the one in
`core` is a measurement instrument, not a usage example; the runnable material
for those crates is the documentation tests that `cargo test --doc` executes
(core: 2 run, macros: 6 are marked `ignore`).

- `cognicode-graph-algos`: `examples/page_rank.rs`, run with `cargo run --example page_rank -p cognicode-graph-algos`
- `cognicode-core`: `examples/measure_incremental_scanner.rs`, a measurement instrument for the incremental scanner (not a product route), run with `cargo run --example measure_incremental_scanner -p cognicode-core`
- `cognicode-macros`: none yet
- `cognicode-graph-wasm`: none yet

## Release and rollback

Publication order follows the dependency graph, dependencies first:

1. `cognicode-graph-algos`
2. `cognicode-macros`
3. `cognicode-core`
4. `cognicode-graph-wasm`

Procedure, from a clean checkout of `main` at the release commit:

1. Set the version in the root `Cargo.toml`: `[workspace.package]` and the
   three `version =` of the internal entries in `[workspace.dependencies]`,
   which must equal it (a contract checks this). The four crates inherit it.
2. Rehearse the whole release without uploading: `bash
   scripts/distribution/rehearse-sdk-publish.sh`. It runs one
   `cargo publish --dry-run --locked` that names the four crates (it runs the
   same `cargo package` build of each tarball that a real publish would), so `core` and
   `graph-wasm` verify against the packaged copy of `graph-algos` even though it
   is not yet on the registry. Running the dry-run per crate fails for those two
   (`no matching package named cognicode-graph-algos`); that is the ordering,
   not a defect. Fix anything it reports before publishing.
3. Check what would stop the upload, read-only: `bash
   scripts/distribution/preflight-sdk-publish.sh`. It prints `READY` or
   `BLOCKED` naming each missing condition (registry credential, a crate name
   or version already taken on crates.io, an unclean tree, `HEAD` not at
   `origin/main`) and exits non-zero unless ready. It never publishes or logs in.
4. Publish in the order above with the operator's registry token, waiting for
   each crate to be indexed before publishing the next.
5. Verify from a clean consumer project that depends on the published
   versions only: `bash scripts/distribution/verify-sdk-consumer.sh
   --confirm-published` (read-only; it builds a fresh project with exact
   `=<version>` pins and rejects any path dependency).
6. Tag the release commit.

Rollback: a published version cannot be overwritten or deleted. If a release is
defective, run `cargo yank --version <x.y.z> <crate>` for every affected crate
(this stops new resolutions and leaves existing lockfiles working), then
publish a patch release with the fix, following the compatibility rules in
`SEMVER_POLICY.md`. A release that fails halfway is completed or yanked in the
same order; consumers must never be left with a published crate whose
dependency is missing.
