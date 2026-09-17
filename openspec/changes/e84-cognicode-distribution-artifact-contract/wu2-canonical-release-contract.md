# e84 WU2 — Canonical Release Artifact Contract

One vocabulary, one producer, one consumer, no second taxonomy.

## The model

```text
ReleaseVersion   semver \d+\.\d+\.\d+(-suffix)?      tag = "v" + ReleaseVersion
Platform         the EXISTING kebab enum              linux-x86-64 | linux-aarch64 |
                 (bundle_manifest::Platform)          mac-os-x86-64 | mac-os-aarch64 | windows-x86-64
PlatformToken    rust target triple, DERIVED          x86_64-unknown-linux-gnu | aarch64-unknown-linux-gnu |
                 from Platform, total mapping         x86_64-apple-darwin | aarch64-apple-darwin |
                                                      x86_64-pc-windows-msvc
ArtifactKind     Cogh | Cognicode | Daemon | Skill | Sandbox | ExplorerApi
                 | BundleManifest | Checksums | Attestation
ArtifactName     {ArtifactKind}-{ReleaseVersion}-{PlatformToken}.{ext}
ArtifactDigest   "sha256:" + 64 lowercase hex, computed, never literal
ArtifactUrl      https://github.com/Rubentxu/CogniCode/releases/download/v{V}/{ArtifactName}
```

### Decision: `Platform` is reused, not replaced

`bundle_manifest::Platform` already exists, already has exactly the five variants
the release matrix uses, and the workflow comment already states the two must move
together. We add a **total, single-source mapping** `Platform → PlatformToken`.
We do **not** introduce a second platform enum.

This is what resolves WU1/M1: today `platform: linux-x86-64` and
`artifact: …x86_64-unknown-linux-gnu.tar.gz` are two unrelated string literals in
the same file. After the contract, the filename is *derived from* the platform.

## The nine rules

| # | Rule | Resolves |
|---|---|---|
| R1 | Artifact name is `{kind}-{version}-{platform_token}.{ext}` — derived, never typed by hand | M1 |
| R2 | **One component per artifact.** No two binaries share a tarball. | M3, M4 |
| R3 | Every produced artifact has exactly one `components[]` entry (no orphans) | M3 |
| R4 | Every `components[]` entry has a produced artifact (no phantoms) | M2 |
| R5 | `ArtifactDigest` is computed from the bytes and must be a real digest; an all-zero or sequential placeholder is a **hard error** | M5 |
| R6 | The manifest is generated *after* hashing and is published *as an asset* | M6 |
| R7 | Profile membership has **one** encoding. `include_kinds` is removed or made authoritative — never both. | M7 |
| R8 | `Version` is the workspace version, and the tag must equal `v{Version}` | M6 |
| R9 | `PlatformToken ↔ Platform` is total and defined in one place | M1 |

### R5 is the load-bearing rule

WU1/M5 is the reason a fundamentally broken distribution looked finished: the
schema accepted `0000…0001`. A contract that cannot reject a placeholder digest
cannot tell a release from a rehearsal. The gate is:

```text
reject if all bytes zero            reject if digest is one of the known placeholders
reject if digest is not 64 hex      reject if the digest was not computed this run
```

### R7 is the quietly load-bearing rule

`ProfileDef::include_kinds` is parsed into the struct and read by **nothing**.
Selection uses `components[].profiles`. Two encodings of the same fact, one inert,
and they disagree: `core` declares `[Cogh, Cognicode]` while every component is
`[reviewer, full]`/`[full]`, so the default profile resolves to zero components.

Decision: **`components[].profiles` is authoritative and `include_kinds` is
deleted.** Profile membership belongs on the component (where it travels with the
artifact); `include_kinds` is a second, coarser taxonomy that can only drift.
`validate()` must additionally reject any profile that resolves to zero
components — a profile that installs nothing is a contract violation, not a
valid configuration.

## Compatibility

`apiVersion: cognicode.bundle/v1` is unchanged in shape for `version`, `platform`,
`released_at`, `profiles[].name`, and `components[]` identity/version/artifact/url.
The breaking changes are: `sha256` becomes validated-and-computed, artifact names
become derived, one-component-per-artifact, and `include_kinds` removal. These are
a **v2** of the schema's *semantics* with the same major version only if we accept
in-place tightening; otherwise `cognicode.bundle/v2`. **Decision: bump to
`cognicode.bundle/v2`** — a placeholder digest accepted by v1 must not be
accepted under the same apiVersion that accepted it.
