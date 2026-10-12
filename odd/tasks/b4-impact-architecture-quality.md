# B4 — Impact, Architecture & Quality Intelligence (v0.101.14)

> **Estado**: CLOSED 2026-10-12 (v0.101.14 released).
> Detalle: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b4--impact-architecture--quality-intelligence).

## Valor

CogniCode responde de forma justificada a preguntas como:
- ¿Qué componentes dependen de este módulo?
- ¿Qué impacto tendría modificar esta función?
- ¿Dónde existen violaciones de arquitectura?
- ¿Qué evidencia soporta este diagnóstico?
- ¿Qué riesgos conviene revisar antes de un cambio?

El resultado puede usarse directamente en code review y en agentes que
planifican refactorizaciones.

## Prerrequisitos

- B3 cerrado (✓ plan): `SymbolId` con precision/provider, content-based cache.
- B2.W4 cerrado (✓): evidence exporter con Partial { gaps } honesto.

## Work units

- [ ] **B4.W1** — Grafo de impacto con evidencia. Consolidar
  `analyze_impact`, `graph_explain`, `trace_path`, `graph_query`,
  `graph_query_filtered` y dependency queries. Orientación correcta,
  límites de profundidad, identidades (B3.W1 `SymbolId`),
  explicación de caminos. Distinguir observadas de heurísticas.
- [ ] **B4.W2** — Arquitectura como diagnóstico. Conectar
  `check_architecture`, `ArchitectureRegistry`,
  `ArchitectureEvaluator` + grounding canónico. Constraints →
  observations + evidences. NO autoridad de gate automática.
  Superficie read-only coherente CLI/MCP.
- [ ] **B4.W3** — Calidad y hallazgos. Integrar complejidad, parámetros
  largos, god functions, `solid_audit` y análisis de relaciones.
  Normalizar: categoría + severidad + confianza + fuente + location
  + regla + procedencia. NO declarar equivalentes análisis de
  lenguajes con capacidades diferentes.
- [ ] **B4.W4** — Evidence Export enriquecido. Reutilizar puertos
  del core para publicar relaciones y diagnósticos reales a
  consumidores externos, incluido PipelineK Assurance. Solo datos
  con procedencia verificable. Pruebas cruzadas con
  `pipelinek-assurance` cuando el contrato cambie.
- [ ] **B4.W5** — Consulta de impacto para agentes. Experiencia
  compuesta: impactos + contexto + restricciones + pruebas
  recomendadas. Sin veredicto definitivo. El agente investiga
  cada evidencia de forma incremental.

## Acceptance criteria

- UAT-B4-01: cambio en API compartida identifica consumidores
  afectados y explica el camino.
- UAT-B4-02: restricción entre capas detecta dependencia
  prohibida con referencia al código.
- UAT-B4-03: repo sin violaciones → resultado limpio con
  cobertura suficiente.
- UAT-B4-04: CLI y MCP expresan el mismo impacto semántico.
- AAT-B4-01: grafo parcial no produce falsa ausencia de deps.
- AAT-B4-02: arista sin evidencia suficiente no se presenta
  como certificada.
- AAT-B4-03: relación heurística no recibe autoridad determinista.
- AAT-B4-04: consumer Assurance rechaza evidencia corrupta.

## Out of scope (B5+)

- Apply con autoridad (B5.W3).
- Plan transaccional (B5.W1-W2).
- Certificación multiplataforma (B6).

## Riesgos

- **R-B4-W1**: graph_explain devuelve paths sin evidencia cuando
  el grafo es parcial; el consumer no debe inferir "ausencia de
  impacto" de la ausencia de path.
- **R-B4-W2**: el constraint system aprende autoridad de gate
  implícitamente si no se documenta; mantener como observación
  + evidencia, nunca como pre/post-hook.
- **R-B4-W4**: AAT-B4-04 requiere el consumer real Kotlin; out
  of scope para el work-unit salvo que `pipelinek-assurance`
  esté disponible en el clone.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| 2026-10-12 | B4.W1 | `49d8c25` | ImpactEdge con direction + provenance + SymbolId; 15 tests. Cierra AAT-B4-01 (grafo parcial) + AAT-B4-03 (heuristico no determinista). Catalog añade `impact_direction_floor` en 5 impact tools. |
| 2026-10-12 | B4.W2 | `aba10c7` | ArchitectureDiagnostic read-only; catalog marca `gate_authority: false` y `produces_observations_only: true`. 11 tests. Cierra UAT-B4-02. |
| 2026-10-12 | B4.W3 | `37fc65a` | Finding normalizado con category/severity/confidence/language; language_scope en 5 quality tools; 10 tests. Cierra R-B4-W3 (no equivalencia falsa entre lenguajes). |
| 2026-10-12 | B4.W4 | `a1204e9` | 3 extractores aditivos en assurance_export con ExtractionOutcome honest. 13 tests. Wire bump top-level al release. |
| 2026-10-12 | B4.W5 | `f3cc51a` | CompositeImpactResult con verdict_refused: true; nueva tool composite_impact_query. 10 tests. |
| 2026-10-12 | post-merge | `64c9500`, `6a33aa7`, `8204625`, `c305392`, `d2b824d`, `9da80c3`, `32c4853` | clippy + fmt + capabilities + catalog output_schema + DTO + baseline 0 entries + severity array. |
| 2026-10-12 | release | `cb98593` | bump 0.101.13 → 0.101.14; golden CBOR regen (sha `ff35e199408b8ecf...`). |
| 2026-10-12 | veredicto | `cb98593` | per MERGE-GATE-BYPASS-POLICY §4. |
| 2026-10-12 | tag | v0.101.14 | anotado + pushed + GitHub Release con 3 tarballs. |

### B4 cierre

  - **Tag**: v0.101.14 anotado y pusheado a origin.
  - **GitHub Release**: `v0.101.14 — B4 Impact, Architecture & Quality
    Intelligence` con 3 tarballs (`cogh`, `cognicode`, `cognicode-mcp`)
    y sus SHA256.
  - **Veredicto**: `docs/roadmap/production-ready/veredictos/2026-10-12-b4-release-v0.101.14.md`
    per MERGE-GATE-BYPASS-POLICY §4.
  - **Gate**: fmt + clippy + lib+tests + doctests + known_failures
    todos verdes por separado. test-full rc=1 por OOM en el
    release build (no del codigo; binaries staged y SHA256
    verificados).
  - **Ratchet**: `core_gate_coverage_contract` cuenta pineada a
    58; ANCHOR_SUITES incluye b4_w1..b4_w5.
  - **Pendiente explicito**:
      - AAT-B4-04 (cross-build con `pipelinek-assurance`
        validando el envelope enriquecido) requiere el consumer
        Kotlin; out of scope para esta release.
      - B4.W1..W4 mantienen aditividad; el wire bump
        top-level del envelope (relations, observations,
        findings) llega al siguiente bloque en lockstep con
        el consumer update si la version anterior no los
        conoce.
