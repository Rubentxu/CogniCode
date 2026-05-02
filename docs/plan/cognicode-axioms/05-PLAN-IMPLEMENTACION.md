# PLAN DE IMPLEMENTACIÓN: cognicode-axiom

## Governance Crate para CogniCode Workspace

**Versión:** 1.0
**Fecha:** 2026-04-30
**Estado:** Borrador para revisión
**Workspace:** CogniCode (`/home/rubentxu/Proyectos/rust/CogniCode/`)

---

## 1. Resumen Ejecutivo

**cognicode-axiom** es un crate de gobernanza (governance crate) que se integra en el workspace de CogniCode para proporcionar:

- **Cedar Policy Engine**: Evaluación de políticas de autorización basadas en el lenguaje Cedar de AWS
- **Análisis de Calidad de Código**: Métricas LCOM, connascence, y heurísticas SOLID
- **Validación de Arquitectura**: Detección de violaciones de boundaries en patrones DDD/hexagonal
- **Audit Trail**: Logging persistente de todas las decisiones y acciones en SQLite
- **Reflexión y Memoria Episódica**: Feedback estructurado y aprendizaje from corrections
- **Integración con Linters Externos**: Clippy, ESLint, Semgrep via wrappers
- **Claude Code Hooks**: PreToolUse/PostToolUse para enforcement automático

### Objetivos

1. Proveer un motor de reglas declarativo basado en Cedar para authorization
2. Detectar violaciones de calidad y arquitectura antes de que se propaguen
3. Mantener un audit trail completo de todas las decisiones del sistema
4. Implementar un ciclo de reflexión que permita al sistema aprender de sus errores

### Alcance

- **Dentro del alcance**: El crate `cognicode-axiom` y su integración con `cognicode-core`
- **Fuera del alcance**: Modificaciones a otros crates del workspace (solo consume APIs públicas)

### Estimación Temporal

| Fase | Duración | Entregable |
|------|----------|------------|
| Fase 1: Fundación + Cedar Engine | 2 semanas | Policy engine funcional |
| Fase 2: Calidad + Boundaries | 2 semanas | Análisis de código integrado |
| Fase 3: ADR + Linters + Audit | 2 semanas | Audit trail completo |
| Fase 4: Reflexión + Memory | 2 semanas | Ciclo de aprendizaje cerrado |
| **Total** | **8 semanas** | **cognicode-axiom v1.0** |

---

## 2. Prerrequisitos

### 2.1 Contexto del Workspace CogniCode

El desarrollador debe tener familiaridad con:

**Arquitectura General:**
- DDD (Domain-Driven Design) con arquitectura hexagonal
- Patrón de plugins MCP (Model Context Protocol)
- Sistema de eventos para comunicación entre crates

**Crates existentes que axiom reutiliza:**

| Crate | Utilidad para axiom |
|-------|---------------------|
| `cognicode-core` | Call graph, complexity calculator, cycle detector, impact analyzer |
| `cognicode-mcp` | Exposición de herramientas MCP |
| `tree-sitter` | Parsing de AST para 6 lenguajes |

**Crates del workspace (dependencias indirectas):**
- `petgraph` — Representación de grafos
- `redb` — Base de datos embebida (para call graphs)
- `rmcp` — Servidor MCP

### 2.2 Conocimiento Requerido de Cedar Policy

Cedar es un lenguaje de políticas desarrollado por AWS. Conceptos clave:

```
Principal (quién) → Action (qué) → Resource (sobre qué) → Context (condiciones)
```

**Estructura de una política Cedar:**

```
permit(
  principal in Role::"developer",
  action in [Action::"read", Action::"write"],
  resource in Resource::"project/*"
);
```

**Conceptos fundamentales:**
- **Entities**: Tipos como `User`, `Role`, `Resource`, `Action`
- **Policies**: Reglas `permit` o `forbid`
- **Schema**: Definición de tipos y relaciones (equivale a un schema JSON)
- **Authorizer**: Motor que evalúa políticas contra una request

### 2.3 Dominios de cognicode-core

El axiom crate opera sobre los siguientes tipos del dominio:

```rust
// Tipos principales consumidos de cognicode-core
use cognicode_core::analysis::{CallGraph, ComplexityMetrics, SymbolInfo};
use cognicode_core::graph::{CycleReport, ImpactScore};
use cognicode_core::parsing::{ParsedFile, Language};
```

### 2.4 Setup del Entorno

```bash
# Clonar workspace si no existe
git clone https://github.com/cognicode/workspace.git
cd workspace

# Verificar estructura
cargo metadata --format-version 1 | jq '.workspace_members[]' | grep axiom

# Dependencias del sistema
# - SQLite (para audit trail)
# - tree-sitter CLI (para parsing de ADRs)
# - Clippy, ESLint, Semgrep (para linters)
```

---

## 3. Fase 1: Fundación del Crate + Cedar Engine

**Duración:** 2 semanas
**Objetivo:** Crate esqueleto funcional con motor de evaluación de políticas Cedar

### T1: Crear esqueleto del crate `cognicode-axiom`

**Descripción:** Crear la estructura inicial del crate dentro del workspace de CogniCode.

**Archivos afectados:**
```
crates/cognicode-axiom/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   └── main.rs (binario opcional)
└── tests/
    └── integration_tests.rs
```

**Dependencias:** Ninguna (creación pura)

**Criterios de aceptación:**
- [ ] `cargo build -p cognicode-axiom` compila sin errores
- [ ] Estructura de módulos definida: `policy/`, `rules/`, `quality/`, `audit/`, `reflection/`
- [ ] README.md con descripción del crate

---

### T2: Añadir dependencia cedar-policy a Cargo.toml

**Descripción:** Agregar el crate `cedar-policy` con features necesarias.

**Cargo.toml:**
```toml
[dependencies]
cedar-policy = "3.0"
cedar-policy-core = "3.0"
serde = { version = "1.0", features = ["derive"] }
thiserror = "1.0"
```

**Esfuerzo estimado:** 1 hora
**Dependencias:** T1

**Criterios de aceptación:**
- [ ] `cargo check -p cognicode-axiom` sin warnings de dependencias
- [ ] Documentación de CedarPolicy crate accesible

---

### T3: Implementar `policy/engine.rs` — Cedar Authorizer wrapper

**Descripción:** Wrapper sobre el Cedar Authorizer para evaluación de políticas.

**API pública propuesta:**
```rust
pub struct PolicyEngine {
    authorizer: cedar_policy::Authorizer,
    policies: cedar_policy::PolicySet,
    schema: cedar_policy::Schema,
}

impl PolicyEngine {
    pub fn new(schema: cedar_policy::Schema) -> Self;
    pub fn add_policy(&mut self, policy: cedar_policy::Policy) -> Result<(), AxiomError>;
    pub fn evaluate(&self, request: &AuthRequest) -> Result<Decision, AxiomError>;
    pub fn with_default_policies() -> Result<Self, AxiomError>;
}
```

**Archivos afectados:**
- `src/policy/engine.rs` (nuevo)
- `src/lib.rs` (actualizar exports)

**Esfuerzo estimado:** 4 horas
**Dependencias:** T2

**Criterios de aceptación:**
- [ ] `evaluate()` retorna `Decision::Allow` o `Decision::Deny`
- [ ] Manejo de errores concretos (`AxiomError::PolicyNotFound`, etc.)
- [ ] Tests unitarios con requests conocidas

