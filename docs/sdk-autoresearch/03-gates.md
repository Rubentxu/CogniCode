# 03 — Gates Catalog

> Catálogo completo de Gates deterministas. Un Gate es una condición binaria
> (pasa/no pasa) que DEBE cumplirse antes de calcular métricas. Si cualquier
> Gate bloqueante falla, el Health Score es 0.

---

## 1. Taxonomía de Gates

| Categoría | Propósito | Ejemplos |
|-----------|----------|----------|
| **Core** | Viabilidad fundamental | Compilación, Tests |
| **Syntax** | Validez de artefactos | Regex, Tree-sitter queries |
| **Style** | Consistencia | Formato, Lint |
| **Security** | Vulnerabilidades | CVEs, dependencias maliciosas |
| **Performance** | Regresiones | Build time, binary size |
| **API** | Compatibilidad | Breaking changes, semver |
| **Legal** | Compliance | Licencias, dependencias prohibidas |

---

## 2. Catálogo Universal

### G001 — CompilationGate

```
Descripción:  El código fuente compila sin errores.
Bloqueante:   SIEMPRE
Tiempo:       ~5-30s
```

| Lenguaje | Comando | Cómo interpretar |
|----------|---------|-----------------|
| Rust | `cargo check 2>&1` | exit code 0 = OK |
| Python | `python -c "import ast; ast.parse(open('FILE').read())"` | sin excepción = OK |
| JS/TS | `tsc --noEmit 2>&1` | exit code 0 = OK |
| Go | `go build ./... 2>&1` | exit code 0 = OK |
| Java | `javac -d /tmp/out $(find src -name '*.java') 2>&1` | exit code 0 = OK |

### G002 — TestsGate

```
Descripción:  Todos los tests existentes pasan.
Bloqueante:   SIEMPRE
Tiempo:       ~30s-5min
```

| Lenguaje | Comando | Parseo |
|----------|---------|--------|
| Rust | `cargo test --workspace 2>&1` | Extraer "N passed; 0 failed" |
| Python | `pytest --tb=short 2>&1` | Extraer "N passed" |
| JS/TS | `jest --ci 2>&1` | Extraer "Tests: N passed" |
| Go | `go test ./... 2>&1` | Buscar "FAIL" |
| Java | `mvn test 2>&1` | Extraer "Tests run: N, Failures: 0" |

### G003 — SyntaxGate

```
Descripción:  Todos los artefactos sintácticos (regex, queries tree-sitter,
              schemas) compilan/parsean correctamente.
Bloqueante:   SIEMPRE
Tiempo:       <1s
```

Este gate es especialmente importante para proyectos que usan reglas de
detección basadas en regex o tree-sitter (como CogniCode).

```
Validaciones:
  - Regex::new(pattern).is_ok() para cada regex en el código
  - Query::new(language, query).is_ok() para cada query tree-sitter
  - serde_json::from_str::<Schema>(content).is_ok() para schemas
  - serde_yaml::from_str::<Manifest>(content).is_ok() para manifests
```

### G004 — LintGate

```
Descripción:  El código no tiene warnings de lint.
Bloqueante:   Configurable (por defecto: SÍ)
Tiempo:       ~10-60s
```

| Lenguaje | Comando | Threshold |
|----------|---------|-----------|
| Rust | `cargo clippy -- -D warnings 2>&1` | 0 warnings |
| Python | `ruff check 2>&1` | 0 errors |
| JS/TS | `eslint --max-warnings 0 . 2>&1` | 0 warnings |
| Go | `golangci-lint run 2>&1` | 0 issues |
| Java | `checkstyle -c rules.xml src/ 2>&1` | 0 violations |

### G005 — FmtGate

```
Descripción:  El código sigue el formato estándar del lenguaje.
Bloqueante:   Configurable (por defecto: NO, es warning)
Tiempo:       ~5-15s
```

| Lenguaje | Comando |
|----------|---------|
| Rust | `cargo fmt --check 2>&1` |
| Python | `ruff format --check 2>&1` |
| JS/TS | `prettier --check "**/*.{js,ts}" 2>&1` |
| Go | `gofmt -l . 2>&1` y verificar salida vacía |

### G006 — SecurityGate

```
Descripción:  No hay vulnerabilidades conocidas en dependencias.
Bloqueante:   SIEMPRE
Tiempo:       ~5-30s
```

| Lenguaje | Comando | Qué verifica |
|----------|---------|-------------|
| Rust | `cargo audit 2>&1` | CVEs en RustSec advisory DB |
| Rust | `cargo deny check 2>&1` | Licencias, bans, duplicados |
| Python | `bandit -r src/ 2>&1` | Vulnerabilidades en código |
| Python | `pip-audit 2>&1` | CVEs en dependencias |
| JS/TS | `npm audit --audit-level=high 2>&1` | CVEs en dependencias |
| Go | `govulncheck ./... 2>&1` | CVEs en módulos |
| Java | `mvn dependency-check:check 2>&1` | OWASP DC |

