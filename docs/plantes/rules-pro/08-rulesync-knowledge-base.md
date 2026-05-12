# Rulesync Knowledge Base — Visión y Arquitectura

> **Fecha**: 12 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Diseño propuesto  
> **Depende de**: `00-diagnostico.md`, `01-arquitectura.md`, `02-rules-as-code.md`

---

## 1. Visión

**Rulesync Knowledge Base** transforma rulesync de un simple extractor→sanitizer→generador de código Rust en una **base de conocimiento estructurada** que agentes de IA pueden consultar para:

1. **Implementar reglas** para CogniCode con conocimiento profundo de patrones de detección, fixes, y ejemplos
2. **Inferir comportamiento** de reglas que solo existen como IDs o descripciones textuales
3. **Mapear entre herramientas** (Semgrep → CogniCode, ESLint → tree-sitter) de forma semántica
4. **Generar código** con contexto rico (no solo un struct vacío)

El objetivo es que un agente de IA pueda preguntar *"¿cómo detecto SQL injection en Python?"* y recibir no solo el CWE y una descripción, sino también:
- Los **patrones AST** que Semgrep usa (con `pattern`, `pattern-not`, `pattern-inside`)
- Los **selectores AST** que ESLint usa (con `Selector` y `create()` return keys)
- **Ejemplos de código vulnerable** y **código seguro**
- **Fix automático** sugerido
- **Metadatos de seguridad** (CWE, OWASP, Certainty, Severity)

---

## 2. Problema Actual

### 2.1 Lo que rulesync extrae AHORA

```
RawExtractedRule {
    source_tool: "Semgrep",
    source_id: "semgrep::java.lang.security.audit.cglib.invoke-missing-finalize",
    name: "Invoking missing finalize()",
    description: "This method should invoke super.finalize() before returning",
    message: "Override finalize() should call super.finalize()",
    cwe: vec!["CWE-583"],
    owasp: vec![],
    ...
}
```

**Problemas**:
- ❌ **No tiene patrones de detección** — no sabe qué buscar en el AST
- ❌ **No tiene fixes** — no puede sugerir cómo arreglar
- ❌ **No tiene ejemplos** — no hay code samples vulnerable/seguro
- ❌ **No tiene severity confiable** — `infer_severity()` fue eliminada por broken
- ❌ **No tiene metadata de seguridad** — ninguna información sobre exploitability, confidence

### 2.2 Lo que un agente de IA NECESITA

```json
{
  "id": "sec/memory-missing-finalize",
  "detection": {
    "primary_pattern": {
      "type": "ast_grep",
      "pattern": "FINALIZE_CALL",
      "language": "java",
      "constraints": ["NOT inside(try)", "NOT preceded_by(super.finalize)"]
    },
    "alternative_patterns": [...],
    "exclusion_patterns": [...]
  },
  "fix": {
    "suggestion": "Add super.finalize() call in finally block",
    "code_fix": "try { ... } finally { super.finalize(); }",
    "complexity": "simple"
  },
  "examples": {
    "vulnerable": [...],
    "secure": [...],
    "false_positives": [...]
  },
  "security": {
    "cwe": ["CWE-583"],
    "owasp": [],
    "certainty": "medium",
    "exploitability": "low"
  }
}
```

---

## 3. Arquitectura del Knowledge Base

### 3.1 Data Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                     RuleKnowledge (top-level)                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │  DetectionKnow-  │  │  FixKnowledge    │  │  Examples         │  │
│  │  ledge          │  │                  │  │                   │  │
│  │                 │  │  - suggestion    │  │  - vulnerable[]   │  │
│  │ - primary_      │  │  - code_fix      │  │  - secure[]       │  │
│  │   pattern       │  │  - references[]  │  │  - false_         │  │
│  │ - alternative_  │  │  - complexity    │  │    positives[]    │  │
│  │   patterns[]    │  │                  │  │                   │  │
│  │ - exclusion_    │  └──────────────────┘  └──────────────────┘  │
│  │   patterns[]    │                                             │
│  │ - detection_    │                                             │
│  │   type          │                                             │
│  │ - confidence    │                                             │
│  └─────────────────┘                                             │
│                                                                   │
│  ┌─────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │  SecurityMeta-  │  │  CrossReferences  │  │  Provenance     │  │
│  │  data           │  │                  │  │                  │  │
│  │                │  │  - related_[]    │  │  - sources[]     │  │
│  │ - cwe[]        │  │  - conflicts[]    │  │  - extracted_at  │  │
│  │ - owasp[]      │  │  - supersedes[]  │  │  - confidence    │  │
│  │ - certainty    │  │                  │  │  - tool_versions │  │
│  │ - exploitabil- │  └──────────────────┘  └──────────────────┘  │
│  │   ity          │                                             │
│  │ - impact       │                                             │
│  └─────────────────┘                                             │
│                                                                   │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 RuleKnowledge Schema (Rust)

