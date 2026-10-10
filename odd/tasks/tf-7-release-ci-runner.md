# Goal — TF-7 Release CI runner con pipelinek sano

> **Estado**: PENDING (operator-gated infra). Autorizado en el goal
> `v1015-consolidation-and-promote` como TF-7, documentado al descubrir
> que el studio/control plane de pipelinek no acepta runs nuevos.

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `odd/tasks/v1015-consolidation-and-promote.md` §TF-7 | origen |
| `odd/tasks/v1015-consolidation-and-promote.md` TF-5e recibo | documenta la publicación manual de v0.101.10 y lo que NO se pudo hacer |
| `~/.pipelinek-studio/runtime/studio.db` `run_state_history` | la última entrada es de 2026-10-10T04:34:31Z (9+ horas antes de la publicación manual de v0.101.10); desde entonces ningún run entra a `CREATED`/`QUEUED` |
| `release-candidate.pipeline.kts` + `release.pipeline.kts` | la release factory completa; estos dos scripts son los que el runner sano ejecutaría |
| TF-5 (v0.101.10 publicación manual) | TF-7 evita que la **próxima** release tenga que recurrir al workaround manual |

## 1. Objetivo

Disponer de un entorno (CI runner) donde `pipelinek run` ejecute la
release factory completa (`release-candidate.pipeline.kts` →
`release.pipeline.kts`) sin el workaround manual que esta sesión tuvo
que usar para v0.101.10. El resultado verificable: la próxima release
del proyecto sale por la pipeline canónica, con todos los gates
(`clean-clone` preflight, `cargo deny`, `cognicode-release verify`
R1-R9, re-download, `verify-latest-resolution`, `verify-as-consumer`)
ejecutados por la propia pipeline, no por un humano en un shell.

## 2. Problema

`pipelinek` local funciona (corre scripts simples, emite eventos NDJSON,
termina con `Pipeline finished with SUCCESS`). Pero el **control plane**
de pipelinek — el Studio que coordina runs entre procesos y stages
dentro de runs largos — está bloqueado:

- `~/.pipelinek-studio/runtime/studio.db` → última entrada en
  `run_state_history` es de `2026-10-10T04:34:31Z`, en estado `CREATED` o
  `QUEUED`. Ningún run entra desde entonces.
- `pipelinek run release-candidate.pipeline.kts` (background) → exit 0
  silencioso, sin eventos, sin crear run record. El proceso arranca,
  emite los warnings de `sun.misc.Unsafe`, dice
  `Discovered external Step plugins`, y sale. Sin stages ejecutadas.
- Mismo síntoma que el `merge-gate` que esta sesión ya tuvo que
  bypass-ear (lesson 85 de MAINTENANCE.md), pero con un exit code
  diferente (`0` vs el `durable-shell code 101` del merge-gate).
- Causa probable: un **proceso externo** está consumiendo recursos del
  control plane. En el momento de esta sesión, hay un `gradle-daemon`
  consumiendo 2.4 GB RAM y otra sesión Claude trabajando
  en `/var/home/rubentxu/Proyectos/kotlin/PipelinekStudio` (el repo
  del propio pipelinek). El Studio de pipelinek detecta que la runtime
  data dir está en uso concurrente y rechaza runs nuevos.

## 3. Por qué ahora

v0.101.10 salió por la ruta manual. La próxima release (v0.101.11 con
TF-6 aarch64, o v0.102.0 con T1-5 presenter move) no debería tener
que recurrir al mismo workaround. Cada release manual es un recibo
menos en el audit trail y una oportunidad menos de que la pipeline
canónica detecte una regresión que el humano no vería.

## 4. Scope

### 4.1 In-scope

* Diagnosticar definitivamente la causa del bloqueo del Studio de
  pipelinek: ¿es el gradle daemon, es la sesión Claude concurrente, es
  el Studio arrancado en `--port 18778` con `--db /tmp/studio-app-uat-1028572.db`
  que se ve en `ps aux`?
* Establecer un entorno donde `pipelinek run` ejecute la release factory
  completa sin workaround. Opciones:
  - **(preferida)** un GitHub Actions runner (o equivalente) con
    pipelinek 0.47.0 pre-instalado, sin concurrencia con el Studio local.
  - **(alternativa)** un contenedor/VM dedicado donde solo corra el
    Studio, con acceso a `~/.cargo-targets` y al repo.
  - **(alternativa)** arreglar el Studio local: kill el proceso
    colgado, limpiar `~/.pipelinek-studio/runtime/`, re-arrancar.
