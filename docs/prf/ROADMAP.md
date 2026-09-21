# Production-Ready Foundation (PRF) — ROADMAP

## Fases

PRF se organiza en fases (F0..FN). Cada fase agrupa unidades de trabajo (W)
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
| F2.W2 | Errores de lectura silenciosos en `PerFileStrategy::build_full_graph` (R3). Cambiar el contrato para reportar archivos omitidos. | **ACCEPTED** (commits `be729275` código+corpus, +docs; cert PRF-F2-W2 en `evidence/CERTIFICATES.md`) |
| F2.W3 | Caracterización de equivalencia `full` vs `per_file` (R4). Las dos estrategias tienen propósitos distintos; documentar. | Pendiente |

Las unidades de F2 siguientes dependerán de los defectos que surjan
durante la ejecución de F2.W1-W3.

## Unidad activa

Ver `STATE.md`.

## Cómo se modifican las unidades

1. Cerrar la unidad anterior (UAT ejecutada + evidencia archivada).
2. Documentar el cierre en `JOURNAL.md`.
3. Actualizar `STATE.md` (hito, unidad, certificación, commit, siguiente).
4. Iniciar la siguiente unidad siguiendo la disciplina del README.
