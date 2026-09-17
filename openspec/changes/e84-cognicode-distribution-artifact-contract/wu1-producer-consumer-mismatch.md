# e84 WU1 — Producer / Consumer Mismatch (proven)

Claim under test:

```text
release producer contract  !=  cogh BundleManifest consumer contract
```

**Proven.** The two vocabularies diverge on naming, on content, on completeness,
on which artifacts are published, and on which profile resolves to what.

Source of the consumer side: `crates/cognicode-cli/src/cmd/installer_transaction.rs`
+ `bundles/v0.95.0/bundle.yaml`. Source of the producer side:
`.github/workflows/release.yml` (lines 121–141).

## What the producer emits

Per matrix lane (`release.yml:135` and `:139`), with `VERSION` = tag minus `v`:

```text
dist/cognicode-${VERSION}-${target}.tar.gz        contains: explorer-api, cognicode-mcp
dist/cognicode-cli-${VERSION}-${target}.tar.gz    contains: cogh
```

Published assets = `dist-all/*.tar.gz`. Nothing else.

## What the consumer expects

`bundles/v0.95.0/bundle.yaml`, `platform: linux-x86-64`:

```text
cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz   kind: Daemon   profiles: [reviewer, full]
skills-cognicode-core-0.95.0.tar.gz                    kind: Skill    profiles: [full]
sandbox-templates-0.95.0.tar.gz                        kind: Sandbox  profiles: [full]
```

And the consumer's resolution code (`InstallerTransaction`) does, per component:
download `comp.url` with `reqwest::blocking`, then `verify_sha256(path, comp.sha256)`.

## Mismatches

### M1 — Naming

| | Producer | Consumer |
|---|---|---|
| MCP artifact | `cognicode-0.95.0-linux-x86-64.tar.gz` | `cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz` |

Neither the prefix nor the platform vocabulary agrees. The producer uses the
*kebab* platform name (`linux-x86-64`, the `Platform` Display impl); the bundle
uses the *Rust target triple* (`x86_64-unknown-linux-gnu`) in the filename while
`platform:` uses the kebab name — **two vocabularies inside one bundle file**.

### M2 — Missing producer artifacts

`skills-cognicode-core-*.tar.gz` and `sandbox-templates-*.tar.gz` are referenced
by **both the `full` profile and the embedded manifest**, and `release.yml`
produces **no artifact by either name**. There is no skills-packaging step and no
sandbox-templates-packaging step anywhere in the workflow.

### M3 — Orphaned producer artifact

`explorer-api` **is shipped** by the producer (inside `cognicode-${VERSION}-*.tar.gz`)
and has **no `components[]` entry at all**. A produced artifact with no consumer
declaration.

### M4 — Co-packaging conflict

The producer packs `cognicode-mcp` **together with** `explorer-api` in one tarball.
The consumer treats each component as its own download URL. A component cannot be
extracted from a shared tarball without a sub-path selector, which the schema has
no field for.

### M5 — Placeholder digests pass validation

All 15 checksums across `bundles/v0.94.1` … `v0.95.0` are `000…0001/2/3`. They
pass `BundleManifest::validate()` because it only checks *length and hex-ness*.
There is no guard against a placeholder digest.

**Concrete consequence.** `cogh install` on v0.95.0 downloads the MCP artifact and
then **fails at `VerifyingSha256`** against `000…0001`. If the URL were also
unreachable (which it is — see M6) it fails earlier at `Downloading`. Either way
the pipeline cannot succeed, and nothing in the schema layer catches it first.

### M6 — Nothing is published

`gh release list --limit 100` → **5 releases, all `draft=true`**:

```text
v0.93.0  draft  assets=1  cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz
v0.5.0   draft  assets=4  cognicode, cognicode-mcp, mcp-client, sandbox-orchestrator
v0.4.1   draft  assets=1  cognicode-mcp
v0.3.0   draft  assets=0
v0.2.0   draft  assets=0
```

The `v0.95.0` tag has no release at all. The URLs in `bundle.yaml` resolve to
nothing. **The consumer's download path has never once resolved a real artifact.**

Note also that `v0.93.0`'s single asset is the *old* single-platform MCP naming —
a third naming scheme, which no current code produces or consumes.

### M7 — The default profile resolves to zero components

`bundles/v0.95.0/bundle.yaml` profiles:

```text
core      include_kinds: [Cogh, Cognicode]              ← declared intent
reviewer  include_kinds: [Cogh, Cognicode, Daemon]
full      include_kinds: [Cogh, Cognicode, Daemon, Skill, Sandbox]
```

But selection uses `components[].profiles`, and every component is `[reviewer, full]`
or `[full]`. `include_kinds` is **never read**.

Therefore `components_for_profile("core")` → **empty**, and `core` is the CLI
default (`#[arg(long, default_value = "core")]`).

**The default install path installs nothing.** The `core` profile promises
"Installer + daily CLI" and the bundle contains no `Cogh` or `Cognicode` component
at all — there is no entry for `cogh` itself (kind `Cogh` is "reserved; cogh
doesn't install itself") and none for the `cognicode` CLI either.

### M8 — The daily CLI is never built or packaged

`crates/cognicode-cli` declares **two** binaries:

```text
cognicode   → src/main.rs        ← the daily CLI, promised by the `core` profile
cogh        → src/bin/cogh.rs    ← the lifecycle manager
```

`release.yml` builds and packages only:

```text
cargo build --release -p cognicode-mcp -p cognicode-runtime --bin explorer-api
cargo build --release -p cognicode-cli --bin cogh --features cognicode-core/evidence-kernel
BIN_LIST=(explorer-api cognicode-mcp)     # → cognicode-${VERSION}-*.tar.gz
tar -czf ...cognicode-cli-... cogh       # → cognicode-cli-${VERSION}-*.tar.gz
```

So the shipped surface is **`explorer-api`, `cognicode-mcp`, `cogh`**. Neither
`cognicode` (the daily CLI) nor `explorer-mcp` (the second binary of
`cognicode-runtime`) is built or packaged anywhere in the workflow.

The `core` profile's promise ("Installer + daily CLI") is therefore doubly
unfulfillable: the component does not exist in the manifest **and** the artifact
does not exist in the release.

### M9 — The version under development has never been tagged

Workspace version = `0.95.0` (`Cargo.toml [workspace.package]`), and the embedded
manifest resolves to `bundles/v0.95.0/bundle.yaml`. But:

```text
git tag -l 'v0.95.0'                    → (nothing)
git ls-remote --tags origin 'v0.95.0'   → (nothing)
```

Newest local tags are `v0.94.15` and `v0.95.0-m6-m7-closure`. The tag `v0.95.0`
— the one every URL in `bundle.yaml` points at — **does not exist locally or on
the remote**, so the release workflow has never even had the chance to run for the
version currently in development.

## Summary

```text
M1  naming: kebab vs rust-triple, and a different prefix            → contract
M2  phantom artifacts: skills + sandbox-templates never produced    → contract
M3  orphan artifact: explorer-api has no component                  → contract
M4  co-packaging: two binaries per consumer artifact                → contract
M5  placeholder digests accepted by the schema                      → contract
M6  nothing published; all 5 releases are drafts                    → process
M7  default profile resolves to zero components                     → contract
M8  the daily CLI and explorer-mcp are never built or packaged      → producer
M9  the v0.95.0 tag does not exist; the version is unreleased       → process
```

Five of the nine (M1, M2, M3, M4, M7) are pure **vocabulary and completeness**
defects, fixable in the contract without new capability. M5 is the one that let it
all look green. M6 and M9 are process: the release path has never been executed
end-to-end.
