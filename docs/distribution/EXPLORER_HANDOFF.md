# Explorer handoff

Where the Explorer lives today and what a consumer can rely on. Every statement
is a fact checked by `scripts/ci/test_explorer_handoff.py` or an explicit
limitation.

## Current state

- The Explorer crate is still in this workspace: `crates/cognicode-explorer`,
  a member of the root `Cargo.toml`. It ships the `cognicode-control-plane`
  binary and is classified as an application, not as an SDK library
  (`SDK_SURFACES.json`).
- An external repository, <https://github.com/Rubentxu/cognicode-explorer>,
  carries the extraction. Its reconciliation with this core is tracked as
  TASK-181; the change that pins core `v0.102.1` and migrates the remaining
  runtime pieces is a pull request that is **not merged** (limitation).
- Nothing in this repository has been removed. The decision to decommission the
  in-tree copy belongs to TASK-177 and has not been taken.

## What a consumer can rely on

- Use the in-tree crate for anything you build from this repository.
- `explorer-api` and `explorer-mcp` are not published as release assets
  (`INSTALL.md`, "Release assets"). The `cognicode`, `cognicode-mcp` and `cogh`
  contracts do not depend on the Explorer.
- The Explorer has no Rust API promise: see `SEMVER_POLICY.md`.

## Limitations

- Until the external pull request is merged and a tag fixes its core pin, this
  document cannot name a published Explorer release.
- When that happens, update this file in the same change that closes TASK-177.
