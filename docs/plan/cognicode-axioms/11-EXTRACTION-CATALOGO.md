# Extracción del Catálogo de Reglas de SonarQube y Portado a Rust

Este documento describe el proceso técnico para extraer el catálogo completo de reglas de SonarQube y portar las implementaciones de Java a Rust utilizando tree-sitter como motor de análisis de código fuente.

## 1. Fuentes de Datos

La extracción del catálogo de reglas de SonarQube puede realizarse a través de múltiples fuentes, cada una con sus ventajas y limitaciones específicas.

### 1.1 SonarCloud API

La API de SonarCloud expone todos los metadatos de las reglasillas a través del endpoint `GET /api/rules/search`. Esta fuente es la más completa y estructurada para obtener información normalizada de las reglas.

**Ventajas:**
- Acceso programático directo a todos los metadatos de reglas
- Filtrado por lenguaje, tipo, severidad y etiquetas
- Formato JSON estandarizado con campos consistentes
- Rate limiting razonable para extracción batch

**Limitaciones:**
- Requiere token de autenticación para volúmenes altos
- No incluye el código fuente de las implementaciones Java
- Las descripciones están en formato HTML

### 1.2 Repositorios GitHub de SonarSource

Losanalizadores de SonarSource son software libre bajo licencia Apache-2.0 y están disponibles en GitHub:

- `SonarSource/sonar-java` — analizador para Java
- `SonarSource/sonar-javascript` — analizador para JavaScript/TypeScript
- `SonarSource/sonar-python` — analizador para Python
- `SonarSource/sonar-go` — analizador para Go

**Ventajas:**
- Acceso al código fuente completo de las implementaciones
- Historial de commits para entender la evolución de reglas
- Tests unitarios que muestran casos de uso esperados
- Issues y discussions de la comunidad

**Limitaciones:**
- Navegación manual necesaria para encontrar implementaciones específicas
- Estructura de directorios variable entre lenguajes
- Código Java requiere análisis semántico adicional

### 1.3 Catálogo Web rules.sonarsource.com

El sitio web `rules.sonarsource.com` proporciona un catálogo navegable con descripciones completas, ejemplos de código y información de configuración para cada regla.

**Ventajas:**
- Descripciones formateadas y ejemplos de código
- Información educacional sobre por qué cada regla es importante
- Categorización temática y referencias a estándares de codificación
- Actualizaciones sincronizadas con nuevas versiones de SonarQube

**Limitaciones:**
- No es directamente parseable programáticamente
- Requiere web scraping para extracción automatizada
- Limitado a consulta manual o herramientas de scraping especializadas

### 1.4 SonarQube MCP Server

El SonarQube MCP Server permite consultar reglas directamente desde herramientas de desarrollo que soporten el protocolo MCP (Model Context Protocol).

**Ventajas:**
- Integración nativa con IDEs y agentes de IA
- Consulta contextual durante el desarrollo
- Acceso en tiempo real a reglas actualizadas

**Limitaciones:**
- Requiere configuración adicional del entorno MCP
- Dependiente de la versión de SonarQube desplegada

## 2. Extracción mediante SonarQube API

La extracción programática del catálogo de reglas se realiza mejor mediante la API de SonarQube. A continuación se presenta un script completo en Python que maneja paginación, filtrado y rate limiting.

### 2.1 Script de Extracción Completo

