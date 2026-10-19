# B4.W1 — Impact graph with evidence and provenance (v0.101.14)

> **Estado**: CLOSED 2026-10-12 (`49d8c25`); B4 publicado en v0.101.14.
> Estado derivado de `docs/roadmap/ROADMAP.md` §10, que es la autoridad.
> Roadmap padre: [`b4-impact-architecture-quality.md`](./b4-impact-architecture-quality.md).

## Objetivo

Unificar las 6 tools de impacto (`analyze_impact`, `graph_explain`,
`trace_path`, `graph_query`, `graph_query_filtered`, dependency queries)
bajo un mismo shape JSON que distinga explícitamente:

- **dirección**: caller→callee / callee→caller / bidireccional / no dirigido.
- **procedencia**: observed | inferred | heuristic — AAT-B4-03: una arista
  heurística NO recibe autoridad determinista.
- **cobertura completa vs parcial**: un grafo parcial (parsers incompletos,
  archivos skipped) NO produce falsa ausencia de dependencias
  (AAT-B4-01).

## Tipos (wire shape)

```rust
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    CallerToCallee,
    CalleeToCaller,
    Bidirectional,
    Undirected,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Observed,
    Inferred,
    Heuristic,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ImpactEdge {
    pub from: SymbolId,
    pub to: SymbolId,
    pub direction: Direction,
    pub provenance: Provenance,
    pub weight: f64,
    pub evidence: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ImpactPath {
    pub edges: Vec<ImpactEdge>,
    pub complete: bool,
    pub coverage: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ImpactResult {
    pub paths: Vec<ImpactPath>,
    pub truncated: bool,
    pub total_visited: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ImpactLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub direction: Direction,
}

pub fn build_impact_result(
    edges: impl IntoIterator<Item = ImpactEdge>,
    limits: &ImpactLimits,
) -> ImpactResult { ... }
```

## Acceptance criteria (B4.W1 subset)

- AAT-B4-01: grafo parcial marca `complete=false` y `coverage < 1.0`.
- AAT-B4-03: arista con `provenance=Heuristic` no se serializa como
  `Observed`.
- Paridad B3.W1: `from`/`to` son `SymbolId` con `byte_range`, NO
  `(line, column)`.
- Catálogo: `impact_direction_floor` declarado en las 5 tools de impacto.
- `depth_limit` honrado: queries con `max_depth=2` no atraviesan depth 5.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| _pendiente_ | B4.W1 | _pendiente_ | _pendiente_ |