---

### T4: Implementar `policy/loader.rs` — Carga de archivos .cedar

**Descripción:** Utilidad para cargar políticas desde archivos del filesystem.

**API propuesta:**
```rust
pub struct PolicyLoader {
    policy_dir: PathBuf,
}

impl PolicyLoader {
    pub fn new(policy_dir: PathBuf) -> Self;
    pub fn load_all(&self) -> Result<Vec<cedar_policy::Policy>, AxiomError>;
    pub fn load_single(&self, filename: &str) -> Result<cedar_policy::Policy, AxiomError>;
    pub fn watch_for_changes(&mut self) -> Result<Notify, AxiomError>;
}
```

**Archivos afectados:**
- `src/policy/loader.rs` (nuevo)
- `src/policy/mod.rs` (actualizar)

**Esfuerzo estimado:** 3 horas
**Dependencias:** T3

**Criterios de aceptación:**
- [ ] Carga archivos `.cedar` desde directorio configurado
- [ ] Valida sintaxis de cada política al cargar
- [ ] Reporta errores de parseo con línea y columna

---

### T5: Implementar `policy/schema.rs` — Entity types para AI agents

**Descripción:** Definir el schema de entidades Cedar para el dominio CogniCode.

**Entidades del dominio:**
```cedar
entity User = {
  roles: Set<Role>,
  clearance: Level,
  lastActive: datetime
};

entity Role = {
  permissions: Set<Permission>
};

entity Resource = {
  owner: User,
  project: Project,
  sensitivity: Level
};

entity Action = {
  category: ActionCategory,
  requiresConfirmation: boolean
};

entity Project = {
  team: Set<User>,
  complianceLevel: Level
};

entity AI_Agent = {
  capabilities: Set<Capability>,
  trustLevel: Level,
  sessionId: Session
};
```

**Archivos afectados:**
- `src/policy/schema.rs` (nuevo)
- `schemas/cognicode-schema.cedar` (archivo de schema)

**Esfuerzo estimado:** 4 horas
**Dependencias:** T3

**Criterios de aceptación:**
- [ ] Schema cubre todos los casos de uso del dominio
- [ ] Entidades `AI_Agent` y `Session` incluidos
- [ ] Tests de validación de requests contra schema

---

### T6: Implementar `rules/store.rs` — Rule CRUD operations

**Descripción:** Almacenamiento de reglas dinámicas en memoria (con opción de persistencia).

**API propuesta:**
```rust
pub struct RuleStore {
    rules: RwLock<HashMap<RuleId, Rule>>,
    event_emitter: EventEmitter,
}

impl RuleStore {
    pub fn new() -> Self;
    pub fn add(&self, rule: Rule) -> Result<RuleId, AxiomError>;
    pub fn remove(&self, id: &RuleId) -> Result<(), AxiomError>;
    pub fn update(&self, id: &RuleId, rule: Rule) -> Result<(), AxiomError>;
    pub fn get(&self, id: &RuleId) -> Option<Rule>;
    pub fn list(&self, filter: Option<RuleFilter>) -> Vec<Rule>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: RuleId,
    pub name: String,
    pub description: String,
    pub cedar_policy: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Archivos afectados:**
- `src/rules/store.rs` (nuevo)
- `src/rules/mod.rs` (nuevo)
- `src/rules/validator.rs` (nuevo)
- `src/lib.rs` (actualizar)

**Esfuerzo estimado:** 6 horas
**Dependencias:** T3, T4

**Criterios de aceptación:**
- [ ] CRUD completo con `add`, `remove`, `update`, `get`
- [ ] Persistencia en SQLite como backend opcional
- [ ] Eventos de cambio emitidos para cache invalidation

---

### T7: Implementar `rules/validator.rs` — Validación de políticas contra schema

**Descripción:** Validar que las políticas Cedar sean sintáctica y semánticamente válidas.

**API propuesta:**
```rust
impl RuleValidator {
    pub fn validate(policy_text: &str, schema: &Schema) -> Result<ValidationResult, AxiomError>;
    pub fn validate_batch(policies: &[String]) -> Vec<ValidationResult>;
}

pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug)]
pub struct ValidationError {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub error_code: ErrorCode,
}
```

**Esfuerzo estimado:** 4 horas
**Dependencias:** T5, T6

**Criterios de aceptación:**
- [ ] Detecta errores de sintaxis Cedar
- [ ] Valida que actions referenced existan en el schema
- [ ] Detecta policies vacías o duplicadas

---

### T8: Tests unitarios para evaluación Cedar

**Descripción:** Suite de tests unitarios para el motor Cedar.

**Casos de prueba:**
```rust
#[cfg(test)]
mod cedar_tests {
    use super::*;

    #[test]
    fn test_simple_permit() {
        let engine = PolicyEngine::with_default_policies().unwrap();
        let request = AuthRequest {
            principal: EntityUid::from_str("User::\"alice\""),
            action: Action::Read,
            resource: ResourceUid::from_str("Resource::\"doc-1\""),
            context: Context::empty(),
        };
        assert_eq!(engine.evaluate(&request).unwrap(), Decision::Allow);
    }

    #[test]
    fn test_role_based_deny() {
        // Usuario sin rol requerido
    }

    #[test]
    fn test_sensitivity_level_enforcement() {
        // Recursos de alta sensibilidad requieren clearance
    }

    #[test]
    fn test_compound_policy_evaluation() {
        // Políticas con múltiples condiciones
    }
}
```

**Archivos afectados:**
- `src/policy/engine.rs` (tests embebidos)
- `tests/cedar_integration.rs` (tests de integración)

**Esfuerzo estimado:** 4 horas
**Dependencias:** T3, T4, T5

**Criterios de aceptación:**
- [ ] >90% coverage en `policy/` module
- [ ] Tests de edge cases (entidades vacías, políticas inválidas)
- [ ] Tests de performance (< 1ms por evaluación)

---

### T9: Test de integración: load policies → evaluate → decision

**Descripción:** Test end-to-end del flujo completo de evaluación.

```rust
#[test]
fn test_full_policy_evaluation_flow() {
    // 1. Setup: crear schema y policies
    let schema = load_cognicode_schema();
    let mut engine = PolicyEngine::new(schema);

    // 2. Cargar políticas desde archivos
    let loader = PolicyLoader::new(PathBuf::from("policies/"));
    let policies = loader.load_all().unwrap();
    for policy in policies {
        engine.add_policy(policy).unwrap();
    }

    // 3. Evaluar request
    let request = AuthRequest::new(
        principal: "User::\"dev\"".parse().unwrap(),
        action: "Action::\"analyze\"".parse().unwrap(),
        resource: "Resource::\"project/backend\"".parse().unwrap(),
    );

    // 4. Verificar decisión
    let decision = engine.evaluate(&request).unwrap();
    assert_matches!(decision, Decision::Allow);
}
```

**Esfuerzo estimado:** 2 horas
**Dependencias:** T8

**Criterios de aceptación:**
- [ ] Test pasa con políticas de ejemplo del repo
- [ ] Tiempo total < 10ms

---

### T10: Añadir herramientas MCP: check_action, add_rule, remove_rule, validate_rule

**Descripción:** Integrar las operaciones del policy engine como herramientas MCP.

**Herramientas MCP:**
```rust
#[mcp_tool]
async fn check_action(
    principal: String,
    action: String,
    resource: String,
    context: Option<HashMap<String, String>>,
) -> Result<Decision, AxiomError>;

