# Estrategia de Extracción por Herramienta

> **Fecha**: 12 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Diseño propuesto  
> **Depende de**: `08-rulesync-knowledge-base.md`

---

## 1. Visión General

Este documento detalla **cómo extraer conocimiento de detección** de cada herramienta de análisis estático, clasificada por prioridad de extracción. La premisa central es:

> **No todas las herramientas son iguales**. Algunas tienen datos estructurados ricos (Semgrep YAML con AST patterns), otras tienes solo descripciones textuales (Clippy). La estrategia adapta el enfoque de extracción al formato disponible.

### 1.1 Matriz de Prioridad

| Herramienta | Prioridad | Riqueza de Datos | Rationale |
|-------------|-----------|-------------------|-----------|
| **Semgrep** | ★★★★★ | Muy alta | YAML estructurado con patterns, fix, examples, metadata |
| **CodeQL** | ★★★★ | Alta | QL queries con lógica `from/where/select`, dataflow |
| **ESLint** | ★★★★ | Alta | AST selectors en `create()` return, fix code en JS |
| **SonarQube** | ★★★ | Media | Regex params, thresholds, pero patterns son texto |
| **Ruff** | ★★ | Baja | Detección en código Rust, pero descripciones buenas |
| **Clippy** | ★ | Baja | 8136 lints, pero detección es código Rust compilado |

### 1.2 Niveles de Inferencia

La extracción opera en **tres niveles de inferencia**, de más confiable a menos:

```
┌──────────────────────────────────────────────────────────────────────┐
│  L1: EXPLICIT MAPPING                                               │
│  CWE/OWASP references in the extraction data                        │
│  Confidence: ★★★★★ (direct, no inference)                          │
│  Example: CWE-89 referenced in Semgrep rule metadata                 │
├──────────────────────────────────────────────────────────────────────┤
│  L2: DESCRIPTION PARSING                                             │
│  Parse rule descriptions for keywords that map to CWE/OWASP          │
│  Confidence: ★★★ (requires NLP or regex patterns)                   │
│  Example: "SQL injection" → CWE-89, "XSS" → CWE-79                 │
├──────────────────────────────────────────────────────────────────────┤
│  L3: SOURCE CODE ANALYSIS                                            │
│  Read the tool's source code to understand detection logic            │
│  Confidence: ★★ (fragile, version-dependent)                        │
│  Example: Clippy lints are Rust code that calls internal APIs        │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 2. Semgrep (★★★★★ — Prioridad Máxima)

### 2.1 Formato de Datos

Semgrep rules se distribuyen como **YAML estructurado** con:

```yaml
rules:
  - id: python.lang.security.audit.formatted-sql-string
    patterns:
      - pattern: |
          f"SELECT ... { $EXPR } ..."
      - pattern-not: |
          f"SELECT ... { ... } ..."  # placeholder
    pattern-inside: |
      def $FUNC(...):
        ...
    message: "SQL injection vulnerability via string formatting"
    severity: ERROR
    fix: |
      cursor.execute("SELECT ... WHERE id = %s", ($EXPR,))
    languages: [python]
    metadata:
      cwe: "CWE-89"
      owasp: "A03:2021"
      category: "security"
      confidence: HIGH
      references:
        - "https://cwe.mitre.org/data/definitions/89.html"