```rust
/// Top-level knowledge entry for a single rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleKnowledge {
    /// CogniCode rule ID (e.g. "sec/auth-hardcoded-credentials")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Full description
    pub description: String,
    /// Issue message template
    pub message: String,
    /// Severity: "critical" | "major" | "minor" | "info"
    pub severity: Severity,
    /// Category: "security" | "bug" | "code-smell" | "performance" | "style"
    pub category: Category,
    /// Languages this rule applies to
    pub languages: Vec<String>,
    /// Detection knowledge
    pub detection: DetectionKnowledge,
    /// Fix knowledge (optional — not all rules have fixes)
    pub fix: Option<FixKnowledge>,
    /// Code examples
    pub examples: Examples,
    /// Security metadata
    pub security: SecurityMetadata,
    /// Cross-references to other rules
    pub cross_references: CrossReferences,
    /// Provenance information
    pub provenance: Provenance,
}
```

### 3.3 DetectionKnowledge

```rust
/// How a rule detects issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionKnowledge {
    /// Primary detection pattern
    pub primary_pattern: DetectionPattern,
    /// Alternative patterns (fallbacks, language variants)
    pub alternative_patterns: Vec<DetectionPattern>,
    /// Patterns that EXCLUDE matches (e.g. "inside try-catch")
    pub exclusion_patterns: Vec<DetectionPattern>,
    /// Type of detection
    pub detection_type: DetectionType,
    /// How confident we are in this detection (0.0 - 1.0)
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionPattern {
    /// Pattern type: ast_grep, tree_sitter, regex, dataflow
    pub pattern_type: PatternType,
    /// The pattern string (language-specific)
    pub pattern: String,
    /// Target language (e.g. "python", "javascript", "generic")
    pub language: String,
    /// Additional constraints (e.g. "inside function body", "not in test file")
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DetectionType {
    /// Simple text/keyword matching (Layer 0)
    Keyword,
    /// AST pattern matching (Layer 1)
    Structural,
    /// Symbol-aware matching (Layer 2)
    Semantic,
    /// Data flow / taint tracking (Layer 3)
    Flow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatternType {
    /// ast-grep YAML pattern
    AstGrep,
    /// tree-sitter query
    TreeSitter,
    /// Regular expression
    Regex,
    /// Dataflow/trace specification
    Dataflow,
}
```

### 3.4 FixKnowledge

```rust
/// How to fix the detected issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixKnowledge {
    /// Human-readable fix suggestion
    pub suggestion: String,
    /// Code fix template (may contain $PLACEHOLDER markers)
    pub code_fix: Option<String>,
    /// External references (e.g. CWE remediation page)
    pub references: Vec<String>,
    /// Fix complexity: "simple" | "moderate" | "complex"
    pub complexity: FixComplexity,
    /// Is this fix safe to auto-apply?
    pub safe_to_auto_fix: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FixComplexity {
    Simple,   // One-line change, e.g. add `super.finalize()`
    Moderate, // Multi-line change, e.g. add try-catch wrapper
    Complex,  // Requires understanding of program semantics
}
```

### 3.5 Examples

```rust
/// Code examples for a rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Examples {
    /// Vulnerable/buggy code samples
    pub vulnerable: Vec<CodeExample>,
    /// Secure/fixed code samples
    pub secure: Vec<CodeExample>,
    /// Known false positive scenarios
    pub false_positives: Vec<CodeExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExample {
    /// Programming language
    pub language: String,
    /// Code sample
    pub code: String,
    /// Why this example is relevant
    pub description: String,
}
```

### 3.6 SecurityMetadata

```rust
/// Security classification for a rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMetadata {
    /// CWE references
    pub cwe: Vec<String>,
    /// OWASP Top 10 references
    pub owasp: Vec<String>,
    /// CERT references
    pub cert: Vec<String>,
    /// How certain is this detection (high/medium/low)
    pub certainty: Certainty,
    /// How exploitable is the vulnerability (high/medium/low/na)
    pub exploitability: Exploitability,
    /// What's the impact if this vulnerability is exploited
    pub impact: Impact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Certainty { High, Medium, Low }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Exploitability { High, Medium, Low, NotApplicable }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Impact { Critical, High, Medium, Low, Info }
```

