# SonarQube Deep-Dive Analysis: Strategy for cognicode-axiom Integration

**Project**: cognicode-axiom  
**Date**: April 30, 2026  
**Author**: SDD Executor  
**Status**: Research Analysis  

---

## 1. Hallazgo Clave: SonarQube MCP Server Ya Existe

### 1.1 Official SonarQube MCP Server by SonarSource

SonarSource, la empresa detrás de SonarQube, ha publicado un **MCP Server oficial** que permite a agentes de IA integrarse directamente con SonarQube. Este descubrimiento transforma fundamentalmente nuestra estrategia para cognicode-axiom.

**Docker Image**: `mcp/sonarqube`  
**Repositorio**: SonarSource/sonarqube-mcp-server  
**Documentation**: https://github.com/SonarSource/sonarqube-mcp-server

### 1.2 Toolsets Disponibles

El MCP Server de SonarQube expone los siguientes toolsets:

| Toolset | Capabilities |
|---------|---------------|
| `analysis` | Execute analysis, get analysis status, compare versions |
| `issues` | Search, create, update, assign issues; add comments |
| `quality-gates` | Evaluate quality gate, get project status |
| `projects` | List, create, delete, update projects |
| `security-hotspots` | Review security hotspots, update security review status |
| `rules` | Search rules, get rule details, create custom rules |
| `sources` | Get source code with issues, syntax highlighting |
| `duplications` | Find code duplications |
| `measures` | Get measures, history, treemap |
| `languages` | Get supported languages |

### 1.3 Integración con Claude Code

```json
{
  "mcpServers": {
    "sonarqube": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "mcp/sonarqube"],
      "env": {
        "SONAR_HOST_URL": "http://localhost:9000",
        "SONAR_TOKEN": "your-token"
      }
    }
  }
}
```

### 1.4 Qué Significa Esto para Nuestra Estrategia

**Antes de este descubrimiento**, la pregunta era:
> "¿Deberíamos construir nuestro propio sistema de análisis estático similar a SonarQube?"

**Ahora la pregunta correcta es**:
> "¿Deberíamos construir componentes nativos en Rust (cognicode-axiom) que complementen y se integren con SonarQube, o intentar reemplazar toda su funcionalidad?"

La respuesta honesta: **Construir todo desde cero en Rust sería un error estratégico**. SonarQube tiene 15+ años de desarrollo, miles de reglas, y un ecosistema de plugins maduro. La integración es la estrategia correcta.

---

## 2. Arquitectura de SonarQube

### 2.1 Componentes Core

