# ADR-047: WASM-for-Shared-Compute — Amendment to ADR-007

**Fecha:** 2026-06-24
**Estado:** ACCEPTED
**Decisión:** Permitir WASM compilado desde Rust compartido, manteniendo la prohibición de duplicar lógica backend→frontend
**Source:** SDDK explore for `sddk/wasm-graph-transforms` (engram obs-f39132b17f6be1e6)
**Amends:** [ADR-007-no-wasm-in-browser.md](ADR-007-no-wasm-in-browser.md)

---

## Context

ADR-007 (jun-12) prohibió categóricamente WASM en el browser para CogniCode. La justificación central era evitar duplicación de lógica entre backend y frontend.

El roadmap (docs/explorer-roadmap.md) lista "WASM graph transforms" como future evolution: *"Rust layout/clustering compiled to WASM for client-side compute"*. La motivación legítima: **evitar round-trips al backend para cálculos costosos** (PageRank iterativo, community detection, SCC) sobre grafos grandes (>500 nodos).

Esto crea una contradicción:
- ADR-007 prohíbe WASM en browser
- ADR-041 (Accepted) menciona "WASM behind `BENCH_ENABLE_SIGMA=1`" (feature-gated, opt-in)
- ADR-031 (Accepted) PageRank algorithm es pure-compute, candidato WASM ideal
- Sprint E6.5 (v0.12.11) type-safety permite que el mismo código se compile a múltiples targets

Una exploración SDDK (engram `sddk/wasm-graph-transforms/explore`) confirma:
- 10 algoritmos de grafo en `cognicode-core` son **pure compute** (sin I/O, sin randomness, sin side effects): PageRank, god_nodes, SCC, transitive_reduction, community detection, etc.
- **Cero infraestructura WASM** existe actualmente
- Algoritmos están acoplados a `CallGraph` (connascence 3.2 bits) — bloquea uso directo en WASM

## Decision

**Enmendar ADR-007 para distinguir tres usos de WASM:**

| Uso | Estado | Justificación |
|-----|--------|---------------|
| **WASM para rendering** (e.g., portar Bloc/GtGraphLayout) | ❌ **REJECTED** | Mantener ADR-007 §1 — frontend ya recibe tree layout del backend; duplicar sería violation |
| **WASM para business logic duplicada** (e.g., re-implementar PageRank en JS) | ❌ **REJECTED** | Mantener ADR-007 §2 — duplicación es exactamente lo que ADR-007 prohíbe |
| **WASM para shared compute** (Rust compila a 2 targets: native + wasm32) | ✅ **PERMITTED** | Único source of truth; cero duplicación; cero round-trip al backend |

**Reglas operacionales:**

1. **Single source**: Cualquier lógica en WASM DEBE existir idénticamente en el backend (compilada del mismo crate Rust). Si el código diverge entre targets, está mal diseñado.

2. **No WASM para reemplazar frontend existente**: El frontend ya usa cytoscape + elkjs para rendering. WASM solo añade compute capabilities que NO existen en el browser (e.g., PageRank sobre grafos grandes).

3. **Feature-gated, opt-in**: WASM debe ser opt-in. Si falla la carga del .wasm, el frontend cae gracefully a la implementación JS o al endpoint del backend.

4. **Connascence audit obligatorio**: Algoritmos deben desacoplarse de `CallGraph` (tipo domain) hacia un trait genérico `GraphLike` antes de compilarse a WASM. Sin esto, WASM arrastra todo el dominio al browser.

## Rationale

1. **Consistencia con ADR-041**: ADR-041 (Accepted) ya permitió "Sigma.js" detrás de `BENCH_ENABLE_SIGMA=1` — feature-gated rendering fallback. WASM-for-compute sigue el mismo principio: opt-in, fallback al backend.

2. **ROI del compilador cruzado**: Rust → WASM es ~5MB binary para ~20 algoritmos de grafo. Comparado con implementar los 10 algoritmos puros en JS (~3-6 meses de trabajo, bugs probables, pierde Rust safety): WASM es estrictamente superior en mantenibilidad.

3. **Cero round-trip para cálculos costosos**: Para un grafo de 1000 nodos, PageRank iterativo en backend = 200-500ms latency. En WASM (mismo CPU pero zero RTT) = instantáneo.

4. **Honra el espíritu de ADR-007**: La regla "no duplicar lógica" se mantiene 100%. WASM-for-shared-compute es exactamente lo opuesto — UNA fuente de verdad, DOS targets.

## Alternatives Considered

- **Opción A — Mantener ADR-007 tal cual**: Forzar implementación JS de los 10 algoritmos. Descartado: 3-6 meses de trabajo + pérdida de Rust safety + divergencia inevitable con backend.

- **Opción B — WASM via wasm-bindgen en todo cognicode-core**: Compilar todo el crate a WASM. Descartado: 30+ dependencias no-WASM-compatibles (sqlx, tokio, etc.).

- **Opción C — Shared crate con feature gate** ✅ ADOPTADO: nuevo `cognicode-graph-algos` crate, `#[cfg(target_arch = "wasm32")]` gates para compat, JSON protocol entre JS y WASM.

## Consequences

### Positive

- 10 algoritmos de grafo disponibles en browser sin duplicación
- Single source of truth mantiene correctness
- Feature-gated → zero risk para users que no quieran WASM
- Edge compute escala sin tocar el backend

### Negative

- **Build complexity**: WASM toolchain (`wasm-pack`, `wasm-bindgen`) añade ~2 min al CI build
- **Bundle size**: ~5MB `.wasm` artefacto (acceptable para dev tool, opt-in)
- **Testing**: Tests deben correr en DOS targets (native + wasm32). CI matrix: 2x build time.
- **Maintenance**: Cada algoritmo nuevo requiere verificación de WASM-compatibility (no I/O, no threads, no random)

## Affected ADRs

- **ADR-007 (amended)**: añade §3 "WASM-for-shared-compute permitted bajo single-source rule"
- **ADR-041 (no change)**: Sigma.js feature-gate es consistente con este ADR
- **ADR-031 (no change)**: PageRank algorithm es compatible (pure compute)
- **NEW ADR-048 (pending)**: Architecture del nuevo `cognicode-graph-algos` crate

## Validation

- [ ] ADR-007 §3 añadida con esta excepción
- [ ] ADR-048 (WASM graph transforms architecture) propuesto y aceptado
- [ ] POC: PageRank compilado a wasm32 ejecutable desde browser (spike)
- [ ] Build matrix funciona: `cargo build` (native) + `wasm-pack build` (wasm32)
- [ ] Tests duales: `cargo test` + `wasm-pack test --node`
- [ ] Bundle size verificado: <500KB gzipped
- [ ] Frontend integration: feature-gated, fallback graceful

## References

- `crates/cognicode-core/src/application/services/graph_analytics.rs` — algorithm source
- `crates/cognicode-core/src/application/services/community_detector.rs` — algorithm source
- ADR-007 original: [ADR-007-no-wasm-in-browser.md](ADR-007-no-wasm-in-browser.md)
- SDDK explore engram: `sddk/wasm-graph-transforms/explore` (obs-f39132b17f6be1e6)
