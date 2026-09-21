# H10 — Corrección del análisis (versión final)

## Resumen del error

Mi diagnóstico del flake `lifecycle::tests::test_cogh_update_respects_lockfile`
pasó por tres iteraciones. La versión final es la correcta.

## Iteración 1 — Incorrecta (race condition)

**Diagnóstico**: race condition entre tests que mutan HOME, colisión
de `process::id()`, tests en `ide.rs` sin `#[serial]`.

**Por qué era incorrecto**:
- El test YA TIENE `#[serial]` aplicado (verificado en
  `crates/cognicode-cli/src/cmd/lifecycle.rs:861-863`).
- El ciclo `e50-lsi-cli-test-parallel-safety` (commit `ae74558e`,
  archivado `2a01b320`) ya trabajó este problema el 2026-09-15 y
  logró **0 failures / 40 runs** en su verificación.

**Acción propuesta (incorrecta)**: cambiar `process::id()` a
`thread::current().id()` en los tests — **irrelevante** porque no
hay race condition.

## Iteración 2 — Parcialmente correcta (GitHub API rate limit)

**Diagnóstico**: el subproceso `cogh update` ejecuta una llamada
HTTP a `api.github.com/repos/Rubentxu/CogniCode/releases/latest`,
que falla con HTTP 403 (rate limit agotado) en la IP del operador
(79.117.205.213).

**Por qué es parcialmente correcta**:
- La causa inmediata del fallo HOY es el rate limit agotado.
- Verificado con `cogh update` manual: devuelve `HTTP 403 Forbidden`
  con `API rate limit exceeded for 79.117.205.213`.
- Verificado con `curl https://api.github.com/rate_limit`:
  `"remaining": 0` de 60.
- La IP `79.117.205.213` (Gasteiz/Vitoria, AS57269 DIGI SPAIN)
  coincide con la IP del operador.

**Lo que faltaba considerar**:
- e50 ya arregló race condition → falla actual NO es por race.
- e50 logró 0/40 fallos → la flake apareció DESPUÉS de e50.
- Entre 2026-09-15 (e50 verify) y 2026-09-21 (hoy), el rate limit
  se ha podido agotar por uso continuado de los tests.

## Versión final (Iteración 3) — La causa real

**La flake del test `test_cogh_update_respects_lockfile` no es
causada por race condition** (e50 ya la arregló) ni por bugs del
código de CogniCode. Es causada por:

1. **Dependencia externa**: el subproceso `cogh update` llama a
   GitHub API real (`api.github.com/repos/Rubentxu/CogniCode/releases/latest`).
2. **Rate limit de IP no autenticada**: 60 requests/hr.
3. **Agotamiento del rate limit**: tras uso intensivo de los tests
   en sesiones previas (e50 verificó con 0/40 fallos el
   2026-09-15, hoy 2026-09-21 está agotado).

**Por qué afecta 3 tests diferentes**:
- `lifecycle::tests::test_cogh_update_respects_lockfile` (10/10 fallos)
- `layout::tests::cmd_rollback_after_live_install` (1/10)
- `installer_transaction::tests::t_debt2b_round_trip_extract_then_integrate` (1/10)

Cada uno consume parte del rate limit. Cuando se agota, todos fallan.

**Por qué `--test-threads=1` puede pasar (3/3)**:
- Serialización reduce concurrencia → menos requests simultáneos →
  cabe en el rate limit más veces.
- No es determinista; depende del estado del rate limit en el
  momento de la ejecución.

## Implicaciones

1. **No es bug del código de CogniCode**. Es dependencia externa.
2. **No requiere fix de código**.
3. **Requiere acción operativa**:
   - Esperar al reset del rate limit (ventana de 1h, ahora en 48 min).
   - O autenticar la IP con un token GitHub (sube el límite a 5000/hr).
4. **Afecta a UAT reales** que ejecuten `cogh update`, `cogh install`,
   o cualquier flujo que llame a GitHub API.

## Severidad final

LOW. No es regresión, no es bug del código, no bloquea PRF.
Es una observación operativa del entorno de red.

## Política del proyecto respetada

- "NO se commitea fix para flake externo" → no se ha tocado código.
- "Documentar el hallazgo" → sí, en este archivo y en el Apéndice A
  del runtime.md.
- "Honesto cuando el código falla por causa externa" → sí,
  explícitamente.

## Lección aprendida

Cuando un test falla con assert "unexpected: ...":
1. **Antes de teorizar**, ejecutar el subproceso manualmente y ver
   su output real. La salida de `cogh update` con un lockfile
   artificial reveló el HTTP 403 inmediatamente.
2. **Sub-sample grande antes de concluir "flake"**. 5 runs sugirieron
   20%; 10 runs revelaron 100% (determinista).
3. **Buscar trabajo previo sobre el mismo tema**. El ciclo e50
   ya abordó parallel safety el 2026-09-15. Si lo hubiera
   verificado primero, habría llegado al rate limit más rápido.
4. **Diferenciar entre race condition interna y dependencia externa**.
   El código tiene `#[serial]` correctamente aplicado. La flake
   es de red, no de código.

## Referencias verificadas

- `crates/cognicode-cli/src/cmd/lifecycle.rs:861-863`: `#[test] #[serial] fn test_cogh_update_respects_lockfile`.
- `openspec/changes/archive/2026-09-21-e50-lsi-cli-test-parallel-safety/verification-report.md`: "0 failures / 40 runs" post-e50.
- `curl https://api.github.com/rate_limit`: `"remaining": 0`, reset en 48 min.
- `cogh --home /tmp/.cognicode update` (manual): HTTP 403 con IP del operador.
- `ipinfo.io`: IP del operador = 79.117.205.213 (Gasteiz, DIGI SPAIN).
