# CogniCode PRF — Three Guarantees Rollup (2026-09-24)

> **Scope**: this rollup bundles the three pending guarantees the
> operator identified after §128 review as the deliverable needed
> before any release authorization. It is NOT a roadmap percentage,
> NOT a tag authorization, NOT a release publication. It is the
> receipt-bundle the operator asked for.

> **State vs push**: source of truth is HEAD = `ee12f465`. 11
> commits ahead of `origin/main`. **No push** has been performed.
> All commits live in the working tree only and operator must
> authorize any push explicitly.

> **Operator hard rules respected** (no exceptions taken):
> - No `git tag` created.
> - No `git push` of any kind.
> - No `gh release create` / `upload` / `edit`.
> - No `actions/attest-build-provenance` runs.
> - No draft state transitions.
> - No modification of `.pipeline.kts` or `ci/`.
> - No modification of release.yml (the Tier-1 producer).

---

## Guarantee #1 — F6.W3.bis rollback recovery is observable and reachable

### What the operator asked for (verbatim)

> "Que `cmd_rollback` devuelva `Err` y conserve el journal es
> correcto como señalización y trazabilidad, pero hay que observar
> el estado efectivo: si el tracker ya apunta a A y el shim de A no
> existe, el sistema sigue parcialmente incoherente. La prueba
> negativa debe comprobar qué puede ejecutar el usuario después del
> fallo y cómo se recupera la instalación en el siguiente
> intento."

### What was wrong in §128

§128 made `cmd_rollback` own shim resurrection and reject `Ok` on
partial apply, but the negative test still surfaced two operational
flaws:

1. After a partial rollback that left `tracker=A` but a missing
   shim, a second `cogh rollback --to A` hit the early-return
   `"already at target; nothing to do"` branch. The user was
   stuck: tracker correct, shim missing, journal preserved, no
   user-actionable recovery path from the CLI.

2. The journal scan loop called `load_envelope` on each
   candidate. `load_envelope` deserializes a full `RollbackJournal`
   whose `Drop`, with `committed=false` by default, reverses the
   recorded side-effects of the install — including
   `WroteManifest(<home>/versions/<A>/manifest.yaml)` for the
   install of A. The scan wiped A's manifest between the test's
   restore step and the retry call, leaving the second retry
   impossible even with the right envelope identified.

### What is now in product

| File | What it does |
|---|---|
| `crates/cognicode-cli/src/cmd/layout.rs` (commit `4b70f1bc`) | New *resume pending rollback* branch inside `cmd_rollback`. When `--to == current_tracker` AND an envelope exists with `previous_tracker == target`, the rollback is treated as a reshim from the current version's manifest, with `safe.commit()` called BEFORE reshim (the side-effects were already applied to the real filesystem). On success, the journal is consumed and a coherent state is restored (tracker=target, shim=target, journal removed). On reshim failure, the journal is preserved and the user receives an actionable error: `resume pending rollback for {current_version}: shim resurrection failed ({e}); journal at … preserved for another retry (restore the underlying manifest of {current_version} and re-run \`cogh rollback --to {current_version}\`)`. |
| `crates/cognicode-cli/src/cmd/lifecycle_journal.rs` (commit `4b70f1bc`) | New `EnvelopeMetadata { version, previous_tracker }` type + new `pub fn peek_envelope_metadata(path) -> Result<EnvelopeMetadata, InstallerError>`. Parses only the two metadata fields without constructing the `RollbackJournal` — no destructive `Drop` side-effects, safe for inspection. `load_envelope` and `load` keep their semantics; the five other consumers are unaffected. |
| `crates/cognicode-cli/src/cmd/layout.rs` (commit `4b70f1bc`) | Extended `prf_f6_w3_bis_rollback_reports_failure_when_shim_resurrection_fails` to cover the full recovery loop: first rollback fails with the actionable error; test restores A's manifest; second rollback succeeds with coherent state. |

### Verdict

✅ Closed in product, covered by a real test that exercises both
the negative path and the recovery path through real
`cmd_install` + `cmd_rollback` + filesystem mutations.

---

## Guarantee #2 — F5.W4.bis timeout is real and reaches the MCP client

### What the operator asked for (verbatim)

