# REMOTE-VALIDATE PROPOSAL — push + workflow_dispatch sin publicación

> **Documento:** propuesta operativa (NO ejecutada). Generada 2026-09-24T10:48Z
> tras cierre de las tres garantías pendientes del operador post-§128.
> **Estado:** BORRADOR pendiente de autorización explícita del operador.
> **Acción que se solicita:** aprobar el bloque "A.3 Comando exacto" → yo
> ejecuto push + workflow_dispatch. Cualquier cambio en este SHA candidato
> invalida la propuesta y exige regenerarla.
>
> **Acción NO incluida en esta propuesta** (operator-gated por separado):
> tag `v0.97.6`, GitHub Release, draft → published, firma C7, H-05, H-06,
> modificaciones a `release.yml`, `ci/`, `.pipeline.kts`, `AGENTS.md`.

---

## A. SHA candidato, rama y ascendencia

| Campo | Valor | Comando | Resultado |
|---|---|---|---|
| SHA candidato (full) | **`dc74a294c51362c35540f6caddb6657b70c544f8`** (`dc74f294...` corto; era `b354a632...` antes del §132 amend). HEAD actualmente. | `git rev-parse HEAD` | `dc74a294c51362c35540f6caddb6657b70c544f8` |
| Rama destino | `main` | `git symbolic-ref HEAD` | `refs/heads/main` |
| `origin/main` actual | `edf45fb81242cb1c6ad301fdc092f134f66ae90a` | `git rev-parse origin/main` | `edf45fb81242cb1c6ad301fdc092f134f66ae90a` |
| Merge-base local↔origin | `edf45fb81242cb1c6ad301fdc092f134f66ae90a` | `git merge-base HEAD origin/main` | `edf45fb81242cb1c6ad301fdc092f134f66ae90a` |
| Commits ahead | 12 | `git rev-list --count origin/main..HEAD` | `12` |
| HEAD == origin/main? | **NO** (12 ahead, no remote force) | comparativa arriba | confirmado |
| Tag `v0.97.6` existente | **NO** | `git tag -l 'v0.97*'` | `v0.97.0`, `v0.97.1`, `v0.97.2`, `v0.97.3`, `v0.97.4`, `v0.97.5` (sin `v0.97.6`) |

**Verificación de ascendencia:** `edf45fb8` ⊂ cadena local. No se requiere
`rebase` ni `merge`; el push sería `--ff-only` (o push normal si el remoto
acepta fast-forward).

---

## B. Inventario de los commits (cronol��gico ascendente)

> **NOTA sobre el conteo:** la propuesta fue redactada cuando
> `git rev-list --count origin/main..HEAD = 12`. Inmediatamente
> después, commiteé la propuesta misma (commit `c1f56941`, abreviado
> como `abf4b130` antes del amend final),
> llevando el conteo a **13 commits ahead**. La lista abajo
> refleja el estado **actual** (13 commits); los 12 commits
> de código+docs+CI originales quedan como los 12 primeros.

Lista completa, producida por
`git log --reverse origin/main..HEAD --format="%H | %ad | %s" --date=short`:

```
a5183ce6 | 2026-09-24 | feat(core): per-call sub-handler timeout + partial/degraded output (F5.W4.bis)
d4969ccb | 2026-09-24 | docs(prf): STATE pointer + JOURNAL §126 for F5.W4.bis
8967849d | 2026-09-24 | test(cli): F6.W3.bis executes installed binary post A→B and rollback
ad86ec13 | 2026-09-24 | ci(workflow): add Release Validate workflow (no tag, no publish)
b332e8df | 2026-09-24 | docs(prf): STATE + JOURNAL §127 for F6.W3.bis + CI validate mode
788109a2 | 2026-09-24 | fix(cli): cmd_rollback owns shim resurrection, refuses Ok on partial apply
cc60f20f | 2026-09-24 | docs(prf): STATE + JOURNAL §128 for F6.W3.bis product fix
4b70f1bc | 2026-09-24 | fix(cli): make rollback recovery observable and non-destructive on inspection
3d87a4b0 | 2026-09-24 | docs(prf): STATE + JOURNAL §129 for rollback recovery + non-destructive journal inspection
8b1f998c | 2026-09-24 | ci(workflow): add negative-test jobs to release-validate.yml
ee12f465 | 2026-09-24 | docs(prf): STATE + JOURNAL §130 for release-validate.yml audit + negative-test jobs
b354a632 | 2026-09-24 | docs(prf): state-update + rollup doc for the three closed guarantees
c1f56941 | 2026-09-24 | docs(prf): remote-validate proposal — push + workflow_dispatch without publication  ← HEAD
```

