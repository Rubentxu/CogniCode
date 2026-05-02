# Roadmap cognicode-axiom: Governance para AI Coding Agents

> **Versión del documento**: 1.0  
> **Fecha**: Abril 2026  
> **Estado**: Draft  
> ** crates relacionadas**: cognicode-core, cognicode-mcp, cognicode-axiom

---

## 1. Visión del Producto

### 1.1 Descripción General

**cognicode-axiom** es un crate de gobernanza dentro del workspace de CogniCode que proporciona un marco de enforcement de políticas para AI coding agents. En 6 meses, axiom será el estándar de facto para equipos que necesitan controlar, auditar y guiar el comportamiento de agentes IA en sus flujos de trabajo de desarrollo.

### 1.2 Usuarios Objetivo

| Segmento | Descripción | Caso de Uso Principal |
|----------|-------------|----------------------|
| **Desarrolladores individuales** | Engineers que usan Claude Code, Cursor o Copilot | Proteger código crítico de modificaciones accidentales |
| **Equipos de engineering** | 5-50 desarrolladores con AI agents | Enforcement de estándares de código y patrones arquitectónicos |
| **Platform teams** | Equipos de platform que proveen tooling | Definir políticas de governance para múltiples proyectos |
| **Enterprises** | Organizaciones con requisitos de compliance | Auditoría completa y reporting regulatorio |

### 1.3 Propuesta de Valor Core

> **"Reglas ejecutables para AI coding agents, potenciadas por code intelligence"**

A diferencia de linters tradicionales o herramientas de policy management convencionales, cognicode-axiom usa el call graph y el symbol analysis de CogniCode para entender el contexto semántico del código. Esto permite:

- **Gobernanza contextual**: Evaluar acciones no solo por path/patrón, sino por impacto real en la arquitectura
- **Quality gates inteligentes**: Detectar violaciones SOLID y connascence que herramientas estáticas no ven
- **Auto-correction loop**: Permitir que el agente corrija sus propias violaciones basándose en feedback previo
- **Aprendizaje acumulativo**: Mejorar políticas basándose en patrones de corrección

### 1.4 Visión a 6 Meses

```
Mes 1-2: Foundation    → Policy engine funcional con Cedar
Mes 3:   Quality Gates → Análisis de código con CogniCode
Mes 4-5: Compliance    → ADR parsing, audit trail, linters
Mes 6:   Intelligence  → Episodic memory, self-correction
```

---

## 2. Releases

### v0.1.0 — "Foundation" (Semanas 1-2)

**Objetivo**: Demonstrar el value proposition básico — policy enforcement blocking acciones prohibidas.

#### Features

- [ ] **Cedar Policy engine integration**
  - Integración del crate `cedar-policy-core` v3.x
  - Parsing y evaluación de policies en tiempo real
  - Soporte para `.cedar` files con sintaxis JSON o YAML

- [ ] **4 MCP tools core**
  ```
  check_action(action: Action, context: Context) → Decision
  add_rule(rule: Rule, policy_id: PolicyId) → Result<()>
  remove_rule(policy_id: PolicyId, rule_id: RuleId) → Result<()>
  validate_rule(rule: Rule) → ValidationResult
  ```

- [ ] **File-based policy storage**
  - Directorio `.axiom/policies/` con archivos `.cedar`
  - Formato: `policy_{name}_{version}.cedar`
  - Hot-reload de policies en cambio de archivo

- [ ] **Path/action rules**
  - Reglas basadas en path patterns: `allow/deny file:edit:src/prod/**`
  - Action taxonomy: `file:read`, `file:write`, `file:delete`, `exec:run`, `net:fetch`

#### Milestone

> **"First policy enforcement: bloquear ediciones a production/"**

```
Ejemplo de uso:
  $ axiom add-rule --policy-id "protect-prod" --rule "forbid action:file:write on path:src/prod/**"
  $ axiom check-action "file:write src/prod/config.yaml"
  DENY: Policy 'protect-prod' forbids action
```

