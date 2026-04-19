# CogniCode — IntelliJ-Inspired Features Plan

> **Fecha**: Abril 2026
> **Estado**: Plan de implementación
> **Alcance**: Features de IntelliJ IDEA replicables en CogniCode para consumo de agentes LLM
> **Fuente**: Investigación de arquitectura de indexing de IntelliJ + análisis de capacidades actuales de CogniCode
> **Prerrequisitos**: Mejoras P0-P6 del `IMPROVEMENT-PLAN-V2.md` deben estar completadas

---

## Resumen Ejecutivo

IntelliJ IDEA construye un índice semántico persistente basado en PSI (Program Structure Interface) que le ofrece features imposible de replicar con grep o LSP solo. Este documento identifica **15 features de IntelliJ que CogniCode puede replicar completamente** y que serían de alto valor para agentes LLM.

**Hallazgo clave**: CogniCode ya tiene 7 de los 8 tipos de `DependencyType` definidos (`Calls`, `Imports`, `Inherits`, `UsesGeneric`, `References`, `Defines`, `AnnotatedBy`, `Contains`) pero **solo popula `Calls`**. Activar los otros 7 tipos desbloquea la mayoría de features de este plan.

**Enfoque**: CogniCode no replica IntelliJ para humanos en IDE. Replica la **inteligencia subyacente** y la expone como API/MCP para **agentes LLM** — un consumidor que IntelliJ no tiene.

---

## Índice

