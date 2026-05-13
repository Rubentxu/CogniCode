# Rules as Code — Proc-Macro + Compile-Time Validation

> **Fecha**: 11 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Diseño de implementación

---

## 1. El Problema con YAML y Configuración Externa

### 1.1 La Fragilidad se Mueve, No Desaparece

El enfoque tradicional de muchas herramientas de análisis usa **archivos de configuración YAML/JSON** para definir reglas:

```yaml
# Ejemplo de regla en Semgrep/ESLint
- id: sec/sql-injection
  pattern: "query($X)"
  message: "Potential SQL injection"
  severity: ERROR
```

**El problema fundamental**: YAML mueve la fragilidad de regex a **strings en config**. Los errores ahora son:
- Errores de **runtime** (no de compilación)
- Errores que solo se detectan al **ejecutar** el linter
- Errores sin **IDE support** (autocomplete, type checking)

### 1.2 Errores Típicos en Enfoque YAML

```yaml
# Error 1: Typo en el pattern - No se detecta hasta runtime
- id: sec/sql-injection
  pattern: "query($X)"  # ❌ Debería ser "query(...)" 
  # El linter acepta esto, pero nunca matcheará nada

# Error 2: Node kind inválido - runtime silent failure
- id: sec/command-injection  
  kind: "call_expresion"  # ❌ Typo: "call_expression"
  # El linter ignora esta regla silenciosamente

# Error 3: Regex inválida - crash en runtime
- id: sec/unsafe-regex
  pattern: "[invalid(regex"  # ❌ Regex malformada
  # El linter puede fallar al cargar esta regla
```

### 1.3 El Costo de Errores en Runtime

| Tipo de Error | YAML/Runtime | Rust/Compile-time |
|---------------|--------------|-------------------|
| Typo en pattern | Silencioso o crash | **Error de compilación** |
| Kind inválido | Silencioso | **Error de compilación** |
| Regex malformada | Crash al cargar | **Error de compilación** |
| Missing field | Default o silencio | **Error de compilación** |
| Type mismatch | Runtime error | **Error de compilación** |

---

## 2. La Solución: Proc-Macro con Validación en Compile-Time

### 2.1 Concepto Central

La proc-macro `#[cogni_rule]` transforma la definición de una regla en código Rust que:

1. **Valida** el pattern contra tree-sitter grammar **en compilación**
2. **Valida** el node kind contra el diccionario conocido
3. **Genera** impl de `Rule` trait con ast-grep matcher precompilado
4. **Registra** automáticamente la regla en el catálogo

### 2.2 Diseño de la Macro

```rust
use cognicode_macros::cogni_rule;

#[cogni_rule(
    id = "sec/crypto-weak-hash",
    severity = "Critical",
    category = "Vulnerability",
    languages = ["rust", "go", "java"],
    pattern = "$FN($$$)",
    kind = "call_expression",
    not = { 
        pattern = "blake3($$$)",
        pattern = "sha3_256($$$)",
    },
    message = "Use of weak cryptographic algorithm detected. Consider BLAKE3 or SHA-256."
)]
struct WeakCryptoRule;
```

### 2.3 Validaciones en Compile-Time

#### Validación del Pattern

```rust
// La macro genera código que valida en compilación:
// 1. Parsear el pattern con ast-grep
// 2. Verificar que sea un pattern AST válido
// 3. Verificar que los placeholders ($$$, $X) sean consistentes

#[cogni_rule(pattern = "func_decl($$$)")]  // ❌ Error de compilación
// Error: "Invalid pattern 'func_decl($$$)'. 
// Did you mean 'function_declaration'?"
```

#### Validación del Kind

```rust
// La macro verifica que el kind exista en el árbol de sintaxis
#[cogni_rule(
    pattern = "foo($X)",
    kind = "call_expresion"  // ❌ Error de compilación
)]
// Error: "Unknown node kind 'call_expresion'. 
// Did you mean 'call_expression'?"
```

#### Validación de Constraints

```rust
// Los campos not/and/or también se validan
#[cogni_rule(
    pattern = "exec($X)",
    not = { pattern = "sanitize($X)" }  // ✅ OK
)]
struct CommandExecRule;
```

---

## 3. Flujo de Compilación Detallado

### 3.1 Fases de la Proc-Macro