#[mcp_tool]
async fn add_rule(
    name: String,
    description: String,
    cedar_policy: String,
    tags: Option<Vec<String>>,
) -> Result<RuleId, AxiomError>;

#[mcp_tool]
async fn remove_rule(rule_id: RuleId) -> Result<(), AxiomError>;

#[mcp_tool]
async fn validate_rule(policy_text: String) -> Result<ValidationResult, AxiomError>;
```

**Archivos afectados:**
- `src/mcp/tools.rs` (nuevo)
- `src/lib.rs` (registro de tools)
- Integración con `cognicode-mcp`

**Esfuerzo estimado:** 6 horas
**Dependencias:** T6, T7

**Criterios de aceptación:**
- [ ] Tools visibles en `mcp__tools__list`
- [ ] Cada tool retorna JSON estructurado
- [ ] Manejo de errores con mensajes útiles

---

## 4. Fase 2: Análisis de Calidad + Boundaries

**Duración:** 2 semanas
**Objetivo:** Integrar análisis de calidad de código con métricas de cognicode-core

### T11: Implementar `quality/lcom.rs` — Calculadora LCOM

**Descripción:** Implementar Lack of Cohesion of Methods (LCOM) usando call graph.

**Fórmula LCOM:**
```
LCOM = 1 - (sum(P) / (M * C))
donde:
  - P = número de pares de métodos que comparten atributos
  - M = número de métodos
  - C = número de clases/atributos
```

**API propuesta:**
```rust
pub struct LcomCalculator {
    call_graph: CallGraph,
}

impl LcomCalculator {
    pub fn new(call_graph: CallGraph) -> Self;
    pub fn calculate_for_struct(&self, struct_name: &str) -> LcomResult;
    pub fn calculate_all(&self) -> HashMap<String, LcomResult>;
}

#[derive(Debug, Clone)]
pub struct LcomResult {
    pub struct_name: String,
    pub lcom_score: f64,           // 0.0 (cohesivo) a 1.0+ (no cohesivo)
    pub method_count: usize,
    pub cohesion_deficit: f64,
    pub suggestions: Vec<Suggestion>,
}
```

**Threshold LCOM:**
| Score | Interpretación | Acción |
|-------|----------------|--------|
| 0.0 - 0.3 | Alta cohesión | OK |
| 0.3 - 0.5 | Cohesión moderada | Considerar refactor |
| 0.5 - 0.7 | Baja cohesión | Refactor recomendado |
| > 0.7 | Muy baja cohesión | Refactor urgente |

**Archivos afectados:**
- `src/quality/lcom.rs` (nuevo)
- `src/quality/mod.rs` (nuevo)

**Esfuerzo estimado:** 6 horas
**Dependencias:** T1 (acceso a cognicode-core)

**Criterios de aceptación:**
- [ ] Score LCOM para cada struct en el proyecto
- [ ] Comparación con thresholds estándar
- [ ] Sugerencias de refactor concretas

---

### T12: Implementar `quality/connascence.rs` — Métricas de acoplamiento

**Descripción:** Analizar connascence (acoplamiento semántico) entre módulos.

**Tipos de connascence a detectar:**

| Tipo | Descripción | Severidad |
|------|-------------|-----------|
| Connascence de nombre (CoN) | Mismos nombres en módulos distintos | Baja |
| Connascence de tipo (CoT) | Tipos compartidos | Baja |
| Connascence de significado (CoM) | Significado compartido de datos | Media |
| Connascence de algoritmo (CoA) | Mismo algoritmo exacto | Media |
| Connascence de posición (CoP) | Orden de argumentos | Alta |
| Connascence de timing (CoTm) | Orden temporal de operaciones | Alta |

**API propuesta:**
```rust
pub struct ConnascenceAnalyzer {
    dependency_graph: DependencyGraph,
    symbol_info: Arc<SymbolStore>,
}

impl ConnascenceAnalyzer {
    pub fn analyze_module(&self, module: &str) -> ConnascenceReport;
    pub fn find_violations(&self, thresholds: &Thresholds) -> Vec<Violation>;
    pub fn calculate_coupling_score(&self) -> f64;
}
```

**Esfuerzo estimado:** 8 horas
**Dependencias:** T11

**Criterios de aceptación:**
- [ ] Detecta los 6 tipos de connascence
- [ ] Reporte por módulo con score de severidad
- [ ] Visualización del grafo de acoplamiento

---

### T13: Implementar `quality/solid.rs` — Verificaciones SOLID

**Descripción:** Heurísticas para detectar violaciones de principios SOLID.

**Implementación por principio:**

```rust
pub struct SolidChecker { /* ... */ }

// SRP: Single Responsibility Principle
// Violación: > 5 razones para cambiar un struct
impl SolidChecker {
    pub fn check_srp(&self, struct_name: &str) -> SrpResult;

    // OCP: Open/Closed Principle
    // Violación: Métodos que modifican entidades externas sin extensión
    pub fn check_ocp(&self, struct_name: &str) -> OcpResult;

    // LSP: Liskov Substitution Principle
    // Violación: Traits con precondiciones más fuertes
    pub fn check_lsp(&self, struct_name: &str) -> LspResult;

    // ISP: Interface Segregation Principle
    // Violación: Structs que implementan métodos no usados
    pub fn check_isp(&self, struct_name: &str) -> IspResult;

    // DIP: Dependency Inversion Principle
    // Violación: Módulos de alto nivel dependen de bajo nivel
    pub fn check_dip(&self, struct_name: &str) -> DipResult;
}
```

**Heurísticas implementadas:**

| Principio | Heurística | Threshold |
|-----------|------------|-----------|
| SRP | Métodos que acceden a > 3 campos privados de otras clases | > 3 |
| OCP | Métodos `modify_*` que no usan trait extension | > 0 |
| LSP | Override con precondiciones más fuertes | > 0 |
| ISP | % de métodos implementados que son usados | < 50% |
| DIP | Distancia abstractión (instability vs abstractness) | DA > 1 |

**Esfuerzo estimado:** 10 horas
**Dependencias:** T12

**Criterios de aceptación:**
- [ ] Reporte con score por cada principio SOLID
- [ ] Sugerencias específicas de refactor
- [ ] Dashboard de calidad general

---

### T14: Implementar `quality/delta.rs` — Comparación before/after

**Descripción:** Calcular delta de calidad entre dos snapshots.

**API propuesta:**
```rust
pub struct QualityDelta {
    pub before: QualitySnapshot,
    pub after: QualitySnapshot,
    pub changes: Vec<QualityChange>,
}

impl QualityDelta {
    pub fn compare(before: &QualitySnapshot, after: &QualitySnapshot) -> Self;
    pub fn score(&self) -> DeltaScore;
    pub fn report(&self) -> DeltaReport;
}

