# e74 — Closure & Strategic STOP Authorization

> **Strategic verdict:** ACCEPTED / GREEN. Implementation sufficient
> to advance. Architecture, contract, and tooling are stable; UAT
> evidence on Linux x86_64 is observed; the other four platforms
> carry typed waivers whose replay path is well-defined.
>
> **STOP class:** e74 cycle ends here. Strategic STOP after e76.

## 1. The 4-stop wording (verbatim)

```text
e74 implementation        CLOSED
e74 contract              SATISFIED with typed waivers
Linux x86_64 UAT          observed
other platform UATs       explicitly waived/pending
cross-platform equivalence NOT YET proven
```

**Important reading.** "Windows/macOS/Linux equivalence has not been
demonstrated" is the honest framing. e74 produces the *machinery*; e76
must produce the *evidence* on the four non-Linux lanes. Do not let
the five-lane release matrix or the linux-x86-64 UAT success trick a
reader into thinking equivalence exists.

## 2. Branch and commits

| Item                | Value                                       |
|---------------------|---------------------------------------------|
| Branch              | `feat/e74-lsi-portable-runtime-distribution` |
| Remote              | `origin/feat/e74-lsi-portable-runtime-distribution` (pushed) |
| Base                | local `main` @ `7df55d49` (e65 final)        |
| Commits added       | 12 (1 cherry-pick + WU0..WU6 + 4 fixes/docs)|
| Status vs main      | 47 ahead, 0 behind                          |
| Tag                 | none                                        |
| PR                  | not opened (deferred)                       |
| Merge to main       | NO                                          |

Push was a durability checkpoint. The branch lives on the remote so
work is not lost; it does NOT promote to main. Promotion awaits the
rest of the LSI block (e75/e76) plus a single explicit PR at the end.

## 3. What was implemented (high level)

- **WU0** — `cogh` rebuild + `bundles/v0.95.0/bundle.yaml` + feature-gate
  audit on `application::local_ci`, `application::evidence_bundle`,
  `application::policy_gate`.
- **WU1** — `PlatformAdapter` trait + `LinuxAdapter` / `MacOsAdapter` /
  `WindowsAdapter` + `PlatformFamily` enum + `PlatformReport`. Replaces
  scattered `#[cfg(unix)] / #[cfg(not(unix))]` blocks in
  `installer_transaction.rs` and `ide.rs`.
- **WU2** — `BundleManifest::assert_host_platform`. Wrong-platform
  bundles rejected loudly before any install work. No silent fallback.
- **WU3** — Native release matrix (5 lanes, no cross-compilation) +
  install-smoke job extended to cover all 3 binaries (cogh,
  cognicode-mcp, explorer-api).
- **WU4** — `cogh doctor` four-dimensional report
  (`CheckStatus::{Pass, Warn, Fail, Unavailable}`) + capability discovery
  surface that finds isolation backends but does NOT enable them.
- **WU5** — Packaging/spike: rejected `cargo-dist` adoption. Decision
  recorded in `docs/adr/ADR-052-reject-cargo-dist-e74.md` (ephemeral
  local, NOT pushed).
- **WU6** — Cross-platform acceptance UAT plan + structured evidence
  script (`scripts/e74-acceptance-evidence.sh`).
- **WU4-followup** — `probe_core_health` extended to surface
  `tracker/version` as a Warn; 5 pre-existing integration tests updated
  to the four-dimension doctor contract.
- **release.yml followup** — `BIN_LIST` bug fix (cognicode-runtime is
  a package, not a binary); install-smoke extended from 1 → 3 binaries.

## 4. Typed waivers (retained for e75/e76/e77+)

These waivers are the honest frame for "the contract permits, but the
evidence is pending." Each waiver has a reason and a revisit trigger.

| Waiver ID                           | Reason                                                | Revisit trigger                                    |
|-------------------------------------|-------------------------------------------------------|----------------------------------------------------|
| `release-not-signed-yet`            | GPG signing requires secrets not in repo             | e75/e76 acquires + commits GPG secrets             |
| `release-no-pkg-yet`                | macOS `.pkg` not in scope                             | e75 adds Apple ID + dev cert                       |
| `release-no-msi-yet`                | Windows `.msi` not in scope                           | e76 adds Authenticode cert + Azure Trusted Signing  |
| `release-no-notarization-yet`       | Apple notarization not in scope                       | e76 acquires Apple notarization API key            |
| `release-no-codesign-yet`           | Windows code signing not in scope                     | e76 acquires codesign cert                         |
| `e74-uat-pending-linux-aarch64`     | Cross-compile install works; UAT A/B/C/D not on runner | aarch64 runner attaches JSONL with all 4 = pass    |
| `e74-uat-pending-macos-x86-64`      | Runner only                                            | macOS runner attaches JSONL with all 4 = pass      |
| `e74-uat-pending-macos-aarch64`     | Runner only                                            | macOS arm64 runner attaches JSONL with all 4 = pass|
| `e74-uat-pending-windows-x86-64`    | Runner only                                            | Windows runner attaches JSONL with all 4 = pass    |

The list is the contract for e76 lifting; reviewing the UAT directory
(`evidence/e74-acceptance/`) and attaching the missing JSONL files is
the e76 acceptance path.

## 5. Visible debt carried forward (debt ledger)

These items are NOT e74 work but were surfaced during e74. The user's
strategic note is that **debt must become visible**, not forgotten.