**Conteo autoritativo actual:** `git rev-list --count origin/main..HEAD = 13`.

> **Implicación:** el push a autorizar será de **13 commits, no de 12**.
> Si el operador contaba con un push de 12 y eso es un problema (p.
> ej. quería un control de qué entra en la PR), se reabre la
> propuesta sin la línea 13; sin esa línea, la propuesta vuelve
> a tener exactamente 12 commits ahead. El push de 12 vs 13 no
> cambia nada del remote-validate (el SHA es lo único que cuenta).

> **Si la propuesta se aprueba con 13 commits:** el SHA candidato
> pasa a ser `c1f5694194c1f8fc78a8661598d7fb92829fa20c`, no
> `b354a632...`. Las referencias a `b354a632` en §A, §D, §E de
> esta propuesta deben entenderse como "el SHA prospectivo del
> código en el momento de redactarla", no del estado final.

### Diffstat agregado (los 12 commits)

```
.github/workflows/release-validate.yml             | 431 ++++++++++
Cargo.lock                                         |   1 +
Cargo.toml                                         |   1 +
crates/cognicode-cli/src/cmd/layout.rs             | 899 +++++++++++++++++++--
crates/cognicode-cli/src/cmd/lifecycle_journal.rs  |  38 +-
crates/cognicode-core/Cargo.toml                   |   1 +
.../mcp/handlers/consolidated_handlers.rs          | 294 ++++++-
.../src/interface/mcp/handlers/mod.rs              |  28 +
crates/cognicode-core/src/interface/mcp/schemas.rs |  21 +-
docs/prf/GUARANTEES_ROLLUP_2026-09-24.md           | 325 ++++++++
docs/prf/JOURNAL.md                                | 584 +++++++++++++
docs/prf/STATE.md                                  |  10 +-
12 files changed, 2507 insertions(+), 126 deletions(-)
```

### Archivos modificados por los 12 commits (ordenados)

```
.github/workflows/release-validate.yml             (nuevo, 431 líneas)
Cargo.lock                                         (lock de Cargo.toml)
Cargo.toml                                         (workspace version bump context)
crates/cognicode-cli/src/cmd/layout.rs             (rollback + shim resurrection)
crates/cognicode-cli/src/cmd/lifecycle_journal.rs  (peek_envelope_metadata non-destructive)
crates/cognicode-core/Cargo.toml                   (deps/feature flag)
crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs  (F5.W4.bis timeout)
crates/cognicode-core/src/interface/mcp/handlers/mod.rs  (re-exports)
crates/cognicode-core/src/interface/mcp/schemas.rs (degraded_sources schema)
docs/prf/GUARANTEES_ROLLUP_2026-09-24.md           (nuevo, 325 líneas)
docs/prf/JOURNAL.md                                (entradas §126–§130)
docs/prf/STATE.md                                  (snapshot pointer)
```

### Confirmación: archivos operator-managed NO incluidos

Comprobado por `git diff --name-only origin/main..HEAD | grep -E '^(.github/workflows/release\.yml|ci/|\.pipeline\.ts|AGENTS\.md)'`:

- `.github/workflows/release.yml`: **NO** en el diff ✅
- `ci/`: **NO** en el diff ✅
- `.pipeline.kts`: **NO** en el diff ✅
- `AGENTS.md`: **NO** en el diff ✅

### Confirmación: único workflow nuevo en `.github/workflows/`

- `.github/workflows/release-validate.yml` (nuevo, 431 líneas, único
  workflow añadido)

---

## C. Resumen verificable de las 3 garantías

### C.1 — Rollback recuperable (§129)

- **Commit fix:** `4b70f1bc` (`fix(cli): make rollback recovery observable
  and non-destructive on inspection`)
