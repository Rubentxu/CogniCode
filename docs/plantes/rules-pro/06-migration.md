# Plan de Migración — De Regex a Rules-as-Code

> **Fecha**: 11 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Roadmap de implementación

---

## 1. Estrategia General

### 1.1 Migración Incremental, No Big-Bang

La migración del sistema actual de regex a Rules-as-Code se hará de manera **incremental**, siguiendo estos principios:

| Principio | Descripción |
|-----------|-------------|
| **No romper existente** | El sistema actual sigue funcionando mientras migramos |
| **Validación continua** | Tests siempre pasando antes de cada fase |
| **Reversibilidad** | Cada fase puede revertirse si hay problemas |
| **Medición** | Benchmarks en cada paso para detectar regresiones |

### 1.2 Fases de Migración

```
┌──────────────────────────────────────────────────────────────────┐
│                    ROADMAP DE MIGRACIÓN                          │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ FASE 0: ESTABILIZACIÓN (Semanas 1-2)                      │ │
│  │ Fix bugs existentes,tests passing                          │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ FASE 1: INFRAESTRUCTURA (Semanas 3-6)                     │ │
│  │ Proc-macro, Rule trait, PreflightFilter, LCPG base         │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ FASE 2: LCPG COMPLETO (Semanas 7-10)                       │ │
│  │ Visitor trait, SymbolTable builder, integración              │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ FASE 3: MIGRACIÓN MASIVA (Semanas 11-18)                   │ │
│  │ Migrar reglas por categoría                                 │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ FASE 4: DATAFLOW (Semanas 19-24)                          │ │
│  │ Taint tracking para security rules                          │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## 2. Fase 0: Estabilización

> **Estado**: Ya hecha parcialmente  
> **Objetivo**: 294/294 tests pasando

### 2.1 Bugs Ya Corregidos

| Bug | Descripción | Estado |
|-----|-------------|--------|
| **E0433 S2259Rule** | Error de compilación | ✅ Corregido |
| **S1135 regex lookahead** | Pattern `(?=...)` no funcional | ✅ Corregido |
| **S1134 malformed regex** | Regex compilation error | ✅ Corregido |
| **S2068 min length** | Validación de longitud | ✅ Corregido |

### 2.2 Bugs Pendientes

| Bug | Descripción | Tests Afectados |
|-----|-------------|-----------------|
| **S4792 DES/RC4** | Regex lookbehind no soportado | 3 tests |
| **S5122 SQL Injection** | Regex lookbehind no soportado | 3 tests |

### 2.3 Acciones de Estabilización

```bash
# 1. Ver estado actual de tests
cd /home/rubentxu/Proyectos/rust/CogniCode
cargo test --lib 2>&1 | tail -20

# 2. Los 19 tests fallando
# - S4792: 3 tests (DES/RC4)
# - S5122: 3 tests (SQL)
# - S1135: 2 tests
# - S1134: 2 tests
# - Others: 9 tests

# 3. Fix temporal: marcar tests conocidos como ignored
# 4. Fix definitivo: migrar a ast-grep patterns en Fase 1
```

---

## 3. Fase 1: Infraestructura

> **Duración estimada**: Semanas 3-6  
> **Objetivo**: Cimientos del nuevo sistema

### 3.1 Crear Crate `cognicode-macros`

```bash
# Estructura del workspace
cognicode/
├── cognicode-core/          # Motor existente
├── cognicode-macros/        # NUEVO: Proc-macros
│   ├── src/
│   │   ├── lib.rs
│   │   └── cogni_rule.rs    # Macro #[cogni_rule]
│   ├── tests/
│   └── Cargo.toml
└── ...
```

```toml
# cognicode-macros/Cargo.toml
[package]
name = "cognicode-macros"
version = "0.1.0"
edition = "2021"

[dependencies]
proc-macro2 = "1.0"
quote = "1.0"
syn = "2.0"
tree-sitter = "0.20"

[lib]
proc-macro = true
```

### 3.2 Implementar Proc-Macro `#[cogni_rule]`

```rust
// cognicode-macros/src/cogni_rule.rs

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_attribute]
pub fn cogni_rule(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as RuleArgs);
    let item = parse_macro_input!(item as DeriveInput);

    // 1. Validar pattern contra tree-sitter
    validate_pattern(&args.pattern);

    // 2. Validar kind contra dictionary
    validate_kind(&args.kind);

    // 3. Generar impl del trait Rule
    let generated = generate_rule_impl(&args, &item);

    generated.into()
}
```

### 3.3 Definir Trait `Rule` Evolucionado

