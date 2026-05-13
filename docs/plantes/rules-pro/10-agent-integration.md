# Integración de Agentes de IA con el Knowledge Base

> **Fecha**: 12 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Diseño propuesto  
> **Depende de**: `08-rulesync-knowledge-base.md`, `09-extraction-strategy.md`

---

## 1. Visión General

Este documento define **cómo los agentes de IA consumen y explotan los datos del Knowledge Base** para implementar reglas de CogniCode. El objetivo es que un agente pueda:

1. **Consultar** el KB para entender QUÉ detectar, CÓMO detectar, y CÓMO arreglar
2. **Generar código** `#[cogni_rule]` completo con patrones, tests, y fixes
3. **Validar** que la regla implementada coincide con el conocimiento extraído
4. **Iterar** cuando hay falsos positivos o edge cases

---

## 2. Casos de Uso por Tipo de Agente

### 2.1 Agente Implementador (Primary User)

**Rol**: Implementa reglas CogniCode a partir del KB.

**Flujo**:
```
Query KB → Recibir RuleKnowledge → Generar #[cogni_rule] → Tests → Validar
```

**Ejemplo de uso**:

```bash
# Consultar KB para una regla específica
rulesync query --id sec/sql-injection-python

# Output: Knowledge JSON completo con detection, fix, examples
```

**Lo que el agente recibe**:
- Patrones de detección (ast-grep, tree-sitter, regex)
- Código de fix sugerido
- Ejemplos vulnerable/secure
- Metadata de seguridad
- Cross-references

**Lo que el agente produce**:
- `#[cogni_rule]` Rust struct completo
- Tests unitarios con los examples
- Documentación inline

### 2.2 Agente deqa (Quality Assurance)

**Rol**: Verifica que las reglas implementadas coinciden con el KB.

**Flujo**:
```
Leer #[cogni_rule] → Comparar con KB → Reportar discrepancias
```

**Checks que realiza**:
- ¿Los patrones de detección coinciden?
- ¿Los examples cubren los casos del KB?
- ¿La severity coincide?
- ¿Los CWE references están todos incluidos?

### 2.3 Agente de Sugerencia (Recommender)

**Rol**: Sugiere qué reglas implementar basándose en prioridades.

**Flujo**:
```
Analizar codebase → Identificar gaps → Priorizar reglas del KB
```

**Criterios de priorización**:
1. Reglas con CWE de alto impacto (SQL injection, XSS, etc.)
2. Reglas con alta demanda (frecuencia en proyectos reales)
3. Reglas con detección de alta confianza (patterns disponibles)
4. Reglas con fix automático disponible

---

## 3. Formato de Consulta del KB

### 3.1 CLI Interface

```bash
# Consultar por ID
rulesync query --id sec/sql-injection-python --format json

# Consultar por CWE
rulesync query --cwe CWE-89 --format yaml

# Consultar por categoría
rulesync query --category security --format json

# Consultar por lenguaje
rulesync query --language python --category security --format json

# Generar todo el KB
rulesync generate --format json --output knowledge-base.json

# Generar solo detección patterns
rulesync generate --format json --only detection --output patterns.json
```

### 3.2 Output JSON por Query

