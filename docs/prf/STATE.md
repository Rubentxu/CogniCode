# Puntero de sesión — PRF (única fuente operativa)

**Actualizado:** 2026-09-21. **Estado:** PLANIFICADO / SIN IMPLEMENTACIÓN PRF CERTIFICADA.

| Campo | Valor actual |
|---|---|
| Baseline de análisis | `0903108fc372766a69a89a76ed7f79ffff90502d` — 2026-09-20 |
| Hito activo | `F0` — inventario honesto; **NO iniciado como ejecución de ingeniería** |
| Próxima unidad | `F0.W1` — baseline reproducible y lista real de binarios/capacidades |
| Trabajo PRF en curso | Ninguno; solo documentos de planificación e histórico |
| Certificación vigente | Ninguna (`C0=NOT_RUN`, `C1..C7=BLOCKED`) |
| Último recibo de trabajo | Ninguno; no confundir esta planificación con un resultado de prueba |
| Bloqueos conocidos | CI manual; endpoint arquitectura incompleto; entrada MCP duplicada; matriz soporte y cobertura real no fijadas. Deben reproducirse en F0. |
| Siguiente acción física | Partir del HEAD actual; leer `AGENTS.md`, este puntero, ROADMAP, `JOURNAL.md`; ejecutar F0.W1, guardar comando, exit code, artifacts, SHA. |

**Actualizar al finalizar CADA unidad:** fecha UTC, SHA antes/después, hito/WU, requisito/UAT, estado `NOT_RUN|FAIL|PASS|BLOCKED`, resultados y rutas de pruebas, excepciones, siguiente unidad. Una sesión sin avance debe registrar el bloqueo con evidencia, no inventar progreso.

**Concurrencia:** la última actualización incorporada en `main` manda. Si el HEAD del checkout difiere de este baseline, comparar commits y los dos últimos recibos; nunca sobrescribir un checkpoint reciente sin reconciliarlo. No usar memoria del agente como autoridad de progreso.
