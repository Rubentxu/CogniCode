# Roadmap — CogniCode Rules Pro

> **Fecha**: 11 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Planificación estratégica

---

## 1. Visión

**CogniCode como plataforma de inteligencia de ingeniería**, no solo un linter. El objetivo es transformar CogniCode en un sistema que:

1. **Entiende el código** semánticamente, no solo busca patrones de texto
2. **Aprende del feedback** colectivo para reducir falsos positivos
3. **Proporciona insights** sobre la calidad y salud del codebase
4. **Se integra naturalmente** en el workflow del desarrollador

---

## 2. Timeline General

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         TIMELINE COGNICODE RULES PRO                         │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ████ Sprint 1-2 (Semanas 1-4) ████                                        │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ FASE 0: ESTABILIZACIÓN + FASE 1: INFRAESTRUCTURA                       │ │
│  │ - 294/294 tests pasando                                                │ │
│  │ - Proc-macro #[cogni_rule] diseñada e implementada                    │ │
│  │ - ast-grep-core integrado                                              │ │
│  │ - 10 reglas como proof of concept                                      │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  ████ Sprint 3-4 (Semanas 5-8) ████                                        │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ FASE 2: LCPG + FASE 1 COMPLETADA                                        │ │
│  │ - PreflightFilter (Aho-Corasick) implementado                          │ │
│  │ - Visitor trait reutilizable                                            │ │
│  │ - SymbolTable builder para LCPG                                        │ │
│  │ - Tests declarativos inline                                            │ │
│  │ - FP reputation system básico                                          │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  ████ Sprint 5-8 (Semanas 9-16) ████                                        │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ FASE 3: MIGRACIÓN MASIVA                                                │ │
│  │ - Security rules migradas                                              │ │
│  │ - Bug rules migradas                                                    │ │
│  │ - Code smell rules migradas                                             │ │
│  │ - Naming mapping completo                                               │ │
│  │ - Benchmark: 3x+ más rápido que regex                                   │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  ████ Sprint 9-12 (Semanas 17-24) ████                                      │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ FASE 4: PLATAFORMA + DATAFLOW                                          │ │
│  │ - Dataflow/taint tracking                                               │ │
│  │ - Análisis incremental (AST diffing)                                    │ │
│  │ - Visual rule creator (UI)                                             │ │
│  │ - Traducción automática desde ESLint/Clippy                             │ │
│  │ - Leaderboard de calidad por equipo                                     │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Sprint 1-2: Cimientos

### Semanas 1-4

| Tarea | Descripción | Entregable |
|-------|-------------|------------|
| **E0.1** | Stabilize tests | 294/294 passing |
| **E0.2** | Fix S4792 DES/RC4 | 3 tests fixed |
| **E0.3** | Fix S5122 SQL | 3 tests fixed |
| **I1.1** | Design proc-macro API | SKILL.md documentado |
| **I1.2** | Crear `cognicode-macros` crate | Crate creado |
| **I1.3** | Implementar `#[cogni_rule]` macro | Macro funcional |
| **I1.4** | Integrar `ast-grep-core` | Dependencia agregada |
| **I1.5** | Implementar `PreflightFilter` | Aho-Corasick funcional |
| **I1.6** | Auto-registro con `inventory` | Catálogo automático |
| **I1.7** | Migrar 10 reglas PoC | 10 rules migrated |

### Definition of Done

- [ ] `cargo test --lib` reporta 294/294 tests pasando
- [ ] `#[cogni_rule]` valida patterns en compile-time
- [ ] Ejemplo de regla nueva compila y funciona
- [ ] Pre-flight filtra reglas por keywords
- [ ] Documentación de la proc-macro actualizada

### Métricas de Éxito

| Métrica | Baseline | Target |
|---------|----------|--------|
| Tests passing | 275/294 | 294/294 |
| Build time | N/A | <5s overhead |
| Runtime overhead | N/A | <50ms startup |

---

## 4. Sprint 3-4: Infraestructura

