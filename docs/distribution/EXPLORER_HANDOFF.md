# Explorer decommission receipt

Where the Explorer lives now and what a consumer can rely on. Every statement
is a fact checked by `scripts/ci/test_explorer_handoff.py` or an explicit
limitation.

## What moved

- The Explorer lives in <https://github.com/Rubentxu/cognicode-explorer>. The
  migration boundary is its annotated tag `v0.102.1` (tag object `32d546e7`),
  on `main` at `3f8e10f2`, which pins core `v0.102.1`.
- `crates/cognicode-explorer` and `crates/cognicode-runtime` are no longer
  members of the root `Cargo.toml`, and `SDK_SURFACES.json` no longer
  classifies them. The last commit of this repository that built them as
  workspace members is `cdab6378` (`main`, release `v0.103.0`).

## What is no longer part of CogniCode

- The `cognicode-control-plane`, `explorer-api` and `explorer-mcp` binaries.
  None of them is a release asset (`INSTALL.md`, "Release assets").
- The Explorer has no Rust API promise here: see `SEMVER_POLICY.md`. The
  `cognicode`, `cognicode-mcp` and `cogh` contracts do not depend on it.

## How to consume it

Build or install the Explorer from the external repository at `v0.102.1` or a
later tag. This repository does not build it.

## Limitations

- The directories `crates/cognicode-explorer` and `crates/cognicode-runtime`
  are still tracked on disk, outside the workspace, and `Dockerfile`,
  `justfile` and `scripts/explorer/real_api_up.sh` still name them. Removing
  those paths depends on where the `explorer-api` host lives and is not done.
- `apps/explorer-ui` is still in this repository; its fate is undecided.
- The external repository's own dependency install, container build and
  release publication were not run for this receipt.