```

### 2.2 Qué Extraemos

| Campo Semgrep | Mapea a | Confianza |
|---------------|---------|-----------|
| `patterns` | `DetectionKnowledge.primary_pattern` + `alternative_patterns` | ★★★★★ |
| `pattern-not` | `DetectionKnowledge.exclusion_patterns` | ★★★★★ |
| `pattern-inside` | `DetectionPattern.constraints` | ★★★★★ |
| `fix` | `FixKnowledge.code_fix` | ★★★★ |
| `message` | `RuleKnowledge.message` | ★★★ (text) |
| `severity` | `RuleKnowledge.severity` | ★★★★ |
| `metadata.cwe` | `SecurityMetadata.cwe` (L1) | ★★★★★ |
| `metadata.owasp` | `SecurityMetadata.owasp` (L1) | ★★★★★ |
| `metadata.confidence` | `SecurityMetadata.certainty` (L1) | ★★★★★ |
| `languages` | `RuleKnowledge.languages` | ★★★★★ |

### 2.3 Estrategia de Extracción

```python
# Pseudocode for Semgrep YAML extraction
def extract_semgrep_rule(yaml_rule):
    knowledge = RuleKnowledge()
    
    # L1: Explicit mapping
    knowledge.security.cwe = yaml_rule.get("metadata", {}).get("cwe", [])
    knowledge.security.owasp = yaml_rule.get("metadata", {}).get("owasp", [])
    
    # Pattern extraction
    if "patterns" in yaml_rule:
        for pattern in yaml_rule["patterns"]:
            if "pattern" in pattern:
                knowledge.detection.primary_pattern = DetectionPattern(
                    pattern_type=PatternType.AstGrep,
                    pattern=pattern["pattern"],
                    language=yaml_rule.get("languages", ["generic"])[0],
                )
            if "pattern-not" in pattern:
                knowledge.detection.exclusion_patterns.append(
                    DetectionPattern(
                        pattern_type=PatternType.AstGrep,
                        pattern=pattern["pattern-not"],
                        language=yaml_rule.get("languages", ["generic"])[0],
                    )
                )
    
    # Fix extraction
    if "fix" in yaml_rule:
        knowledge.fix = FixKnowledge(
            suggestion=f"Apply fix template",
            code_fix=yaml_rule["fix"],
            safe_to_auto_fix=True,  # Semgrep fixes are explicit
            complexity=assess_fix_complexity(yaml_rule["fix"]),
        )
    
    # Severity mapping
    knowledge.severity = map_semgrep_severity(yaml_rule.get("severity", "WARNING"))
    
    return knowledge
```

### 2.4 Fuente de Datos

- **Semgrep Registry**: https://semgrep.dev/r/ — ~4000+ rules en YAML
- **Comando**: `semgrep --dump-rules --json` (local, si disponible)
- **GitHub**: `semgrep/semgrep-rules` repo (público)

### 2.5 Desafíos

1. **Patrones Semgrep ≠ ast-grep**: Semgrep usa metavariables (`$EXPR`), ast-grep también pero la sintaxis difiere. Se necesita un traductor.
2. **Multi-language patterns**: Algunas reglas aplican a múltiples lenguajes, cada uno con variantes.
3. **Pattern-inside vs pattern-not**: Semgrep tiene una semántica específica para estos modificadores que no existe igual en ast-grep.

---

## 3. CodeQL (★★★★ — Alta Prioridad)

### 3.1 Formato de Datos

CodeQL rules se escriben en **QL language** con estructura:

```ql
/**
 * @name SQL injection
 * @description SQL injection vulnerability
 * @kind path
 * @problem.severity error
 * @precision high
 * @id py/sql-injection
 * @tags security
 *       external CWE-89
 */
import python

from Expr e, Call cfg
where cfg.getParameter(0).toString().regexpMatch(".*SELECT.*")
  and e = cfg.getArgument(0)
