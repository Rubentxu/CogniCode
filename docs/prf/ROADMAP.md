# Roadmap único y secuencial — Production-Ready Foundation

**Status:** PROPUESTO para implementación; planificación registrada el 2026-09-21. Baseline `0903108fc372766a69a89a76ed7f79ffff90502d`. **Regla de secuencia:** ningún hito de implementación posterior se inicia sin pasar el gate obligatorio del anterior; pueden investigarse riesgos en spikes read-only, sin abrir un nuevo motor o capa por anticipación. Los porcentajes se calculan por requisitos `ACCEPTED / aplicables` del hito, nunca por número de archivos o commits.

| Orden | Hito | Dependencias y resultado objetivo | Certificación de salida |
|---|---|---|---|
| F0 | Inventario honesto | Fijar baseline, comandos/binarios realmente publicados, herramientas MCP/lenguajes/flags, puertos de datos, matriz de clientes, corpus, tiempos; registrar fallos conocidos y peligros de refactor. Congelar contratos existentes sin prometer que todo es estable. | C0 |
| F1 | Producto mínimo instalable y arrancable | `cogh` instala/verifica `cognicode` + `cognicode-mcp`; CLI exit codes correctos, stdio exclusivo para MCP; sin collector OTLP ni API/daemon obligatorios; clean-HOME y offline core. Clasificar `explorer-*`, conservar adaptadores. | C1 |
| F2 | Correctitud reproducible | Corpus real de operaciones críticas: escaneo anidado, cambios con mtime preservado, errores de lectura y cobertura explícita, identidad de revisión, estrategias full/per_file/on_demand; igualdad semántica entre dos ejecuciones y no false-clean. Primero caracterizar, después corregir fallos localizados. | C2 |
| F3 | Base de aplicación compartida | Una vertical real (abrir workspace → analizar → impacto/evidencia → resultado) con puertos neutrales de transporte; CLI/MCP llaman el mismo caso de uso. Composición en runtime; refactor incremental de `AnalysisService`/`WorkspaceSession` solo donde lo exijan tests. No introducir segundo grafo canónico. | C3 |
| F4 | Persistencia y aislamiento | Inventario de fuentes de verdad (LadybugDB/caches/LSI), identidad workspace+config+revisión, migración conservadora, cancelación/crash/reinicio, bloqueo entre procesos, backups y rollback sin mezclar proyectos. Recorrido real para cada capacidad que prometa evidencia duradera. | C4 |
| F5 | Seguridad, fiabilidad y extensibilidad acotada | Permisos read/write/execute/net explícitos; traversal, symlink, límites, secretos, cancelación, interacciones MCP adversariales; una extensión de análisis read-only sin modificar varios despachadores ni duplicar estado. Telemetría opcional, stderr-only para MCP. | C5 |
| F6 | CI y artefactos reproducibles | Definir CI local-first + verificación independiente para cada SHA de merge/release; tests críticos bloqueantes, features matrix, advisories/licencias, rendimiento con baseline, SBOM/provenance cuando aplicable; release real GNU x86_64/aarch64 e instalación/actualización/rollback desde bundle limpio. | C6 |
| F7 | Release candidate y soporte | UAT end-to-end de CLI/MCP desde *el mismo artefacto*, scorecard con enlaces, semver/changelog/guía de migración, matriz real de soporte, pruebas repetidas en plataformas soportadas, go/no-go firmado. | C7 |

## Primeras unidades de trabajo, sin saltos

- **F0.W1** Verificar HEAD, árbol, gitignore, estado de CI, release, binarios y lista de herramientas MCP, y obtener corpus/manifest con hashes. Dejar prueba/artefacto y registrar líneas base en `STATE.md` y `JOURNAL.md`.
- **F0.W2** Matriz de capacidades por operación/lenguaje/transport/flag: `SUPPORTED | PREVIEW | DEPRECATED | UNKNOWN`; 2 clientes reales y versiones publicadas. Diferenciar `cognicode`, `cognicode-mcp`, `cogh`, `explorer-mcp`.
- **F0.W3** Triage de fallos P0: análisis parcial por lectura, `per_file` no recursivo, `mtime` no fiable, estado por workspace, instalación/reinstalación; convertir cada hallazgo en reproducción y test RED **antes de tocar producción**.
- **F0.W4** Ejecutar/recoger `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` (o baseline de lint aprobado), `cargo test --workspace --all-targets`, test de cliente MCP real y UAT clean-HOME; marcar NOT_RUN si falta runner/dependencias.
- **F0.W5** Aprobar contrato estable mínimo y puertas C1–C7 con presupuesto medido; registrar decisión sobre workflow local-first vigente antes de activación remota. Gate C0 = inventario firmado y reproducible; no exigir arreglar todos los defectos durante el inventario.

## Límite temporal de trabajo nuevo

F0–F2 fijan **qué hace el producto y cuándo responde de forma cierta**. F3 elimina duplicidad demostrada sin refactor generalista. F4 garantiza que el estado que F3 consume sobrevive y no se mezcla. F5 hace segura/extensible esa misma vertical. F6–F7 certifican y publican exactamente esos binarios. LSI/RPC/CP son dependencias o posibles consumidores futuros, nunca hitos PRF ocultos.

## Prohibiciones de cierre

Un informe de tests unitarios no cierra una UAT de binario; `|| true`, `continue-on-error`, falta de salida o ausencia de evidencias no se convierten en PASS; los límites de soporte no se amplían por disponer de un módulo. Cualquier reducción de alcance se aprueba **antes** de medir un gate y actualiza la promesa pública; no se rebaja un umbral a posteriori para cerrar un ciclo.
