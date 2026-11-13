---
title: "ADR — Release provenance without GitHub Actions"
slug: "ADR-RELEASE-PROVENANCE-PIPELINEK"
status: accepted
date: 2026-10-03
accepted: 2026-11-13
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

## Status: accepted (2026-11-13)

This ADR was written as `proposed` on 2026-10-03: it recorded a **measured**
conclusion and one decision it could not take, because where the signing key
lives is a custody decision with consequences that outlive any release.

The maintainer took that decision on 2026-11-13 and delegated the choice of
custody model to the author of this record; it is written down in "Custody
decision" below. The sections before it keep the reasoning as it stood while
the decision was open; the sections after it describe the current state.

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

## Custody decision (formerly "Open decision, and it is the whole decision")

**Where does the private key live, and who can use it?**

The candidates, with what each one costs:

| Custody | What it buys | What it risks |
|---|---|---|
| Maintainer workstation, passphrase-protected | No new infrastructure | A single machine is a single point of loss; the passphrase is a human secret |
| CI secret on a runner | Works unattended | The key is readable by whatever can run the lane; runner compromise becomes signing compromise |
| Offline signer, invoked deliberately | The strongest separation | Manual step before each release, which is a step people skip |

The requirement never changed: the key must not live in this repository, and
the public half must be part of the release.

### Decision, 2026-11-13

**The maintainer's workstation, passphrase-protected, used deliberately at
release time.** That is the first row's infrastructure with the third row's
discipline: no new service, and nothing signs unless a person runs the command.
The CI-secret row is rejected: with no unattended signing there is no secret on a
runner for a runner compromise to reach.

How it is held:

- **Private key** `cognicode-release.key`: outside any repository, under the
  maintainer's `~/.config/cognicode/provenance/`, mode 600, protected by a
  passphrase that is never written to the repository, to a lane's configuration
  or to a lane's environment.
- **Backup**: an encrypted copy elsewhere is the maintainer's responsibility.
  Nothing in this repository can verify that it exists. Losing the key costs the
  ability to sign future releases with it; bundles already published stay
  verifiable against the public key that was published with them.
- **Creation**: the maintainer generated the pair in their own terminal with the
  pinned cosign (3.1.3). `attest-provenance.sh` still contains no
  `generate-key-pair` call, and a contract still fails if one appears.
- **Public key**: published with every release as the asset
  `cognicode-release.pub`. Its sha256 for the first key is
  `3937ad77ef19712a05e3a5f467aaafefa9dec9b3713b20239f38ccf3d48b08a8`, recorded
  here so a consumer can compare the release asset against a second place.
- **Signing**: from a checkout whose `HEAD` is the release commit (the statement
  records `git rev-parse HEAD` of the current directory),
  `scripts/ci/attest-provenance.sh --key <key> --out-dir <dir> <artifact>...`
  with the pinned cosign; the `<artifact>.bundle.json` files and the public key
  are uploaded as additional release assets.
- **Verification by a consumer**:
  `cosign verify-blob --key cognicode-release.pub --bundle <artifact>.bundle.json <artifact>`,
  which prints `Verified OK` on stderr. It refuses a file whose bytes differ
  (`provided artifact digests do not match digests in statement`).

### What this decision does not change

- `RELEASE_REQUIRE_PROVENANCE` stays `0`. `verify-provenance.sh` verifies with
  `gh attestation verify`, which does not recognise a key-pair signature, so
  setting it to `1` would make every release refuse. Flipping it needs the
  script to verify key-signed bundles with the published key, which changes the
  behaviour of the release lane and is a separate unit of work.
- A signing key held by a lane stays rejected. If it is ever wanted, it is a new
  custody decision, not a configuration change.

### First use: v0.103.1, signed retroactively

On 2026-11-13 the seven artifacts listed in `SHA256SUMS` of the already published
v0.103.1 were signed, and the bundles plus the public key were added to that
release as new assets; none of the original eight assets was replaced. Measured
from a clean download of the published release: `sha256sum -c SHA256SUMS`
passes, `cosign verify-blob` prints `Verified OK` for 7 of 7, each statement's
subject digest equals the file's sha256, and each statement names commit
`793033d40687781c68ab70613bc6210e9a6f54ac`, the commit the tag points to.