#[derive(Debug, Clone)]
pub struct QualitySnapshot {
    pub timestamp: DateTime<Utc>,
    pub lcom_scores: HashMap<String, f64>,
    pub solid_scores: HashMap<String, SolidScore>,
    pub connascence_scores: HashMap<String, f64>,
    pub complexity_metrics: ComplexityMetrics,
}
```

**Esfuerzo estimado:** 4 horas
**Dependencias:** T11, T12, T13

**Criterios de aceptación:**
- [ ] Serialización de snapshots a JSON
- [ ] Cálculo de diff entre métricas
- [ ] Generación de reporte HTML simple

---

### T15: Implementar boundary checking con CallGraph existente

**Descripción:** Validar que las dependencias respeten los boundaries DDD/hexagonal.

**Reglas de boundary:**

```
Domain Core ────→ Application Services ────→ Infrastructure
     │                    │
     │                    │
  No deps              No deps to
  outward            Domain Core
```

**API propuesta:**
```rust
pub struct BoundaryChecker {
    call_graph: CallGraph,
    boundaries: Vec<BoundaryDefinition>,
}

impl BoundaryChecker {
    pub fn new(boundaries: Vec<BoundaryDefinition>) -> Self;
    pub fn check_violations(&self) -> Vec<BoundaryViolation>;
    pub fn is_within_boundary(&self, from: &str, to: &str) -> bool;
}

#[derive(Debug)]
pub struct BoundaryViolation {
    pub from_module: String,
    pub to_module: String,
    pub boundary: String,
    pub violation_type: ViolationType,
    pub severity: Severity,
}
```

**Tipos de violación:**
- `CrossBoundaryDependency`: Módulo intenta usar otro fuera de su allowed scope
- `ImplicitDependency`: Dependencia no declarada en Cargo.toml
- `CyclicDependency`: Ciclo entre boundaries (usa CycleDetector)

**Esfuerzo estimado:** 6 horas
**Dependencias:** T1 (cognicode-core CallGraph)

**Criterios de aceptación:**
- [ ] Detecta todas las violaciones de boundaries
- [ ] Usa CycleDetector de cognicode-core para ciclos
- [ ] Reporte con path completo de la violación

---

### T16: Añadir MCP tools: check_quality, quality_delta, check_boundaries

**Descripción:** Exponer herramientas de análisis de calidad via MCP.

```rust
#[mcp_tool]
async fn check_quality(
    project_path: String,
    metrics: Option<Vec<String>>, // ["lcom", "solid", "connascence"]
) -> Result<QualityReport, AxiomError>;

#[mcp_tool]
async fn quality_delta(
    before_snapshot: QualitySnapshot,
    after_snapshot: QualitySnapshot,
) -> Result<DeltaReport, AxiomError>;

#[mcp_tool]
async fn check_boundaries(
    project_path: String,
    boundaries_config: Option<String>,
) -> Result<Vec<BoundaryViolation>, AxiomError>;
```

**Esfuerzo estimado:** 4 horas
**Dependencias:** T14, T15

**Criterios de aceptación:**
- [ ] Herramientas listadas en MCP
- [ ] Retorna JSON estructurado con todos los datos
- [ ] Performance: análisis completo < 30s para proyecto de 100 archivos

---

### T17: Integrar boundary rules en Cedar policies

**Descripción:** Usar resultados de boundary check como input para decisiones de autorización.

**Ejemplo de política:**
```
permit(
  principal in Role::"developer",
  action == Action::"deploy",
  resource == Resource::"production/*"
)
when {
  // Verificar que el código no viola boundaries críticos
  context.code_quality_score >= 7.0 &&
  context.boundary_violations == []
};
```

**Esfuerzo estimado:** 4 horas
**Dependencias:** T15, T16

**Criterios de aceptación:**
- [ ] Policies referencian `context.code_quality_score`
- [ ] Policy engine pasa context enriquecido
- [ ] Test de evaluación con context

---

### T18: Tests de integración con proyectos reales de CogniCode

**Descripción:** Validar que axiom funciona correctamente con proyectos del workspace.

**Proyectos de test:**
- `cognicode-core` — Test con grafo de llamadas real
- `cognicode-mcp` — Test con herramientas MCP
- Proyectos de ejemplo del workspace

**Esfuerzo estimado:** 8 horas
**Dependencias:** T17

**Criterios de aceptación:**
- [ ] Todos los tests de integración pasan
- [ ] No hay regressions en cognicode-core
- [ ] Cobertura combinada > 70%

---

## 5. Fase 3: ADR + Linters + Audit

**Duración:** 2 semanas
**Objetivo:** ADR parsing, wrappers de linters, y audit trail completo

### T19: Implementar `rules/adr_parser.rs` — Parse ADR markdown

**Descripción:** Parsear Architecture Decision Records del formato estándar.

**Formato ADR aceptado:**
```markdown
---
status: accepted
date: 2024-01-15
deciders: alice, bob
---

# ADR-001: Usar Cedar para políticas de autorización

## Contexto

Necesitamos un motor de políticas declarativo...

## Decisión

Usar Cedar Policy de AWS...

## Consecuencias

Positivas: Evaluación rápida, tipo-safe...
Negativas: Vendor lock-in con AWS...
```

**API propuesta:**
```rust
pub struct AdrParser {
    frontmatter_parser: frontmatter::Parser,
}

impl AdrParser {
    pub fn parse(content: &str) -> Result<ParsedAdr, AdrError>;
    pub fn parse_file(path: &Path) -> Result<ParsedAdr, AdrError>;
    pub fn to_cedar_rules(adr: &ParsedAdr) -> Vec<String>;
}
```

**Conversión ADR → Cedar:**

| Campo ADR | Conversión Cedar |
|-----------|-----------------|
| `deciders` | Entidad `DecisionMaker` en contexto |
| `date` | Timestamp de auditoría |
| `status: accepted` | Policy habilitada |
| `status: deprecated` | Policy deshabilitada |
| Consecuencias positivas | Condiciones `when` permisivas |
| Consecuencias negativas | Condiciones `unless` restrictivas |

**Esfuerzo estimado:** 8 horas
**Dependencias:** T6

**Criterios de aceptación:**
- [ ] Parsea frontmatter YAML
- [ ] Extrae todos los campos del ADR
- [ ] Genera rules para políticas relacionadas
- [ ] Detecta ADRs obsoletas (status: deprecated)

---

### T20: Implementar `linters/clippy.rs` — Wrapper cargo clippy

**Descripción:** Wrapper para invocar clippy y parsear resultados.

```rust
pub struct ClippyRunner {
    cargo_path: PathBuf,
    clippy_args: Vec<String>,
}

impl Linter for ClippyRunner {
    fn run(&self, project_path: &Path) -> Result<LinterReport, AxiomError>;
    fn name(&self) -> &str { "clippy" }
}

#[derive(Debug, Clone)]
pub struct LinterReport {
    pub linter_name: String,
    pub execution_time_ms: u64,
    pub issues: Vec<LinterIssue>,
    pub summary: LinterSummary,
}

