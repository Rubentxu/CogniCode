# Registro de decisiones y excepciones

**Estado inicial: propuestas documentales, ninguna autorización de refactor masivo ni certificación de release.** Una decisión aceptada aquí no sustituye un ADR formal del repositorio ni pruebas; la decisión operativa se registra con SHA, motivación, alternativas, consecuencias, owner y fecha.

| ID | Estado inicial | Decisión/alternativas y disparador de revisión |
|---|---|---|
| PRF-D01 | PROPOSED | CLI de análisis + MCP stdio + gestor `cogh`; `explorer-*` legacy con compatibilidad hasta inventario y decisión de migración. Reabrir si manifiesto real contradice identificación. |
| PRF-D02 | PROPOSED | Extraer solo casos de uso demostrados por CLI/MCP; no exponer `WorkspaceSession` íntegra como API estable. Reabrir tras F0 characterization. |
| PRF-D03 | PROPOSED | Un dueño de cada dato: estado canónico vs caché derivada; LSI gradual por consumidor con equivalencia y migración. Reabrir ante pérdida de capacidad de evidencia. |
| PRF-D04 | PROPOSED | Seguridad por capability y read-only mínimo; escrituras/refactor/ejecución no heredan permisos de lectura. Reabrir solo con amenaza, pruebas y consentimiento. |
| PRF-D05 | PROPOSED | No crear daemon/RPC, UI/CP, registry de packs, AI autónoma ni federación hasta C7 y consumidor real. Excepción requiere ADR que identifique WU que desbloquea, costes, tests y rollback. |
| PRF-D06 | PROPOSED | Producción requiere CI/verificación **independiente del SHA** y release probada desde bundle. Decidir si se altera política local-first actual; si no, instalar garantía independiente equivalente y documentada. |
| PRF-D07 | PROPOSED | Extensibilidad mínima por descriptor y un caso read-only; no plugins dinámicos ni DI sin consumidores. |
| PRF-D08 | PROPOSED | Archivo selectivo y reversible de planes de alto nivel. OpenSpec, ADR vigentes, evidencias y rutas usadas por automatización no se mueven sin auditoría de enlaces. |
| PRF-D09 | PROPOSED | No anunciar v1.0.0 por fecha ni por hitos legacy; conservar ADR-031/032 como precedente y reconciliar requisitos vigentes antes de corte. |

**Formato de cambio:** `ID | fecha UTC | SHA | ACCEPTED/SUPERSEDED/REJECTED | autor | hechos | tradeoffs | aprobación | rollback`. No marcar ACCEPTED por haber creado este documento.