```
┌─────────────────────────────────────────────────────────────────┐
│                    COMPILACIÓN DE RUST                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Phase 1: Parsing del Attribute                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  #[cogni_rule(...)]  →  Parsear args a structs            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                          │                                       │
│                          ▼                                       │
│  Phase 2: Validación del Pattern                                │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  pattern = "$FN($$$)"                                      │ │
│  │  ├─ Tokenizar con tree-sitter grammar                      │ │
│  │  ├─ Verificar que $FN es placeholder válido                │ │
│  │  ├─ Verificar que $$$ es varargs válido                    │ │
│  │  └─ Generar AST matcher o error de compilación             │ │
│  └────────────────────────────────────────────────────────────┘ │
│                          │                                       │
│                          ▼                                       │
│  Phase 3: Validación del Kind                                   │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  kind = "call_expression"                                 │ │
│  │  ├─ Consultar dictionary de node kinds                     │ │
│  │  ├─ Verificar que existe para el lenguaje objetivo        │ │
│  │  └─ Generar kind filter o error de compilación            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                          │                                       │
│                          ▼                                       │
│  Phase 4: Generación de Impl                                    │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Genera: impl Rule for WeakCryptoRule { ... }             │ │
│  │  ├─ id() → "sec/crypto-weak-hash"                          │ │
│  │  ├─ severity() → Severity::Critical                       │ │
│  │  ├─ category() → Category::Vulnerability                  │ │
│  │  ├─ check() → { ... AST matching logic ... }             │ │
│  │  ├─ required_keywords() → &["md5", "sha1", ...]           │ │
│  │  └─ layer() → 1                                            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                          │                                       │
│                          ▼                                       │
│  Phase 5: Registro en Inventario                                │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  inventory::submit! { WeakCryptoRule }                     │ │
│  │  └─ Auto-registro en el catálogo global de reglas         │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Mensajes de Error Informativos

La proc-macro proporciona errores **acciónables**:

```rust
// ❌ Pattern con placeholder desconocido
#[cogni_rule(pattern = "$UNKNOWN($$$)")]
struct BadRule;
// Error: "Unknown placeholder '$UNKNOWN'. 
// Valid placeholders are: $X, $Y, $Z, $$, $$$ (varargs)"

// ❌ Pattern malformado
#[cogni_rule(pattern = "func($X")]
struct BadRule2;
// Error: "Unclosed parenthesis in pattern 'func($X'. 
// Expected ')' after '$X'"

// ❌ Kind que no existe para el lenguaje
#[cogni_rule(pattern = "class($NAME)", kind = "class_decl")]
// Para Rust donde no hay "class":
// Error: "Node kind 'class_decl' not found in Rust grammar. 
// Use 'struct_item' for Rust classes."
```

---

## 4. Auto-Registro con Inventory

### 4.1 El Crate `inventory`

El crate `inventory` permite registro automático de items sin macros duplicadas:

```rust
// En cognicode_macros/src/lib.rs
use inventory::inventory;

inventory! {
    pub static ALL_RULES: RuleRegistration;
}

pub struct RuleRegistration {
    pub id: &'static str,
    pub severity: Severity,
    pub category: Category,
    pub layer: u8,
    pub factory: fn() -> Box<dyn Rule>,
}
```

### 4.2 Generación Automática del Registro

La proc-macro `#[cogni_rule]` genera:

```rust
// Lo que la macro genera automáticamente:
inventory::submit! {
    RuleRegistration {
        id: "sec/crypto-weak-hash",
        severity: Severity::Critical,
        category: Category::Vulnerability,
        layer: 1,
        factory: || Box::new(WeakCryptoRule),
    }
}
```

### 4.3 Acceso al Catálogo

```rust
pub struct RuleCatalog {
    rules: Vec<Box<dyn Rule>>,
}

impl RuleCatalog {
    /// Carga todas las reglas registradas vía inventory
    pub fn load_all() -> Self {
        let rules = inventory::iter::<RuleRegistration>
            .map(|reg| (reg.factory)())
            .collect();
        Self { rules }
    }

    /// Busca reglas por ID
    pub fn find(&self, id: &str) -> Option<&dyn Rule> {
        self.rules.iter()
            .find(|r| r.id() == id)
            .map(|r| r.as_ref())
    }

    /// Filtra reglas por severidad
    pub fn filter_by_severity(&self, severity: Severity) -> Vec<&dyn Rule> {
        self.rules.iter()
            .filter(|r| r.severity() == severity)
            .map(|r| r.as_ref())
            .collect()
    }
}
```

---

## 5. Tests Declarativos Inline

### 5.1 El Atributo `#[test_rule]`