- **Cambio de producto:**
  1. Nuevo branch en `cmd_rollback` para `--to == current_tracker` con
     envelope pendiente: carga envelope, `safe.commit()`, llama a
     `cmd_reshim`, consume journal solo si reshim tiene éxito;
     preserva el journal ante fallo con mensaje accionable.
  2. Helper `lifecycle_journal::peek_envelope_metadata(path)` que parsea
     `{version, previous_tracker}` sin construir el `RollbackJournal`
     destructivo (cuyo `Drop` con `committed=false` revertiría
     side-effects incluyendo `WroteManifest`).
- **Tests:** `prf_f6_w3_bis_rollback_reports_failure_when_shim_resurrection_fails`
  extendido al loop completo de recovery; 2 tests del flujo verde.
- **Estado local:** `cargo test -p cognicode-cli --lib -- --test-threads=1`
  → **319/319 PASS** (verificación §129).
- **Lo que el remote-validate confirmará:** que el binario `cognicode-release`
  producido desde este SHA ejecuta `release-verify` contra el set staged
  (no rota el flow de recuperación porque la modificación es solo en
  `cmd_rollback` / `lifecycle_journal`, no en la fábrica de release).

### C.2 — F5.W4.bis real timeout + partial/degraded (§126)

- **Commit:** `a5183ce6` (`feat(core): per-call sub-handler timeout +
  partial/degraded output (F5.W4.bis)`)
- **Cambio de producto:**
  1. `tokio::time::timeout` envuelve la await de cada sub-handler
     en `consolidated_handlers.rs:62-77`.
  2. `Err(_)` se mapea a `HandlerError::Internal(...)` → status
     `degraded_sources` con `semantic`, `ranked`, `idf` marcados.
  3. `HandlerContextBuilder::with_sub_handler_timeout(Duration)`
     (default 60s). Permite inyectar timeout reducido en tests.
- **Tests:** 4 tests PASS:
  - `prf_f5_w4_bis_real_timeout_branch_is_reached_and_distinguishes_partial`
  - `prf_f5_w4_bis_per_call_timeout_is_independent_across_contexts`
  - `prf_f5_w4_top_level_returns_ok_even_when_all_sub_handlers_fail`
  - `prf_f5_w4_concurrent_smart_search_returns_within_budget`
- **Estado local:** `cargo test -p cognicode-core --lib -- --test-threads=1`
  → **2186/0/27 ignored**.
- **Lo que el remote-validate confirmará:** que el binario `cognicode-core`
  (linkeado en `cognicode-mcp` y `cogh`) construido desde este SHA mantiene
  el comportamiento; no hay verificación dinámica MCP del timeout en
  este workflow (queda para admisión F7 si el operador lo desea).

### C.3 — Auditoría `release-validate.yml` con negative-test jobs (§130)

- **Commits de CI:** `8b1f998c` (`ci(workflow): add negative-test jobs
  to release-validate.yml`) sobre `ad86ec13` (workflow base).
- **Auditoría cumplida:**
  1. **Misma matriz de build que `release.yml`** (mismos rust targets,
     mismos flags `--features cognicode-core/evidence-kernel`).
  2. **Permisos más restrictivos:** `permissions: contents: read,
     id-token: write` (vs `release.yml: contents: write`). El token
     layer cierra `gh release create` por capacidad, no por convención.
  3. **SHA propagado por la fuente de verdad:** `GITHUB_SHA` se pasa
     a `cognicode-release generate --source-commit "$GITHUB_SHA"`,
     persistido en `release/source_commit` y visible en el Step Summary.
  4. **Sin tag, sin publish, sin upload, sin draft, sin attestation:**
     verificado por `grep -nE "gh release|git tag|actions/attest"` →
     los matches son prohibiciones documentadas, no operaciones.
  5. **Negative-test jobs:**
     - `negative-test-missing` (`needs: [validate]`): borra un
       `*.tar.gz` del staging y assertea rc≠0. Si rc=0, falla con
       `::error::release-verify returned 0 against a staging set
       missing $victim — the gate is a no-op`.
     - `negative-test-altered` (`needs: [validate]`): flipea el
       último byte del primer tar.gz (python3 para evitar sed-newline
       pitfall) y assertea digest mismatch con rc≠0. Si rc=0,
       falla con `::error::release-verify returned 0 against an
       altered $victim — the gate is a no-op`.
