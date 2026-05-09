# Rule Segregation Strategy — SOLID & Clean Code

## Problema Actual

`catalog.rs` tiene **28,000+ líneas** con **861 reglas** en un solo archivo monolítico.

```
crates/cognicode-axiom/src/rules/
├── catalog.rs          ← 28,000 líneas monolíticas ❌
├── types.rs            ← tipos bien separados ✓
└── mod.rs              ← re-exports ✓
```

**Problemas para el sistema auto-evolutivo:**
- Dos agentes mejorando reglas distintas → conflicto de merge en el mismo archivo
- Difícil para el LLM entender el contexto (28K líneas = ~70K tokens)
- Imposible testear reglas individualmente sin cargar todo
- Viola **Single Responsibility Principle**

## Diseño Propuesto: Una Regla = Un Archivo

```
crates/cognicode-axiom/src/rules/
├── mod.rs                          # Declaraciones de módulos
├── types.rs                        # Rule trait, enums, tipos base
├── registry.rs                     # Inventory::submit registry (auto-generado)
│
├── rules/                          # Una regla = un archivo
│   ├── mod.rs                      # Re-exports
│   │
│   ├── rust/                       # Reglas específicas de Rust
│   │   ├── mod.rs
│   │   ├── security/
│   │   │   ├── mod.rs
│   │   │   ├── s2068_hardcoded_credentials.rs  ← ~60 líneas
│   │   │   ├── s4792_weak_crypto.rs
│   │   │   ├── s5122_sql_injection.rs
│   │   │   ├── s5332_clear_text_http.rs
│   │   │   └── s5631_unsafe_unwrap.rs
│   │   ├── code_smells/
│   │   │   ├── mod.rs
│   │   │   ├── s134_deep_nesting.rs
│   │   │   ├── s138_long_function.rs
│   │   │   ├── s107_too_many_params.rs
│   │   │   ├── s3776_cognitive_complexity.rs
│   │   │   └── s1135_todo_tags.rs
│   │   └── bugs/
│   │       ├── mod.rs
│   │       ├── s1656_self_assignment.rs
│   │       ├── s1764_identical_branches.rs
│   │       ├── s2259_null_pointer.rs
│   │       └── s2589_tautology.rs
│   │
│   ├── python/                     # Reglas específicas de Python
│   │   ├── mod.rs
│   │   ├── security/
│   │   │   ├── mod.rs
│   │   │   └── py_s2068_hardcoded.rs
│   │   └── code_smells/
│   │       ├── mod.rs
│   │       └── py_s134_deep_nesting.rs
│   │
│   ├── javascript/                 # Reglas JS/TS
│   │   ├── mod.rs
│   │   ├── security/
│   │   │   ├── mod.rs
│   │   │   ├── js_s1523_eval.rs
│   │   │   ├── js_s2611_innerhtml_xss.rs
│   │   │   └── js_s5247_dangerously_set.rs
│   │   ├── code_smells/
│   │   ├── es6/
│   │   │   ├── mod.rs
│   │   │   ├── js_es1_arrow_functions.rs
│   │   │   └── ...
│   │   └── react/
│   │       ├── mod.rs
│   │       └── ...
│   │
│   ├── java/                       # Reglas Java
│   │   ├── mod.rs
│   │   └── ...
│   │
│   └── go/                         # Reglas Go
│       ├── mod.rs
│       └── ...
```

### Estructura de un Archivo de Regla

```rust
// rules/rust/security/s2068_hardcoded_credentials.rs
//! Hard-coded credentials detection rule.
//!
//! Detects passwords, API keys, tokens, and other secrets
//! hard-coded in source code.

use crate::{Severity, Category, Issue, Rule, RuleContext, RuleEntry};
use crate::rules::{
    CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity,
};
use cognicode_macros::declare_rule;
use inventory::submit;

declare_rule! {
    id: "S2068"
    name: "Hard-coded credentials are security sensitive"
    severity: Blocker
    category: SecurityHotspot
    language: "*"
    params: {}

    explanation: "Hard-coded credentials make secrets accessible to anyone with source code access, increasing the risk of credential leakage and unauthorized system access."
    clean_code: Trustworthy
    impacts: [Security: High, Reliability: Medium, Maintainability: Low]

    check: => {
        let mut issues = Vec::new();
        let patterns = [
            ("password", r#"(?i)password\s*[=:]\s*["'][^"']+["']"#),
            ("api_key", r#"(?i)api[_-]?key\s*[=:]\s*["'][^"']+["']"#),
            ("secret", r#"(?i)secret\s*[=:]\s*["'][^"']+["']"#),
            ("token", r#"(?i)token\s*[=:]\s*["'][^"']{20,}["']"#),
        ];

        for (name, pattern) in &patterns {
            let re = match regex::Regex::new(pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };

            for (line_idx, line) in ctx.source.lines().enumerate() {
                if re.is_match(line) {
                    issues.push(Issue::new(
                        "S2068",
                        format!("Hard-coded {} detected at line {}", name, line_idx + 1),
                        Severity::Blocker,
                        Category::SecurityHotspot,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::moderate(
                        &format!("Move {} to environment variables or a secrets manager", name)
                    )));
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detects_password() {
        // ... test inline
    }
    
    #[test]
    fn test_no_false_positive_variable() {
        // ... test inline
    }
}
```