```
┌─────────────────────────────────────────────────────────────────┐
│                        SONARQUBE                                 │
│                                                                 │
│  ┌─────────────┐    ┌─────────────────┐    ┌───────────────┐  │
│  │   SCANNER    │───▶│  COMPUTE ENGINE │───▶│    DATABASE   │  │
│  │  (External)  │    │  (Post-process) │    │   (PostgreSQL │  │
│  └─────────────┘    └─────────────────┘    │   or HA mode) │  │
│                          │                   └───────────────┘  │
│                          │                                        │
│                    ┌─────▼─────┐                                 │
│                    │  SEARCH   │                                 │
│                    │  SERVER   │                                 │
│                    │(Elasticsearch)                             │
│                    └───────────┘                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Flujo de Análisis

1. **Scanner** (sonar-scanner, build tool plugin, CI plugin)
   - Analiza el código fuente localmente
   - Produce un "report" JSON/Protobuf
   - No requiere acceso a la base de datos

2. **Server Reception**
   - Recibe el report del scanner
   - Lo persiste en la base de datos

3. **Compute Engine** (Background processing)
   - **Post-processing**: Calcula métricas, ratings, aggregations
   - **Rating computation**: A-E para cada dimensión
   - **Duplication detection**: Token-based comparison
   - **Coverage aggregation**: Combina reports de coverage

4. **Search Server** (Elasticsearch)
   - Indexa issues, medidas,山水代码
   - Provee búsqueda rápida
   - Maneja dashboards

### 2.3 Language Analyzers

SonarQube usa un enfoque mixto:

| Language | Parser/Technology | Notes |
|----------|-------------------|-------|
| Java, JS/TS, Python, C/C++, C# | **SSLR** (SonarSource Language Recognition) | Librería propietaria de SonarSource |
| Go | SSLR + Go parser | |
| Kotlin | SSLR + Kotlin grammar | |
| Others | Plugin-based | |

**Critical Limitation**: Los language analyzers son **propietarios** y escritos en Java. No hay forma de extenderlos con código externo sin escribir un plugin Java.

### 2.4 Plugin System

```
┌──────────────────────────────────────────────────────────────┐
│                    PLUGIN ECOSYSTEM                           │
│                                                              │
│  ┌────────────┐  ┌────────────┐  ┌────────────────────────┐ │
│  │  Language  │  │  Governance │  │     Integration       │ │
│  │  Plugins   │  │  (PHP, COBOL)│  │  (GitHub, Jira, etc.) │ │
│  └────────────┘  └────────────┘  └────────────────────────┘ │
│                                                              │
│  ⚠️ LIMITATION: Plugins must be written in Java              │
│  ⚠️ LIMITATION: Plugin API is internal, unstable             │
└──────────────────────────────────────────────────────────────┘
```

---

## 3. Mapa de Features: SonarQube vs CogniCode

### 3.1 Tabla Comparativa Detallada

| Feature | SonarQube | CogniCode | Gap Analysis |
|---------|-----------|-----------|--------------|
| **Análisis de Grafos** | | | |
| Call Graph | ✅ Sí | ✅ Sí | Paridad |
| Entry Points | ✅ Sí | ✅ Sí | Paridad |
| Hot Paths | ✅ Sí | ✅ Sí | Paridad |
| Impact Analysis | ✅ Sí | ✅ Sí | Paridad |
| **Análisis de Complejidad** | | | |
| Cyclomatic Complexity | ✅ Sí | ✅ Sí | Paridad |
| Cognitive Complexity | ✅ Sí | ✅ Sí | Paridad |
| Depth of Nesting | ✅ Sí | ✅ Limitado | Mejora posible |
| **Arquitectura** | | | |
| Cycle Detection | ✅ Tarjan | ✅ Tarjan | Paridad |
| Architecture Rules | ✅ Sí | ❌ No | GAP |
| Layer Validation | ✅ Sí | ❌ No | GAP |
| **Métricas de Código** | | | |
| Lines of Code | ✅ Sí | ✅ Sí | Paridad |
| Comment Ratio | ✅ Sí | ⚠️ Parcial | Mejora posible |
| Duplications | ✅ Token-based | ❌ No | GAP significativo |
| Dead Code | ✅ Sí | ⚠️ Básico | Mejora necesario |
| **Code Smells** | | | |
| God Classes | ✅ Sí | ❌ No | GAP |
| Long Methods | ✅ Sí | ⚠️ Parcial | Mejora necesario |
| Deep Nesting | ✅ Sí | ⚠️ Parcial | Mejora necesario |
| Too Many Parameters | ✅ Sí | ❌ No | GAP |
| Feature Envy | ✅ Sí | ❌ No | GAP |
| Shotgun Surgery | ✅ Sí | ❌ No | GAP |
| **Security** | | | |
| SAST (Static Analysis) | ✅ Sí | ❌ No | GAP significativo |
| Taint Analysis | ✅ Sí (Enterprise+) | ❌ No | GAP |
| Security Hotspots | ✅ Sí | ❌ No | GAP |
| Vulnerabilities DB | ✅ Sí (CWE, OWASP) | ❌ No | GAP |
| **Quality Governance** | | | |
| Rules Engine | ✅ Sí (poderoso) | ⚠️ Básico (ADR) | GAP |
| Quality Profiles | ✅ Sí | ❌ No | GAP |
| Quality Gates | ✅ Sí | ❌ No | GAP |
| Technical Debt | ✅ SQALE | ❌ No | GAP |
| **Integración** | | | |
| MCP Server | ✅ Oficial | ✅ Sí | Paridad |
| SARIF Support | ✅ Sí | ❌ No | Oportunidad |
| CI/CD Integration | ✅ Excelente | ⚠️ Básico | Mejora posible |
| **Ventajas Diferenciales** | | | |
| Real-time Analysis | ⚠️ Post-commit | ✅ Sí | CogniCode gana |
| No JVM required | ❌ Requiere JDK | ✅ Rust-native | CogniCode gana |
| AI Agent Integration | ⚠️ MCP básico | ✅ Profundo | CogniCode gana |
| Entropy Analysis | ❌ No | ✅ Connascence | CogniCode gana |
| Chronos Integration | ❌ No | ✅ Sí | CogniCode gana |
| SDD Workflow | ❌ No | ✅ Sí | CogniCode gana |
| Reflexión/Governance | ❌ No | ✅ Cedar/OPA | CogniCode gana |

### 3.2 Análisis de Gaps Críticos

**Gaps donde SonarQube es significativamente mejor**:
1. **Duplications detection**: Token-based, muy preciso
2. **Security analysis**: SAST completo con taint analysis
3. **Code smells factory**: God classes, feature envy, shotgun surgery
4. **Quality Profiles/Gates**: Sistema maduro de governance
5. **Technical Debt calculation**: Metodología SQALE

**Gaps donde CogniCode ya gana**:
1. **Real-time**: Sin necesidad de commit/push
2. **Rust-native**: Sin overhead de JVM
3. **Agentic**: Diseñado para AI agents desde el inicio
4. **Entropy/SOLID**: Análisis de arquitectura desde primera principios
5. **Chronos**: Trazado de ejecución para debugging

---

## 4. Sistema de Reglas de SonarQube (Deep Dive)

### 4.1 Dos Modos de Operación

**Standard Mode** (legacy):
- Una regla = un issue
- Simplicidad, pero limitada expresividad

**MQR Mode (Multi-Quality Rule)** (moderno):
- Una regla puede generar múltiples issues
- Permite expresar relaciones entre code smells
- Ejemplo: "This method is both a long method AND has high cyclomatic complexity"

### 4.2 Tipos de Reglas

| Type | Purpose | Example |
|------|---------|---------|
| `BUG` | Defectos de comportamiento | NullPointerException, resource leak |
| `VULNERABILITY` | Security flaws | SQL injection, XSS |
| `CODE_SMELL` | Maintainability issues | Long method, duplicated code |
| `SECURITY_HOTSPOT` | Security-sensitive code | Use of crypto, authentication |

### 4.3 Severidad

| Severity | Meaning | Typical Use |
|----------|---------|-------------|
| **BLOCKER** | Crash, memory corruption | Resource leaks, security breaches |
| **CRITICAL** | Security, potential crashes | SQL injection, unhandled exceptions |
| **MAJOR** | Significant maintainability | Long method, duplicate code |
| **MINOR** | Minor issues | Missing default case, TODO comments |
| **INFO** | Not a problem | Documentation requests |

### 4.4 Cómo se Determina la Severidad

La severidad se calcula mediante una **matriz impact × likelihood**:

```
Impact: ¿Cuánto daño causa si ocurre?
  - HIGH: Security breach, data loss
  - MEDIUM: Runtime error, incorrect behavior
  - LOW: Minor issue, cosmetic