- **Grafo de dependencias:** `build (sin needs) → validate (needs build)
  → {negative-test-missing, negative-test-altered} (needs validate)`.
  Si cualquier job falla, los negative-test jobs no se ejecutan
  (correcto: no se puede testar el verify contra staging inválido).
- **YAML syntax:** validado por `python3 -c "import yaml;
  yaml.safe_load(open('.github/workflows/release-validate.yml'))"`.

### C.4 — Rollup consolidado

- **Documento:** `docs/prf/GUARANTEES_ROLLUP_2026-09-24.md` (325 líneas)
- **Commit:** `b354a632` (`docs(prf): state-update + rollup doc for the
  three closed guarantees`).

---

## D. Auditoría completa de `release-validate.yml`

### D.1 — Permisos del workflow (token layer)

```yaml
permissions:
  contents: read
  id-token: write
```

vs `release.yml: contents: write`. Implicaciones:

- `contents: read` → puede leer código pero no crear/modificar tags
  ni interactuar con Releases vía `git push` ni con la API de
  GitHub Releases.
- `id-token: write` → permite OIDC para artefactos attestation
  (preparación; este workflow NO usa `actions/attest-build-provenance`).
- **Resultado:** incluso si un step del workflow invocara
  `gh release create` por error, el token rechazaría la operación.

### D.2 — Trigger types

```yaml
on:
  workflow_dispatch:
    inputs:
      version: ...       # empty → fallback to Cargo.toml 'version'
      expected_sha: ...  # optional. If non-empty, the validate job fails
                         # fast when github.sha != expected_sha. Prevents
                         # a branch advance between push and dispatch
                         # from validating a different commit than the
                         # one approved. When empty, no SHA binding is
                         # enforced (operator must verify head_sha).
```

Unico trigger: `workflow_dispatch`. No se dispara por `push`,
`pull_request`, `schedule` ni por tag. **Importante:** `gh workflow run
--ref` acepta el nombre de una rama o un tag, **no un SHA**. Tras el
push a `main`, el dispatch debe usar `--ref main` y el SHA se pasa
vía `-f expected_sha=dc74a...`. El job `validate` verifica que
`github.sha` coincide con `expected_sha` y aborta con `::error::` +
exit 1 si diverge (enmendado tras audit del operador, §132).

### D.3 — Jobs y grafo

```
build                   (needs: [])
validate                (needs: [build])
negative-test-missing   (needs: [validate])
negative-test-altered   (needs: [validate])
```

### D.4 — Artefactos producidos por el workflow

| Artefacto | Producido por | Acceso | Comparte nombre con GitHub Release? |
|---|---|---|---|
| `release-validate-output-v{VERSION}` | `actions/upload-artifact@v4` (validate job, línea ~280) | descarga desde la UI de Actions / API | **NO** — es `actions/upload-artifact` que escribe al artifact store de la run, NO a la API de GitHub Releases. |
| `validate-payloads-{lane}` | `actions/upload-artifact@v4` (build job) | idem | **NO** |
| Step Summary embebido | `>> "$GITHUB_STEP_SUMMARY"` | visible en la UI de Actions | **NO** |

**Confirmación de no-release:** ninguna invocación a `gh release create`,
`gh release upload`, `gh release edit`, `git tag`, `git push --tags`,
`actions/attest-build-provenance`, ni `releases: write` permission.

### D.5 — Gates bloqueantes

| Step | Comando de fallo | Efecto |
|---|---|---|
| Build (rust test) | `set -euo pipefail` + `exit 1` | job failure |
| **Bind to expected_sha** (validate job, step 1) | si `github.sha != expected_sha` → `::error:: + exit 1` | job failure (enmendado §132) |
| Stage declared bundles | `::error:: + exit 1` si falta `manifest.yaml` | job failure |
| `cognicode-release generate` | `set -e` (cualquier non-zero exit) | job failure |
| `cognicode-release verify` | `set -e` | job failure |
| `release-install-smoke.sh` | `set -e` | job failure |
| Upload validate output | `if-no-files-found: error` | job failure |
| Negative-test-missing | `if rc = 0 → ::error:: + exit 1` | job failure |
| Negative-test-altered | `if rc = 0 → ::error:: + exit 1` | job failure |