```json
{
  "id": "sec/sql-injection-python",
  "name": "SQL Injection via String Formatting",
  "description": "Detects SQL injection vulnerabilities...",
  "message": "SQL injection vulnerability via string formatting",
  "severity": "critical",
  "category": "security",
  "languages": ["python"],
  "detection": {
    "primary_pattern": {
      "pattern_type": "ast_grep",
      "pattern": "f\"SELECT ... { $EXPR }\"",
      "language": "python",
      "constraints": ["inside:format_string", "not:test_file"]
    },
    "alternative_patterns": [
      {
        "pattern_type": "regex",
        "pattern": "(SELECT|INSERT|UPDATE|DELETE).*format\\(",
        "language": "generic",
        "constraints": []
      },
      {
        "pattern_type": "tree_sitter",
        "pattern": "(string (interpreted_string_literal (format_spec)))",
        "language": "python",
        "constraints": ["contains:SELECT"]
      }
    ],
    "exclusion_patterns": [
      {
        "pattern_type": "ast_grep",
        "pattern": "$FUNC(sqlalchemy.text($ARGS))",
        "language": "python",
        "constraints": []
      }
    ],
    "detection_type": "structural",
    "confidence": 0.9
  },
  "fix": {
    "suggestion": "Use parameterized queries with cursor.execute()",
    "code_fix": "cursor.execute(\"SELECT * FROM users WHERE id = %s\", (user_id,))",
    "references": [
      "https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet.html"
    ],
    "complexity": "moderate",
    "safe_to_auto_fix": false
  },
  "examples": {
    "vulnerable": [
      {
        "language": "python",
        "code": "query = f\"SELECT * FROM users WHERE id = {user_id}\"\ncursor.execute(query)",
        "description": "f-string with user input in SQL query"
      },
      {
        "language": "python",
        "code": "query = \"SELECT * FROM users WHERE id = \" + str(user_id)\ncursor.execute(query)",
        "description": "String concatenation with user input"
      }
    ],
    "secure": [
      {
        "language": "python",
        "code": "cursor.execute(\"SELECT * FROM users WHERE id = %s\", (user_id,))",
        "description": "Parameterized query using psycopg2"
      }
    ],
    "false_positives": [
      {
        "language": "python",
        "code": "query = \"SELECT * FROM users\"  # no user input",
        "description": "Static query without user input"
      }
    ]
  },
  "security": {
    "cwe": ["CWE-89"],
    "owasp": ["A03:2021"],
    "cert": [],
    "certainty": "high",
    "exploitability": "high",
    "impact": "critical"
  },
  "cross_references": {
    "related": [
      "sec/sql-injection-javascript",
      "sec/sql-injection-java",
      "sec/sql-injection-go"
    ],
    "conflicts": [],
    "supersedes": []
  },
  "provenance": {
    "sources": [
      {
        "tool": "Semgrep",
        "original_id": "python.lang.security.audit.formatted-sql-string",
        "contributed": ["detection", "fix", "examples", "metadata"],
        "confidence": 0.95
      },
      {
        "tool": "SonarQube",
        "original_id": "S2077",
        "contributed": ["metadata", "severity"],
        "confidence": 0.85
      }
    ],
    "extracted_at": "2026-05-12T10:00:00Z",
    "confidence": 0.92,
    "tool_versions": ["semgrep 1.50", "sonarqube 10.3"]
  }
}
```

### 3.3 Output YAML por Query (mismo contenido, formato legible)

```yaml
id: sec/sql-injection-python
name: SQL Injection via String Formatting
description: Detects SQL injection vulnerabilities...
severity: critical
category: security
languages:
  - python
detection:
  primary_pattern:
    pattern_type: ast_grep
    pattern: 'f"SELECT ... { $EXPR }"'
    language: python
    constraints:
      - inside:format_string
      - not:test_file
  detection_type: structural
  confidence: 0.9
fix:
  suggestion: Use parameterized queries with cursor.execute()
  code_fix: 'cursor.execute("SELECT * FROM users WHERE id = %s", (user_id,))'
  complexity: moderate
  safe_to_auto_fix: false
...
```

---

## 4. Prompt Templates para Agentes

### 4.1 Template: Implementar Regla

```
SYSTEM: You are an expert CogniCode rule implementer. You write Rust code
using the #[cogni_rule] proc-macro system.

KNOWLEDGE BASE ENTRY:
{kb_entry_json}

TASK: Implement a CogniCode rule for the above knowledge base entry.

REQUIREMENTS:
1. Use #[cogni_rule] proc-macro with id, severity, category, detection_type
2. Implement the Rule trait with check() method
3. Use ast-grep patterns from detection.primary_pattern
4. Include exclusion patterns from detection.exclusion_patterns
5. Write tests using the examples provided (vulnerable and secure)
6. If fix is available, implement fix() method
7. Add documentation with CWE references

OUTPUT: Complete Rust struct implementation with tests.
```

### 4.2 Template: Validar Regla

```
SYSTEM: You are a CogniCode rule QA agent. You verify rule implementations
match their Knowledge Base specifications.

RULE IMPLEMENTATION:
{rule_rust_code}

KNOWLEDGE BASE ENTRY:
{kb_entry_json}

VERIFY:
1. Detection patterns match KB entry
2. All exclusion patterns are implemented
3. Severity and category match
4. CWE references are present
5. Examples from KB are covered in tests
6. Fix matches KB suggestion (if applicable)
7. No traces of source tool IDs

OUTPUT: Verification report with PASS/FAIL per check.
```

### 4.3 Template: Sugerir Reglas

```
SYSTEM: You are a CogniCode rule recommender. You analyze project code
and suggest which rules from the Knowledge Base should be implemented.

PROJECT ANALYSIS:
- Language: {primary_language}
- Framework: {framework}
- Security requirements: {requirements}

AVAILABLE KB RULES (by priority):
{kb_rules_summary}

SUGGEST:
1. Top 10 rules to implement first (by impact + feasibility)
2. Which detection_type (keyword, structural, semantic, flow) to use
3. Available fixes to auto-apply
4. Expected false positive rate based on KB confidence

OUTPUT: Prioritized recommendation list with justification.
```