### 5.1 `cogh install --home PATH` silently ignored

**Surface.** The `--home` global flag is parsed by clap and passed to
`CognicodeHome::resolve(cli.home.as_deref())`, but
`InstallerTransaction::run(profile)` ignores `home` and writes the
manifest via `layout::install_manifest_path(...)`, which calls
`cognicode_home()` directly (reading `COGNICODE_HOME` env var or
falling back to `~/.cognicode`).

**Repro.** `cogh install mcp-server --home /tmp/foo --version 0.95.0`
writes the manifest to `~/.cognicode/install/0.95.0/manifest.yaml`
(`/home/rubentxu/.cognicode` on this box), not `/tmp/foo`.

**Revisit trigger.** Promoted to e75 scope automatically if e76 uses
isolated installations with temporary homes where the silent ignore
would damage reproducibility.

**Lift path.** Thread `home: &CognicodeHome` through
`InstallerTransaction::run(...)` → `layout::install_manifest_path(&home, &version)`.
Replace the free-function `layout::install_manifest_path` with a
method on `CognicodeHome` so the home becomes the authoritative
argument.

**Severity today.** Low — the lifecycle tests use `COGNICODE_HOME`
explicitly and the docs/support scripts use the same. No real user
path triggers it. Still a debt.

### 5.2 M9 `RequestedBy::LlmAgent / Plugin` is unrestricted

**Surface.** M9 (SoftwareWorld lineage) does not gate which
`RequestedBy` is permitted to produce `ChangeProposal`. The
`Plugin` and `LlmAgent` variants exist but anything calling
`submit_proposal(requested_by: RequestedBy)` works.

**Why now.** e75 will surface this if I implement an isolated-execution
oracle under `TrialExecutor` and need to scope who may launch a
trial.

**Revisit trigger.** Pre-M11 — before the Fix Agent is permitted to
generate real `ChangeProposal`s.

**Lift path.** Add a `LlmAgent/Plugin gate` doc + tests in
`cognicode-core/src/domain/change_tracking/proposal/` that asserts
current contracts and (in M11) restricts `ChangeProposal` submission
to the developer / supply chain path; reject `LlmAgent` and
`Plugin`-driven proposals except via the human-mediated review
boundary defined in the M9 lineage spec.

**Severity today.** M10/M11 contention surface; not blocking e75 work.

## 6. SDDK / openspec posture

Per AGENTS.md, ephemeral docs (`docs/adr/**`, `docs/ROADMAP.md`,
`CONTEXT.md`) are local-only. Specifically:

- `docs/adr/ADR-052-reject-cargo-dist-e74.md` is in the working tree
  but gitignored. NOT staged, NOT committed, NOT pushed.
- `openspec/changes/e74-.../wu5-packaging-spike.md` carries the
  e74-visible cargo-dist rationale in the **change** folder
  (which IS remote) for future reviewers, separate from the
  ephemeral ADR.

The open change directory `openspec/changes/e74-lsi-portable-runtime-distribution/`
contains:
- `proposal.md` (initial intent)
- `tasks.md` (initial WU decomposition)
- `wu5-packaging-spike.md` (cargo-dist evaluation — public on remote)
- `wu6-cross-platform-uat.md` (acceptance contract)
- `prt-requirement-to-check-traceability.md` (PRT-001..008 mapping)
- `final-acceptance-ledger.md` (public-interface observations)
- `closure-authorization.md` (this file)

The full evidence ledger lives under `evidence/e74-acceptance/`
(2 JSONL + 4 .txt files) and is committed on the branch.

## 7. Authority surface preserved

The e74 implementation did NOT alter:

- The bundle manifest format (5 `Platform` variants, kebab-case
  `Display`, no schema migration).
- The doctor contract (4 dimensions; `CheckStatus` enum stable).
- The installer transaction state machine (`InstallerTransaction::run`
  signature unchanged; `assert_host_platform` added alongside, not
  replacing).
- Authority semantics (`ChangeProposal → SoftwareWorld →
  TrialExecutor → ExecutionBackend → WorkResult → EvidenceBundle →
  PolicyGate → PromotionEvaluation → PromotionPermit`).

Anything that looked like an authority change was REFUSED in WU5
(`cargo-dist` rejected) and recorded in WU2 (assertion added, not
removed).

## 8. STOP authorization — what happens next

Per the user's strategic note:

1. e74 branch lives on `origin/feat/e74-lsi-portable-runtime-distribution`
   as a durability checkpoint.
2. **Do NOT** push ADR-052, **do NOT** tag, **do NOT** open PR.
3. e76 self-hosting scoring is out of e74 scope.
4. M9 authority semantics are preserved unchanged.
5. **Strategic STOP after e76** — e74's STOP authorization is
   satisfied; e75 will run with same WU cadence and STOP under
   the conditions in the e75 directive (fail-open, dep
   uncertainty, third backend, ownership ambiguity, etc.).

## 9. Receipt

- Branched from: local main @ `7df55d49`
- Closed: `feat/e74-lsi-portable-runtime-distribution` @ `09f83c97`
- Pushed: `origin/feat/e74-lsi-portable-runtime-distribution`
- Next: `feat/e75-lsi-portable-execution` from `09f83c97`
- Strand: portable runtime is GREEN; portable execution is the
  next block before self-hosting validation can begin honestly.