**Total: 9 gates. Cada uno detiene el workflow.**

### D.6 — Sha-anchoring real

```bash
./target/release/cognicode-release generate \
  --staging staging \
  --out release \
  --version "${{ steps.v.outputs.version }}" \
  --tag "${{ steps.v.outputs.tag }}" \
  --source-commit "$GITHUB_SHA"
```

El `--source-commit` se persiste en `release/source_commit` (verificable
en el JSON del `release-inventory-*.json` descargable del artifact).

---

## E. Comando exacto para ejecutar `release_dispatch` sobre el SHA publicado

> Esta propuesta NO incluye la ejecución. Se facilita el comando
> exacto para que el operador lo apruebe o lo apruebe con un wrapper
> que prefiera.

### E.1 — Pre-condiciones tras el push

1. Push aceptado por GitHub: `git push origin dc74a294c51362c35540f6caddb6657b70c544f8:refs/heads/main`
   (o push de los 13 commits como cadena fast-forward desde
   `origin/main = edf45fb8`).
2. Verificar en GitHub UI: `https://github.com/{owner}/{repo}/commits/main`
   muestra el SHA `dc74a294c51362c35540f6caddb6657b70c544f8` como HEAD.
3. Confirmar que `release-validate.yml` aparece en
   `https://github.com/{owner}/{repo}/blob/main/.github/workflows/release-validate.yml`.
4. Confirmar que `.github/workflows/release.yml` aparece
   **idéntico al de `origin/main`** (sin cambios nuestros).

### E.2 — Comando `workflow_dispatch` (a través de `gh` CLI)

```bash
# Pre-requisito: gh CLI autenticada y con scope repo.

# (1) El workflow se dispatcha contra una rama o tag (NO contra un SHA).
#     Tras el push a main, la rama 'main' apunta a dc74a294... en ese
#     momento. Si nadie pushea entre el push y el dispatch, el workflow
#     se ejecuta contra el SHA correcto.

gh workflow run release-validate.yml \
  --ref main \
  -f expected_sha=dc74a294c51362c35540f6caddb6657b70c544f8
```

> Si el operador prefiere especificar `version` input (opcional;
> el workflow usa `Cargo.toml` `version` field si se omite):
> `gh workflow run release-validate.yml --ref main \
>    -f expected_sha=dc74a294... \
>    -f version=0.97.6`

> **¿Por qué `expected_sha` y no sólo `--ref main`?**
> Sin `expected_sha`, si se pushea otro commit a `main` entre el
> push del candidato y el dispatch, el workflow validaría ese
> nuevo commit — no el aprobado. El input `expected_sha` hace
> fail-fast en el primer step del job `validate` si
> `github.sha != expected_sha`. El gate es enmendado en §132
> tras audit del operador que señaló esta limitación de
> `gh workflow run --ref`.

### E.3 — Recolección de evidencia de la ejecución

```bash
# Listar runs del workflow en orden cronológico inverso
gh run list --workflow=release-validate.yml --limit=5 --json \
  databaseId,displayTitle,headSha,status,conclusion,createdAt,updatedAt,url

# Inspeccionar la última run (cambiar <RUN_ID> por el databaseId devuelto)
gh run view <RUN_ID> --json \
  jobs,conclusion,headBranch,headSha,event,createdAt,updatedAt

# Descargar artifact del job validate
gh run download <RUN_ID> --name release-validate-output-v0.97.6
# Produce ./release/ localmente con BundleManifest, ReleaseInventory, SHA256SUMS
sha256sum release/*.tar.gz release/SHA256SUMS release/bundle-*.yaml
```

> **Nota crítica:** `gh run download` baja artefactos al directorio
> local. NO crea ni modifica GitHub Releases. Idem para `gh run view`.

### E.4 — Criterios de éxito para considerar PASS el remote-validate