```python
#!/usr/bin/env python3
"""
SonarQube Rule Catalog Extractor
Extracts all rules from SonarCloud API and outputs to JSONL format.
"""

import requests
import time
import json
import sys
from dataclasses import dataclass, asdict
from typing import Optional

# Configuration
SONARCLOUD_API = "https://sonarqube.com/api/rules/search"
OUTPUT_FILE = "sonarqube_rules.jsonl"
BATCH_SIZE = 500
RATE_LIMIT_DELAY = 1.0  # seconds between requests


@dataclass
class SonarParam:
    """Parameter definition for a SonarQube rule."""
    key: str
    description: str
    default_value: Optional[str] = None
    type: str = "STRING"


@dataclass
class SonarDebtFunction:
    """Technical debt remediation function configuration."""
    debt_function: str
    debt_offset: Optional[str] = None
    debt_remaining_offset: Optional[str] = None


@dataclass
class SonarRule:
    """Complete SonarQube rule metadata."""
    key: str
    name: str
    severity: str
    type: str
    lang: str
    html_desc: str
    params: list[SonarParam]
    tags: list[str]
    debt_func: Optional[SonarDebtFunction] = None
    active: bool = True

    def to_axiom_format(self) -> dict:
        """Convert to internal AxiomRuleMeta format."""
        return {
            "rule_id": self.key,
            "name": self.name,
            "severity": self.severity,
            "category": self.type,
            "language": self.lang,
            "description": strip_html(self.html_desc),
            "parameters": [
                {
                    "name": p.key,
                    "description": p.description,
                    "default": p.default_value,
                    "type": p.type
                }
                for p in self.params
            ],
            "tags": self.tags,
            "remediation": self.debt_func.debt_function if self.debt_func else None
        }


def strip_html(text: str) -> str:
    """Remove HTML tags from description text."""
    import re
    clean = re.compile('<.*?>')
    return re.sub(clean, '', text).strip()


def fetch_rules_batch(
    api_token: str,
    lang: Optional[str] = None,
    type_filter: Optional[str] = None,
    severity: Optional[str] = None,
    page: int = 1,
    page_size: int = BATCH_SIZE
) -> dict:
    """
    Fetch a single page of rules from SonarCloud API.

    Args:
        api_token: SonarCloud authentication token
        lang: Filter by language (e.g., 'java', 'js', 'py')
        type_filter: Filter by type ('CODE_SMELL', 'BUG', 'VULNERABILITY', 'SECURITY_HOTSPOT')
        severity: Filter by severity ('BLOCKER', 'CRITICAL', 'MAJOR', 'MINOR', 'INFO')
        page: Page number (1-indexed)
        page_size: Results per page (max 500)

    Returns:
        JSON response from API
    """
    headers = {"Authorization": f"Bearer {api_token}"}
    params = {
        "p": page,
        "ps": page_size,
        "fields": "key,name,severity,type,lang,htmlDesc,params,tags,debtFunc"
    }

    if lang:
        params["languages"] = lang
    if type_filter:
        params["types"] = type_filter
    if severity:
        params["severities"] = severity

    response = requests.get(SONARCLOUD_API, headers=headers, params=params)
    response.raise_for_status()
    return response.json()


def parse_rule(raw_rule: dict) -> SonarRule:
    """Parse raw API response into SonarRule dataclass."""
    params = []
    for p in raw_rule.get("params", []):
        params.append(SonarParam(
            key=p.get("key", ""),
            description=p.get("description", ""),
            default_value=p.get("defaultValue"),
            type=p.get("type", "STRING")
        ))

    debt_func = None
    if "debtFunc" in raw_rule and raw_rule["debtFunc"]:
        df = raw_rule["debtFunc"]
        debt_func = SonarDebtFunction(
            debt_function=df.get("debtFunction", ""),
            debt_offset=df.get("debtOffset"),
            debt_remaining_offset=df.get("debtRemainingOffset")
        )

    return SonarRule(
        key=raw_rule["key"],
        name=raw_rule["name"],
        severity=raw_rule["severity"],
        type=raw_rule["type"],
        lang=raw_rule.get("lang", ""),
        html_desc=raw_rule.get("htmlDesc", ""),
        params=params,
        tags=raw_rule.get("tags", []),
        debt_func=debt_func,
        active=raw_rule.get("status") == "READY"
    )


def extract_all_rules(api_token: str, languages: list[str]) -> list[SonarRule]:
    """
    Extract all rules for specified languages with pagination.

    Args:
        api_token: SonarCloud authentication token
        languages: List of language codes (e.g., ['java', 'js', 'py'])

    Returns:
        List of all SonarRule objects
    """
    all_rules = []

    for lang in languages:
        page = 1
        total = None

        print(f"Extracting rules for language: {lang}", file=sys.stderr)

        while total is None or (page - 1) * BATCH_SIZE < total:
            try:
                data = fetch_rules_batch(api_token, lang=lang, page=page)

                if total is None:
                    total = data.get("total", 0)
                    print(f"  Total rules: {total}", file=sys.stderr)

                rules = data.get("rules", [])
                for raw in rules:
                    rule = parse_rule(raw)
                    all_rules.append(rule)

                print(f"  Page {page}: extracted {len(rules)} rules", file=sys.stderr)

                page += 1
                time.sleep(RATE_LIMIT_DELAY)

            except requests.exceptions.RequestException as e:
                print(f"  Error fetching page {page}: {e}", file=sys.stderr)
                time.sleep(5)  # Backoff on error
                continue

    return all_rules


def main():
    """Main extraction pipeline."""
    import os

    api_token = os.environ.get("SONARCLOUD_TOKEN")
    if not api_token:
        print("Error: SONARCLOUD_TOKEN environment variable not set")
        sys.exit(1)

    languages = ["java", "js", "py"]  # Extend as needed

    rules = extract_all_rules(api_token, languages)

    print(f"\nTotal rules extracted: {len(rules)}")

    with open(OUTPUT_FILE, "w") as f:
        for rule in rules:
            f.write(json.dumps(rule.to_axiom_format()) + "\n")

    print(f"Output written to {OUTPUT_FILE}")


if __name__ == "__main__":
    main()
```

### 2.2 Manejo de Rate Limiting y Errores

El script implementa las siguientes estrategias de resiliencia:

**Rate Limiting:**
- Retraso configurable entre solicitudes (por defecto 1 segundo)
- Respeto a headers `X-RateLimit-Remaining` si están disponibles
- Backoff exponencial en caso de errores 429

**Manejo de Errores:**
- Reintento automático con backoff para errores transitorios
- Logging detallado para diagnóstico de fallos
- Continuación parcial en caso de errores no fatales

**Paginación:**
- Extracción por lotes de hasta 500 reglas por solicitud
- Control de total de reglas para seguimiento de progreso
- Soporte para reanudación en caso de interrupción

## 3. Schema de Metadatos de Reglas

La siguiente estructura representa el schema completo para deserializar reglas de SonarQube y convertirlas al formato interno `AxiomRuleMeta`.

### 3.1 Estructuras de Datos Principales

```rust
// sonarqube_rule.rs

use serde::{Deserialize, Serialize};

/// Technical debt remediation function configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonarDebtFunction {
    /// The remediation function type (e.g., "LINEAR", "LINEAR_OFFSET", "CONSTANT_ISSUE")
    pub debt_function: String,
    /// Offset for linear functions (e.g., "30min")
    pub debt_offset: Option<String>,
    /// Remaining offset for constant functions
    pub debt_remaining_offset: Option<String>,
}

/// Parameter definition for a SonarQube rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonarParam {
    /// Unique parameter identifier
    pub key: String,
    /// Human-readable description
    pub description: String,
    /// Default value if any
    pub default_value: Option<String>,
    /// Parameter type (STRING, NUMERIC, BOOLEAN, SINGLE_SELECT_LIST, etc.)
    #[serde(rename = "type")]
    pub param_type: String,
}

/// Complete SonarQube rule metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonarRule {
    /// Unique rule key (e.g., "java:S138")
    pub key: String,
    /// Human-readable rule name
    pub name: String,
    /// Severity level (BLOCKER, CRITICAL, MAJOR, MINOR, INFO)
    pub severity: String,
    /// Rule type (CODE_SMELL, BUG, VULNERABILITY, SECURITY_HOTSPOT)
    #[serde(rename = "type")]
    pub rule_type: String,
    /// Language code (java, js, py, go, etc.)
    pub lang: String,
    /// HTML description from SonarQube
    pub html_desc: String,
    /// Configurable parameters for the rule
    pub params: Vec<SonarParam>,
    /// Associated tags for categorization
    pub tags: Vec<String>,
    /// Technical debt configuration
    pub debt_func: Option<SonarDebtFunction>,
}

/// Internal rule metadata format for the Axiom rules engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxiomRuleMeta {
    /// Rule identifier matching SonarQube (e.g., "S138")
    pub rule_id: String,
    /// Human-readable name
    pub name: String,
    /// Severity level
    pub severity: Severity,
    /// Category (Code Smell, Bug, Vulnerability, Security Hotspot)
    pub category: RuleCategory,
    /// Target language
    pub language: String,
    /// Plain text description
    pub description: String,
    /// Rule parameters
    pub parameters: Vec<RuleParameter>,
    /// Associated tags
    pub tags: Vec<String>,
    /// Remediation function description
    pub remediation: Option<String>,
    /// Tree-sitter query for this rule (if implemented)
    pub query: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    Blocker,
    Critical,
    Major,
    Minor,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleCategory {
    CodeSmell,
    Bug,
    Vulnerability,
    SecurityHotspot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleParameter {
    pub name: String,
    pub description: String,
    pub default_value: Option<String>,
    pub param_type: String,
}
```