#[derive(Debug)]
pub struct LinterIssue {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}
```

**Esfuerzo estimado:** 4 horas
**Dependencias:** T1

**Criterios de aceptación:**
- [ ] Invoca `cargo clippy` como subprocess
- [ ] Parsea output JSON de clippy
- [ ] Mapea severities: warning → Medium, error → High
- [ ] Soporta filtros por allow/warn/deny

---

### T21: Implementar `linters/eslint.rs` — Wrapper npx eslint

**Descripción:** Wrapper para invocar ESLint en proyectos JavaScript/TypeScript.

**API similar a T20** con adaptaciones para:
- Detección automática de `node_modules/.bin/eslint`
- Soporte para `.eslintrc.json`, `.eslintrc.yml`, `eslint.config.js`
- Formato de output `--format json`

**Esfuerzo estimado:** 4 horas
**Dependencias:** T20

**Criterios de aceptación:**
- [ ] Detecta automáticamente el binario de eslint
- [ ] Parsea output JSON estándar de ESLint
- [ ] Filtra por severity configurado
- [ ] Soporta TypeScript via `@typescript-eslint/parser`

---

### T22: Implementar `linters/semgrep.rs` — Wrapper semgrep CLI

**Descripción:** Wrapper para Semgrep con reglas predefinidas.

**Reglas predefinidas:**
```yaml
rules:
  - id: cognicode/secure-random
    pattern: $X = Math.random()
    message: Usar crypto.randomBytes() en lugar de Math.random()
    severity: ERROR

  - id: cognicode/hardcoded-secret
    pattern: $X = "$SECRET"
    message: Posible secreto hardcodeado detectado
    severity: ERROR
```

**API:**
```rust
pub struct SemgrepRunner {
    semgrep_path: PathBuf,
    rules_dir: PathBuf,
}

impl Linter for SemgrepRunner {
    fn run(&self, project_path: &Path) -> Result<LinterReport, AxiomError>;
    fn name(&self) -> &str { "semgrep" }
}
```

**Esfuerzo estimado:** 4 horas
**Dependencias:** T20

**Criterios de aceptación:**
- [ ] Invoca `semgrep --config=rules/` como subprocess
- [ ] Soporte para reglas custom en YAML
- [ ] Incluye ruleset de seguridad standard de Semgrep

---

### T23: Implementar `audit/trail.rs` — SQLite audit logging

**Descripción:** Logging persistente de todas las decisiones y acciones.

**Schema SQLite:**
```sql
CREATE TABLE audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    event_type TEXT NOT NULL,
    principal TEXT,
    action TEXT,
    resource TEXT,
    decision TEXT,
    policy_id TEXT,
    context_json TEXT,
    metadata_json TEXT,
    duration_ms INTEGER
);

CREATE INDEX idx_audit_timestamp ON audit_events(timestamp);
CREATE INDEX idx_audit_principal ON audit_events(principal);
CREATE INDEX idx_audit_event_type ON audit_events(event_type);
```

**API propuesta:**
```rust
pub struct AuditTrail {
    db: Connection,
}

impl AuditTrail {
    pub fn new(db_path: &Path) -> Result<Self, AxiomError>;
    pub fn log(&self, event: AuditEvent) -> Result<u64, AxiomError>;
    pub fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEvent>, AxiomError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub principal: Option<String>,
    pub action: Option<String>,
    pub resource: Option<String>,
    pub decision: Option<Decision>,
    pub policy_id: Option<String>,
    pub context: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    PolicyEvaluation,
    RuleCreated,
    RuleUpdated,
    RuleDeleted,
    LinterRun,
    QualityCheck,
    BoundaryViolation,
    ReflexionEvent,
}
```

**Esfuerzo estimado:** 6 horas
**Dependencias:** T10

**Criterios de aceptación:**
- [ ] Escritura asíncrona (no bloquea evaluación)
- [ ] Índices para queries frecuentes
- [ ] TTL automático para eventos antiguos (configurable)

---

### T24: Implementar `audit/report.rs` — Query y generación de reportes

**Descripción:** Utilities para consultar y generar reportes del audit trail.

```rust
pub struct AuditReporter {
    trail: AuditTrail,
}