#### Criteria de Éxito

- [ ] Agent puede bloquear una acción de escritura a `src/prod/**`
- [ ] Tiempo de evaluación < 10ms por check
- [ ] Error messages claros indicando qué policy fue violada

---

### v0.2.0 — "Quality Gates" (Semanas 3-4)

**Objetivo**: Integrar análisis de código usando call graph de CogniCode para quality gates semánticos.

#### Features

- [ ] **SOLID analysis**
  - **SRP (Single Responsibility Principle)**: LCOM4 > 0.5 activa violación
  - **OCP (Open/Closed)**: Análisis de cambios requeridos en dependents
  - **LSP (Liskov Substitution)**: Validación de trait bounds
  - **ISP (Interface Segregation)**: Detección de fat interfaces
  - **DIP (Dependency Inversion)**: Verificación de abstract dependencies

- [ ] **Connascence analysis**
  - Connascence métricas: CMC (Connascence of Method Calls), CN (Connascence of Name)
  - Umbrales configurables por team/project
  - Visualización de coupling graph

- [ ] **Quality delta (before/after)**
  ```
  $ axiom quality-delta --before="git HEAD~1" --after="."
  +15 LCOM4 score on src/services/UserService
  -3 connascence violations in src/domain/
  BREAKING: src/api/breaking change in public API
  ```

- [ ] **DDD boundary validation**
  - Verificación de bounded contexts usando CogniCode symbols
  - Prevention de cyclic dependencies entre bounded contexts
  - Enforcement de domain layer dependencies (domain → domain, application → domain, infrastructure → domain)

- [ ] **3 nuevas MCP tools**
  ```
  check_quality(target: Target, quality_model: Model) → QualityReport
  quality_delta(before: Ref, after: Ref) → DeltaReport
  check_boundaries(context: BoundedContext) → BoundaryValidation
  ```

#### Milestone

> **"Agent auto-corrects SRP violations detected via LCOM"**

```
Flujo:
  1. Agent intenta agregar método a UserService
  2. axiom detecta LCOM > 0.5 post-commit
  3. Agent recibe: "UserService tiene 7 methods, 2 responsibilities detected"
  4. Agent propone extracción de clase UserValidator
  5. axiom valida LCOM < 0.3 post-refactor
```

#### Criteria de Éxito

- [ ] Detectar 90%+ de violaciones SOLID en código de test
- [ ] False positive rate < 15%
- [ ] Sugerencias de refactor reducir LCOM en 80%+ de casos

---

### v0.3.0 — "Compliance" (Semanas 5-6)

**Objetivo**: Completar el ciclo de auditoría y compliance con parsing de ADRs, linters externos y audit trail.

#### Features

- [ ] **ADR parsing → Cedar rules**
  - Parser para Architecture Decision Records (ADRs format)
  - Extracción automática de constraints de ADRs existentes
  - Generación de Cedar policies desde ADRs
  ```
  $ axiom import-adr ./docs/adr/0017-use-cedar-policy.md
  Parsed: "Must use Cedar for all new policy definitions"
  Generated rule: "require engine:cedar for policy:*"
  ```

- [ ] **External linter integration**
  - **clippy**: Integración via `cargo clippy --message-format=json`
  - **eslint**: Via `npx eslint --format=json` (projectos JS/TS)
  - **semgrep**: Via `semgrep --json` con rules personalizadas
  - Normalización de outputs a formato axiom

- [ ] **SQLite audit trail**
  - Schema: `audit_log(id, timestamp, agent_id, action, resource, decision, policy_hit, latency_ms)`
  - Índices en: `timestamp`, `agent_id`, `action`, `resource`
  - Query interface para análisis retrospectivo
  ```
  $ axiom query --filter="action:file:write AND decision:DENY" --from="2026-04-01"
  47 denials in April, 12 unique resources protected
  ```

- [ ] **Claude Code hook templates**
  - Template de pre-commit hook para Claude Code
  - Template de action filter hook
  - Integration con `~/.claude/settings.json`

