# PRF-CI-06 — Decisión de política local-first y equivalencia a gate

> Estado: **DOCUMENTO REQUERIDO POR PRF-CI-06 MUST**. Este documento es la
> evidencia que el requisito exige. Se rige por las decisiones ya adoptadas
> (AGENTS.md "Local CI Is the Source of Truth", ADR-031, B3 del checklist
> V1.0.0 pre-cut del 2026-08-16). No introduce política nueva; la documenta
> y la hace auditable.

## 1. La decisión

**La política de CI de CogniCode es local-first.** El día a día ejecuta
CI en la máquina del desarrollador vía `act` + `podman` (recetas `just ci-local`,
`just ci-t6-dry`), con los workflows de `.github/workflows/` como definición
única y versionada de los gates. GitHub Actions remoto queda reservado
exclusivamente para el **release gate** (cortes de tag vía `release.yml`).

Origen de la decisión (evidencia verificable, no folklore):

- **AGENTS.md** — sección "Local CI Is the Source of Truth": GitHub Actions
  runs son reservados para release gate; CI diario es local.
- **ADR-031** — Release 1.0.0: definition of production-ready; el flujo de
  certificación depende de scorecards y flaky logs generados localmente
  (`sandbox/results/`).
- **B3 (V1.0.0 pre-cut checklist, 2026-08-16)** — triggers remotos
  deshabilitados deliberadamente: `on: push` / `on: pull_request` eliminados
  de `ci.yml`; la única vía de activación es `workflow_dispatch` vía `act`.

## 2. Por qué local-first (justificación)

1. **Fuente de verdad única:** la definición del gate vive en el repo
   (`.github/workflows/ci.yml`, `regression-check.yml`), no en la
   configuración de un proveedor externo. Un cambio de gate es un commit
   revisable como cualquier otro.
2. **Determinismo y control de recursos:** `act` + `podman` ejecuta los
   mismos pasos con las mismas imágenes; sin colas de runners compartidos ni
   variabilidad de infraestructura externa.
3. **Privacidad por diseño:** el core se ejecuta sin red; el código no
   abandona la máquina del desarrollador salvo en release.
4. **El release gate es remoto a propósito:** la verificación de publicación
   (tag cut, artefactos, provenance) sí corre en GitHub Actions porque sus
   inputs (tag, secretos de publicación) son remotos por naturaleza.

## 3. Equivalencia a gate obligatorio (la parte que exige PRF-CI-06)

PRF-CI-06 exige que la política local-first sea **equivalente a un gate
obligatorio** antes de activación remota, y que el workflow manual no se
cite como protección automática de PR. La equivalencia se materializa así:

### 3.1. Disco local como gate de commit

```bash
just ci-local
```

ejecuta la batería completa (`regression-check.yml` + `ci.yml`) vía `act`
+ `podman` sobre la revisión candidate. Un fallo de cualquier paso aborta
con exit code != 0. **El commit no se promueve a candidato de release sin
esta batería en verde.**

### 3.2. Los mismos workflows sirven para verificación remota

Los workflows versionados son la definición de gate. Si se activa CI remoto
(un `on: pull_request` en `ci.yml`), no se escribe un gate nuevo: se activa
el trigger del mismo workflow que ya corre localmente. **La activación del
trigger remoto es una decisión separada, explícita y reversible**, tomada
por el operador, no un cambio semántico de gate.

### 3.3. Disciplina de integración (reemplaza la protección automática de PR)

En ausencia de protección automática remota, la protección equivalente es
procedimental y está documentada:

| Momento | Gate obligatorio | Evidencia exigida |
|---|---|---|
| Antes de `git commit` | Batería focalizada (T0/T1) | output de tests focalizados |
| Antes de promover a candidato | `just ci-local` completo (T4 local) | exit code 0 de `act` |
| Antes de tag cut | `release.yml` en GitHub Actions | run exitoso en remoto |
| Antes de merge a main | bateria focal + ci-local sobre la revisión final consolidada | receipts locales |

**Regla dura:** ninguna integración a `main` ni tag cut ocurre sin la
batería completa ejecutada sobre la revisión consolidada final. Esta regla
está reforzada por PRF-CI-01/07 (recibo por SHA + prueba negativa del
pipeline), que son gaps abiertos de la matriz y quedan excluidos del alcance
de este documento.

## 4. Lo que este documento NO declara

- **No declara** que exista protección automática de PR remota. No la hay.
  La protección es procedimental (§3.3) y depende de disciplina del
  operador.
- **No declara** PRF-CI-01 ni PRF-CI-07 como satisfechos. La prueba negativa
  del pipeline y el recibo por SHA son gaps abiertos en la matriz
  (`ci.yml` sólo tiene `workflow_dispatch`; un E2E con `\|\| true`).
- **No declara** que `just ci-local` se ejecute automáticamente en ningún
  hook de git. No lo está. Su ejecución es una obligación del operador.
- **No declara** SLOs de rendimiento (PRF-CI-04). Baseline de rendimiento no
  publicado; queda fuera de este documento.

## 4.bis. Honestidad sobre el gap que persiste

PRF-CI-06 exige "equivalencia a un gate obligatorio". Este documento
documenta la política y su equivalencia **procedimental**. La equivalencia
**automática** (que un hook o el servidor rechace un push sin gate en verde)
NO existe hoy. Si el operador decide que la equivalencia exigida es
automática, el trabajo pendiente es:

1. Añadir pre-push hook (husky o equivalente) que ejecute `just ci-local`.
2. Activar `on: pull_request` en `ci.yml` con branch protection en GitHub.
3. Cerrar PRF-CI-01/07 (recibo por SHA + prueba negativa) para que el gate
   remoto sea auditable.

Esa decisión es del operador. Hasta entonces, la disposición honesta de
PRF-CI-06 en la matriz es **PARTIAL (mejorado)**: la política existe, está
documentada, es equivalente a gate en el sentido procedimental, pero la
enforcement automático no existe y su introducción es una decisión nueva
del operador.

## 5. Evidencia verificable

| Afirmación | Evidencia |
|---|---|
| La política existe y es deliberada | `.github/workflows/ci.yml` header (B3, 2026-08-16), AGENTS.md "Local CI Is the Source of Truth", ADR-031 |
| El mismo workflow sirve local y remoto | `just ci-local` ejecuta los mismos YAML que GitHub Actions ejecutaría |
| El release gate es remoto | `release.yml` (tag cuts en GitHub Actions) |
| La protección procedimental está documentada | §3.3 de este documento |
| El enforcement automático NO existe | Ausencia de pre-push hook que ejecute `just ci-local`; `ci.yml` sin `on: pull_request` |
| La matriz lo registra honestamente | `RECONCILIATION-MATRIX.md` §PRF-CI-06 actualizado a PARTIAL (mejorado) con este documento como evidencia |
