# ADR-031 — Release 1.0.0: Definition of Production-Ready

**Estado**: PROPUESTO
**Fecha**: 2026-08-05
**Decisores**: CogniCode Architecture Team
**Contexto**: v0.85.0 (e14-C2 mergeado PR #225); programa E28-E29 cerrado; e13-wave2 Phase 1 en rama `feat/e13-wave2-knowledge-layer-ports` (`aa23af61`)

---

## Resumen ejecutivo

**1.0.0 = production ready con verificación estricta, automatizada y basada en evidencia contra proyectos reales.** No se taggea `v1.0.0` sin prueba. La prueba es un **Release Readiness Scorecard** generado por máquina con **12 criterios duros** (cada uno con target medible, valor actual y artefacto de evidencia). Los 12 deben estar **GREEN durante 3 ejecuciones consecutivas** antes de taggear.

Este ADR es la fuente de verdad de QUÉ significa 1.0.0. ADR-032 define el sistema de validación (sandbox podman/quadlets + repos reales + scoring). Las especificaciones ejecutables viven en `openspec/specs/release-readiness-gate/spec.md` y `openspec/specs/sandbox-validation-system/spec.md`.

---

## Contexto

CogniCode ha acumulado 40+ ciclos SDDK, ~2.800 tests Rust, 401 requirements openspec, 956 escenarios y 43 MCP tools. El roadmap funcional (E28, E29, E12-E21) está mayormente completo. El riesgo restante no es de features: es de **verificación de producción** — ¿funciona de verdad contra repositorios reales, a escala, de forma estable?

La decisión de no release sin pruebas reales responde al historial del proyecto: múltiples features se implementaron, mergearon y taggearon pero con wiring roto detectado después (v0.72.5 stub executor, v0.73.6 analytics registry sin wiring, v0.81.2 D4 audit). Un gate automatizado previene esa clase de falsa confianza.

---

## Decisión

### 1. Definición de 1.0.0

**1.0.0 se alcanza cuando el Release Readiness Scorecard (12 gates) está GREEN en 3 ejecuciones consecutivas de la campaña de validación automática, contra un corpus fijo de repositorios reales pinneados.**

| # | Criterio | Target | Evidencia |
|---|----------|--------|-----------|
| G1 | e13-wave2 knowledge layer completo | 100% tasks | 3 PRs mergeados, spotter 11 familias |
| G2 | Cobertura MCP tools en sandbox | 100% de 43 tools con ≥1 scenario | matriz de cobertura auto-generada |
| G3 | MCP Health Score | ≥ 85/100 en 3 runs consecutivos | `sandbox/results-runs/<id>/health.json` |
| G4 | Correctitud (ground truth) | ≥ 90% en repos Tier-1 | scoring engine `correctitud` |
| G5 | Presupuesto de latencia | search < 500ms p95; call-graph < 2s p95 (10k LOC); analytics < 5s p95 | benchmark + latency scores |
| G6 | Consistencia | varianza run-to-run < 10% | stability.json (repeat ≥ 3) |
| G7 | Robustez | 0 crashes (panic/SIGSEGV/OOM) en campaña completa | failure class audit |
| G8 | Escalabilidad | ingest repo 100k+ LOC sin timeout/OOM | scenarios tier scale |
| G9 | Sin regresiones vs baseline | 0 unexpected failures | `orchestrator report --baseline` |
| G10 | Conformance openspec | 100% de 401 requirements verificados | auditoría de conformance |
| G11 | Documentación al día | MCP-TOOLS 43 tools verificadas; ADRs revisados; ROADMAP reconciliado | auditoría docs |
| G12 | Higiene de release | changelog v0.85.0→v1.0.0; semver limpio; sin ramas stale | auditoría git |

### 2. No-goals explícitos para 1.0.0

- Compatibilidad completa ISO GQL/Cypher
- Renderer WebGL
- Backend Neo4j de producción
- Colaboración multi-usuario

Estos permanecen como candidatos post-1.0.

### 3. Mecánica del gate

1. `just release-scorecard` ejecuta: campaña full → scoring 5D → health score → trends → compara contra baseline → emite scorecard JSON+MD.
2. Cada gate tiene estado `GREEN | AMBER | RED` + artefacto de evidencia.
3. El gate se ejecuta **cada noche** (workflow `sandbox-nightly.yml`). Tres noches consecutivas con 12/12 GREEN → candidato a tag.
4. El tag `v1.0.0` es un MINOR bump desde v0.85.0 (sin breaking changes — el semver del proyecto ya es estable).

### 4. Consecuencias

- Un gate RED no es "fail del proyecto": es un defecto tracked. Cada RED genera ciclo SDDK para cerrarlo.
- El scorecard se archiva en `docs/analysis/release-1.0.0-scorecard.md` al publicar.
- Si un criterio se revela imposible de medir (infra), se renegocia con el owner ANTES de Phase 3 baseline, no después.

---

## Alternativas consideradas

| Alternativa | Rechazada porque |
|-------------|------------------|
| Release por feature-completeness (checklist manual) | Repite el patrón de falsa confianza del historial (wiring roto detectado post-tag) |
| 1.0.0 = "todo el roadmap funcional" | El roadmap funcional no tiene criterio de calidad; infinito |
| 1.0.0 sin sandbox, solo tests unitarios | Los tests unitarios no prueban integración con repos reales ni escala |

---

## Trazabilidad

- Especificación ejecutable: `openspec/specs/release-readiness-gate/spec.md`
- Sistema de validación: ADR-032 + `openspec/specs/sandbox-validation-system/spec.md`
- Plan maestro: `docs/RELEASE-1.0.0-PLAN.md`
- ROADMAP: sección `Release 1.0.0 Program (E30)`