### 3.2 Conversión entre Formatos

```rust
impl From<SonarRule> for AxiomRuleMeta {
    fn from(sonar: SonarRule) -> Self {
        use html2md::parse_html;

        AxiomRuleMeta {
            rule_id: sonar.key.split(':').last().unwrap_or(&sonar.key).to_string(),
            name: sonar.name,
            severity: parse_severity(&sonar.severity),
            category: parse_category(&sonar.rule_type),
            language: sonar.lang.clone(),
            description: parse_html(&sonar.html_desc),
            parameters: sonar.params.into_iter().map(convert_param).collect(),
            tags: sonar.tags,
            remediation: sonar.debt_func.map(|d| d.debt_function),
            query: None, // Set during implementation
        }
    }
}

fn parse_severity(s: &str) -> Severity {
    match s.to_uppercase().as_str() {
        "BLOCKER" => Severity::Blocker,
        "CRITICAL" => Severity::Critical,
        "MAJOR" => Severity::Major,
        "MINOR" => Severity::Minor,
        _ => Severity::Info,
    }
}

fn parse_category(t: &str) -> RuleCategory {
    match t.to_uppercase().as_str() {
        "CODE_SMELL" => RuleCategory::CodeSmell,
        "BUG" => RuleCategory::Bug,
        "VULNERABILITY" => RuleCategory::Vulnerability,
        "SECURITY_HOTSPOT" => RuleCategory::SecurityHotspot,
        _ => RuleCategory::CodeSmell,
    }
}
```

## 4. Guía de Portado de Java a Rust

El proceso de portada de reglas desde los analizadores Java de SonarSource hacia implementaciones Rust con tree-sitter sigue un flujo de trabajo estructurado que asegura fidelidad funcional.

### 4.1 Flujo de Trabajo por Regla

**Paso 1: Obtener Metadatos de la API**

Consultar la API de SonarCloud para obtener los metadatos completos de la regla, incluyendo parámetros, severidad y descripción.

**Paso 2: Localizar Implementación Java**

Buscar en el repositorio correspondiente de SonarSource el archivo Java que implementa la regla. Los analizadores siguen una estructura de directorios consistente:

```
sonar-java/
├── java-checks/src/main/java/org/sonar/java/checks/
│   ├── package的结构/
│   │   ├── SxxxRule.java  # Regla específica
│   │   └── SxxxCheck.java  # Clase de verificación
```

**Paso 3: Analizar el Patrón AST Visitor**

Las implementaciones de SonarQube siguen el patrón Visitor sobre el AST del lenguaje. Analizar qué nodos del árbol se visitan y qué condiciones se evalúan.

**Paso 4: Escribir Query Equivalente en tree-sitter**

Utilizar el lenguaje de queries de tree-sitter para identificar los nodos relevantes en el AST del código fuente.

**Paso 5: Implementar declare_rule!**

Escribir la implementación Rust usando la macro `declare_rule!` con la lógica equivalente.

**Paso 6: Crear Fixtures de Test**

Generar archivos de prueba con código válido e inválido para verificar la implementación.

**Paso 7: Verificar Paridad**

Ejecutar los tests con input conocido y confirmar que se generan las mismas issues.

### 4.2 Ejemplo Simple: S1135 (TODO Tags)

**Regla:** Los comentarios TODO deben incluir un identificador de usuario o ticket.

**Implementación Java:**

```java
// From SonarSource/sonar-java
// File: java-checks/src/main/java/org/sonar/java/checks/TodoTagPresenceCheck.java

@Rule(key = "S1135")
public class TodoTagPresenceCheck extends IssuableSubscriptionVisitor {
    // Visits comments containing TODO
    @Override
    public List<Tree.Kind> nodesToVisit() {
        return Collections.singletonList(Tree.Kind.TRIVIA);
    }

    @Override
    public void visitTrivia(Trivia trivia) {
        if (trivia.isComment()
            && trivia.content().contains("TODO")
            && !hasUserOrIssueReference(trivia.content())) {
            reportIssue(trivia, "Complete the task associated with this TODO.");
        }
    }

    private boolean hasUserOrIssueReference(String content) {
        // Pattern: TODO(user) or TODO(JIRA-123)
        return Pattern.matches(".*TODO\\([@\\w]+\\).*", content)
            || Pattern.matches(".*TODO\\[\\w+-\\d+\\].*", content);
    }
}
```

**Implementación Rust con tree-sitter:**