#### Milestone

> **"Full audit report de todas las acciones de agent en una sesión"**

```
Reporte generado:
  Session: claude-code-2026-04-30
  Agent: claude-sonnet-4
  Duration: 2h 34m
  
  Actions: 156 total
    - file:read: 89 (57%)
    - file:write: 34 (22%)
    - exec:run: 18 (12%)
    - net:fetch: 15 (10%)
  
  Decisions: 
    - ALLOW: 142 (91%)
    - DENY: 14 (9%)
  
  Policies triggered: 14
    - protect-prod: 8 denials
    - no-inline-secrets: 3 denials  
    - solid-gate: 3 denials (quality violations)
  
  Quality metrics:
    - Pre-session LCOM4: 0.32
    - Post-session LCOM4: 0.29
    - New violations introduced: 0
```

#### Criteria de Éxito

- [ ] Parsear 95%+ de ADRs en formato estándar
- [ ] Integrar con clippy, eslint, semgrep sin falsos positivos adicionales
- [ ] Audit log query response < 500ms para 1M rows

---

### v0.4.0 — "Intelligence" (Semanas 7-8)

**Objetivo**: Cerrar el loop de self-correction con episodic memory y rule learning.

#### Features

- [ ] **Episodic reflection memory**
  - Almacenamiento de "episodes": triplets (action, context, outcome)
  - Vector similarity search para encontrar episodios similares
  - Persistencia en SQLite con extension `vss0`
  ```
  Episode schema:
    - id: UUID
    - timestamp: DATETIME
    - action: Action
    - context: { file, function, diff }
    - decision: Decision
    - feedback: Feedback  // explicit correction or implicit success
    - policy_violated: Option<PolicyId>
  ```

- [ ] **Self-correction feedback loop**
  - Detección de corrección: agent ignora DENY → próxima evaluación con `forced=true`
  - Implicit feedback: acción DENY seguida de éxito = "false positive"
  - Explicit feedback: `axiom feedback --correct --episode-id=X --reason="rule too strict"`
  ```
  Loop:
    1. Agent action → DENY
    2. Agent modifies action → retry
    3. Success → positive feedback stored
    4. (Alternative) Agent forces → negative feedback + rule review
  ```

- [ ] **Rule learning from corrections**
  - ML simple: Bayesian updating de rule scores basado en feedback
  - Context-aware policy relaxation
  - "Learning rate" configurable
  ```
  Rule adjustment:
    original: "deny file:write on src/prod/**"
    feedback: 3 consecutive false positives on src/prod/tests/**
    adjusted: "deny file:write on src/prod/** except path:src/prod/tests/**"
  ```

#### Milestone

> **"Agent aprende de errores a través de sesiones"**

```
Escenario:
  Día 1:
    Agent intenta escribir en src/prod/config.yaml
    DENY (policy: protect-prod)
    Agent consulta axiom suggest
    Sugerencia: "Create config loader that reads from env instead"
  
  Día 2:
    Agent tiene nuevo proyecto similar
    axiom detecta similar context (archetype: "config-in-prod")
    Sugiere preemptivamente: "Use config loader pattern"
    Agent lo aplica sin DENY

  Día 7:
    Agent ha internalizado el pattern
    No necesita DENY para escenarios de config-in-prod
    Rule scoring: "protect-prod" relevance 0.9 → 0.7 (learned behavior)
```

#### Criteria de Éxito

- [ ] Reducción de 30%+ en repeated violations después de 5 sesiones
- [ ] False positive rate decrementa 20% por semana de uso
- [ ] Episode retrieval < 50ms para similarity search

---

### v0.5.0 — "Production Ready" (Mes 3)

**Objetivo**: Preparar para adopción empresarial con multi-project support y tooling completo.

#### Features

- [ ] **Multi-project support**
  - Workspace-aware policy inheritance
  - Per-project policy overrides
  - Cross-project policy queries
  - Project hierarchy: `org > team > project > module`