### Beneficios para el Sistema Auto-Evolutivo

| Beneficio | Impacto |
|-----------|---------|
| **Sin conflictos de merge** | Dos agentes editan archivos distintos → 0 conflictos |
| **Contexto LLM manejable** | ~60 líneas por regla vs ~28,000 líneas monolíticas |
| **Testabilidad** | Cada regla tiene sus tests inline → `cargo test s2068` |
| **SRP (Single Responsibility)** | Un archivo = una responsabilidad = una regla |
| **OCP (Open/Closed)** | Añadir regla = añadir archivo, sin modificar existentes |
| **ISP (Interface Segregation)** | Cada regla solo importa lo que necesita |
| **DIP (Dependency Inversion)** | Reglas dependen de traits, no de implementaciones concretas |

### Estrategia de Migración

**No hacer big-bang.** Migración progresiva en 3 fases:

```
Phase 1: Nuevas reglas en archivos separados
  → Cualquier regla nueva o mejorada se crea en rules/{lang}/{category}/

Phase 2: Migrar reglas priorizadas (50 reglas más usadas)
  → Las reglas que el sistema auto-evolutivo toca con frecuencia
  → Mover de catalog.rs a su propio archivo
  → Dejar re-export en catalog.rs para backward compatibility

Phase 3: Migración completa
  → Migrar las 861 reglas restantes
  → catalog.rs se convierte en un thin registry
  → Script de migración automatizado
```

### `catalog.rs` Post-Migración (Thin Registry)

```rust
//! Rule catalog registry — auto-generated from rules/ directory.

// Re-export all rules
pub use crate::rules::rules::rust::security::*;
pub use crate::rules::rules::rust::code_smells::*;
pub use crate::rules::rules::rust::bugs::*;
pub use crate::rules::rules::python::security::*;
pub use crate::rules::rules::javascript::security::*;
// ... etc

// Backward compatibility re-exports
pub use crate::rules::rules::rust::security::s2068_hardcoded_credentials::S2068Rule;
pub use crate::rules::rules::rust::code_smells::s134_deep_nesting::S134Rule;
// ... etc
```

### Convenciones de Naming

```
Formato: {lang}_{category}_{rule_id}_{description}.rs

Ejemplos:
  rust_security_s2068_hardcoded_credentials.rs
  python_code_smells_py_s134_deep_nesting.rs
  js_security_js_s1523_eval.rs
  java_bugs_java_s2259_null_pointer.rs

O más simple (preferido):
  s2068_hardcoded_credentials.rs
  py_s134_deep_nesting.rs
  js_s1523_eval.rs
```

### Auto-Generación del Registry

Script que escanea `rules/` y genera `registry.rs`:

```python
# autoresearch/tools/generate_registry.py
"""Auto-generate rule registry from rules/ directory."""

import os
from pathlib import Path

RULES_DIR = Path("crates/cognicode-axiom/src/rules/rules")

def generate_registry():
    """Scan rules/ directory and generate mod.rs + registry.rs."""
    imports = []
    reexports = []
    
    for root, dirs, files in os.walk(RULES_DIR):
        for f in sorted(files):
            if f.endswith(".rs") and f != "mod.rs":
                rel_path = Path(root).relative_to(RULES_DIR.parent)
                module_name = f.replace(".rs", "")
                
                # Generate module declaration
                module_path = str(rel_path / module_name).replace("/", "::")
                imports.append(f"pub mod {module_name};")
                
                # Extract rule struct name (e.g., S2068Rule from s2068_hardcoded_credentials.rs)
                struct_name = f"{module_name.split('_')[0].upper()}Rule"
                reexports.append(f"pub use {module_path}::{struct_name};")
    
    # Write registry
    registry = "//! Auto-generated rule registry. DO NOT EDIT MANUALLY.\n\n"
    registry += "\n".join(imports)
    registry += "\n\n// Re-exports\n"
    registry += "\n".join(reexports)
    
    Path("crates/cognicode-axiom/src/rules/registry.rs").write_text(registry)

if __name__ == "__main__":
    generate_registry()
```

### Impacto en el Sistema Auto-Evolutivo

**Antes (monolítico):**
```python
# ImproverAgent lee 28,000 líneas para encontrar 1 regla
catalog = read("catalog.rs")  # 28K líneas, ~70K tokens
rule_block = extract_rule_block(catalog, "S2068")  # Frágil, regex sobre 28K líneas
```

**Después (segregado):**
```python
# ImproverAgent lee solo el archivo de la regla
rule_file = f"rules/rust/security/s2068_hardcoded_credentials.rs"  # ~60 líneas
rule_content = read(rule_file)  # ~200 tokens
# Edición directa, sin riesgo de afectar otras reglas
```
