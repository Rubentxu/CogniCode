# Release Receipt — v0.97.0 (e87 baseline)

Date: 2026-09-18. Released by the e87 remote checkpoint authorization.

## Identity

| Field | Value |
|---|---|
| Version | 0.97.0 (MINOR bump: 4× `feat` since v0.96.0, per conventional-commits contract) |
| Release commit (HEAD) | `627b2efcac068fcc56686be5fa7f16d32422fd94` |
| Tag | `v0.97.0` (annotated, points to the release commit — verified) |
| ReleaseInventory source_commit | `627b2efcac068fcc56686be5fa7f16d32422fd94` — matches |
| Workflow run | 35374506747 |
| Jobs | build-linux-x86-64: success · build-linux-aarch64: success · assemble-and-publish: success |
| Draft at publish | false (draft-first model preserved; published 2026-09-18T17:42:05Z) |

## Gates executed pre-tag

- workspace release build: OK
- cogh bin suite: 274 passed / 0 failed / 1 ignored (one transient env-race flake re-verified 3/3 green)
- known-failures exactness: 41 entries unchanged (OK)
- fmt: cognicode-cli clean (core fmt violations pre-existing, out of e87 scope)
- clippy: no new errors
- check-release-matrix (R9/tiers/runners): OK
- commit-lint since v0.96.0: all conventional
- e87 tripwires on the release-built binary (not just source):
  - `install --version 9.9.9` → real GitHub API call (404) → hard ERROR, no DEV fallback, no versions/ tree
  - full install v0.96.0 assets + `doctor` → overall: healthy via shim probe

## Asset inventory (12)

bundle-0.97.0-{x86_64,aarch64}.yaml · cogh-0.97.0-{x86_64,aarch64}.tar.gz ·
cognicode-0.97.0-{x86_64,aarch64}.tar.gz · cognicode-0.97.0.tar.gz ·
cognicode-mcp-0.97.0-{x86_64,aarch64}.tar.gz · cognicode-mcp-0.97.0.tar.gz ·
release-inventory-0.97.0.json · SHA256SUMS

**SHA256SUMS consistency: 11/11 assets verified against the published checksums (OBSERVED).**

## e88 entry gates (public bits only)

Fresh download of the public release into an empty dir; disposable HOME;
no checkout, no target/debug, no localhost, no staging, no COGNICODE_* seams.
Script: `scripts/e88-entry-gates.sh`.

- **E88-G0a PASS**: public `cogh --version` == `cogh 0.97.0`; bootstrap via the
  public `install.sh` (checksum verified, `788cec59…`); `cogh install mcp-server
  --version 0.97.0 --profile reviewer` resolved the public v0.97.0 release,
  consumed the public BundleManifest, verified component assets
  (cognicode-mcp-0.97.0-x86_64…), tracker = 0.97.0, zero DEV-fixture warnings.
- **E88-G0b PASS**: `cogh doctor` (the *published* binary) reports
  `MCP: cognicode-mcp shim present` via the canonical shim and
  **overall: healthy** — the doctor fix is provably in the consumable artifact.

## Conclusion

The v0.97.0 release is public, non-draft, externally verifiable and coherent.
**e88 — Fresh Linux Lifecycle UAT is hereby unlocked** (baseline: public-consumer
available). e88's core rule: any path inside the checkout (e.g. `target/debug/cogh`)
is invalid evidence.