- [ ] **Policy templates library**
  - Templates predefinidos: "microservice-boundaries", "ddd-layered", "react-clean-architecture"
  - Template marketplace interno
  - Version pinning para templates

- [ ] **Web dashboard para audit visualization**
  - Dashboard en Next.js con shadcn/ui
  - Gráficos de métricas: violations por día, policies más activadas
  - Timeline interactivo de sesiones de agent
  - Drill-down: violation → episode → correction

- [ ] **Performance optimization**
  - Cached policy evaluations (LRU cache, 10k entries)
  - Incremental call graph updates
  - Parallel quality analysis con rayon
  - Target: < 5ms p99 para check_action

#### Criteria de Éxito

- [ ] Soportar 100+ projects sin degradación
- [ ] Dashboard carga < 2s para 30 días de datos
- [ ] p99 latency < 5ms para operations normales

---

### v1.0.0 — "Governance Platform" (Mes 6)

**Objetivo**: Consolidar como plataforma de governance para equipos y organizaciones.

#### Features

- [ ] **Team policy sharing**
  - Policy export/import entre equipos
  - Policy inheritance graphs
  - Access control: owner, maintainer, consumer roles

- [ ] **Policy versioning y rollback**
  - Git-like versioning para policies
  - Diff entre versiones de policy
  - Rollback atómico a versión anterior
  - Policy changelog automático

- [ ] **CI/CD pipeline integration**
  - GitHub Actions integration
  - GitLab CI integration
  - PR comment con policy analysis
  - Blocking builds por violations críticas

- [ ] **Multi-agent coordination rules**
  - Reglas de coordinación entre múltiples agents
  - Conflict resolution para concurrent actions
  - Agent identity y authentication

- [ ] **Community policy marketplace**
  - Publish policies públicas
  - Star y fork de policies
  - Curated collections: "SOC2 compliance", "GDPR-ready", "Startup defaults"

#### Criteria de Éxito

- [ ] 10+ equipos usando axiom en producción
- [ ] NPS > 40 entre developers
- [ ] < 1% de issues reportados en GitHub después de 3 meses

---

## 3. Métricas de Éxito

### Tabla Consolidada por Release

| Release | Metric | Target | Measurement |
|---------|--------|--------|-------------|
| **v0.1.0** | Policy evaluation latency | < 10ms p99 | Benchmark interno |
| v0.1.0 | Tool adoption | 1 user activo | Analytics |
| v0.1.0 | False positive rate | < 5% | User feedback |
| **v0.2.0** | SOLID detection accuracy | > 90% | Test suite |
| v0.2.0 | LCOM reduction post-refactor | > 80% | Code analysis |
| v0.2.0 | Auto-correct success rate | > 60% | Episode analysis |
| **v0.3.0** | ADR parsing coverage | > 95% | Parser test suite |
| v0.3.0 | Audit log query latency | < 500ms @ 1M rows | Query benchmarks |
| v0.3.0 | Linter integration coverage | 3 linters (clippy, eslint, semgrep) | Feature test |
| **v0.4.0** | Repeated violation reduction | > 30% after 5 sessions | Episode comparison |
| v0.4.0 | False positive decrement | > 20% per week | Trend analysis |
| v0.4.0 | Episode retrieval latency | < 50ms | Benchmarks |
| **v0.5.0** | Multi-project scale | 100+ projects | Load testing |
| v0.5.0 | Dashboard load time | < 2s @ 30 days | Performance test |
| v0.5.0 | p99 latency | < 5ms | Production monitoring |
| **v1.0.0** | Team adoption | 10+ teams | User analytics |
| v1.0.0 | Developer NPS | > 40 | Survey |
| v1.0.0 | Production issues | < 1% of sessions | Bug tracking |

### Métricas Cualitativas

| Release | Metric | Method |
|---------|--------|--------|
| All | Developer satisfaction | Quarterly survey |
| All | Time saved in code review | Before/after time tracking |
| v0.3.0+ | Audit completeness | Compliance checklist |
| v1.0.0 | Policy coverage | % of code covered by policies |

