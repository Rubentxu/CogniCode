# e85 — CogniCode Linux Release Factory

## Intent

Produce the first internally coherent, verifiable, **real** CogniCode Linux
release. This cycle **implements** the artifact contract decided in e84; it does
not redesign distribution.

```text
source -> native Linux build -> one component = one artifact -> real SHA256
       -> generated ReleaseInventory -> generated BundleManifest v2
       -> release-verify -> attestations -> DRAFT release
       -> final verification -> published release
```

## The point

e84 proved the producer and the consumer did not speak the same artifact
language, and that nothing compared them. e85 makes them speak it, and makes the
comparison executable (`release-verify`) so it runs *before* anything is public.

## Inputs treated as immutable (e84)

```text
R1 derived artifact names      R2 one component per artifact
R3 no orphan produced payloads R4 no phantom manifest artifacts
R5 real computed digest        R6 manifest generated from artifacts
R7 components[].profiles authoritative    R8 workspace version == tag version
R9 Platform <-> target token total
KEEP_CUSTOM_RELEASE            Linux Tier 1: x86_64 + aarch64 GNU
```

## Non-goals (WU18)

```text
cogh latest / update implementation
user-facing cogh rollback
version resolution / remote bundle fetching (e86)
channel-aware cogh self-update
mise / install.sh / aqua / Homebrew / Nix (e87)
real fresh-Linux lifecycle UAT (e88)
Backstage / Control Plane
dist / cargo-dist (ADR-052 stands)
```

The only lifecycle work here is what the release producer contract strictly
requires.

## Deliverable

A published, non-draft GitHub release for v0.95.0 with both Tier-1 platforms,
independently generated manifests and `SHA256SUMS`, verified attestations, and no
placeholder digest anywhere.
