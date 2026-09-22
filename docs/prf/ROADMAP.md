# Production-Ready Foundation (PRF) — ROADMAP

> **Programa vigente**: F0–F7 con certificaciones C0–C7 (directiva
> del operador 2026-09-21). Las fases F0, F1 y F2 quedaron definidas
> al inicio del programa; F3–F7 se desarrollan **al cierre de cada
> fase anterior** para no anticipar infraestructura.

## Fases

PRF se organiza en fases (F0..F7). Cada fase agrupa unidades de trabajo (W)
con un objetivo verificable. Las fases avanzan en orden secuencial; las
unidades dentro de una fase pueden tener dependencias internas pero no se
consideran completas hasta que todas sus dependencias están satisfechas.

### F0 — Inventario y baseline

**Objetivo**: caracterizar el estado real de los productos CLI y MCP, sus
binarios, comandos, herramientas, dependencias, y comportamiento observable.
Establecer la línea base sobre la que se medirán los avances posteriores.

**Criterio de salida**: STATE.md actualizado con el inventario completo;
JOURNAL.md documentando los comandos ejecutados; UAT.md ejecutado sobre los
binarios reales con resultados observados (no inferidos del código).

| Unidad | Descripción |
|---|---|
| **F0.W1** | Inventario verificable de los binarios `cognicode`, `cognicode-mcp`, `cogh`, `explorer-mcp`, `explorer-api`: versiones, comandos, herramientas MCP, parámetros, esquemas, permisos. |
| F0.W2 | Caracterización del arranque, inicialización de dependencias, persistencia, red, stdout/stderr de cada binario. |
| F0.W3 | Baseline de pruebas: ejecutar `cargo test -p cognicode-core --lib`, `cargo test -p cognicode-cli --bin cogh`, `cargo test -p cognicode-cli --test cognicode_ide_adapter`. Documentar resultados actuales. |

### F1 — Estabilización

**Objetivo**: cerrar defectos críticos descubiertos en F0, asegurar la
reproducibilidad de los flujos CLI/MCP, eliminar comportamiento
experimental no documentado o documentar el comportamiento experimental.

**Criterio de salida**: cada defecto crítico de F0.W1 cerrado con prueba
de regresión; cada flujo CLI/MCP con test de integración que ejecute el
binario real y verifique el contrato observable.

(Las unidades concretas se definirán al cerrar F0.W1.)

### F2..FN — Pendientes de definición

Las fases F2 en adelante se definirán al cerrar F1, en función de los
defectos reales descubiertos y las prioridades que el inventario y la
estabilización revelen.

### F2 — Correctitud reproducible (definida tras F1)

**Objetivo**: garantizar que los resultados de análisis (CLI y MCP)
sean reproducibles, tengan cobertura explícita y no oculten cambios o
errores.

**Criterio de salida**: cada vertical de análisis dispone de un corpus
determinista con un oráculo independiente, los defectos descubiertos
están cerrados con tests de regresión y un UAT que ejecute el binario
real.

