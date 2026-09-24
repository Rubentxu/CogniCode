# DISTRIBUTION SCOPE — v0.98.0 contract

> **Source**: synthesis from JOURNAL §105, §112 v2 (T5 snapshot 0.97.4),
> §135 (release v0.98.0 publication), and `.github/workflows/release.yml`
> actual matrix. Validates H12 of `docs/prf/AUDIT-2026-09-22-FINDINGS.md`
> with concrete data points; does NOT close H12, only advances its
> WIP state from "scattered across docs" to "single authoritative
> declaration of what is and is not in scope".
>
> **Update policy**: this file is the canonical reference for what
> the release pipeline produces and validates. Edits require
> appending to JOURNAL §N (PRF traceability), but do NOT require
> a release tag (the file's content is data + declaration, not
> contract closure).

## What "Supported" means in this contract

A platform is **Supported** if, at the commit the operator triggers
a release, **ALL** of the following hold:

1. The `release.yml` matrix lists the platform's rust target
   (`platform: <x>`, `runner: <runner>`, `rust_target: <triple>`).
2. The CI runner uses a native runner for that target (no cross-compile).
3. The build job produces a per-component archive under
   `dist/<component>-<version>-<rust_target>.tar.gz` (NOT a
   generic-place build). Verified by `scripts/ci/stage-platform-payloads.sh`.
4. The `assemble-and-publish` job produces `release/bundle-<version>-<rust_target>.yaml`,
   `release/SHA256SUMS`, and `release/release-inventory-<version>.json`
   referencing that target.
5. `scripts/ci/release-install-smoke.sh` (called before publication)
   executes the published archive on a clean `HOME` and verifies:
   - `cogh --version` matches the tag version,
   - `cognicode --version` matches the tag version,
   - `cognicode-mcp --version` matches the tag version,
   - `cogh doctor` reports `PASS  MCP`,
   - `cogh install mcp-server --version <v>` + `cogh update` + `cogh doctor`
     all green,
   - `cogh reshim` + `cogh uninstall` + jq assertions pass,
   - skills `cognicode` + `cognicode-mcp` are installed at expected paths.
6. The `verify-rejects-missing-artifact` and `verify-rejects-altered-artifact`
   negative-test jobs in `release-validate.yml` (re-run from the
   validate-mode path) prove that `release-verify` rejects corruption
   of the production set.

Anything failing any of these criteria for a given platform puts that
platform out of the **Supported** set for that release.

## Tier-1 (currently Supported)

Per `release.yml` strategy matrix and the verdicts of release runs
`#36034410448` (v0.98.0, SUCCESS), `#36026057157` (release-validate
on 5/5 verde), and the negative tests in `release-validate.yml`:

| Platform | Rust target | Runner | Status @ v0.98.0 |
|---|---|---|---|
| `linux-x86-64` | `x86_64-unknown-linux-gnu` | `ubuntu-latest` | **Supported** |
| `linux-aarch64` | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | **Supported** |

**5/5 SUCCESS** on both platforms for v0.98.0 (`93b7a9a3` pre-bump,
`8505ad85` post-bump).

## Tier-1.5 (declared in matrix, NOT exercised this release run)

NONE. Per the matrix above, only two platforms are even attempted.

## Tier-2 (NOT supported — would require runner + matrix change)

| Platform | Rust target | Status | Why not |
|---|---|---|---|
| `linux-musl-x86-64` | `x86_64-unknown-alpine-linux-musl` | **Not supported** | Tier-1 contract is GNU-only; Alpine is not a Tier-1 platform per `R8 tag == v{workspace version}` / `R9 Platform <-> target token is total` in `release_contract.rs` (release.yml comments). |
| `darwin-x86-64` | `x86_64-apple-darwin` | **Not supported** | No macOS runner in the matrix; cross-compile from Linux adds a separate toolchain surface (osxcross). |
| `darwin-aarch64` | `aarch64-apple-darwin` | **Not supported** | Same. |
| `windows-x86-64` | `x86_64-pc-windows-msvc` | **Not supported** | No Windows runner in the matrix; MSVC toolchain + signing/Codesign policies out of scope. |
| `windows-aarch64` | `aarch64-pc-windows-msvc` | **Not supported** | Same. |

These would each need a separate release artifact set per the contract
(`R2 one component per artifact` + `R5 computed digests only`), a runner
change, and a CI cost decision. Out of the v0.98.0 contract.

## Tier-3 (not even Tier-2 — explicitly excluded by the contract)

Per the comment block at the top of `release.yml`:

> **Tier 1 is Linux x86_64 and aarch64 GNU on NATIVE runners. The
> Platform variants and adapter seams for macOS, Windows and MUSL are
> preserved in the Rust contract, but they are not advertised as supported
> until they pass the same gates.**

The "Platform variants and adapter seams" reference indicates that
`crates/cognicode-release` and `cognicode-core`'s `release_contract.rs`
declare the matrix of (platform, adapter) pairs but only mark Tier-1 as
`published: true` for the surface calculation. Tier-2/3 are
**adapter seams present in code, behavior gated to "DO NOT PUBLISH"
until explicit Tier promotion**.

## `SkillBundleSpec` entries (related to H12)

The release contract has `SkillBundleSpec` entries for skill bundles
in `crates/cognicode-cli/src/cmd/release_contract.rs`:

```
pub const SKILL_BUNDLES: &[SkillBundleSpec] = &[
    SkillBundleSpec { id: "cognicode",         profiles: &["core", "reviewer"], published: true,  },
    SkillBundleSpec { id: "cognicode-mcp",     profiles: &["reviewer"],        published: true,  },
    SkillBundleSpec { id: "cognicode-developer", profiles: &[],                published: false, },
];
```

