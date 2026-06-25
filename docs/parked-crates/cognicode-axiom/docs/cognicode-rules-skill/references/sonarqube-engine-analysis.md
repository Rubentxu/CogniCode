# SonarQube Rule Engine — Análisis Algorítmico para CogniCode

## 1. Motor de Reglas Determinista de SonarQube

SonarQube NO usa LLMs para evaluar reglas. Usa un **motor determinista basado en el patrón Visitor** sobre el AST.

### Flujo de ejecución

```
Source File
     │
     ▼
┌─────────────┐
│  Lexer       │  → Token stream
└──────┬──────┘
       ▼
┌─────────────┐
│  Parser      │  → AST (SyntaxTree)
└──────┬──────┘
       ▼
┌──────────────────────────────────────────┐
│  Rule Engine (SonarComponents)            │
│                                           │
│  For each rule:                           │
│    1. Does rule apply to this language?   │
│    2. Create visitor from rule            │
│    3. visitor.visitTree(ast)              │
│    4. Collect issues from visitor         │
└──────────────────────────────────────────┘
```

### Jerarquía de Visitors

```
                    SyntaxTreeVisitor (abstract)
                           │
              ┌────────────┴────────────┐
              │                         │
   SubscriptionVisitor         IssuableSubscriptionVisitor
   (subscribe to node types)    (subscribe + issue reporting)
              │
   ┌──────────┴──────────┐
   │                      │
BaseTreeVisitor    JavaFileScanner
  (default impl)    (file-level scan)
```

## 2. Patrón SubscriptionVisitor (el más importante)

**Algoritmo**: En lugar de iterar todo el AST, cada regla se **suscribe** a tipos específicos de nodos.

```java
// SonarQube — Regla S2068 (Hardcoded Credentials)
@Rule(key = "S2068")
public class HardcodedCredentialsCheck extends IssuableSubscriptionVisitor {

    // Solo se ejecuta para estos tipos de nodo
    @Override
    public List<Kind> nodesToVisit() {
        return Arrays.asList(
            Kind.STRING_LITERAL,    // Solo strings literales
            Kind.ASSIGNMENT          // Solo asignaciones
        );
    }

    // Se llama AUTOMÁTICAMENTE para cada nodo suscrito
    @Override
    public void visitNode(Tree node) {
        // El motor ya filtró: esto NUNCA es un comentario ni docstring
        if (isPasswordAssignment(node)) {
            reportIssue(node, "Hard-coded credential detected");
        }
    }
}
```

**Por qué esto es determinista y sin falsos positivos:**

1. El motor **solo llama a `visitNode`** para nodos del tipo suscrito
2. Los nodos de tipo `STRING_LITERAL` **nunca** están dentro de comentarios (el parser los excluye)
3. Los nodos de tipo `ASSIGNMENT` son **estructura real del código**, no texto
4. No se escanea texto crudo — se navega el AST

### Equivalente en CogniCode con Tree-Sitter

```rust
// Nuestra versión determinista — MISMO algoritmo
struct S2068Rule;

impl S2068Rule {
    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        
        // Query tree-sitter — solo nodos reales, no comentarios
        let query = tree_sitter::Query::new(
            &ctx.language.to_ts_language(),
            r#"
            (let_declaration
              pattern: (identifier) @var
              value: (string_literal) @val)
            "#
        ).unwrap();
        
        let mut cursor = tree_sitter::QueryCursor::new();
        for m in cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes()) {
            let var = m.captures.iter().find(|c| c.1 == "var").unwrap().0;
            let var_name = var.utf8_text(ctx.source.as_bytes()).unwrap();
            
            // Check determinista — sin regex sobre texto
            if matches!(var_name, "password" | "secret" | "token" | "api_key") {
                issues.push(Issue::new("S2068", ...));
            }
        }
        issues
    }
}
```

## 3. Patrón CheckVerifier — Testing Determinista

SonarQube tiene un framework de testing que **compara la salida esperada con la real** sin depender de asserts manuales.