select e, "SQL injection via $0", e
```

### 3.2 Qué Extraemos

| Campo CodeQL | Mapea a | Confianza |
|--------------|---------|-----------|
| `from` clause | `DetectionPattern.constraints` (source predicates) | ★★★★ |
| `where` clause | `DetectionPattern.pattern` (QL logic) | ★★★ (needs translation) |
| `select` clause | `RuleKnowledge.message` template | ★★★★ |
| `@id` annotation | Cross-reference to CodeQL rule | ★★★★★ |
| `@kind path` | `DetectionType.Flow` | ★★★★★ |
| `@precision` | `SecurityMetadata.certainty` | ★★★★★ |
| `@tags`, `CWE-*` | `SecurityMetadata.cwe` (L1) | ★★★★★ |
| `@problem.severity` | `RuleKnowledge.severity` | ★★★★ |

### 3.3 Estrategia de Extracción

- **Fuente**: GitHub `github/codeql` repo (público, Apache-2.0/MIT)
- **Método**: Parse QL files con comentario de anotación (`@name`, `@description`, etc.)
- **QL Logic → Patterns**: `from/where` se exporta como texto descriptivo + se intenta traducir a constraints
- **Dataflow**: `@kind path` indica que es una regla de dataflow → `DetectionType.Flow`

### 3.4 Desafíos

1. **QL es un lenguaje completo** — No se puede "parsear" la lógica de detección simplemente. Se necesita un analizador QL completo o se exporta como texto descriptivo.
2. **Solo aplicable al conocimiento** — CodeQL rules son conocimiento valioso para entender QUÉ detectan, pero NO para copiar CÓMO lo detectan (eso es QL, no es portable).
3. **Prioridad de extracción** — Los `@tags`, `@kind`, `@precision` son L1 (structure metadata), pero la lógica `from/where` es L3 (requires code analysis).

---

## 4. ESLint (★★★★ — Alta Prioridad)

### 4.1 Formato de Datos

ESLint rules se implementan como **JavaScript/TypeScript** con:

```javascript
module.exports = {
  create(context) {
    return {
      // AST selector — this IS the detection pattern
      "CallExpression[callee.name='eval']"(node) {
        context.report({
          node,
          messageId: "unexpectedEval",
          fix(fixer) {
            return fixer.replaceText(node, "Function(...)");
          }
        });
      },
      
      // More selectors...
      "BinaryExpression[operator='==']"(node) {
        // ...
      }
    };
  },
  meta: {
    docs: {
      description: "disallow the use of eval()",
      category: "Security",
      recommended: true,
    },
    fixable: "code",  // ← Has auto-fix!
    schema: [],        // ← Rule options
    messages: {
      unexpectedEval: "eval() can be harmful.",
    },
  },
};
```

### 4.2 Qué Extraemos

| Campo ESLint | Mapea a | Confianza |
|--------------|---------|-----------|
| `create()` return keys (AST selectors) | `DetectionKnowledge.primary_pattern` + `alternative_patterns` | ★★★★★ |
| `fix()` function existence | `FixKnowledge.safe_to_auto_fix` = true | ★★★★★ |
| `fix()` implementation | `FixKnowledge.code_fix` (if simple) | ★★★ (needs JS analysis) |
| `meta.docs.description` | `RuleKnowledge.description` | ★★★★ |
| `meta.docs.category` | `RuleKnowledge.category` | ★★★★ |
| `meta.docs.recommended` | `RuleKnowledge.severity` (recommended → higher) | ★★★ |
| `meta.fixable` | `FixKnowledge` presence | ★★★★★ |
| `meta.messages` | `RuleKnowledge.message` | ★★★★ |
| `meta.schema` | Rule options/thresholds | ★★★ |

### 4.3 Estrategia de Extracción

**Nivel 1: AST Selectors** (Alta confianza)

ESLint AST selectors son **directamente mapeables** a ast-grep patterns porque ambos operan sobre AST node types:

```
ESLint: CallExpression[callee.name='eval']
  ↓