Likelihood: ¿Cuán probable es que ocurra?
  - HIGH: Common path, frequent usage
  - MEDIUM: Edge case, specific scenarios
  - LOW: Very rare, almost never

Matrix:
                    │ HIGH   │ MEDIUM │ LOW
  ──────────────────┼────────┼────────┼─────
  HIGH likelihood   │BLOCKER │CRITICAL│MAJOR
  MEDIUM likelihood │CRITICAL│MAJOR   │MINOR
  LOW likelihood    │MAJOR   │MINOR   │INFO
```

### 4.5 Cómo se Implementan las Reglas (Java Plugins)

```java
// Ejemplo simplificado de rule en plugin Java
@Rule(key = "S1234",
      name = "Cognitive Complexity",
      description = "Methods should not have high cognitive complexity",
      priority = RulePriority.MAJOR,
      tags = {"brain-overload", "design"})
public class CognitiveComplexityCheck extends IssuableSubscriptionVisitor {

    @Override
    public List<Kind> nodesToVisit() {
        return Collections.singletonList(Kind.METHOD);
    }

    @Override
    public void visitNode(Tree tree) {
        MethodTree method = (MethodTree) tree;
        int complexity = calculateCognitiveComplexity(method);
        
        if (complexity > 15) {
            context.addIssue(method, this, 
                "Cognitive complexity is " + complexity + 
                " (max is 15)");
        }
    }
}
```

**Flujo de AST Visitor**:
1. El scanner usa SSLR para parsear código
2. El visitor recibe nodos AST del tipo especificado
3. Se implementa lógica de análisis
4. Se reportan issues al context

### 4.6 Rule Templates y Custom Rules

SonarQube permite crear reglas custom mediante **templates**:

1. **Template**: Define el patrón base (ej: "Method should not exceed X lines")
2. **Custom Rule**: Instancia el template con parámetros específicos

Para custom rules en plugins:
```java
@RuleTemplate
@Description("Number of lines should not exceed ${param} in ${category}")
public class TooManyLinesCheck extends IssuableSubscriptionVisitor {
    
    @Param(defaultValue = "20")
    private int maxLines;
    
    @Param(values = {"METHOD", "FUNCTION", "CONSTRUCTOR"})
    private String category;
    
