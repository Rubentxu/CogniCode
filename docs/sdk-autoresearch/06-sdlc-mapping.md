# 06 — SDLC Mapping

> Mapeo de las 7 fases del Ciclo de Vida de Desarrollo de Software al framework
> AutoResearch. Cada fase tiene su nivel de autonomía, sus Gates, sus Métricas,
> y su mecanismo de backtrack cuando algo falla.

---

## 1. Niveles de Autonomía por Fase

| Nivel | Significado | Intervención humana |
|-------|-------------|---------------------|
| **Full Auto** | El agente decide y ejecuta sin preguntar | Cero (solo revisar logs después) |
| **AI-Led** | El agente propone y ejecuta, el humano revisa | Aprobación post-ejecución |
| **AI-Assisted** | El agente analiza y sugiere, el humano decide | Aprobación pre-ejecución |
| **Human-Led** | El humano hace, el agente asiste con información | Total |

| Fase SDLC | Nivel de Autonomía | Justificación |
|-----------|-------------------|---------------|
| Planificación | AI-Assisted | Decisiones estratégicas requieren juicio humano |
| Requisitos | AI-Led | El agente extrae, el humano valida |
| Diseño | AI-Led (LLD) / AI-Assisted (HLD) | Arquitectura = decisión humana; diseño detallado = automatizable |
| Desarrollo | **Full Auto** | Cambios acotados con gates de seguridad |
| Pruebas | **Full Auto** | Generación y ejecución de tests automatizable |
| Despliegue | AI-Led | Rollback automático si falla, pero deployment requiere aprobación |
| Mantenimiento | **Full Auto** | Bucle eterno de mejora continua |

---

## 2. Fase 1: Planificación

```
┌─────────────────────────────────────────────────────────────┐
│ FASE: PLANNING                        Autonomía: AI-Assisted │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  INPUTS:                                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐  │
│  │ Git log  │  │Health    │  │Call graph│  │Issue       │  │
│  │          │  │Score     │  │          │  │tracker     │  │
│  └──────────┘  └──────────┘  └──────────┘  └────────────┘  │
│       │              │              │              │         │
│       └──────────────┴──────────────┴──────────────┘         │
│                          │                                    │
│                     ┌────▼────┐                              │
│                     │ AGENTE  │                              │
│                     │ analiza │                              │
│                     └────┬────┘                              │
│                          │                                    │
│  OUTPUTS:              ┌──▼──────────────────────────────┐  │
│  ┌─────────────────────┤  Backlog Priorizado              │  │
│  │ [P0] Fix S107       │  • Health impact estimation     │  │
│  │ [P1] Reduce build   │  • Effort estimation            │  │
│  │ [P2] Add Python     │  • Dependency ordering          │  │
│  │ [P3] Improve tests  │  • Risk assessment              │  │
│  └─────────────────────┘                                  │  │
│                                                              │
│  ⚠️  HUMANO REVISA Y APRUEBA EL BACKLOG                     │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Lo que hace el agente

1. Analiza el historial de git: ¿qué áreas cambian más? ¿dónde hay más bugs?
2. Evalúa el Health Score actual: ¿qué dimensiones están peor?
3. Analiza el call graph: ¿qué módulos son hotspots?
4. Consulta el issue tracker: ¿qué bugs/peticiones pendientes?
5. Genera un backlog priorizado con estimaciones de impacto y esfuerzo

### Métricas específicas de Planning

| Métrica | Qué mide |
|---------|----------|
| Technical Debt Ratio | Remediation cost / rewrite cost |
| Hot Paths | Funciones más llamadas (priorizar optimización) |
| Bug Density | Bugs por KLOC por módulo |
| Change Frequency | Archivos más modificados (inestabilidad) |

---

## 3. Fase 2: Análisis de Requisitos

```
┌─────────────────────────────────────────────────────────────┐
│ FASE: REQUIREMENTS                    Autonomía: AI-Led     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ACCIONES DEL AGENTE:                                        │
│                                                              │
│  1. Extraer specs de código legacy                           │
│     └─▶ Lee funciones públicas, tipos, traits               │
│         Genera documentación de comportamiento observado     │
│                                                              │
│  2. Detectar gaps docs-vs-impl                               │
│     └─▶ Compara doc comments con firmas reales              │
│         "La doc dice que devuelve Option<T> pero el código   │
│          devuelve Result<T, E>"                              │
│                                                              │
│  3. Generar escenarios BDD                                   │
│     └─▶ Dado el comportamiento observado                     │
│         Genera Given/When/Then para tests                    │
│                                                              │
│  ⚠️  HUMANO VALIDA LOS REQUISITOS EXTRAÍDOS                  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. Fase 3: Diseño

