# Escenarios mínimos de UAT, E2E y AAT efímeros (por bloque)

Todos usan **artefacto o binarios reales del SHA del bloque** y ruta temporal fuera del repo. Herramienta exacta y args obtenidos primero desde `--help`, `product/tools.json` y catálogo MCP vigente: NO escribir scripts con comandos inventados. La ejecución corresponde al agente, no a este paquete.

| Hito | UAT (un recorrido consumidor) | AAT (un negativo decisivo) | E2E (solo frontera imprescindible) | Regresión mínima a retener |
|---|---|---|---|---|
| **CU-1** | Construir fixture Rust con homónimos+grafo; comparar `AnalysisOutcome` normalizado SDK/CLI/MCP y sus gaps. `UAT-CU-1` | Desactivar capacidad grafo o perfilar backend no soportado y comprobar `Partial/Unsupported`, jamás vacío+Complete. `AAT-CU-1` | Arrancar `cognicode-mcp` desde bin real y realizar JSON-RPC `initialize → tools/list → call`. `E2E-CU-1` si no lo cubre el UAT | Test de conversión Outcome y de la fachada pública |
| **CU-2** | Guard consumo aislado: falla técnica real → evidence → decision `Warn/Block` solo cuando verificada + admisión válida. `UAT-CU-2` | `EvidenceId` inventado, snapshot cruzado y firma forjada -> nunca `Block`. Agrupar mutantes en un único AAT parametrizado, no tres E2E. `AAT-CU-2` | E2E real Rust->Guard en RC; puede integrarse dentro del UAT, **no duplicarlo** | Scope mismatch, evidence missing y verificación de firma en Guard |
| **CU-3** | Agente solo recibe catálogo, localiza símbolo, explica impacto y usa `next_action` por CLI y MCP; comparar semántica. `UAT-CU-3` | Cursor manipulado/caducado + `Read` apuntando a mutación: error explícito sin permisos. `AAT-CU-3` | MCP stdio limpio en el UAT; no repetir si ya se probó | Ley de paginación y no-upcast de permiso |
| **CU-4** | Cold→warm→edición mismo mtime/tamaño→new build features: resultados correctos y cambio de cache key. `UAT-CU-4` | Missing asset, checksum corrupto y versión N−1 incompatible: RC/release rechaza false green. `AAT-CU-4` | Descargar release remoto, instalar CLI/MCP/cogh y probar una consulta real de Guard. `E2E-CU-4` | Snapshot key anti-stale + inventory completeness |

## Criterio de éxito y no éxito

- Igualdad de conjuntos normalizados: `symbol identity`, `observation kind`, `scope`, `coverage`, `reason`, `precision`, `evidence references`. Ignorar únicamente timestamps/log order no contractuales.
- Números de línea del source, byte spans, tool id y `snapshot` se conservan cuando forman parte del contrato. Un `Unknown` no puede transformarse en `[]` o `Clean`.
- Las respuestas paginadas concatenadas reconstruyen lo mismo que la lectura completa; cursor alterado/expirado no lee otro workspace.
- La ausencia de binario, cliente externo Guard no disponible, errores de build, falta de permiso GitHub o runner sin toolchain dan `BLOCKED`/`FAIL` con causa y no cuentan como UAT aprobado.
- **Limpieza:** los guiones efímeros se eliminan al cerrar CU-1..CU-4, una vez registrada su evidencia y reemplazadas garantías críticas. No versionar screenshots, dumps, colecciones gigantes ni falsos tests `#[ignore]` de aceptación.
