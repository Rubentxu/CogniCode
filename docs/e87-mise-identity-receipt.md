# e87 mise channel — Identity Receipt (M2)

Generated: 2026-09-18. Platform: linux x86_64. Release: v0.96.0 (public GitHub release, SoT).

## Claim

`install.sh` and `mise` resolve the SAME published artifact for a given
release + platform. No channel-specific builds exist.

## Evidence (all OBSERVED, commands reproducible)

1. Official asset digest (SHA256SUMS of the release):
   `cogh-0.96.0-x86_64-unknown-linux-gnu.tar.gz`
   sha256 = `90a1735915d71fb35522e8f874819e3ff02cbad823c68acb1c54ff47c9ca2ac1`

2. Fresh direct download of the asset, extracted:
   `bin/cogh` sha256 = `b058c6eebd8492cfb3b29b9aba9674e7cd3140b39b921659a4866810344923e2`

3. install.sh bootstrap (v0.96.0 pin, checksum-verified — fail-closed against
   the SHA256SUMS digest above), installed binary:
   `~/.cognicode/bin/cogh` sha256 = `b058c6eebd8492cfb3b29b9aba9674e7cd3140b39b921659a4866810344923e2`

4. mise declarative backend, pinned:
   `mise install "github:Rubentxu/CogniCode[matching=cogh-]"@0.96.0`
   installed binary sha256 = `b058c6eebd8492cfb3b29b9aba9674e7cd3140b39b921659a4866810344923e2`
   Channel identity metadata: `installs/.mise-installs.toml` +
   `<install>/.mise.backend.toml` (`short = "github:Rubentxu/CogniCode"`,
   `[opts] matching = "cogh-"`).

## Result

```text
install.sh asset == mise asset == direct download      (same release asset)
install.sh sha   == mise sha   == SHA256SUMS-verified  (b058c6ee…, binary)
tarball digest   == SHA256SUMS entry                   (90a17359…)
cogh --version   == 0.96.0 in all three paths
```

## M4 — Channel ownership contract (Layer 0 / Layer 1)

- mise owns the Layer-0 `cogh` binary when installed via mise: the install
  lives under `MISE_DATA_DIR/installs/github-rubentxu-cogni-code/<v>/` with
  ownership metadata in `.mise.backend.toml` / `.mise-installs.toml`. Future
  `cogh self-update` MUST detect this ownership (via mise registry metadata,
  never path heuristics) and defer the binary replacement to `mise upgrade`.
- `install.sh`-installed binaries are owned by the `~/.cognicode/bin` layout;
  only there may cogh self-update replace itself.
- In both cases cogh owns Layer 1 (`~/.cognicode` runtime: versions/,
  shims/, bundle.yaml, tracker). Channel and runtime are independent.
- Status: contract documented; `cogh self-update` is NOT implemented in
  e87 (deferred; mise detection must use mise metadata when built).