### Semanas 5-8

| Tarea | Descripción | Entregable |
|-------|-------------|------------|
| **I2.1** | Visitor trait reutilizable | Trait documentado |
| **I2.2** | SymbolTable builder | LCPG básico funcional |
| **I2.3** | Integración RuleContext | SymbolTable accesible |
| **I2.4** | Tests declarativos inline | `#[test_rule]` macro |
| **I2.5** | FP reputation system v1 | Sistema básico funcional |
| **I2.6** | Documentar arquitectura | Diagrams actualizados |
| **I2.7** | Benchmark suite | Suite de benchmarks |

### Definition of Done

- [ ] Visitor trait implementado y documentado
- [ ] Al menos 4 reglas usan LCPG
- [ ] Tests inline funcionan para reglas migradas
- [ ] FP suppression guarda en `.cognicode-suppressions.json`
- [ ] Benchmarks muestran <100ms para archivo típico

### Métricas de Éxito

| Métrica | Baseline | Target |
|---------|----------|--------|
| LCPG build time | N/A | <10ms por archivo |
| Memory overhead | N/A | <100KB por archivo |
| Rules using LCPG | 0 | ≥4 |

---

## 5. Sprint 5-8: Migración

### Semanas 9-16

| Tarea | Descripción | Entregable |
|-------|-------------|------------|
| **M3.1** | Migrar security rules | 20 rules migrated |
| **M3.2** | Migrar S4792, S5122 | 6 tests fixed |
| **M3.3** | Migrar bug rules | 50 rules migrated |
| **M3.4** | Migrar code smell rules | 100 rules migrated |
| **M3.5** | Migrar performance rules | 50 rules migrated |
| **M3.6** | Naming mapping completo | IDs descriptivos |
| **M3.7** | Benchmark performance | 3x speedup vs regex |

### Definition of Done

- [ ] Todas las 854 reglas tienen IDs descriptivos
- [ ] Security rules migradas a Layer 3 (dataflow)
- [ ] Benchmark muestra 3x+ mejora vs regex
- [ ] 100% de rules tienen tests inline
- [ ] Catálogo de reglas documentado

### Métricas de Éxito

| Métrica | Baseline | Target |
|---------|----------|--------|
| Scan speed (1000 files) | ~13s | <4s |
| Rules migrated | 0 | 854 |
| False positive rate | ~20% | <5% |
| New rule format | 0% | 100% |

---

## 6. Sprint 9-12: Plataforma

### Semanas 17-24

| Tarea | Descripción | Entregable |
|-------|-------------|------------|
| **P4.1** | Dataflow/taint tracking | TaintTracker implementado |
| **P4.2** | SQL injection dataflow | Rule migrada a Layer 3 |
| **P4.3** | Command injection dataflow | Rule migrada a Layer 3 |
| **P4.4** | Incremental analysis | AST diffing funcional |
| **P4.5** | Visual rule creator | UI para crear reglas |
| **P4.6** | ESLint/Clippy translator | Import de rules externo |
| **P4.7** | Team quality leaderboard | Dashboard implementado |

### Definition of Done

- [ ] TaintTracker implementado y documentado
- [ ] Al menos 5 rules usan Layer 3
- [ ] Análisis incremental reduce tiempo en 50%+ para cambios pequeños
- [ ] UI permite crear reglas sin código
- [ ] Importer convierte ESLint rules a `#[cogni_rule]`
- [ ] Leaderboard muestra métricas por equipo

### Métricas de Éxito

| Métrica | Baseline | Target |
|---------|----------|--------|
| Security rules with dataflow | 0 | ≥5 |
| Incremental analysis speedup | N/A | 50%+ para cambios <10 files |
| Rules importable from ESLint | 0 | ≥100 |
| Team dashboard | No | Sí |

---

## 7. Métricas de Éxito Globales

### Al Final del Proyecto (Semana 24)