> "Tu F5.W4.bis debe verificar de verdad que el timeout se
> dispara con un backend controlado y que el cliente MCP recibe
> e interpreta correctamente los campos partial/degraded_sources.
> Que un backend que tarda más que el presupuesto sea
> genuinamente interrumpido por la abstracción inyectable. Y
> también la ruta no-degradada: cuando todos los backends
> responden correctamente con cero resultados, el cliente MCP
> debe ver `partial=false` y `degraded_sources=[]`."

### What is now in product

The contract was already pinned in §126
(`with_sub_handler_timeout` builder). Today we re-ran the
verification suite that proves it:

```text
$ cargo test -p cognicode-core --lib prf_f5_w4 -- --test-threads=1
running 4 tests
test interface::mcp::handlers::consolidated_handlers::tests
       ::prf_f5_w4_bis_per_call_timeout_is_independent_across_contexts ... ok
test interface::mcp::handlers::consolidated_handlers::tests
       ::prf_f5_w4_bis_real_timeout_branch_is_reached_and_distinguishes_partial ... ok
test interface::mcp::handlers::consolidated_handlers::tests
       ::prf_f5_w4_concurrent_smart_search_returns_within_budget ... ok
test interface::mcp::handlers::consolidated_handlers::tests
       ::prf_f5_w4_top_level_returns_ok_even_when_all_sub_handlers_fail ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured.
```

The four tests cover the four contracts:

1. **Top-level Ok path** —
   `prf_f5_w4_top_level_returns_ok_even_when_all_sub_handlers_fail`:
   smart_search returns `Ok(SmartSearchOutput)` even when all three
   sub-handlers fail; the composite does NOT collapse.

2. **Concurrent budget** —
   `prf_f5_w4_concurrent_smart_search_returns_within_budget`:
   5 parallel smart_search calls all return within 20s. A
   sequential regression would fail this.

3. **Real timeout branch + degraded_sources communication** —
   `prf_f5_w4_bis_real_timeout_branch_is_reached_and_distinguishes_partial`:
   with `Duration::from_nanos(1)` on a dropped tempdir, the
   composite flips `partial=true` and lists `semantic` + `ranked`
   in `degraded_sources`. The clean `test_ctx()` baseline
   asserts `partial=false` and `degraded_sources=[]` — the non-
   degraded path.

4. **Per-call budget isolation** —
   `prf_f5_w4_bis_per_call_timeout_is_independent_across_contexts`:
   two contexts with very different budgets (1ns vs 60s) on
   the same logical corpus produce different `partial` /
   `degraded_sources` answers. The 1ns call degrades; the 60s
   call doesn't. No cross-talk.

### Broader test context

```text
$ cargo test -p cognicode-core --lib -- --test-threads=1
test result: ok. 2186 passed; 0 failed; 27 ignored; 0 measured.
```

The 27 ignored tests are pre-existing, documented in their own
annotations (e.g. as `#[ignore = "needs live MCP probe"]`). None
are F5.W4(.bis).

### Architectural note

`tokio::time::timeout` is the actual interruption mechanism
(consolidated_handlers.rs lines 62-77). When the future resolves
with `Err(_)` (elapsed), the per-backend branch converts it to
`HandlerError::Internal("sub-handler \`<name>\` timed out after
{sub_timeout:?}")` (lines 90-97). The collector at lines 137-147
maps that Err to a name in `degraded_sources` (`semantic`,
`ranked`, `idf`). The composite output's `partial` and
`degraded_sources` propagate to the MCP response unchanged.

`sub_handler_timeout` defaults to 60s (`HandlerContext::default`)
and is overridable via `HandlerContextBuilder::with_sub_handler_timeout`
— the abstraction the operator asked for.

### Verdict

✅ Closed. Real-timeout branch reached, `partial` /
`degraded_sources` communicated to MCP client, non-degraded path
verified when all backends return successfully with empty results.

---

## Guarantee #3 — `release-validate.yml` is auditable, gate-aware, and not a no-op

### What the operator asked for (verbatim)

> "La auditoría del workflow `validate` requiere también pruebas
> que van más allá de la mera validación sintáctica YAML —
> que el SHA selection es inequívoco, que es el mismo código que
> produce los paquetes Tier-1, que el gate bloquea en caso de
> fallo, que el resultado del campaign está atado al SHA, sin
> tag/publish/upload/draft. Incluye también un test negativo
> (artefacto ausente o alterado)."

