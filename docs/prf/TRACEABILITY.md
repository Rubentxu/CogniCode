# Trazabilidad temporal LSI → RPC → Control Plane → PRF

Cada fila distingue **origen histórico** de **obligación actual**. No hacer depender un requisito estable de implementar un roadmap entero. Fuente histórica: [índice](../historico/roadmaps/THREE-ROADMAPS.md).

| Origen | Requisito recuperable | Obligación PRF y fase | Diferido/descartado del corte |
|---|---|---|---|
| LSI M0–M3 | Golden baseline, identidad, equivalencia de proyecciones, snapshots | F0/F2/F4: corpus, identity/basis y coherencia real | Migración indiscriminada a facts sin consumidor. |
| LSI M4–M6 | Provider tiers, Detector IR, provenance, findings verificados | F2/F3/F5: solo herramientas CLI/MCP estables que prometan esos resultados | Nuevos backends/analyzers por variedad tecnológica. |
| LSI M7–M8 | Unknown/partial, budgets, cambios y work scheduling | F2/F5/F6: resultados honestos, límites y smoke CI real | Event bus distribuido/reactivo y sistema de CI autónomo. |
| LSI M9–M13 | Policy/authority, propuestas, pruebas de promoción, AI | F5: asegurar mutaciones/refactor actuales; conservar modelos existentes | AI autónoma, autofix, auto-promoción, históricos shadow en release base. |
| LSI M12/M14 | Evidencia runtime/SBOM/federación | F6: auditoría dependencias y release provenance donde se anuncie | Inteligencia de ejecución y federación multi-repo nuevas. |
| RPC C0–C8 | Contratos neutrales, analysis basis, workspace separation, API compatibility | F3/F4/F5: casos de uso y metadata como contratos **internos** | `.proto`, gRPC, UDS, socket activation, daemon, multiworkspace RPC. |
| Control Plane CP0–CP7 | read models seguros, incertidumbre, permisos y links de evidencia | F2/F3/F5: DTOs de análisis estables y diagnósticos consumibles | Backstage/React CP, commands/cases/timelines/UI y nuevos plugins. |
| Roadmap release E30/E31 | suite core, conformance, scorecard, distribution, rollback | F0/F6/F7: reaprovechar infra y baseline sin fingir que es UAT PRF | Criterios centrados en UI/no core reclasificados como fuera del corte base. |

**Regla de cierre:** una columna anterior no pasa a `ACCEPTED` por tener un módulo implementado. Cada WU añade filas más específicas `source-requirement | test | use-case | artifact | gate | SHA` y documenta exclusión de capacidades anunciadas ANTES de lanzar la RC.