| Unidad | Descripción | Estado |
|---|---|---|
| **F2.W1** | Caracterización de la correctitud de `PerFileStrategy` (CLI `cognicode graph per-file`, MCP `get_per_file_graph`). Cerrar la invalidación del cache por cambio de contenido (R2 del brief). | **ACCEPTED** (commit `70f0b0cf`) |
| F2.W3 | Caracterización de equivalencia `full` vs `per_file` (R4). Las dos estrategias tienen propósitos distintos; documentar. | **ACCEPTED** (commit `d9aa09c0`; cert PRF-F2-W3 en `evidence/CERTIFICATES.md`). H-R4-1 (0 edges sobre corpus con cross-file call) registrado para F2.W4. |
| F2.W4 | Cerrar H-R4-1 capa 1 (parser: resolución de qualified calls). Otros 5 frentes del brief original → deuda documentada. | **ACCEPTED-parcial** (commit `084b5c00`; cert PRF-F2-W4 en `evidence/CERTIFICATES.md`). H-R4-2 (lookup per-file) registrado como OPEN. |
| F2.W5 | Resolver H-R4-2 (capa 2): lookup global `name → SymbolId` con resolución scope-aware. Cross-file edges preservados en ambos paths (per_file y full); homonimia no se inventa. | **IMPLEMENTED** (commit `3f27a31d`; **input de PRF-C2 consolidado en `44fad7a5`**). H-R4-2 cerrado. UAT de binario: ver UAT-F2-W7. |
| F2.W7 | Integrar F2.W5 en el camino real del binario (`analysis_service::build_project_graph`). El binario no invocaba las strategies modificadas por W5; usaba un mapa plano con tie-break FQN lexicográfico que violaba D33. | **IMPLEMENTED** (commit `5ce8eb1e`; **input de PRF-C2 + UAT-F2-W7 firmada**). UAT real con `cognicode-mcp` muestra `relationships_found: 4` correcto, `get_call_hierarchy` consistente. H-R4-2 cerrado en el binario. |
| F2.W8 | Errores silenciosos de lectura en `build_project_graph` (R3). 4 fuentes: `read_to_string(...).ok()?`, `TreeSitterParser::with_cache(...).ok()?`, `find_all_symbols_with_path(...).unwrap_or_default()`, `find_call_relationships(...).unwrap_or_default()`. API pública `AnalysisService::get_last_build_report()` enumera los archivos omitidos con razón clasificada (`SkipReason::{Read,Parse,Other}`); el handler MCP `build_graph` los surface como `skipped_files[]` con `path` + `reason_kind` + `reason`. | **IMPLEMENTED** (commit próximo, JOURNAL §17; **input de PRF-C2 + UAT-F2-W8-001 firmada**). H-R3 cerrado en `analysis_service::build_project_graph` + observable vía JSON-RPC `build_graph` sobre corpus con `chmod 000` y UTF-8 inválido. |
| **F2 (hito)** | **Correctitud reproducible** consolidada en PRF-C2 (`44fad7a5`). H-R4-1 y H-R4-2 cerrados; R3 cerrado en `analysis_service` (W8); mtime invalidación de cache cerrada (W9); equivalencia de aristas + reproducibilidad pineada (W10). | **ACCEPTED** (cert consolidado PRF-C2 firmado, `evidence/CERTIFICATES.md`). RELEASED pendiente del push+tag (gate del operador per directive §3). |
Las unidades de F2 siguientes dependerán de los defectos que surjan
durante la ejecución de F2.W1-W3.
Las unidades de F2 siguientes dependerán de los defectos que surjan
durante la ejecución de F2.W1-W3.

## Unidad activa

Ver `STATE.md`.

## Cómo se modifican las unidades

1. Cerrar la unidad anterior (UAT ejecutada + evidencia archivada).
2. Documentar el cierre en `JOURNAL.md`.
3. Actualizar `STATE.md` (hito, unidad, certificación, commit, siguiente).
4. Iniciar la siguiente unidad siguiendo la disciplina del README.

---

## Programa PRF vigente: F0–F7 / C0–C7

> **Origen**: directiva del operador del 2026-09-21 (ver
> `JOURNAL.md` §12, "Reconciliación administrativa"). El roadmap
> local anterior terminaba en F2 con F3..FN "pendientes de
> definición"; esa sección queda preservada por integridad
> histórica y se complementa con el desglose siguiente. Las
> unidades concretas de F3..F7 se definirán **al cierre de cada
> fase anterior**, no por adelantado.

### F3 — Vertical de análisis compartida (CLI + MCP)

**Objetivo**: demostrar que el análisis de grafo (full, per_file,
call_relationships, etc.) está disponible con la misma semántica
y la misma evidencia tanto desde el CLI `cognicode` como desde
las tools MCP (`cognicode-mcp`, `explorer-mcp`). No duplicar
lógica: ambas caras consumen el mismo puerto de aplicación.

**Criterio de salida**: para cada vertical cubierta por F2
(`graph per-file`, `graph full`, etc.), existe un UAT en CLI y un
UAT en MCP que produzcan el mismo resultado observable sobre el
mismo corpus. Las herramientas MCP que delegan al puerto de
análisis no contienen lógica de cálculo propia.