    // ... implementation
}
```

### 4.7 Quality Profiles

**Concepto**: Conjunto de reglas activas para un lenguaje

**Características**:
- **Inheritance**: Un profile puede heredar de otro
- **Sonar Way**: Profile built-in "recomendado" por SonarSource
- **Per-language**: Cada lenguaje tiene su propio profile
- **Activation**: Puedes activar/desactivar reglas individualmente

**Estructura**:
```
Quality Profile: "Sonar way"
├── Inherits from: (none)
├── Language: TypeScript
├── Rules activated:
│   ├── S1234: Cognitive Complexity > 15
│   ├── S2222: No disabled security checks
│   └── ...
└── Status: DEFAULT
```

### 4.8 Quality Gates

**Quality Gate**: Conjunto de condiciones que un proyecto debe cumplir

**Condiciones Predefinidas**:

| Metric | Operator | Value | On Leonardo |
|--------|----------|-------|-------------|
| Bugs | > | 0 | ✅ Pass |
| Vulnerabilities | > | 0 | ❌ Fail |
| Security Hotspots | > | 0 | ✅ Pass |
| Code Smells | > | 0 | ✅ Pass |
| Coverage | < | 80% | ❌ Fail |
| Duplications | > | 3% | ✅ Pass |
| Security Review | < | 100% | ✅ Pass |

**Evaluación**: El Quality Gate se evalúa en el Compute Engine post-análisis.

---

## 5. Detección de Code Smells

### 5.1 Cognitive Complexity

**Métrica propia de SonarSource** (no confundir con cyclomatic complexity)

**Reglas de puntuación**:
- **Increment +1**: `if`, `else if`, `catch`, `switch`, `case`, `for`, `while`, `do while`, `recursive call`
- **Increment +N** (nesting level): nesting above the initial +1
- **Increment +1**: `&&`, `||`, `catch` with try nested inside, ternary operator
- **No increment**: `else` after `if` with no body, `default` after `case`, `break`, `return`

**Threshold típico**: >15 = Major, >25 = Critical

**Cómo CogniCode puede replicarlo**:
```rust
pub fn cognitive_complexity(node: &SyntaxNode) -> u32 {
    let mut complexity = 0;
    let mut nesting_increment = 0;
    
    for child in node.children() {
        match child.kind() {
            SyntaxKind::IF | SyntaxKind::FOR | SyntaxKind::WHILE => {
                complexity += 1 + nesting_increment;
                nesting_increment += 1;
            }
            SyntaxKind::BINOP if is_logical(&child) => {
                complexity += 1;
            }
            _ => {}
        }
        complexity += cognitive_complexity(&child);
    }
    complexity
}
```

### 5.2 God Classes

**Detection técnica**: Cobertura de Lack of Cohesion of Methods (LCOM)

**Condiciones típicas**:
- >10 methods públicas
- >500 líneas
- LCOM > 0.7

**SonarQube también detecta**:
- God Class pattern con "Data Class" asociada
- Brain Class (clase muy compleja + muchos métodos)

**Cómo CogniCode puede replicarlo**:
- Ya tiene `quality/lcom.rs` para LCOM
- Necesita counting de métodos y líneas

### 5.3 Long Methods

**Threshold típico**: >20 líneas = Major, >40 líneas = Critical

**Técnica de detección**:
```rust
fn analyze_method_length(node: &MethodNode) -> Option<Issue> {
    let line_count = node.end_line() - node.start_line();
    
    if line_count > 40 {
        Some(Issue::critical("Method exceeds 40 lines"))
    } else if line_count > 20 {
        Some(Issue::major("Method exceeds 20 lines"))
    } else {
        None
    }
}
```

### 5.4 Deep Nesting

**Threshold típico**: >3 niveles = Major, >5 niveles = Critical

**Técnica**:
```rust
fn nesting_depth(node: &SyntaxNode) -> u32 {
    let mut max_depth = 0;
    let mut current = 0;
    
    fn visit(node: &SyntaxNode, current: &mut u32, max: &mut u32) {
        if node.is_control_flow() {
            *current += 1;
            *max = (*max).max(*current);
        }
        for child in node.children() {
            visit(child, current, max);
        }
        if node.is_control_flow() {
            *current -= 1;
        }
    }
    
    visit(node, &mut current, &mut max_depth);
    max_depth
}
```

### 5.5 Too Many Parameters

**Threshold típico**: >4 parámetros = Major, >7 = Critical

**Técnica**: AST visitor en function/method declarations

### 5.6 Duplicated Code

**Algoritmo token-based** (no AST-based):
1. Tokenización del código
2. Ignorar comments, whitespace, identifiers variables
3. Crear fingerprint de tokens (>100 tokens necesarios)
4. Comparar fingerprints entre archivos
5. Clustering de duplications

**Critical**: Esto requiere implementación de tokenizer-aware duplication detection

### 5.7 Dead Code

**Detection mediante flow analysis**:
- Variables asignadas pero nunca leídas
- Métodos definidos pero nunca llamados (requiere call graph)
- Clases nunca instanciadas
- Imports/uses nunca utilizados

**CogniCode ya tiene** `find_dead_code` tool usando el call graph

### 5.8 Feature Envy

**Detección**:
1. Para cada método, contar accesos a fields de `this` vs. otras clases
2. Si >50% de accesos son a otra clase → Feature Envy

**Técnica**: AST visitor que tracks field access patterns

### 5.9 Shotgun Surgery

**Detección**: Un change en un requirement requiere modifications en >N archivos

**Técnica**:
1. Usar call graph y dependency graph
2. Medir "fan-out" de cada change semántico
3. Si change single responsibility causa modifications en >5 files → Shotgun Surgery

---

## 6. Security Analysis

### 6.1 SAST en SonarQube

**Tres capas de análisis**:

1. **Pattern Matching (AST-based)**:
   ```java
   // Ejemplo: SQL Injection detection
   if (variable.contains("'")) {
       reportIssue("Potential SQL Injection");
   }
   ```

2. **Taint Analysis (Data Flow)**:
   ```
   SOURCE ──▶ PROPAGATION ──▶ SINK
   user input    string concat   SQL query
   request param   +=            execute()
   cookie         escaping       database call
   ```

3. **Semantic Analysis**:
   - Cryptographic misuse
   - Authentication bypass patterns
   - Authorization logic flaws

### 6.2 Taint Analysis

**Modelo de datos**:
- **Source**: Entry point de datos no confiables
  - `HttpServletRequest.getParameter()`
  - `request.getQueryString()`
  - `System.getenv()`

- **Propagation**: Cómo los datos flows contaminados
  - String concatenation
  - Assignment
  - Method calls

- **Sink**: Punto donde datos pueden causar daño
  - `executeQuery(sql)`
  - `eval(code)`
  - `Runtime.exec(cmd)`

**Enterprise+ Feature**: Cross-library y cross-file deep analysis

### 6.3 Security Hotspots

**Concepto**: Código que requiere revisión humana, no es necesariamente un problema

**Ejemplos**:
- Uso de `eval()`
- Hard-coded credentials
- Cryptographic weak algorithms
- File I/O operations

**Flujo**:
1. Se detecta Hotspot
2. Security reviewer lo marca como "Reviewed - Safe" o "Reviewed - Risk"
3. Si Risk → se convierte en Vulnerability

### 6.4 Security Categories

SonarQube mapea a estándares conocidos:
- **CWE**: Common Weakness Enumeration
- **OWASP Top 10**: Web application risks
- **CERT**: Secure coding standards
- **CIS**: Center for Internet Security

---

## 7. Métricas y Ratings

### 7.1 Sistema de Ratings (A-E)

**Maintainability Rating** (Technical Debt):

| Rating | SQALE Index | Interpretation |
|--------|-------------|---------------|
| A | 0-0.05 | Excellent |
| B | 0.05-0.1 | Very good |
| C | 0.1-0.2 | Technical debt ratio < 5% |
| D | 0.2-0.5 | Technical debt ratio 5-10% |
| E | >0.5 | Technical debt ratio > 10% |

**Security Rating**:

| Rating | Vulnerabilities |
|--------|----------------|
| A | 0 Vulnerabilities |
| B | 0 blocker, 0 critical, < N major |
| C | 0 blocker, < N critical, < M major |
| D | < N critical, < M major |
| E | ≥ N critical OR ≥ M major |

**Reliability Rating**: Similar matrix para Bugs

### 7.2 Technical Debt - SQALE Method

**SQALE = Software Quality Assessment based on Lifecycle Expectations**

```
Technical Debt = Σ (Remediation Effort_i) / (Cost to Fix One Line)

