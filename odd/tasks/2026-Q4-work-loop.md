# 2026-Q4 Work Loop — Programa CLI/MCP B1..B6

> **Estado**: AGENDA ACTIVA. Define la cadencia de iteración sobre
> [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md).
> Cada bloque (B1..B6) tiene su goal en `odd/tasks/b*-*.md`.

## Principios

1. **Un bloque a la vez.** No abrir B2 hasta B1 cerrado con `v0.101.11` released.
2. **Una WU a la vez dentro del bloque.** Cada WU es un commit atómico.
3. **Evidencia real o nada.** No cerrar WU sin UAT/AAT ejecutado sobre binario/servidor real.
4. **Bash scripts/test-fast.sh** entre iteraciones (loop sub-segundo cuando el subset está en cache).
5. **bash scripts/test-full.sh** antes de PR (gate obligatorio).
6. **No push forzado, no bypass de branch protection.** Blocked ≠ Failed: documentar y consultar.

## Cadencia

| Nivel | Frecuencia objetivo | Comando / cuándo |
|---|---|---|
| **Iteración de WU** | ~1-3 días | `bash scripts/test-fast.sh` entre cada cambio de comportamiento |
| **Pre-PR** | Al cerrar todas las WUs de un bloque | `bash scripts/test-full.sh` + UAT + AAT |
| **Cierre de bloque** | Al cierre de las WUs | `git tag -a v0.101.NN` + GitHub Release + bump SemVer |
| **Actualización de ROADMAP** | Cada bloque cerrado | `docs/roadmap/ROADMAP.md` §10 + JOURNAL.md |

## Pre-WU (al arrancar una WU)

1. Verificar prerrequisitos del bloque: ¿están las WUs anteriores cerradas? ¿está `bash scripts/test-fast.sh` verde?
2. Confirmar branch protection: `git status` para ver divergencia vs `origin/main`.
3. Si la WU es nueva y sustancial, actualizar el goal doc con la estimación.
4. Si la WU requiere exploración amplia (4+ archivos), lanzar una `mcp__codegraph__codegraph_explore` antes de escribir.

## WU (loop de implementación)

1. **Branch** desde `origin/main` actualizado (si branch protection lo exige — para CogniCode, los T0-N precedents commitean directo a `main`, mantener el patrón vigente).
2. **TDD**: RED → GREEN → REFACTOR.
   - Si toca `commands.rs`, `presenter.rs`, o `ApplicationService`, escribir el test primero.
   - Si toca wire format, escribir el contrato first.
3. **Commits atómicos Conventional Commits**:
   - `feat(...)`, `refactor(...)`, `test(...)`, `chore(...)`, `docs(...)`.
   - Mensaje con scope y razón.
   - Sin atribución IA en el commit.
4. **Test-fast** entre iteraciones:
   ```bash
   bash scripts/test-fast.sh
   ```
5. Si la WU toca la frontera library/bin (T1-5, B2.W5), verificar que el text path sigue byte-identical con snapshot tests.

## Post-WU (al cerrar una WU)

1. `bash scripts/test-full.sh` exit 0. Si falla, NO cerrar la WU.
2. Actualizar el goal doc: SHA del commit + nota breve.
3. Si la WU está completa, marcar `[x]` en el checklist.
4. Si quedan WUs en el bloque, mover a la siguiente.

## Pre-PR (al cerrar todas las WUs de un bloque)

1. `bash scripts/test-full.sh` exit 0.
2. Ejecutar UAT del bloque (definidas en `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`).
3. Ejecutar AAT del bloque.
4. `git status` clean.
6. PR a `main` (no fast-forward; PR con `merge-gate` estricto).

## Cierre de bloque (al merge del PR)

1. Bump SemVer según el release target del bloque.
2. Tag anotado: `git tag -a v0.101.NN -m "..."`.
3. Push del tag: `git push origin v0.101.NN`.
4. GitHub Release con assets (binarios `cognicode`, `cognicode-mcp`, `cogh` + checksums + `install.sh`).
5. Actualizar `docs/roadmap/ROADMAP.md` §10 con SHA integrado + recibo + deuda residual.
6. Append a `JOURNAL.md` con fecha, bloque, SHA, gates, UAT/AAT.
7. Actualizar `docs/roadmap/CURRENT.md` snapshot.

## Cross-block (al cerrar B1..B6)

- B6 cierra el programa con la firma contractual de C8-R por el operador.
- Sin firma, programa `BLOCKED` con disparador explícito.
- PR con `merge-gate` estricto es el ÚNICO mecanismo válido.

## Anti-patrones prohibidos

- "Completar" ciclos sin que los gates estén en verde real.
- Crear dos fuentes de verdad sobre el roadmap.
- Saltar bloqueos para aparentar progreso.
- Tocar archivos del usuario sin permiso explícito del bloque.
- Cerrar WU con pruebas vacías o capacidades simuladas.
- Comunicar "PASS" cuando el orquestador (PipelineK) falla.

## Reglas duras (no negociables)

- Sin cambios destructivos al workspace preexistente (no reset, no rebase forzado).
- Sin nuevos `println!` en `cognicode-core` durante refactors (van a `tracing` o al struct de retorno).
- Sin abstracciones genéricas sin acoplamiento real probado con tests.
- Sin fusionar nombres iguales de dominios distintos (lección §10 de `EvidenceStore`).

## Métricas de progreso

| Métrica | Cómo leerla |
|---|---|
| WUs cerradas / WUs totales del bloque | El ratio del checklist del goal doc |
| `bash scripts/test-fast.sh` | exit 0 entre iteraciones (sub-segundo en cache) |
| `bash scripts/test-full.sh` | exit 0 antes de PR |
| `python3 scripts/ci/test_roadmap_version_ratchet.py` | exit 0 al regenerar snapshot |

## Herramientas de orquestación

- **CronCreate**: enqueuear recordatorios a futuro ("avanzar con el siguiente WU", "regenerar CURRENT.md si han pasado 14 días"). Una sesión puede tener varios cron jobs; los in-memory mueren con la sesión, los `durable: true` persisten en `.claude/scheduled_tasks.json`.
- **ScheduleWakeup**: solo en `/loop` dynamic mode (no aplica aquí, salvo que el operador autorice `/loop`).
- **ListAgents / SendMessage**: para delegar explorers/writers/verifiers cuando la WU excede el trigger de mapeo (4+ archivos) o writer (2+ archivos no triviales).

## Estado actual del programa

| Bloque | Release | Estado | Próxima WU |
|---|---|---|---|
| B1 | v0.101.11 | PENDING | B1.W1 (reconciliación de versión) |
| B2 | — | BLOCKED (B1 prerequisite) | — |
| B3 | — | BLOCKED (B2 prerequisite) | — |
| B4 | — | BLOCKED (B3 prerequisite) | — |
| B5 | — | BLOCKED (B4 prerequisite) | — |
| B6 | — | BLOCKED (B5 prerequisite) | — |