```
┌─────────────────────────────────────────────────────────────┐
│ FASE: DESIGN                           Autonomía: AI-Led    │
│                                       (LLD: Full Auto)      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  HLD (AI-Assisted):                                         │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ • Análisis de connascencia (9 tipos)                  │   │
│  │ • Detección de ciclos arquitectónicos (Tarjan SCC)    │   │
│  │ • Propuesta de refactors para reducir acoplamiento    │   │
│  │ • Generación de diagramas C4 desde el código          │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  LLD (Full Auto):                                            │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ • Extraer interfaces de código existente              │   │
│  │ • Proponer firmas de funciones nuevas                 │   │
│  │ • Validar contra patrones de diseño conocidos         │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  GATES DE DISEÑO:                                            │
│  ✅ Architecture cycle gate (no nuevos ciclos)               │
│  ✅ Dependency direction gate (infra → dominio, no al revés) │
│                                                              │
│  MÉTRICAS DE DISEÑO:                                         │
│  • Coupling score (bajo = mejor)                             │
│  • Cohesion score (alto = mejor)                             │
│  • Abstraction/Concretion ratio                              │
│  • SOLID compliance                                          │
│  • Design pattern adherence (LLM-assisted)                   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. Fase 4: Desarrollo (Coding)

```
┌─────────────────────────────────────────────────────────────┐
│ FASE: CODING                         Autonomía: Full Auto   │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ESTA ES LA FASE DONDE OCURRE EL BUCLE KARPATHY             │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              FOREVER LOOP (por iteración)             │   │
│  │                                                      │   │
│  │  1. EVALUATE   → health_before                       │   │
│  │  2. SUGGEST    → LLM analiza qué cambiar             │   │
│  │  3. PROPOSE    → LLM genera diff                     │   │
│  │  4. MODIFY     → Agente aplica cambio mínimo         │   │
│  │  5. PRE-GATE   → Compila? Tests pasan?               │   │
│  │  6. COMMIT     → git commit (checkpoint)             │   │
│  │  7. EVALUATE   → health_after                        │   │
│  │  8. DECIDE     → KEEP (health subió) o DISCARD       │   │
│  │  9. LOG        → results.tsv                         │   │
│  │  10. REPEAT    → vuelta al paso 1                    │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  GATES (pre-commit):                                         │
│  ✅ CompilationGate                                          │
│  ✅ TestsGate (rápido: unit tests)                           │
│  ✅ SyntaxGate (regex/tree-sitter queries válidos)           │
│                                                              │
│  GATES (post-commit, full eval):                             │
│  ✅ LintGate                                                 │
│  ✅ FmtGate (warning)                                        │
│                                                              │
│  SEGURIDAD: Si cualquier gate bloqueante falla → DISCARD     │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. Fase 5: Pruebas

```
┌─────────────────────────────────────────────────────────────┐
│ FASE: TESTING                        Autonomía: Full Auto   │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ACCIONES DEL AGENTE:                                        │
│                                                              │
│  1. Identificar gaps de cobertura                            │
│     └─▶ Funciones sin tests, ramas no cubiertas             │
│                                                              │
│  2. Generar tests                                            │
│     └─▶ Unit tests para uncovered code                      │
│     └─▶ Integration tests para flujos críticos              │
│     └─▶ Property-based tests para funciones puras           │
│                                                              │
│  3. Mutation testing                                         │
│     └─▶ Introducir bugs (mutaciones)                        │
│     └─▶ Verificar que los tests los detectan                │
│     └─▶ Mutation score < 80% → refuerzo de tests           │
│                                                              │
│  4. Differential testing (Chronos)                           │
│     └─▶ Comparar comportamiento OLD vs NEW                  │
│                                                              │
│  GATES:                                                      │
│  ✅ CoverageGate (≥70%)                                      │
│  ✅ MutationScoreGate (≥80%)                                 │
│                                                              │
│  MÉTRICAS:                                                   │
│  • Coverage (líneas, ramas, funciones)                       │
│  • Mutation score                                            │
│  • Test quality (LLM: ¿los tests son significativos?)       │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 7. Fase 6: Despliegue

```
┌─────────────────────────────────────────────────────────────┐
│ FASE: DEPLOYMENT                     Autonomía: AI-Led      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  GATES PRE-DEPLOY (BLOQUEANTES):                             │
│  ✅ SecurityGate (cargo audit, sin CVEs críticos)            │
│  ✅ ApiBreakGate (sin breaking changes no documentados)      │
│  ✅ LicenseGate (todas las deps con licencias permitidas)    │
│  ✅ BuildSizeGate (binario ≤ +10%)                           │
│  ✅ AllTestsGate (integración + sistema + e2e)               │
│                                                              │
│  ESTRATEGIA DE DEPLOY:                                       │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  1. Canary deploy a 5% del tráfico                   │   │
│  │  2. Monitorear métricas (errores, latencia)          │   │
│  │  3. Si OK → 25% → 50% → 100%                        │   │
│  │  4. Si error → rollback automático                   │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ⚠️  EL HUMANO INICIA EL DEPLOY                              │
│  ✅  EL AGENTE GESTIONA EL PROCESO Y ROLLBACK                │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 8. Fase 7: Mantenimiento