Remediation Effort = Time to fix × Complexity Factor × Environment Factor
```

**Ejemplo**:
- 100 líneas duplicadas × 5 min/line = 500 min remediation effort
- Cost per line = $25
- Technical Debt = $12,500

### 7.3 Métricas Catalog

| Category | Metrics |
|----------|---------|
| **Size** | Lines of Code, Lines of Comments, Classes, Functions |
| **Complexity** | Cyclomatic, Cognitive, Depth of Nesting |
| **Coverage** | Line Coverage, Branch Coverage, Condition Coverage |
| **Duplications** | Duplicated Lines, Duplicated Blocks, Token Similarity |
| **Issues** | Bugs, Vulnerabilities, Code Smells, Security Hotspots |
| **Maintainability** | Technical Debt Ratio, SQALE Index, Code Smell Density |
| **Reliability** | Bugs Count, Reliability Rating |
| **Security** | Vulnerabilities Count, Security Hotspots, Security Rating |

---

## 8. API y Capacidades de Integración

### 8.1 Web API Endpoints

**Base URL**: `http://localhost:9000/api/`

| Endpoint | Methods | Description |
|----------|---------|-------------|
| `/measures/component` | GET | Get measures for a component |
| `/measures/search` | GET | Search for measure definitions |
| `/issues/search` | GET | Search issues with filters |
| `/issues/bulk` | POST | Bulk issue actions |
| `/rules/search` | GET | Search rules |
| `/rules/show` | GET | Get rule details |
| `/qualitygates/project_status` | GET | Get quality gate status |
| `/qualitygates/select` | POST | Set quality gate for project |
| `/qualityprofiles/search` | GET | Search quality profiles |
| `/sources/show` | GET | Get source code with issues |
| `/duplications/show` | GET | Get duplication details |

### 8.2 Push Integrations

**Generic Issue Data**:
```json
{
  "issues": [
    {
      "engineId": "my-linter",
      "ruleId": "my-rule-001",
      "severity": "MAJOR",
      "type": "CODE_SMELL",
      "message": "Consider using struct instead of class",
      "line": 42,
      "file": "src/main.rs"
    }
  ]
}
```

**SARIF Support** (Security Assertions and Results XML):
```json
{
  "version": "2.1.0",
  "runs": [{
    "tool": {
      "driver": {
        "name": "cognicode-axiom",
        "version": "1.0.0"
      }
    },
    "results": [
      {
        "ruleId": "RustLint/use-struct",
        "level": "warning",
        "message": { "text": "Consider using struct instead of class" },
        "locations": [{
          "physicalLocation": {
            "artifactLocation": { "uri": "src/main.rs" },
            "region": { "startLine": 42 }
          }
        }]
      }
    ]
  }]
}
```

### 8.3 Pull Integrations

**Ejemplo: Get rules de SonarQube**:
```bash
curl -u token: "http://localhost:9000/api/rules/search?languages=rust&tags=brain-overload"
```

**Respuesta**:
```json
{
  "total": 15,
  "rules": [
    {
      "key": "rust:S1234",
      "name": "Cognitive Complexity",
      "severity": "MAJOR",
      "tags": ["brain-overload"],
      "params": [
        { "key": "max", "defaultValue": "15" }
      ]
    }
  ]
}
```

### 8.4 SonarQube MCP Server Toolsets (Completo)