| Categoría | Métrica | Target |
|-----------|---------|--------|
| **Tests** | Tests passing | 294/294 (100%) |
| **Performance** | Scan time (1000 files) | <30s |
| **Precision** | False positive rate | <5% |
| **Coverage** | Rules with descriptive IDs | 854 (100%) |
| **Code Quality** | YAML en codebase | 0 files |
| **Documentation** | Rules documentadas | 100% |

### Indicadores de Salud del Proyecto

| Indicador | Verde | Amarillo | Rojo |
|-----------|-------|----------|-------|
| Tests passing | 294/294 | 280-293 | <280 |
| Build time | <5min | 5-10min | >10min |
| False positive rate | <5% | 5-20% | >20% |
| Coverage | >90% | 70-90% | <70% |
| Benchmark trend | Improving | Stable | Regressing |

---

## 8. Estructura de Sprints

### Formato de Sprint

```
┌──────────────────────────────────────────────────────────────────┐
│                       SPRINT PLANNING                             │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  DURACIÓN: 2 semanas                                             │
│  EQUIPO: ~3 agentes                                             │
│  CAPACITY: ~40 story points                                     │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ SPRINT GOAL: [Objetivo principal del sprint]              │  │
│  └────────────────────────────────────────────────────────────┘  │
│                                                                  │
│  STORIES:                                                       │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ [P1] Como desarrollador, quiero que los tests pasen...   │  │
│  │ [P1] Como desarrollador, quiero que #[cogni_rule]...     │  │
│  │ [P2] Como usuario, quiero que el sistema filtre...        │  │
│  │ [P3] Como usuario, quiero ver un dashboard de...         │  │
│  └────────────────────────────────────────────────────────────┘  │
│                                                                  │
│  DEFINITION OF DONE:                                            │
│  □ Código mergeado en main                                      │
│  □ Tests pasan                                                 │
│  □ Benchmark realizado                                         │
│  □ Documentación actualizada                                    │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## 9. Riesgos y Contingencias

### Riesgos Principales

| ID | Riesgo | Probabilidad | Impacto | Mitigación |
|----|--------|-------------|---------|------------|
| R1 | tree-sitter grammar breaking changes | Baja | Alto | Proc-macro valida en compile-time |
| R2 | ast-grep-core tiene bugs/baja cobertura | Media | Medio | Fallback a tree-sitter queries |
| R3 | Performance regression | Media | Alto | Benchmarks automáticos en CI |
| R4 | scope creep en features | Alta | Medio | Strict sprint goals |
| R5 | Tests se rompen durante migración | Alta | Medio | Migración incremental |

### Plan de Contingencia

```
SI ast-grep-core tiene problemas graves:
  → Fallback a tree-sitter queries directos
  → Posponer Layer 3 (dataflow)
  → Mantener regex para rules simples

SI performance regression > 20%:
  → Perfilado detallado con chronos-mcp
  → Optimizar hot paths
  → Considerar caching de AST parsed

SI falsos positivos no mejoran:
  → Incrementar investment en FP reputation
  → Involucrar usuarios más activamente
  → Considerar ML-based filtering
```

---

## 10. Referencias Cruzadas

| Documento | Descripción |
|-----------|-------------|
| `00-diagnostico.md` | Estado actual del sistema |
| `01-arquitectura.md` | Arquitectura de 4 capas |
| `02-rules-as-code.md` | Proc-macro y compile-time validation |
| `03-lcpg.md` | Lightweight Code Property Graph |
| `04-pre-flight.md` | Layer 0 con Aho-Corasick |
| `05-fp-reputation.md` | Sistema de reputación de FPs |
| `06-migration.md` | Plan de migración incremental |

---

## 11. Aprobaciones

| Rol | Nombre | Fecha | Firma |
|-----|--------|-------|-------|
| Tech Lead | [Pending] | 2026-05-11 | |
| Product Owner | [Pending] | 2026-05-11 | |
| QA Lead | [Pending] | 2026-05-11 | |

---

*Documento creado como parte del plan CogniCode Rules Pro*
*Última actualización: 11 de Mayo de 2026*