```
┌─────────────────────────────────────────────────────────────┐
│ FASE: MAINTENANCE                   Autonomía: Full Auto    │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ESTA ES LA FASE CONTINUA. EL BUCLE NUNCA SE DETIENE.       │
│                                                              │
│  SUB-FASES:                                                  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ 7.1 Bug Detection                                    │   │
│  │     • Analizar logs de producción                    │   │
│  │     • Detectar crashes (SIGSEGV, panics)             │   │
│  │     • Correlacionar con cambios recientes (git log)  │   │
│  │     • Proponer fix + test de regresión               │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ 7.2 Technical Debt Reduction                         │   │
│  │     • Medir Technical Debt Ratio (SQALE)             │   │
│  │     • Priorizar issues por remediation cost          │   │
│  │     • Reducir deuda en iteraciones pequeñas          │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ 7.3 Self-Evolving Rules                              │   │
│  │     • Mejorar reglas de detección (catálogo SonarQube)│   │
│  │     • Corregir metadatos                              │   │
│  │     • Reducir falsos positivos                        │   │
│  │     • Añadir nuevas reglas                            │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ 7.4 Dependency Updates                               │   │
│  │     • cargo update / pip upgrade                     │   │
│  │     • Verificar que tests pasan                      │   │
│  │     • Verificar que no hay nuevas CVEs               │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  GATES: TODOS (el set completo)                              │
│  MÉTRICAS: TODAS (el set completo)                           │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 9. Backtrack Pattern

Cuando una fase falla, el sistema retrocede a la fase más temprana que
puede arreglar el problema:

```
Test failure en Deploy
  └─▶ backtrack a Coding: "los tests no cubrían este caso"
        └─▶ agente genera tests + fix

Design flaw detectado en Test
  └─▶ backtrack a Design: "hay un ciclo de dependencias"
        └─▶ agente propone refactor para romper el ciclo

Security CVE en Deploy
  └─▶ backtrack a Coding: "vulnerabilidad en dependencia X"
        └─▶ agente actualiza dependencia + verifica tests

Architecture cycle en Maintenance
  └─▶ backtrack a Design: "nuevo ciclo introducido"
        └─▶ agente analiza y propone romper ciclo

Build failure en Coding
  └─▶ backtrack a Planning: "el cambio propuesto no es viable"
        └─▶ agente descarta y busca alternativa
```

### Implementación del Backtrack

```rust
pub struct BacktrackEngine {
    pipelines: HashMap<SdlcPhase, Box<dyn SdlcPipeline>>,
}

impl BacktrackEngine {
    pub fn handle_failure(
        &self,
        current_phase: SdlcPhase,
        failure: &PipelineResult,
    ) -> Option<BacktrackAction> {
        // Preguntar al pipeline actual: ¿a qué fase retroceder?
        let pipeline = self.pipelines.get(&current_phase)?;
        let (target_phase, suggestion) = pipeline.backtrack(failure)?;

        Some(BacktrackAction {
            from: current_phase,
            to: target_phase,
            reason: failure.error_description.clone(),
            suggestion,
            auto_fixable: target_phase.autonomy_level() >= AutonomyLevel::AiLed,
        })
    }
}
```

---

## 10. Diagrama de Flujo SDLC Completo

```
                    ┌─────────────┐
                    │  PLANNING   │ ◀── Human approves backlog
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │ REQUIREMENTS│ ◀── Human validates specs
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │   DESIGN    │ ◀── Human approves HLD
                    └──────┬──────┘
                           │
              ┌────────────▼────────────┐
              │       CODING            │
              │  ┌───────────────────┐  │
              │  │ KARPATHY LOOP     │  │
              │  │ FOREVER           │  │
              │  └───────────────────┘  │
              └────────────┬────────────┘
                           │
              ┌────────────▼────────────┐
              │       TESTING           │
              │  ┌───────────────────┐  │
              │  │ Coverage gaps     │  │
              │  │ Mutation testing  │  │
              │  │ Test generation   │  │
              │  └───────────────────┘  │
              └────────────┬────────────┘
                           │
              ┌────────────▼────────────┐
              │      DEPLOYMENT         │
              │  ┌───────────────────┐  │
              │  │ Security scan     │  │
              │  │ API break check   │  │
              │  │ Canary → rollback │  │
              │  └───────────────────┘  │
              └────────────┬────────────┘
                           │
              ┌────────────▼────────────┐
              │     MAINTENANCE         │◀── BUCLE CONTINUO
              │  ┌───────────────────┐  │
              │  │ Bug detection     │  │
              │  │ Tech debt reduction│  │
              │  │ Self-evolving rules│  │
              │  │ Dependency updates │  │
              │  └───────────────────┘  │
              └─────────────────────────┘

         ANY FAILURE → BACKTRACK a la fase más temprana que puede arreglarlo
```

---

## Siguiente: [07 — Multi-Agent Swarm](07-multi-agent-swarm.md)
