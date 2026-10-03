---
title: "ADR — Release provenance without GitHub Actions"
slug: "ADR-RELEASE-PROVENANCE-PIPELINEK"
status: proposed
date: 2026-10-03
deciders: Maintainer
related:
  - "[[ADR-CI-ORCHESTRATOR-CUTOVER]]"
  - "[[ADR-OWNERSHIP-MAP-install-vs-versions]]"
context:
  - "Verification survived the cutover; generation did not. `gh attestation verify` still runs and `RELEASE_REQUIRE_PROVENANCE=1` fails closed, but nothing produces an attestation, because `actions/attest-build-provenance` was an Action and PipelineK has no Actions runtime"
  - "Three release tags carry no GitHub Release at all, and the one being published first (v0.101.0) ships with no attestation"
  - "cosign 3.1.3 is present on the build host and produces a verifiable SLSA provenance bundle offline with a key pair — measured, not assumed"
---

# ADR — Release provenance without GitHub Actions

## Status: proposed, not accepted

This ADR records a **measured** conclusion and one decision that is still open.
It is written as `proposed` because the part that matters operationally — where
the signing key lives — is a custody decision with consequences that outlive any
release, and it is not mine to take.

## The gap, stated exactly

Consumption survived the cutover. Generation did not:

| Half | Status | Where |
|---|---|---|
| Verify an artifact has provenance | present | `gh attestation verify`, in `release.pipeline.kts` |
| Fail closed when provenance is required | present | `RELEASE_REQUIRE_PROVENANCE=1` → exit 1 |
| Fail closed *by default* | **absent** | the variable defaults to `0`, so the gate is opt-in |
| Generate an attestation | **absent** | was `actions/attest-build-provenance` |

So the lane today can be told to require provenance, will correctly refuse when
there is none, and will then have no way to obtain it. That is the honest shape
of the hole: the refusal is already right, the producer is missing.

The practical consequence is not theoretical. v0.99.2, v0.100.0 and v0.101.0
were tagged and never published; `install.sh` without a pin resolves
`api/releases/latest`, which serves **v0.98.1**, published 2026-09-24. A fresh
install therefore receives a binary three minor versions behind the repository
while `README.md` tells the reader that both channels install the same published
asset. Fixing the distribution is the whole of R1; this ADR is what stops the
next release from shipping with the same gap.

## What retiring Actions actually cost

This is the part worth recording, because it is not visible in the diff.

Keyless signing is the *default* shape of SLSA provenance: no long-lived secret
exists, the signer proves an identity through a federated OIDC token, and the
attestation is anchored in a public transparency log. That worked here because
`actions/attest-build-provenance` ran inside Actions, and **Actions was the OIDC
token issuer**. `cosign` does not implement that token minting; it consumes one.

So the cutover did not merely remove a step. It removed the only thing in the
pipeline that could produce a *verifiable identity* for it. Any replacement
provenance path has to answer the question the token used to answer, and it has
to answer it from outside Actions, or provenance silently degrades from
"verifiable by anyone, no secret" to "verifiable against a key we hold".

## Options considered

### A. Keyless via a different OIDC issuer — rejected for now

Keeps the no-secret property. Requires an OIDC provider reachable from the build
host, and a trust-root decision for consumers. Nothing on the measured host
provides one, and adding an identity provider is a larger commitment than a
release-provenance gap warrants.

### B. cosign key pair, offline — recommended, subject to custody

Measured on the build host, cosign **3.1.3**:

```
$ cosign generate-key-pair
Private key written to cosign.key
Public key written to cosign.pub

$ cosign attest-blob --key cosign.key --bundle att.bundle.json \
    --predicate predicate.json --type https://slsa.dev/provenance/v1 \
    artifact.bin
Wrote bundle to file att.bundle.json

$ cosign verify-blob --key cosign.pub --bundle att.bundle.json artifact.bin
Verified OK
```

And the exit gate this ADR exists to satisfy, checked rather than asserted —
`subject.digest.sha256` inside the DSSE envelope against the artifact's own
`sha256sum`:

