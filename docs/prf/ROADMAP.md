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

## Unidad activa

Ver `STATE.md`.

## Cómo se modifican las unidades

1. Cerrar la unidad anterior (UAT ejecutada + evidencia archivada).
2. Documentar el cierre en `JOURNAL.md`.
3. Actualizar `STATE.md` (hito, unidad, certificación, commit, siguiente).
4. Iniciar la siguiente unidad siguiendo la disciplina del README.
