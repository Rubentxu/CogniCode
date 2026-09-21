# Disposición de los tres programas previos (2026-09-21)

Se recibieron **tres paquetes externos ZIP** de propuesta y pruebas (LSI, RPC Analysis Service, Control Plane). Sus nombres se registran como procedencia; esta integración **no afirma haber movido unos ZIP que no estaban versionados en este repositorio**. Los contratos LSI sí están integrados en `openspec/changes/cognicode-living-software-intelligence/` y especificaciones promovidas `openspec/specs/`; permanecerán en sus rutas por los consumidores y la gobernanza de OpenSpec.

| Programa/documento | Estado respecto al PRF | Qué se reutiliza ahora | Qué se congela |
|---|---|---|---|
| LSI (`CogniCode_Living_Software_Intelligence_Docs(1).zip`) | Contratos de referencia, implementación parcial de producto | Identidad, procedencia, evidencia, proyecciones, análisis, reglas fail-closed y tests de equivalencia **donde haya consumidor CLI/MCP real** | Log reactivo general, agentes AI autónomos, packs, federación, mejora gobernada y migración masiva del almacén. No borrar implementación existente. |
| RPC (`CogniCode-RPC-Analysis-Service-Evolution-Refined-2026-09-10(1).zip`) | Diferido; su desarrollo no cierra PRF | Separación casos de uso/transportes, análisis basis, aislamiento workspace, límites/capability metadata | `cognicode-protocol`, `cognicode-rpc`, `cognicode-daemon`, gRPC, activación socket y RPC multiespacio; reabrir con 1 cliente real y ADR. |
| Control Plane (`cognicode-post-roadmap-control-plane(1).zip`) | Diferido; prototipos existentes no se retiran | Read models verificables, evidencia enlazable, incertidumbre explícita y autoridad separada | Más pantallas Backstage, acciones, timeline/UI/plugins hasta tener análisis estable con datos reales. |

**Límites importantes:** no copiar ADR-001..007 del ZIP RPC a `docs/adr` porque sus IDs colisionan con ADR preexistentes; no editar retrospectivamente `state.yaml` de LSI para declarar que se ha completado el roadmap original; no confundir el cierre de CP0/CP1.0 con el de CP0–CP7.

**Reapertura:** decisión registrada en `docs/prf/DECISIONS.md` + consumidor concreto, coste y UAT que no pueda cubrir la CLI/MCP estable. El nuevo ciclo parte de la última certificación PRF, no la sustituye.