---

## 5. Programa de Alimentación (Feedback Loop)

### 5.1 Ciclo de Mejora Continua

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     KNOWLEDGE BASE FEEDBACK LOOP                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐        │
│   │  EXTRACT │───▶│  ENRICH  │───▶│ SANITIZE │───▶│  QUERY   │        │
│   └──────────┘    └──────────┘    └──────────┘    └─────┬────┘        │
│        ▲                                                    │           │
│        │                                                    ▼           │
│   ┌────┴─────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐        │
│   │  UPDATE  │◀───│  MERGE   │◀───│ VALIDATE │◀───│ IMPLEMENT│        │
│   └──────────┘    └──────────┘    └──────────┘    └──────────┘        │
│        │                                                                 │
│        ▼                                                                 │
│   ┌──────────┐                                                          │
│   │ FEEDBACK │                                                          │
│   │ - FP rate│                                                          │
│   │ - Missing│                                                          │
│   │ - Better │                                                          │
│   │   pattern│                                                          │
│   └──────────┘                                                          │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Tipos de Feedback

| Tipo | Fuente | Ejemplo | Acción |
|------|--------|---------|--------|
| **FP Report** | Usuario/CI | "Rule X tiene 30% FP en React" | Añadir exclusion pattern |
| **FN Report** | Auditoría | "CWE-89 no detectado en ORM" | Añadir alternative pattern |
| **Better Pattern** | Agente IA | "tree-sitter pattern más preciso" | Mejorar primary_pattern |
| **New Example** | Proyecto real | "Código X dispara FP" | Añadir a false_positives |
| **Security Update** | MITRE/CVE | "Nueva variante de SQLi" | Añadir alternative_pattern |

### 5.3 Cómo el Agente Contribuye de Vuelta

Cuando un agente implementador trabaja con el KB y descubre:

1. **Un pattern mejor**: Lo sugiere como `alternative_pattern` para la regla
2. **Un falso positivo**: Lo añade a `examples.false_positives`
3. **Un fix más seguro**: Lo propone como mejora del `code_fix`
4. **Una cross-reference**: La añade a `cross_references.related`

El agente NO modifica el KB directamente — crea un **suggestion** que se mergea manualmente o vía PR:

```json
{
  "type": "suggestion",
  "rule_id": "sec/sql-injection-python",
  "suggestion_type": "add_false_positive",
  "content": {
    "language": "python",
    "code": "cursor.execute(sqlalchemy.text(query))",
    "description": "SQLAlchemy text() is safe if used with bind params"
  },
  "justification": "Found in project X that this triggers FP when using sqlalchemy.text()",
  "confidence": 0.8
}
```

---

## 6. Criterios de Explotación por Tipo de Dato

### 6.1 Detección (Qué buscar)

| Dato del KB | Uso por el Agente | Criterio de Uso |
|-------------|-------------------|-----------------|
| `primary_pattern` (ast-grep) | Usar directamente como pattern en `#[cogni_rule]` | Si confidence > 0.8 |
| `primary_pattern` (regex) | Traducir a PreflightFilter keyword si es simple, AST pattern si complejo | Si no hay AST disponible |
| `alternative_patterns` | Probar si el primary tiene falsos positivos | Como fallback |
| `exclusion_patterns` | Implementar como `pattern-not` o condición adicional | Siempre incluir |
| `detection_type` | Determinar Layer (0=keyword, 1=structural, 2=semantic, 3=flow) | Directo |

**Regla de decisión**:
```
IF confidence >= 0.8 AND pattern_type == ast_grep:
    USE as primary pattern directly
ELIF confidence >= 0.8 AND pattern_type == regex:
    USE as PreflightFilter keyword (if simple)
    OR implement as tree-sitter query (if complex)
ELIF confidence < 0.8:
    REVIEW manually before implementing
    Flag as "needs validation"
```

### 6.2 Fix (Cómo arreglar)

| Dato del KB | Uso por el Agente | Criterio de Uso |
|-------------|-------------------|-----------------|
| `fix.suggestion` | Doc comment en la regla | Siempre |
| `fix.code_fix` | Implementar `fix()` method si `safe_to_auto_fix == true` | Si safe_to_auto_fix |
| `fix.complexity == simple` | Auto-fix viable | Documentar |
| `fix.complexity == moderate/complex` | Solo sugerencia, no auto-fix | Mostrar al usuario |
| `fix.references` | Links en doc comment | Siempre |