---

## 4. Timeline Visual

```
═══════════════════════════════════════════════════════════════════════════════════════
                              ROADMAP cognicode-axiom
═══════════════════════════════════════════════════════════════════════════════════════

2026                          Mayo                         Junio                        Julio
┌─────────────────────────────┼─────────────────────────────┼─────────────────────────────┤
        ▼                             ▼                              ▼
    v0.1.0 "Foundation"      │   v0.2.0 "Quality Gates"    │   v0.3.0 "Compliance"       │
   Semanas 1-2               │   Semanas 3-4                │   Semanas 5-6               │
                             │                              │                              │
  ┌─────────────────────┐    │   ┌─────────────────────┐    │   ┌─────────────────────┐    │
  │ • Cedar integration │    │   │ • SOLID analysis    │    │   │ • ADR parsing       │    │
  │ • 4 MCP tools       │    │   │ • Connascence       │    │   │ • Linter integration│    │
  │ • File-based storage│    │   │ • Quality delta    │    │   │ • SQLite audit trail│    │
  │ • Path/action rules │    │   │ • DDD validation    │    │   │ • Claude hooks     │    │
  └─────────────────────┘    │   │ • 3 new MCP tools  │    │   └─────────────────────┘    │
                              │   └─────────────────────┘    │                              │
  ┌─────────────────────┐    │                              │   ┌─────────────────────┐    │
  │ MILESTONE:          │    │   ┌─────────────────────┐    │   │ MILESTONE:          │    │
  │ First enforcement   │    │   │ MILESTONE:          │    │   │ Full audit report   │    │
  │ block edits to prod │    │   │ Auto-correct SRP    │    │   │ of agent session    │    │
  └─────────────────────┘    │   │ violations via LCOM │    │   └─────────────────────┘    │
                              │   └─────────────────────┘    │                              │
                              │                              │                              │
                          v0.4.0 "Intelligence"         │                              │
                         Semanas 7-8                    │                              │
                                                      │                              │
                      ┌─────────────────────┐        │                              │
                      │ • Episodic memory   │        │                              │
                      │ • Self-correction   │        │                              │
                      │ • Rule learning     │        │                              │
                      └─────────────────────┘        │                              │
                                                      │                              │
                      ┌─────────────────────┐        │                              │
                      │ MILESTONE:          │        │                              │
                      │ Agent learns from   │        │                              │
                      │ mistakes            │        │                              │
                      └─────────────────────┘        │                              │
                                                      │                              │
──────────────────────────────────────────────────────┼──────────────────────────────│
                                                      ▼
                                                 Mes 3 (Julio-Agosto)               │
                                            v0.5.0 "Production Ready"              │
                                                                            ┌─────────┴────────┐
                                                                            │ • Multi-project  │
                                                                            │ • Templates lib  │
                                                                            │ • Web dashboard  │
                                                                            │ • Performance    │
                                                                            └──────────────────┘
                                                                                    
────────────────────────────────────────────────────────────────────────────────────────────────
                                                      ▼
                                                 Mes 6 (Octubre)
                                            v1.0.0 "Governance Platform"
                                                                           ┌─────────────────┐
                                                                           │ • Team sharing   │
                                                                           │ • Policy versioning│
                                                                           │ • CI/CD integra. │
                                                                           │ • Multi-agent    │
                                                                           │ • Marketplace    │
                                                                           └─────────────────┘

═══════════════════════════════════════════════════════════════════════════════════════
Semanas:      1  2  3  4  5  6  7  8  9  10 11 12 13 14 15 16 17 18 19 20 21 22 23 24
═══════════════════════════════════════════════════════════════════════════════════════
```

---

## 5. Competencia y Posicionamiento

### 5.1 Panorama Competitivo