### 3.7 CrossReferences y Provenance

```rust
/// References to other related rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossReferences {
    /// Rules that are related (same topic, different language)
    pub related: Vec<String>,
    /// Rules that conflict (should not both trigger)
    pub conflicts: Vec<String>,
    /// Rules that this one replaces (deprecated)
    pub supersedes: Vec<String>,
}

/// Where this knowledge came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// Source tools that contributed knowledge
    pub sources: Vec<SourceContribution>,
    /// When this knowledge was extracted/compiled
    pub extracted_at: String,
    /// Overall confidence in the knowledge (0.0 - 1.0)
    pub confidence: f64,
    /// Tool versions used for extraction
    pub tool_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceContribution {
    /// Which tool provided this knowledge
    pub tool: String,
    /// Original ID in that tool
    pub original_id: String,
    /// What this source contributed (detection, fix, examples, metadata)
    pub contributed: Vec<String>,
    /// Confidence in this particular source's data
    pub confidence: f64,
}
```

---

## 4. Pipeline de Generación

### 4.1 Flujo de Datos

```
┌────────────────────────────────────────────────────────────────────────┐
│                     RULESYNC KB GENERATION PIPELINE                     │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────────────┐  │
│  │ EXTRACT  │──▶│ ENRICH   │──▶│ SANITIZE │──▶│ GENERATE OUTPUT  │  │
│  │          │   │          │   │          │   │                  │  │
│  │ Clippy   │   │ CWE DB   │   │ Strip    │   │ Knowledge JSON  │  │
│  │ Ruff     │   │ OWASP DB │   │ traces   │   │ Knowledge YAML   │  │
│  │ ESLint   │   │ Fix DB   │   │ tool     │   │ Rust structs     │  │
│  │ Semgrep  │   │ Examples │   │ IDs      │   │ (#[cogni_rule])  │  │
│  │ CodeQL   │   │ AST DB   │   │ refs     │   │                  │  │
│  │ SonarQub │   │          │   │          │   │                  │  │
│  └──────────┘   └──────────┘   └──────────┘   └──────────────────┘  │
│                                                                        │
│  Phase 1         Phase 2        Phase 3         Phase 4               │
│  (Current)       (New)          (Current)       (Enhanced)            │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Phase 2: Enrichment (NUEVO)

El paso de **Enrichment** es la innovación clave. Después de extraer datos crudos, pero ANTES de sanitizar, enriquecemos con:

| Enrichment Source | What it adds | Priority |
|-------------------|--------------|----------|
| **CWE Database** | Descriptions, severity, exploitability | P0 |
| **OWASP Mappings** | OWASP Top 10 categories | P0 |
| **Semgrep YAML parsing** | AST patterns, fix code, examples | P0 |
| **ESLint rule source** | AST selectors, fix functions | P1 |
| **CodeQL QL patterns** | Dataflow query logic | P1 |
| **SonarQube params** | Regex patterns, thresholds | P2 |
| **Community fixes** | Auto-fix suggestions from GitHub | P3 |

### 4.3 Output Formats

El KB genera **tres formatos** de salida:

1. **Knowledge JSON** — para agentes de IA (formato principal)
   ```json
   {
     "id": "sec/sql-injection-python",
     "detection": {
       "primary_pattern": {
         "pattern_type": "ast_grep",
         "pattern": "f\"SELECT ... { $EXPR }\"",
         "language": "python",
         "constraints": ["inside:format_string"]
       },
       "detection_type": "structural",
       "confidence": 0.9
     },
     "fix": {
       "suggestion": "Use parameterized queries",
       "code_fix": "cursor.execute(\"SELECT ... WHERE id = %s\", (user_id,))",
       "complexity": "moderate"
     },
     ...
   }
   ```

2. **Knowledge YAML** — para consumo humano y fácil edición
   ```yaml
   id: sec/sql-injection-python
   detection:
     primary_pattern:
       pattern_type: ast_grep
       pattern: 'f"SELECT ... { $EXPR }"'
       language: python
       constraints:
         - inside:format_string
     detection_type: structural
     confidence: 0.9
   ```

3. **Rust Code** — para generar `#[cogni_rule]` (formato existente, mejorado)
   ```rust
   #[cogni_rule(
       id = "sec/sql-injection-python",
       severity = "critical",
       category = "security",
       detection_type = "structural"
   )]
   pub struct SqlInjectionPython;
   ```