Cada regla puede incluir tests declarativos junto a su definición:

```rust
#[cogni_rule(
    id = "sec/crypto-weak-hash",
    severity = "Critical",
    category = "Vulnerability",
    pattern = "$FN($$$)",
    kind = "call_expression",
    not = { pattern = "blake3($$$)" },
    message = "Weak cryptographic algorithm detected"
)]
struct WeakCryptoRule;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::test_rule;

    #[test_rule(WeakCryptoRule)]
    const CASES: &[(&str, bool)] = &[
        // (code_snippet, should_match)
        ("md5(data)", true),           // ❌ FAIL - weak hash
        ("sha1(input)", true),         // ❌ FAIL - weak hash
        ("sha256(data)", true),        // ❌ FAIL - still considered weak
        ("blake3(data)", false),        // ✅ PASS - approved hash
        ("sha3_256(data)", false),     // ✅ PASS - approved hash
        ("hashlib.md5(data)", true),    // ❌ FAIL - md5 via module
        ("CryptoJS.MD5(data)", true),   // ❌ FAIL - JS md5
    ];
}
```

### 5.2 Mecánica de `test_rule`

```rust
/// Macro que genera tests unitarios para una regla
#[proc_macro]
pub fn test_rule(input: TokenStream) -> TokenStream {
    let rule_type = parse_rule_type(&input);
    let test_cases = extract_test_cases(&input);

    let tests = test_cases.iter().map(|(code, should_pass)| {
        let test_name = format!("test_{}_{}", rule_type.name, sanitize(code));
        quote! {
            #[test]
            fn #test_name() {
                let rule = #rule_type {};
                let ctx = RuleContext::mock(#code);
                let issues = rule.check(&ctx);
                let has_issues = !issues.is_empty();

                assert_eq!(
                    has_issues, #should_pass,
                    "Rule {} on '{}': expected {} but got {}",
                    rule.id(), #code, if #should_pass { "issues" } else { "no issues" },
                    if has_issues { format!("{:?}", issues) } else { "no issues".to_string() }
                );
            }
        }
    });

    quote! { #(#tests)* }.into()
}
```

### 5.3 Convenciones de Test

| Sufijo | Significado | Ejemplo |
|--------|-------------|---------|
| `// FAIL` | El código viola la regla | `"md5(data) // FAIL"` |
| `// PASS` | El código cumple la regla | `"blake3(data) // PASS"` |
| `// WARN` | Advertencia (severity menor) | `"old_api() // WARN"` |

```rust
#[test_rule(MyRule)]
const CASES: &[&str] = &[
    "md5(data) // FAIL",
    "sha1(data) // FAIL",
    "blake3(data) // PASS",
    // Tests con comentario inline
];
```

---

## 6. El Trait `Rule` Unificado

### 6.1 Definición del Trait

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Blocker,
    Critical,
    Major,
    Minor,
    Info,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Vulnerability,
    SecurityHotspot,
    Bug,
    CodeSmell,
    Vulnerability,
    Hotspot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Go,
    Java,
    Python,
    JavaScript,
    TypeScript,
    C,
    Cpp,
}

/// Contexto compartido para todas las reglas
pub struct RuleContext<'a> {
    pub source: &'a str,
    pub ast: &'a Tree,
    pub symbol_table: Option<&'a SymbolTable>,
    pub language: Language,
    pub file_path: &'a Path,
}

/// El trait fundamental que todas las reglas implementan
pub trait Rule: Send + Sync + 'static {
    /// Identificador único de la regla (ej: "sec/crypto-weak-hash")
    fn id(&self) -> &str;

    /// Severidad del finding
    fn severity(&self) -> Severity;

    /// Categoría del finding
    fn category(&self) -> Category;

    /// Verifica el código y retorna los issues encontrados
    fn check(&self, ctx: &RuleContext) -> Vec<Issue>;

    /// Keywords requeridas para que esta regla se ejecute
    /// (Layer 0: Pre-flight)
    fn required_keywords(&self) -> &[&str] { &[] }

    /// Indica en qué capa opera esta regla
    /// - 0: Pre-flight (Aho-Corasick)
    /// - 1: Structural (AST)
    /// - 2: Semantic (LCPG)
    /// - 3: Flow (Dataflow/Taint)
    fn layer(&self) -> u8 { 1 }

    /// Lenguajes soportados por esta regla
    fn languages(&self) -> &[Language] { &[Language::Rust] }

    /// Descripción opcional para documentación
    fn description(&self) -> Option<&str> { None }
}