```java
// SonarQube — Test de regla
public class HardcodedCredentialsCheckTest {

    @Test
    public void test_detects_password() {
        // CheckVerifier: ejecuta la regla sobre el código fuente
        // y verifica que el número de issues y sus líneas coinciden
        CheckVerifier.newVerifier()
            .onFile("src/test/files/S2068.java")  // Archivo de test
            .withCheck(new HardcodedCredentialsCheck())
            .verifyIssues();  // ← Compara con un archivo .java.issues esperado
    }
    
    @Test  
    public void test_no_fp_on_comments() {
        CheckVerifier.newVerifier()
            .onFile("src/test/files/S2068_no_fp.java")
            .withCheck(new HardcodedCredentialsCheck())
            .verifyNoIssues();  // ← Verifica CERO issues
    }
}
```

**El archivo `.java.issues` esperado:**
```
# file: S2068.java
# rule: S2068
# expected issues: 2
L5: Hard-coded credential detected
L12: Hard-coded credential detected
```

**Esto es determinista**: el motor ejecuta la regla, cuenta issues, compara líneas. Sin LLM, sin heurísticas.

### Equivalente en CogniCode

```rust
// Nuestro CheckVerifier determinista
struct RuleTestSpec {
    rule_id: String,
    source: String,
    expected_issues: Vec<ExpectedIssue>,
}

struct ExpectedIssue {
    line: usize,
    message_contains: Vec<String>,
}

fn verify_rule(spec: &RuleTestSpec) -> TestResult {
    let issues = run_rule(&spec.rule_id, &spec.source);
    
    let mut result = TestResult::default();
    
    // Check expected count
    if issues.len() != spec.expected_issues.len() {
        result.add_failure(format!(
            "Expected {} issues, got {}: {:?}",
            spec.expected_issues.len(), issues.len(), issues
        ));
    }
    
    // Check exact locations
    for (expected, actual) in spec.expected_issues.iter().zip(issues.iter()) {
        if actual.line != expected.line {
            result.add_failure(format!(
                "Expected issue at line {}, got line {}",
                expected.line, actual.line
            ));
        }
    }
    
    result
}
```

## 4. Algoritmo de Análisis Semántico (Cross-File)

SonarQube puede analizar archivos cruzados usando **resolución de símbolos**:

```java
// SonarQube — Regla que detecta variables no usadas
@Rule(key = "S1854")
public class UnusedLocalVariableCheck extends IssuableSubscriptionVisitor {
    
    @Override
    public List<Kind> nodesToVisit() {
        return Arrays.asList(Kind.VARIABLE);
    }
    
    @Override
    public void visitNode(Tree node) {
        VariableTree var = (VariableTree) node;
        
        // API SEMÁNTICA — busca usos del símbolo en todo el proyecto
        Symbol symbol = var.symbol();
        List<IdentifierTree> usages = symbol.usages();  // ← Cross-file!
        
        if (usages.size() <= 1) {  // Solo la declaración, sin usos
            reportIssue(var, "Remove this unused variable");
        }
    }
}
```

**Nuestro equivalente**: `ctx.graph` (CallGraph) + tree-sitter scoping.

## 5. Tabla Comparativa: SonarQube vs CogniCode (Actual vs Ideal)

| Componente | SonarQube | CogniCode Actual | CogniCode Ideal |
|-----------|-----------|-----------------|-----------------|
| **Motor** | Visitor pattern sobre AST | `declare_rule!` + `ctx.source.lines()` | Tree-sitter QueryCursor |
| **Suscripción** | `nodesToVisit()` filtra tipos | ❌ Itera todo el texto | `tree_sitter::Query` con capturas |
| **Determinismo** | Solo nodos AST (no texto) | 100+ reglas escanean texto raw | Migrar a queries AST |
| **Testing** | CheckVerifier + `.issues` files | 40 asserts manuales por regla | `verify_rule()` con spec |
| **Semántico** | Symbol API (cross-file) | `ctx.graph` (call graph) | `ctx.graph` + tree-sitter scoping |
| **Suppression** | `// NOSONAR` | No implementado | Añadir `// cognicode:disable=SXXXX` |

