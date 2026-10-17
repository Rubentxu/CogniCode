# Mandato operativo para agente — programa CU

**Proyecto exclusivo del paquete:** CogniCode (`Rubentxu/CogniCode`), con CogniCode Guard como consumidor externo prioritario. El paquete es propuesta inicial `PROPOSED`; no declara ninguna implementación o prueba ejecutada. Aplica todo lo dispuesto en `AGENTS.md` del repositorio, que prevalece sobre este documento.

## GOAL

Avanzar el primer bloque CU pendiente del subprograma `docs/roadmap/consumer-contracts/ROADMAP.md` **sin reabrir PRF o B1–B6**, y terminar con valor real de SDK/CLI/MCP, PR integrado, release remota y pruebas temporales retiradas cuando proceda. Mantener el roadmap activo `docs/roadmap/ROADMAP.md` como autoridad de agenda. No añadir otra agenda o harness.

## PRE-FLIGHT, antes de escribir

1. `git status --short --branch`, `git rev-parse HEAD`, `git log -5 --oneline`, `git remote -v` y `git tag --sort=-version:refname | head -5` (registrar valores reales).
2. Leer `AGENTS.md`, `docs/roadmap/ROADMAP.md` §10/§11 si se integró, `docs/roadmap/CURRENT.md`, dos últimas entradas `JOURNAL.md` y el OpenSpec de la capacidad si existe; reconciliar con código, HEAD remoto y releases. No confiar en versiones de esta propuesta si cambiaron.
3. Investigar implementación real de las capacidades descritas en SPEC-CU-001..006. Para lo que ya existe, medir si el contrato está cumplido; eliminar del plan cambios redundantes. Para toda corrección: test RED que discrimine y GREEN posterior.
4. Leerse router real `delivery-first-testing` desde su skill cuando esté disponible; seleccionar lentes `ready` por estado observado, no por nombre imaginado. `cross-repo-consumer` figura `planned`: no ejecutarla.
5. Resolver antes de editar cualquier conflicto con el roadmap §10 y el enlace no resuelto a `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`. Si hay discrepancia, documentar y parar ese tramo concreto, no inventar archivos de evidencia histórica.

## LOOP DE ENTREGA PARA CADA CU-N

1. Definir 1 o más verticales reales del consumidor; ejemplo concreto y negativo; cap en tareas para no desviarse hacia harness masivo.
2. Worktrees de agent teams aislados, contratos congelados, commits atómicos `feat|fix|refactor|test|docs|chore(scope): ...`; arquitectura orientada a tipos y pureza en la medida de lo posible.
3. TDD afectado con `bash scripts/test-fast.sh`, sin falsa certificación por selector; contratos necesarios para seguridad/estado verificable. Test efímero `UAT-CU-N`, `AAT-CU-N` y E2E condicional en tmp externo.
4. Revisar seams/métricas, capacidad realmente nueva y código duplicado retirado. Triage de primeros fallos en vez de pasar días arreglando UAT obsoletas.
5. Antes de PR/tag ejecutar el full real del repo `bash scripts/test-full.sh` según AGENTS. Ejecutar `merge-gate.pipeline.kts` y `integration.pipeline.kts` vía PipelineK autorizado del entorno según su contrato; si no hay runner marcar BLOCKED, no usar GitHub Actions inventadas.
6. Crear PR, obtener checks y mergear remoto a `main`; resolver conflictos entre teams y reconciliar SHA. Solo después cortar RC en `release-candidate.pipeline.kts` y publicar con `release.pipeline.kts` si autorizado.
7. Descargar los assets de la GitHub Release y comprobar **conjunto completo** esperado en product manifest más hashes, instalación, CLI, MCP y contrato SDK; detectar faltantes aunque checksums de los presentes pasen.
8. Registrar recibo veraz en `docs/roadmap/JOURNAL.md`; regenerar `CURRENT.md` y ratchet; limpiar UAT/E2E/AAT temporales después de decidir regresión, sin borrar protección crítica.
9. Cierre SOLO si `DONE` real. Si release bloqueada por CI/autoridad del operador, estado `PAUSED` con motivo verificable y próximo paso, **sin prometer trabajo en background**.

## Leyes de diseño y de datos

- Una semántica, un dueño. No fabricar registro paralelo de herramientas, diagrama/índice de grafos o almacén de evidencia solo por comodidad de CLI/MCP.
- Dominio: ADTs, smart constructors, `Result`/`NonEmpty`, `fold/map`, leyes de composición y transformación pura. Side effects confinados en adapters; no introducir traits o layers si no reducen acoplamiento real.
- Precisión ontológica: `Detection != Evidence != VerifiedFinding != RuleAdmission != PolicyDecision`; `Complete != Partial != Unknown != Unsupported != Failed`.
- Never upcast: precisión heurística no es determinismo; firma no vacía no es validez; empty results no prueban ausencia de problemas.
- Evitar `StringlyTyped` en identidades de rutas, snapshots, capacidades y permisos cuando ya existan enums/newtypes.
- `Read` no implica escribir caché en workspace; `apply` de refactor no es consulta.
- Evitar código muerto, test duplicado, wrappers no usados y cambios de estilo no relacionados.

## Paraleo autorizado y secuencia de merges

CU-1 equipos A (core facade), B (catalog+wire), C (Guard consumer fixture); CU-2 A (evidence bridge), B (ADT/ontology), C (Guard contract); CU-3 A (affordances), B (cursor), C (agent acceptance); CU-4 A (cache), B (compat Guard), C (release inventory), D (test portfolio). Team B no edita tipos que Team A todavía está cambiando. Un integrador solo es dueño de `product/tools.json`, `docs/roadmap/*`, `Cargo.toml` y los pipelines cuando hay conflictos; fusiona pequeñas PRs en serie. Team de auditoría no publica releases de otro team.

## Reporte mínimo del agente por hito

```text
CU-N: capacidad útil; baseline sha; delta de código; tests afectados y coste;
UAT/AAT/E2E ejecutados y rc; proofs/gaps; test permanente promovido;
UAT temporales eliminadas; merge PR+SHA remoto; release+asset inventory+checksums;
smoke consumer real; estados externos Guard; siguiente bloque desbloqueado.
```

**STOP conditions:** falta autorización de push/merge/release, gate rojo no justificado, divergencia de autoridad entre roadmaps, posible fuga o cambio de datos, firma humana solicitada. Estado `BLOCKED`/`PAUSED`, nunca PASS ficticio.
