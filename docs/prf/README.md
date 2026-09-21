# Production-Ready Foundation (PRF) — README

> **Working-tree-only**: este directorio está cubierto por `.gitignore`
> (`docs/`). Sus archivos NO se commitean al repositorio ni se empujan
> al remoto. Ver `JOURNAL.md` §"Política git de `docs/prf/`" para la
> justificación completa (PRF es documentación estructurada de un
> programa interno de trabajo, análoga a ADRs/ROADMAP/CONTEXT — vive
> solo en el working tree local).

## ¿Qué es PRF?

**Production-Ready Foundation** es el programa que aterriza la promesa de
ADR-031 ("Release 1.0.0: Definition of Production-Ready") sobre los dos
productos directamente visibles para el usuario:

- **CLI `cogh`** — gestor de versiones estilo `asdf-vm` para CogniCode.
- **Servidor MCP `cognicode-mcp`** — superficie MCP que expone las
  capacidades de CogniCode a los IDEs (opencode, zcode, claude, codex).

PRF **NO** abre nuevos evolutivos. Su objetivo es estabilizar, asegurar,
reproducir, extender y verificar los productos CLI y MCP existentes.

## Alcance

En alcance:

- `cogh` (instalador, version manager, IDE adapters)
- `cognicode-mcp` (servidor MCP, catálogo de tools)
- `cognicode` (binario principal de análisis)
- `explorer-api`, `explorer-mcp` (binarios del Explorer)
- Especificaciones de `docs/specs/{cognicode-cli,cognicode-ide-adapter,cognicode-lifecycle,cognicode-plugin,portable-skill-bundle,cognicode-mcp-tools}`
- Pruebas UAT de los flujos CLI y MCP sobre los binarios reales

Fuera de alcance (preservados para fases posteriores):

- RPC, Control Plane, Backstage
- Packs, IA autónoma, federación
- Nuevos evolutivos de producto

## Estructura de documentos

| Documento | Propósito |
|---|---|
| `docs/prf/STATE.md` | Puntero de la unidad activa, certificación, commit, bloqueos |
| `docs/prf/JOURNAL.md` | Diario cronológico de unidades ejecutadas |
| `docs/prf/README.md` | Este archivo — qué es PRF y qué no |
| `docs/prf/ROADMAP.md` | Roadmap del programa: fases F0..FN y unidades W |
| `docs/prf/CERTIFICATION.md` | Cómo se certifica cada requisito (SPECIFIED → RELEASED) |
| `docs/prf/UAT.md` | Procedimiento de pruebas de aceptación con binarios reales |
| `docs/prf/TEST-PLAN.md` | Plan de pruebas de caracterización, regresión y UAT |
| `docs/prf/TRACEABILITY.md` | Matriz requisito ↔ implementación ↔ prueba ↔ commit |
| `docs/prf/evidence/CERTIFICATES.md` | Evidencias de certificación (1 entrada por cambio de estado) |

## Estado del programa

Ver `STATE.md`.

## Cómo continuar

1. Lee `STATE.md` para conocer la unidad activa.
2. Lee las dos últimas entradas de `JOURNAL.md` para contexto.
3. Ejecuta la unidad siguiendo `UAT.md` y `TEST-PLAN.md`.
4. Al cerrar la unidad, actualiza `STATE.md`, añade entrada a `JOURNAL.md`,
   y si procede, actualiza `evidence/CERTIFICATES.md` y `TRACEABILITY.md`.

## Relación con otros programas del repositorio

PRF se apoya en el trabajo previo pero **NO** duplica roadmaps. Estos
documentos siguen siendo la fuente de verdad para su respectivo programa:

- `docs/ROADMAP.md` — roadmap general del proyecto
- `~/.sddk-knowledge/CogniCode/milestones/PROG-productization.md` — programa Productization E33-E38 (cerrado E33; E34-E38 planificados)
- `docs/V1.0.0-PRE-CUT-CHECKLIST.md` — gates operacionales del tag v1.0.0
- ADR-031 — definición de "production-ready"

PRF **no reemplaza** ninguno de estos. Los complementa con foco en la
estabilización de CLI y MCP, sin abrir nuevos evolutivos.
