# Después de certificar la base: evolución por consumidores reales

**No forma parte de la certificación PRF ni desbloquea F0–F7.** Este documento conserva la línea temporal de la visión LSI → RPC → Control Plane y sus capacidades futuras sin anticipar infraestructura. Cada nuevo tramo requiere ADR aceptado, usuario/cliente, contrato, presupuesto de mantenimiento y UAT sobre el producto publicado.

| Tramo posterior | Prerrequisito observable | Evolución propuesta y criterio de cierre |
|---|---|---|
| E0 · Base certificada | C7 PASS en CLI/MCP | Congelar versión de API de aplicación, catálogo estable de capabilities, security/basis y tests de old-client/new-client. Mantener soporte y seguimiento de incidencias. |
| E1 · Knowledge-driven analysis en producción | Un consumidor actual necesita trazabilidad histórica fiable y F4 demuestra el almacenamiento | Introducir piezas LSI **donde aporten**: FactStore duradero, evidencia, proyección fiel, manifest temporal; cutover por operación con tests de equivalencia y rollback. No exigir migrar todo el producto para la primera capability. |
| E2 · CI y arquitectura ejecutable | E1 muestra facts/evidence reales; hay una restricción admitida y un cliente CLI/MCP que la consulta | Hacer la vertical de cambios → impacto/violación → evidencia → decisión read-only/CI, usando permisos existentes. Validar fail-closed ante fuentes parciales y compare old/new. |
| E3 · RPC de lectura mínima | Un segundo cliente *real* necesita proceso separado/reutilización/concurrencia que MCP local no cubre | Recuperar C0/C1 del paquete RPC: contrato agregado versionado, `AnalysisBasis`, 1 workspace y `AnalyzeScope`/consulta de impacto. Adaptador in-process primero; daemon/protobuf después si el caso de negocio lo requiere. Aceptación: misma semántica C7, sin doble motor. |
| E4 · Multi-workspace y escalado | E3 demuestra demanda y medidas de latencia/aislamiento bajo 2+ workspaces | Recuperar pruebas RPC C2–C6 (basis, delta frío/caliente, permisos UDS, concurrencia, activación/cancelación). Sin mezclar usuario/sesión/graph revision. |
| E5 · Control Plane read-only | E2/E3 ofrecen estado y evidencia duraderos con cursor/identidad; usuario necesita verlos | Reabrir CP1 con consumidor real de `Evaluated`, después CP2/CP3 read models y UI opcional; host Backstage solo si requiere capacidades organizativas adicionales, nunca segunda fuente de verdad. |
| E6 · Gobernanza/acciones/packs | Al menos dos consumidores independientes y política verificable | Reabrir CP commands/cases, packs/SDK, AI proposals, mejora gobernada o federación por pieza y UAT propia, preservando autoridad humana y evaluación independiente. |

**Dependencias:** E1 no requiere RPC; E3 no exige CP; E5 puede consumir MCP/HTTP existentes si es suficiente. M12 (evidencia runtime) y M14 (federación) son proyectos opt-in después de consumidores reales y benchmarks de escalabilidad, no cierre automático por completar E1–E6. Si una fase no ofrece valor, puede omitirse mediante decisión documentada sin renunciar a garantías de las demás.

## Recuperación de los tres ZIP históricos

- LSI: `openspec/changes/cognicode-living-software-intelligence/`, con especificaciones promovidas en `openspec/specs/` y pruebas históricas. **No reescribir su cierre de alcance**.
- RPC: paquete `CogniCode-RPC-Analysis-Service-Evolution-Refined-2026-09-10(1).zip`, C0–C8, 14 UAT; usar contratos, no trasladar ADR-001..007 a un namespace ya ocupado.
- CP: paquete `cognicode-post-roadmap-control-plane(1).zip`, CP0–CP7 y pruebas; los recibos CP0/CP1.0 permanecen donde están.
- Los ZIP originales son **fuentes suministradas externamente**, no archivos existentes que esta reorganización haya borrado o trasladado. El [histórico](../historico/roadmaps/THREE-ROADMAPS.md) identifica su procedencia.

**Certificación de nuevas fases:** cada una inicia `NOT_RUN`, no hereda PASS de C7 para features nuevas. Se crea un requisito/UAT explícito y una campaña de compatibilidad contra el último CLI/MCP stable.
