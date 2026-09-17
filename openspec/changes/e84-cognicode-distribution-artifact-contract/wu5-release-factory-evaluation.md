# e84 WU5 — Release-factory evaluation: can `dist` replace the handcrafted release workflow?

> Scope: read-only evaluation. Question: can `dist` (formerly `cargo-dist`,
> axodotdev) replace or generate the mechanical parts of the current
> handcrafted release workflow (`.github/workflows/release.yml`)?
>
> Prior, pre-existing decision: `docs/adr/ADR-052-reject-cargo-dist-e74.md`
> (ACCEPTED 2026-09-16) rejected adoption during e74, with a three-condition
> re-evaluation trigger. This document re-tests that decision against
> `dist`'s **current** documented capabilities. It does not assume dist is
> unadoptable, and it does not assume the prior spike's capability table was
> correct. Where the prior spike was wrong, that is stated explicitly.

---

## 1. Observed current state (from the tree, not from memory)

### 1.1 The producer workflow

`.github/workflows/release.yml` (253 lines) is a hand-written, three-job,
tag-triggered (`v*`) pipeline:

| Job | What it does |
|-----|--------------|
| `build` | 5-lane native matrix (`<<: *MATRIX`). Runs `cargo build --release --target <triple>`, then `tar -czf` **two** tarballs per lane, then `actions/upload-artifact@v4`. |
| `install-smoke` | Re-downloads the per-lane artifacts, extracts them into a clean `HOME`/`XDG_*`/`USERPROFILE`, and runs `cogh --version`, `cognicode-mcp --version`, `explorer-api --version`, and `cogh --help`. |
| `release` | Downloads all artifacts (`merge-multiple: true`), asserts one tarball per platform, generates notes via `scripts/generate-release-notes.sh`, publishes with `softprops/action-gh-release@v2`. |

The native matrix, verbatim:

| `target` (kebab, BundleManifest::Platform) | runner | `rust_target` |
|---|---|---|
| `linux-x86-64` | `ubuntu-latest` | `x86_64-unknown-linux-gnu` |
| `linux-aarch64` | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` |
| `mac-os-x86-64` | `macos-13` | `x86_64-apple-darwin` |
| `mac-os-aarch64` | `macos-latest` | `aarch64-apple-darwin` |
| `windows-x86-64` | `windows-latest` | `x86_64-pc-windows-msvc` |

The two tarballs emitted per lane:

- `cognicode-${VERSION}-${target}.tar.gz` — contains `explorer-api` (+`.exe`)
  and `cognicode-mcp` (+`.exe`).
- `cognicode-cli-${VERSION}-${target}.tar.gz` — contains `cogh` (+`.exe`).

Note the name fragment is the **CogniCode kebab platform** (`linux-x86-64`),
not the Rust triple.

Build-feature nuance that matters for any generator:

- `cogh` is built **separately**, with `--features cognicode-core/evidence-kernel`
  (comment: "cogh must always build with the evidence-kernel feature on").
- `explorer-api` is selected with `--bin explorer-api` while
  `cognicode-runtime`'s package also produces `explorer-mcp`.

`crates/cognicode-cli/src/cmd/bundle_manifest.rs` exposes
`BundleManifest::Platform` whose `Display` yields exactly the kebab names above.
`scripts/check-release-matrix.sh` **parses `release.yml` textually** to keep the
YAML matrix and the Rust enum in sync (plus a vetted-runner allowlist). This is
a hard coupling between the workflow file's shape and a checked-in invariant.

### 1.2 The workspace and its binaries

`Cargo.toml` (workspace `version = "0.95.0"`, edition 2024) has 13 members.
Binary-producing packages, verified from manifests:

| Package | `[[bin]]` targets |
|---|---|
| `cognicode-cli` | `cognicode`, `cogh` |
| `cognicode-runtime` | `explorer-api`, `explorer-mcp` |
| `cognicode-mcp` | `cognicode-mcp`, `cognicode-mcp-server`, `mcp-client` (plus lib `cognicode_mcp`) |

The remaining members are libraries / non-binary packages. There is **no
`[workspace.metadata.dist]`, no `dist-workspace.toml`, and no `[dist]` section
anywhere** — grep for `dist-workspace`, `cargo-dist`, `axodotdev`, `metadata.dist`
returns only documentation references, never configuration. `dist` is not wired
in.

### 1.3 Bundle / profile semantics (the ownership constraint)

`crates/cognicode-cli/src/cmd/bundle_manifest.rs` defines:

- `BundleManifest { apiVersion, kind, version, platform, released_at, profiles, components }`
- `ProfileDef { name, description, include_kinds }` — the **`core` / `reviewer` /
  `full`** profiles.
- `BundleComponent { name, kind, version, artifact, sha256, url, profiles }` —
  **per-component lowercase-hex SHA256 is mandatory**.
- `ComponentKind` includes `Cogh`, `Cognicode`, `Daemon`, `Skill`, `Sandbox`,
  `ExplorerAsset`, `Plugin`.

Example checked-in manifest `bundles/v0.95.0/bundle.yaml`:

```yaml
apiVersion: cognicode.bundle/v1
kind: Bundle
version: "0.95.0"
platform: linux-x86-64
profiles:
  - name: core
    include_kinds: [Cogh, Cognicode]
  - name: reviewer
    include_kinds: [Cogh, Cognicode, Daemon]
  - name: full
    include_kinds: [Cogh, Cognicode, Daemon, Skill, Sandbox]