---

## 5. Diferencias con el Pipeline Actual

| Aspecto | Pipeline Actual | Knowledge Base |
|---------|----------------|----------------|
| **Output principal** | `.rs` files con structs | JSON/YAML + `.rs` files |
| **Patrones de detección** | ❌ No incluidos | ✅ AST patterns, regex, dataflow |
| **Fixes** | ❌ No incluidos | ✅ Sugerencias + código |
| **Ejemplos** | ❌ No incluidos | ✅ Vulnerable/secure/false-positive |
| **Severity** | Inferida de texto | ✅ Enriquecida desde CWE DB |
| **Cruce de reglas** | ❌ No existe | ✅ Related/conflicts/supersedes |
| **Sanitización** | ✅ Agresiva | ✅ Mantiene estructura |
| **Uso por IA** | ❌ Limitado | ✅ Nativo |

---

## 6. Implementación Propuesta

### 6.1 Nuevo Módulo: `rulesync-core/src/knowledge/`

```
rulesync-core/src/knowledge/
├── mod.rs           — Module root, re-exports
├── schema.rs        — RuleKnowledge and all sub-structs
├── enrichment.rs    — CWE/OWASP/fix/example enrichment
├── generator.rs     — KnowledgeBaseGenerator (JSON/YAML/Rust)
└── merge.rs         — Merge knowledge from multiple sources
```

### 6.1.1 `schema.rs`

Define `RuleKnowledge` y todos los sub-structs listados en §3.2-3.7. 
Incluye `Serialize`/`Deserialize` para JSON y YAML.

### 6.1.2 `enrichment.rs`

```rust
/// Enriches a RawExtractedRule with knowledge from external databases.
pub struct EnrichmentPipeline {
    cwe_db: CweDatabase,
    owasp_db: OwaspDatabase,
    /// Pattern enrichment by source tool
    pattern_enrichers: Vec<Box<dyn PatternEnricher>>,
}

pub trait PatternEnricher: Send + Sync {
    fn enrich(&self, rule: &mut RuleKnowledge) -> Result<(), EnrichmentError>;
    fn name(&self) -> &str;
}
```

### 6.1.3 `generator.rs`

```rust
/// Generates Knowledge Base output in multiple formats.
pub struct KnowledgeBaseGenerator {
    rules: Vec<RuleKnowledge>,
}

impl KnowledgeBaseGenerator {
    pub fn new(rules: Vec<RuleKnowledge>) -> Self { ... }
    pub fn generate_json(&self) -> Result<String, serde_json::Error> { ... }
    pub fn generate_yaml(&self) -> Result<String, serde_yaml::Error> { ... }
    pub fn generate_rust(&self) -> Result<String, OutputError> { ... }
    pub fn write_to_directory(&self, path: &Path) -> Result<(), OutputError> { ... }
}
```

### 6.1.4 `merge.rs`

```rust
/// Merges knowledge from multiple sources for the same rule.
pub fn merge_knowledge(
    primary: RuleKnowledge,
    secondary: Vec<RuleKnowledge>,
) -> RuleKnowledge {
    // Merge detection patterns (keep all, dedup)
    // Merge fixes (prefer higher confidence)
    // Merge examples (keep all unique)
    // Merge cross-references (keep all)
    // Merge provenance (list all sources)
}
```

---

## 7. CWE Database para Enrichment

### 7.1 Fuente

Usaremos la **CWE Dictionary** del MITRE (pública, gratuitamente disponible):

- URL: https://cwe.mitre.org/data/xml.html
- Formato: XML (descarga directa)
- Licencia: Public domain para uso de referencia

### 7.2 Datos Enriquecidos por CWE

| Campo CWE | Uso en Knowledge |
|-----------|-----------------|
| `ID` | Referencia cruzada |
| `Name` | Validación de nombre |
| `Description` | Enriquecer descripción de la regla |
| `Potential_Mitigations` | Generar `FixKnowledge` |
| `Common_Consequences` | Llenar `SecurityMetadata.impact` |
| `Likelihood_of_Exploit` | Llenar `SecurityMetadata.exploitability` |
| `Applicable_Platforms` | Validar `languages` |
| `Related_Attack_Patterns` | Generar `CrossReferences` |

### 7.3 Ontología de Severity

La severity en el KB se derivará de múltiples fuentes:

```
severity = f(cwe_impact, cwe_exploitability, source_tool_severity, community_weight)

CWE-89 (SQL Injection)    → impact=HIGH, exploit=HIGH → critical
CWE-79  (XSS)             → impact=HIGH, exploit=MED  → major
CWE-583 (Missing finalize) → impact=LOW, exploit=LOW  → minor
```

