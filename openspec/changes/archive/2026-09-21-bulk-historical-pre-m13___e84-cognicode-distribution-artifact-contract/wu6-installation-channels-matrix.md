# e84 WU6 — Installation-Channels Matrix (LAYER 0: installing `cogh`)

> **Status:** decision input for **e87**. This document proposes nothing to implement in e84.
> **Scope:** LAYER 0 only, i.e. how a user obtains the bootstrap CLI `cogh` (a single Rust
> binary). LAYER 1 (`cogh` installing the CogniCode runtime into `~/.cognicode/`) is out of
> scope.
> **Method:** every non-obvious claim below was checked against the vendor's official
> documentation. Doc URLs are given per channel. Anything I could not confirm from a primary
> source is listed in the **UNVERIFIED** section and is *not* used to justify a recommendation.

---

## 0. Facts about our release surface (measured from the repo, not assumed)

These are the constraints every channel must satisfy. Taken from
`.github/workflows/release.yml` at the time of writing.

| Property | Value |
|---|---|
| Host | GitHub Releases, repo `github.com/Rubentxu/CogniCode` |
| Tag | `v{VER}` (release lane triggers on `refs/tags/v*`) |
| Assets per release | **two** tarballs per target: `cognicode-{VER}-{target}.tar.gz` and `cognicode-cli-{VER}-{target}.tar.gz` |
| Targets | `linux-x86-64`, `linux-aarch64`, `mac-os-x86-64`, `mac-os-aarch64`, `windows-x86-64` |
| Archive format | `.tar.gz` for all targets (including Windows) |
| Publisher | `softprops/action-gh-release@v2`, `draft: false`, `prerelease: false`, `GITHUB_TOKEN` |
| Published checksum files | **none** (no `.sha256` / `checksums.txt` asset is emitted) |
| Published attestations | **none** (no `actions/attest` / provenance / cosign step in the workflow) |

Two consequences that drive the whole matrix:

1. **Two archives per target in one release.** Any channel that auto-selects "the asset for my
   platform" must be told *which* of the two to take (the `cognicode-cli-*` one is `cogh`).
2. **Our target tokens are non-standard** (`linux-x86-64`, `mac-os-aarch64`). They are not Rust
   triples (`x86_64-unknown-linux-gnu`) and not the common `x86_64`/`arm64` spellings.
   Autodetection may or may not match them (see UNVERIFIED).

---

## 1. Evaluation matrix

Legend: **Y** = yes, **N** = no, **P** = partial / conditional, **?** = unverified.