* Restaurar la cadena canónica: el `merge-gate` del repo (que también
  está bloqueado por la misma infra, per lesson 85) debería volver a
  correr via pipelinek en lugar del bypass.

### 4.2 Out-of-scope

* Mejorar la observabilidad de pipelinek. Es un proyecto upstream
  (`Rubentxu/pipeline-kotlin`); TF-7 delega el fix al maintainer de
  pipelinek si la causa es un bug del orquestador.
* Cambiar la arquitectura de la release factory. El diseño
  `release-candidate.pipeline.kts` + `release.pipeline.kts` está bien;
  el problema es el runtime, no el script.
* TF-6 (aarch64) — TF-7 no depende de TF-6 ni viceversa; ambos
  están pendientes pero son ortogonales.

## 5. Constraints

* **Sin pérdida de lo ya ganado**: el probe fix (`af8faa5`) y el scope
  TF-6 (`451d0b7`) deben seguir siendo válidos cuando el runner esté
  sano.
* **No reintroducir el workaround manual como camino "oficial"**: el
  TF-5e recibo documenta que la publicación manual fue una excepción,
  no el flujo canónico. La release canónica es la pipeline.
* **Idempotencia**: si TF-7 se logra con un CI runner dedicado, no
  debe depender de que el host local esté limpio (porque el host local
  tiene el Studio en estado de compilación por la sesión Claude de
  PipelinekStudio).

## 6. Work items

| ID | Tarea | Verificación |
|----|-------|--------------|
| **TF-7.1** | Diagnosticar definitivamente: ¿gradle daemon, sesión Claude en PipelinekStudio, Studio en `--port 18778`, o combinación? | `ps aux` + `studio.db` schema + logs del Studio identifican el culprit |
| **TF-7.2** | Decidir el entorno: CI runner dedicado, contenedor, o fix local | Decisión operator-gated documentada |
| **TF-7.3** | Provisionar el entorno (sea el que sea) | `pipelinek run` ejecuta un script simple en menos de 30s, evento `RunFinished outcome:success` aparece en el studio db |
| **TF-7.4** | Re-habilitar `merge-gate` para que NO requiera bypass (lesson 85); restaurar la branch protection con `strict:true, contexts:[merge-gate]` sin bypass | `gh pr view <N> --json mergeStateStatus` retorna `CLEAN` para un PR de smoke sin bypass |
| **TF-7.5** | Correr la release factory completa sobre un SHA de prueba (puede ser el mismo v0.101.10) y verificar que la pipeline produce + publica sin intervención humana | `gh release view v0.101.10 --json assets` lista los assets; el `release.db` del Studio tiene `outcome:success` |

## 7. Acceptance criteria

1. TF-7.1..5 en DONE.
2. `merge-gate` verde vía pipelinek (sin bypass per lesson 85) en un
   PR de smoke contra `main`.
3. `pipelinek run release-candidate.pipeline.kts` completa todos los
   stages (`consume-candidate`, `negative`, `draft`, `provenance`,
   `publish`) en un runner sano.
4. `pipelinek run release.pipeline.kts` completa `consume-candidate`,
   `negative`, `draft`, `provenance`, `publish` con `verify-as-consumer`
   verde.
5. Update a `odd/tasks/v1015-consolidation-and-promote.md` TF-7 row.

## 8. Risks

* **R-1**: el diagnóstico (TF-7.1) puede requerir acceso al proyecto
  `Rubentxu/pipeline-kotlin` upstream, lo que es operator-gated.
* **R-2**: provisionar un CI runner dedicado es operator-gated y
  depende de presupuesto / decisión de platform team.
* **R-3**: si la causa es un bug en pipelinek 0.47.0, el fix puede
  tardar (upstream release cycle). Mitigación: downgrade a 0.46.0
  documentado en `scripts/ci/probe-pipelinek-semantics.sh` puede ser
  una solución provisional.

## 9. Recibo (append-only)

| Fecha | WU | Commit | Notas |
|-------|----|--------|-------|
| _pendiente_ | TF-7.1 | _pendiente_ | diagnóstico del Studio bloqueado |
| _pendiente_ | TF-7.2..5 | _pendiente_ | entorno + merge-gate + release factory |