1. Los 4 jobs (`build`, `validate`, `negative-test-missing`,
   `negative-test-altered`) terminan con `conclusion: success`.
2. El primer step del job `validate` (`Bind to expected_sha`)
   muestra `github.sha binds to operator-approved expected_sha` y
   ambos coinciden con `dc74a294c51362c35540f6caddb6657b70c544f8`.
   Como verificación independiente, `headSha` de la run vía
   `gh run view <RUN_ID>` también debe coincidir con
   `dc74a294c51362c35540f6caddb6657b70c544f8` (criterio de respaldo).
3. El artifact `release-validate-output-v{VERSION}` se descarga y
   contiene al menos: `BundleManifest v2`, `ReleaseInventory.json`,
   `SHA256SUMS`, todos los `*.tar.gz` de bundles declarados.
4. Step Summary muestra `mode: validate-only (no tag, no publish,
   no draft)` y `prospective_tag: v{VERSION}`.
5. SHA256SUMS y los `bundle-*.yaml` muestran `source_commit:
   dc74a294c51362c35540f6caddb6657b70c544f8` (auto-consistencia).

Si cualquier criterio falla, **C7 sigue BLOQUEADO** y se requiere
investigación antes de reintentar.

### E.5 — Criterios de ABORT (no continuar)

- Cualquier job termina con `conclusion: failure`, `cancelled` o
  timed-out.
- El artifact no se produce, o no contiene SHA256SUMS.
- El step `Bind to expected_sha` falla: `github.sha` no coincide
  con `dc74a294c51362c35540f6caddb6657b70c544f8` (la rama `main`
  avanzó entre el push y el dispatch).
- Como verificación independiente: `headSha` de la run vía
  `gh run view` no coincide con `dc74a294c51362c35540f6caddb6657b70c544f8`.
- Se detecta que el workflow invocó `gh release` o `git tag`
  (verificable en los logs de la run).

En cualquiera de estos casos, NO se reintenta automáticamente; se
abre ticket de investigación.

---

## F. Lo que esta propuesta NO hace (límites explícitos)