ast-grep: $.call(callee: { name: "eval" })
```

**Nivel 2: Fix Code** (Media confianza)

La presencia de `fix()` implica que la regla tiene fix. Extraer el **código del fix** requiere análisis del JS, pero se puede almacenar como referencia.

**Nivel 3: Messages y Schema** (Alta confianza)

Los `messages` y `schema` son texto estructurado que se extrae directamente.

### 4.4 Fuente de Datos

- **ESLint repo**: https://github.com/eslint/eslint (MIT license)
- **plugins**: tiposcript-eslint, eslint-plugin-react, eslint-plugin-security, etc.
- **ESLint rule index**: https://eslint.org/docs/rules/

### 4.5 Mapping de AST Selectores

Los selectores CSS-like de ESLint se mapean a ast-grep:

| ESLint Selector | ast-grep Pattern | Notas |
|----------------|-------------------|-------|
| `CallExpression[callee.name='eval']` | `eval($$$ARGS)` | Direct |
| `MemberExpression[computed=true]` | `$OBJ[$PROP]` | Computed access |
| `BinaryExpression[operator='==']` | `$A == $B` | Operator matching |
| `Identifier[name='foo']` | `foo` | Simple name |
| `FunctionDeclaration[id.name='foo']` | `function foo($$$) { $$$ }` | With body |

**Automatización**: Se puede crear un traductor ESLint → ast-grep porque los selectores ESLint siguen una gramática predecible.

---

## 5. SonarQube (★★★ — Prioridad Media)

### 5.1 Formato de Datos

SonarQube rules se acceden via **API JSON** con:

```json
{
  "key": "S2077",
  "name": "SQL queries should not be vulnerable to injection attacks",
  "htmlDesc": "<p>SQL injection...</p>",
  "severity": "CRITICAL",
  "type": "VULNERABILITY",
  "tags": ["cwe", "security", "sql"],
  "params": [
    { "key": "customPatterns", "defaultValue": "", "description": "Comma-separated list of keywords" }
  ]
}
```

### 5.2 Qué Extraemos

| Campo SonarQube | Mapea a | Confianza |
|----------------|---------|-----------|
| `key` | `Provenance.original_id` | ★★★★★ |
| `name` | `RuleKnowledge.name` | ★★★★ |
| `htmlDesc` | `RuleKnowledge.description` | ★★★ (needs HTML stripping) |
| `severity` | `RuleKnowledge.severity` | ★★★★ (direct mapping) |
| `type` | `RuleKnowledge.category` | ★★★★ (VULNERABILITY→security, etc) |
| `tags` | `RuleKnowledge.security.cwe` y otros | ★★★ (L2 parsing) |
| `params` | `DetectionPattern.constraints` | ★★ (thresholds, patterns) |

### 5.3 Estrategia de Extracción

- **Fuente primaria**: SonarQube API (necesita instancia corriendo)
- **Fuente alternativa**: Web scraping de https://rules.sonarsource.com/ (público)
- **L2 parsing**: Buscar CWE IDs en `tags` y en `htmlDesc`
- **Param extraction**: Los `params` a veces contienen regex patterns o thresholds

### 5.4 Limitaciones

1. **Detection logic is NOT in the API** — SonarQube rules son código Java private, no accessible
2. **HTML descriptions** — Necesitan stripping de HTML para ser útiles
3. **Regex patterns** — Algunos `params` tienen regex, pero limitados
4. **No fix code** — SonarQube no expone auto-fix

---

## 6. Ruff (★★ — Baja Prioridad)

### 6.1 Formato de Datos

Ruff lints se definen en **código Rust** (`crates/ruff/src/rules/`):

```rust
/// ## What it does
/// Checks for `exec()` calls.
///
/// ## Why is this bad?
/// `exec()` can execute arbitrary code...
///
/// ## Example
/// ```python
/// exec("print('hello')")  # Bad
/// ```
#[violation]
pub struct ExecBuiltin {
    pub func: String,
}

impl Violation for ExecBuiltin {
    const FIX_FLAGS: FixFlags = FixFlags::SAFE_NO_FIX;

    fn message(&self) -> String {
        format!("Use of exec() can be dangerous")
    }
}

