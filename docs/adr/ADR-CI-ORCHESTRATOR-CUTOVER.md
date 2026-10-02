---
title: "ADR — CI/CD orchestration authority: GitHub Actions to PipelineK"
slug: "ADR-CI-ORCHESTRATOR-CUTOVER"
status: accepted
date: 2026-10-02
deciders: Maintainer
related:
  - "[[ADR-CANONICAL-LAYOUT-versions]]"
context:
  - "Six GitHub Actions workflows totalling 2354 lines orchestrate change-fast, integration, release-candidate, release and certification for CogniCode"
  - "Three gates could not be seen by the coverage inventory, and one job's failure could not fail the lane it was supposed to gate, because the topology being ported was a property of the YAML rather than of the work"
  - "The clippy gate lived in `ci.yml`, which the policy treats as local-only, while the check `main` actually requires came from `pr-ci.yml`"
---

# ADR — CI/CD orchestration authority: GitHub Actions to PipelineK

## Decision

PipelineK is the only execution authority for CI/CD in this repository.
GitHub Actions is retired. GitHub Releases remains the *destination* for
distribution; the `gh release …` calls stay, and nothing else does.

Six lanes replace the workflows, with distinguishable responsibilities rather
than a port of the job graph:

| Lane | Replaces | Responsibility |
|---|---|---|
| `merge-gate.pipeline.kts` | `pr-ci.yml` | what a change must pass to merge |
| `integration.pipeline.kts` | `ci.yml` | full matrix, cross-platform, adversarial |
| `product-fast.pipeline.kts` | — | product build without release binaries |
| `release-candidate.pipeline.kts` | `release-validate.yml` | build and certify an immutable candidate |
| `release.pipeline.kts` | `release.yml` | publish exactly the validated candidate |
| `certification.pipeline.kts` | `sandbox-nightly.yml`, `regression-check.yml` | the expensive, slow, nightly work |

## Why the job graph was not ported

The workflows' shape encoded three things, only one of which was the work:

1. **The work.** Gates, builds, negative tests. Migrated.
2. **A topology.** `upload-artifact` / `download-artifact` between jobs on
   isolated runners. Measured under `pipelinek` 0.46.0: stages share a
   filesystem, so the candidate is a directory and no transfer exists to
   reproduce. `release.pipeline.kts` reads `release/`; it cannot rebuild, and
   therefore cannot accidentally publish something other than what was
   certified.
3. **An accident of the runner.** A failing step aborting the lane came from
   `set -euo pipefail` in the shell, not from a workflow feature. The same
   measurement showed a failed stage aborts a PipelineK run, so the property
   carried over — and so did its consequence: PipelineK has no
   `continue-on-error`, which is why every advisory stage ends with
   `|| echo 'ADVISORY: …'`. That is the whole mechanism, not a loose substitute.

## What died, and what it was protecting

`action_ref_pin_contract.rs` is deleted. It required every third-party `uses:`
ref to be pinned to an immutable SHA, after an audit found 19 refs resolved
through a mutable branch while a comment claimed otherwise. The property was
real and the contract caught it.

It becomes vacuous here. With no third-party actions there is no third-party
ref to pin, and a test that passes because its subject is empty is the
N+66 ghost-filter shape the repository already has a name for. The same applies
to `prf_f6_w3_bis_staging_contract.rs`'s assertion that both workflows agree on
`upload-artifact` name patterns, and to two `pr-ci.yml` fixtures left orphaned
when QW-09 was retired.

The properties underneath them did not die, and were kept:

- **Mutable third-party pointer** → nothing in the release path resolves a
  version that is not a declared input. `cargo install cargo-deny --locked`
  and the pinned toolchain are the surviving form of the guarantee.
- **Validate and publish the same bytes** → the candidate is a directory on
  disk, hashed by `SHA256SUMS`, re-verified after upload and again by a
  consumer that re-downloads and runs `sha256sum -c`.
- **Flatten script rejects a malformed staging tree** → kept as a behavioural
  contract against the script, with its five negative cases, because that is
  product behaviour rather than workflow topology.

## Two things this cutover does not solve

Both are maintainer decisions, recorded here rather than settled here.

### The required check cannot outlive its own producer

`main` requires a status context named `merge-gate` with `strict: true`. That
context has always come from a job *named* `merge-gate` in `pr-ci.yml`. The
pull request that deletes `pr-ci.yml` therefore cannot produce its own required
check, and is blocked permanently. This is not missing work.

`scripts/ci/publish-merge-gate.sh` closes the mechanism: it runs the merge
authority and reports the exit code under that context, with the run database
kept inside the repository so the verdict is auditable afterwards. It was
demonstrated against the real API, not against a recorder.

It does not close the decision. A status published by a script is an assertion
by whoever ran it. GitHub verifies that a check reported success, not that a
lane ran to produce it. Under Actions only a workflow triggered by the pull
request itself could report it. That is a real exchange of governance for
convenience, and it is the maintainer's to accept or reject.

### Provenance has no equivalent

`actions/attest-build-provenance` generates SLSA provenance. It is an Action,
and PipelineK has no Actions runtime. The consumption half survives —
`gh attestation verify` still runs, and `RELEASE_REQUIRE_PROVENANCE=1` fails
closed rather than silently publishing without it — but nothing generates the
attestation. A substitute has not been chosen, and shipping a release whose
provenance is opt-in and unmet is a decision, not an oversight.

## Consequences

- `scripts/ci/test_ci_orchestrator_gap.py` had a job as a migration
  instrument: keep Actions from losing a gate while it was being ported. When
  Actions reaches zero it must not survive still demanding that the Actions side
  have gates, or it keeps as a dependency on the system being deleted. It
  becomes a single invariant: executable GitHub Actions workflows == 0.
- Every contract that read a workflow as its authority had to be re-anchored by
  intent, not renamed. `target/release/` is not "the build output" on a machine
  with `build.target-dir` set, and `${'$'}cd` in a Kotlin raw string is not a
  directory change. Both were live defects in the first end-to-end run.
- An advisory lane produces visible evidence without a semantic PASS. A gate
  that exists only in documentation is not a gate.

## What was deliberately left alone

Three things still name a workflow, and rewriting them would have been the
wrong move. Each is recorded here so the decision is visible rather than
inferred from a diff that did not touch them.

**`openspec/specs/ci-postgres-pipeline/spec.md` still requires
`.github/workflows/ci.yml`.** The spec is marked `OBSOLETE` since 2026-08-04:
PostgreSQL was removed from CI and the stack under ADR-026 (e29-7), it has no
enforcing test, and its own header says to archive it. It is not a live
contract, so editing its requirements would have been reworking a closed
decision rather than reconciling an open one. The right action is to archive
the file, which is a separate piece of housekeeping.

**`docs/prf/specs/SPEC-CI.md` still names `ci.yml` and `just ci-local`.** PRF
is historical evidence and `docs/prf/` is read-only by project rule. The
contract that reads it only asserts that `-D warnings` is mentioned, which is
still true, and the document's claims about CI are evidence of what was
certified rather than a statement about the present.

**`crates/cognicode-cli/tests/prf_ci_01_07_clippy_gate_uat.rs` still reads the
PRF spec.** Same reason, same conclusion. The part of that file that named
`ci.yml` as the authority was re-anchored; the part that reads the spec was
not, because the spec is the record of the certification.
