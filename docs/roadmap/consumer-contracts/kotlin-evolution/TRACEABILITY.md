# Trazabilidad de debilidades a código y autoridad documental

**Base analizada:** `Rubentxu/CogniCode` `main@88f6d02afc9c90fe7ac3c5c4de975d9b9f8c8abb`, 2026-10-16. La implementación puede cambiar: los paths son puntos de inspección, no contratos para siempre.

| Debilidad constatada | Código actual observado | Requisito nuevo y hito | Documento existente (no reabrir ni editar histórico) |
|---|---|---|---|
| Kotlin no aparece como lenguaje ni ext `.kt/.kts` | `crates/cognicode-core/src/domain/value_objects/language.rs` | K-SYM-01 K1 | `product/languages.json` (generado); `docs/prf/specs/SPEC-ANALYSIS.md` (hist.) |
| Kotlin no tiene `LanguageConfig` | `crates/cognicode-core/src/infrastructure/parser/language_config.rs` | K-SYM-02..04 K1 | `docs/roadmap/ROADMAP.md` |
| Tree-sitter sin gramática Kotlin y extractor con nodo singular | `crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs`; `Cargo.toml` | K-SYM-03..06 K1 | `docs/adr/ADR-029-callgraph-projection-port-seam.md` |
| PerFile reporte filtra solo `rs|py|js|ts` | `crates/cognicode-core/src/infrastructure/graph/strategy.rs` | K-COV-01..08 K0 | `docs/prf/specs/SPEC-ANALYSIS.md`; `docs/roadmap/production-ready/METRICS-AND-ACCEPTANCE.md` |
| Builder alternativo omite errores con `continue` | `crates/cognicode-core/src/infrastructure/graph/strategy.rs` | K-COV-02..04 K0 | `docs/adr/ADR-029-callgraph-projection-port-seam.md` |
| Resolución por nombre sin firma/receiver completo | `crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs` | K-REL-01..07 K2 | `docs/adr/ADR-016-renderer-neutral-semantic-projections.md` |
| Kotlin sin LSP | `crates/cognicode-core/src/domain/value_objects/language.rs` `lsp_server_binary`; `infrastructure/lsp/providers/composite.rs` | K-PROV-01..05 K3 | `docs/prf/specs/SPEC-MCP.md`, `docs/adr/ADR-018-evidence-gated-product-operability.md` |
| S3 reservado, sin productor | `crates/cognicode-core/src/domain/traits/code_intelligence.rs` | K-PROV-06..08 K4 condicional | `docs/adr/ADR-029-callgraph-projection-port-seam.md` |
| Catálogo de tools anuncia requisitos diferentes | `product/tools.json` | K-API-01..08 K4 | `docs/cognicode-community-productization/13-ROADMAP-COMMUNITY-PRODUCTIZATION.md` CP0, CP2, CP5 |
| No confundir projection visual con semántica observada | `crates/cognicode-core/src/application/fact_bridge/{tree_sitter_facts,lsp_facts}.rs` | K-REL-02, K-API-05 K2/K4 | `docs/adr/ADR-016-renderer-neutral-semantic-projections.md` |
| Distribución de binarios/versiones | `release.pipeline.kts`, `release-candidate.pipeline.kts`, `product/product-manifest.json` | DoD de cada Kx | `docs/adr/ADR-034-cognicode-distribution-package.md` |
| Full gate obligatorio, fast solo inner loop | `scripts/test-fast.sh`, `scripts/test-full.sh`, `AGENTS.md` | `TEST-STRATEGY.md` | `docs/roadmap/ROADMAP.md`, `AGENTS.md` |

## Autoridades y contradicciones detectadas

- **AGENDA:** `docs/roadmap/ROADMAP.md` es la autoridad; no usar `docs/prf/` para crear otra agenda. `CURRENT.md` es snapshot y **está atrasado** respecto de la release `v0.101.17` publicada el 2026-10-16 con 8 assets. No sobrescribirlo sin recomputar sobre HEAD local actual.
- **Referencia rota:** `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md` aparece enlazado en el roadmap principal, pero **no existe** en el árbol `main@88f6d02a`. La integración no crea ese nombre de fichero como truco para verde; se reconcilia con decisión de mantenimiento y fuente existente.
- **SERVICIO:** `product/languages.json` v0.101.17 anuncia las familias soportadas con evidencia, no Kotlin. `product/tools.json` contiene los nombres reales de herramientas como `get_file_symbols`, `query_symbol_index`, `get_symbol_code`, `find_references` y requisitos de grafo. El agent debe consultar el runtime; el catálogo persistido es referencia, no prueba de funcionamiento por lenguaje.
- **SKILL:** `https://github.com/Rubentxu/agent-skill/tree/main/skills/delivery-first-testing` es la referencia versionada de skills/lentes, `references/lens-registry.json` marca cuáles están `ready`. El router es puro y no reemplaza el test runner.

## Documentos existentes verificados

`docs/roadmap/ROADMAP.md`; `docs/roadmap/CURRENT.md`; `docs/roadmap/JOURNAL.md`; `docs/roadmap/MAINTENANCE.md`; `docs/prf/specs/SPEC-ANALYSIS.md`; `docs/prf/specs/SPEC-CLI.md`; `docs/prf/specs/SPEC-MCP.md`; `docs/prf/specs/SPEC-STATE.md`; `docs/adr/ADR-016-renderer-neutral-semantic-projections.md`; `docs/adr/ADR-018-evidence-gated-product-operability.md`; `docs/adr/ADR-029-callgraph-projection-port-seam.md`; `docs/adr/ADR-034-cognicode-distribution-package.md`; `docs/cognicode-community-productization/13-ROADMAP-COMMUNITY-PRODUCTIZATION.md`; `docs/roadmap/production-ready/METRICS-AND-ACCEPTANCE.md`; `docs/roadmap/production-ready/ROADMAP-ADDENDUM.md`.

Los documentos `ADR-K00*` de este paquete son **nuevos y propuestos**; no atribuirles aprobación previa.