| Channel | Direct GitHub Release support (how) | Version pinning | Linux | macOS | Windows | Checksums (algo / handled by) | Attestations / provenance | Custom plugin or registry entry we must author+maintain | Upgrade UX | Ongoing maintenance cost |
|---|---|---|---|---|---|---|---|---|---|---|
| **mise** (`github:` backend) | **Y** — native `github:owner/repo`, no plugin; OS/arch/libc/format autodetect; `asset_pattern` / `matching` / `version_prefix` / `bin` / `rename_exe` | **Y + lockfile** (`mise.toml` + `mise.lock`, records URL+checksum+provenance) | **Y** | **Y** | **Y** | SHA-256 (and Blake3 in lockfile); recorded/verified by **mise** | **Y** — GitHub Artifact Attestations (default on) + SLSA provenance (default on) | **No plugin.** Only a tiny tool config (`matching`/`asset_pattern` + `rename_exe`), which is declarative TOML, not code | `mise use`, `mise upgrade`, locked installs | **Very low** |
| **mise** (`aqua:` backend) | **Y** — consumes aqua package definitions from the aqua registry (compiled in; custom registries allowed); no aqua CLI needed | **Y + lockfile** | **Y** | **Y** | **Y** | Checksum from registry metadata / release API / lockfile; by **mise/aqua** | **Y** — Cosign, Minisign, SLSA, GitHub Artifact Attestations (metadata-gated) | **Registry entry** (YAML) for `Rubentxu/CogniCode`; no plugin. Also usable via `aqua.registries` or a PR to aqua-registry | `mise use aqua:…`, `mise upgrade` | **Low–medium** (keep registry entry current) |
| **asdf** | **N** native. Needs a plugin Git repo (`bin/list-all`, `bin/download`, `bin/install`). | **P** — `.tool-versions` only; no built-in lockfile | **Y** | **Y** | **N/?** (Unix-shell oriented) | **N** by default; we'd implement it in our plugin (handled by **us**) | **N** | **YES — custom plugin repo we own forever** (`asdf-cognicode`) + optional shortname-index PR | `asdf set`, `asdf install`, `asdf latest`; no self-upgrade for asdf itself | **High** (indefinite plugin upkeep) |
| **aqua** (standalone CLI) | **Y** — package definition generated from a GitHub release via `aqua gr` / `argd s` (`github_release` auto-scaffolding); entry lives in aqua-registry or a custom registry | **Y** — forced pinning (no `latest`), `aqua.yaml`; `aqua update` / Renovate preset | **Y** | **Y** | **Y** (Windows supported) | Checksums generated into the registry entry; handled by **aqua** | **Y** — Checksum, Policy-as-Code, Cosign, SLSA, Minisign, GitHub Artifact Attestations | **Registry entry** (YAML, generated) we author; no plugin | `aqua up`; Renovate; per-project switching | **Medium** |
| **Homebrew / Linuxbrew** | **P** — not in core by default (core builds from source). A **custom tap** formula can `url` our GitHub tarball + `sha256`. | **P/N** — rolling; `brew pin` blocks upgrades; no lockfile; old versions not retained | **Y** | **Y** | **N** (WSL only) | **SHA-256** pinned in the formula; enforced by **Homebrew** | **P** — Homebrew verifies *bottle* provenance attestations; our upstream release asset is only sha256-protected unless we also emit bottles | **YES — a tap repo (`homebrew-…`) + formula we author**, or a homebrew-core PR (source build) | `brew upgrade` | **Medium–high** per release |
| **cargo-binstall** | **P** — works from GitHub Release binaries, but the **default `pkg-url` assumes standard filenames** (`{name}-{target}-{version}` with Rust triples). Our names are non-standard → needs `[package.metadata.binstall]`. Also resolves crate info from **crates.io**. | **P** — `crate@version`; no lockfile | **Y** | **Y** | **Y** | Verifies the **crate tarball** checksum from crates.io; binary artifact checksum not verified. Optional **minisign** signature | **N** | **No plugin**, but requires publishing the crate to crates.io and adding `[package.metadata.binstall]` to `Cargo.toml` | `cargo binstall crate@ver`; `cargo-update` | **Low–medium** |
| **proto** | **N** native. Needs a plugin (non-WASM TOML or WASM) or a community-plugins registry entry. | **Y** — `.prototools`, contextual version detection; no project lockfile documented | **Y** | **Y** | **Y** (msvc) | Checksum verification built into the install flow (supplied by plugin config) | **N** | **YES — plugin/registry entry we author** (or a contribution to moonrepo/community-plugins) | `proto install/use/upgrade` | **Medium** |
| **SDKMAN** | **N** native. Vendor pushes releases through a **secured vendor API** (credentials, case-by-case onboarding) or a GH Action / Maven / Gradle vendor plugin. | **P** — `.sdkmanrc`, `sdk default`; no lockfile | **Y** | **Y** | **N** (Unix-based) | Optional MD5/SHA-1/224/256/384/512 map supplied by the vendor; enforced by **SDKMAN** | **N** | Not a plugin, but requires vendor onboarding + credentials + release API automation we maintain | `sdk install/use/default/env` | **Medium** |
| **Nix** | **N** native. A `fetchurl`/`fetchzip` **derivation** must be authored (nixpkgs PR or our own flake). | **Y** — nixpkgs pin / `flake.lock`; content-addressed hashes | **Y** | **Y** (x86_64-darwin deprecating) | **N** native (WSL2 only) | Nix hash (sha256) in the derivation; enforced by **Nix** | **N** (no SLSA/cosign/GitHub-attestation step; Nix binary-cache signatures only) | **YES — a derivation** (nixpkgs or flake) we author/maintain | `nix profile upgrade`; declarative | **Medium–high** per release |

---

## 2. Per-channel notes and verification sources

### 2.1 mise — native GitHub backend  *(PRIMARY)*
**Verified claims:**
- `github:owner/repo` is a first-class backend needing **no plugin**; it autodetects OS, arch,
  libc (gnu/musl/msvc), archive format and build type. Options include `asset_pattern`,
  `matching`, `matching_regex`, `version_prefix`, `strip_components`, `bin`, `bin_path`,
  `rename_exe`, `checksum`, `github_attestations`.
