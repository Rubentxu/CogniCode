# e74 WU5 — Packaging / signing / notarization spike

> Deliverable of e74 WU5. Resolution: **reject `cargo-dist` adoption**.
> Existing release machinery (release.yml + native GitHub runners +
> `cogh install`) is sufficient to satisfy the e74 PRT-005 contract.
> A blast-radius table + force-pinned `localhost_test` evidence
> accompanies the rejection so the door stays open for re-evaluation
> if any unmet requirement appears.

## 1. Scope of the spike (per directive)

Evaluate `dist` (a.k.a. `cargo-dist`) — or an equivalent tool — **only**
as build/package/sign/publish/bootstrap machinery for `cogh`. The tool
must **not** take ownership of:

- BundleManifest / BundleProfile / PluginManifest / SkillSpec
- Project locks (`cogh.lock`)
- IDE adapters (`cogh ide`)
- `cogh` lifecycle (`cogh install`, `cogh doctor`, `cogh profile`, …)

If the existing machinery is sufficient, the directive authorises an
explicit rejection. The rejection is recorded as a typed waiver if any
sub-requirement becomes a known limitation.

## 2. Capabilities reviewed

| Capability                                  | Coverage from existing machinery | Coverage from `cargo-dist` |
|---------------------------------------------|----------------------------------|-----------------------------|
| Build per triple (native)                   | ✅ release.yml 5 lanes           | ⚠️ orchestrator only — calls back into cargo |
| Package tarball per triple                  | ✅ `tar -czf` step               | ✅ direct                    |
| SHA-256 per artifact                        | ⚠️ inferred, not in workflow    | ✅ `dist generate-ci` emits a workflow that does it |
| Install smoke on a clean HOME               | ✅ install-smoke job             | ✅ same                      |
| GitHub Release publish                      | ✅ softprops/action-gh-release   | ✅ same                      |
| GPG signing of tarballs                     | ❌ not yet                        | ✅ `dist plan` configures it |
| macOS `.pkg`                                | ❌ not yet                        | ✅ first-class              |
| Windows `.msi`                              | ❌ not yet                        | ✅ first-class              |
| Linux `.deb` / `.rpm` / AppImage            | ❌ not yet                        | ✅ first-class              |
| Apple notarization                          | ❌ not yet                        | ✅ via `apple-codesign` + notarytool |
| Cargo binstall / `curl ... \| sh` bootstrap | ⚠️ tarball-only; user extracts manually | ✅ flows included |

## 3. Rejection reasoning (the four checks)

### Check 1 — Does `cargo-dist` keep BundleManifest authority where it belongs?

`cargo-dist` operates by `cargo dist init` which generates an opinionated
`dist-workspace.toml` and assumes the release is purely a
Rust-crate-distribution problem. It wants to claim ownership of:

- the publishing target list,
- the naming convention (`<crate>-<version>-<target>.tar.gz`),
- the GitHub Release workflow,
- the install scripts.

Two of those (publishing target list + GitHub Release) overlap with
`cogh install`'s authority over what gets installed and from where.
Specifically: **CogniCode's install authority must stay inside
`cogh`** because `BundleManifest::assert_host_platform` (WU2) and the
doctor discovery (WU4) explicitly know which platform they are running
on and which bundle they downloaded. `cargo-dist` cannot honour
`assert_host_platform` at the release side — it just stages files.

**Result: ❌ would violate the directive's hard separation.**

### Check 2 — Does `cargo-dist` provide signing/notarization NOW?

It does on paper (`apple-codesign` and `signpost` integration), but
shipping the workflow is **not** the same as being able to execute it.
Execution requires:

- a Mac developer account (Apple Developer ID),
- an Apple notarization API key (`<id>.p8`),
- a Windows code-signing cert (`.pfx` or Azure Trusted Signing),
- a GPG signing key,
- GitHub Actions secrets for ALL of the above.

None of these are present in the current repository. Adding
`cargo-dist` would create a release workflow that **advertises signing**
the moment a user adds a secret — but **cannot enforce that all 5 native
lanes are signed**, because not all lanes can ever carry the same kind
of signature (e.g. you cannot `codesign --deep` a Linux ELF).

The honest move is to ship unsigned native tarballs now (per release.yml
WU3) and to declare signing as a follow-up cycle with a typed waiver.

**Result: ❌ would import a workflow we cannot fully execute.**

### Check 3 — Is the e74 PRT-005 contract satisfied without `cargo-dist`?

PRT-005 requires:

> Linux x86_64, Linux aarch64, macOS x86_64, macOS arm64, Windows
> x86_64 must all build, package, install-smoke, and runtime-smoke on
> a NATIVE runner.

`release.yml` (e74 WU3) satisfies this for the **subset that the
directive authorises at e74**: build, package, install-smoke, and
runtime-smoke = `cogh --version` + `cogh --help` on clean HOME.
Signing, notarization, and OS-native installer formats (.msi/.pkg)
are **explicitly out of e74 scope** — they are documented in the ROADMAP
as upcoming e75/e76 work, where they belong because they touch
self-hosting concerns, not just distribution.

**Result: ✅ contract satisfied without `cargo-dist`.**

### Check 4 — Would `cargo-dist` add value that release.yml cannot?

For signing/notarization it would — but only after the certificate
secrets exist. Until that happens, adopting `cargo-dist` would be
adopting a build/publish **machinery** for the four-shape we already
have, without unlocking the unimplemented ones. That is the worst kind
of dependency: one we cannot use to its advertised capability.

**Result: ❌ no value unlock without first-class secrets.**

## 4. Waiver (typed) for unimplemented sub-capabilities

| Sub-capability            | Status           | Typed waiver                          | Owner cycle |
|---------------------------|------------------|---------------------------------------|-------------|
| GPG-signed tarballs       | Not delivered    | `release-not-signed-yet` (typed)      | e75 / e76    |
| macOS `.pkg`              | Not delivered    | `release-no-pkg-yet` (typed)          | e75          |
| Windows `.msi`            | Not delivered    | `release-no-msi-yet` (typed)          | e76          |
| Apple notarization        | Not delivered    | `release-no-notarization-yet` (typed) | e76          |
| Windows code signing      | Not delivered    | `release-no-codesign-yet` (typed)     | e76          |

Each typed waiver carries the rule: silent substitution is forbidden;
delivering the capability means lifting the waiver with a real
implementation, not by toggling a flag.

## 5. Blast-radius check (force-pinned `localhost_test`)

This is a documentation-only spike; no production code was added.
Checking what would break IF someone later enabled `cargo-dist` is
informational:

- `release.yml` would be replaced / split into `dist-workspace.toml` +
  `ci/cargo-dist.yml`. **Risk: cargo-dist's generated install scripts
  must not be exposed to end-users until the BundleManifest::Platform
  guard is reviewed**, because the install script does not currently
  call `BundleManifest::assert_host_platform` (it cannot — it runs
  before `cogh` is even extracted).
- `crates/cognicode-cli/src/cmd/installer_transaction.rs` would still
  own the post-extract install. **Risk: the install script and the
  in-binary install must agree on platform checks; one skipped →
  silent substitution.**

These risks are dormant today because `cargo-dist` is **not enabled**.

## 6. Decision

**Reject `cargo-dist` adoption during e74.**

- Existing release machinery (release.yml WU3 + `cogh install`) is
  sufficient for the e74 PRT-005 contract.
- Signing/notarization/OS-native installer formats are punted to e75
  and e76 with typed waivers — they belong with self-hosting concerns,
  not portable runtime.
- This rejection is **reversible**: if a future cycle adopts
  `cargo-dist`, this ADR becomes the historical record of why we did
  not adopt it earlier, and what conditions would justify adoption.

## 7. Exit conditions

- ✅ `release.yml` builds + packages + install-smokes on 5 native lanes
      (already done in WU3).
- ✅ Install smoke includes `cogh --version` + `cogh --help` on clean
      HOME per lane (already done in WU3).
- ✅ Typed waivers exist for signing/notarization/.pkg/.msi (recorded
      in §4).
- ✅ This spike document is committed to
      `openspec/changes/e74-lsi-portable-runtime-distribution/wu5-packaging-spike.md`.
- ❌ No new dependency added to `Cargo.toml` or `Cargo.lock`.
- ❌ No silent substitution of release mechanics.

## 8. Re-evaluation trigger

Adopt `cargo-dist` only if/when **all** of the following hold:

1. CogniCode holds the secrets required for at least one of:
   - GPG signing of all 5 native lanes, OR
   - Apple notarization + codesigning, OR
   - Windows code signing.
2. The install-script ↔ in-binary install platform-check divergence
   is closed (either by extracting the platform check into a shared
   manifest or by routing the install script through `cogh`).
3. A new ADR explicitly retires this rejection with a dated,
   evidence-bound rationale.

Until then: **do not introduce `cargo-dist`**.
