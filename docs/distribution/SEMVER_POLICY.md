# Semver and API compatibility policy

This is the only compatibility policy for the CogniCode Rust SDK. The crate
READMEs link here; they do not restate it. Which crates are libraries is
declared in `SDK_SURFACES.json`.

## Scope

The policy covers the four library crates and nothing else:
`cognicode-core`, `cognicode-graph-algos`, `cognicode-macros` and
`cognicode-graph-wasm`. The applications (`cognicode`, `cognicode-mcp`,
`cogh`, the explorer) and the internal crates carry no Rust API promise; their
published contracts are the CLI flags, the MCP tool catalogue and the JSON
schemas, each versioned by its own manifest.

What is covered per crate:

- `cognicode-core`: only what the crate root re-exports (`WorkspaceSession`,
  `WorkspaceError`, `WorkspaceResult` and the `dto` module). The other `pub`
  modules exist because the workspace binaries need them; they may change in
  any release and are not part of the SDK.
- `cognicode-graph-algos`: the free functions and types re-exported at the
  crate root, plus `GraphBuilder` and `AnalyticsError`.
- `cognicode-macros`: the names and accepted syntax of the macros.
- `cognicode-graph-wasm`: the exported wasm functions and the JSON protocol
  types in `protocol`.

## Breaking changes

All four libraries share one workspace version and are released together.
While the major version is 0, a change to the minor number may break the
covered API; a patch release never does. From 1.0.0 on, only a major release
may break it.

A change is breaking when a consumer of the covered API that compiled before
no longer compiles or changes behavior: removing or renaming an item, adding a
required trait method or struct field, changing a signature, tightening a
generic bound, changing a documented error or returned value, or changing the
wire format of a published JSON protocol. Adding an item, a defaulted trait
method or an enum variant on a `#[non_exhaustive]` type is not.

## Features

Features are additive: enabling one never removes an item or changes the
meaning of a covered one. The features each crate declares are documented in
its README, and a contract keeps that list equal to the manifest. The default
set is part of the covered API: removing something from it is breaking.
Features named `test-support` or ending in `-ladybug` serve the workspace
binaries and may change in any release.

## Toolchain

The minimum supported Rust version is the `rust-version` each manifest
declares, and it is the floor the product publishes in `manifest.toml`.
Raising it is announced in the release notes and happens only in a minor
release while the major version is 0, and in a major release afterwards. It is
never raised in a patch release.

## Deprecation

A covered item is first marked `#[deprecated]` with the replacement named in
the message, and stays for at least one minor release before removal. A
security fix may skip that period and says so in the release notes.

## Release cadence

Releases are cut on demand from `main`, in dependency order:
`cognicode-graph-algos`, `cognicode-macros`, `cognicode-core`,
`cognicode-graph-wasm`. A published version is never overwritten; a defective
one is yanked and superseded by a patch release.