```json
// Analysis tools
{ "tool": "execute_analysis", "args": { "project": "my-project", "branch": "main" } }
{ "tool": "get_analysis_status", "args": { "analysisId": "abc123" } }

// Issues tools
{ "tool": "search_issues", "args": { "project": "my-project", "severity": "MAJOR" } }
{ "tool": "create_issue", "args": { "project": "my-project", "rule": "S1234", "line": 42 } }
{ "tool": "add_comment", "args": { "issueKey": "ABC-123", "comment": "Fixed in PR #42" } }

// Quality Gates
{ "tool": "evaluate_quality_gate", "args": { "project": "my-project" } }
{ "tool": "get_project_status", "args": { "project": "my-project" } }

// Rules
{ "tool": "search_rules", "args": { "language": "rust", "tags": ["security"] } }
{ "tool": "get_rule", "args": { "ruleKey": "S1234" } }

// Measures
{ "tool": "get_measures", "args": { "component": "my-project", "metrics": "complexity,coverage" } }

// Security Hotspots
{ "tool": "search_hotspots", "args": { "project": "my-project" } }
{ "tool": "update_hotspot_review", "args": { "hotspotKey": "HS-123", "status": "REVIEWED" } }
```

---

## 9. Evaluación Honesta: Replicar vs Integrar

### 9.1 Features donde Replicar tiene Sentido

| Feature | Why Replicate | Effort | Risk |
|---------|---------------|--------|------|
| **Rules Engine (basic)** | Custom DSL para AI governance | MEDIUM | Low |
| **Code Smell Detection (basic)** | Long method, deep nesting | LOW | Low |
| **Quality Gates (simple)** | Threshold-based config | LOW | Low |
| **Dead Code Detection** | Ya tenemos call graph base | LOW | Very Low |

### 9.2 Features donde Integrar es Mejor

| Feature | Why Integrate | Integration Method |
|---------|---------------|-------------------|
| **SAST/Security Analysis** | Expertise de 15+ años | MCP Server or SARIF |
| **Duplications Detection** | Algoritmo token-based complejo | MCP Server |
| **Security Hotspots** | Workflow de revisión humana | MCP Server |
| **Quality Profiles** | Ecosistema de reglas maduro | API |
| **Technical Debt (SQALE)** | Metodología completa | API |

### 9.3 CogniCode Advantages (Where we WIN)

1. **Real-time Analysis**: Sin commit, sin push, directamente sobre working directory
2. **No JVM Overhead**: Rust-native, startup instantáneo
3. **AI Agent Integration**: Diseñado para AI agents desde día 1
4. **Entropy/SOLID Analysis**: Connascence metrics, información bottleneck
5. **Chronos Integration**: Runtime behavior analysis, crash investigation
6. **SDD Workflow**: Spec-driven development con delta specs
7. **Cedar/OPA Policy**: Governance declarativo para AI agents

### 9.4 Three Integration Strategies

#### Strategy A: Standalone (Build Everything in Rust)

**Pros**:
- Control total
- No dependencies externas
- 100% Rust ecosystem

**Cons**:
- **Effort: EXTREME** (5-10 años para igualar)
- Mantenimiento de parsers, rules, detection algorithms
- Security updates (vulnerabilidades conocidas)

**Verdict**: ❌ No recomendado

#### Strategy B: Integration Only (Use SonarQube API/MCP)

**Pros**:
- Acceso instantáneo a todas las features de SonarQube
- Security updates handled por SonarSource
- Ecosistema maduro

**Cons**:
- Requiere SonarQube server (Docker o instalar)
- No real-time (post-commit analysis)
- JVM required para server
- AI agent experience menos fluida

**Verdict**: ⚠️ Viable pero incompleto

#### Strategy C: Hybrid (RECOMMENDED)

**CogniCode hace lo que hace bien + SonarQube como backend de enterprise**

```
┌─────────────────────────────────────────────────────────────────┐
│                     COGNICODE-AXIOM                             │
│                                                                 │
│  ┌──────────────────┐         ┌──────────────────────────────┐ │
│  │  REAL-TIME ENGINE │         │    ENTERPRISE BACKEND       │ │
│  │                  │         │                              │ │
│  │  • Call Graph     │         │  SonarQube MCP Server       │ │
│  │  • Complexity     │◄───────►│  (Docker container)         │ │
│  │  • Dead Code      │  SARIF  │                              │ │
│  │  • Impact Analysis │         │  • Security Analysis        │ │
│  │  • SOLID/Entropy   │         │  • Duplications             │ │
│  │  • Quality Gates   │         │  • Security Hotspots         │ │
│  │    (simple)        │         │  • Quality Profiles         │ │
│  │                    │         │  • Technical Debt           │ │
│  │  MCP Tools (32+)   │         │                              │ │
│  └──────────────────┘         └──────────────────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Pros**:
- Best of both worlds
- CogniCode: real-time, agentic, lightweight
- SonarQube: security, quality, enterprise
- SARIF como bridge

**Cons**:
- Requiere dos sistemas
- Integración puede ser complex

**Verdict**: ✅ Recomendado

### 9.5 Technical Viability Table

| Component | Standalone | Integration | Hybrid | Recommended |
|-----------|------------|-------------|--------|-------------|
| Call Graph | ✅ | ❌ | ✅ CogniCode | CogniCode |
| Complexity | ✅ | ❌ | ✅ CogniCode | CogniCode |
| Dead Code | ✅ | ❌ | ✅ CogniCode | CogniCode |
| SOLID/Entropy | ✅ | ❌ | ✅ CogniCode | CogniCode |
| Code Smells (basic) | ✅ | ❌ | ✅ CogniCode | CogniCode |
| Code Smells (advanced) | ⚠️ Medium | ✅ | ✅ SonarQube | SonarQube |
| Security Analysis | ⚠️ Hard | ✅ | ✅ SonarQube | SonarQube |
| Duplications | ⚠️ Hard | ✅ | ✅ SonarQube | SonarQube |
| Quality Gates | ✅ | ✅ | ✅ Both | Hybrid |
| Quality Profiles | ⚠️ Medium | ✅ | ✅ SonarQube | SonarQube |
| Technical Debt | ⚠️ Medium | ✅ | ✅ SonarQube | SonarQube |
| Security Hotspots | ❌ | ✅ | ✅ SonarQube | SonarQube |

---

## 10. Recomendaciones para cognicode-axiom

### Phase 1: Rules Engine MVP (Semanas 1-4)

**Objetivo**: Construir base para policy-driven analysis

**Tasks**:
1. [ ] Diseñar Rule trait en `axiom/rules/`
2. [ ] Implementar ADR-based rule definitions
3. [ ] Crear Tree-sitter visitor para rule execution
4. [ ] Soporte para severity y tags
5. [ ] Basic Quality Gate evaluation (threshold-based)

**Estructura propuesta**:
```rust
// axiom/src/rules/engine.rs
pub trait Rule: Send + Sync {
    fn key(&self) -> &str;
    fn name(&self) -> &str;
    fn severity(&self) -> Severity;
    fn execute(&self, ctx: &RuleContext) -> Vec<Issue>;
}