impl AuditReporter {
    pub fn summary(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> ReportSummary;
    pub fn by_principal(&self, principal: &str) -> Vec<AuditEvent>;
    pub fn violations(&self, severity: Severity) -> Vec<ViolationReport>;
    pub fn trends(&self, metric: &str, interval: Interval) -> Vec<DataPoint>;
}

pub struct ReportSummary {
    pub total_events: u64,
    pub by_event_type: HashMap<String, u64>,
    pub by_decision: HashMap<Decision, u64>,
    pub average_duration_ms: f64,
    pub top_violations: Vec<ViolationReport>,
}
```

**Esfuerzo estimado:** 4 horas
**Dependencias:** T23

**Criterios de aceptación:**
- [ ] Reporte semanal/mensual automático
- [ ] Exportación a CSV y JSON
- [ ] Visualización de trends (Markdown tables)

---

### T25: Añadir MCP tools: get_audit_trail, check_lint

**Descripción:** Exponer herramientas de audit y linters via MCP.

```rust
#[mcp_tool]
async fn get_audit_trail(
    from_date: Option<String>,
    to_date: Option<String>,
    event_type: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<AuditEvent>, AxiomError>;

#[mcp_tool]
async fn check_lint(
    project_path: String,
    linters: Option<Vec<String>>, // ["clippy", "eslint", "semgrep"]
    fail_on: Option<String>,      // "error", "warning", "never"
) -> Result<LintReport, AxiomError>;
```

**Esfuerzo estimado:** 4 horas
**Dependencias:** T22, T24

**Criterios de aceptación:**
- [ ] `get_audit_trail` soporta filtros y paginación
- [ ] `check_lint` invoca múltiples linters en paralelo
- [ ] Retorna reporte estructurado con issues

---

### T26: Integración con Claude Code hooks (PreToolUse/PostToolUse)

**Descripción:** Integrar axiom como Claude Code hook para enforcement automático.

**Configuración del hook:**
```json
{
  "hooks": {
    "PreToolUse": [
      {
        "name": "axiom-policy-check",
        "description": "Evaluar políticas antes de ejecutar herramientas",
        "trigger": {
          "tool_names": ["Bash", "Write", "Edit", "Read"]
        }
      }
    ],
    "PostToolUse": [
      {
        "name": "axiom-audit",
        "description": "Registrar uso de herramientas",
        "trigger": {
          "tool_names": ["*"]
        }
      }
    ]
  }
}
```

**Flujo PreToolUse:**
```
1. Claude Code invoca herramienta
2. Hook axiom intercede
3. axiom evalúa políticas con:
   - principal: usuario actual
   - action: nombre de la herramienta
   - resource: archivo/path objetivo
   - context: estado actual del workspace
4. Si Allow → continuar
   Si Deny → retornar error + sugerencia
```

**Flujo PostToolUse:**
```
1. Herramienta ejecuta exitosamente
2. Hook axiom registra:
   - Qué herramienta se usó
   - Con qué argumentos
   - En qué timestamp
   - Duración de ejecución
3. Si herramienta es linter → procesar output
4. Si violations → generar feedback
```

**Esfuerzo estimado:** 10 horas
**Dependencias:** T23, T25

**Criterios de aceptación:**
- [ ] Hook responde en < 50ms
- [ ] Cache de decisiones para herramientas frecuentes
- [ ] Error messages útiles cuando Deny
- [ ] Integración documentada para usuarios

---

## 6. Fase 4: Reflexión + Memory

**Duración:** 2 semanas
**Objetivo:** Sistema de reflexión episódica y aprendizaje from corrections

### T27: Implementar `reflection/memory.rs` — Episodic memory storage

**Descripción:** Almacenamiento de experiencias del sistema (episodios).

**Episodio:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: EpisodeId,
    pub timestamp: DateTime<Utc>,
    pub situation: Situation,
    pub actions: Vec<Action>,
    pub outcome: Outcome,
    pub lessons: Vec<Lesson>,
}

#[derive(Debug)]
pub struct Situation {
    pub context: HashMap<String, String>,
    pub policy_state: PolicyState,
    pub quality_metrics: QualitySnapshot,
    pub recent_events: Vec<AuditEvent>,
}

#[derive(Debug)]
pub struct Outcome {
    pub success: bool,
    pub violations: Vec<Violation>,
    pub feedback_score: f64,
    pub user_correction: Option<String>,
}
```

**Storage:**
```rust
pub struct EpisodicMemory {
    db: Connection,
    max_episodes: usize,
}

impl EpisodicMemory {
    pub fn store(&self, episode: Episode) -> Result<EpisodeId, AxiomError>;
    pub fn retrieve(&self, filter: EpisodeFilter) -> Vec<Episode>;
    pub fn similar_situations(&self, situation: &Situation) -> Vec<Episode>;
    pub fn recent(&self, count: usize) -> Vec<Episode>;
}
```

**Esfuerzo estimado:** 8 horas
**Dependencias:** T23

**Criterios de aceptación:**
- [ ] Almacenamiento persistente en SQLite
- [ ] Búsqueda por similitud de situación
- [ ] TTL para episodios antiguos (configurable)

---

### T28: Implementar `reflection/feedback.rs` — Structured feedback generation

**Descripción:** Generar feedback estructurado desde violaciones y errores.

```rust
pub struct FeedbackGenerator {
    templates: HashMap<ViolationType, FeedbackTemplate>,
}

impl FeedbackGenerator {
    pub fn generate(&self, violation: &Violation) -> Feedback;
    pub fn generate_from_violations(&self, violations: &[Violation]) -> Vec<Feedback>;
    pub fn generate_correction_hint(&self, violation: &Violation) -> String;
}

#[derive(Debug, Clone)]
pub struct Feedback {
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub affected_files: Vec<PathBuf>,
    pub suggestion: String,
    pub learning_opportunity: Option<String>,
    pub links: Vec<ResourceLink>,
}

#[derive(Debug)]
pub struct FeedbackTemplate {
    pub title_template: String,
    pub description_template: String,
    pub suggestion_template: String,
    pub examples: Vec<String>,
}
```

**Templates de ejemplo:**
```rust
let templates = hashmap! {
    ViolationType::BoundaryCrossing => FeedbackTemplate {
        title_template: "Violación de boundary: {from} → {to}",
        description_template: "El módulo {from} está accediendo a {to} violando los límites de arquitectura",
        suggestion_template: "Usar patrón de adapter o mover la lógica a una capa compartida",
        examples: vec![
            "Domain::validate() no debería invocar Infrastructure::db",
        ],
    },
    // ... más templates
};
```

**Esfuerzo estimado:** 6 horas
**Dependencias:** T27

**Criterios de aceptación:**
- [ ] Feedback específico por tipo de violación
- [ ] Incluye links a documentación relevante
- [ ] Sugerencias accionables y concretas

---

### T29: Implementar `reflection/loop.rs` — Integración del ciclo de reflexión

**Descripción:** Integrar situación → acción → evaluación → aprendizaje.

```rust
pub struct ReflexionLoop {
    memory: EpisodicMemory,
    feedback: FeedbackGenerator,
    policy_engine: PolicyEngine,
    learning: LearningEngine,
}

impl ReflexionLoop {
    pub fn new(
        memory: EpisodicMemory,
        feedback: FeedbackGenerator,
        policy_engine: PolicyEngine,
    ) -> Self;

    /// Ciclo principal de reflexión
    pub async fn reflect(
        &self,
        situation: Situation,
        proposed_actions: Vec<Action>,
    ) -> ReflectionResult;

    /// Evaluar resultado de acciones pasadas
    pub async fn evaluate_outcome(&self, episode_id: EpisodeId) -> EvaluationResult;

    /// Detectar patrones en episodios recientes
    pub fn detect_patterns(&self) -> Vec<Pattern>;
}
```

**Diagrama del ciclo:**

```
┌─────────────────────────────────────────────────────────────────┐
│                          REFLEXION LOOP                          │
│                                                                  │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │ OBSERVE  │───▶│ ANALYZE  │───▶│ DECIDE    │───▶│ ACT      │  │
│  │ situación│    │ patrones │    │ acciones │    │ ejecutar │  │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘  │
│       │                                            │             │
│       │                                            ▼             │
│       │                                     ┌──────────┐       │
│       │                                     │ EVALUATE │       │
│       │                                     │ outcome  │       │
│       │                                     └──────────┘       │
│       │                                          │             │
│       ▼                                          ▼             │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    LEARN (actualizar reglas)             │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

**Esfuerzo estimado:** 8 horas
**Dependencias:** T27, T28

**Criterios de aceptación:**
- [ ] Ciclo completo ejecutable
- [ ] No hay recursion infinita
- [ ] Logging de cada paso

---

### T30: Implementar aprendizaje: corrections → nuevas reglas

**Descripción:** Sistema que aprende de correcciones del usuario para generar nuevas políticas.

```rust
pub struct LearningEngine {
    memory: EpisodicMemory,
    rule_store: RuleStore,
    min_confirmation_rate: f64,
}

impl LearningEngine {
    /// Analizar correcciones del usuario para detectar patrones
    pub fn analyze_corrections(&self, timeframe: TimeFrame) -> Vec<LearningOpportunity>;

    /// Generar propuesta de nueva regla desde corrección
    pub fn propose_rule(&self, correction: &UserCorrection) -> ProposedRule;

    /// Validar que la regla propuesta no contradice existentes
    pub fn validate_proposed_rule(&self, rule: &ProposedRule) -> ValidationResult;

    /// Confirmar aprendizaje (usuario approves)
    pub fn confirm_learning(&self, proposal_id: ProposalId) -> Result<RuleId, AxiomError>;

    /// Calcular confidence de una regla propuesta
    pub fn calculate_confidence(&self, proposal: &ProposedRule) -> f64;
}

#[derive(Debug)]
pub struct LearningOpportunity {
    pub pattern: String,
    pub occurrence_count: u32,
    pub avg_user_correction_time: Duration,
    pub affected_policies: Vec<PolicyId>,
    pub proposed_rule: ProposedRule,
    pub confidence: f64,
}
```

**Algoritmo de aprendizaje:**

```
1. DETECT: Usuario corrige decisión del sistema (Deny → Allow)
2. CONTEXTUALIZE: Buscar episodios similares en memoria
3. PATTERN: Identificar condiciones comunes en las correcciones
4. PROPOSE: Generar nueva regla con las condiciones halladas
5. VALIDATE: Verificar que la regla no contradice existentes
6. CONFIRM: Solicitar confirmación del usuario (o auto-confirmar si confidence > threshold)
7. DEPLOY: Añadir regla al RuleStore
```

**Esfuerzo estimado:** 10 horas
**Dependencias:** T29

**Criterios de aceptación:**
- [ ] Detecta patrones de correcciones recurrentes
- [ ] Genera reglas con > 70% de precision (basado en historical data)
- [ ] Tasa de false positives < 10%
- [ ] Learning loop completable en < 5 minutos

---

### T31: Añadir MCP tools: reflect_on_result, get_past_reflections, learn_rule

**Descripción:** Exponer herramientas de reflexión via MCP.

```rust
#[mcp_tool]
async fn reflect_on_result(
    tool_name: String,
    tool_args: HashMap<String, String>,
    outcome: String, // "success" | "violation" | "user_override"
    user_feedback: Option<String>,
) -> Result<ReflectionResult, AxiomError>;

#[mcp_tool]
async fn get_past_reflections(
    pattern: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<EpisodeSummary>, AxiomError>;

#[mcp_tool]
async fn learn_rule(
    learning_opportunity_id: String,
    approve: bool,
    modifications: Option<String>,
) -> Result<RuleCreationResult, AxiomError>;
```

**Esfuerzo estimado:** 4 horas
**Dependencias:** T30

**Criterios de aceptación:**
- [ ] `reflect_on_result` dispara ciclo de reflexión
- [ ] `get_past_reflections` retorna episodios resumidos
- [ ] `learn_rule` confirma o rechaza propuesta

---

### T32: Wire feedback loop en respuestas MCP

**Descripción:** Integrar feedback en todas las respuestas del sistema.

**Integración en cognicode-mcp:**
```rust
// En el response builder de cognicode-mcp
impl ToolResponse {
    pub fn with_feedback(mut self, feedback: Vec<Feedback>) -> Self {
        self.metadata.insert("axiom_feedback", serde_json::to_value(&feedback));
        self
    }

    pub fn with_violations(mut self, violations: Vec<Violation>) -> Self {
        self.metadata.insert("axiom_violations", serde_json::to_value(&violations));
        self
    }
}
```

**Ejemplo de respuesta enriquecida:**
```json
{
  "result": "...",
  "metadata": {
    "axiom_feedback": [
      {
        "severity": "medium",
        "title": "Mantenibilidad: LCOM elevado en UserService",
        "suggestion": "Considerar dividir UserService en UserRepository y UserValidator",
        "links": [
          {"text": "LCOM Wiki", "url": "https://example.com/lcom"}
        ]
      }
    ],
    "axiom_violations": []
  }
}
```

**Esfuerzo estimado:** 6 horas
**Dependencias:** T28, T31

**Criterios de aceptación:**
- [ ] Todas las respuestas MCP pueden incluir feedback
- [ ] Feedback solo se incluye si hay contenido relevante
- [ ] No hay overhead significativo (< 5ms)

---

## 7. Dependencias entre Fases

### Diagrama de Dependencias

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              FASE 1: FUNDACIÓN                               │
│  T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8 → T9 → T10                          │
│  (Foundation + Cedar Engine)                                                  │
│                          │                                                    │
│                          ▼                                                    │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                         FASE 2: CALIDAD                                │  │
│  │  T11 → T12 → T13 → T14 ───────────────────────────────────────────┐  │  │
│  │  T15 ─────────────────────────────────────────────────────────┐  │  │  │
│  │  T16 ───────────────────┐                                        │  │  │
│  │  T17 ───────────────────┼── T18                                   │  │  │
│  └──────────────────────────┼────────────────────────────────────────┼──┘  │
│                             │                                               │
│                             ▼                                               │
│  ┌─────────────────────────────────┐    ┌────────────────────────────────┐  │
│  │     FASE 3: ADR + LINTERS       │    │     FASE 4: REFLEXIÓN          │  │
│  │                                 │    │                                 │  │
│  │  T19 → T20 ─┐                  │    │  T27 → T28 → T29 → T30 ────┐   │  │
│  │       T21 ─┼── T22 ─┐          │    │                       │   │   │  │
│  │       T23 ─┼────────┼── T24    │    │                       │   │   │  │
│  │       T25 ─┼────────┼──────┐   │    │                       ▼   │   │  │
│  │       T26 ─┴────────┴──────┼───│───│── T31 → T32              │   │  │
│  └────────────────────────────┼───┼───┴───────────────────────────┼───┘  │
│                               │   │                               │        │
│                               ▼   │                               │        │
│                          INTEGRATION TESTING                       │        │
│                               │   │                               │        │
└──────────────────────────────┼───┼───────────────────────────────┼────────┘
                               │   │                               │
                               ▼   ▼                               ▼
                        ┌────────────────────────────────────────────┐
                        │         RELEASE: cognicode-axiom v1.0     │
                        └────────────────────────────────────────────┘
```

### Ruta Crítica

**T1 → T2 → T3 → T6 → T10 → T16 → T25 → T31 → T32**

```
Semanas:  1    2    3    4    5    6    7    8
          ├────┤
          T1,T2,T3
                ├────┤
                T4,T5,T6
                      ├────┤
                      T7,T8,T9
                            ├────┤
                            T10
                                  ├────┤
                                  T11-T15
                                        ├────┤
                                        T16-T18
                                              ├────┤
                                              T19-T26 (||) T27-T32
                                                              ├────┤
                                                              T31,T32
                                                                    ├────┤
                                                                    Release
```

### Tasks que pueden ejecutarse en paralelo

| Fase | Tasks | Paralelismo |
|------|-------|-------------|
| Fase 1 | T3, T4, T5 | T3 y T4 pueden desarrollarse en paralelo una vez T2 esté listo |
| Fase 2 | T11, T12, T13, T14 | Completamente paralelos — todos consumen CallGraph |
| Fase 2 | T15 | Depende de T14 para contexto de quality |
| Fase 2 | T16, T17, T18 | T16 paralelo a T17; T18 al final |
| Fase 3 | T19, T20, T21, T22 | Completamente paralelos |
| Fase 3 | T23, T24 | T24 depende de T23 |
| Fase 4 | T27, T28 | T28 depende de T27 para casos de prueba |
| Fase 4 | T29, T30 | Secuenciales — T30 consume T29 |

---

## 8. Criterios de Aceptación por Fase

### Fase 1: Fundación + Cedar Engine

| Criterio | Métrica | Verificación |
|----------|---------|--------------|
| Crate compila | `cargo build -p cognicode-axiom` sin errores | CI/CD pipeline |
| Tests pasan | > 90% coverage en policy module | `cargo tarpaulin` |
| Cedar evalúa | Request de prueba retorna Allow/Deny correcto | Test integration |
| MCP tools | 4 herramientas visibles y funcionales | `mcp__tools__list` |
| Performance | Evaluación < 1ms por request | Benchmark |
| Documentation | README.md con ejemplos de uso | Code review |

**Definition of Done:**
- [ ] Código en `main` branch
- [ ] Todos los tests pasan
- [ ] 90% coverage mínimo
- [ ] MCP tools registradas en cognicode-mcp
- [ ] Documentación completa

---

### Fase 2: Calidad + Boundaries

| Criterio | Métrica | Verificación |
|----------|---------|--------------|
| LCOM funciona | Score calculado para todos los structs | Test con proyecto real |
| Connascence | Detecta los 6 tipos documentados | Unit tests |
| SOLID | Score por cada principio para cada módulo | Integration test |
| Delta | Before/after comparison genera diff | Snapshot test |
| Boundaries | Detecta violaciones en proyectos DDD | Test con proyecto de ejemplo |
| MCP tools | 3 herramientas registradas | `mcp__tools__list` |
| Wired to Cedar | Policies consumen quality metrics | Policy evaluation test |

**Definition of Done:**
- [ ] Métricas LCOM para `cognicode-core` completas
- [ ] ConnascenceAnalyzer pasa casos de prueba estándar
- [ ] Boundary violations detectadas en cognicode-mcp
- [ ] MCP tools responde en < 30s para proyectos de 100 archivos
- [ ] Delta report genera output legible

---

### Fase 3: ADR + Linters + Audit

| Criterio | Métrica | Verificación |
|----------|---------|--------------|
| ADR parse | Parsea formato estándar con frontmatter | Unit tests |
| ADR → Cedar | Conversión genera policies válidas | Integration test |
| Clippy wrapper | Invoca y parsea output de clippy | Integration test |
| ESLint wrapper | Invoca y parsea output de eslint | Integration test |
| Semgrep wrapper | Invoca y parsea output de semgrep | Integration test |
| Audit SQLite | Escritura asíncrona, query funcional | Integration test |
| Audit reporter | Genera reportes semanales | Manual verification |
| MCP tools | 2 herramientas registradas | `mcp__tools__list` |
| Claude hooks | Hooks responden en < 50ms | Load test |

**Definition of Done:**
- [ ] Los 3 linters ejecutables via MCP
- [ ] Audit trail captura > 95% de eventos
- [ ] Claude Code hooks configurables y funcionales
- [ ] Query de audit responde en < 100ms para 100k eventos

---

### Fase 4: Reflexión + Memory

| Criterio | Métrica | Verificación |
|----------|---------|--------------|
| Episodic memory | Almacena y retrieve episodios | Unit tests |
| Feedback | Genera feedback específico por tipo | Template tests |
| Reflexion loop | Ciclo completo sin recursion | Integration test |
| Learning | Corrige reglas basadas en feedback | Simulation test |
| MCP tools | 3 herramientas registradas | `mcp__tools__list` |
| Integration | Feedback aparece en respuestas MCP | End-to-end test |

**Definition of Done:**
- [ ] Memoria episódica funcional con búsqueda por similitud
- [ ] Feedback generado para todos los tipos de violación
- [ ] Ciclo de reflexión ejecuta sin deadlock ni recursion
- [ ] Learning engine propone reglas con > 70% precision
- [ ] Respuestas MCP incluyen feedback cuando relevante

---

### Release Criteria: cognicode-axiom v1.0

| Área | Criterio |
|------|----------|
| Completitud | Todas las 32 tasks completadas |
| Tests | > 75% coverage en todo el crate |
| Performance | Ninguna operación > 100ms (excluyendo linters externos) |
| Docs | README.md, API docs via `cargo doc` |
| Integración | Compila con `cognicode-mcp` y `cognicode-core` |
| Claude hooks | Hooks configurables via JSON |
| Audit | Trail grabando en producción |

---

## 9. Riesgos y Mitigaciones

### Riesgos Técnicos

| ID | Riesgo | Probabilidad | Impacto | Mitigación |
|----|--------|--------------|---------|------------|
| R1 | **Cedar policy parsing complexity** — Cedar tiene sintaxis estricta y errores de parseo pueden ser crípticos | Media | Alta | Usar cedar-policy-cli para validación intermedia; desarrollar wrapper con errores más claros |
| R2 | **Performance de LCOM con grafos grandes** — Call graphs de proyectos grandes pueden causar timeout | Alta | Media | Implementar cache con invalidación incremental; procesar en background para > 1000 nodos |
| R3 | **Fragmentación de políticas** — Muchas reglas pequeñas dificultan debugging | Media | Media | Dashboard para visualizar todas las reglas; herramienta de `axiom doctor` para diagnóstico |
| R4 | **Cambios en cedar-policy API** — Breaking changes entre versiones | Baja | Alta | Pin dependency a versión específica;抽象 wrapper que aisle cambios de API |
| R5 | **SQLite como bottleneck** — Audit trail con alto throughput puede causar lock contention | Media | Media | Usar WAL mode; batching de writes; conexión por thread |

### Riesgos de Integración

| ID | Riesgo | Probabilidad | Impacto | Mitigación |
|----|--------|--------------|---------|------------|
| R6 | **Breaking changes en cognicode-core** — Cambios en CallGraph API rompen axiom | Media | Alta | Interface Adapter pattern; versioned API bindings; CI con latest core |
| R7 | **Conflicts en Claude Code hooks** — Hooks de axiom interfieren con hooks existentes del usuario | Baja | Media | Hooks opt-in; configuración que permite deshabilitar sin desinstalar |
| R8 | **Circular dependency** — axiom importa de core pero algunas features de core usan axiom | Media | Alta | Definir clear module boundaries; evitar `pub use` en crate root |

### Riesgos de Proyecto

| ID | Riesgo | Probabilidad | Impacto | Mitigación |
|----|--------|--------------|---------|------------|
| R9 | **Scope creep** — Presión para añadir más métricas/quality checks | Alta | Media | Phase gates estrictos; guardar ideas para v2.0 |
| R10 | **Conocimiento concentrado** — Solo 1 persona conoce Cedar + Rust | Media | Media | Documentar decisiones; pair programming; architecture diagrams |
| R11 | **Tiempos de linters externos** — Clippy/ESLint pueden ser lentos | Media | Baja | Async execution con timeout configurable; cache de resultados |

### Plan de Contingencia

| Escenario | Trigger | Respuesta |
|-----------|---------|-----------|
| Cedar API breaking change | Tests fallan tras update | Revertir a versión pinned; evaluar migración |
| Performance unacceptable | Benchmark > 100ms threshold | Profile con `cargo flamegraph`; оптимизировать hot path |
| Core API change | compilation error con latest core | Freezing core version; crear adapter layer |
| Claude hook conflict | Usuario reporta conflicto | Agregar flag `AXIOM_HOOKS_ENABLED=false` |

---

## Anexo: Glosario

| Término | Definición |
|---------|------------|
| **Axiom** |crate de gobernanza para CogniCode,得名 from "axiom" (truth taken as self-evident) |
| **Cedar Policy** | Lenguaje de políticas declarative de AWS para authorization |
| **Connascence** | Métrica de acoplamiento entre módulos (tipos: CoN, CoT, CoM, CoA, CoP, CoTm) |
| **LCOM** | Lack of Cohesion of Methods — cuánto los métodos de un clase no comparten atributos |
| **SOLID** | Principios de diseño: SRP, OCP, LSP, ISP, DIP |
| **ADR** | Architecture Decision Record — documento que captura una decisión arquitectónica importante |
| **Episodic Memory** | Memoria que almacena experiencias específicas del sistema |
| **Reflexion Loop** | Ciclo observar → analizar → decidir → actuar → evaluar → aprender |
| **Boundary** | Límite entre capas/modules en arquitectura DDD/hexagonal |

---

## Anexo: Referencias

### Documentación Externa

- [Cedar Policy Documentation](https://docs.cedarpolicy.com/)
- [cedar-policy crate](https://crates.io/crates/cedar-policy)
- [CogniCode Workspace](https://github.com/cognicode/workspace)
- [Claude Code Hooks](https://docs.anthropic.com/claude-code/hooks)

### RFCs Internas

- RFC-001: Arquitectura de governance (pendiente)
- RFC-002: Formato ADR (pendiente)
- RFC-003: Integración con Claude Code (pendiente)

---

*Documento creado: 2026-04-30*
*Última actualización: 2026-04-30*
*Autor: Equipo CogniCode*
