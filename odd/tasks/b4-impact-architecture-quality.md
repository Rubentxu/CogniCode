# B4 — Impact, Architecture & Quality Intelligence (v0.101.14)

> **Estado**: PENDING. Desbloqueado por B3 (semantic identity + content-based invalidation).
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
| _pendiente_ | B4.W1 | _pendiente_ | _pendiente_ |
| _pendiente_ | B4.W2 | _pendiente_ | _pendiente_ |
| _pendiente_ | B4.W3 | _pendiente_ | _pendiente_ |
| _pendiente_ | B4.W4 | _pendiente_ | _pendiente_ |
| _pendiente_ | B4.W5 | _pendiente_ | _pendiente_ |