### C3 — Certificación de F3

C3 aplica el modelo `SPECIFIED → IMPLEMENTED → INTEGRATED →
ACCEPTED → RELEASED` a los requisitos de F3. Particular énfasis
en INTEGRATED: que la tool MCP funcione con el binario real,
capturando el intercambio JSON-RPC, y que el binario CLI exhiba
el mismo resultado en stdout. La evidencia se archiva en
`docs/prf/evidence/PRF-F3-*.md` y se resume en
`evidence/CERTIFICATES.md`.

### F4 — Persistencia, fuentes de verdad y aislamiento

**Objetivo**: demostrar que la persistencia (caches, índices,
grafo materializado, configs por workspace) sobrevive a reinicio,
no corrompe datos, está aislada por workspace, y que las
migraciones de esquema se aplican sin pérdida cuando corresponda.

**Criterio de salida**: existe una suite que arranca dos veces el
binario contra el mismo workspace, ejecuta el mismo flujo, y
compara el resultado bit-a-bit; existe otra suite que crea dos
workspaces independientes y demuestra que no comparten estado.

### C4 — Certificación de F4

C4 valida los requisitos de persistencia: tras reinicio, el
binario recupera su estado sin intervención; ante un esquema
obsoleto, ejecuta la migración y queda usable; los workspaces
son independientes. La evidencia incluye prueba de reinicio real
(kill+restart del proceso, no solo llamada a función) cuando
aplique.

### F5 — Seguridad, autorización por capacidad, límites

**Objetivo**: demostrar que el binario aplica sandboxing, límites
de recursos, autorización por capacidad (no por path) y
cancelación cooperativa de operaciones largas. La extensibilidad
mínima (plugins) debe ejercitarse en un caso real sin romper los
límites.

**Criterio de salida**: una UAT que pruebe (a) rechazo de acceso
fuera de capacidades declaradas, (b) timeout de operación larga,
(c) cancelación desde señal externa. La evidencia se archiva en
`docs/prf/evidence/PRF-F5-*.md`.

### C5 — Certificación de F5

C5 valida los requisitos de seguridad y límites con evidencia
ejecutable. El detalle se definirá al cierre de F5.

### F6 — Distribución, instalación, actualización, rollback

**Objetivo**: gates por SHA sobre los artefactos publicados;
instalación limpia; actualización desde una versión anterior;
rollback a una versión anterior. Todo sobre artefactos reales,
no sobre mocks.

**Criterio de salida**: una UAT que descarga un release taggeado
de un canal verificable, valida el SHA, lo instala, ejecuta un
flujo canónico, actualiza a otra versión, ejecuta el mismo flujo,
y revierte. La evidencia incluye los SHA verificados y los logs
de cada paso.

### C6 — Certificación de F6

C6 valida los requisitos de distribución e instalación con
evidencia ejecutable sobre el canal real. El detalle se definirá
al cierre de F6.

### F7 — Aceptación de release

**Objetivo**: ejecutar la aceptación completa de release para una
versión candidata, con la documentación, las plataformas
soportadas, las pruebas repetidas y la decisión formal de
publicación. F7 NO declara el release publicado por sí misma;
la publicación efectiva (push, tag firmado, distribución) sigue
requiriendo autorización explícita del operador.

**Criterio de salida**: existe un `docs/prf/RELEASE-CANDIDATE.md`
con el SHA candidato, las plataformas probadas, los UAT
ejecutados, las certificaciones C0-C6 en PASS, y la decisión
formal registrada (puede ser "READY FOR RELEASE" o "HOLD").

### C7 — Certificación de F7

C7 es el certificado consolidado del programa PRF. Sólo puede
firmarse cuando **todas** las certificaciones C0–C6 estén en
PASS, todos los frentes abiertos de F2/F3/F4/F5/F6 estén
cerrados (no diferidos), y la aceptación de release haya sido
firmada. Hasta entonces, **C7 = NO CERTIFICADO**.