### G007 — CoverageGate

```
Descripción:  La cobertura de tests supera un umbral mínimo.
Bloqueante:   Configurable (por defecto: NO)
Threshold:    Configurable (por defecto: 70%)
Tiempo:       ~30s-5min
```

```rust
CoverageGate {
    min_line_coverage: 0.70,
    min_branch_coverage: None,
    min_function_coverage: None,
}
```

### G008 — BuildSizeGate

```
Descripción:  El binario/bundle no crece más de un X% respecto al baseline.
Bloqueante:   Configurable (por defecto: NO)
Threshold:    Configurable (por defecto: 10%)
```

### G009 — ApiBreakGate

```
Descripción:  No hay cambios que rompan la API pública (semver).
Bloqueante:   Configurable (por defecto: NO, solo en fase Deploy)
Tiempo:       ~10-60s
```

| Lenguaje | Herramienta |
|----------|------------|
| Rust | `cargo semver-checks` |
| Python | Comparar `__all__` exports antes/después |
| JS/TS | `tsc --declaration` + diff de `.d.ts` |
| Java | `japicmp` (comparar JARs) |

### G010 — LicenseGate

```
Descripción:  Todas las dependencias tienen licencias permitidas.
Bloqueante:   Configurable (por defecto: NO)
```

```rust
LicenseGate {
    allowed_licenses: vec!["MIT", "Apache-2.0", "BSD-3-Clause", "ISC"],
    denied_licenses: vec!["GPL-3.0", "AGPL-3.0"],
}
```

---

## 3. Configuración Típica por Fase SDLC

| Gate | Planning | Coding | Testing | Deploy | Maintenance |
|------|----------|--------|---------|--------|-------------|
| CompilationGate | — | ✅ **B** | ✅ **B** | ✅ **B** | ✅ **B** |
| TestsGate | — | ✅ **B** | ✅ **B** | ✅ **B** | ✅ **B** |
| SyntaxGate | — | ✅ **B** | — | ✅ **B** | ✅ **B** |
| LintGate | — | ✅ **B** | ✅ **W** | ✅ **B** | ✅ **B** |
| FmtGate | — | ✅ **W** | ✅ **W** | ✅ **W** | ✅ **W** |
| SecurityGate | — | — | — | ✅ **B** | ✅ **B** |
| CoverageGate | — | — | ✅ **B** | ✅ **B** | ✅ **W** |
| BuildSizeGate | — | — | — | ✅ **W** | ✅ **W** |
| ApiBreakGate | — | — | — | ✅ **B** | — |
| LicenseGate | — | — | — | ✅ **W** | ✅ **W** |

**B** = Bloqueante (fallo → Health Score = 0)
**W** = Warning (fallo → se registra pero no bloquea)

---

## 4. Implementación de un Gate

```rust
use cognicode_autoresearch_sdk::prelude::*;

pub struct CompilationGate {
    adapter: Box<dyn ToolAdapter>,
}

impl QualityGate for CompilationGate {
    fn name(&self) -> &str { "compilation" }
    fn description(&self) -> &str { "Source code compiles without errors" }

    fn check(&self, ctx: &ProjectContext) -> Result<GateResult, GateError> {
        let start = Instant::now();
        let output = self.adapter.check_compilation(ctx)?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(GateResult {
            name: self.name().into(),
            passed: output.success,
            detail: Some(output.stdout.clone()),
            message: if output.success {
                None
            } else {
                Some(output.stderr.clone())
            },
            duration_ms,
        })
    }
}
```

---

## 5. Extensión: Añadir un Nuevo Gate

```rust
// 1. Implementar el trait
pub struct CustomGate {
    threshold: f64,
}

impl QualityGate for CustomGate {
    fn name(&self) -> &str { "custom_check" }
    fn description(&self) -> &str { "My custom quality check" }

    fn check(&self, ctx: &ProjectContext) -> Result<GateResult, GateError> {
        // Tu lógica aquí
        let value = compute_custom_metric(ctx)?;
        Ok(GateResult {
            name: self.name().into(),
            passed: value >= self.threshold,
            detail: Some(format!("value={:.2}, threshold={:.2}", value, self.threshold)),
            message: None,
            duration_ms: 0,
        })
    }
}

// 2. Registrar en el harness
let harness = Harness::new(
    HarnessConfig::for_rust_project(".")?
        .with_gate(Box::new(CustomGate { threshold: 0.8 }))
)?;
```

---

## Siguiente: [04 — Metrics Catalog](04-metrics.md)
