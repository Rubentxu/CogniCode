# Spec: lifecycle-draft-safety

> e86 WU1. Operational authority is `state.yaml`.

## Purpose

Between "release assets uploaded" and "draft published" in the e85
workflow, the GitHub Release is briefly present with `draft: true`.
A consumer that installs during that window gets the real binary and the
real manifest, but the release is not yet considered shipped. The contract
requires the consumer to refuse that window so a typo, an aborted
attestation, or a failed verify gate cannot leak into a downstream
install.

## Scope

In scope:

- Rejection of any release with `draft: true` or `prerelease: true`.
- The check is performed in `lifecycle_resolver` (REQ-LR-03), so every
  code path that consumes a release — `cogh latest`, `cogh update`,
  future `cogh rollback` to a draft — is protected by the same gate.
- A clear error message naming the tag and the reason.

Out of scope:

- A `--allow-draft` escape hatch. There is no scenario in which a
  consumer should install a draft. (CI does not install from cogh.)
- Auto-retry until the draft flips to published. The user runs
  `cogh update` again; the contract is deterministic, not polling.

## Requirements

### REQ-LDS-01 — single check

The draft/prerelease check MUST live in one place: the resolver. Any
command path that resolves a release MUST go through that gate.

### REQ-LDS-02 — error message shape

**When** a draft is detected,
**then** the error reads:

```text
release vX.Y.Z is still a draft; refusing to install (e85 WU13)
```

The "e85 WU13" reference points the operator to the producer-side gate.

**When** a prerelease is detected,
**then** the error reads:

```text
release vX.Y.Z is a prerelease; refusing to install (only stable is supported today)
```

### REQ-LDS-03 — applies to `--staging` resolver

The check MUST also fire when the resolver reads from
`--staging <dir>/releases.json`. A test that wants to exercise the happy
path MUST publish the synthetic release with `draft: false` and
`prerelease: false`.

## Scenarios

| ID | When | Then |
|---|---|---|
| SC-LDS-01 | The resolved release has `draft: true` | Resolver returns `DraftRelease` with the WU13 reference |
| SC-LDS-02 | The resolved release has `prerelease: true` | Resolver returns `DraftRelease` with the prerelease message |
| SC-LDS-03 | `--staging` resolver sees a synthetic draft | Same rejection as SC-LDS-01 |
| SC-LDS-04 | The resolved release is `draft: false`, `prerelease: false`, has the manifest asset | Resolver proceeds to the install pipeline |

## Exit gates

This spec is satisfied when:

- The resolver tests for SC-LDS-01..04 pass.
- The published v0.95.0 release passes the check (real-world acceptance:
  `cogh update` against `https://github.com/Rubentxu/CogniCode` resolves
  to v0.95.0 and proceeds to install).