/// Un issue (finding) producido por una regla
#[derive(Debug, Clone)]
pub struct Issue {
    pub rule_id: &'static str,
    pub severity: Severity,
    pub node: Option<Span>,
    pub message: String,
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone)]
pub struct Fix {
    pub replacement: String,
    pub message: String,
}
```

### 6.2 Implementación Base para Rules Estructurales

```rust
/// Implementación base para reglas que usan pattern matching estructural
pub abstract struct StructuralRule {
    pub id: &'static str,
    pub severity: Severity,
    pub category: Category,
    pub pattern: &'static str,
    pub kind: &'static str,
    pub message: String,
}

impl Rule for StructuralRule {
    fn id(&self) -> &str { self.id }
    fn severity(&self) -> Severity { self.severity }
    fn category(&self) -> Category { self.category }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();

        for node in ctx.ast.find_all(self.kind) {
            if self.matches_pattern(&node) {
                issues.push(Issue {
                    rule_id: self.id,
                    severity: self.severity,
                    node: Some(node.span()),
                    message: self.message.clone(),
                    fix: None,
                });
            }
        }

        issues
    }

    fn layer(&self) -> u8 { 1 }
}
```

### 6.3 Ejemplo Completo de Rule

```rust
use cognicode_macros::cogni_rule;

#[cogni_rule(
    id = "sec/crypto-weak-hash",
    severity = "Critical",
    category = "Vulnerability",
    languages = ["rust", "go", "java"],
    pattern = "$FN($$$)",
    kind = "call_expression",
    not = { 
        pattern = "blake3($$$)",
        pattern = "sha3_256($$$)",
        pattern = "sha3_512($$$)",
        pattern = "argon2($$$)",
    },
    message = "Weak cryptographic hash detected. Use BLAKE3, SHA-3, or Argon2."
)]
struct WeakCryptoRule;

#[cfg(test)]
mod tests {
    use crate::testing::test_rule;
    use super::*;

    #[test_rule(WeakCryptoRule)]
    const CASES: &[(&str, bool)] = &[
        ("md5(data)", true),
        ("sha1(data)", true),
        ("sha256(data)", false),  // Considerado seguro
        ("blake3(data)", false),
        ("hashlib.md5(b'data')", true),
    ];
}
```

---

## 7. Integración con el Sistema de Capas

### 7.1 Relación Proc-Macro ↔ Layer

```rust
#[cogni_rule(
    // ...
)]
struct MyStructuralRule;  // → layer() = 1

// Para reglas de Layer 2 (Semantic), se puede extender:
#[cogni_rule(
    // ...
)]
struct MySemanticRule;

impl Rule for MySemanticRule {
    fn layer(&self) -> u8 { 2 }  // Override explícito

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let symbol_table = ctx.symbol_table
            .expect("LCPG required for semantic rules");

        // Usar el SymbolTable para queries semánticas
        // ...
    }
}
```

### 7.2 Required Keywords para Pre-Flight

```rust
#[cogni_rule(
    id = "sec/sql-injection",
    severity = "Critical",
    category = "Vulnerability",
    // ...
)]
struct SqlInjectionRule;

impl Rule for SqlInjectionRule {
    // Pre-flight: estas keywords DEBEN estar presentes
    fn required_keywords(&self) -> &[&str] {
        &["sql", "SELECT", "INSERT", "UPDATE", "DELETE", "query", "format!"]
    }

    fn layer(&self) -> u8 { 3 }  // Dataflow analysis
}
```

---

## 8. Mapeo de IDs de Reglas

### 8.1 Sistema de Naming Descriptivo

| ID Antiguo | ID Nuevo Descriptivo |
|------------|---------------------|
| S1135 | `convention/todo-comment` |
| S107 | `design/parameters-excessive` |
| S138 | `design/function-too-long` |
| S1656 | `scope/variable-reassigned` |
| S2068 | `security/hardcoded-credential` |
| S5122 | `security/sql-injection` |
| S4792 | `security/weak-crypto` |
| S5332 | `security/ssl-verification-disabled` |

### 8.2 Formato de IDs

```
{category}/{descriptor}

Donde:
- category: sec (security), bug, design, convention, performance, etc.
- descriptor: kebab-case descriptivo

Ejemplos:
- sec/sql-injection
- bug/null-dereference  
- design/parameters-excessive
- convention/todo-comment
- performance/regex-redos
```