```
subject.digest.sha256 : 0e02297fb55098e1f7c96e078707d0c4048d0dd3bda0fa2074bb7552cdb592de
artifact sha256sum    : 0e02297fb55098e1f7c96e078707d0c4048d0dd3bda0fa2074bb7552cdb592de
MATCH                 : True
```

No network, no transparency log, no registry. Two properties worth knowing
because both cost time otherwise:

- cosign 3.1.3 **requires `--bundle`**; without it the command fails with
  `must specify --bundle with --new-bundle-format`.
- The bundle's `verificationMaterial` decides how to verify it. With a key pair
  it contains `publicKey`, so verification takes `--key` **and nothing else**;
  passing `--certificate-identity-regexp` as well is an error that names the
  field to inspect.
- The subject digest is the **hex** sha256, stored verbatim. Decoding it as
  base64 — which is a reasonable guess, and which I did — produces a mismatch
  that looks like a signing bug and is not one.

Cost: a long-lived private key whose loss makes every attestation signed by it
unverifiable *as key-signed* for the rest of time. The public half travels with
the release, so consumers can always check that a bundle was signed by the key
this release publishes; what a lost key destroys is the ability to produce
*future* attestations that chain to it.

### C. Ship unsigned and record it — what happens if this ADR is not accepted

`RELEASE_REQUIRE_PROVENANCE` stays `0`, the lane publishes with an advisory, and
nothing in the release says so except the absence of a file. A consumer cannot
tell "never signed" from "signed by a key we have not seen". The refusal logic
already in the lane is good and is currently reachable only by someone who knows
to set an environment variable.

## Recommendation

Adopt **B**, with two rules that make it more than a default:

1. **The public key ships with the release**, alongside `SHA256SUMS`, and the
   release lane's `provenance` stage verifies the bundle with *that* key. A
   consumer needs the release, not the publisher, to check it.
2. **`RELEASE_REQUIRE_PROVENANCE` defaults to `1`**, and generation runs in
   `release-candidate` so the attestation covers the candidate that was
   certified. Flip the default only once generation exists in the same lane —
   inverting this order breaks releases, which is the failure the current
   opt-in default was chosen to avoid.

Provenance belongs to the **release infrastructure boundary**: the lanes and
`scripts/`. It does not enter `cognicode-core`, and nothing about it should be
reachable from the MCP tool surface.

## Open decision, and it is the whole decision

**Where does the private key live, and who can use it?**

The candidates, with what each one costs:

| Custody | What it buys | What it risks |
|---|---|---|
| Maintainer workstation, passphrase-protected | No new infrastructure | A single machine is a single point of loss; the passphrase is a human secret |
| CI secret on a runner | Works unattended | The key is readable by whatever can run the lane; runner compromise becomes signing compromise |
| Offline signer, invoked deliberately | The strongest separation | Manual step before each release, which is a step people skip |

This ADR does not pick one. What it does fix is the requirement: the key must
not live in this repository, and the public half must be part of the release.

## Consequences if accepted

- `release-candidate` gains a provenance stage that signs `SHA256SUMS` and the
  per-platform archives, and writes the bundle plus the public key into the
  candidate directory.
- `release` uploads them and verifies with the key from the same candidate, which
  is the same "publish exactly what was certified" property the candidate
  directory already provides for binaries.
- A contract asserts the three digests agree:
  `candidate digest == attestation subject digest == published asset digest`,
  and that verification succeeds from a clean download, not from the build host.
  Verified from the build host proves the signer works; verified from a clean
  download proves the *consumer's* path works, and only the second one is the
  property users depend on.

## What is deliberately not decided here

- Whether to migrate later to keyless once an OIDC issuer is available. The
  bundle format does not need to change: a key-signed bundle and a Fulcio-signed
  bundle are the same DSSE envelope with different `verificationMaterial`.
- Transparency-log anchoring. `cosign attest-blob` did not require it and the
  measured path did not use it; a log entry would add a third party to the
  release's trust story and that is a separate decision.
- Retroactive provenance for v0.98.1 and earlier. It cannot be manufactured, and
  a release that predates the tooling must be described as predating it.