- For a release with **multiple binaries published as separate per-platform assets**, the docs
  say to use `matching` / `matching_regex` to select the intended binary while keeping platform
  autodetection. This is exactly our case (two tarballs per target), and it confirms we can
  target `cognicode-cli-*` without a plugin.
- `mise.lock` records artifact URL, checksum (SHA-256 / Blake3) and verification metadata for
  URL-lockable backends; the GitHub backend is URL-lockable.
- GitHub Artifact Attestations verification is **default on** for the GitHub backend
  (`github.github_attestations`, default `true`), along with SLSA provenance
  (`github.slsa`, default `true`).

**Sources:**
- https://mise.jdx.dev/dev-tools/backends/github.html
- https://mise.jdx.dev/dev-tools/mise-lock.html
- https://mise.jdx.dev/security.html

**Caveat:** because we publish two assets per target and our target tokens are non-standard, a
small tool definition (e.g. `matching = "cognicode-cli-"`, possibly `version_prefix`, plus a
`rename_exe`) will almost certainly be required. That is declarative config we author in
`mise.toml`, not a plugin, and it is version-controlled by us.

### 2.2 mise — aqua backend  *(confirmation of the prior claim)*
**Verified claim:** mise does consume aqua package definitions. `aqua:` is a native backend;
mise does **not** shell out to the aqua CLI and uses the aqua registry (compiled into the mise
binary, plus configurable custom registries via `aqua.registries`). So *if* an aqua definition
exists for our repo, mise users get it via `aqua:Rubentxu/CogniCode`.
**Sources:**
- https://mise.jdx.dev/dev-tools/backends/aqua.html
- https://mise.jdx.dev/security.html

### 2.3 asdf  *(COMPATIBILITY)*
**Verified claims:**
- asdf 0.16 is a **complete rewrite in Go**, shipped as a binary rather than Bash scripts.
- Breaking changes confirmed from the official upgrade guide: `asdf global` and `asdf local`
  were **removed and replaced by `asdf set`**; `asdf update` was **removed** (upgrades now via
  the OS package manager or a manual binary download); `asdf shell` was removed; hyphenated
  commands (`plugin-add`, `list-all`, …) were removed.
- A plugin is a Git repo with executable scripts (`bin/list-all`, `bin/download`, `bin/install`
  required). There is **no** native GitHub-release backend and no built-in checksum step:
  whatever verification exists we write ourselves inside the plugin.
- Recommended install of a plugin is by URL; a shortname-index PR is needed for short-name use.

**Sources:**
- https://asdf-vm.com/guide/upgrading-to-v0-16.html
- https://asdf-vm.com/plugins/create.html

**Consequence:** asdf support is *not free* — it is a plugin repository we must author and keep
alive. That is why it belongs in Compatibility, not in the first wave.

### 2.4 aqua  *(SECONDARY)*
**Verified claims:**
- aqua generates package definitions from a GitHub release: `aqua gr` scaffolds
  `aqua-generate-registry.yaml`, and `argd s` auto-generates registry code for
  `github_release` (and `cargo`) packages. Contributions to the Standard Registry must use
  `argd s`.
- Security features: Checksum Verification, Policy as Code, Cosign + SLSA Provenance, Minisign,
  and **GitHub Artifact Attestations** (all documented in the security reference).
- aqua forces strict version pinning (does not allow installing `latest`) and supports
  per-project version switching, with a Renovate preset and `aqua update`.
- Windows is supported.

**Sources:**
- https://aquaproj.github.io/docs/ (overview, comparison, security list)
- https://aquaproj.github.io/docs/reference/security/
- https://github.com/aquaproj/aqua-registry/blob/main/docs/add_package.md
- https://github.com/aquaproj/aqua-registry/blob/main/CONTRIBUTING.md

**Consequence:** aqua is plugin-free but registry-entry dependent. The entry can be authored
fresh each release with tooling, or contributed upstream. This is materially cheaper than a
hand-written plugin, which is why aqua sits in Secondary.

### 2.5 Homebrew / Linuxbrew  *(SECONDARY)*
**Verified claims:**
- A **formula** is a Ruby package definition that builds from upstream sources; a **cask**
  installs pre-built binaries signed by upstream. The docs state that *"Open-source
  command-line-only software normally belongs in homebrew/core as a formula built from
  source"* — i.e. our prebuilt tarball is not the natural homebrew-core shape.