// Detection logic in visitor:
fn visit_call(&mut self, call: &ExprCall) {
    if call.func.name() == "exec" || call.func.name() == "eval" {
        // ... report
    }
}
```

### 6.2 Qué Extraemos

| Campo Ruff | Mapea a | Confianza |
|------------|---------|-----------|
| Violation struct name | `RuleKnowledge.name` | ★★★★ |
| `message()` | `RuleKnowledge.message` | ★★★★ |
| Doc comments (## What/Why/Example) | `Examples` + `FixKnowledge.suggestion` | ★★★ |
| `FIX_FLAGS` | `FixKnowledge.safe_to_auto_fix` | ★★★★★ |
| Detection logic (visitor code) | `DetectionPattern` (L3) | ★★ |

### 6.3 Estrategia de Extracción

- **Fuente**: `astral-sh/ruff` GitHub repo (MIT license)
- **Método**: Parse doc comments con regex (structured format)
- **Detection**: L3 — analizamos el visitor code, pero es fragile
- **Rendimiento**: Ruff tiene pocas reglas (~400), gran cobertura Python

### 6.4 Desafíos

1. **Detection logic en Rust** — No portable a ast-grep directamente, pero la descripción es buena
2. **Doc comments son informales** — No todos los lints los tienen, formato variable
3. **Baja prioridad** — Ruff tiene pocas reglas y son en su mayoría estilo Python

---

## 7. Clippy (★ — Prioridad Mínima)

### 7.1 Por Qué Baja Prioridad

Clippy tiene **8136 lints**, pero:

1. **Detection logic es código Rust compilado** — No hay forma de extraerlo sin analizadores Rust avanzados
2. **Los lints son muy específicos de Rust** — No se mapean a otros lenguajes
3. **Metadata mínima** — Solo ID, descripción, y sometimes una help URL
4. **Ya extraído y sanitizado** — Los 8136 lints ya están en el pipeline actual

### 7.2 Qué Extraemos (Limitado)

| Campo Clippy | Mapea a | Confianza |
|--------------|---------|-----------|
| Lint name | `RuleKnowledge.name` | ★★★★ |
| Description | `RuleKnowledge.description` | ★★★★ |
| Help text | `Examples` (if contains code) | ★★ |
| Lint group | `RuleKnowledge.category` (partial) | ★★★ |
| Default level (warn/deny) | `RuleKnowledge.severity` | ★★ |

### 7.3 Estrategia de Extracción

- **Fuente actualizada**: `rust-lang/rust-clippy` repo (MIT/Apache-2.0)
- **Método**: Parse lint definitions con regex sobre código Rust
- **L2 enrichment**: Parse descriptions para keywords → CWE mapeo
- **No L3** — No vale la pena analizar el código Rust de detección

### 7.4 El Valor de Clippy en el KB

Aunque Clippy tiene baja prioridad de extracción, su **volumen** (8136 lints) proporciona:

1. **Cobertura masiva de Rust** — El KB más completo de reglas Rust
2. **Descriptions de calidad** — Los doc comments son buenos
3. **Categorización** — Lint groups como `style`, `correctness`, `complexity`
4. **Sanitización probada** — Ya está en el pipeline, solo falta enriquecer

---

## 8. Enrichment Pipeline por Prioridad

```
┌────────────────────────────────────────────────────────────────────────┐
│                    ENRICHMENT EXECUTION ORDER                         │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  Phase 1: CWE/OWASP Database (L1 — explicit mappings)                │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ • Download MITRE CWE XML                                       │  │
│  │ • Cross-reference with OWASP Top 10                            │  │
│  │ • Build CWE→Severity, CWE→Exploitability lookup                │  │
│  │ • Apply L1 enrichment to ALL rules (all 6 tools)               │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│  Phase 2: Structural Pattern Extraction (P0 tools)                     │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ • Parse Semgrep YAML → DetectionPattern + FixKnowledge        │  │
│  │ • Parse ESLint JS → AST selectors + fix code                   │  │
│  │ • Parse CodeQL QL → from/where logic + dataflow type           │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│  Phase 3: Description Enrichment (L2 — all tools)                    │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ • Keyword-based CWE inference from descriptions                │  │
│  │ • Code example extraction from doc comments                     │  │
│  │ • Severity inference from category + CWE                        │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│  Phase 4: Source Code Analysis (L3 — Clippy/Ruff only)               │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ • Parse Clippy visitor code for detection patterns              │  │
│  │ • Parse Ruff checker code for detection patterns                │  │
│  │ • Low confidence, but fills gaps                                │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│  Phase 5: Cross-Reference Merge                                       │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ • Group rules by CWE/error type                                 │  │
│  │ • Merge detection patterns from multiple sources                 │  │
│  │ • Deduplicate overlapping rules                                  │  │
│  │ • Build CrossReferences links                                    │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Tabla de Mapeo de Severity

