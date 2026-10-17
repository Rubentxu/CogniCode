# ADR-CU-001 — Evolucionar la API core, no construir un segundo SDK

**Estado:** PROPOSED · **Bloque:** CU-1 · **Autoridad:** este ADR requiere revisión/aceptación en PR; no reemplaza ADR-029 ni ST-04.

## Contexto y prueba

CogniCode ya publica `cognicode-core`, `AnalysisBasis`, `AnalysisOutcome`, `Analyze`, `WorkspaceSession`, `CallGraphProjectionPort` y una identidad de símbolo precisa. Guard tiene un `RealCalculator` que utiliza el motor real, pero sus entrypoints CLI/MCP no comparten preflight ni construcción de grafo. Una nueva crate SDK con tipos espejo introduciría drift y crecimiento sin capacidad nueva.

## Decisión propuesta

- Añadir una **facade de consumo estrecha en `cognicode-core`** usando los ADTs existentes. `Analyze` es la autoridad; adapters CLI/MCP no poseen algoritmos de análisis.
- Mantener la frontera FP: capa dominio devuelve ADTs y proyectores puros; capa aplicación compone casos de uso; adaptador de filesystem/indexado se concentra en infraestructura; CLI/MCP presentan DTOs.
- Exponer lo mínimo necesario para un consumidor real. Incorporar `cognicode-sdk` separado **solo** si medición de dependencias/compilación demuestra que extraer una biblioteca ligera aporta ventaja real y puede migrarse sin doble autoridad.
- La runtime API declara features reales y versión proveedor. Retener `v0.101.10` N−1 del consumer Guard hasta aceptación formal de la migración.

## Alternativas rechazadas

1. **Nuevo `sdk` primero**: otra superficie pública antes de medir necesidades; rechazada de inicio.
2. **CogniCode Guard reconstruye el grafo**: viola dueño único; rechazada.
3. **CLI/MCP implementan análisis cada uno**: resultados divergentes; rechazada.
4. **RPC/gRPC/daemon adicional**: fuera de alcance, solo si aparece segundo consumidor que precise proceso/aislamiento real (véase E3 en roadmap).

## Consecuencias y reversión

Positiva: adaptador pequeño, menos mantenimiento y compatibilidad fiable. Riesgo: crecimiento de API pública, mitigado por tipos mínimos + N−1 tests. Rollback: retirar la nueva facade sin destruir el servicio real; mantener antiguos DTOs durante deprecación.

**Verificación:** consumidor externo compila + equivalencia semántica SDK/CLI/MCP en snapshot controlado.