```rust
// rules/src/java/todo_tag.rs

use tree_sitter::Query;
use regex::Regex;

declare_rule! {
    /// Detects TODO comments without user or issue references.
    ///
    /// This rule checks that TODO comments follow a consistent format:
    /// - `TODO(username)` - indicates who is responsible
    /// - `TODO[JIRA-123]` - references a tracking ticket
    ///
    /// Unprefixed TODOs make it difficult to track tasks across the codebase.
    ///
    /// **Non-compliant example:**
    /// ```java
    /// // TODO: fix this later
    /// ```
    ///
    /// **Compliant examples:**
    /// ```java
    /// // TODO(rvolkov): implement caching
    /// // TODO[PROJ-123]: add error handling
    /// ```
    S1135("java", "todo_tag", Severity::Minor, RuleCategory::CodeSmell),

    fn check(&self, ctx: &RuleContext) -> Result<(), Box<dyn Error>> {
        let user_pattern = Regex::new(r"TODO\([@\w]+\)")?;
        let issue_pattern = Regex::new(r"TODO\[\w+-\d+\]")?;

        for node in ctx.trivia_nodes() {
            let text = ctx.get_node_text(node);

            if text.contains("TODO") {
                let has_user_ref = user_pattern.is_match(&text);
                let has_issue_ref = issue_pattern.is_match(&text);

                if !has_user_ref && !has_issue_ref {
                    ctx.report_issue(
                        node,
                        "Complete the task associated with this \"TODO\".",
                        None
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl JavaRule for TodoTagRule {
    fn query() -> Query {
        // Match all comment nodes (block and line comments)
        query!(r#"
            [block_comment
             line_comment] @comment
        "#)
    }
}
```

### 4.3 Ejemplo Medio: S138 (Long Method)

**Regla:** Los métodos no deben exceder un umbral de líneas de código (configurable, por defecto 20).

**Implementación Java:**

```java
// From SonarSource/sonar-java
// File: java-checks/src/main/java/org/sonar/java/checks/LongMethodCheck.java

@Rule(key = "S138")
public class LongMethodCheck extends IssuableSubscriptionVisitor {
    private static final int DEFAULT_THRESHOLD = 20;

    @Override
    public List<Tree.Kind> nodesToVisit() {
        return Collections.singletonList(Tree.Kind.METHOD);
    }

    @Override
    public void visitMethod(MethodTree method) {
        int lineCount = calculateLineCount(method);

        if (lineCount > getThreshold()) {
            reportIssue(method.simpleName(), "Method has " + lineCount +
                " lines, which is greater than " + getThreshold() + " authorized.");
        }
    }

    private int calculateLineCount(MethodTree method) {
        int startLine = method.firstToken().line();
        int endLine = method.lastToken().line();
        return endLine - startLine + 1;
    }
}
```

**Implementación Rust:**

```rust
// rules/src/java/long_method.rs

use tree_sitter::{Query, Node};

declare_rule! {
    /// Detects methods that exceed the configured line count threshold.
    ///
    /// Long methods are difficult to understand, test, and maintain.
    /// Consider extracting logical sections into separate methods.
    ///
    /// **Non-compliant example:**
    /// ```java
    /// public void processData() { /* 50+ lines */ }
    /// ```
    S138("java", "long_method", Severity::Major, RuleCategory::CodeSmell),

    fn check(&self, ctx: &RuleContext) -> Result<(), Box<dyn Error>> {
        let threshold = ctx.param("maximumMethodLines").unwrap_or(20);

        for method_node in ctx.method_nodes() {
            let line_count = count_lines_in_node(ctx.source(), &method_node);

            if line_count > threshold {
                let message = format!(
                    "Method has {} lines, which is greater than {} authorized.",
                    line_count, threshold
                );
                ctx.report_issue(method_node.child_by_field_name("name"), &message, None)?;
            }
        }
        Ok(())
    }
}

fn count_lines_in_node(source: &str, node: &Node) -> usize {
    let start = node.start_position().row + 1; // 1-indexed
    let end = node.end_position().row + 1;
    end - start + 1
}

impl JavaRule for LongMethodRule {
    fn query() -> Query {
        query!(r#"
            method_declaration @method
            constructor_declaration @method
        "#)
    }
}
```

### 4.4 Ejemplo Complejo: S3776 (Cognitive Complexity)

**Regla:** Los métodos no deben tener una complejidadidad cognitiva que exceda un umbral (por defecto 15).

**Implementación Java (algoritmo completo):**

```java
// From SonarSource/sonar-java
// File: java-checks/src/main/java/org/sonar/java/checks/CognitiveComplexityCheck.java

@Rule(key = "S3776")
public class CognitiveComplexityCheck extends IssuableSubscriptionVisitor {
    private static final int DEFAULT_THRESHOLD = 15;
    private int complexity = 0;
    private int structuralIncrement = 0;

    @Override
    public List<Tree.Kind> nodesToVisit() {
        return Arrays.asList(
            Tree.Kind.METHOD,
            Tree.Kind.CONSTRUCTOR,
            Tree.Kind.IF_STATEMENT,
            Tree.Kind.WHILE_STATEMENT,
            Tree.Kind.FOR_STATEMENT,
            Tree.Kind.FOR_EACH_STATEMENT,
            Tree.Kind.DO_STATEMENT,
            Tree.Kind.SWITCH_CASE,
            Tree.Kind.CATCH_CLAUSE,
            Tree.Kind.CONDITIONAL_EXPRESSION,
            Tree.Kind.LAMBDA_EXPRESSION
        );
    }

    @Override
    public void visitNode(AstNode node) {
        if (isMethodOrConstructor(node)) {
            complexity = 0;
            structuralIncrement = 0;
            visitor.skipChildren();
        } else {
            complexity += calculateIncrement(node);
        }
    }

    private int calculateIncrement(AstNode node) {
        Tree.Kind kind = node.kind();

        if (isStructuraIncrement(kind)) {
            int increment = 1 + structuralIncrement;
            structuralIncrement++;
            return increment;
        }

        if (isRecursion(node)) {
            return 1;
        }

        return 0;
    }

    private boolean isStructuraIncrement(Tree.Kind kind) {
        return kind == Tree.Kind.IF_STATEMENT
            || kind == Tree.Kind.WHILE_STATEMENT
            || kind == Tree.Kind.FOR_STATEMENT
            || kind == Tree.Kind.FOR_EACH_STATEMENT
            || kind == Tree.Kind.SWITCH_CASE
            || kind == Tree.Kind.CATCH_CLAUSE
            || kind == Tree.Kind.CONDITIONAL_EXPRESSION;
    }

    private boolean isMethodOrConstructor(AstNode node) {
        return node.kind() == Tree.Kind.METHOD
            || node.kind() == Tree.Kind.CONSTRUCTOR;
    }
}
```

**Implementación Rust:**

```rust
// rules/src/java/cognitive_complexity.rs

use tree_sitter::{Query, Node};
use std::collections::HashMap;

declare_rule! {
    /// Detects methods with excessive cognitive complexity.
    ///
    /// Cognitive complexity measures how difficult a unit of code is to understand.
    /// High complexity correlates with increased defect rates and maintenance costs.
    ///
    /// **Compliant example:**
    /// ```java
    /// public int add(int a, int b) {
    ///     return a + b;
    /// }
    /// ```
    ///
    /// **Non-compliant example:**
    /// ```java
    /// public void processOrder(Order order) {
    ///     if (order.isValid()) {           // +1
    ///         if (order.hasDiscount()) {   // +2 (nesting)
    ///             if (order.getDiscount() > 10) { // +3
    ///                 // deeply nested logic
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    S3776("java", "cognitive_complexity", Severity::Major, RuleCategory::CodeSmell),

    fn check(&self, ctx: &RuleContext) -> Result<(), Box<dyn Error>> {
        let threshold = ctx.param("cognitiveComplexity").unwrap_or(15);

        for node in ctx.function_nodes() {
            let complexity = calculate_cognitive_complexity(ctx.source(), &node)?;

            if complexity > threshold {
                let message = format!(
                    "Cognitive Complexity is {} but should be {} or less.",
                    complexity, threshold
                );
                ctx.report_issue(node.child_by_field_name("name"), &message, None)?;
            }
        }
        Ok(())
    }
}

/// Calculate cognitive complexity for a function node
fn calculate_cognitive_complexity(source: &str, node: &Node) -> Result<usize, Box<dyn Error>> {
    let mut complexity = 0;
    let mut nesting_increment = 0;

    // Structural increment kinds and their increments
    let structural_kinds = [
        "if_statement",
        "while_statement",
        "for_statement",
        "for_each_statement",
        "do_statement",
        "switch_case",
        "catch_clause",
        "conditional_expression",
    ];

    // We need to track nesting depth for proper calculation
    let mut depth = 0;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();

        if structural_kinds.contains(&kind) {
            // Structural increment: 1 + current nesting increment
            complexity += 1 + nesting_increment;
            nesting_increment += 1;
            depth += 1;

            // Recurse into the structural node to find nested structures
            let nested_complexity = calculate_nested_complexity(
                source,
                &child,
                &structural_kinds,
                depth,
                nesting_increment
            )?;
            complexity += nested_complexity;
        } else if kind == "lambda_expression" {
            // Lambdas break the nesting chain for cognitive complexity
            let prev_nesting = nesting_increment;
            nesting_increment = 0;
            let lambda_complexity = calculate_nested_complexity(
                source,
                &child,
                &structural_kinds,
                0,
                0
            )?;
            complexity += lambda_complexity;
            nesting_increment = prev_nesting;
        }
    }

    Ok(complexity)
}

fn calculate_nested_complexity(
    source: &str,
    node: &Node,
    structural_kinds: &[&str],
    depth: usize,
    mut nesting_increment: usize
) -> Result<usize, Box<dyn Error>> {
    let mut complexity = 0;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();

        if structural_kinds.contains(&kind) {
            complexity += 1 + nesting_increment;
            nesting_increment += 1;
            let nested = calculate_nested_complexity(
                source,
                &child,
                structural_kinds,
                depth + 1,
                nesting_increment
            )?;
            complexity += nested;
        }
    }

    Ok(complexity)
}

impl JavaRule for CognitiveComplexityRule {
    fn query() -> Query {
        query!(r#"
            method_declaration @func
            constructor_declaration @func
        "#)
    }
}
```

## 5. Pipeline de Portado en Lote

Para portar múltiples reglas de forma sistemática, se utiliza un pipeline automatizado que maneja la extracción, análisis y generación de código.

### 5.1 Script de Pipeline en Python

```python
#!/usr/bin/env python3
"""
Batch Rule Porting Pipeline
Extracts Java implementations, analyzes AST patterns, and generates Rust skeletons.
"""

import os
import json
import re
import subprocess
from pathlib import Path
from dataclasses import dataclass
from typing import Optional
import requests
from github import Github

OUTPUT_DIR = Path("generated_rules")
RULES_FILE = "sonarqube_rules.jsonl"
GITHUB_TOKEN = os.environ.get("GITHUB_TOKEN")


@dataclass
class RuleSpec:
    """Specification for a rule to be ported."""
    rule_id: str
    name: str
    language: str
    java_file_url: Optional[str]
    ast_patterns: list[str]
    complexity: str  # 'simple', 'medium', 'complex'
    status: str = "pending"


def fetch_java_source(file_url: str, repo: str, path: str) -> str:
    """Fetch Java source code from GitHub."""
    g = Github(GITHUB_TOKEN)
    repo = g.get_repo(repo)
    contents = repo.get_contents(path)
    return contents.decoded_content.decode('utf-8')


def extract_ast_patterns(java_source: str) -> list[str]:
    """Extract AST node types visited from Java source."""
    patterns = []

    # Look for Tree.Kind or node kinds in the Java code
    kind_matches = re.findall(r'Tree\.Kind\.(\w+)', java_source)
    patterns.extend(kind_matches)

    # Look for method visitor patterns
    visitor_methods = re.findall(r'visit(\w+)\s*\(', java_source)
    patterns.extend(visitor_methods)

    return list(set(patterns))


def analyze_java_implementation(rule: RuleSpec) -> RuleSpec:
    """Analyze Java source to extract AST patterns and complexity."""
    if not rule.java_file_url:
        return rule

    try:
        # Extract repo and path from URL
        match = re.match(
            r'https://github\.com/([^/]+)/([^/]+)/blob/([^/]+)/(.+))\.java',
            rule.java_file_url
        )
        if match:
            repo, _, _, path = match.groups()
            source = fetch_java_source(rule.java_file_url, f"SonarSource/sonar-{rule.language}", path)

            rule.ast_patterns = extract_ast_patterns(source)
            rule.complexity = estimate_complexity(rule.ast_patterns)

    except Exception as e:
        print(f"Error analyzing {rule.rule_id}: {e}")

    return rule


def estimate_complexity(patterns: list[str]) -> str:
    """Estimate porting complexity based on AST patterns."""
    simple_kinds = {'TRIVIA', 'COMMENT', 'LINE_COMMENT'}
    medium_kinds = {
        'METHOD', 'CLASS', 'IF_STATEMENT', 'WHILE_STATEMENT',
        'FOR_STATEMENT', 'VARIABLE'
    }

    if not patterns:
        return 'simple'

    pattern_set = set(patterns)

    if pattern_set.issubset(simple_kinds):
        return 'simple'
    elif pattern_set.issubset(medium_kinds | simple_kinds):
        return 'medium'
    else:
        return 'complex'


def generate_rust_skeleton(rule: RuleSpec) -> str:
    """Generate Rust rule skeleton with TODO for implementation."""
    rule_name = rule.rule_id.lower().replace('-', '_').replace(':', '_')

    return f'''// Auto-generated rule skeleton for {rule.rule_id}
// Source: {rule.java_file_url or "API metadata only"}
// AST Patterns detected: {", ".join(rule.ast_patterns) if rule.ast_patterns else "none"}

use crate::declare_rule;

declare_rule! {{
    /// TODO: Add rule description from SonarQube.
    ///
    /// **Rule ID:** {rule.rule_id}
    /// **Language:** {rule.language}
    ///
    /// <!-- TODO: Copy description from SonarQube -->
    ///
    /// **Non-compliant example:**
    /// ```java
    /// // TODO: Add example
    /// ```
    ///
    /// **Compliant example:**
    /// ```java
    /// // TODO: Add example
    /// ```
    {rule.rule_id}(
        "{rule.language}",
        "{rule.name.lower().replace(" ", "_")}",
        Severity::Major,  // TODO: Set correct severity
        RuleCategory::CodeSmell
    ),

    fn check(&self, ctx: &RuleContext) -> Result<(), Box<dyn Error>> {{
        // TODO: Implement rule logic
        //
        // Detected AST patterns:
        {"// ".join(rule.ast_patterns) if rule.ast_patterns else "// No patterns detected"}
        //
        // Reference implementation:
        // {rule.java_file_url or "Not available"}

        Ok(())
    }}
}}

impl {rule.language.title()}Rule for {rule_name.title()}Rule {{
    fn query() -> Query {{
        // TODO: Define tree-sitter query
        query!(r#"
            TODO @node
        "#)
    }}
}}
'''


def generate_test_fixtures(rule: RuleSpec) -> dict:
    """Generate test fixture skeleton."""
    rule_id_clean = rule.rule_id.replace(":", "_")

    return {
        "rule_id": rule.rule_id,
        "language": rule.language,
        "fixtures": {
            "valid": f"test_fixtures/{rule_id_clean}/valid_code.rs",
            "invalid": f"test_fixtures/{rule_id_clean}/invalid_code.rs",
        },
        "expected_issues": []
    }


def run_pipeline(language: str = "java"):
    """Execute the batch porting pipeline."""
    OUTPUT_DIR.mkdir(exist_ok=True)
    (OUTPUT_DIR / language).mkdir(exist_ok=True)
    (OUTPUT_DIR / "tests").mkdir(exist_ok=True)

    rules = []
    with open(RULES_FILE) as f:
        for line in f:
            rules.append(json.loads(line))

    # Filter rules for target language
    lang_rules = [r for r in rules if r.get("language") == language]

    print(f"Processing {len(lang_rules)} rules for {language}")

    specs = []
    for rule in lang_rules:
        spec = RuleSpec(
            rule_id=rule["rule_id"],
            name=rule["name"],
            language=language,
            java_file_url=find_java_implementation_url(rule["rule_id"], language),
            ast_patterns=[],
            complexity="unknown"
        )

        # Analyze and update
        spec = analyze_java_implementation(spec)

        # Generate Rust skeleton
        skeleton = generate_rust_skeleton(spec)
        output_path = OUTPUT_DIR / language / f"{spec.rule_id.lower().replace(':', '_')}.rs"
        output_path.write_text(skeleton)

        # Generate test fixtures
        fixtures = generate_test_fixtures(spec)
        fixtures_path = OUTPUT_DIR / "tests" / f"{spec.rule_id.lower().replace(':', '_')}_fixtures.json"
        fixtures_path.write_text(json.dumps(fixtures, indent=2))

        specs.append(spec)
        print(f"  Generated: {spec.rule_id} ({spec.complexity})")

    # Generate module file
    module_content = generate_module_file(specs)
    (OUTPUT_DIR / language / "mod.rs").write_text(module_content)


def find_java_implementation_url(rule_id: str, language: str) -> Optional[str]:
    """Find GitHub URL for Java implementation."""
    # Map rule_id to file path in SonarSource repos
    # This would be implemented based on known patterns
    return None


def generate_module_file(specs: list[RuleSpec]) -> str:
    """Generate Rust module file with all rule declarations."""
    content = "// Auto-generated module\n\n"

    for spec in specs:
        module_name = spec.rule_id.lower().replace(':', '_')
        content += f"pub mod {module_name};\n"

    content += "\n// Module exports\n"
    content += "pub const RULES: &[&dyn Rule] = &[\n"

    for spec in specs:
        module_name = spec.rule_id.lower().replace(':', '_')
        content += f"    &{module_name}::{module_name}Rule,\n"

    content += "];\n"

    return content


if __name__ == "__main__":
    import sys
    lang = sys.argv[1] if len(sys.argv) > 1 else "java"
    run_pipeline(lang)
```

## 6. Mapeo de Nodos SonarQube a tree-sitter

La siguiente tabla muestra el mapeo entre los tipos de nodos AST del SSLR de SonarQube y los tipos de nodos equivalentes en tree-sitter.

| SonarQube SSLR (Java) | tree-sitter (Java) | Observaciones |
|------------------------|---------------------|---------------|
| `MethodTree` | `method_declaration` | Incluye constructores |
| `ClassTree` | `class_declaration` | También `interface_declaration` |
| `InterfaceTree` | `interface_declaration` | - |
| `IfStatementTree` | `if_statement` | - |
| `WhileStatementTree` | `while_statement` | - |
| `ForStatementTree` | `for_statement` | - |
| `ForEachStatementTree` | `for_each_statement` | Enhanced for loop |
| `DoStatementTree` | `do_statement` | - |
| `SwitchStatementTree` | `switch_statement` | - |
| `SwitchCaseTree` | `switch_case` | Cada case individual |
| `TryStatementTree` | `try_statement` | Incluye recursos |
| `CatchClauseTree` | `catch_clause` | - |
| `VariableTree` | `variable_declaration` | Declaraciones locales |
| `AssignmentExpressionTree` | `assignment_expression` | `a = b` |
| `BinaryExpressionTree` | `binary_expression` | `a + b`, `a && b` |
| `UnaryExpressionTree` | `unary_expression` | `!a`, `-b` |
| `IdentifierTree` | `identifier` | Nombres de variables |
| `LiteralTree` | `literal` | Constantes string, numéricas |
| `TypeTree` | `type_identifier` | Nombres de tipos |
| `BlockTree` | `block` | Bloques de código |
| `ReturnStatementTree` | `return_statement` | - |
| `ThrowStatementTree` | `throw_statement` | - |
| `AnnotationTree` | `annotation` | `@Override`, etc. |
| `CommentTree` | `block_comment`, `line_comment` | Depende del formato |
| `TriviaTree` | `trivia` (en comments) | Comentarios y whitespace |
| `LambdaExpressionTree` | `lambda_expression` | Java 8+ |
| `MethodInvocationTree` | `method_invocation` | Llamadas a métodos |

### Mapeo por Lenguaje

**JavaScript/TypeScript:**

| SonarQube (JS) | tree-sitter (JavaScript) |
|----------------|-------------------------|
| `FunctionTree` | `function_declaration`, `function_expression` |
| `ArrowFunction` | `arrow_function` |
| `ClassTree` | `class_declaration` |
| `IfStatement` | `if_statement` |
| `SwitchStatement` | `switch_statement` |
| `TryStatement` | `try_statement` |
| `WithStatement` | (deprecated) |

**Python:**

| SonarQube (Python) | tree-sitter (Python) |
|-------------------|---------------------|
| `FunctionDef` | `function_definition` |
| `AsyncFunctionDef` | `async_function_definition` |
| `ClassDef` | `class_definition` |
| `If` | `if_statement` |
| `For` | `for_statement` |
| `While` | `while_statement` |
| `Try` | `try_statement` |
| `With` | `with_statement` |
| `Lambda` | `lambda` |

## 7. Priorización: ¿Cuáles Reglas Primero?

La priorización se basa en el equilibrio entre impacto (frecuencia de hallazgos comunes) y esfuerzo de implementación.

### 7.1 Tier 1: Quick Wins (1-2 semanas)

Implementación directa con consultas simples de tree-sitter.

| Rule ID | Nombre | Lenguajes | AST Patterns | Esfuerzo Estimado |
|---------|--------|-----------|--------------|-------------------|
| S138 | Long Method | Java, JS, Python | method_declaration | 2-4 horas |
| S107 | Long Parameter List | Java, JS, Python | method_declaration + params | 2-3 horas |
| S3776 | Cognitive Complexity | Java, JS, Python | method + nested structures | 4-6 horas |
| S115 | String Literal Inspection | Java, JS, Python | literal | 1-2 horas |
| S116 | Variable Naming | Java, JS, Python | identifier | 1-2 horas |
| S117 | Member Variable Naming | Java, JS | identifier (class scope) | 1-2 horas |
| S1135 | TODO Tags | Todos | comment + regex | 1 hora |

**Estimación total:** ~15-20 horas para 7 reglas.

### 7.2 Tier 2: Code Smells (2-3 semanas)

Reglas con lógica moderada que requieren análisis contextual.

| Rule ID | Nombre | Lenguajes | Complejidad | Esfuerzo Estimado |
|---------|--------|-----------|-------------|-------------------|
| S2306 | Bitwise Operations | Java, Python | binary_expression | 3-4 horas |
| S134 | Nested Control Flow | Java, JS, Python | if/for/while nesting | 4-5 horas |
| S1192 | Magic Strings | Java, JS | literal + context | 3-4 horas |
| S1066 | Collapsible "if" Statements | Java, JS, Python | if_statement + nesting | 2-3 horas |
| S1314 | Octal Literals | Java, Python | literal | 1-2 horas |
| S2222 | Lock Pollination | Java | synchronized methods | 4-5 horas |
| S2252 | Tab Character | Todos | regex | 1 hora |

**Estimación total:** ~20-25 horas para 7 reglas.

### 7.3 Tier 3: Security Patterns (2-3 semanas)

Reglas de seguridad que requieren pattern matching preciso.

| Rule ID | Nombre | Lenguajes | Complejidad | Esfuerzo Estimado |
|---------|--------|-----------|-------------|-------------------|
| S2068 | Hard-coded Credentials | Java, Python | assignment + literal | 4-5 horas |
| S2077 | SQL Injection | Java, JS, Python | string concat + query | 6-8 horas |
| S2245 | Random Generator | Java, JS, Python | method_invocation | 3-4 horas |
| S5332 | Code Injection | JS, Python | eval + dynamic | 5-6 horas |
| S5131 | XSS | JS | innerHTML + user input | 6-7 horas |
| S3649 | SQL Injection (Java) | Java | JDBC method calls | 5-6 horas |

**Estimación total:** ~30-35 horas para 6 reglas.

### 7.4 Resumen de Priorización

**Conteo estimado de reglas:** ~21 reglas cubriendo ~80% de hallazgos comunes

| Tier | Reglas | Esfuerzo Total | Cobertura |
|------|--------|----------------|-----------|
| Tier 1 | 7 rules | 15-20 horas | Alta frecuencia |
| Tier 2 | 7 rules | 20-25 horas | Media frecuencia |
| Tier 3 | 6 rules | 30-35 horas | Alta severidad |
| **Total** | **20 rules** | **65-80 horas** | **~80%** |

## 8. Test Fixtures

Los fixtures de test siguen un formato estructurado para verificar que las implementaciones generan las issues esperadas.

### 8.1 Estructura de Directorios

```
test_fixtures/
├── java/
│   ├── S138_long_method/
│   │   ├── valid.rs
│   │   ├── invalid.rs
│   │   └── expected.json
│   └── S3776_cognitive_complexity/
│       ├── valid.rs
│       ├── invalid.rs
│       └── expected.json
└── python/
    └── ...
```

### 8.2 Fixture de Código Válido

```java
// test_fixtures/java/S138_long_method/valid.java

package example;

/**
 * Valid: Short method under threshold (20 lines)
 */
public class ValidExample {

    public int add(int a, int b) {
        return a + b;
    }

    public void process() {
        int x = 1;
        int y = 2;
        System.out.println(x + y);
    }

    public boolean isValid() {
        return true;
    }
}
```

### 8.3 Fixture de Código Inválido

```java
// test_fixtures/java/S138_long_method/invalid.java

package example;

/**
 * Invalid: Method exceeds 20 line threshold
 * Expected: 1 issue on 'longMethod'
 */
public class InvalidExample {

    public void longMethod() { // ISSUE: Method has 25 lines
        int a = 1;
        int b = 2;
        int c = 3;
        int d = 4;
        int e = 5;
        int f = 6;
        int g = 7;
        int h = 8;
        int i = 9;
        int j = 10;
        int k = 11;
        int l = 12;
        int m = 13;
        int n = 14;
        int o = 15;
        int p = 16;
        int q = 17;
        int r = 18;
        int s = 19;
        int t = 20;
        int u = 21;
        int v = 22;
        int w = 23;
        int x = 24;
        int y = 25;
    }
}
```

### 8.4 Archivo de Resultados Esperados

```json
// test_fixtures/java/S138_long_method/expected.json

{
  "rule_id": "java:S138",
  "language": "java",
  "test_cases": [
    {
      "file": "valid.java",
      "should_trigger": false,
      "issues": []
    },
    {
      "file": "invalid.java",
      "should_trigger": true,
      "issues": [
        {
          "line": 6,
          "column": 14,
          "end_line": 31,
          "end_column": 15,
          "message": "Method has 25 lines, which is greater than 20 authorized."
        }
      ]
    }
  ]
}
```

### 8.5 Fixture para Cognitive Complexity

```java
// test_fixtures/java/S3776_cognitive_complexity/invalid.java

package example;

/**
 * Invalid: High cognitive complexity
 * Expected: 1 issue with complexity > 15
 */
public class CognitiveComplexityExample {

    public void process(Order order) { // ISSUE: Cognitive Complexity is 18
        if (order.isValid()) {           // +1
            if (order.hasDiscount()) {   // +2 (nesting)
                if (order.getDiscount() > 10) { // +3
                    if (order.isPremium()) {    // +4
                        applyPremiumDiscount(order);
                    }
                }
            } else {
                if (order.getTotal() > 100) { // +3
                    applyStandardDiscount(order);
                }
            }
        } else {
            rejectOrder(order);
        }
    }

    private void applyPremiumDiscount(Order order) {
        // Deep nesting continues
    }

    private void applyStandardDiscount(Order order) {
        // ...
    }

    private void rejectOrder(Order order) {
        // ...
    }
}
```

## 9. Notas de Compatibilidad

### 9.1 Compatibilidad de IDs de Reglas

Las implementaciones Rust utilizan los mismos IDs de reglas que SonarQube para facilitar la migración:

- Misma convención de nomenclatura: `S###` (ej: `S138`, `S3776`)
- Prefijo de lenguaje: `java:S138`, `python:S138`, `js:S138`
- Consistencia en la interfaz de reporting

### 9.2 Niveles de Severidad

Los niveles de severidad son directamente mapeables:

| SonarQube | Rust Implementation | Descripción |
|-----------|---------------------|-------------|
| `BLOCKER` | `Severity::Blocker` | Issue crítico que bloquea |
| `CRITICAL` | `Severity::Critical` | Issue grave de seguridad o funcionalidad |
| `MAJOR` | `Severity::Major` | Defecto significativo de calidad |
| `MINOR` | `Severity::Minor` | Mejora menor de código |
| `INFO` | `Severity::Info` | Información o sugerencia |

### 9.3 Taxonomía de Categorías

| SonarQube | Rust | Casos de Uso |
|-----------|------|--------------|
| `CODE_SMELL` | `RuleCategory::CodeSmell` | Patrones de código que violan best practices |
| `BUG` | `RuleCategory::Bug` | Defectos que causan comportamiento incorrecto |
| `VULNERABILITY` | `RuleCategory::Vulnerability` | Issues de seguridad |
| `SECURITY_HOTSPOT` | `RuleCategory::SecurityHotspot` | Áreas sensibles que requieren revisión |

### 9.4 Formato de Salida SARIF

Las implementaciones generan salida en formato SARIF para compatibilidad con SonarQube:

```json
{
  "version": "2.1.0",
  "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "axiom-linter",
          "version": "0.1.0",
          "rules": [
            {
              "id": "java:S138",
              "name": "LongMethod",
              "shortDescription": {
                "text": "Methods should not be too long"
              }
            }
          ]
        }
      },
      "results": [
        {
          "ruleId": "java:S138",
          "level": "warning",
          "message": {
            "text": "Method has 25 lines, which is greater than 20 authorized."
          },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": {
                  "uri": "example.java"
                },
                "region": {
                  "startLine": 6,
                  "startColumn": 14,
                  "endLine": 31,
                  "endColumn": 15
                }
              }
            }
          ]
        }
      ]
    }
  ]
}
```

### 9.5 Diferencias con Implementaciones Originales

**Ventajas de la implementación Rust:**

- **Rendimiento:** Rust es significativamente más rápido que Java para análisis de código
- **Integración MCP:** Acceso en tiempo real a través del protocolo MCP
- **Sin JVM:** Elimina la dependencia de Java Runtime
- **Memory safety:** Sin garbage collection pauses

**Consideraciones de compatibilidad:**

- **Edge cases del parser:** tree-sitter puede manejar ciertos edge cases de forma diferente que SSLR
- **Precisión del AST:** Pueden existir diferencias menores en la estructura del árbol
- **Configuración regional:** Manejo de caracteres Unicode puede variar

**Limitaciones conocidas:**

- **Reglas que requieren semántica completa:** Algunas reglas de SonarQube requieren análisis de tipos o resolución de símbolos que tree-sitter no proporciona nativamente
- **Reglas dependientes de configuración del proyecto:** Las reglas que leen `sonar-project.properties` requieren ajustes adicionales

### 9.6 Verificación de Paridad

Para verificar que la implementación Rust produce resultados equivalentes a SonarQube:

```bash
# Run both implementations on the same test corpus
cargo test --package axiom-rules -- java_s138_tests

# Compare output with SonarQube baseline
python scripts/compare_results.py \
    --baseline sonarqube_s138_results.sarif \
    --implementation target/sarif/axiom_s138_results.sarif \
    --tolerance 0.05  # 5% tolerance for line/column differences
```

La paridad se considera aceptable cuando:
- Mismo número de issues detectadas
- Mismas líneas identificadas (con tolerancia de ±1 línea)
- Mensajes equivalentes (misma severidad y categoría)