```rust
// En cognicode-core/src/rule/trait.rs

pub trait Rule: Send + Sync + 'static {
    fn id(&self) -> &str;
    fn severity(&self) -> Severity;
    fn category(&self) -> Category;
    fn check(&self, ctx: &RuleContext) -> Vec<Issue>;

    /// Keywords requeridas para pre-flight (Layer 0)
    fn required_keywords(&self) -> &[&str] { &[] }

    /// Capa de análisis (0=preflight, 1=structural, 2=semantic, 3=flow)
    fn layer(&self) -> u8 { 1 }

    /// Lenguajes soportados
    fn languages(&self) -> &[Language] { &[Language::Rust] }
}
```

### 3.4 Crear `PreflightFilter`

```rust
// En cognicode-core/src/preflight/mod.rs

pub struct PreflightFilter {
    automaton: AhoCorasick,
    keyword_to_rules: Vec<Vec<RuleId>>,
}

impl PreflightFilter {
    pub fn new(rules: &[Box<dyn Rule>]) -> Self { ... }
    pub fn filter_rules(&self, source: &str, all_rules: &[Box<dyn Rule>]) -> Vec<Box<dyn Rule>> { ... }
}
```

### 3.5 Integrar `ast-grep-core`

```toml
# En cognicode-core/Cargo.toml
[dependencies]
ast-grep-core = { git = "https://github.com/ast-grep/ast-grep", package = "ast-grep-core" }
```

### 3.6 Proof of Concept: 10 Reglas Migradas

Seleccionar 10 reglas para migrar como PoC:

| ID Actual | Nombre | Prioridad |
|-----------|--------|-----------|
| S1135 | TODO comments | Alta (fácil) |
| S1656 | Variable shadowing | Alta (impacto) |
| S107 | Demasiados parámetros | Media |
| S138 | Función demasiado larga | Media |
| S1764 | Indentation | Baja |
| S134 | nesting depth | Baja |
| S3776 | complexity | Media |
| S2757 | Operator precedence | Media |
| S2583 | Collapsible if | Baja |
| S126 | Max switch cases | Baja |

---

## 4. Fase 2: LCPG

> **Duración estimada**: Semanas 7-10  
> **Objetivo**: Análisis semántico ligero

### 4.1 Crear Visitor Trait Reutilizable

```rust
// cognicode-core/src/visitor/trait.rs

pub trait Visitor {
    /// Callback: función encontrada
    fn on_function(&mut self, node: &Node, name: &str) { }

    /// Callback: llamada a función
    fn on_call(&mut self, node: &Node, callee: &str) { }

    /// Callback: asignación
    fn on_assignment(&mut self, node: &Node, var_name: &str) { }

    /// Callback: referencia a variable
    fn on_identifier(&mut self, node: &Node, name: &str) { }

    /// Callback: import
    fn on_import(&mut self, node: &Node, path: &str) { }

    /// Callback genérico para cualquier nodo
    fn on_node(&mut self, node: &Node) { }
}
```

### 4.2 Implementar SymbolTable Builder

```rust
// cognicode-core/src/lcpg/mod.rs

pub struct LcpqBuilder {
    symbols: HashMap<SymbolId, Symbol>,
    pending_refs: Vec<(Span, String)>,
    // ...
}

impl LcpqBuilder {
    pub fn build(mut self, ast: &Tree) -> SymbolTable { ... }
}

impl Visitor for LcpqBuilder { ... }
```

### 4.3 Integrar en RuleContext

```rust
// Actualizar RuleContext para incluir SymbolTable opcional

pub struct RuleContext<'a> {
    pub source: &'a str,
    pub ast: &'a Tree,
    pub symbol_table: Option<&'a SymbolTable>,  // NUEVO
    pub language: Language,
    pub file_path: &'a Path,
}
```

### 4.4 Migrar Reglas LCPG

| Regla | Descripción |
|-------|-------------|
| `unused/public-function` | Función pública sin llamadas |
| `dead/assignment-no-read` | Variable asignada sin leer |
| `unused/import` | Import no utilizado |
| `dead/code-island` | Código sin punto de entrada |

---

## 5. Fase 3: Migración Masiva

> **Duración estimada**: Semanas 11-18  
> **Objetivo**: Migrar las 854 reglas

### 5.1 Orden de Priorización