What that proves, and what it does not:

- It proves the bytes, the commit the tag names, and that the holder of this key
  signed them on 2026-11-13.
- It does **not** prove how or where the artifacts were built. The statement
  carries the builder id `https://cognicode.dev/pipelinek/release-candidate`,
  but that is a label written at signing time, not evidence of the build
  environment.
- `gh attestation verify` still fails for v0.103.1 (HTTP 404), because no GitHub
  attestation exists. v0.102.1 and earlier carry no provenance at all.

## Consequences

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

With the custody decided above, the first bullet is carried out by the
maintainer at release time, with the same script, rather than by an unattended
stage: the stage described below only runs where a key is present
(`RELEASE_PROVENANCE_KEY`), and the key stays on the maintainer's workstation.

## What has been built since this ADR was written

B7 (2026-10-04) built the generation half and **left the decision open**; the
custody decision followed on 2026-11-13 (see "Custody decision"). The
mechanism and the custody were separate gaps, and both are now closed for the
signing half. The consumer half in the release lane is not (see the table).

| Piece | Where | State |
|---|---|---|
| Generate an attestation | `scripts/ci/attest-provenance.sh` | works, measured |
| SLSA/in-toto statement, DSSE envelope | inside the script | works, measured |
| Public key travels with the release | script copies it to the candidate dir | works, measured |
| First of the three digests checked | in the script, on every run | works, measured |
| Version del firmante | `scripts/ci/cosign-tool.sh` | **fijada**, con su suma declarada |
| Third digest, consumer download | where publishing happens | still the release lane's `gh attestation verify` |
| Custody | — | **decided 2026-11-13**: maintainer workstation, passphrase-protected, deliberate signing |
| `RELEASE_REQUIRE_PROVENANCE` default | `verify-provenance.sh` | still `0`, deliberately: the script cannot verify a key-pair bundle yet |
| Latest published release | `v0.103.1` | key-signed on 2026-11-13, retroactively; `cosign verify-blob` passes for 7 of 7 artifacts, `gh attestation verify` still returns HTTP 404 |
| Earlier releases (`v0.102.1` and before) | — | no provenance; checksums match, so integrity holds and provenance does not (`v0.102.1` measured 2026-11-09) |

The generation stage lives in `release-candidate.pipeline.kts`, not in
`release.pipeline.kts`, so the attestation covers the candidate that was
certified. It is activated by the presence of a key (`RELEASE_PROVENANCE_KEY`),
which is a capability and not a policy: `RELEASE_REQUIRE_PROVENANCE` still has
exactly one reader.

### Three measurements that qualify the section above

The measurements in "Option B" were taken in an interactive terminal. Re-taken
in the conditions a lane and a test actually run, on cosign 3.1.3:

- `cosign generate-key-pair` **prompts for a password and has no TTY fallback**.
  Without `COSIGN_PASSWORD` in the environment it dies with `inappropriate
  ioctl for device`. The offline path is real; it is not the one that runs
  unattended.
- `generate-key-pair --output-key-prefix P` writes **`P.key` and `P.pub`** —
  the same prefix. `P.key.pub` is not a thing, and assuming it is produces a
  generator that refuses a perfectly good key pair.
- `cosign verify-blob` prints `Verified OK` on **stderr**, not stdout. A test
  that looks for it in stdout fails with an empty message, which reads as a
  broken assertion rather than a wrong assumption.

The third-digest claim is unchanged and re-confirmed: the bundle's
`subject[0].digest.sha256` is the hex sha256 of the artifact, and
`verify-blob` refuses a bundle presented against different bytes.

### What is deliberately still not done

- The release lane still verifies with `gh attestation verify`, which cannot
  verify a cosign bundle. Migrating it changes the behaviour of a published
  release; the custody decision it waited on is taken, so what remains is the
  work itself.
- `RELEASE_REQUIRE_PROVENANCE` is still `0`. A key now exists, which was the
  condition this ADR set, but flipping the default before the lane can verify a
  key-signed bundle would make every release refuse; the order is the one the
  Recommendation already gives.