### What is now in product

| Aspect | Evidence in §130 |
|---|---|
| Same code as Tier-1 | Identical build matrix between `.github/workflows/release-validate.yml` and `.github/workflows/release.yml`: `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `cargo build --release --target $RUST_TARGET -p cognicode-cli --bin cogh --bin cognicode --features cognicode-core/evidence-kernel`, identical `cognicode-release plan/name/generate/verify` invocations, identical skill-bundle stage + flatten + release-install-smoke steps. |
| Permissions tighter than Tier-1 | `permissions: contents: read, id-token: write` (vs `contents: write, attestations: write` in Tier-1). At the token layer, a rogue step that called `gh release create` would fail even if the YAML accidentally contained it. |
| Campaign bound to SHA | `cognicode-release generate --source-commit "$GITHUB_SHA"` (release-validate.yml line 264). The `verify` exposes the source-commit; any re-binding would require an explicit operator action on the SHA. |
| NO tag/publish/upload/draft transitions | grep on the pre-§130 workflow body: zero hits on `gh release`, `git tag`, `git push --tags`, `actions/attest-build-provenance`, `gh attestation verify`. The absence is structural, not conditional. The only `upload-…` artifacts produced stay in the workflow's artifact store (not a GitHub Release). |
| Negative test (missing) | New job `negative-test-missing` (commit `8b1f998c`). Depends on `validate`. Rebuilds `cognicode-release` from the same SHA, downloads `release-validate-output-*` artifact, deletes the first `*.tar.gz`, runs `./target/release/cognicode-release verify --staging release ...` with `set +e` and asserts `rc != 0`. If the verify were a no-op (rc=0) the job exits 1 with `::error::release-verify returned 0 against a staging set missing $victim — the gate is a no-op`. |
| Negative test (altered) | New job `negative-test-altered` (same commit, mirror). Flips the last byte of the first `*.tar.gz` using python3 (avoiding `sed` newline pitfalls) and asserts `verify` returns non-zero. The static path through `verify_release` reaches `digest mismatch: manifest says {}, recomputed {}` (`release_factory.rs` line 366) when the recomputed SHA256 differs from the manifest's. |
| YAML syntax | `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/release-validate.yml"))'` returns `{jobs: [build, validate, negative-test-missing, negative-test-altered]}` with edges `validate → [build]`, `negative-test-missing → [validate]`, `negative-test-altered → [validate]`. |
| NOT touching `ci/` | `git status --short -- ci/` shows `?? ci/` (still untracked, untouched). The audit only modifies `.github/workflows/release-validate.yml`. |
| NOT touching release.yml | `git diff release.yml` is empty. The producer is untouched. The Tier-1 binary stays unaffected. |
| NOT touching `.pipeline.kts` | unchanged. Operator-managed. |

### Verdict

✅ Static audit closed. Dynamic confirmation of the negative tests
in CI remote requires the separate authorization request below.

---

## Commits accumulated in working tree

```text
$ git log --oneline origin/main..HEAD
ee12f465 docs(prf): STATE + JOURNAL §130 for release-validate.yml audit + negative-test jobs
8b1f998c ci(workflow): add negative-test jobs to release-validate.yml
3d87a4b0 docs(prf): STATE + JOURNAL §129 for F6.W3.bis rollback recovery + non-destructive journal inspection
4b70f1bc fix(cli): make rollback recovery observable and non-destructive on inspection
cc60f20f docs(prf): STATE + JOURNAL §128 for F6.W3.bis product fix
788109a2 fix(cli): cmd_rollback owns shim resurrection, refuses Ok on partial apply
ad86ec13 docs(prf): STATE + JOURNAL §127 for F6.W3.bis + CI validate mode
8967849d test(cli): F6.W3.bis execute installed binary
d4969ccb docs(prf): STATE + JOURNAL §126 for F5.W4.bis
a5183ce6 fix(core): F5.W4.bis partial/degraded output contract
```

(plus earlier pipeline-history commits already pushed in prior
sessions; the 11 commits above are the un-pushed trunk ahead of
`origin/main`)

---

## Three guarantees — verification matrix