Of these:

- **`cogh`** and **`cognicode`** are Layer 0/1 components published
  by every release (see `COMPONENTS` table in `release_contract.rs`,
  both `published: true`).
- **`cognicode`** and **`cognicode-mcp`** are the two published skill
  bundles (`SKILL_BUNDLES` table, both `published: true`). Validated
  by `release-install-smoke.sh` assertions on
  `~/.config/opencode/skills/cognicode-$VERSION` and
  `~/.config/opencode/skills/cognicode-mcp-$VERSION`.
- **`cognicode-developer`** exists in the codebase (both a
  `skills/cognicode-developer/` directory AND a `SkillBundleSpec`
  entry) but is `published: false` — the staging step (see
  `release.yml` "Stage declared skill bundle payloads (DEBT-2)")
  iterates `published_skill_bundles()` and skips entries where
  `published == false`. **Not validated by any release run.**
  For this bundle to reach users, an operator must flip
  `published: false → true` in `release_contract.rs` AND verify
  the bundle's full smoke (install + doctor + reshim) on at least
  one Tier-1 platform.
- `skills/cognicode-recommended/` exists on disk but is NOT
  referenced in `SKILL_BUNDLES` at all (no `SkillBundleSpec`
  entry), so the staging step never finds it. (It would be
  treated as orphan or phantom depending on the gate path — see
  the relevant `release_contract.rs` and `release_factory.rs`
  logic; this is out of scope for distribution-shape validation.)

This is consistent with the audit's H12 sub-point: "alcance menor
que el producto descrito". Specifically, `cognicode-developer` and
`cognicode-recommended` are part of the development product surface
but are not yet wired into the release factory's published skill
bundle list.

## What v0.98.0 actually shipped

Per `gh release view v0.98.0` on `2026-09-24` and the
`release-inventory-0.98.0.json` artifact shipped with the release:

| Asset | Count | Verification |
|---|---|---|
| Per-component tarballs (cogh, cognicode, cognicode-mcp) | 6 (3 components × 2 platforms) | SHA-256 in `SHA256SUMS` |
| Per-platform bundle YAMLs | 2 (x86_64 + aarch64) | `bundle-0.98.0-*-unknown-linux-gnu.yaml` |
| Source tarballs per component | 3 | `cogh-0.98.0.tar.gz`, `cognicode-0.98.0.tar.gz`, `cognicode-mcp-0.98.0.tar.gz` |
| Integrity file | 1 | `SHA256SUMS` |
| Release inventory | 1 | `release-inventory-0.98.0.json` |

Total: 13 assets. ALL targeting Linux GNU only. No musl/darwin/windows.

## Run-history evidence

| Run | Date | Outcome | Coverage |
|---|---|---|---|
| `#36033099039` | 2026-09-24 17:16 | FAIL (step 12 install-smoke, draft-first safety net blocked publication) | n/a (failed before pub) |
| `#36034410448` | 2026-09-24 17:36 | SUCCESS (Linux x86_64+aarch64, 13 assets published, isDraft=false) | Tier-1 ✅ |
| `#36038178581` (validate) | 2026-09-24 18:08 | SUCCESS | Tier-1 ✅ |
| `#36039255746` (validate negative) | 2026-09-24 18:15 | FAIL (gate caught forced version=0.99.0 mismatch) | Tag-coherence gate verified |

## How this doc advances H12

`AUDIT-2026-09-22-FINDINGS.md` has H12 in OPEN state with the
description "Distribución validada tiene alcance menor que el producto
descrito. 5/5 cubre Linux x86_64 + aarch64; PRF-DIST-05 y PRF-DIST-07
parciales para otras plataformas y explorer-*".

After this doc, H12 state advances to **WIP** with concrete progress:

  - The "Tier-1 vs Tier-2 vs Tier-3" structure makes the "alcance
    menor" claim falsifiable per platform.
  - The contract criteria (1-6 above) give a clear promotion path
    for any new platform: satisfy all 6 and the matrix entry moves
    from "Not supported" to "Supported".
  - The non-published skill bundle boundary (`cognicode-developer`,
    `cognicode-recommended`) is named explicitly.
  - The audit language ("PRF-DIST-05 y PRF-DIST-07 parciales")
    maps onto: PRF-DIST-05 ↔ "Tier-2/3 platforms" (not Linux GNU
    Tier-1); PRF-DIST-07 ↔ "skill bundles beyond the two currently
    published" (`cognicode-developer` + `cognicode-recommended`).


What remains to CLOSE H12:

  1. Decide whether PRF-DIST-05 is "Linux Tier-1" (already exercised)
     or "any platform declared in code". If the former, H12 closes
     for that sub-point with this doc as evidence.
  2. Same for PRF-DIST-07 with `cognicode-developer` + `cognicode-recommended` skill bundles.
  3. Each decision requires operator review (per `AUDIT-2026-09-22-
     FINDINGS.md` policy: CLOSED state requires operator-gated
     decision + JOURNAL evidence).

## Refs

- `docs/prf/evidence/u112-t5-release-snapshot/SNAPSHOT.md` (T5
  pre-release evidence, 0.97.4).
- `docs/prf/evidence/u24-dist-rollback/OBSERVATIONS.md` (F6.W1
  distribution rollback recovery).
- `docs/prf/AUDIT-2026-09-22-FINDINGS.md` (H12 entry).
- `docs/prf/JOURNAL.md` (§105, §112, §135).
- `.github/workflows/release.yml` (matrix entries cited above).
- Release run `#36034410448` (Tier-1 ✅ for v0.98.0).