```
┌──────────────────────────────────────────────────────────────────┐
│                 PRIORIZACIÓN DE MIGRACIÓN                         │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  PRIORIDAD 1: Las 6 reglas con tests fallando                     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ S4792 DES/RC4 (3 tests) - regex lookbehind                │ │
│  │ S5122 SQL Injection (3 tests) - regex lookbehind           │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  PRIORIDAD 2: Reglas de Security                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ S2068 Hardcoded credentials                                 │ │
│  │ S5332 SSL verification disabled                             │ │
│  │ S5631 Server hostname verification                          │ │
│  │ S4792 Weak crypto                                          │ │
│  │ S5122 SQL injection                                        │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  PRIORIDAD 3: Reglas de Bugs                                     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ S1656 Variable shadowing                                    │ │
│  │ S1764 Indentation                                           │ │
│  │ S2589 Short-circuit                                         │ │
│  │ S2757 Operator precedence                                  │ │
│  │ S2259 Null dereference                                      │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  PRIORIDAD 4: Code Smells                                        │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ S107 Too many parameters                                    │ │
│  │ S138 Function too long                                      │ │
│  │ S134 Nesting depth                                          │ │
│  │ S3776 Cognitive complexity                                  │ │
│  │ S1135 TODO comments                                        │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  PRIORIDAD 5: Performance y Resto                                │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 5.2 Naming Mapping

| ID Antiguo | ID Nuevo Descriptivo |
|------------|---------------------|
| S1135 | `convention/todo-comment` |
| S107 | `design/parameters-excessive` |
| S138 | `design/function-too-long` |
| S1656 | `scope/variable-reassigned` |
| S1764 | `format/indentation` |
| S134 | `design/nesting-depth` |
| S3776 | `design/cognitive-complexity` |
| S2068 | `security/hardcoded-credential` |
| S5122 | `security/sql-injection` |
| S4792 | `security/weak-crypto` |
| S5332 | `security/ssl-verification-disabled` |
| S5631 | `security/hostname-verification-disabled` |
| S2259 | `bug/null-dereference` |
| S2589 | `bug/short-circuit` |
| S2757 | `style/operator-precedence` |

### 5.3 Migración por Categoría

#### Security (20 reglas)

```rust
// Ejemplo: S5122 SQL Injection migrada

#[cogni_rule(
    id = "security/sql-injection",
    severity = "Critical",
    category = "Vulnerability",
    languages = ["rust", "go", "java", "python", "javascript"],
    pattern = "$FUNC($$$ARGS)",
    kind = "call_expression",
    has = {
        pattern = "$SQL",
        regex = "^(SELECT|INSERT|UPDATE|DELETE|DROP|CREATE|ALTER|EXEC|EXECUTE)\\b"
    },
    not = {
        pattern = "$FUNC($SQL, $SANITIZED)",
        inside = { pattern = "sanitize($X)", kind = "call_expression" }
    },
    message = "Potential SQL injection: untrusted input reaches SQL query"
)]
struct SqlInjectionRule;

impl Rule for SqlInjectionRule {
    fn layer(&self) -> u8 { 3 }  // Dataflow

    fn required_keywords(&self) -> &[&str] {
        &["sql", "SELECT", "INSERT", "UPDATE", "DELETE", "query", "execute"]
    }
}
```

#### Bugs (50 reglas)

```rust
// Ejemplo: S2259 Null Dereference migrada

#[cogni_rule(
    id = "bug/null-dereference",
    severity = "Critical",
    category = "Bug",
    languages = ["rust", "go", "java"],
    pattern = "$EXPR.$FIELD",
    kind = "field_expression",
    where = {
        "$EXPR" = { regex = "\\b(unwrapped|unwrap\\(\\)|expect\\(.*\\)\\s*\\.)\\b" }
    },
    message = "Null/None dereference detected"
)]
struct NullDereferenceRule;
```

#### Code Smells (100 reglas)

```rust
// Ejemplo: S107 Excessive Parameters migrada

#[cogni_rule(
    id = "design/parameters-excessive",
    severity = "Major",
    category = "CodeSmell",
    languages = ["rust", "go", "java", "python"],
    pattern = "fn $NAME($PARAMS)",
    kind = "function_declaration",
    where = {
        "$PARAMS" = { regex = "^[^,]+(,\\s*[^,]+){7,}$" }
    },
    message = "Function has too many parameters (more than 7). Consider refactoring."
)]
struct ExcessiveParametersRule;
```

### 5.4 Tests Declarativos Inline

```rust
#[cfg(test)]
mod tests {
    #[test_rule(SqlInjectionRule)]
    const SQL_INJECTION_CASES: &[(&str, bool)] = &[
        // True positives
        ("query(\"SELECT * FROM users WHERE id = \" + userId)", true),
        ("db.execute(\"DELETE FROM orders WHERE id = \" + orderId)", true),
        ("cursor.execute(f\"SELECT * FROM {table}\")", true),
        // False positives
        ("query(\"SELECT * FROM users WHERE id = ?\", userId)", false),  // parameterized
        ("sanitize(input)", false),  // sanitized
        ("log_sql(\"SELECT * FROM\")", false),  // not a query
    ];