pub struct RuleContext<'a> {
    pub source_file: &'a SourceFile,
    pub ast: &'a SyntaxTree,
    pub call_graph: Option<&'a CallGraph>,
}

pub trait Issue {
    fn rule_key(&self) -> &str;
    fn message(&self) -> &str;
    fn location(&self) -> SourceRange;
    fn severity(&self) -> Severity;
}
```

### Phase 2: Code Smell Detection (Semanas 5-8)

**Objetivo**: Implementar detection de code smells básicos

**Rules a implementar**:

| Rule | Technique | Priority |
|------|-----------|----------|
| Long Method | AST visitor, line counting | HIGH |
| Deep Nesting | Recursive depth calculation | HIGH |
| Cognitive Complexity | Custom algorithm (see Section 5.1) | HIGH |
| Too Many Parameters | AST visitor on function signatures | MEDIUM |
| God Class | LCOM calculation (ya existe en axiom) | MEDIUM |
| Dead Code | Call graph analysis (ya existe) | HIGH |

**Arquitectura**:
```
┌─────────────────────────────────────────────────────┐
│              RULE REGISTRY                          │
│                                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌───────────┐ │
│  │ LongMethod  │  │DeepNesting  │  │Cognitive  │ │
│  │   Rule      │  │   Rule      │  │Complexity │ │
│  └──────┬──────┘  └──────┬──────┘  └─────┬─────┘ │
│         │                 │               │        │
│         └─────────────────┼───────────────┘        │
│                           │                         │
│                    ┌──────▼──────┐                  │
│                    │   RULE      │                  │
│                    │   ENGINE    │                  │
│                    │ (Tree-sitter│                  │
│                    │  Visitors)  │                  │
│                    └─────────────┘                  │
└─────────────────────────────────────────────────────┘
```

### Phase 3: SonarQube Integration (Semanas 9-12)

**Objetivo**: Integración con SonarQube via MCP y SARIF

**Tasks**:

1. **SARIF Output Support**:
   ```rust
   pub fn export_as_sarif(issues: Vec<Issue>) -> SarifReport {
       // Convert issues to SARIF 2.1.0 format
   }
   ```

2. **SonarQube MCP Client**:
   ```rust
   pub struct SonarQubeMcpClient {
       // Connect to mcp/sonarqube
       // Pull security issues
       // Push cognicode findings
   }
   ```

3. **Quality Profile Synchronization**:
   - Import rules from SonarQube as inspiration
   - Map SonarQube rule keys to cognicode rule keys

**Integración con SonarQube MCP**:
```rust
// En axiom/src/mcp/sonarqube.rs
pub async fn fetch_sonarqube_issues(
    client: &SonarQubeMcpClient,
    project: &str,
) -> Result<Vec<Issue>> {
    let security_issues = client.search_issues(
        IssueQuery::new()
            .project(project)
            .type_("VULNERABILITY")
    ).await?;
    
    let hotspots = client.search_hotspots(
        HotspotQuery::new()
            .project(project)
    ).await?;
    
    Ok(convert_to_issues(security_issues, hotspots))
}
```

### 10.1 Importar Reglas desde SonarQube

**Source**: https://rules.sonarsource.com

**Approach**:
1. Fetch rules via API: `GET /api/rules/search?languages=rust`
2. Parse rule definitions
3. Generate cognicode rule stubs
4. Implement detection logic

**Ejemplo de script**:
```bash
# Fetch Rust rules from SonarQube
curl -s -u "$SONAR_TOKEN:" \
  "https://sonarcloud.io/api/rules/search?languages=rust&ps=500" \
  | jq '.rules[] | {key, name, severity, description}'
