# Arquitectura del Motor de Reglas — 4 Capas

> **Fecha**: 11 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Diseño arquitectónico propuesto

---

## 1. Visión General

El motor de reglas de CogniCode Rules Pro se organiza en **4 capas jerárquicas**, cada una especializada en un tipo de análisis con características de validación en tiempo de compilación. Este diseño permite seleccionar la capa adecuada para cada regla, optimizando tanto el rendimiento como la precisión.

```
┌─────────────────────────────────────────────────────────────┐
│                    LAYER 3: FLOW                            │
│         Dataflow / Taint Tracking (5% de reglas)            │
│         ┌─────────────────────────────────────────┐         │
│         │  SQL Injection, Hardcoded Credentials, │         │
│         │  Null Dereference, Data Leakage         │         │
│         └─────────────────────────────────────────┘         │
├─────────────────────────────────────────────────────────────┤
│                    LAYER 2: SEMANTIC                        │
│         LCPG - Lightweight Code Property Graph (15%)        │
│         ┌─────────────────────────────────────────┐         │
│         │  Símbolos, Referencias, Calls,          │         │
│         │  Dead Code, Unused Functions           │         │
│         └─────────────────────────────────────────┘         │
├─────────────────────────────────────────────────────────────┤
│                    LAYER 1: STRUCTURAL                      │
│         AST Pattern Matching (80% de reglas)                │
│         ┌─────────────────────────────────────────┐         │
│         │  ast-grep-core via proc-macro           │         │
│         │  Compile-time validation                 │         │
│         └─────────────────────────────────────────┘         │
├─────────────────────────────────────────────────────────────┤
│                    LAYER 0: PRE-FLIGHT                      │
│         Aho-Corasick ultra-fast text scan (todas)           │
│         ┌─────────────────────────────────────────┐         │
│         │  Keyword matching antes de AST parse    │         │
│         │  O(n) con cualquier número de patterns   │         │
│         └─────────────────────────────────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

### Principios de Diseño

1. **Gradualidad**: Cada capa añade complejidad solo cuando es necesaria
2. **Validación temprana**: Los errores se detectan en compilación, no en runtime
3. **Rendimiento**: Las capas inferiores filtran antes de invocar las superiores
4. **Reutilización**: Las capas superiores heredan capacidades de las inferiores

---

## 2. Layer 0: Pre-Flight (Aho-Corasick)

### 2.1 Propósito

Descartar reglas irrelevantes **antes** de parsear el AST mediante escaneo ultra-rápido de texto plano. Si un archivo no contiene la palabra `sql`, no tiene sentido cargar las reglas de SQL injection.

### 2.2 Mecánica

```rust
use aho_corasick::AhoCorasick;

pub struct PreflightFilter {
    automaton: AhoCorasick,
    keyword_to_rules: Vec<Vec<RuleId>>,
}

