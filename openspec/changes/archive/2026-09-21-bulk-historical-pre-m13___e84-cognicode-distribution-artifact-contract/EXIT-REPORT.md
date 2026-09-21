# e84 — Exit Report

Cycle: **Distribution Reality & Artifact Contract**
Kind: bounded distribution/release characterization and contract cycle.
Scope honoured: no Control Plane, no core redesign, no generic version manager.

---

## Required status lines

```text
existing cogh foundation          REUSED
release producer mismatch         CHARACTERIZED
canonical artifact contract       CLOSED
bootstrap/runtime ownership       CLOSED
Linux Tier 1 targets              CLOSED
future OS extension seam          PRESERVED

dist decision                     KEEP_CUSTOM_RELEASE
mise decision                     PRIMARY (Layer 0 channel)
asdf decision                     COMPATIBILITY ONLY (custom plugin; not first wave)
aqua decision                     SECONDARY
Homebrew decision                 SECONDARY
SDKMAN decision                   NOT INITIALLY JUSTIFIED
proto decision                    NOT INITIALLY JUSTIFIED
Nix decision                      SECONDARY

real artifact publishing          NOT STARTED / e85
update lifecycle                  NOT STARTED / e86
external channels                 NOT STARTED / e87
real Linux install UAT            NOT STARTED / e88

CP0 Backstage                     DEFERRED UNTIL e84-e88
```

## What each line means

**`existing cogh foundation` — REUSED.** Nothing was redesigned or replaced. The
atomic `InstallerTransaction` (staged Resolve → Download → SHA256 → Extract →
Shims → Manifest → Commit/Rollback), the rollback journal, the install lock, the
platform adapters (533 LOC, 5 platforms), `BundleManifest`, `COGNICODE_HOME`, the
native release matrix and the IDE adapters are all kept as-is.

**`release producer mismatch` — CHARACTERIZED.** Nine mismatches proven, not
asserted (`wu1-producer-consumer-mismatch.md`): M1 naming (kebab vs Rust triple,
different prefix), M2 phantom artifacts (skills + sandbox-templates never
produced), M3 orphan artifact (`explorer-api` has no component), M4 co-packaging,
M5 placeholder digests accepted by the schema, M6 nothing published (all 5 GitHub
releases are drafts), M7 the default `core` profile resolves to **zero
components**, M8 the daily `cognicode` CLI and `explorer-mcp` are never built or
packaged, M9 the `v0.95.0` tag does not exist so the release path has never run
for the version in development.

**`canonical artifact contract` — CLOSED** as a *decision*, in
`wu2-canonical-release-contract.md`: one vocabulary (`ReleaseVersion`, `Platform`
reused not duplicated, `PlatformToken` derived, `ArtifactKind`, `ArtifactName`
derived, `ArtifactDigest` computed, `ArtifactUrl`) and nine rules R1–R9. Schema
bumps to `cognicode.bundle/v2` because a placeholder digest accepted under v1 must
not be accepted under the same apiVersion that accepted it. Implementation is e85.

**`bootstrap/runtime ownership` — CLOSED** (`wu3`, `wu7`). Layer 0 (installing
`cogh`) is owned by an external channel; Layer 1 (`cogh` installing CogniCode) is
owned by `cogh`. `cogh update` means *update the runtime bundle*, never
self-replacement. Any future `cogh self-update` must be provenance-aware and must
refuse when an external channel owns the binary.

**`Linux Tier 1 targets` — CLOSED** (`wu9`): `x86_64-unknown-linux-gnu` and
`aarch64-unknown-linux-gnu`, both on **native** runners (`ubuntu-latest`,
`ubuntu-24.04-arm`).

**`future OS extension seam` — PRESERVED** (`wu9`). All five `Platform` variants,
the platform adapters, and all matrix lanes stay. Tier 2 = MUSL, macOS x86_64,
macOS arm64, Windows x86_64. Nothing deleted.

---

## The two decisions that contradict the prior hypothesis

### `dist` → **`KEEP_CUSTOM_RELEASE`** (not the expected `USE_DIST_WITH_CUSTOM_BUNDLE_STAGE`)

**This confirms an existing decision rather than making a new one.** `ADR-052`
(*"Rechazar la adopción de `cargo-dist` durante e74"*) is already **ACEPTADO**
(2026-09-16, e74 WU5). WU5 re-derived the same outcome independently, from the
current tree and current `dist` behaviour, without taking the ADR as given. No new
ADR is required for `dist`; **ADR-052 stands.**

The prior hypothesis is *mechanically valid* but not supported by the evidence, so
per WU5's own hard rule the conservative outcome wins:

- `dist` **can** host custom assets (`extra-artifacts` verified), which removes the
  strongest a-priori objection — but `extra-artifacts` is **package-local and
  build-time**, so cross-lane per-component SHA256 bundle assembly **must remain a
  bespoke CogniCode job**. Adoption *re-homes* the handcrafted stage instead of
  deleting it.
- `install-smoke` (run the produced binaries on a clean `HOME`) has **no `dist`
  equivalent** and survives as custom code under every outcome, so the "less code"
  premise is not met.
- Migration would break a checked-in invariant: `scripts/check-release-matrix.sh`
  textually parses `release.yml`. That is cost, not savings.
- Several needed capabilities are **unverified**: build-feature parity for
  `--features cognicode-core/evidence-kernel`, archive name/content parity,
  byte-reproducible archives.
- WU5 also found one claim in ADR-052's comparison table **overstated**: macOS and
  Sigstore signing are recorded there as available, whereas the current upstream
  issues leave them open. The ADR's *decision* is unaffected and remains correct;
  only that supporting cell needs correcting in the local working copy.
  (LOCAL: `docs/adr/**` is never pushed. ADR-052 needs no status change.)

**Re-evaluation trigger:** only when a *verified* signing/attestation need exists
that the current machinery cannot meet.

### Channels → **Primary `install.sh` + mise**, with a refinement

Confirmed, with one correction: mise is **not zero-config for us**. We ship two
tarballs per target and our tokens are non-standard (`linux-x86-64`, `mac-os-x86-64`,
`mac-os-aarch64`), so mise needs a small **declarative tool definition**
(`asset_pattern`/`matching` + `rename_exe`), not a plugin.

Two prior claims were **contradicted** by the evidence:
- **cargo-binstall does not work from our tarballs as-is** — its default `pkg-url`
  assumes standard filenames with Rust triples, so it needs
  `[package.metadata.binstall]` *plus* a crates.io publish.
- **SDKMAN is not JVM-only** and can ship arbitrary multiplatform binaries; it
  stays not-justified for a different reason (vendor onboarding + credentials +
  a release-API integration we would own).

| Channel | Role |
|---|---|
| `install.sh` + mise | **Primary** |
| aqua, Homebrew, Nix | **Secondary** |
| asdf, cargo-binstall | **Compatibility** |
| SDKMAN, proto | **Not initially justified** |

---

## Cross-cutting finding with the widest leverage

The release publishes **tarballs only** — no `SHA256SUMS`/`.sha256` asset and no
attestations. Therefore **every** channel's checksum and attestation story is
*conditional on us publishing those artifacts at all*. This is a single workflow
step, it belongs to the e84 contract (`ArtifactKind::Checksums` /
`ArtifactKind::Attestation`), and it removes the caveat from the entire channel
matrix. Recorded in `wu8`.

## Additional finding

`scripts/check-release-matrix.sh` (4431 bytes) enforces exactly the right
invariant — YAML matrix ↔ `BundleManifest::Platform` coherence plus a runner
allowlist — and is **never invoked** by the `justfile` or any workflow. In practice
it is a manual check, though e74's ledger records it under a "maintainers / CI"
column. A drift guard that is not wired in cannot prevent drift between the matrix
and the Rust enum, which is precisely the coherence WU2/R9 makes total. Wiring it
into CI is a one-line change and a candidate for e85.

---

## Traceability

| WU | Artifact |
|---|---|
| WU0 | `wu0-distribution-characterization.md` |
| WU1 | `wu1-producer-consumer-mismatch.md` (M1–M9) |
| WU2 | `wu2-canonical-release-contract.md` (R1–R9) |
| WU3 | `wu3-bootstrap-vs-lifecycle.md` |
| WU4 | `wu4-adr-035-revisit.md` (LOCAL — ADR edit not pushed) |
| WU5 | `wu5-release-factory-evaluation.md` → KEEP_CUSTOM_RELEASE |
| WU6 | `wu6-installation-channels-matrix.md` → Primary/Secondary/Compatibility |
| WU7 | `wu7-installation-ownership.md` |
| WU8 | `wu8-release-manifest-generation.md` |
| WU9 | `wu9-support-tiers.md` |
| WU10 | confirmed in `proposal.md` |

## Verification character of this cycle

This cycle's output is **analysis and contract decisions**, not executable code, so
its evidence is *observation*: files read, `gh` queries run, and the two delegated
evaluations whose decisive claims I re-verified independently
(`scripts/check-release-matrix.sh` exists and parses `release.yml`;
`bundles/v0.95.0/bundle.yaml` carries the placeholder digests; all 5 GitHub
releases are drafts; `include_kinds` has no reader). No code was changed, so no
build or test result is claimed.

## STOP

**Stopping here for review before e85**, as instructed.