---

## 8. Integración con el Pipeline Existente

### 8.1 Modificaciones al Flujo Actual

```
ANTES:
  Extractor → RawExtractedRule → Sanitizer → SanitizedRule → Rust Code

DESPUÉS:
  Extractor → RawExtractedRule → Enricher → EnrichedRule → Sanitizer → 
    RuleKnowledge (clean) → KnowledgeBaseGenerator → JSON/YAML/Rust
```

### 8.2 Nuevos Structs Intermedios

```rust
/// After enrichment, before sanitization.
pub struct EnrichedRule {
    /// Original raw extraction data
    pub raw: RawExtractedRule,
    /// Enriched detection patterns
    pub detection_patterns: Vec<DetectionPattern>,
    /// Enriched fix knowledge
    pub fix: Option<FixKnowledge>,
    /// Enriched examples
    pub examples: Examples,
    /// Enriched security metadata
    pub security: SecurityMetadata,
    /// Cross-references found
    pub cross_references: CrossReferences,
}
```

### 8.3 Sanitización Preserva Estructura

La sanitización existente (stripping de tool IDs, bare lint names) se aplica **dentro de los campos de texto**, pero NO elimina la estructura de `DetectionKnowledge`, `FixKnowledge`, etc. Los patrones AST están en campos dedicados, no embebidos en descriptions.

---

## 9. Consumo por Agentes de IA

### 9.1 Modo de Uso

Un agente de IA (como Claude, GPT, etc.) puede consultar el KB de tres formas:

1. **Full Knowledge JSON** — Para entrenamiento o batch generation
   ```bash
   rulesync generate --format json --output knowledge-base.json
   ```

2. **Query por ID** — Para implementar una regla específica
   ```bash
   rulesync query --id sec/sql-injection-python --format yaml
   ```

3. **Query por Criterio** — Para buscar reglas por CWE, categoría, etc.
   ```bash
   rulesync query --cwe CWE-89 --language python --format json
   ```

### 9.2 Prompt Engineering con KB

Cuando un agente de IA necesita implementar una regla, el KB le proporciona:

```
Given rule: sec/sql-injection-python

DETECTION KNOWLEDGE:
- Primary: ast_grep pattern 'f"SELECT ... { $EXPR }"'
  Language: python
  Constraints: inside format_string
- Alternative: regex pattern r'(SELECT|INSERT|UPDATE|DELETE).*\+|format\(.*sql'
  Language: generic
- Exclusion: inside test file, inside fixture

FIX KNOWLEDGE:
- Suggestion: Use parameterized queries
- Code fix: cursor.execute("SELECT ... WHERE id = %s", (user_id,))
- Complexity: moderate
- Safe to auto-fix: NO

EXAMPLES:
- Vulnerable:
    query = f"SELECT * FROM users WHERE id = {user_id}"
- Secure:
    query = "SELECT * FROM users WHERE id = %s"
    cursor.execute(query, (user_id,))
- False positive:
    query = "SELECT * FROM users"  # no user input

SECURITY: CWE-89, OWASP A03:2021, Certainty: HIGH, Exploitability: HIGH
```

Esto es **drásticamente más útil** que un simple ID + descripción.

---

## 10. Próximos Pasos

1. **Implementar `schema.rs`** — Definir todos los structs del KB
2. **Implementar `enrichment.rs`** — CWE database + OWASP mappings
3. **Mejorar Semgrep extractor** — Parsear YAML con patterns/fix/examples (P0)
4. **Mejorar ESLint extractor** — Extraer AST selectors y fix code (P1)
5. **Implementar `KnowledgeBaseGenerator`** — Output JSON/YAML/Rust
6. **Implementar `merge.rs`** — Merge de múltiples fuentes
7. **CLI integration** — `rulesync generate --format knowledge-json`

---

## 11. Referencias Cruzadas

| Documento | Descripción |
|-----------|-------------|
| `00-diagnostico.md` | Estado actual del sistema |
| `01-arquitectura.md` | Arquitectura de 4 capas |
| `02-rules-as-code.md` | Proc-macro y compile-time validation |
| `09-extraction-strategy.md` | Estrategia de extracción por herramienta |
| `10-agent-integration.md` | Criterios de explotación por agentes |

---

*Documento creado como parte del plan CogniCode Rules Pro*  
*Última actualización: 12 de Mayo de 2026*