1. [Fundamento: Full Reference Index (F3)](#f3--full-reference-index--base-de-todo)
2. [Tier 1 — Alto valor, building blocks existen](#tier-1--alto-valor-building-blocks-existen)
3. [Tier 2 — Alto valor, trabajo moderado](#tier-2--alto-valor-trabajo-moderado)
4. [Tier 3 — Valioso, más trabajo](#tier-3--valioso-más-trabajo)
5. [Roadmap de Implementación](#roadmap-de-implementación)
6. [Dependencias entre Features](#dependencias-entre-features)
7. [Métricas de Éxito](#métricas-de-éxito)

---

## F3 — Full Reference Index (Base de todo)

> **Prioridad**: CRÍTICA — Desbloquea F1, F2, F5, F6, F7, F8, F9, F10, F11, F13, F14
> **Esfuerzo**: 5-7 días
> **Estado actual**: `DependencyType` enum tiene 8 variantes, solo `Calls` se popula

### Contexto

IntelliJ mantiene un **reference index bidireccional**: `{symbol → [todos los sitios que lo referencian]}`. Esto incluye calls, imports, usages en strings, annotations, configs, etc. Es la base de Find Usages, Dead Code Detection, Rename Refactoring, y casi toda feature inteligente.

CogniCode hoy solo rastrea `Calls` (quién llama a quién). Los otros 7 tipos de relación están **definidos pero vacíos**:

```rust
pub enum DependencyType {
    Calls,          // ✅ POPULADO — function calls
    Imports,        // ❌ VACÍO — use/import statements
    Inherits,       // ❌ VACÍO — impl/extends/implements
    UsesGeneric,    // ❌ VACÍO — generic type parameters
    References,     // ❌ VACÍO — variable/type references (let x: User)
    Defines,        // ❌ VACÍO — module contains symbol
    AnnotatedBy,    // ❌ VACÍO — attributes/decorators
    Contains,       // ❌ VACÍO — parent-child (module→function, struct→field)
}
```

### Plan de implementación

#### F3.1 Extraer imports (`DependencyType::Imports`)

**Qué**: Durante parsing, detectar `use`, `import`, `require`, `from X import Y` y crear edges.

**Por lenguaje**:

| Lenguaje | Sintaxis | Edge |
|----------|----------|------|
| Rust | `use crate::auth::User;` | `current_file::User` → `auth::User` (Imports) |
| TypeScript | `import { User } from "./auth"` | `current_file::User` → `auth::User` (Imports) |
| Python | `from auth import User` | `current_file::User` → `auth.User` (Imports) |
| Go | `"myapp/auth"` | `current_file` → `auth` (Imports) |
| Java | `import com.example.auth.User;` | `current_file::User` → `com.example.auth.User` (Imports) |

**Firma**:
```rust
impl AnalysisService {
    /// Extract import statements from a parsed file and add Import edges.
    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &Path,
        graph: &mut CallGraph,
    ) -> Vec<(SymbolId, SymbolId)>;
}
```

**Criterio de aceptación**:
- `use crate::auth::User;` en `main.rs` genera edge `main_rs::User --Imports--> auth::User`
- `import { login } from "./auth"` en `app.ts` genera edge `app_ts::login --Imports--> auth::login`
- Los imports no resueltos (símbolo no encontrado en el grafo) se trackean como `UnresolvedImport` en el diagnostics

---

#### F3.2 Extraer references (`DependencyType::References`)

**Qué**: Detectar cuando un identificador referencia a un tipo, variable, o constant — no solo calls.

**Ejemplos**:

```rust
// Rust
let user: User = User::new();     // References User (type), Calls User::new
fn process(data: Vec<Order>) {}   // References Vec, References Order
const MAX: usize = 100;           // Defines MAX
if config.debug { ... }           // References config.debug (field access)
```

```typescript
// TypeScript
const user: User = new User();    // References User (type), Calls User constructor
function process(data: Order[]) { // References Order (type)
```

**Enfoque**: Para cada identificador en el código que NO es un call, buscar en `name_index`. Si existe un símbolo con ese nombre, crear edge `References`.

**Criterio de aceptación**:
- `let x: User` genera edge `current_fn --References--> User`
- Un struct usado como tipo en una firma genera edge `References`
- No genera edges para keywords del lenguaje (`if`, `let`, `fn`, etc.)

---

#### F3.3 Extraer inheritance (`DependencyType::Inherits`)

**Qué**: Detectar relaciones de herencia e implementación.

**Por lenguaje**:

| Lenguaje | Sintaxis | Edge |
|----------|----------|------|
| Rust | `impl Validator for User` | `User --Inherits--> Validator` |
| Rust | `struct Admin(User)` (newtype) | `Admin --Inherits--> User` |
| TypeScript | `class Admin extends User` | `Admin --Inherits--> User` |
| TypeScript | `interface Editable extends Printable` | `Editable --Inherits--> Printable` |
| Python | `class Admin(User):` | `Admin --Inherits--> User` |
| Java | `class Admin extends User implements IUser` | `Admin --Inherits--> User`, `Admin --Inherits--> IUser` |
| Go | `type Admin struct { User }` (embedding) | `Admin --Inherits--> User` |

**Criterio de aceptación**:
- `impl Trait for Struct` genera edge `Struct --Inherits--> Trait`
- `class Admin extends User` genera edge `Admin --Inherits--> User`
- Se puede consultar con `get_type_hierarchy()` (nueva API)

---

#### F3.4 Extraer contains/defines (`DependencyType::Contains` / `DependencyType::Defines`)

**Qué**: Relaciones padre-hijo entre símbolos.

**Ejemplos**:
```
module auth:
    ├── fn login()          → auth --Contains--> login
    ├── struct User:        → auth --Contains--> User
    │   ├── field name      → User --Contains--> name
    │   └── field email     → User --Contains--> email
    └── trait Validator:    → auth --Contains--> Validator
        └── fn validate()   → Validator --Contains--> validate
```

**Criterio de aceptación**:
- Un módulo contiene sus funciones, structs, traits
- Un struct contiene sus fields
- Un trait contiene sus métodos
- Se puede consultar "todos los símbolos hijos de X"

---

#### F3.5 Extraer annotations (`DependencyType::AnnotatedBy`)

**Qué**: Detectar attributes, decorators, annotations.

**Ejemplos**:

| Lenguaje | Sintaxis | Edge |
|----------|----------|------|
| Rust | `#[derive(Debug)]` | `Struct --AnnotatedBy--> Debug` |
| Rust | `#[test]` | `fn --AnnotatedBy--> test` |
| TypeScript | `@Injectable()` | `class --AnnotatedBy--> Injectable` |
| Java | `@Override` | `method --AnnotatedBy--> Override` |
| Python | `@dataclass` | `class --AnnotatedBy--> dataclass` |

**Criterio de aceptación**:
- `#[derive(Debug, Clone)]` genera 2 edges: `AnnotatedBy-->Debug`, `AnnotatedBy-->Clone`
- `#[test]` permite identificar funciones de test para F14

---

#### F3.6 Nuevas APIs habilitadas por Full Reference Index

```rust
impl WorkspaceSession {
    /// Find ALL references to a symbol (not just calls).
    /// Includes: calls, imports, type references, inheritance, annotations.
    pub fn find_all_references(&self, symbol: &str) -> WorkspaceResult<Vec<ReferenceEntry>> { ... }

    /// Get symbols grouped by dependency type.
    pub fn get_dependencies_by_type(
        &self,
        symbol: &str,
        dep_type: DependencyType,
    ) -> WorkspaceResult<Vec<SymbolDto>> { ... }

    /// Get type hierarchy (parents and children).
    pub fn get_type_hierarchy(&self, symbol: &str, depth: usize) -> WorkspaceResult<TypeHierarchy> { ... }

    /// Get symbols contained by a parent (module, struct, trait).
    pub fn get_contained_symbols(&self, parent: &str) -> WorkspaceResult<Vec<SymbolDto>> { ... }
}
```

---

## Tier 1 — Alto valor, building blocks existen

### F1. Dead Code Detection — Detectar código muerto

> **Prioridad**: Alta
> **Esfuerzo**: 1 día
> **Depende de**: Nada (usa reverse_edges existente) — sin F3 funciona para calls
> **Con F3**: Detecta no solo funciones no llamadas sino types no referenciados, imports no usados, etc.

#### Problema

No hay forma de saber qué funciones/types/constants son inalcanzables. Los agentes LLM no pueden sugerir cleanup sin esta información.

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadCodeEntry {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub kind: SymbolKind,
    pub reason: DeadCodeReason,
    pub confidence: f64,  // 0.0-1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeadCodeReason {
    /// No incoming edges (no callers, no references)
    NoIncomingEdges,
    /// Only referenced from test files
    OnlyReferencedByTests,
    /// Symbol is in an unreachable module
    UnreachableModule,
    /// Symbol is overridden but never called directly
    OnlyOverridden,
}

impl WorkspaceSession {
    /// Detect potentially dead code.
    pub fn detect_dead_code(&self) -> WorkspaceResult<Vec<DeadCodeEntry>> {
        // 1. Get all entry points (main, pub fns, trait fns, etc.)
        // 2. BFS from entry points → mark all reachable symbols
        // 3. Unmarked callable symbols = dead code candidates
        // 4. Filter out: test functions, trait definitions (might be used externally)
        // 5. Score confidence: pub fn with 0 callers = 0.5 (might be API)
        //                     private fn with 0 callers = 0.95 (almost certainly dead)
    }
}
```

#### Algoritmo

```
1. entry_points = get_entry_points()  // main, pub fns, http handlers, trait impls
2. reachable = BFS(entry_points, direction=outgoing)
3. all_callables = symbols.filter(|s| s.kind.is_callable() || s.kind.is_type_definition())
4. dead = all_callables - reachable
5. Para cada dead symbol:
   - Si es pub → confidence 0.5 (puede ser API externo)
   - Si es pub(crate) → confidence 0.7
   - Si es privado → confidence 0.95
   - Si solo se referencea desde tests → confidence 0.3 (puede ser test-only por diseño)
```

#### Criterio de aceptación

- `detect_dead_code()` retorna funciones con 0 callers que no son entry points
- Funciones `pub` tienen confidence más bajo que privadas
- No marca como muerto: `main()`, `#[test]` functions, trait definitions
- Con F3: detecta también structs/enums/constants no referenciados

#### Herramienta MCP

```
Tool: detect_dead_code
Input: {}
Output: Lista de DeadCodeEntry con symbol, file, reason, confidence
Ejemplo: "Found 12 potentially dead functions. 3 private functions have 0 callers with 95% confidence."
```

---

### F4. Module Dependency Graph — Dependencias entre módulos

> **Prioridad**: Alta
> **Esfuerzo**: 2 días
> **Depende de**: Nada (edges + file paths existen)
> **Con F3**: Más preciso (incluye imports/references cross-module)

#### Problema

No hay forma de ver la arquitectura de módulos de un proyecto: qué módulo depende de qué, acoplamiento, ciclos entre módulos.

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependency {
    pub module: String,           // "src/auth" o "crates/agent"
    pub depends_on: Vec<String>,  // módulos de los que depende
    pub depended_by: Vec<String>, // módulos que dependen de este
    pub coupling_score: usize,    // número de cross-module edges
    pub stability: f64,           // 0.0-1.0 (más incoming = más estable)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependencyGraph {
    pub modules: Vec<ModuleDependency>,
    pub cycles: Vec<Vec<String>>,  // ciclos entre módulos
    pub coupling_matrix: HashMap<(String, String), usize>,  // (from, to) → edge count
}

impl WorkspaceSession {
    /// Get module-level dependency graph.
    pub fn get_module_dependencies(&self) -> WorkspaceResult<ModuleDependencyGraph> { ... }
}
```

#### Algoritmo

```
1. Para cada edge (A → B) en el call graph:
   - module_A = directory_parent(A.file)
   - module_B = directory_parent(B.file)
   - Si module_A != module_B → cross-module edge
2. Agrupar edges por (module_A, module_B) → coupling_matrix
3. Para cada módulo:
   - depends_on = módulos a los que tiene edges salientes
   - depended_by = módulos de los que recibe edges entrantes
   - coupling_score = total cross-module edges
   - stability = depended_by.len() / (depends_on.len() + depended_by.len())
4. Detectar ciclos entre módulos (Tarjan SCC sobre module graph)
```

#### Criterio de aceptación

- `get_module_dependencies()` retorna un grafo donde nodos son directorios/módulos
- Los ciclos entre módulos se detectan (ej: auth → db → auth)
- `coupling_score` refleja cuántas llamadas cross-module hay
- La `coupling_matrix` permite ver la matriz completa

#### Herramienta MCP

```
Tool: get_module_dependencies
Input: {}
Output: ModuleDependencyGraph con modules, cycles, coupling_matrix
Ejemplo: "auth depends on db (12 calls), crypto (5 calls). Cycle detected: auth → db → auth."
```

---

### F5. API Surface Analysis — Superficie pública de un módulo

> **Prioridad**: Alta
> **Esfuerzo**: 2 días
> **Depende de**: Tree-sitter visibility parsing (añadir campo `visibility` a Symbol)
> **Con F3**: Incluye quién usa cada API entry

#### Problema

No hay forma de ver qué expone un módulo públicamente vs qué es interno. Los agentes no pueden sugerir "haz esto privado" sin esta información.

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Visibility {
    Public,          // pub
    PublicCrate,     // pub(crate)
    PublicSuper,     // pub(super)
    PublicModule(String), // pub(in path)
    Private,         // sin pub
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEntry {
    pub symbol: String,
    pub kind: SymbolKind,
    pub visibility: Visibility,
    pub signature: String,
    pub external_callers: usize,  // callers desde fuera del módulo
    pub internal_callers: usize,  // callers desde dentro del módulo
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSurface {
    pub module: String,
    pub public_symbols: Vec<ApiEntry>,
    pub private_symbols: Vec<ApiEntry>,
    pub total_symbols: usize,
    pub public_ratio: f64,  // pub / total
}

impl WorkspaceSession {
    /// Get the API surface of a module or directory.
    pub fn get_api_surface(&self, module_path: &str) -> WorkspaceResult<ApiSurface> { ... }
}
```

#### Criterio de aceptación

- `get_api_surface("src/auth/")` retorna todas las functions/structs en auth con su visibility
- `external_callers` cuenta callers desde fuera de `src/auth/`
- Símbolos `pub` con 0 external_callers son candidatos a `pub(crate)` o privado
- `public_ratio` > 0.7 sugiere API surface demasiado grande

#### Herramienta MCP

```
Tool: get_api_surface
Input: { module_path: "src/auth/" }
Output: ApiSurface con public/private symbols y sus callers
Ejemplo: "auth exposes 23 public symbols. 8 have 0 external callers (candidates for reduced visibility)."
```

---

### F6. Layered Architecture Enforcement — Reglas de capas

> **Prioridad**: Alta
> **Esfuerzo**: 3 días
> **Depende de**: F4 (module dependencies)
> **Con F3**: Más preciso (detecta imports/references entre capas, no solo calls)

#### Problema

`check_architecture()` detecta ciclos pero no verifica reglas de capas (ej: controllers no deben llamar a database directamente).

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureLayer {
    pub name: String,
    pub path_patterns: Vec<String>,  // glob patterns
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureRule {
    pub from_layer: String,
    pub to_layer: String,
    pub allowed: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerViolation {
    pub from_layer: String,
    pub to_layer: String,
    pub from_symbol: String,
    pub to_symbol: String,
    pub file: String,
    pub line: u32,
    pub rule: ArchitectureRule,
}

impl WorkspaceSession {
    /// Check architecture against defined layer rules.
    pub fn check_layered_architecture(
        &self,
        layers: Vec<ArchitectureLayer>,
        rules: Vec<ArchitectureRule>,
    ) -> WorkspaceResult<Vec<LayerViolation>> { ... }
}
```

#### Ejemplo de configuración

```rust
let layers = vec![
    ArchitectureLayer { name: "controllers".into(), path_patterns: vec!["src/controllers/**".into()] },
    ArchitectureLayer { name: "services".into(),    path_patterns: vec!["src/services/**".into()] },
    ArchitectureLayer { name: "domain".into(),      path_patterns: vec!["src/domain/**".into()] },
    ArchitectureLayer { name: "infra".into(),        path_patterns: vec!["src/infrastructure/**".into()] },
];

let rules = vec![
    ArchitectureRule { from_layer: "controllers".into(), to_layer: "services".into(), allowed: true, description: Some("Controllers can call services".into()) },
    ArchitectureRule { from_layer: "controllers".into(), to_layer: "domain".into(), allowed: false, description: Some("Controllers must not bypass services".into()) },
    ArchitectureRule { from_layer: "controllers".into(), to_layer: "infra".into(), allowed: false, description: Some("Controllers must not access infra directly".into()) },
    ArchitectureRule { from_layer: "services".into(), to_layer: "domain".into(), allowed: true, description: Some("Services can access domain".into()) },
    ArchitectureRule { from_layer: "services".into(), to_layer: "infra".into(), allowed: true, description: Some("Services can access infra".into()) },
    ArchitectureRule { from_layer: "domain".into(), to_layer: "infra".into(), allowed: false, description: Some("Domain must not depend on infra".into()) },
];
```

#### Criterio de aceptación

- Si un controller tiene edge hacia infra → LayerViolation
- Si domain tiene edge hacia infra → LayerViolation
- Las violaciones muestran símbolo origen, destino, archivo, línea
- Se puede persistir la configuración de capas (`.cognicode/layers.toml`)

#### Herramienta MCP

```
Tool: check_layered_architecture
Input: { config_path: ".cognicode/layers.toml" }  o  { layers: [...], rules: [...] }
Output: Lista de LayerViolation
Ejemplo: "3 violations found. UserController::get_user() calls DatabasePool::query() directly (controller → infra not allowed)."
```

---

### F9. Dependency Direction Matrix — Matriz de acoplamiento

> **Prioridad**: Alta
> **Esfuerzo**: 2 días
> **Depende de**: F4 (module dependencies)
> **Con F3**: Incluye imports/references en la matriz

#### Problema

No hay forma de ver un "mapa de calor" de acoplamiento entre módulos.

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingMatrix {
    pub modules: Vec<String>,
    pub matrix: Vec<Vec<usize>>,         // matrix[from][to] = número de edges
    pub total_coupling: usize,           // suma total
    pub most_coupled: Vec<CouplingPair>, // top 10 pares más acoplados
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingPair {
    pub from: String,
    pub to: String,
    pub edge_count: usize,
    pub direction: CouplingDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CouplingDirection {
    OneWay,     // A → B solo
    Bidirectional, // A → B y B → A (ciclo)
}

impl WorkspaceSession {
    /// Get coupling matrix between all modules.
    pub fn get_coupling_matrix(&self) -> WorkspaceResult<CouplingMatrix> { ... }
}
```

#### Output visual

```
         auth  db  api  utils  models
auth      -    12   5    3      8
db        0    -    0    8      15
api       15   7    -    2      4
utils     0    0    0    -      0
models    0    0    0    0      -       ← models no depende de nadie (bien)
```

#### Criterio de aceptación

- `get_coupling_matrix()` retorna una matriz N×N donde N = número de módulos
- Celdas con 0 = sin acoplamiento directo
- `most_coupled` lista los pares con más edges
- Bidirectional = ciclo entre módulos

#### Herramienta MCP

```
Tool: get_coupling_matrix
Input: {}
Output: CouplingMatrix con matriz y most_coupled pairs
Ejemplo: "Most coupled: auth↔api (20 edges), db→models (15 edges). 2 bidirectional cycles detected."
```

---

### F10. God Object Detection — Detectar "objetos Dios"

> **Prioridad**: Alta
> **Esfuerzo**: 2 días
> **Depende de**: F3.4 (Contains edges) para relacionar símbolos con su parent
> **Sin F3**: Heurística por file path (símbolos en el mismo archivo)

#### Problema

No hay forma de detectar módulos/structs que tienen demasiadas responsabilidades.

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodObject {
    pub symbol: String,
    pub kind: SymbolKind,
    pub file: String,
    pub contained_symbols: usize,     // cuántos hijos tiene
    pub outgoing_dependencies: usize,  // a cuántos símbolos externos llama
    pub incoming_dependencies: usize,  // cuántos símbolos lo llaman
    pub total_complexity: u32,        // suma de complejidad ciclomática de hijos
    pub score: f64,                   // 0.0-1.0 god-ness
    pub suggested_split: Vec<String>, // sugerencia de división por "clúster" de dependencias
}

impl WorkspaceSession {
    /// Detect god objects (modules/structs with too many responsibilities).
    pub fn detect_god_objects(&self, threshold: usize) -> WorkspaceResult<Vec<GodObject>> { ... }
}
```

#### Algoritmo

```
god_score = (
    contained_symbols * 0.3 +
    outgoing_dependencies * 0.3 +
    total_complexity * 0.2 +
    incoming_dependencies * 0.2
) / normalize_factor

Si god_score > threshold → God Object

suggested_split:
  1. Agrupar hijos del god object por sus dependencias compartidas
  2. Clúster por dependencias comunes (k-means sobre edge overlap)
  3. Cada clúster → sugerencia de submódulo
```

#### Criterio de aceptación

- Detecta módulos con >20 símbolos y >50 dependencias
- `score` refleja cuán "dios" es el objeto
- `suggested_split` agrupa funciones por afinidad de dependencias

#### Herramienta MCP

```
Tool: detect_god_objects
Input: { threshold: 50 }
Output: Lista de GodObject con score y suggested_split
Ejemplo: "user_service has 23 functions, 89 outgoing deps, complexity 187. Suggest splitting into: UserService, UserAuthService, UserProfileService."
```

---

### F11. Change Ripple Analysis — Análisis de impacto en cadena

> **Prioridad**: Alta
> **Esfuerzo**: 2 días
> **Depende de**: Nada (extiende analyze_impact existente)
> **Con F3**: Incluye impacto en imports, type references, inheritance

#### Problema

`analyze_impact()` muestra solo 1 nivel de impacto. Cambios en código se propagan en cadena — los agentes necesitan ver el ripple completo.

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRipple {
    pub source: String,
    pub levels: Vec<RippleLevel>,
    pub total_affected_files: usize,
    pub total_affected_symbols: usize,
    pub risk: RiskLevel,
    pub suggested_tests: Vec<String>,  // archivos de test afectados
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RippleLevel {
    pub depth: usize,
    pub affected_symbols: Vec<RippleEntry>,
    pub risk_contribution: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RippleEntry {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub kind: SymbolKind,
    pub dependency_type: DependencyType,  // Calls, References, Inherits, etc.
    pub has_tests: bool,  // si el archivo afectado tiene tests
}

impl WorkspaceSession {
    /// Analyze full change ripple from a symbol.
    pub fn analyze_change_ripple(
        &self,
        symbol: &str,
        max_depth: usize,
    ) -> WorkspaceResult<ChangeRipple> { ... }
}
```

#### Algoritmo

```
1. BFS desde symbol, direction=incoming, max_depth=N
2. Para cada nivel:
   - Recopilar símbolos afectados
   - Clasificar por dependency_type (Calls > References > Imports en riesgo)
   - Verificar si el archivo tiene tests (heurística: test file en el mismo directorio)
3. Calcular risk:
   - LOW: <5 afectados, todos con tests
   - MEDIUM: 5-20 afectados, >50% con tests
   - HIGH: >20 afectados o <50% con tests
4. suggested_tests: archivos de test que cubren símbolos afectados
```

#### Criterio de aceptación

- `analyze_change_ripple("User::validate", 3)` muestra 3 niveles de callers
- Cada nivel indica dependency_type (call, reference, import)
- `has_tests` marca si el archivo afectado tiene test file asociado
- `suggested_tests` lista los archivos de test que deberían ejecutarse

#### Herramienta MCP

```
Tool: analyze_change_ripple
Input: { symbol: "User::validate", max_depth: 3 }
Output: ChangeRipple con niveles, riesgo, tests sugeridos
Ejemplo: "Level 1: 3 direct callers. Level 2: 12 indirect. Level 3: 28 affected. Total: 43 files. Risk: HIGH. 12/43 have tests. Suggested test files: [user_test.rs, auth_test.rs, ...]"
```

---

## Tier 2 — Alto valor, trabajo moderado

### F2. Unused Imports Detection — Detectar imports no usados

> **Prioridad**: Alta
> **Esfuerzo**: 2 días
> **Depende de**: F3.1 (Import edges)

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedImport {
    pub import_statement: String,    // "use crate::auth::User;"
    pub file: String,
    pub line: u32,
    pub imported_symbol: String,     // "User"
    pub is_used: bool,
    pub usage_count: usize,          // veces que se referencia en el archivo
}

impl WorkspaceSession {
    /// Detect unused imports in a file or project.
    pub fn detect_unused_imports(&self, file_path: Option<&str>) -> WorkspaceResult<Vec<UnusedImport>> { ... }
}
```

#### Algoritmo

```
1. Para cada import edge (A imports B):
   - Contar References edges desde A hacia B en el mismo archivo
   - Si count == 0 → UnusedImport
2. Categorizar:
   - Totalmente sin uso (0 referencias)
   - Parcialmente usado (import de múltiples símbolos, algunos no usados)
```

#### Criterio de aceptación

- `detect_unused_imports(Some("src/main.rs"))` retorna imports no usados en main.rs
- `detect_unused_imports(None)` escanea todo el proyecto
- Import de un símbolo que se usa en type annotation no se marca como no usado

#### Herramienta MCP

```
Tool: detect_unused_imports
Input: { file_path: "src/main.rs" }  o  {}
Output: Lista de UnusedImport
Ejemplo: "12 unused imports found in 8 files. Run with auto_fix=true to remove them."
```

---

### F7. Type Hierarchy (estructural) — Jerarquía de tipos

> **Prioridad**: Media-alta
> **Esfuerzo**: 4 días
> **Depende de**: F3.3 (Inherits edges)

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeHierarchyNode {
    pub symbol: String,
    pub kind: SymbolKind,
    pub file: String,
    pub parents: Vec<String>,    // lo que implementa/extiende
    pub children: Vec<String>,   // lo que lo implementan/extienden
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeHierarchy {
    pub root: String,
    pub ancestors: Vec<TypeHierarchyNode>,   // hacia arriba (parents)
    pub descendants: Vec<TypeHierarchyNode>, // hacia abajo (children)
    pub total_implementors: usize,
}

impl WorkspaceSession {
    /// Get type hierarchy for a symbol (struct, trait, class, interface).
    pub fn get_type_hierarchy(&self, symbol: &str, depth: usize) -> WorkspaceResult<TypeHierarchy> { ... }
}
```

#### Criterio de aceptación

- `get_type_hierarchy("Validator", 3)` muestra qué traits hereda y qué structs lo implementan
- Funciona para Rust (impl/struct), TypeScript (extends/implements), Python (inheritance), Java (extends/implements)
- Sin type resolution: detecta la relación estructural, no la resuelve al tipo concreto

#### Herramienta MCP

```
Tool: get_type_hierarchy
Input: { symbol: "Validator", depth: 3 }
Output: TypeHierarchy con ancestors y descendants
Ejemplo: "Validator is implemented by 7 structs: User, Order, Product, ... Parent traits: Validate + Serialize."
```

---

### F8. Duplicate Code Detection — Detectar código duplicado

> **Prioridad**: Media
> **Esfuerzo**: 5 días
> **Depende de**: Tree-sitter function bodies disponibles

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub hash: String,              // hash del AST normalizado
    pub occurrences: Vec<DuplicateOccurrence>,
    pub similarity: f64,           // 0.0-1.0
    pub lines_of_code: usize,
    pub suggested_action: String,  // "Extract shared function"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateOccurrence {
    pub function: String,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

impl WorkspaceSession {
    /// Find duplicate code blocks across the project.
    pub fn find_duplicates(
        &self,
        min_lines: usize,
        similarity_threshold: f64,
    ) -> WorkspaceResult<Vec<DuplicateGroup>> { ... }
}
```

#### Algoritmo

```
1. Para cada función en el grafo:
   a. Extraer body (Tree-sitter ya lo tiene)
   b. Normalizar AST:
      - Reemplazar nombres de variables por VAR_0, VAR_1...
      - Reemplazar literals por LIT
      - Normalizar whitespace y comments
   c. Hash del AST normalizado (blake3)
2. Agrupar por hash → duplicates exactos (similarity 1.0)
3. Para pares con hashes diferentes:
   a. AST distance (Zhang-Shasha o simple tree edit distance)
   b. similarity = 1.0 - (distance / max_nodes)
4. Filtrar por similarity_threshold y min_lines
```

#### Criterio de aceptación

- Dos funciones idénticas excepto por nombres de variables se detectan (similarity ~0.9)
- `min_lines=5` ignora funciones de 1-4 líneas
- `similarity_threshold=0.8` solo reporta >80% similar

#### Herramienta MCP

```
Tool: find_duplicates
Input: { min_lines: 5, similarity_threshold: 0.8 }
Output: Lista de DuplicateGroup
Ejemplo: "3 groups of duplicates found. Group 1: process_order, process_invoice, process_return (92% similar, 15 LOC each). Suggested: Extract shared function."
```

---

## Tier 3 — Valioso, más trabajo

### F12. Inverted Word Index — Búsqueda instantánea de cualquier identificador

> **Prioridad**: Media
> **Esfuerzo**: 3 días
> **Depende de**: Nada (se construye durante parsing)

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordOccurrence {
    pub word: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub context: WordContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WordContext {
    Code,        // en código
    String,      // en string literal
    Comment,     // en comentario
    Import,      // en import statement
    DocComment,  // en doc comment
}

impl WorkspaceSession {
    /// Search for all occurrences of a word across the project.
    pub fn search_word(&self, word: &str, context: Option<WordContext>) -> WorkspaceResult<Vec<WordOccurrence>> { ... }
}
```

#### Criterio de aceptación

- `search_word("UserService")` retorna todas las ocurrencias (código, strings, comments, imports)
- Filtrable por contexto (solo en código, solo en comments, etc.)
- O(1) lookup vía inverted index (HashMap<String, Vec<Occurrence>>)

---

### F13. Smart Change Suggestions — Sugerencias proactivas

> **Prioridad**: Alta
> **Esfuerzo**: 3 días (pero requiere F1, F4, F5, F6, F10 implementados)
> **Depende de**: F1, F4, F5, F6, F10, F11

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementSuggestion {
    pub category: SuggestionCategory,
    pub message: String,
    pub affected_files: Vec<String>,
    pub severity: SuggestionSeverity,
    pub auto_fixable: bool,
    pub details: serde_json::Value,  // category-specific data
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionCategory {
    DeadCode,
    UnusedImport,
    HighComplexity,
    GodObject,
    CircularDependency,
    LayerViolation,
    TightCoupling,
    MissingTests,
    ApiSurfaceTooLarge,
    DuplicateCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionSeverity {
    Info,
    Warning,
    Critical,
}

impl WorkspaceSession {
    /// Get smart improvement suggestions for the project.
    pub fn suggest_improvements(&self) -> WorkspaceResult<Vec<ImprovementSuggestion>> { ... }
}
```

#### Integración con proactive injection

Las sugerencias se incluyen en `<code-intelligence>` XML:

```xml
<code-intelligence>
  ...
  <suggestions>
    <suggestion severity="warning" category="dead_code" auto_fixable="true">
      12 potentially dead functions found. 3 private functions have 0 callers.
    </suggestion>
    <suggestion severity="critical" category="circular_dependency">
      Cycle detected: auth → db → auth. Consider introducing an interface.
    </suggestion>
  </suggestions>
</code-intelligence>
```

#### Criterio de aceptación

- `suggest_improvements()` combina resultados de F1, F4, F5, F6, F10, F11
- Las sugerencias se ordenan por severity (Critical > Warning > Info)
- `auto_fixable` indica si el agente puede arreglarlo automáticamente (unused imports = yes, god object = no)

---

### F14. Test Coverage Gap Analysis — Gaps de cobertura

> **Prioridad**: Alta
> **Esfuerzo**: 2 días
> **Depende de**: F3.5 (annotations para detectar `#[test]`), F1 (dead code incluye test functions)

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCoverageGap {
    pub symbol: String,
    pub kind: SymbolKind,
    pub file: String,
    pub is_hot_path: bool,      // fan-in alto
    pub complexity: u32,        // complejidad ciclomática
    pub has_tests: bool,
    pub test_files: Vec<String>, // archivos de test que lo cubren (si tiene)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCoverageReport {
    pub total_symbols: usize,
    pub symbols_with_tests: usize,
    pub symbols_without_tests: usize,
    pub coverage_percent: f64,
    pub hot_paths_without_tests: Vec<TestCoverageGap>,
    pub high_complexity_without_tests: Vec<TestCoverageGap>,
}

impl WorkspaceSession {
    /// Analyze test coverage gaps.
    pub fn analyze_test_coverage(&self) -> WorkspaceResult<TestCoverageReport> { ... }
}
```

#### Algoritmo

```
1. Detectar funciones de test:
   - Rust: #[test], #[tokio::test]
   - TypeScript: describe(), test(), it()
   - Python: def test_*, @pytest.mark.*
   - Go: func Test*(t *testing.T)
   - Java: @Test

2. Heurística de "qué testea cada test":
   - Nombre: test_user_validate → cubre User::validate
   - Imports: test file importa auth::User → cubre símbolos de auth
   - Calls: test llama a validate() → cubre validate

3. Comparar:
   - Hot paths (fan-in > 5) sin tests → CRITICAL gap
   - Funciones con complexity > 10 sin tests → WARNING
   - Funciones con 0 tests → INFO
```

#### Criterio de aceptación

- `analyze_test_coverage()` identifica funciones de test heurísticamente
- Los hot paths sin tests se priorizan
- Funciones con alta complejidad sin tests se marcan como WARNING

---

### F15. Shared Indexes — Índices distribuibles

> **Prioridad**: Baja (solo para equipos/CI)
> **Esfuerzo**: 3 días
> **Depende de**: P2 (persistencia bincode + redb)

#### Firma propuesta

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedIndexManifest {
    pub project_hash: String,    // blake3 del project path
    pub cognicode_version: String,
    pub created_at: String,      // RFC 3339
    pub symbol_count: usize,
    pub edge_count: usize,
    pub file_count: usize,
    pub languages: Vec<String>,
}

impl WorkspaceSession {
    /// Export current index as a shareable archive.
    pub fn export_shared_index(&self, output_path: &Path) -> WorkspaceResult<SharedIndexManifest> { ... }

    /// Import a shared index from an archive.
    pub fn import_shared_index(&self, archive_path: &Path) -> WorkspaceResult<SharedIndexManifest> { ... }
}
```

#### Flujo

```
CI/Build Server:
  1. cargo build → compila
  2. cognicode index → construye índice
  3. cognicode export --output shared-index.tar.zst
  4. Upload to S3/artifactory

Dev Machine:
  1. git pull
  2. cognicode import --from s3://bucket/shared-index.tar.zst
  3. Ready in ~500ms instead of 3-15s
  4. Incremental update para archivos cambiados
```

#### Criterio de aceptación

- `export_shared_index()` genera un archivo portable (tar.zst)
- `import_shared_index()` carga el índice en <500ms
- Compatible cross-OS (same CogniCode version)
- Incremental update después de import (si archivos cambiaron desde el export)

---

## Roadmap de Implementación

### Fase 1: Foundation (F3 — Full Reference Index)

```
Semana 1-2:
├── Día 1-2: F3.1 Imports edges + F3.4 Contains/Defines edges
├── Día 3-4: F3.2 References edges
├── Día 5-6: F3.3 Inherits edges + F3.5 Annotations edges
└── Día 7: F3.6 Nuevas APIs (find_all_references, get_type_hierarchy, get_contained_symbols)
```

**Gate**: Los 8 DependencyTypes se populan. `find_all_references("User")` retorna calls + imports + type refs + inheritance.

### Fase 2: Quick Wins (F1, F4, F5, F9, F11)

```
Semana 3:
├── Día 1: F1 Dead Code Detection
├── Día 2-3: F4 Module Dependencies + F9 Coupling Matrix
├── Día 4: F5 API Surface Analysis
└── Día 5: F11 Change Ripple Analysis
```

**Gate**: 5 nuevas tools MCP. El agente puede detectar código muerto, ver acoplamiento entre módulos, y analizar ripple de cambios.

### Fase 3: Architecture & Quality (F6, F10, F2)

```
Semana 4:
├── Día 1-2: F6 Layered Architecture Enforcement
├── Día 3: F10 God Object Detection
└── Día 4-5: F2 Unused Imports Detection
```

**Gate**: 3 nuevas tools. El agente puede verificar capas de arquitectura, detectar objetos dios, y limpiar imports.

### Fase 4: Advanced (F7, F8, F12, F13, F14)

```
Semana 5-6:
├── Día 1-3: F7 Type Hierarchy
├── Día 4-6: F8 Duplicate Code Detection
├── Día 7-8: F12 Word Index
├── Día 9: F13 Smart Suggestions (combina F1-F11)
└── Día 10: F14 Test Coverage Gaps
```

**Gate**: 5 nuevas tools. El agente tiene sugerencias proactivas completas.

### Fase 5: Distribution (F15)

```
Semana 7:
└── Día 1-3: F15 Shared Indexes (export/import)
```

**Gate**: Índice exportable/importable en <500ms.

---

## Dependencias entre Features

```
F3 (Full Reference Index)
├── F1 (Dead Code) — usa reverse_edges mejorado
├── F2 (Unused Imports) — usa Import edges
├── F4 (Module Deps) — mejorado con Import/Reference edges
├── F5 (API Surface) — usa Reference edges para external_callers
├── F6 (Layered Arch) — depende de F4
├── F7 (Type Hierarchy) — usa Inherits edges
├── F8 (Duplicates) — independiente
├── F9 (Coupling Matrix) — depende de F4
├── F10 (God Objects) — usa Contains edges
├── F11 (Change Ripple) — mejorado con todos los edge types
├── F12 (Word Index) — independiente
├── F13 (Smart Suggestions) — depende de F1, F4, F5, F6, F10, F11
└── F14 (Test Gaps) — usa Annotation edges (#[test])
```

---

## Métricas de Éxito

### Cuantitativas

| Métrica | Antes | Después (F1-F15) |
|---------|-------|-------------------|
| DependencyTypes populados | 1 de 8 (Calls) | **8 de 8** |
| Herramientas MCP | 19 | **30+** (19 + 11 nuevas) |
| Líneas de código muerto detectables | 0 | **Todo callable sin callers** |
| Nivel de análisis de impacto | 1 (direct callers) | **N niveles (ripple completo)** |
| Detección de violaciones arquitecturales | Solo ciclos | **Ciclos + capas + acoplamiento** |

### Cualitativas

- Un agente LLM puede responder "¿Qué pasa si cambio X?" con análisis completo de ripple
- Un agente puede sugerir "Limpia estos 12 imports no usados" sin análisis manual
- Un agente puede detectar "Esta función es un hot path sin tests — riesgo ALTO"
- El `<code-intelligence>` XML incluye sugerencias proactivas sin que el agente pida nada
- Múltiples agentes pueden compartir el mismo índice persistente

---

## Relación con IMPROVEMENT-PLAN-V2

Este documento **no reemplaza** `IMPROVEMENT-PLAN-V2.md`. Se complementa:

| Plan | Qué cubre | Cuándo |
|------|-----------|--------|
| `IMPROVEMENT-PLAN-V2.md` | Infraestructura: API, rendimiento, persistencia, features base, absorción RCode | Primero |
| **Este documento** | Features IntelliJ-inspired: dead code, coupling, architecture, duplicates, etc. | Después (necesita P0-P6) |

**Secuencia recomendada**:
1. Ejecutar `IMPROVEMENT-PLAN-V2.md` Fases A-C (P0-P5 + P6)
2. Ejecutar este documento Fase 1 (F3 — Full Reference Index)
3. Ejecutar este documento Fases 2-5 (F1, F2, F4-F15)

---

## Apéndice: Comparación con IntelliJ

| Feature | IntelliJ | CogniCode (este plan) | Nota |
|---------|----------|----------------------|------|
| Full Reference Index | ✅ (PSI-based) | ✅ F3 (Tree-sitter based) | Sin type resolution |
| Dead Code | ✅ | ✅ F1 | Comparable |
| Unused Imports | ✅ | ✅ F2 | Comparable |
| Module Dependencies | ⚠️ (no expuesto) | ✅ F4 | **CogniCode expone como API** |
| API Surface | ⚠️ (implícito) | ✅ F5 | **CogniCode lo hace explícito** |
| Layered Architecture | ⚠️ (plugins) | ✅ F6 | Comparable con ArchUnit |
| Coupling Matrix | ⚠️ (DSM plugin) | ✅ F9 | **CogniCode nativo** |
| God Objects | ⚠️ (inspections) | ✅ F10 | Comparable |
| Change Ripple | ❌ (solo preview manual) | ✅ F11 | **CogniCode exclusivo** |
| Type Hierarchy | ✅ (completo) | ⚠️ F7 (estructural) | Sin generics |
| Duplicate Code | ✅ | ✅ F8 | Comparable |
| Word Index | ✅ | ✅ F12 | Comparable |
| Smart Suggestions | ⚠️ (inspections dispersas) | ✅ F13 | **CogniCode unificado + LLM** |
| Test Gaps | ⚠️ (coverage plugin) | ✅ F14 | Heurístico |
| Shared Indexes | ✅ (JDK + project) | ✅ F15 | Más simple pero funcional |
| **Hot Paths** | ❌ | ✅ (ya existe) | **CogniCode exclusivo** |
| **Impact Analysis** | ❌ | ✅ (ya existe) | **CogniCode exclusivo** |
| **Architecture Score** | ❌ | ✅ (ya existe) | **CogniCode exclusivo** |
| **Proactive XML Injection** | N/A | ✅ (ya existe) | **CogniCode exclusivo** |