components:
  - name: cognicode-mcp
    artifact: cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    ...
  - name: skills-cognicode-core
    artifact: skills-cognicode-core-0.95.0.tar.gz
  - name: sandbox-templates
    artifact: sandbox-templates-0.95.0.tar.gz
```

Two observations that are load-bearing for this evaluation:

1. **The bundle already uses per-component, Rust-triple artifact names**
   (`cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz`), i.e. a
   different naming convention from `release.yml`'s combined
   `cognicode-0.95.0-linux-x86-64.tar.gz`. The repository already carries two
   artifact-naming conventions, and they do not currently agree.
2. The `sha256` values in the checked-in bundle are **placeholder**
   (`...0001`, `...0002`, `...0003`). Bundle assembly is therefore not yet a
   working, end-to-end publisher — it is a contract ahead of its producer.

`openspec/.../portable-runtime-distribution/spec.md` defines the governing
requirements:

- **PRT-005** — every supported platform must have a build/package/verification
  lane, or an explicit typed waiver.
- **PRT-006** — "An external packaging tool MAY build/sign/package/bootstrap
  `cogh`, but version profiles, plugin/skill composition, project locks and
  CogniCode bundle semantics **remain owned by `cogh` contracts**." Its scenario
  is literally "the project replaces GitHub workflow packaging with `dist` or
  another tool". **PRT-006 explicitly anticipates dist** and makes bundle
  authority the boundary condition, not a prohibition.

### 1.4 Other release automation

- `scripts/generate-release-notes.sh` (17 lines): `git log` filtered by
  `feat`/`fix` between two tags. (A second, older `scripts/release-notes.sh`
  greps `CHANGELOG.md`.)
- `justfile`: `bundle-skills` (tars each `skills/*/`), `bundle-musl`
  (x86_64-unknown-linux-musl tarball), `release-draft` / `release-publish`
  (`gh release create/edit` + `gh release upload`), plus a
  `sandbox/scripts/release_scorecard.py` invocation. These are local/publish
  helpers; the tarball tars are plain `tar -czf`.
- `dist/` at repo root holds stale 0.94.x artifacts from a local run; it is not
  configuration.

---

## 2. Verified `dist` capabilities

All statements below are from the official dist book / repo, fetched during this
evaluation. Source URLs are given inline. Anything I could not confirm is
deferred to §6.

### 2.1 Configuration surface

Configuration is read from, in increasing preference, the language manifest
(`Cargo.toml`), a workspace `dist-workspace.toml` / `dist.toml`, and a package
`dist.toml`. For Rust users the `[dist]` section may also live under
`[workspace.metadata.dist]` / `[package.metadata.dist]` in `Cargo.toml`.
Source: <https://axodotdev.github.io/cargo-dist/book/reference/config.html>

Documented settings relevant here:

- Selection: `packages`, per-package `dist = true|false`.
- Targets: `targets = [...]` with supported choices including
  `x86_64-unknown-linux-gnu`, **`aarch64-unknown-linux-gnu`**,
  `x86_64-unknown-linux-musl`, **`aarch64-unknown-linux-musl`**,
  `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.
- Artifacts: `checksum` (default **`"sha256"`**, one `.sha256` per archive;
  also sha512/sha3/blake2s/blake2b/false), **`extra-artifacts`**, `source-tarball`,
  `recursive-tarball`.
- Archive shaping: `include` (files/dirs copied to archive root), `auto-includes`,
  `unix-archive` (default `.tar.xz`), `windows-archive` (default `.zip`),
  `package-libraries`, `bin-aliases`, per-platform `binaries`.
- Build: `features`, `default-features`, `all-features`, `precise-builds`,
  `min-glibc-version`, `cargo-auditable`, `cargo-cyclonedx`, `omnibor`,
  `msvc-crt-static`, `dependencies` (apt/chocolatey/homebrew).
- Installers: `installers = [shell, powershell, npm, homebrew, msi]`.
- Hosting: `github-attestations`, `github-attestations-phase`,
  `github-attestations-filters`, `create-release`, `github-releases-repo`.
- CI: `ci`, `github-custom-runners`, `github-build-setup`, `pr-run-mode`,
  `allow-dirty`, `tag-namespace`, and custom job hooks.

### 2.2 Multi-package / multi-binary workspaces

dist is workspace-centric. `packages` selects which packages are distributed;
`dist = true/false` at package level allow/deny-lists. Each distributed *package*
gets an archive per target containing that package's binaries (and optionally
libraries via `package-libraries`). `binaries` overrides the installed binary
list per platform. Source: config reference §`packages`, §`dist`, §`binaries`.

**Consequence (verified, and central):** dist's unit of packaging is the
**package**, not an arbitrary grouping. It cannot be told to place two different
packages' binaries into one tarball, and the documented config surface contains
**no setting to override archive filenames**. So CogniCode's current combined
`cognicode-<ver>-<target>.tar.gz` (explorer-api + cognicode-mcp, one file) is not
expressible as a single dist archive; dist would instead emit one archive per
package.

### 2.3 Custom extra assets — **yes**

`extra-artifacts` (since 0.6.0, package-local) takes repeated entries:

```toml
[[dist.extra-artifacts]]
artifacts = ["schema.json"]
build = ["cargo", "run", "--", "generate-schema"]

[[dist.extra-artifacts]]
artifacts = ["target/coolsignature.txt", "target/importantfile.xml"]
build = ["make"]
```

`build` is a command to run; `artifacts` is the list of relative paths dist
expects to exist afterwards, and **each file is uploaded individually to the
release as its own artifact**. dist uses this itself to ship its
`dist-manifest-schema.json`. Source: config reference §`extra-artifacts`;
<https://axodotdev.github.io/cargo-dist/book/artifacts/index.html>

`include` additionally copies files/dirs into every archive's root.
Source: config reference §`include`.

**Consequence:** a `bundle.yaml`, a `skills-*.tar.gz`, and a
`sandbox-templates-*.tar.gz` **can** be produced and uploaded as extra assets,
with their generation logic living in a CogniCode script invoked by `build`.
So dist does **not** need to understand profiles; it only needs to host files.
This directly satisfies PRT-006's boundary.

### 2.4 Generated outputs

| Output | Supported? | Source |
|---|---|---|
| Archives (tarball/zip) per app/target | ✅ | `artifacts/archives.html` |
| `.sha256` per archive (checksum, default on) | ✅ | config §`checksum` |
| `dist-manifest.json` (machine-readable plan/manifest) | ✅ | `ci/index.html` |
| shell installer (`curl \| sh`) | ✅ | config §`installers` |
| powershell installer (`irm \| iex`) | ✅ | config §`installers` |
| npm package | ✅ | config §`installers` |
| Homebrew formula | ✅ | config §`installers` |
| Windows MSI | ✅ | config §`installers` |
| GitHub Artifact Attestations (`actions/attest`, Sigstore/Rekor) | ✅ (opt-in; public repo or GH Enterprise) | `supplychain-security/attestations/github.html` |
| CycloneDX SBOM / `cargo auditable` / OmniBOR | ✅ opt-in | `supplychain-security/index.html` |
| `.deb` / `.rpm` / AppImage / pacman | ❌ not in the documented installer set | config §`installers` |
| Windows code signing (ssl.com or Azure Artifact Signing) | ✅ x86_64 only | `supplychain-security/signing/windows.html` |
| macOS code signing / notarization | ❌ **still open** (issue #1121) | `supplychain-security/index.html` |
| Sigstore signing / Linux code signing | ❌ **still open** (issue #120) | `supplychain-security/index.html` |

**Correction to the prior spike.** `wu5-packaging-spike.md` and ADR-052 asserted
cargo-dist provides "✅ first-class" Linux `.deb`/`.rpm`/AppImage, "✅" macOS
`.pkg`, "✅" Apple notarization, and "✅" GPG signing. Per the current docs:

- there is **no `.deb`/`.rpm`/AppImage installer** in the documented installer
  set (only shell/powershell/npm/homebrew/msi);
- macOS code signing/notarization is **not implemented** (open issue #1121);
- Sigstore/Linux signing is **not implemented** (open issue #120);
- GPG signing of tarballs is **not offered** — the checksum feature is explicitly
  unsigned (`checksum` config), and "more robust signed checksums" is a *future
  work* item (issue #120).

So dist today is **not** a signing factory for CogniCode's platforms. The
signing rationale that drove ADR-052's "re-evaluate when secrets exist" framing
rests on capability dist does not currently have on macOS/Linux.

### 2.5 Platform support on GitHub runners

- Linux x86_64 GNU, aarch64 GNU, x86_64 musl, aarch64 musl: all listed targets.
- `github-custom-runners` lets you pin the runner per target, including native
  ARM runners, e.g. `aarch64-unknown-linux-gnu = "buildjet-8vcpu-ubuntu-2204-arm"`.
- Without a custom runner, dist **defaults to cross-compiling** aarch64-linux-gnu
  from `ubuntu-22.04` via `cargo-zigbuild`. dist's defaults do **not** match
  PRT-005's "native runner, no cross-compilation" rule; a native ARM runner
  (e.g. `ubuntu-24.04-arm`) would have to be configured explicitly.
- macOS x86_64/arm64 and Windows x86_64 targets supported; default runners
  listed in `ci/customizing.html` (`macos-15-intel`, `macos-14`, `windows-2022`).
- Cross-compilation helpers `cargo-zigbuild` / `cargo-xwin` exist.
Sources: config §`targets`, `ci/customizing.html`, `artifacts/archives.html`.

### 2.6 Local dry-run / plan / build without publishing — **yes**

- `dist plan` = `dist manifest --artifacts=all --no-local-paths`: "the exact
  command that CI will run to make its build plan", machine-readable with
  `--output-format=json`.
- `dist build --artifacts host|local|global|all|lies`. **`lies`** fakes all
  artifacts "useful for testing/mocking/staging".
- `dist generate --check` verifies generated CI/MSI templates are up to date.
- `dist build --print linkage` reports dynamic-library linkage.
Source: <https://axodotdev.github.io/cargo-dist/book/reference/cli.html>

### 2.7 CI generation and custom jobs

`dist init` generates `release.yml` on GitHub, implementing plan →
build-local-artifacts → build-global-artifacts → host → publish → announce.
Generated CI is treated as owned by dist: hand-edits are an **error** unless
`allow-dirty = ["ci"]`, which forfeits automatic regeneration and upgrade UX.
Custom jobs can be injected at `plan-jobs`, `build-local-artifacts-jobs`,
`build-global-artifacts-jobs`, `host-jobs`, `publish-jobs`, `post-announce-jobs`
as reusable workflows receiving the JSON plan. `github-build-setup` injects
steps into `build-local-artifacts`.
Sources: `ci/index.html`, `ci/customizing.html`.

**Consequence:** there is **no native install-smoke / post-extract runtime
verification**. The existing `install-smoke` job (clean `HOME`, run each binary
`--version`) has no dist equivalent and would have to be preserved as a custom
reusable workflow. Likewise the bundle aggregation stage.

### 2.8 Archive layout and naming

- Archive root: "for tarballs, a directory with the same name as the archive,
  without the extension"; dist suggests unpacking with `--strip-components=1`.
  `release.yml` currently extracts flat binaries. Source: `artifacts/archives.html`.
- Archive filenames are chosen by dist; **no override setting is present in the
  documented config surface**. Exact template not stated in the pages read.
  Source: config reference (absence of a naming setting).

---

## 3. Comparison against the WU5 criteria

Legend: ✅ meets / ⚠️ partial or conditional / ❌ does not meet / ？ not verified.

| Criterion | Current (`release.yml` + `cogh`) | `dist` | Verdict / evidence |
|---|---|---|---|
| **Less code** | 253 hand-written YAML lines + 4 scripts; fully owned, directly editable | Replaces hand-written logic with `dist-workspace.toml`, but commits a **large generated `release.yml`** that must not be hand-edited (`allow-dirty` forfeits upgrades) | ⚠️ Less *hand-written logic*, **more committed YAML**. Net line reduction not demonstrated. (`ci/customizing.html`) |
| **Deterministic artifacts** | `tar -czf` with no `--sort=name`/mtime pinning → not byte-reproducible; per-package SHA256 in `bundle.yaml` currently placeholders | Consistent naming, `dist-manifest.json`, `.sha256` per archive; **byte-reproducibility not documented** | ⚠️ dist better on naming/manifest/checksums; archive byte-reproducibility ？ unverified |
| **Linux x86_64 + aarch64** | Both, **native** (`ubuntu-latest`, `ubuntu-24.04-arm`) | Both supported; default is **cross-compile** aarch64 via zigbuild. Native requires explicit `github-custom-runners` | ⚠️ Capable, but must override defaults to preserve PRT-005's native rule (`ci/customizing.html`) |
| **Future macOS/Windows** | 3 lanes exist (macos-13/latest, windows-latest); no signing/notarization | macos/windows targets + MSI + Windows signing supported; **macOS signing/notarization and Sigstore still open issues**; no `.pkg` | ⚠️ Parity or slight gain (MSI, Windows signing); **no** macOS signing either (`supplychain-security/index.html`) |
| **Checksums** | Not generated by the workflow; `bundle.yaml` per-component SHA256 is stubbed | `.sha256` per archive, default on | ✅ dist clearly better (config §`checksum`) |
| **Attestations** | None | GitHub Artifact Attestations, opt-in (`actions/attest`, Sigstore/Rekor); public repo or GH Enterprise | ✅ dist clearly better (`supplychain-security/attestations/github.html`) |
| **Multi-binary workspace** | Handled, but by **selecting binaries manually**; ships 3 of 7 package binaries; combined tarball spans two packages | Packages per package; cannot merge two packages into one archive; **cannot rename archives**; would ship *all* package binaries (superset), requiring `binaries` overrides to restrict | ⚠️ Capable, but changes artifact contents and cannot reproduce the current combined tarball (config §`packages`/§`binaries`) |
| **Custom bundle assets** | Produced by bespoke `justfile`/scripts | `extra-artifacts` `build`+`artifacts` uploads arbitrary files individually; `include` adds files to archives | ✅ Verified — **the deciding constraint (PRT-006 authority) is satisfiable** (config §`extra-artifacts`) |
| **Local dry-run** | None (no plan/preview; `gh release create --draft` only) | `dist plan`, `dist build --artifacts lies`, `dist generate --check`, `dist manifest` | ✅ dist clearly better (`reference/cli.html`) |
| **Migration / rollback cost** | — | Replaces `release.yml`; must re-home `install-smoke` (no native equivalent) and the bundle stage; **breaks `scripts/check-release-matrix.sh`** which parses `release.yml` textually; adds a pinned tool; introduces dist-generated YAML discipline | ❌ High. Rollback is mechanically easy (delete config, restore `release.yml` from git), but the matrix-coherence tool and install-smoke must be re-implemented first |

---

## 4. Outcome

**`KEEP_CUSTOM_RELEASE`.**

Default to keeping the handcrafted release workflow. Do **not** adopt `dist`
during e84. Re-evaluate only when the trigger conditions in §5 are re-checked
and, in particular, when a *verified* signing/attestation need exists that the
current machinery cannot meet.

### 4.1 Justification

1. **The decisive contract (PRT-006 / bundle authority) is *satisfiable*, but
   only by keeping a bespoke stage.** `extra-artifacts` proves dist can host a
   `bundle.yaml`, a skills tarball and a sandbox-templates tarball without
   touching profile semantics. That removes the strongest *a-priori* objection
   to dist. But because the per-component SHA256 values and the
   `core`/`reviewer`/`full` profile→component mapping are CogniCode-owned, the
   assembly stage **must remain a CogniCode script**, and because
   `extra-artifacts` is package-local and runs at package-build time, it cannot
   aggregate artifacts across all five lanes into one platform bundle. The
   cross-lane bundle assembly therefore lands in a custom `host-jobs` /
   `post-announce-jobs` reusable workflow. Net effect: dist *re-homes* the
   handcrafted stage rather than deleting it.

2. **The strongest net-new value dist offers today is checksums + attestations +
   local dry-run** — real, verified gains. But they do not require replacing the
   producer: `.sha256` files can be emitted by the existing `Package tarball`
   step, GitHub Artifact Attestations can be added as `actions/attest` in the
   existing workflow, and a plan/preview can be scripted. Adopting a
   workflow-owning generator to obtain three additive features is a poor
   cost/benefit trade while it also forces re-implementation of `install-smoke`,
   the bundle stage, and `check-release-matrix.sh`.

3. **The signing rationale that motivated re-evaluation is substantially
   weaker than ADR-052 recorded.** dist does **not** currently provide macOS
   code signing/notarization (#1121), Sigstore/Linux signing, or GPG-signed
   checksums (#120), and it ships no `.deb`/`.rpm`/AppImage installer. ADR-052's
   capability table marked those "✅ first-class". They are not. Since the
   remaining ADR-052 re-evaluation trigger is also unmet (no signing secrets
   exist in this repository, and no independent trigger fired), the governance
   gate that would authorise adoption has not opened.

4. **`install-smoke` has no dist equivalent.** PRT-005 requires per-lane
   native package + verification. dist builds and packages but does not run the
   produced binaries on a clean `HOME`. That job survives as custom code under
   any dist outcome, so the "less code" premise is not met.

5. **Migration breaks a checked-in invariant.** `scripts/check-release-matrix.sh`
   textually parses `release.yml` to enforce matrix ↔ `BundleManifest::Platform`
   coherence plus a runner allowlist. dist owns and regenerates that file; the
   tool would have to be rewritten against `dist-workspace.toml`. This is real,
   bounded work, but it is cost on top of the adoption, not savings.

Taken together: the evidence neither refutes that dist *could* be adopted, nor
supports adopting it now. Per the WU5 hard rule — default to `KEEP_CUSTOM_RELEASE`
when the evidence is weak or a needed capability is unverified — and given that
several needed capabilities (build-feature parity, artifact-name/contents parity,
byte-reproducibility) are **unverified**, `KEEP_CUSTOM_RELEASE` is the
defensible outcome. The prior hypothesis
(`USE_DIST_WITH_CUSTOM_BUNDLE_STAGE`) is a *mechanically valid* architecture but
the evidence does not support executing it in e84: it does not reduce owned
surface and the governance gate is closed.

---

## 5. Rejected alternatives

### 5.1 `USE_DIST` (dist owns the whole release)

**Rejected.** This would require dist to own the bundle/profile layer, which it
cannot: it has no concept of named install profiles (`core`/`reviewer`/`full`),
no `ComponentKind`, and no per-component SHA256 contract. PRT-006 makes that
ownership non-negotiable. Additionally, a plain `USE_DIST` would:
ship *all* binaries of each package (a superset of the current 3-of-7) with no
way to reproduce the combined `cognicode-<ver>-<target>.tar.gz`, drop the
`install-smoke` verification lane (no dist equivalent), and break
`check-release-matrix.sh`. It would import a dependency we cannot use to its
advertised signing capability.

### 5.2 `USE_DIST_WITH_CUSTOM_BUNDLE_STAGE` (the prior hypothesis)

**Rejected for e84 — but not refuted as a future architecture.** The hypothesis
correctly identifies that dist can host the bundle assets via `extra-artifacts`
and that bundle authority stays in CogniCode (verified, §2.3, §3). Its weakness
is that the "custom bundle stage" is not a dist feature but a bolted-on
workflow: `extra-artifacts` is package-local and build-time, so a cross-lane,
post-build `bundle.yaml` (which needs the SHA256 of artifacts produced on other
runners) must be assembled in a custom `host-jobs`/`post-announce-jobs` job that
reads dist's `.sha256` sidecars. Add the re-homing of `install-smoke`, the
`check-release-matrix.sh` rewrite, the loss of archive-name control, and the
unchanged absence of macOS/Sigstore signing, and the net result is *more* owned
moving parts for parity-plus-checksums-plus-attestations, contrary to the
hypothesis's implied savings. It stays the most plausible adoption shape **if**
the governance gate opens (§4.1.3) and the unverified items in §6 are resolved.

### 5.3 `KEEP_CUSTOM_RELEASE`

**Chosen.** Justified in §4.1.

---

## 6. Unverified / uncertain

These are explicitly **not** verified and must not be presented as fact. They are
the open questions a future adoption spike (or a future ADR) would have to close.

1. **Build-feature parity.** Whether dist can build `cogh` with exactly
   `--features cognicode-core/evidence-kernel` while other packages build
   without it. dist's `features` setting passes feature strings to
   `cargo build` and forces `precise-builds` when per-package features differ,
   but I did **not** verify that a dependency-scoped feature spec
   (`crate/feature`) is accepted, nor that the resulting binary is identical to
   the current `cargo build --release -p cognicode-cli --bin cogh --features
   cognicode-core/evidence-kernel`. Unverified.
2. **Archive naming/spec and contents parity.** The exact dist archive filename
   template is not stated in the pages read; what *is* verified is the absence
   of any filename-override setting. Whether dist can reproduce the current
   names (`cognicode-<ver>-linux-x86-64.tar.gz`,
   `cognicode-cli-<ver>-linux-x86-64.tar.gz`) and their flat binary layout is
   therefore unverified and appears **not** achievable without dist-native
   changes. Also unverified: whether controlling the shipped binary set via
   `binaries`/`package-libraries` yields exactly `{explorer-api,
   cognicode-mcp, cogh}` rather than the full package binary sets.
3. **Byte-reproducible archives.** Whether dist produces deterministic tarballs
   (sorted entries, zeroed mtimes) is not documented in the pages read.
4. **Post-build cross-lane bundle aggregation mechanics.** That a custom
   `host-jobs`/`post-announce-jobs` reusable workflow can read dist's
   `.sha256` sidecars, emit `bundle.yaml`, and upload it as a release asset
   without fighting dist's own hosting/announce ordering. The hook mechanism is
   verified; the concrete 5-lane aggregation + upload sequence is not.
5. **Native ARM runner string under dist.** The mechanism
   (`github-custom-runners`) is verified; that dist cleanly accepts
   `ubuntu-24.04-arm` as a native (non-cross, correct `host`) runner for
   `aarch64-unknown-linux-gnu` is not verified, and dist's default would
   cross-compile (§2.5), which PRT-005 forbids.
6. **MSI/installer build viability.** Whether `msi`/`homebrew`/`npm` installers
   build in CogniCode's CI is unverified. Note also that enabling shell/powershell
   installers would *introduce* an external install script that does not route
   through `cogh install`, i.e. it would widen the install-authority divergence
   ADR-052's trigger #2 wants closed — an argument for `installers = []` if
   dist were ever adopted.
7. **dist version pin.** The exact current dist release and whether any
   documented behaviour has changed since the pages were fetched were not
   pinned; configuration is versioned via `cargo-dist-version`, which is
   mandatory.
8. **Governance state.** ADR-052 is `ACCEPTED` and its three-condition trigger is
   unmet (no signing secrets; install divergence open; no successor ADR). This is
   an observed repository state, not a capability judgement; any adoption must
   first retire ADR-052 with a dated, evidence-bound successor ADR.

---

## 7. Sources

- dist config reference:
  <https://axodotdev.github.io/cargo-dist/book/reference/config.html>
- dist CLI manual:
  <https://axodotdev.github.io/cargo-dist/book/reference/cli.html>
- Archives: <https://axodotdev.github.io/cargo-dist/book/artifacts/archives.html>
- CI overview: <https://axodotdev.github.io/cargo-dist/book/ci/index.html>
- CI customization (custom jobs, custom runners, build-setup, allow-dirty):
  <https://axodotdev.github.io/cargo-dist/book/ci/customizing.html>
- Supply-chain security (signing status): 
  <https://axodotdev.github.io/cargo-dist/book/supplychain-security/index.html>
- GitHub attestations:
  <https://axodotdev.github.io/cargo-dist/book/supplychain-security/attestations/github.html>
- Custom builds (`build-command`):
  <https://axodotdev.github.io/cargo-dist/book/custom-builds.html>
- Workspaces overview:
  <https://axodotdev.github.io/cargo-dist/book/workspaces/index.html>
- Repo / README (fork notice; upstream `axodotdev/cargo-dist` active):
  <https://github.com/astral-sh/cargo-dist>

Repository sources: `.github/workflows/release.yml`, `Cargo.toml`,
`crates/cognicode-cli/src/cmd/bundle_manifest.rs`, `bundles/v0.95.0/bundle.yaml`,
`scripts/check-release-matrix.sh`, `scripts/generate-release-notes.sh`,
`justfile`, `openspec/changes/cognicode-living-software-intelligence/specs/portable-runtime-distribution/spec.md`,
`openspec/changes/e74-lsi-portable-runtime-distribution/wu5-packaging-spike.md`,
`docs/adr/ADR-052-reject-cargo-dist-e74.md`.