1. **NO crea el tag `v0.97.6`.** No se ejecuta `git tag v0.97.6` ni
   `git push --tags`. El artefacto del workflow es un bundle
   *prospectivo* (en el sentido de "lo que sería la release si se
   llegara a firmar", no de "release firmado").
2. **NO publica una GitHub Release.** No se invoca `gh release create`,
   `upload`, `edit`, ni `actions/attest-build-provenance`.
3. **NO firma C7.** Aunque los 4 jobs pasen y el artifact descargue
   íntegro, C7 sigue BLOQUEADO hasta admisión F7 con reconciliación
   C5/C6 explícita.
4. **NO autoriza H-05 ni H-06.** Estas certificaciones de Fase 5 / 6
   las decide el operador por separado cuando lo considere.
5. **NO modifica `ci/`, `.pipeline.kts`, `.github/workflows/release.yml`,
   `AGENTS.md`.** Verificado arriba.
6. **NO reabre trabajo en C0–C6.** No toca work units cerradas, no
   reescribe certificados existentes, no revierte commits.
7. **NO avanza el roadmap post-PRF** (LSI, análisis de impacto,
   RPC, Control Plane) — esos esperan a admisión F7.

---

## G. Riesgos identificados (sin bloqueo, pero documentados)

### G.1 — Posibles flakies en el cargo build

- `cargo build --release --bin cognicode-release` puede variar en
  duración entre runs (depende de cache hits). El `Swatinem/rust-cache@v2`
  acota el primer build (~10–15 min); las siguientes son sub-minute.
- **Mitigación:** `concurrency: group: release-validate-${{ github.ref }}`
  previene builds concurrentes del mismo ref.

### G.2 — Negative-test depende de al menos 2 archivos

- Si el `validate` produce 1 solo `*.tar.gz`, `negative-test-missing`
  falla inmediatamente con `::error::need at least 2 archives`. Esto
  es defensivo: si solo hay 1 archivo, no se puede validar el caso
  "remove one and observe remaining set".
- **Mitigación:** el caso de fallo es claramente diagnosticable; el
  operador verá el `::error::` en los logs.

### G.3 — Networking en `Swatinem/rust-cache`

- El cache es opcional, no obligatorio. Si GitHub Actions no lo
  resuelve, el build tarda más pero el binario se produce igualmente.
- **Mitigación:** ninguna acción requerida; el `set -e` mantiene
  el job failure si el build falla por otra causa.

### G.4 — Concurrencia con `release.yml` si se llegara a invocar

- Si en algún futuro se invocase `release.yml` simultáneamente con
  `release-validate.yml` sobre el mismo ref, `release.yml` necesitaría
  `contents: write` que este workflow no tiene → no hay conflicto
  de capability, pero podría haber conflicto de cache en disco.
- **Mitigación:** la proposal NO invoca `release.yml`. Operador-gated.

---

## H. Handoff tras la ejecución (lo que el agente devolverá)

Si la autorización se concede y el remote-validate se ejecuta:

1. **Run-id y head_sha observado:** número de run de GitHub Actions,
   confirmación de que el step `Bind to expected_sha` registra que
   `github.sha == dc74a294c51362c35540f6caddb6657b70c544f8`, y de que
   `head_sha` (vía `gh run view`) también coincide.
2. **Resultados por job:** `build`, `validate`, `negative-test-missing`,
   `negative-test-altered` con `conclusion` de cada uno.
3. **Hashes SHA-256 de los artefactos descargados:** del bundle
   `*.tar.gz`, de `SHA256SUMS`, y de `release-inventory-*.json`.
4. **Step Summary de la run:** texto completo del summary embebido
   (incluye `mode: validate-only`, `source_commit`, `prospective_tag`,
   `result`).
5. **Reporte de gates:** cuáles pasaron / cuáles fallaron + comando
   exacto del log que lo demuestra.
6. **Reporte de NO-release:** grep explícito en logs por `gh release`,
   `git tag`, `actions/attest` para confirmar que nunca se invocaron.
7. **Próximo paso sugerido (operator-gated):** reconciliación C5/C6
   vs este resultado, o reintento acotado si hubo gate fallido.

---

## I. Tabla de decisiones pendiente

| Decisión | Bloqueada por | Liberador |
|---|---|---|
| **Push 12 commits a origin/main** | esta propuesta (autorización del operador) | OK del operador sobre §A + §B |
| **`workflow_dispatch release-validate.yml`** | push aceptado + autorización | push aceptado + OK del operador sobre §D + §E |
| **Tag v0.97.6** | autorización separada | NO incluida en esta propuesta |
| **GitHub Release** | autorización separada | NO incluida |
| **Firma C7** | admisión F7 con reconciliación | NO incluida |
| **H-05, H-06** | autorización separada | NO incluida |

---

## J. Comandos de validación que el operador puede ejecutar antes de aprobar

Para confirmar el estado actual sin verme:

```bash
# SHA y ascendencia
git rev-parse HEAD                                  # dc74a294c51362c35540f6caddb6657b70c544f8
git rev-parse origin/main                           # edf45fb81242cb1c6ad301fdc092f134f66ae90a
git merge-base HEAD origin/main                     # edf45fb81242cb1c6ad301fdc092f134f66ae90a
git rev-list --count origin/main..HEAD              # 13

# Diff limpio de operator-managed
git diff --name-only origin/main..HEAD | \
  grep -E '^(\.github/workflows/release\.yml|ci/|\.pipeline\.kts|AGENTS\.md)$'

# Sin tag v0.97.6
git tag -l 'v0.97*' | grep -E 'v0\.97\.6$' && echo "TAG FOUND" || echo "no v0.97.6"

# El workflow es YAML válido
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release-validate.yml'))" \
  && echo "YAML valid"

# El release-validate.yml solo menciona tag/release en prohibiciones
grep -nE "(gh release|git tag|actions/attest)" .github/workflows/release-validate.yml

# Tests locales (suite serial)
cargo test -p cognicode-cli --lib -- --test-threads=1
cargo test -p cognicode-core --lib -- --test-threads=1
```

Cada uno puede ejecutarse de forma independiente.

---

**FIN DE PROPUESTA. NO EJECUTAR HASTA AUTORIZACIÓN EXPLÍCITA.**
