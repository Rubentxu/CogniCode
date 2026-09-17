# `bundles/` — retired in e85

This directory used to hold hand-authored, version-pinned `bundle.yaml` files
(`v0.94.1` … `v0.95.0`) that were embedded into `cogh` via `include_str!`.

They were **retired in e85**. Every one of them carried placeholder digests
(`0000…0001/2/3`), and under schema v1 the validator accepted those because it
only checked length and hexadecimal-ness. That is what allowed a fundamentally
broken release contract to look finished (see
`openspec/changes/e84-cognicode-distribution-artifact-contract/wu1-*`).

## What replaced them

Bundle manifests are now **generated from produced artifacts**:

```text
build artifact  ->  hash artifact  ->  generate BundleManifest v2
                                    ->  generate ReleaseInventory
                                    ->  generate SHA256SUMS
                                    ->  publish as release assets
```

`cogh` resolves a manifest explicitly (`COGNICODE_BUNDLE_MANIFEST`, or a manifest
in `COGNICODE_HOME`). A well-formed **dev-only** fixture lives next to the code at
`crates/cognicode-cli/src/cmd/dev-bundle.yaml`; it is not authoritative and is not
a published release manifest.

Remote resolution of `version + platform -> published BundleManifest v2` is **e86**.