## 6. Plan de Implementación Determinista

### Paso 1: Motor de Suscripción (SubscriptionVisitor)

```rust
/// Trait que reemplaza el escaneo de líneas por suscripción a nodos AST
trait SubscriptionRule {
    /// Tipos de nodo a los que esta regla se suscribe
    fn subscribed_nodes(&self) -> Vec<&'static str>;
    
    /// Llamado por el motor para cada nodo suscrito encontrado
    fn visit_node(&self, node: tree_sitter::Node, ctx: &RuleContext) -> Vec<Issue>;
}

/// Motor que ejecuta reglas por suscripción
struct SubscriptionEngine;

impl SubscriptionEngine {
    fn run(&self, rule: &dyn SubscriptionRule, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let subscribed = rule.subscribed_nodes();
        
        // Walk the tree, only call visit_node for subscribed types
        Self::walk(ctx.tree.root_node(), &subscribed, |node| {
            issues.extend(rule.visit_node(node, ctx));
        });
        issues
    }
    
    fn walk(node: tree_sitter::Node, subscribed: &[&str], 
            f: &mut dyn FnMut(tree_sitter::Node)) {
        if subscribed.contains(&node.kind()) {
            f(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk(child, subscribed, f);
        }
    }
}
```

### Paso 2: CheckVerifier Determinista

```rust
/// Spec que reemplaza los 40 asserts por regla
struct RuleSpec {
    id: String,
    source: String,
    language: Language,
    expected: Vec<IssueExpectation>,
}

enum IssueExpectation {
    AtLine { line: usize, message_contains: String },
    NoIssues,
    Count(usize),
}

fn verify_spec(spec: &RuleSpec) -> Result<(), String> {
    let issues = run_rule_by_id(&spec.id, &spec.source, &spec.language);
    
    for expected in &spec.expected {
        match expected {
            IssueExpectation::AtLine { line, message_contains } => {
                let found = issues.iter().any(|i| i.line == *line);
                if !found {
                    return Err(format!("Expected issue at line {}", line));
                }
            }
            IssueExpectation::NoIssues => {
                if !issues.is_empty() {
                    return Err(format!("Expected 0 issues, got {:?}", issues));
                }
            }
            IssueExpectation::Count(n) => {
                if issues.len() != *n {
                    return Err(format!("Expected {} issues, got {}", *n, issues.len()));
                }
            }
        }
    }
    Ok(())
}
```

### Paso 3: Formato de Spec Natural + Determinista

```yaml
# S2068.rule.yaml — ÚNICO archivo que define la regla Y sus tests
id: S2068
name: "Hard-coded credentials should not be used"
severity: Blocker
category: SecurityHotspot
language: rust
engine: subscription  # ← "subscription" o "line_scan"

# Para engine=subscription
subscriptions:
  - node_type: let_declaration
    filter:
      pattern: "(identifier) @var"
      match_var: ["password", "secret", "token", "api_key", "pwd"]
      value_type: string_literal

# Para engine=line_scan (legacy)
line_patterns:
  - regex: '(?:\b|_)(password|secret|token)\s*[=:]\s*["\']'
    skip_comments: true
    skip_strings: true

# Tests deterministas
tests:
  detect:
    - source: 'let password = "secret123";'
      at_line: 1
    - source: 'let api_key = "sk-abc";'
      at_line: 1
      
  no_detect:
    - source: 'let password = get_env("DB_PASS");'
    - source: '// password = "test"'
    - source: 'let password_hash = compute(data);'
```

**Este YAML es:**
1. **Determinista**: El motor lo parsea y ejecuta reglas sin LLM
2. **Evaluable**: `verify_spec()` comprueba cada test automáticamente
3. **Natural**: Las descripciones son legibles por humanos y LLMs
4. **Unificado**: Un solo archivo = regla + tests + documentación