- Formulae pin downloads to an explicit **sha256** in the reviewed formula file; Homebrew
  refuses installs whose bytes do not match. That digest is supplied by the package author.
- Homebrew supports **Linux** (Homebrew on Linux; default prefix `/home/linuxbrew/.linuxbrew`)
  and macOS; **Windows is only via WSL**.
- Third-party repositories are **taps**; tapping does not grant whole-tap trust, and tap code
  runs with the user's privileges. A tap repo is named `homebrew-<name>`.
- Bottle provenance: `HOMEBREW_VERIFY_ATTESTATIONS` makes Homebrew verify **bottle** build
  provenance via GitHub attestations, and third-party taps can attest their own bottles. This
  applies to *bottles*, not to a bare upstream tarball we `url`-download inside a formula.

**Sources:**
- https://docs.brew.sh/Formula-Cookbook
- https://docs.brew.sh/Acceptable-Casks
- https://docs.brew.sh/Homebrew-Security-and-Supply-Chain
- https://docs.brew.sh/Taps
- https://docs.brew.sh/Installation

**Consequence:** our realistic Homebrew path is a **custom tap + formula** we maintain (or a
homebrew-core source-build PR, which is heavier and out of scope for a prebuilt-binary plan).
That authoring/maintenance cost is why it stays in Secondary rather than Primary.

### 2.6 cargo-binstall  *(COMPATIBILITY)*
**Verified claims:**
- cargo-binstall resolves crate information from **crates.io** and searches the linked
  `repository` for matching releases.
- Default `pkg-url` patterns assume filenames like `{name}-{target}-{version}` /
  `{name}-{version}-{target}` with **Rust target triples**. If those defaults do not match, the
  crate must add `[package.metadata.binstall]` with templated `pkg-url` / `bin-dir` /
  `pkg-fmt`. Our names (`cognicode-cli-1.2.3-linux-x86-64.tar.gz`) do **not** match the
  defaults → metadata override is required.
- Checksums: binstall verifies the **crate tarball** checksum from crates.io and enforces
  HTTPS/TLS ≥ 1.2 for the artifact download; it does **not** verify a checksum for the release
  binary itself.
- Signatures: optional **minisign** (with `pubkey` in `Cargo.toml` and a `.sig` asset).
  Sigstore/SLSA/cosign are explicitly **not** supported ("Why not X?" in SIGNING.md).

**Sources:**
- https://github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md
- https://github.com/cargo-bins/cargo-binstall/blob/main/README.md
- https://github.com/cargo-bins/cargo-binstall/blob/main/SIGNING.md

**Consequence:** it is not "free from the release tarballs as-is". It needs a crates.io publish
plus binstall metadata, and it gives no attestation story. Compatibility only.

### 2.7 proto  *(NOT INITIALLY JUSTIFIED)*
**Verified claims:**
- proto is a Rust/WASM version manager with cross-platform support (macOS intel/arm, Linux
  GNU/musl intel/arm, Windows msvc), checksum verification, and **contextual version detection**
  (`.prototools`).
- Tools not built in are added via **plugins** (non-WASM config plugin or WASM plugin) or via a
  community-plugins registry entry generated from `moonrepo/community-plugins`; publishing a
  plugin uses static JSON in the proto repo.

**Sources:**
- https://moonrepo.dev/docs/proto
- https://moonrepo.dev/docs/proto/plugins

**Consequence:** getting `cogh` in proto means authoring and maintaining a plugin/registry
entry. With mise + aqua already covering the no-plugin path, proto is not justified initially.

### 2.8 SDKMAN  *(NOT INITIALLY JUSTIFIED — prior claim partially contradicted)*
**Verified claims:**
- SDKMAN's model is **SDK vendor** oriented: vendors publish releases through a **secured JSON
  REST API** at `vendors.sdkman.io` using `Consumer-Key`/`Consumer-Token`; access is granted
  "case-by-case" via a vendor onboarding process. There is no GitHub-release autodetection.
- It **does** support **multi-platform binary distributions**, with explicit platform keys
  including `LINUX_64`, `LINUX_ARM64`, `MAC_OSX`, `MAC_ARM64`, `WINDOWS_64` (and more), plus an
  optional vendor-supplied checksum map (MD5/SHA-1/…/SHA-512).
