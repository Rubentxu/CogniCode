# e87 — Verification Report (closure)

Status: **CLOSED** — all acceptance criteria OBSERVED GREEN.

## Acceptance

| Criterion | Evidence |
|---|---|
| install.sh bootstrap GREEN | T1 platform fail-loud (Darwin/arm64), T2 tamper fail-closed (no binary installed), T3 version pin exact, T4 Layer-0 purity — disposable HOMEs |
| mise bootstrap GREEN | `mise install "github:Rubentxu/CogniCode[matching=cogh-]"@0.96.0` — declarative backend, no executable plugin |
| same release artifact PROVEN | docs/e87-mise-identity-receipt.md — 3-way digest identity |
| same SHA PROVEN | binary sha256 `b058c6ee…` identical (install.sh == mise == direct download); tarball `90a17359…` == SHA256SUMS |
| requested version governs Layer-1 | e87.1 strict tests T1/T2 (T2 demonstrated RED pre-fix), resolver-driven path without COGNICODE_BUNDLE_MANIFEST seam |
| public BundleManifest consumed | WU4 UAT: `cogh install mcp-server --version 0.96.0` against real GitHub v0.96.0 |
| public component SHA verification | WU4 UAT: SHA-verified component downloads, tracker = 0.96.0 |
| DEV fallback product path IMPOSSIBLE | e871_t4: resolution failure = hard error, no versions/ tree |
| Layer-0 / Layer-1 separation PROVEN | install.sh installs only `bin/cogh`; runtime via `cogh install`; mise ownership contract documented |
| real HOME pollution ZERO | all UATs in disposable HOMEs; pre-existing real `~/.cognicode` untouched |

## UAT classification

- GREEN: **public-release-assets UAT** (local cogh with e87.1 + public v0.96.0 manifests/assets).
- NOT claimed: 100% public consumer UAT — requires the next release binary (e88 gate).

## Extra findings fixed in-cycle (regression repair, no new DEBT cycle)

- 3 lifecycle uninstall tests stale after versions/ layout switch (planted `versions/<v>` tree).
- `doctor` MCP probe followed pre-versions/ layout (`bin/cognicode-mcp`) — now probes the canonical `shims/cognicode-mcp` (ff78f78e).

## Handoff

Next release must contain: install bridge, doctor shim fix, mise docs/receipt.
After publication: e88 — Fresh Linux Lifecycle UAT (public bits only; any checkout
path such as `target/debug/cogh` is invalid evidence for e88).