| Herramienta | Tipo | Fortalezas | Debilidades | Posición de axiom |
|-------------|------|------------|-------------|-------------------|
| **mxcp** | MCP server | Ecosistema MCP maduro | Sin policy enforcement, solo tools | axiom compite en governance layer |
| **mcp-guardian** | Security layer | Seguridad básica | No integra code intelligence | axiom usa CogniCode para análisis profundo |
| **ArchUnit** | Java architecture testing | Mature, Java ecosystem | Solo JVM, solo tests | axiom es language-agnostic, runtime enforcement |
| **SonarQube** | Code quality | Completoh, enterprise-ready | Solo análisis estático, no enforcement | axiom permite action blocking + self-correction |
| **OPA/Rego** | Policy engine | Standard industry | Sin code intelligence | axiom es OPA + CogniCode |
| **copilot-gateway** | GitHub Copilot governance |Enterprise features | Solo Copilot, no custom agents | axiom soporta cualquier agent compatible MCP |

### 5.2 Diferenciador Único

> **"Code intelligence-powered governance para AI agents"**

```
Diferenciador clave:

  Herramientas tradicionales (SonarQube, ArchUnit):
    → Analizan código estáticamente
    → Reportan violations post-factum
    → No tienen contexto de agent actions

  axiom:
    → Usa call graph y symbol analysis de CogniCode
    → Evalúa actions ANTES de ejecución (prevention)
    → Entiende contexto semántico del cambio
    → Auto-corrects basado en episodic memory
```

### 5.3 Matriz de Posicionamiento

```
                    │
    Alta            │    SonarQube
    Code             │
    Intelligence     │
                    │    ArchUnit
                    │
    Baja            ├───────────────────────────
                    │    OPA     mcp-guardian   mxcp
    Baja            │                       copilot-gateway
    Enforcement     │
                    │
                    └────────────────────────────────── Alta
                           Enforcement Runtime
```

**Estrategia**: Posicionar axiom en el cuadrante "Alta Code Intelligence + Alto Enforcement" — un espacio actualmente no ocupado por ninguna herramienta existente.

---

## 6. Riesgos del Roadmap

### 6.1 Riesgos Técnicos

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| **Cedar Policy performance** | Media | Alto | Benchmarking temprano; fallback a OPA si Cedar no escala |
| **CogniCode API breaking changes** | Baja | Alto | API versioning; abstraction layer en axiom |
| **SQLite vss extension inmadurez** | Media | Medio | Evaluar alternatives (pgvector, ChromaDB); feature flag |
| **Call graph accuracy** | Baja | Medio | Test suites con casos edge; fallback a AST analysis |
| **Concurrent evaluation contention** | Media | Medio | RwLock en lugar de Mutex; lock-free paths críticos |

### 6.2 Riesgos de Adopción

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| **Developer resistance a enforcement** | Alta | Medio | UX primero: friction mínima; opt-in por default |
| **False positives dañando trust** | Alta | Alto | Conservative defaults; easy feedback loop; tuning guide |
| **Complexity overwhelms small teams** | Media | Bajo | Onboarding flow simple; templates pre-configurados |
| **Agent compatibility issues** | Media | Alto | Soporte multi-agent (Claude Code, Cursor, Copilot); test matrix |
| **Cold start: no policies = no value** | Alta | Medio | Policy templates library desde day 1; migration tools |

### 6.3 Riesgos de Competencia

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| **mcp-guardian agrega code intelligence** | Baja | Alto | First-mover advantage; network effects en policy library |
| **OpenAI/Anthropic integran governance nativo** | Baja | Alto | Focus en enterprise features; integration con ecosistema |
| **SonarQube agrega MCP support** | Media | Medio | Mejor developer experience; faster iteration que enterprise |
| **Nuevo entrant con más recursos** | Media | Medio | Open source early; community building; partnership strategy |

### 6.4 Risk Response Strategies

```
│ Riesgo                  │ Estrategia                          │ Trigger Point     │
│─────────────────────────│────────────────────────────────────│───────────────────│
│ Cedar perf < 10ms       │ Parallel evaluation + caching       │ < 50ms en bench  │
│ False positives > 20%   │ Relaxed defaults + explicit config  │ > 10% en survey  │
│ Agent compatibility     │ Abstraction layer + test matrix     │ First blocker    │
│ Adoption < 10 users     │ Developer advocacy + case studies   │ Mes 3 check-in   │
```