- It is Unix-based (installed/used via shell), so there is no native Windows host support.

**Source:** https://sdkman.io/vendors/ , https://sdkman.io/usage/

**Verdict:** the prior claim "fundamentally SDK/JVM-oriented" is **partially contradicted**: it
is vendor/SDK-oriented, but it is not JVM-only and it *can* distribute an arbitrary
multiplatform binary. What it cannot do is discover our GitHub release on its own — we would
have to onboard as a vendor, hold credentials, and run a release-API integration. That ongoing
integration is why it is not justified in the first wave.

### 2.9 Nix  *(SECONDARY)*
**Verified claims:**
- Packages are Nix expressions (`fetchurl`/`fetchzip` + derivation); a package must be added to
  nixpkgs or provided via a flake. Nothing auto-consumes a GitHub release.
- Reproducibility/pinning is via nixpkgs pin or `flake.lock`; fetch functions lock an output
  hash (content-addressed).
- Nixpkgs platform tiers cover Linux (tier 1/2) and macOS (tier 2; note the announced
  `x86_64-darwin` deprecation). **Windows is not a nixpkgs platform**; NixOS on Windows runs
  under WSL via the community NixOS-WSL project.
- No SLSA/cosign/GitHub-attestation verification for a `fetchurl` artifact — only the Nix hash.

**Sources:**
- https://nixos.org/manual/nixpkgs/stable/ (platform support tiers, fetchers)
- https://nix.dev/concepts/flakes.html (flake.lock pinning)
- https://wiki.nixos.org/wiki/WSL (Windows via WSL)

**Consequence:** authoring a derivation is cheap-ish, but per-release hash bumps and the
nixpkgs PR cadence are real recurring cost, and no Windows. Secondary.

---

## 3. Recommended role assignment for e87 (decision input only)

Presented as the hypothesis under test. I do not propose implementing any channel in e84.

### Primary — direct `install.sh` + mise GitHub backend
**Confirmed**, with one refinement.
- `install.sh` is the only channel fully under our control: it works on every target we ship
  (including Windows), needs no third-party registry/plugin, and is the bootstrap that can
  later hand off to LAYER 1 itself.
- The mise **GitHub backend** is the strongest managed channel: no plugin, cross-OS,
  per-project pinning, `mise.lock` with checksums, and GitHub Artifact Attestations + SLSA
  verification enabled by default.
- **Refinement:** treat it as "*install.sh* + *a tiny, declaratively configured mise tool
  definition*", not as zero-config, because we ship two archives per target and use
  non-standard target tokens.

### Secondary — aqua, Homebrew, Nix
**Confirmed.**
- **aqua**: plugin-free, machine-generated registry entry, strongest verification matrix
  (checksum + policy-as-code + cosign + SLSA + minisign + GitHub attestations), and it is
  reachable through mise's own `aqua:` backend, so one entry serves two channels.
- **Homebrew**: large reach, and Homebrew enforces our sha256; cost is a maintained tap/formula.
- **Nix**: strong pinning and a real audience, at the cost of a maintained derivation; no
  Windows.
- All three require *authored metadata* (registry entry / formula / derivation) rather than a
  plugin. That is the accepted cost for Secondary, and it is lower and more declarative than a
  code plugin.

### Compatibility — asdf, cargo-binstall
**Confirmed.**
- **asdf** requires a **custom plugin we author and maintain indefinitely**; only worth it as a
  compatibility bridge for existing asdf users.
- **cargo-binstall** requires binstall metadata plus a crates.io publish, gives no attestation
  story, and is only meaningful to Rust-toolchain users.

### Not initially justified — SDKMAN, proto plugin
**Confirmed, with a correction to the prior SDKMAN rationale.**
- **SDKMAN**: not because it cannot ship a multiplatform binary (it can), but because it
  requires vendor onboarding, credentials, and a release-API integration we would own — for an
  audience that is SDK/JVM-centric.
- **proto**: requires authoring a plugin/registry entry, duplicating what mise/aqua cover
  plugin-free.

### Hypothesis verdict

| Role | Hypothesis | Verdict |
|---|---|---|
| Primary | `install.sh` + mise GitHub backend | **CONFIRMED** (refine: mise needs a small tool definition, not zero config) |
| Secondary | aqua, Homebrew, Nix | **CONFIRMED** (all need authored metadata, none needs a code plugin) |
| Compatibility | asdf, cargo-binstall | **CONFIRMED** (asdf = custom plugin; cargo-binstall = metadata + crates.io) |
| Not justified | SDKMAN, proto plugin | **CONFIRMED** (rationale for SDKMAN clarified) |