- The agent that maintains this repository creates no keys.
  `attest-provenance.sh` contains no `cosign generate-key-pair` call, and a
  contract fails if one appears; the first key was generated by the maintainer.

## What is deliberately not decided here

- Whether to migrate later to keyless once an OIDC issuer is available. The
  bundle format does not need to change: a key-signed bundle and a Fulcio-signed
  bundle are the same DSSE envelope with different `verificationMaterial`.
- Transparency-log anchoring. `cosign attest-blob` did not require it and the
  measured path did not use it; a log entry would add a third party to the
  release's trust story and that is a separate decision.
- Retroactive provenance for v0.98.1 and earlier. It cannot be manufactured, and
  a release that predates the tooling must be described as predating it. The
  v0.103.1 signature is a bounded exception, not a precedent: it signs bytes
  that were already published and says so (see "First use").

## La version del firmante: decision tomada, con lo que cuesta

### Que cambio, y por que

La v3.1.4 de cosign —la que instalo el gestor de paquetes de la maquina— ya no
tiene `--tlog-upload`: el registro de transparencia paso a ser obligatorio, y
cada `attest-blob` pide aceptar los terminos de uso del servicio de sigstore.

MEDIDO, con y sin terminal:

- sin terminal, el prompt se resuelve como `Error: user declined the prompt` y
  el comando sale con rc=1;
- con terminal, respondiendo `y`, firma y **publica la huella en el registro
  publico de Rekor**;
- la aceptacion no se cachea: el prompt vuelve en cada llamada.

Un lane no tiene terminal. MEDIDO: un step de PipelineK arranca con
`STDIN: NO TTY` y `STDOUT: NO TTY`.

La consecuencia se midio entera: `scripts/ci/test_provenance_generation.py`
dejaba dos comprobaciones en rojo, y el `merge-gate` —que corre en la misma
maquina— no podia pasar su etapa de contratos.

### Las tres vias, medidas antes de decidir

| Via | Firmar | Verificar | Sale algo de la maquina |
|---|---|---|---|
| `--yes` | sin prompt | limpio | **si**, al registro publico de Rekor |
| `--signing-config` sin tlog | sin prompt, sin red | pide `--insecure-ignore-tlog` | no |
| version fijada (3.1.3) | sin prompt, sin red | limpio | no |

**La decision: fijar la v3.1.3.** Es la que este ADR ya habia medido, y fijarla
conserva exactamente lo que se midio: firmar offline, sin red y sin registro,
con la clave que acompana a la release. Ademas, no obliga a quien verifica a
pasar un flag que el propio cosign declara inseguro.

Publicar en Rekor sigue siendo una decision valida, y mejor para la
auditabilidad, pero es **otra** decision, y no la toma un gate: exige que el
lane firme con red contra un servicio publico, y deja la huella de cada
release en un registro que no se puede borrar.

### Lo que cuesta, dicho de frente

El lane necesita **red** para descargar la herramienta, una vez por maquina.
El binario se verifica por su suma, y esa suma esta escrita junto a la version
en `scripts/ci/cosign-tool.sh`; el instalador se niega a instalar una version
que no sea la declarada, de modo que la lane y los contratos no puedan
discrepar sobre que herramienta miden.

El generador y el contrato ya no preguntan al PATH: piden el binario fijado
por su ruta. Un `cosign` suelto en el PATH es lo que rompio el lane, asi que
volver a el seria volver al defecto.

### Lo que sigue abierto

- **La custodia de la clave.** Decidida el 2026-11-13 (ver «Custody decision»).
  Cuando se escribio este apartado era lo que mantenia el ADR en `proposed`; el
  pin del firmante no la cambiaba.
- **El anclaje al registro de transparencia.** Fijar la version conserva el
  comportamiento medido —sin anclaje—, no lo decide. Publicar en Rekor sigue
  disponible y sigue sin estar decidido.
- **El camino del consumidor.** La lane de publicacion sigue verificando con
  `gh attestation verify`, que no verifica un bundle de cosign. Migrarlo cambia
  el comportamiento de una release ya publicada; la decision de custodia de la
  que dependia esta tomada, y falta el trabajo.