---

## 7. Dependencias Externas

### 7.1 Crates de Rust

| Crate | Versión | Uso | Criticalidad | Notas |
|-------|---------|-----|--------------|-------|
| `cedar-policy-core` | 3.x | Policy engine | **Crítica** | Monitoring de releases; pin version |
| `cedar-policy-cli` | 3.x | CLI tooling | Alta | Depende de cedar-policy-core |
| `rmcp` | latest | MCP server/client | **Crítica** | Feature parity con spec |
| `tokio` | 1.x | Async runtime | **Crítica** | Runtime para MCP server |
| `rusqlite` | 0.31 | SQLite wrapper | Alta | Para audit trail |
| `rusqlite-vss` | git | Vector similarity | Media | Feature flag; fallback a text search |
| ` rayon` | 1.x | Parallel processing | Media | Para quality analysis |
| `tree-sitter` | 0.22 | AST parsing | Media | Para ADR parsing |
| `clap` | 4.x | CLI parsing | Baja | Para axiom CLI |
| `tracing` | 0.1 | Observability | Baja | Para logs y debugging |

### 7.2 External Tools

| Tool | Versión | Uso | Criticalidad | Notas |
|------|---------|-----|--------------|-------|
| **cargo clippy** | stable | Linter integration | Alta | Required para Rust projects |
| **eslint** | 9.x | JS/TS linting | Media | Optional; feature flag |
| **semgrep** | 1.x | Pattern matching | Media | Optional; enterprise appeal |
| **Claude Code** | latest | Primary agent target | **Crítica** | Hook API stability |
| **Cursor** | latest | Secondary agent target | Media | MCP compatibility |
| **GitHub Copilot** | latest | Secondary agent target | Media | Via copilot-gateway |

### 7.3 API/SDK Dependencies

| SDK/API | Versión | Uso | Criticalidad | Notas |
|---------|---------|-----|--------------|-------|
| **MCP Protocol** | 2024-11 | Protocol | **Crítica** | Spec change = breaking |
| **Claude Code Hooks** | alpha | Pre-action hooks | **Crítica** | Unstable API; monitor changes |
| **CogniCode Core** | 0.1.x | Call graph, symbols | **Crítica** | Internal crate, co-development |
| **CogniCode MCP** | 0.1.x | MCP integration | **Crítica** | Internal crate |

### 7.4 Monitoring de Dependencias

```rust
// Risk: Cedar Policy breaking changes
// Monitoring: cedar-policy-core GitHub releases, Cedar slack
// Action: Pin version + integration tests on minor updates

// Risk: MCP protocol evolution  
// Monitoring: model-context-protocol GitHub, MCP Discord
// Action: Abstraction layer for transport + protocol

// Risk: Claude Code hook API changes
// Monitoring: Claude Code changelog, anthropic-docs
// Action: Version detection + graceful degradation
```

---

## 8. Apendice: Glossary

| Término | Definición |
|---------|------------|
| **Agent** | AI coding assistant (Claude Code, Cursor, Copilot) |
| **Cedar** | Policy language y engine de AWS |
| **Claim** | Assert about code architecture (e.g., "module A should not depend on B") |
| **Connascence** | Métrica de coupling entre elementos de código |
| **Enforcement** | Blocking o allowing de una action basada en policies |
| **LCOM** | Lack of Cohesion of Methods — métrica de cohesión |
| **MCP** | Model Context Protocol — protocolo para herramientas de AI |
| **Policy** | Regla que define allow/deny para actions |
| **Quality Gate** | CHECK que evalúa calidad post-change |
| **Rule** | Componente atómico de una policy |

---

*Documento generado como parte del roadmap de cognicode-axiom. Para más información, ver SPEC.md y ARCHITECTURE.md en este directorio.*
