# e84 WU8 — Release Manifest Generation

## The rule

> A manifest is **generated from produced artifacts**, never authored alongside
> them. A hand-written digest is not a digest.

## Pipeline

```text
1  build artifact            (cargo build --release --target <triple>)
2  package artifact          ({kind}-{version}-{platform_token}.tar.gz, ONE binary per archive)
3  compute SHA256            over the packaged bytes
4  publish artifact          to the GitHub Release for tag v{version}
5  GENERATE release manifest from steps 2-4 (names + computed digests, nothing typed)
6  publish manifest          as a release asset (bundle.yaml / release-manifest.json)
7  optional: publish SHA256SUMS and attestations
```

Everything mechanical in steps 1–7 is a candidate for the release factory (WU5).
Steps 5–6 are CogniCode-owned regardless of which factory is used, because the
bundle/profile semantics are ours.

## The release MUST fail if

```text
manifest references an artifact that is not among the produced artifacts   (phantom)
produced artifact has no manifest entry                                    (orphan)
computed checksum differs from the manifest's                              (digest drift)
platform token does not match the Platform the artifact was built for       (platform mismatch)
artifact version != ReleaseVersion, or tag != "v"+ReleaseVersion           (version mismatch)
any digest is a placeholder (all-zero, or sequential)                       (rehearsal)
```

Any one of these fails the **release**, not the consumer. The invariant is that a
published release is always internally consistent, so `cogh` never has to reason
about a broken manifest.

## The verification gate

```text
just release-verify   →  for each artifact:
                            exists, digest matches manifest, platform matches,
                            version matches, no orphans, no phantoms
                         prints a single PASS/FAIL
```

This gate is the executable form of the contract and must be runnable **locally
against a dry-run build** (`KEEP_CUSTOM_RELEASE` and `USE_DIST` alike), so the
contract can be validated before a tag is ever pushed.

## Beyond the manifest: checksums and attestations (WU6 tie-in)

WU6 found a cross-cutting gate that belongs to *this* work unit, not to any channel:

> The release currently publishes **tarballs only** — no `checksums.txt`/`.sha256`
> asset and no attestations. Every channel's "verify the download" story is
> therefore conditional on us publishing those artifacts at all.

So steps 5–7 of the pipeline must additionally emit, as **release assets**:

```text
SHA256SUMS                      (or per-asset .sha256)  ← unlocks channel-native checksum verification
GitHub Artifact Attestations    (actions/attest-build-provenance)
```

This is the single change with the widest downstream leverage: it is what lets the
mise GitHub backend disambiguate and attest, what aqua and Homebrew can enforce,
and what makes a plain `install.sh` trustworthy. It costs one workflow step and it
removes the "conditional on us publishing X" caveat from the entire channel matrix.

`ArtifactKind` already carries `Checksums` and `Attestation` (WU2), so these are
first-class artifacts of the contract rather than loose files.

## Why this is the fix for the current state

Today the manifest is authored *ahead of* the artifacts, by hand, with
`0000…0001/2/3`, and the schema accepts those values. Every downstream symptom in
WU1 flows from that single inversion: the manifest is a wish, the release is a
reality, and nothing compares them.

Inverting the direction — **generate the manifest from the artifacts** — makes the
class of defect impossible rather than merely detectable, and `release-verify`
makes any residual inconsistency fail before publication.
