# e84 WU9 — Distribution Support Tiers

A support policy. It is **not** an instruction to delete working code.

```text
TIER 1 — supported, published, tested
  Linux x86_64 GNU      x86_64-unknown-linux-gnu     ubuntu-latest
  Linux aarch64 GNU     aarch64-unknown-linux-gnu    ubuntu-24.04-arm

TIER 2 — retained, not promised
  Linux x86_64 MUSL     x86_64-unknown-linux-musl
  Linux aarch64 MUSL    aarch64-unknown-linux-musl
  macOS x86_64          x86_64-apple-darwin
  macOS arm64           aarch64-apple-darwin
  Windows x86_64        x86_64-pc-windows-msvc
```

## What "tier" means

| | Tier 1 | Tier 2 |
|---|---|---|
| Published on every release | must | no promise |
| Install-smoke on a native runner | required | retains its lane, not gated |
| UAT (e88) | required | no |
| May break without a release waiver | no | yes, with a recorded waiver |

## The existing seams are preserved

`release.yml` defines five native lanes and
`crates/cognicode-cli/src/cmd/platform_adapter.rs` (533 LOC) implements all five
platforms. **Nothing is removed.** `Platform` keeps all five variants, the
`PlatformToken` mapping (WU2) is total across all five, and `ComponentKind`,
`BundleManifest`, and the adapter traits keep their cross-platform shape.

Concretely: a Tier 2 lane that is not exercised this cycle keeps its code, keeps
its place in the mapping, and simply is not a release blocker. The workflow's own
existing rule still applies unchanged:

> if a target cannot honestly be delivered, the lane is removed AND a typed waiver
> is recorded — silent substitution is forbidden.

## Why Linux first

Tier 1 is where the demand and the CI infrastructure both are: GitHub-hosted
`ubuntu-24.04-arm` makes a **native** aarch64 lane possible, which is what the
"no cross-compilation to fake platform support" rule requires. It is the cheapest
honest coverage available.
