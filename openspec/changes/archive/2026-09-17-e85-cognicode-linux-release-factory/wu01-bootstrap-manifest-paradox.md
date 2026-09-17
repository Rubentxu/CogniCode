# e85 WU0.1 — The Bootstrap Manifest Paradox (resolved)

## The paradox

e84 requires:

```text
build artifact -> hash artifact -> GENERATE the manifest
```

But `cogh` had:

```rust
include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../bundles/v",
                     env!("CARGO_PKG_VERSION"), "/bundle.yaml"))
```

A **version-pinned manifest committed before the artifacts exist**. A manifest
generated from artifacts cannot be embedded in the binary that is itself one of
those artifacts, without either freezing a lie into the binary or making the
binary's content depend on its own hash.

## What was forbidden as a resolution

```text
placeholder checksums            (the v1 disease; now rejected by ArtifactDigest)
pre-computed fake digests        (same, one indirection deeper)
CI rewriting a committed manifest(creates a file that lies about its origin)
cogh installing itself           (collapses Layer 0 into Layer 1)
```

None of these was used.

## The resolution: three separate concepts

```text
ReleaseInventory          everything published, BOTH layers, plus meta artifacts
   ├── Layer 0 payload : cogh
   ├── Layer 1 payload : cognicode, cognicode-mcp
   └── meta            : BundleManifest, ReleaseInventory, SHA256SUMS

BundleManifest v2         a PROJECTION: only Layer 1 installable components
                          (cogh is absent by construction, not by omission)

cogh                      the Layer 0 bootstrap; it CONSUMES a manifest
```

**One produced-artifact ledger drives both.** `generate_release` scans and hashes
the staged payloads once, builds the `ReleaseInventory` from that single scan, and
derives each platform's `BundleManifest v2` from the same in-memory inventory. No
fact is authored twice.

### R3 clarified (a clarification, not a weakening)

```text
every produced PAYLOAD artifact        must appear in ReleaseInventory
every Layer 1 INSTALLABLE artifact     must appear exactly once in BundleManifest
```

`cogh`, `SHA256SUMS`, the inventory and attestation metadata are **release
artifacts**. They are not runtime components and must never appear as bundle
components. `ArtifactKind::layer()` encodes exactly that split, and
`BundleManifest::validate` rejects any non-installable kind inside a bundle, so
`cogh` cannot be smuggled into the install plan to satisfy a completeness rule.

## How `cogh` resolves a manifest now

```text
1. COGNICODE_BUNDLE_MANIFEST        explicit path (release tool / test / user)
2. ~/.cognicode/bundle.yaml         a manifest in COGNICODE_HOME
3. DEV-ONLY fixture                 announced with a loud warning
```

The dev fixture exists so offline development and the crate's own tests have a
well-formed v2 manifest. Its digests are **not** the digests of any real artifact,
so an install driven by it fails at the SHA256 stage by construction — it cannot
silently "succeed" the way the v1 placeholder bundle did when the `core` profile
resolved to zero components.

## The e86 seam

Remote resolution of `version + platform -> published BundleManifest v2` is
**e86**. e85 leaves the seam explicit: the precedence above is the extension point,
and the fetch origin is already overridable (`COGNICODE_RELEASE_BASE_URL`), so e86
adds resolution without touching the contract.

This is load-bearing because e84 established that

```text
cogh bootstrap version  !=  installed CogniCode runtime version
```

is allowed. A version-pinned embedded manifest can therefore never be the
long-term production authority, and after e85 it no longer is.
