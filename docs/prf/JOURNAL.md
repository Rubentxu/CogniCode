# Diario PRF — cronológico y append-only

Este fichero contiene recibos **de trabajo y de decisiones**, no una narración subjetiva. Cada nueva sesión añade un apartado `## YYYY-MM-DDTHH:MMZ | F#.W# | SHA`; jamás sobrescribir un recibo anterior ni convertir un `NOT_RUN` en `PASS` sin ejecución. Actualizar `STATE.md` como puntero de lectura; incluir enlaces exactos a commits y anexos de pruebas cuando existan.

## 2026-09-21 | PRF.DOC.0 | Preparación documental

- **Baseline examinado:** `0903108fc372766a69a89a76ed7f79ffff90502d`.
- **Alcance realizado:** ordenación de planes históricos, contrato PRF, secuencia, matriz de certificaciones/UAT, instrucciones de recuperación para AGENTS.md.
- **Código de producto modificado:** no.
- **Tests de aplicación/UAT ejecutados:** NOT_RUN.
- **Certificación lograda:** ninguna.
- **Siguiente unidad:** `F0.W1` — inventario de ejecutables, clientes, versiones, capacidades y baseline reproducible.
- **Riesgo:** el histórico LSI/RPC/CP conserva especificaciones activas en OpenSpec; no borrarlas ni interpretarlas como producto entregado.

## Plantilla obligatoria para nuevos recibos

```markdown
## <UTC> | <F#.W#> | <SHA final>
- HEAD inicial / HEAD final:
- Objetivo y requisitos (IDs):
- Cambios (archivos y por qué):
- Evidencia: comando exacto, entorno, exit code, log/artefacto, hash; UAT y cliente utilizado:
- Resultado de cada gate: PASS / FAIL / BLOCKED / NOT_RUN:
- Contrato CLI/MCP antes/después, seguridad, regresión y reversión:
- Drift documental detectado/resuelto:
- Pendientes/riesgos y siguiente WU:
```