**Prior claims tested:**

| Prior claim | Verdict |
|---|---|
| mise has a native `github:` backend, no plugin, autodetects OS/arch/libc, asset patterns, checksums, `mise.lock`, GitHub Artifact Attestation verification | **CONFIRMED** (all documented) |
| mise's aqua backend lets mise consume an aqua package definition | **CONFIRMED** |
| asdf 0.16 is a Go rewrite; removed `asdf global`, `asdf local`, `asdf update` | **CONFIRMED** (replaced by `asdf set`; `asdf update`/`asdf shell` removed) |
| aqua generates definitions from a GitHub release; supports policy-as-code + cosign + SLSA + minisign + GitHub attestations | **CONFIRMED** |
| cargo-binstall consumes release binaries as-is | **CONTRADICTED (partially):** default patterns assume standard filenames + Rust triples; ours need `[package.metadata.binstall]` and a crates.io publish |
| SDKMAN is fundamentally SDK/JVM-oriented and cannot distribute an arbitrary multiplatform binary | **CONTRADICTED (partially):** vendor/SDK-oriented, but it does support arbitrary multi-platform binaries via its vendor API; the real blocker is onboarding + credentials + integration |
| proto: Rust, WASM plugins, checksums, multi-OS, contextual versioning | **CONFIRMED** (adds: custom plugin/registry entry required) |

---

## 4. Cross-cutting finding for e87 (not a channel recommendation)

The release currently publishes **tarballs only**: no `checksums.txt` / `.sha256` assets and no
attestations. Every "checksums" and "attestations" cell above is therefore *conditional on us
publishing those artifacts*:

- Publishing a `checksums.txt` and/or per-asset `.sha256` unlocks checksum verification in
  channel-native ways (aqua registry metadata, cargo-binstall signing, plain `install.sh`).
- Emitting GitHub Artifact Attestations from the release workflow unlocks the strongest default
  path we have (mise GitHub backend disambiguation, mise aqua backend, aqua, and Homebrew
  bottles if we ever produce them).
- This artifact contract is exactly e84's remit; WU6 records it here only because the channel
  matrix depends on it.

---

## 5. UNVERIFIED

The following could not be confirmed from a primary source. They are **not** used to justify any
recommendation, and each could change a cell if resolved.

1. **Whether mise's GitHub backend autodetection actually selects our assets without an explicit
   `asset_pattern`/`matching`.** Our tokens are `linux-x86-64`, `mac-os-x86-64`,
   `mac-os-aarch64`, `windows-x86-64`. Documented autodetection scores on `linux/macos/windows`
   and `x64/arm64/x86/arm`; `mac-os` is not spelled `macos` and `arch64` is not `arm64`. The docs
   guarantee `matching`/`matching_regex` as the resolver for multi-binary releases, so a
   workaround exists, but I did not empirically verify the default behaviour against our repo.
   **Needs an experiment before e87 commits to "zero-config mise".**
2. **Whether mise's autodetection is happy with the same archive containing our binary layout.**
   The tarball's internal path structure was not inspected; `bin_path`/`rename_exe` may be
   needed.
3. **cargo-binstall crate availability.** Whether a crate named `cognicode-cli` (or similar) is
   (or will be) published to crates.io was not verified; binstall cannot work without it.
4. **Homebrew homebrew-core acceptance.** Whether homebrew-core would accept a *prebuilt-binary*
   formula for an OSS CLI is unverified; the docs point to source builds, so the custom-tap
   route is assumed.
5. **asdf Windows support.** No official statement either way was found; asdf's plugin model is
   Unix-shell oriented. Marked `N/?`.
6. **SDKMAN acceptance.** Whether CogniCode would be onboarded as an SDK vendor was not
   verified (onboarding is explicitly case-by-case).
7. **mise GitHub attestations reliability.** mise's own discussion tracker reports intermittent
   GitHub attestation verification failures (e.g. trusted-root/network issues). This is a
   maturity risk on the Primary channel, not a capability gap.
8. **Nix `x86_64-darwin` future.** Nixpkgs release notes announce a deprecation; the exact
   timeline was read only from release notes, not from a dedicated policy document.
