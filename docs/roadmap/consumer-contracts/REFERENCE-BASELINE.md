# Baseline y referencias — solo rutas constatadas

Consulta remota: 2026-10-16; `CogniCode main` SHA `88f6d02afc9c90fe7ac3c5c4de975d9b9f8c8abb`. Verificar de nuevo en el checkout antes de tocar código.

## Gobernanza y entrega (CogniCode)

| Referencia existente | Uso obligatorio |
|---|---|
| `AGENTS.md` | recuperación de estado, full pre-PR/pre-tag, no doble autoridad |
| `docs/roadmap/ROADMAP.md` | única agenda activa; §10 B1–B6 cerrados técnicamente |
| `docs/roadmap/CURRENT.md` | snapshot, puede estar obsoleto; NO agenda |
| `docs/roadmap/JOURNAL.md` | recibos append-only sobre SHAs y gates |
| `docs/roadmap/production-ready/ROADMAP-ADDENDUM.md` | programa PR existente, no sustituir |
| `docs/roadmap/adr/ADR-012-st04-cli-mcp-separation.md` | respetar elección existente; NO escindir CLI/MCP sin evidencia |
| `docs/adr/ADR-029-callgraph-projection-port-seam.md` | puerto de grafo canónico |
| `odd/tasks/b2-shared-analysis-capability-contracts.md` | B2 contratos y catálogo ya entregados |
| `odd/tasks/b3-agent-code-understanding.md` | B3 navegación, identidades, incremental ya entregados |
| `odd/tasks/b4-impact-architecture-quality.md` | B4 hallazgos, impacto y arquitectura diagnóstica |
| `odd/tasks/b5-safe-refactoring-authorized-changes.md` | B5 plan/aplicar con autoridad y rollback |
| `odd/tasks/b6-agent-ready-runtime.md` | B6 robustez y distribución CLI/MCP |
| `docs/cognicode-community-productization/13-ROADMAP-COMMUNITY-PRODUCTIZATION.md` | CP2 contratos, CP4 distribución y CP5 skills |
| `product/tools.json`, `product/profiles.json`, `product/product-manifest.json` | catálogo/contratos públicos: son fuente de verdad; no inventar segundo catálogo |
| `scripts/ci/test_roadmap_version_ratchet.py` | coherencia de versión y roadmap |
| `scripts/test-fast.sh`, `scripts/test-full.sh` | selector real y full gate existente |
| `merge-gate.pipeline.kts`, `integration.pipeline.kts` | autoridades CI existentes, no GitHub Actions nuevas |
| `release-candidate.pipeline.kts`, `release.pipeline.kts` | validar candidato y publicar los mismos bytes |

## Código existente a reutilizar (CogniCode)

- `crates/cognicode-core/src/application/analysis_basis.rs`: `AnalysisBasis`, `AnalysisOutcome`, `Analyze`, `OutcomeState`.
- `crates/cognicode-core/src/application/services/analysis_service.rs`: motor real; no segundo scanner.
- `crates/cognicode-core/src/interface/catalog.rs`: `ToolDescriptor`, filtrado de features.
- `crates/cognicode-core/src/interface/mcp/output_contracts.rs`: output schemas derivados de Rust y compatibilidad.
- `crates/cognicode-core/src/interface/symbol_identity.rs`: símbolo + precisión + procedencia.
- `crates/cognicode-core/src/domain/ports/call_graph_projection.rs`: proyección canónica.
- `crates/cognicode-core/src/domain/findings/{assembler,verifier,binding,admission}.rs`: hechos, grounding y verificación.
- `crates/cognicode-core/src/domain/evidence_kernel/`: vocabulario canónico con feature gate.
- `crates/cognicode-core/src/findings/mod.rs`: vista normalizada de hallazgos de calidad.
- `crates/cognicode-core/src/architecture/diagnostic.rs`: diagnóstico arquitectónico sin autoridad de gate.
- `crates/cognicode-core/src/impact/{composite,graph}.rs`: impactos de solo lectura.
- `crates/cognicode-core/src/incremental/content_hash.rs`: hashes de contenido.
- `crates/cognicode-core/src/refactor/`: autoridad separada para refactorización; no incorporarla a lectura.
- `crates/cognicode-core/src/assurance_export/envelope.rs`: contrato `assurance-evidence/v1` para consumidor externo; no cambiarlo como efecto colateral.

## Consumidor afectado (CogniCode Guard)

- `crates/guard-cognicode/{Cargo.toml,src/calculator.rs}`: dependencia fijada `v0.101.10`, adaptador real.
- `crates/guard-application/src/{scan.rs,graph_adapter.rs,metric_provider.rs,rule_bridge.rs,scan_runner.rs}`: modelos y pérdidas de cobertura/precisión observadas.
- `crates/guard-cli/src/main.rs` y `crates/guard-mcp/src/main.rs`: planes divergentes y `graph: None` en MCP.
- `crates/guard-domain/src/{observation.rs,admission.rs}`: `Grounded/Ungrounded`, `AdmittedFinding`.
- `crates/guard-policy/src/{admission.rs,policy.rs}`: decisión/autoridad; no debe delegarse al core.

## Advertencias de estado no resueltas

- `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md` está referenciado por §10 pero **no estaba accesible** vía GitHub `main`. No inventar su contenido: verificar y reparar el enlace bajo un WU de higiene de CU-1 solo si persiste.
- El `CURRENT.md` leído es snapshot del 15-10-2026 y menciona v0.101.17 aún sin publicar, mientras la release v0.101.17 ha aparecido publicada el 16-10-2026. Reconstruir el snapshot, no copiar su estado antiguo.
- `C8-R` figura pendiente de firma humana. Nunca convertirlo automáticamente en PASS al ejecutar este programa.
- Las capacidades y pruebas descritas en estos docs son objetivos, **no resultados observados en CI**.