    #[test_rule(ExcessiveParametersRule)]
    const PARAMETERS_CASES: &[(&str, bool)] = &[
        ("fn foo(a, b, c, d, e, f, g, h) {}", true),  // 8 params
        ("fn bar(a, b, c, d, e, f) {}", false),  // 6 params OK
    ];
}
```

---

## 6. Fase 4: Dataflow

> **Duración estimada**: Semanas 19-24  
> **Objetivo**: Taint tracking para security rules

### 6.1 Implementar TaintTracker

```rust
// cognicode-core/src/dataflow/taint.rs

pub struct TaintTracker<'a> {
    ctx: &'a RuleContext<'a>,
    sources: HashSet<SymbolId>,
    tainted: HashSet<SymbolId>,
    sanitized: HashSet<SymbolId>,
}

impl<'a> TaintTracker<'a> {
    pub fn new(ctx: &'a RuleContext) -> Self { ... }

    /// Registra un source de datos no confiables
    pub fn add_source(&mut self, symbol: SymbolId) {
        self.sources.insert(symbol);
        self.tainted.insert(symbol);
    }

    /// Registra un sanitizer
    pub fn add_sanitizer(&mut self, symbol: SymbolId) {
        self.sanitized.insert(symbol);
        self.tainted.remove(&symbol);
    }

    /// Propaga taint a través de assignments
    pub fn propagate(&mut self, from: SymbolId, to: SymbolId) {
        if self.tainted.contains(&from) {
            self.tainted.insert(to);
        }
    }

    /// Verifica si un símbolo está contaminado
    pub fn is_tainted(&self, symbol: SymbolId) -> bool {
        self.tainted.contains(&symbol)
    }

    /// Retorna el source original del taint
    pub fn source_of(&self, symbol: SymbolId) -> Option<SymbolId> {
        if self.tainted.contains(&symbol) {
            // Walk back through propagation
            Some(symbol)  // Simplified
        } else {
            None
        }
    }
}
```

### 6.2 Reglas con Dataflow

| Regla | Source | Sink |
|-------|--------|------|
| `security/sql-injection` | `request.param()`, `request.body()` | `db.execute()` |
| `security/command-injection` | `request.param()`, `stdin` | `exec()`, `system()` |
| `security/path-traversal` | `request.param()` | `fs.open()`, `Path::new()` |
| `security/hardcoded-credential` | `String` literal | `crypto.hash()`, `auth()` |
| `security/xxe` | `xml.parse()` | `request.body()` |

---

## 7. Checklist de Migración

### Fase 0: Estabilización
- [ ] Fix S4792 DES/RC4 (regex lookbehind → ast-grep)
- [ ] Fix S5122 SQL Injection (regex lookbehind → ast-grep)
- [ ] 294/294 tests pasando

### Fase 1: Infraestructura
- [ ] Crear crate `cognicode-macros`
- [ ] Implementar proc-macro `#[cogni_rule]`
- [ ] Validación compile-time de patterns
- [ ] Definir trait `Rule` evolucionado
- [ ] Implementar `PreflightFilter` con Aho-Corasick
- [ ] Integrar `ast-grep-core`
- [ ] Sistema de auto-registro con `inventory`
- [ ] Migrar 10 reglas como PoC

### Fase 2: LCPG
- [ ] Crear `Visitor` trait reutilizable
- [ ] Implementar `SymbolTable` builder
- [ ] Integrar `SymbolTable` en `RuleContext`
- [ ] Migrar 4 reglas LCPG iniciales

### Fase 3: Migración Masiva
- [ ] Migrar security rules (20)
- [ ] Migrar bug rules (50)
- [ ] Migrar code smell rules (100)
- [ ] Migrar performance rules (50)
- [ ] Migrar resto de rules (634)
- [ ] Naming mapping para todas las reglas
- [ ] Tests declarativos para cada regla migrada

### Fase 4: Dataflow
- [ ] Implementar `TaintTracker`
- [ ] Migrar SQL injection a dataflow
- [ ] Migrar command injection a dataflow
- [ ] Migrar path traversal a dataflow
- [ ] Migrar hardcoded credentials a dataflow

---

## 8. Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|-------------|---------|------------|
| tree-sitter grammar cambia | Baja | Alta | Proc-macro valida en compile-time |
| ast-grep-core tiene bugs | Baja | Media | Fallback a tree-sitter queries |
| Performance regression | Media | Alta | Benchmarks automáticos en CI |
| Tests se rompen durante migración | Alta | Media | Migración incremental + tests por fase |
| Memoria aumenta significativamente | Media | Media | LCPG lazy, solo cuando se necesita |