```

### 10.2 SARIF como Formato de Interchange

**Por qué SARIF**:
- Estándar OASIS abierto
- Soportado por GitHub, GitLab, Azure DevOps
- SonarQube puede importar SARIF
- cognicode-axiom puede exportar SARIF

**Flujo**:
```
cognicode-axiom              SonarQube
    │                            │
    │  ──SARIF──▶               │
    │  (export)     SonarQube    │
    │               MCP Server    │
    │                            │
    │  ◀──Security Issues──     │
    │  (pull)        (mcp/sonarqube) │
    │                            │
```

---

## 11. Entropy Analysis

### 11.1 Connascence between Components

**Concepto de Connascence** (ver `quality/connascence.rs` en cognicode-axiom):

| Type | Description | Severity |
|------|-------------|----------|
| Co | Connascence of Name | Low |
| Cm | Connascence of Meaning | Medium |
| Cl | Connascence of Location | Medium |
| Cv | Connascence of Value | High |
| Ce | Connascence of Execution | High |
| Ct | Connascence of Timing | Very High |
| Cp | Connascence of Position | Very High |
| Cr | Connascence of Reference | Critical |

### 11.2 Tree-sitter AST ↔ Rules Visitors: Critical Coupling Point

**El punto crítico de acoplamiento**:

```
┌─────────────────────────────────────────────────────────────┐
│                    CRITICAL COUPLING                        │
│                                                             │
│   Tree-sitter AST                Rules Engine               │
│   Parser                         Visitors                  │
│       │                              │                     │
│       │    ┌─────────────────────┐   │                     │
│       └───►│  SYNTAX NODE TREE   │◄──┘                     │
│            │                     │                          │
│            │  • function_def     │  ◄─── coupling point     │
│            │  • parameter_list   │                          │
│            │  • block             │                          │
│            │  • if_statement      │                          │
│            │  • ...               │                          │
│            └─────────────────────┘                          │
│                                                             │
│  Coupling occurs because:                                   │
│  1. Rules depend on specific node types                    │
│  2. Changes to AST schema break rules                      │
│  3. Different languages have different node kinds          │
└─────────────────────────────────────────────────────────────┘
```

**H_external Estimation** (Entropía de acoplamiento externo):

```
H_external = Σ (node_type_dependencies × change_frequency)

Where:
- node_type_dependencies = number of rules depending on a node type
- change_frequency = how often that node type changes across versions
```

**Minimizar H_external**:
1. **Abstraction layer**: Crear `RuleContext` que aísla AST details
2. **Visitor pattern**: Cada rule implementa su propio visitor
3. **Language-agnostic rules**: Rules that work across languages

### 11.3 H_external para SonarQube Integration

**Al integrar con SonarQube MCP**:

```
┌─────────────────────────────────────────────────────────────┐
│                 SONARQUBE INTEGRATION                        │
│                                                             │
│   cognicode-axiom              SonarQube                     │
│       │                          │                          │
│       │   MCP Protocol           │                          │
│       │   (mcp/sonarqube)        │                          │
│       │                          │                          │
│       │  ──Request──▶            │                          │
│       │  ◀──Response──           │                          │
│       │                          │                          │
│   Impact:                                               │
│   - API coupling (H_api)                               │
│   - Schema coupling (H_schema)                          │
│   - Latency coupling (H_latency)                        │
│                                                             │
│   H_external_total = H_api + H_schema + H_latency        │
│                    = MEDIUM (acceptable)                   │
└─────────────────────────────────────────────────────────────┘
```

---

## 12. Conclusiones

### 12.1 Descubrimiento Clave

SonarSource ha publicado un **MCP Server oficial para SonarQube**. Esto cambia fundamentalmente nuestra estrategia de "build vs integrate".

### 12.2 Estrategia Recomendada: Hybrid

**CogniCode hace lo que hace bien**:
- Real-time analysis
- Agentic workflow
- SOLID/Entropy analysis
- Chronos integration
- SDD governance

**SonarQube como backend enterprise**:
- Security analysis (SAST, taint)
- Duplications detection
- Security Hotspots
- Quality Profiles/Gates
- Technical Debt

### 12.3 Roadmap Suggested

| Phase | Time | Goal |
|-------|------|------|
| Phase 1 | Weeks 1-4 | Rules Engine MVP with tree-sitter visitors |
| Phase 2 | Weeks 5-8 | Basic code smell detection (long method, deep nesting, cognitive) |
| Phase 3 | Weeks 9-12 | SonarQube MCP integration (SARIF, API pull) |

### 12.4 Open Questions

1. **Licensing**: ¿SonarQube MCP Server tiene las mismas restricciones de licencia que SonarQube?
2. **Deployment**: ¿Requiere SonarQube server corriendo o puede usar SonarCloud?
3. **Real-time**: ¿El MCP Server soporta webhooks para real-time, o es siempre post-commit?

---

## Anexo: Recursos

- SonarQube MCP Server: https://github.com/SonarSource/sonarqube-mcp-server
- SonarQube Web API: https://docs.sonarqube.org/latest/extend/web-api/
- Rules Database: https://rules.sonarsource.com
- SARIF Specification: https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html
- cognicode-axiom: `/home/rubentxu/Proyectos/rust/CogniCode/crates/cognicode-axiom/`

---

*Documento generado como parte del análisis SDD para cognicode-axiom governance capabilities.*