impl PreflightFilter {
    /// Filtra las reglas relevantes basándose en keywords presentes
    pub fn filter_rules(&self, source: &str, all_rules: &[RuleId]) -> Vec<RuleId> {
        let present_keywords: HashSet<usize> = self.automaton
            .find_iter(source)
            .map(|m| m.pattern().as_usize())
            .collect();

        all_rules
            .iter()
            .cloned()
            .filter(|rule_id| {
                let rule_keywords = &self.keyword_to_rules[rule_id.index()];
                rule_keywords.iter().any(|ki| present_keywords.contains(ki))
            })
            .collect()
    }
}
```

### 2.3 Características

| Aspecto | Detalle |
|---------|---------|
| **Complejidad** | O(n) donde n = longitud del texto |
| **Memoria** | Construcción O(m) donde m = suma de longitudes de patterns |
| **False Negatives** | Imposibles (si la keyword existe, la regla se carga) |
| **False Positives** | Posibles pero la capa 1 los filtra |

### 2.4 Ejemplo Práctico

```rust
// Regla S5122: SQL Injection
fn required_keywords(&self) -> &[&str] {
    &["sql", "SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "format!", "format_args"]
}

// Preflight check
// Si el archivo NO contiene ninguna de estas keywords → saltar S5122
```

---

## 3. Layer 1: Structural (AST Pattern Matching)

### 3.1 Propósito

Realizar pattern matching estructural sobre el AST usando **ast-grep-core** integrado con proc-macros de validación en tiempo de compilación. Esta capa来处理 el 80% de las reglas.

### 3.2 Proc-Macro `#[cogni_rule]`

La macro valida los patterns contra el tree-sitter grammar **durante la compilación**:

```rust
use cognicode_macros::cogni_rule;

#[cogni_rule(
    id = "sec/crypto-weak-hash",
    severity = "Critical",
    category = "Vulnerability",
    pattern = "$FN($$$)",
    kind = "call_expression",
    not = { pattern = "blake3($$$)" },
    message = "Weak cryptographic hash function detected. Use BLAKE3, SHA-256, or stronger."
)]
struct WeakCryptoRule;
```

### 3.3 Validación en Compile-Time

La proc-macro realiza las siguientes validaciones:

1. **`pattern`**: Verifica que el pattern sea parseable por ast-grep
2. **`kind`**: Verifica que el node kind exista en el tree-sitter grammar
3. **`not` / `and` / `or`**: Valida constraints secundarias

```rust
// Si el usuario escribe:
#[cogni_rule(
    pattern = "func_decl($$$)",  // ❌ Error: "Invalid node kind 'func_decl'. Did you mean 'function_declaration'?"
    kind = "call"
)]
struct BadRule;

// Compilación falla con mensaje claro
```

### 3.4 Operadores Soportados

| Operador | Descripción | Ejemplo |
|----------|-------------|---------|
| `pattern` | AST pattern principal | `pattern = "$FN($$$)"` |
| `kind` | Node kind esperado | `kind = "call_expression"` |
| `regex` | Regex sobre el contenido del nodo | `regex = "^sql.*"` |
| `inside` | El nodo debe estar dentro de | `inside = "function_body"` |
| `has` | Debe tener un hijo matching | `has = "identifier"` |
| `precedes` | Debe preceder a otro nodo | `precedes = "$ARG"` |
| `follows` | Debe seguir a otro nodo | `follows = "let_binding"` |
| `all` | Todos los nodos matching | `all = { has = "type_annotation" }` |
| `any` | Al menos uno | `any = { pattern = "a()", pattern = "b()" }` |
| `not` | Negación | `not = { pattern = "safe($X)" }` |

### 3.5 Implementación de la Regla

```rust
pub struct WeakCryptoRule;

impl Rule for WeakCryptoRule {
    fn id(&self) -> &str { "sec/crypto-weak-hash" }
    fn severity(&self) -> Severity { Severity::Critical }
    fn category(&self) -> Category { Category::Vulnerability }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();

        for node in ctx.ast().find_all("call_expression") {
            let func_name = node.child_by_name("identifier")
                .map(|n| n.text());

            match func_name {
                Some("md5" | "sha1" | "sha224" | "sha256" | "sha384" | "sha512" | "des" | "rc4") => {
                    issues.push(Issue::new(
                        rule_id: self.id(),
                        severity: self.severity(),
                        node,
                        message: format!("Use of weak cryptographic algorithm '{}'. Consider BLAKE3.", func_name),
                    ));
                }
                _ => {}
            }
        }

        issues
    }

    fn layer(&self) -> u8 { 1 }
    fn required_keywords(&self) -> &[&str] {
        &["md5", "sha1", "sha256", "des", "rc4", "hash", "crypto"]
    }
}
```

---

## 4. Layer 2: Semantic (LCPG)

### 4.1 Propósito

Análisis semántico ligero usando el **Lightweight Code Property Graph** (LCPG). Permite responder preguntas como:

- "¿Esta función pública es llamada desde fuera del módulo?"
- "¿Esta variable es asignada pero nunca leída?"
- "¿Este import es utilizado?"

### 4.2 Estructura del LCPG

```rust
pub struct SymbolTable {
    symbols: HashMap<String, Symbol>,
    calls: Vec<CallEdge>,
    references: Vec<RefEdge>,
    /// Mapa de nombre → Vec<SymbolId> para lookup rápido
    by_name: HashMap<String, Vec<SymbolId>>,
}

pub struct Symbol {
    /// Identificador único del símbolo
    id: SymbolId,
    /// Nombre del símbolo
    name: String,
    /// Kind: Function, Variable, Import, Type, Constant
    kind: SymbolKind,
    /// Visibilidad: Public, Private, Crate, Super
    visibility: Visibility,
    /// Span en el source code
    span: Span,
    /// Símbolos que referencian a este
    references_in: Vec<SymbolId>,
    /// Símbolos que este referencia
    references_out: Vec<SymbolId>,
}

#[derive(Clone, Copy)]
pub enum SymbolKind {
    Function,
    Variable,
    Parameter,
    Import,
    Type,
    Constant,
    Struct,
    Enum,
    Trait,
    Impl,
}

#[derive(Clone, Copy)]
pub enum Visibility {
    Public,
    Crate,
    Super,
    Private,
}
```

### 4.3 Integración con RuleContext

```rust
impl RuleContext {
    /// Retorna el SymbolTable construido para el archivo actual
    pub fn symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    /// Busca un símbolo por nombre
    pub fn find_symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbol_table.by_name.get(name)
            .and_then(|ids| ids.first())
            .and_then(|id| self.symbol_table.symbols.get(id))
    }

    /// Retorna todas las referencias a un símbolo
    pub fn get_references(&self, symbol_id: SymbolId) -> &[SymbolId] {
        &self.symbol_table.symbols.get(&symbol_id)
            .map(|s| s.references_in.as_slice())
            .unwrap_or(&[])
    }
}
```

### 4.4 Reglas Habilitadas por LCPG

| Regla | Descripción | Query LCPG |
|-------|-------------|------------|
| `unused/public-function` | Función pública sin llamadas externas | `visibility=Public && references_in.is_empty()` |
| `dead/code-island` | Código sin punto de entrada | Análisis de grafo de calls |
| `unused/variable-assigned` | Variable asignada pero no leída | `kind=Variable && references_out.is_empty()` |
| `unused/import` | Import no utilizado | `kind=Import && references_out.is_empty()` |
| `shadow/redefinition` | Variable que reasigna otra | Análisis de scope |

### 4.5 Construcción del LCPG

```rust
pub struct LcpqBuilder {
    symbols: HashMap<String, Symbol>,
    pending_refs: Vec<(Span, String)>, // (ubicación de referencia, nombre referenciado)
}

impl LcpqBuilder {
    pub fn build(mut self, ast: &Tree) -> SymbolTable {
        // Pass 1: Recolectar definiciones de símbolos
        self.visit_definitions(ast.root_node());

        // Pass 2: Recolectar referencias y resolver
        self.visit_references(ast.root_node());

        // Pass 3: Crear edges de llamadas
        self.build_call_edges();

        SymbolTable {
            symbols: self.symbols,
            calls: self.call_edges,
            references: self.ref_edges,
            by_name: self.by_name,
        }
    }
}

impl Visitor for LcpqBuilder {
    fn on_function(&mut self, node: &Node) {
        let name = node.child_by_name("identifier")
            .map(|n| n.text().to_string())
            .unwrap_or_default();
        let visibility = self.compute_visibility(node);

        let symbol = Symbol {
            id: self.next_id(),
            name: name.clone(),
            kind: SymbolKind::Function,
            visibility,
            span: node.span(),
            references_in: Vec::new(),
            references_out: Vec::new(),
        };

        self.symbols.insert(symbol.id, symbol);
        self.by_name.entry(name).or_default().push(symbol.id);
    }

    fn on_call(&mut self, node: &Node) {
        if let Some(func_name) = node.child_by_name("identifier").map(|n| n.text()) {
            self.pending_refs.push((node.span(), func_name.to_string()));
        }
    }
}
```

---

## 5. Layer 3: Flow (Dataflow / Taint Tracking)

### 5.1 Propósito

Análisis de flujo de datos para reglas de seguridad críticas donde el matching estructural es insuficiente. Esta capa maneja el **5% de las reglas más complejas**.

### 5.2 Tipos de Análisis

#### Taint Tracking
Rastreo de datos desde sources (entrada de usuario) hasta sinks (uso peligroso):

```rust
/// Source: donde datos externos entran al programa
enum Source {
    RequestParam,
    RequestBody,
    EnvironmentVar,
    FileRead,
    Stdin,
    // ...
}

/// Sink: donde datos no validados son usados peligrosamente
enum Sink {
    SqlQuery,        // "SELECT * FROM users WHERE id = " + tainted
    FileOpen,       // open(tainted_path)
    CommandExec,    // exec(tainted_cmd)
    HtmlRender,     // innerHTML = tainted
    CryptoUse,      // crypto.update(tainted)
    // ...
}

///Propagation: cómo se mueve el taint
enum Propagation {
    Direct,        // x = tainted
    Concat,        // x = "prefix" + tainted
    Sanitizer,     // x = sanitize(tainted) → untaint
    Filter,        // x = taint.filter(...) → partial taint
}
```

### 5.3 Reglas Implementadas en Layer 3

| Regla | Descripción | Taint Flow |
|-------|-------------|------------|
| `sec/sql-injection` | SQL injection | `RequestParam → SqlQuery` |
| `sec/hardcoded-credentials` | Credenciales en código | `Literal → CryptoUse` |
| `sec/command-injection` | OS command injection | `RequestParam → CommandExec` |
| `sec/path-traversal` | Path traversal | `RequestParam → FileOpen` |
| `sec/xxe` | XML external entity | `RequestBody → XmlParse` |
| `bug/null-dereference` | Null pointer deref | `Nullable → Dereference` |

### 5.4 Ejemplo de Implementación

```rust
pub struct SqlInjectionRule;

impl Rule for SqlInjectionRule {
    fn id(&self) -> &str { "sec/sql-injection" }
    fn severity(&self) -> Severity { Severity::Critical }
    fn category(&self) -> Category { Category::Security }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let taint_tracker = TaintTracker::new(ctx);

        // Find all SQL query constructions
        for query_node in ctx.ast().find_all("call_expression") {
            if !is_sql_api(&query_node) {
                continue;
            }

            // Get the arguments to the SQL function
            let args = query_node.children()
                .filter(|n| n.kind() == "string_literal");

            for arg in args {
                let taint = taint_tracker.get_taint(arg);

                if taint.is_tainted() {
                    issues.push(Issue::new(
                        rule_id: self.id(),
                        severity: self.severity(),
                        node: arg,
                        message: format!(
                            "Potential SQL injection: untrusted input '{}' reaches SQL query",
                            taint.source()
                        ),
                    ));
                }
            }
        }

        issues
    }

    fn layer(&self) -> u8 { 3 }
    fn required_keywords(&self) -> &[&str] {
        &["sql", "SELECT", "INSERT", "query", "execute", "format!"]
    }
}
```

---

## 6. Diagrama de Flujo Completo

```
┌──────────────────────────────────────────────────────────────────┐
│                        SOURCE CODE                                │
│                    (archivo.rs)                                  │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│                     LAYER 0: PRE-FLIGHT                          │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  Aho-Corasick Scanner                                       │ │
│  │  1. Extrae keywords del archivo                             │ │
│  │  2. Filtra reglas cuyo required_keywords ⊈ presentes        │ │
│  │  3. Retorna lista de reglas candidatas                      │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  Input:  "SELECT * FROM users"                                   │
│  Output: [S5122, S2068, ...] (reglas de SQL)                    │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│                     LAYER 1: STRUCTURAL                          │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  AST Parser (tree-sitter) → Parsear a AST                  │ │
│  │  ast-grep-core → Pattern matching sobre AST                │ │
│  │  Compile-time validation de patterns via proc-macro        │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  Input: AST del archivo                                          │
│  Output: Findings estructurales                                  │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│                     LAYER 2: SEMANTIC (LCPG)                     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  SymbolTable Builder → Construir grafo de símbolos          │ │
│  │  Queries: "función sin calls", "variable sin reads"        │ │
│  │  Reutilizable via Visitor trait                            │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  Input: AST + símbolos                                           │
│  Output: Findings semánticos                                     │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│                     LAYER 3: FLOW (Taint)                       │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  Taint Tracker → Dataflow analysis                         │ │
│  │  Sources → Propagations → Sinks                            │ │
│  │  Only for: security-critical rules                          │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  Input: AST + CFG                                               │
│  Output: Security findings (SQL injection, etc.)                 │
└──────────────────────────────────────────────────────────────────┘
```

---

## 7. Comparativa con Herramientas Existentes

| Herramienta | Enfoque Principal | Fortaleza | Debilidad |
|-------------|-------------------|-----------|-----------|
| **SonarQube** | AST + heuristics | Excelente cobertura | Java-centric, regex limitado |
| **Semgrep** | AST patterns + YAML | Pattern matching poderoso | YAML en runtime, sin compile-time validation |
| **Clippy** | Rust lints | Excelente para Rust | Solo Rust, solo warnings |
| **CodeQL** | Query language + AST | Muy expresivo | Curva de aprendizaje alta, requiere query language |
| **ast-grep** | AST patterns + Rust | Compile-time validation | Solo structural, no semantic |
| **CogniCode Pro** | 4 capas + LCPG + Taint | Tudo + compile-time + reputation | Nuevo, menos maduro |

### Ventajas de CogniCode Rules Pro

1. **Compile-time validation**: Errores de patterns detectados en compilación
2. **4 capas especializadas**: Optimización por tipo de análisis
3. **Visitor trait reutilizable**: DRY para traversal de AST
4. **LCPG local**: Análisis semántico sin overhead de análisis completo
5. **FP reputation system**: Aprendizaje colectivo de falsos positivos
6. **Rust nativo**: Sin dependencias externas de runtime

---

## 8. Decisiones Arquitectónicas

### 8.1 Proc-Macro sobre YAML

**Decisión**: Usar proc-macro `#[cogni_rule]` en lugar de YAML para definir reglas.

**Razón**: 
- Validación en compile-time vs. runtime errors
- IDE support completo (autocomplete, type checking)
- Refactoring seguro

### 8.2 Rust Types sobre JSON/DSL

**Decisión**: Rules como structs de Rust con campos tipados.

**Razón**:
- Type safety
- Sin parsing de strings
- Composable patterns

### 8.3 LCPG Local sobre Análisis Global

**Decisión**: LCPG opera por-archivo, no inter-procedural.

**Razón**:
- Escalabilidad: O(n) donde n = tamaño del archivo
- Parallelizable: Cada archivo se analiza independientemente
- Suficiente para 95% de los casos de uso

### 8.4 Taint Tracking Opcional

**Decisión**: Layer 3 solo para reglas que lo requieren explícitamente.

**Razón**:
- El 95% de las reglas no necesitan taint tracking
- El overhead de dataflow es significativo
- Mejor usar resources donde importa realmente
