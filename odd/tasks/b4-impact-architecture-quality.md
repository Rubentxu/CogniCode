# B4 — Impact, Architecture & Quality Intelligence (v0.101.14)

> **Estado**: PENDING. Desbloqueado por B3.
> Detalle espeficado: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b4--impact-architecture--quality-intelligence).

## Valor

CogniCode responde de forma justificada a: ¿qué depende de este módulo?, ¿qué impacto tendría modificar esta función?, ¿dónde existen violaciones de arquitectura?, ¿qué evidencia soporta este diagnóstico?, ¿qué riesgos revisar antes de un cambio? El resultado se usa directamente en code review y agentes que planifican refactorizaciones.

## Prerrequisitos

- B3 cerrado: navegación semántica unificada + análisis incremental fiable.

## Work units

- [ ] **B4.W1** — Grafo de impacto con evidencia. Consolidar `analyze_impact`, `graph_explain`, `trace_path`, `graph_query`, `graph_query_filtered`. Orientación correcta de aristas, dirección de impacto, límites de profundidad, identidades, caminos explicados. Distinguir observado de heurístico.
- [ ] **B4.W2** — Arquitectura como diagnóstico. Conectar `check_architecture`, `ArchitectureRegistry`, `ArchitectureEvaluator`, grounding canónico. Constraints generan observaciones y evidencias (NO autoridad de gate). Superficie read-only coherente CLI/MCP.
- [ ] **B4.W3** — Calidad y hallazgos. Integrar complejidad, parámetros largos, god functions, `solid_audit`, análisis de relaciones. Normalizar: categoría, severidad, confianza, fuente, localización, regla/algoritmo, procedencia. NO declarar equivalentes análisis de lenguajes con capacidades distintas.
- [ ] **B4.W4** — Evidence Export enriquecido. Reutilizar puertos del core para publicar relaciones y diagnósticos reales. Capabilities incompletas siguen declarándolo. Pruebas cruzadas con `pipelinek-assurance` cuando cambie el contrato.
- [ ] **B4.W5** — Consulta de impacto para agentes. Experiencia compuesta que reúne impactos + contexto + restricciones + pruebas recomendadas sin inventar veredicto. Investigación incremental por evidencia.

## Acceptance criteria (resumen)

- UAT-B4-01..04: API compartida → consumidores + camino; violación de capa detectada; repo limpio con cobertura; CLI/MCP equivalencia semántica.
- AAT-B4-01..04: grafo parcial NO produce falsa ausencia; arista sin evidencia NO certificada; relación heurística NO determinista; consumer Assurance rechaza evidencia corrupta.
- `bash scripts/test-full.sh` exit 0.
- Roundtrip con `pipelinek-assurance` consumer real.

## Out of scope

- Plan/apply de refactorización (B5).
- Certificación multiplataforma (B6).

## Riesgos

- **R-B4-W1**: el grafo de impacto crece con el workspace; budgets deben ser honestos. Mitigación: declarar límites en metadata, no inflar precisión.
- **R-B4-W2**: ArchitectureEvaluator como gate es lo que ya prohibió el contrato (PRF). Mantener read-only por diseño.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| _pendiente_ | B4.W1 | _pendiente_ | _pendiente_ |
| _pendiente_ | B4.W2 | _pendiente_ | _pendiente_ |
| _pendiente_ | B4.W3 | _pendiente_ | _pendiente_ |
| _pendiente_ | B4.W4 | _pendiente_ | _pendiente_ |
| _pendiente_ | B4.W5 | _pendiente_ | _pendiente_ |

## Cross-block signals

B4.W1 + B4.W4 son prerrequisito de B5 (refactor consume grafo de impacto y evidence export).