**Regla de decisión**:
```
IF safe_to_auto_fix AND code_fix disponible:
    IMPLEMENT fix() method
ELIF suggestion disponible:
    ADD to doc comment as "How to fix: ..."
ALWAYS:
    ADD references as links in documentation
```

### 6.3 Ejemplos (Cómo testar)

| Dato del KB | Uso por el Agente | Criterio de Uso |
|-------------|-------------------|-----------------|
| `examples.vulnerable` | `#[test_rule]` con expectativa de MATCH | Siempre |
| `examples.secure` | `#[test_rule]` con expectativa de NO MATCH | Siempre |
| `examples.false_positives` | `#[test_rule]` con expectativa de NO MATCH | Siempre, prioritizar |

**Regla de decisión**:
```
FOR EACH vulnerable example:
    CREATE test expecting MATCH (positive test)
FOR EACH secure example:
    CREATE test expecting NO MATCH (negative test)
FOR EACH false_positive example:
    CREATE test expecting NO MATCH (FP prevention test)
    HIGHLIGHT in code comment: "This was reported as FP"
```

### 6.4 Seguridad (Cómo clasificar)

| Dato del KB | Uso por el Agente | Criterio de Uso |
|-------------|-------------------|-----------------|
| `security.cwe` | Referencia en doc comment y metadata | Siempre |
| `security.owasp` | Referencia en doc comment | Siempre |
| `security.certainty == high` | confianza en detección | No necesita review |
| `security.certainty == medium` | requiere revisión manual | Flag como "needs_review" |
| `security.certainty == low` | solo como referencia | No implementar como regla |
| `security.exploitability` | priorizar reglas con exploitability alta | Priorización |
| `security.impact` | determinar severity | Override si severity no coincide |

**Regla de decisión**:
```
IF certainty == "high" AND exploitability IN ("high", "medium"):
    IMPLEMENT as PRIMARY rule (Layer 1 or 2)
ELIF certainty == "high" AND exploitability == "low":
    IMPLEMENT as INFO rule (lower severity)
ELIF certainty == "medium":
    IMPLEMENT but FLAG as "needs validation"
    Suggest FP reputation system integration
ELIF certainty == "low":
    DO NOT implement
    ADD to "future consideration" list
```

---

## 7. API del KB para Agentes

### 7.1 Consultas Soportadas

```rust
/// Knowledge Base query interface.
pub trait KnowledgeBaseQuery: Send + Sync {
    /// Get a specific rule by ID.
    fn get_rule(&self, id: &str) -> Option<RuleKnowledge>;
    
    /// Find rules by CWE.
    fn find_by_cwe(&self, cwe: &str) -> Vec<RuleKnowledge>;
    
    /// Find rules by category.
    fn find_by_category(&self, category: &Category) -> Vec<RuleKnowledge>;
    
    /// Find rules by language.
    fn find_by_language(&self, language: &str) -> Vec<RuleKnowledge>;
    
    /// Find rules by severity.
    fn find_by_severity(&self, severity: &Severity) -> Vec<RuleKnowledge>;
    
    /// Find rules by detection type.
    fn find_by_detection_type(&self, dt: &DetectionType) -> Vec<RuleKnowledge>;
    
    /// Find rules with auto-fix available.
    fn find_with_fix(&self) -> Vec<RuleKnowledge>;
    
    /// Find rules by keyword in description/name.
    fn search(&self, query: &str) -> Vec<RuleKnowledge>;
    
    /// Get related rules for a given rule.
    fn get_related(&self, id: &str) -> Vec<RuleKnowledge>;
    
    /// Get all rules (for batch operations).
    fn all_rules(&self) -> Vec<RuleKnowledge>;
    
    /// Get statistics about the KB.
    fn stats(&self) -> KnowledgeBaseStats;
}

#[derive(Debug, Clone)]
pub struct KnowledgeBaseStats {
    pub total_rules: usize,
    pub by_category: HashMap<String, usize>,
    pub by_severity: HashMap<String, usize>,
    pub by_language: HashMap<String, usize>,
    pub with_fix: usize,
    pub with_examples: usize,
    pub avg_confidence: f64,
}
```

### 7.2 Formatos de Salida

```rust
/// Output format for Knowledge Base queries.
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    /// JSON format (machine-readable)
    Json,
    /// YAML format (human-readable, easy to edit)
    Yaml,
    /// Rust code (#[cogni_rule] output)
    Rust,
    /// Markdown documentation
    Markdown,
}
```

---

## 8. Integración con el Pipeline SDD

### 8.1 Flujo en SDD (Spec-Driven Development)