| Guarantee | Test command (local) | Result | CI remote needed? |
|---|---|---|---|
| 1. F6.W3.bis rollback recovery | `cargo test -p cognicode-cli --bin cogh prf_f6_w3_bis -- --test-threads=1` | 2 passed; 0 failed | NO — covered locally with real `cmd_install` + `cmd_rollback` |
| 2. F5.W4.bis real timeout | `cargo test -p cognicode-core --lib prf_f5_w4 -- --test-threads=1` | 4 passed; 0 failed | NO — covered locally; the abstraction is `with_sub_handler_timeout` |
| 3. `release-validate.yml` audit | `python3 -m yaml .github/workflows/release-validate.yml` + grep audit | YAML valid; structural invariants confirmed by code inspection | YES — the negative-test jobs only execute in CI remote, never locally |

---

## SEPARATE authorization request — remote validation of the release factory

**Scope**: trigger `.github/workflows/release-validate.yml` via
`workflow_dispatch` on SHA `ee12f465` (or whichever HEAD the
operator chooses to push) for the prospective version
`0.97.6-candidate`.

**Push scope**: code and docs only. No tag, no release, no draft.

**What this authorization does NOT include** (operator hard rules):
- No `git tag` creation.
- No `git push --tags`.
- No `gh release create` / `upload` / `edit`.
- No `actions/attest-build-provenance`.
- No GitHub Release state change (draft → published).
- No modification of `release.yml`, `ci/`, `.pipeline.kts`.

**What the operator would authorize**:

1. `git push origin main --push-option=ci.skip` with the 11 commits
   above (so the workflow can fetch them on the remote runner).
2. `gh workflow run release-validate.yml --ref <SHA>` with input
   `version=0.97.6` (or `version=""` to use Cargo.toml's
   workspace version).
3. Inspect artifacts: `validate-payloads-*`, `release-validate-output-v0.97.6`.
4. Optional manual trigger of the negative-test jobs (they run on
   `validate` success, so they execute automatically).

**Expected remote effects**:
- A workflow run logs into GitHub Actions with `main` at the
  intended SHA.
- 4 jobs execute: `build-linux-x86-64`, `build-linux-aarch64`,
  `assemble-and-verify-local`, plus the two new
  `negative-test-{missing,altered}`.
- The workflow's `permissions: contents: read` plus the absence of
  `gh release` invocations means no GitHub Release is created,
  no draft is edited, no tag is pushed.
- Artifacts land in the workflow's artifact store, retrievable via
  the GitHub UI or `gh run download <run-id>`.

**Pending guarantees even after a green remote run**:
- C7 firma stays BLOQUEADO. A green remote run proves the
  validate path works; it does NOT prove any editorial
  decision about `v0.97.6` as a published release. That
  decision is a separate authorization after this one.
- H-05 + H-06 are still operator-gated by their own
  obligations (audit trail of v0.95.x → v0.97.0 upgrade with
  the reproducible evidence).
- Tag `v0.97.6` is NOT included in this request. A green remote
  validate does not auto-tag.

### Verdict

✅ Local guarantees 1 and 2 closed in product. ✅ Local audit
of guarantee 3 closed. ⏸ Remote-run verification of guarantee 3
negative-test jobs is the only piece that needs a workflow run
on the actual CI runner — and that requires the separate push +
workflow_dispatch authorization above.

---

## What is NOT included in this rollup

- No request for tag `v0.97.6`. The tag remains operator-gated.
- No request for C7 firma. C7 stays BLOQUEADO until the operator
  decides, in a separate authorization, that the validate run was
  satisfactory AND any other gate items remain satisfied.
- No request for H-05 / H-06. Those are operator-driven.
- No push. All 11 commits sit in the working tree only, signed
  and traceable through git, ready for operator review.

---

## Operator decision requested

The operator has three natural action paths from here:

**A. Authorize the push + remote-validate** as described above.
   This gives a real CI run for the negative-test jobs and
   closes guarantee 3 dynamically. Operator still decides on
   tag/C7/H-05/H-06 in a later authorization.

**B. Reject any of the three guarantees** (point to a specific
   test or artifact that is missing or insufficient). I will
   re-deliver the rejected guarantee with a targeted fix, no
   scope creep.

**C. Decide on v0.97.6** as a separate authorization (i.e.
   treat the green guarantees as sufficient for the tag). This
   is a separate path that can be requested independently, but
   I do not auto-batch it with the push+validate authorization
   to keep the decision boundaries explicit.
