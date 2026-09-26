# Matriz de priorización

Escala: impacto 1–5, esfuerzo XS/S/M/L/XL. La prioridad combina riesgo para producción, capacidad de bloquear release y dependencia sobre trabajo posterior.

| Hallazgo | Impacto | Esfuerzo | Prioridad | Fase | Clasificación |
|---|---:|---:|---|---|---|
| C8 certificada sobre estado local no reproducible | 5 | S | P0 | 1–2 | Quick win + crítico |
| Expediente C8 referenciado pero ausente | 5 | XS | P0 | 1 | Quick win |
| Drift ROADMAP/JOURNAL/MAINTENANCE/OpenSpec | 4 | S | P0 | 1 | Quick win |
| `.gitignore` permitió perder source/documentación | 5 | S | P0 | 1 | Quick win |
| CP puede devolver `evaluated` sin demostrar las 3 constraints esperadas | 4 | S | P0 | 2 | Crítico |
| e91 Graph Insights: G5 rojo, 367 s p95 vs 5 s | 5 | L | P0 | 2 | Proyecto estratégico crítico |
| `application -> infrastructure` | 5 | L | P1 | 2–3 | Proyecto estratégico |
| `application -> interface::mcp` | 5 | M | P1 | 2–3 | Proyecto estratégico |
| CP no protege boundary de application | 5 | S/M | P1 | 2 | Quick win arquitectónico |
| Múltiples rutas con semántica de graph build | 4 | L | P1 | 3 | Deuda técnica estratégica |
| HandlerContext de superficie muy amplia | 4 | L | P1 | 3 | Deuda técnica estratégica |
| WorkspaceSession compone implementaciones concretas | 4 | L | P1 | 3 | Proyecto estratégico |
| AnalysisService concentra demasiadas responsabilidades | 4 | XL | P1 | 3 | Deuda técnica estratégica |
| Cobertura core 74,15% y report-only | 3 | M | P2 | 2 | Deuda técnica |
| PR-CI deliberadamente parcial y no sensible a paths | 4 | M | P1 | 2 | Proyecto estratégico DevEx |
| Flake/state pollution en tests seriales | 3 | S/M | P2 | 2 | Deuda técnica |
| RUSTSEC-2024-0437 vía OTel/protobuf | 4 | M | P1 | 2 | Deuda security |
| GitHub Actions no pinneadas por SHA | 3 | S | P2 | 1 | Quick win security |
| Sin Dependabot/Renovate | 2 | S | P3 | 1 | Quick win maintenance |
| Workspace `0.99.0` sin release `v0.99.0` | 4 | S | P1 | 2 | Crítico release |
| `.env` trackeado aunque sin secretos observados | 2 | XS | P3 | 1 | Quick win higiene |

## Cuadrantes impacto/esfuerzo

### Alto impacto / bajo esfuerzo
- restaurar y versionar expediente C8;
- reconciliar documentación;
- guard de referencias versionadas;
- guard de clean-clone certification;
- ampliar constraints CP a boundary de application;
- pinear Actions en release.

### Alto impacto / alto esfuerzo
- e91;
- retirar dependencias inversas de application;
- unificar semántica de graph build;
- profundizar WorkspaceSession/AnalysisService/HandlerContext.

### Impacto medio / bajo esfuerzo
- automatización de dependencias;
- sanear `.env`;
- estabilizar tests seriales identificados.

### Impacto medio / esfuerzo medio
- política de cobertura;
- CI adaptativa por paths;
- migración OTel 0.28.