Cuando se crea una nueva regla de CogniCode usando el proceso SDD:

```
1. /sdd-explore "SQL injection detection"
   → KB query: search("SQL injection")
   → Returns: sec/sql-injection-python, sec/sql-injection-javascript, etc.
   → Agent uses KB data to understand the problem space

2. /sdd-propose
   → Agent proposes implementing sec/sql-injection-python
   → KB data fills: detection patterns, examples, severity, CWE refs
   → Proposal includes: which Layer to implement at, confidence level

3. /sdd-spec
   → Spec includes: test cases from examples, expected behavior from KB
   → Verification criteria from: certainty, exclusion patterns

4. /sdd-design
   → Design includes: ast-grep pattern from KB, PreflightFilter keywords
   → Architecture: which Layer (0-3), Visitor pattern if needed

5. /sdd-apply
   → Agent generates #[cogni_rule] using KB data
   → Uses: detection.primary_pattern, fix.code_fix, examples.vulnerable/secure

6. /sdd-verify
   → Tests validate against KB examples
   → FP tests use KB false_positives
   → CogniCode quality check against KB expectations
```

### 8.2 KB como Fuente de Verdad

El KB es la **fuente de verdad** para:
- **Qué detectar** (detection patterns)
- **Cómo detectarlo** (detection type, Layer)
- **Cómo arreglarlo** (fix knowledge)
- **Cómo testarlo** (examples)
- **Cómo clasificarlo** (severity, CWE, OWASP)

Cualquier discrepancia entre el KB y la implementación es un **bug** que debe corregirse.

---

## 9. Matriz de Decisión: Cuándo Consultar el KB

| Situación | Consultar KB? | Qué consultar | Decisión |
|-----------|---------------|---------------|----------|
| Nueva regla desde cero | **Sí, obligatorio** | `search(keyword)` + `get_rule(id)` | Usar KB como base |
| Migrar regla existente | **Sí, recomendado** | `get_rule(id)` | Validar contra KB |
| Validar regla implementada | **Sí, obligatorio** | `get_rule(id)` y comparar | Reportar discrepancias |
| Arreglar falso positivo | **Sí, recomendado** | `get_rule(id)` → `exclusion_patterns` | Añadir exclusion al KB |
| Priorizar qué regla Implementar | **Sí, obligatorio** | `find_by_cwe()` + `find_by_severity()` | Ordenar por impact |
| Arreglar bug en detección | **Sí, recomendado** | `get_rule(id)` → `alternative_patterns` | Probar alternatives |

---

## 10. Limitaciones y Riesgos

### 10.1 Limitaciones Actuales

1. **Semgrep patterns ≠ ast-grep**: Traducción no 1:1, necesita manual review
2. **ESLint selectors**: Gramática CSS-like, necesita parser dedicado
3. **CodeQL logic**: QL es un lenguaje completo, no se puede automatizar la traducción
4. **SonarQube**: Detection logic es código Java private
5. **Clippy/Ruff**: Detection logic es código Rust/Python compilado

### 10.2 Riesgos

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|-------------|---------|------------|
| Sintaxis de patterns no traducible | Alta | Medio | Catalogar como "needs manual review" |
| Patrones de detección incompletos | Media | Alto | Marcar confidence bajo |
| Falsos positivos en KB | Media | Medio | Sistema de FP reputation del KB |
| KB desactualizado vs source tools | Baja | Medio | Re-extract periódicamente |
| Conflicto entre fuentes | Media | Bajo | Merge con prioridad por confidence |

### 10.3 Evolución del KB

El KB es **incremental** — cada nueva extracción y feedback enriquece el conocimiento:

- **v0.1**: Solo metadata (CWE, severity, descripción) — ya funciona
- **v0.2**: + Detection patterns (Semgrep, ESLint) — alto valor
- **v0.3**: + Fix code y examples — completo para agentes
- **v0.4**: + Cross-references y merge — valor agregado
- **v0.5**: + Feedback loop y FP reputation — mejora continua

---

## 11. Referencias Cruzadas

| Documento | Descripción |
|-----------|-------------|
| `08-rulesync-knowledge-base.md` | Visión y arquitectura del KB |
| `09-extraction-strategy.md` | Estrategia de extracción por herramienta |
| `01-arquitectura.md` | Arquitectura de 4 capas |
| `02-rules-as-code.md` | Proc-macro y compile-time validation |
| `04-pre-flight.md` | Layer 0 con Aho-Corasick |

---

*Documento creado como parte del plan CogniCode Rules Pro*  
*Última actualización: 12 de Mayo de 2026*