### 9.1 Mapeo por Herramienta

| Severity Source | → CogniCode KB | Notes |
|----------------|----------------|-------|
| **Semgrep**: ERROR | critical | Semgrep solo usa ERROR/WARNING/INFO |
| **Semgrep**: WARNING | major | |
| **Semgrep**: INFO | minor | |
| **CodeQL**: error | critical | |
| **CodeQL**: warning | major | |
| **CodeQL**: recommendation | minor | |
| **ESLint**: 2 (error) | major | ESLint no tiene "critical" |
| **ESLint**: 1 (warn) | minor | |
| **SonarQube**: BLOCKER | critical | |
| **SonarQube**: CRITICAL | major | |
| **SonarQube**: MAJOR | major | |
| **SonarQube**: MINOR | minor | |
| **SonarQube**: INFO | info | |
| **Ruff**: error | major | Ruff no distingue critical |
| **Ruff**: warn | minor | |
| **Clippy**: deny | major | |
| **Clippy**: warn | minor | |

### 9.2 Override desde CWE

La severity se puede **promocionar** (nunca degradar) basándose en el CWE:

```
CWE-89 (SQL Injection)  → force to critical if not already
CWE-79  (XSS)           → force to critical if not already
CWE-798 (Hardcoded Cred) → force to critical if not already
CWE-22  (Path Traversal) → force to major if minor
CWE-583 (Missing finalize) → keep as-is
```

---

## 10. Implementación por Fase

### 10.1 Sprint 1-2: Fundamentos

| Tarea | Descripción | Prioridad |
|-------|-------------|-----------|
| **E-KB1** | Crear `schema.rs` con todos los structs del KB | P0 |
| **E-KB2** | Download y parse CWE XML database | P0 |
| **E-KB3** | Implementar `EnrichmentPipeline` con L1 (CWE DB) | P0 |
| **E-KB4** | Crear `KnowledgeBaseGenerator` con output JSON | P0 |

### 10.2 Sprint 3-4: Extracción Estructurada

| Tarea | Descripción | Prioridad |
|-------|-------------|-----------|
| **E-KB5** | Mejorar Semgrep extractor: parse YAML patterns | P0 |
| **E-KB6** | Mejorar ESLint extractor: parse AST selectors | P1 |
| **E-KB7** | Mejorar CodeQL extractor: parse QL annotations | P1 |
| **E-KB8** | Implementar L2 enrichment (description parsing) | P1 |

### 10.3 Sprint 5-6: Enriquecimiento y Merge

| Tarea | Descripción | Prioridad |
|-------|-------------|-----------|
| **E-KB9** | Implementar cross-reference merge | P2 |
| **E-KB10** | Implementar L3 enrichment (Clippy/Ruff source analysis) | P2 |
| **E-KB11** | Severity override desde CWE | P2 |
| **E-KB12** | YAML output format | P2 |

---

## 11. Referencias Cruzadas

| Documento | Descripción |
|-----------|-------------|
| `08-rulesync-knowledge-base.md` | Visión y arquitectura del KB |
| `10-agent-integration.md` | Cómo agentes IA explotan los datos |
| `01-arquitectura.md` | Arquitectura de 4 capas |
| `02-rules-as-code.md` | Proc-macro y compile-time validation |

---

*Documento creado como parte del plan CogniCode Rules Pro*  
*Última actualización: 12 de Mayo de 2026*