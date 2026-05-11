# 04 — Metrics Catalog

> Catálogo completo de Métricas de calidad. Valores normalizados [0.0-1.0].
> Se dividen en Deterministas (herramientas) y LLM-Assisted (criterio del modelo).

---

## 1. Taxonomía de Dimensiones

| Dimensión | Tipo | Descripción |
|-----------|------|-------------|
| Complexity | Determinista | Complejidad ciclomática, cognitiva, índice de mantenibilidad |
| Solid | Determinista | 5 principios SOLID medidos por proxy |
| Connascence | Determinista | Acoplamiento por nombre, tipo, algoritmo, posición |
| Smells | Determinista | Code smells en 3 niveles: arquitectura, diseño, implementación |
| Coverage | Determinista | Cobertura de tests (líneas, ramas, funciones) |
| Documentation | Determinista | API pública documentada |
| Security | Determinista | Hallazgos de análisis de seguridad |
| CleanCode | LLM-Assisted | Nombres, estructura, estilo |
| DesignQuality | LLM-Assisted | Calidad de diseño arquitectónico |

---

## 2. Métricas Deterministas

### M001 — CyclomaticComplexity

```
Dimensión: Complexity
Fuente:    get_complexity MCP tool o tree-sitter
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let avg_cc = average_cyclomatic_complexity(ctx)?;
    // Normalizar: 0 CC = score 1.0, 50+ CC = score 0.0
    let score = (1.0 - (avg_cc / 50.0)).clamp(0.0, 1.0);
    MetricValue { name: "cyclomatic_complexity", score, .. }
}
```

### M002 — CognitiveComplexity

```
Dimensión: Complexity
Fuente:    SonarQube-style analysis via tree-sitter
```

La complejidad cognitiva penaliza estructuras de control anidadas y operadores
de flujo, a diferencia de la ciclomática que solo cuenta caminos.

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let total_cognitive = sum_cognitive_complexity(ctx)?;
    let loc = count_lines_of_code(ctx)?;
    // Normalizar: densidad cognitiva por KLOC
    let density = total_cognitive as f64 / (loc as f64 / 1000.0);
    let score = (1.0 - (density / 25.0)).clamp(0.0, 1.0);
    MetricValue { name: "cognitive_complexity", score, .. }
}
```

### M003 — MaintainabilityIndex

```
Dimensión: Complexity
Fuente:    Halstead Volume + Cyclomatic Complexity + LOC
Fórmula:   MI = max(0, (171 − 5.2×ln(HV) − 0.23×CC − 16.2×ln(LOC)) × 100 / 171)
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let hv = halstead_volume(ctx)?;
    let cc = avg_cyclomatic_complexity(ctx)?;
    let loc = count_lines_of_code(ctx)?;

    let mi = (171.0 - 5.2 * hv.ln() - 0.23 * cc - 16.2 * (loc as f64).ln())
        .max(0.0) * 100.0 / 171.0;

    // Normalizar: >80 = excelente, <20 = pésimo
    let score = (mi / 80.0).clamp(0.0, 1.0);
    MetricValue { name: "maintainability_index", score, .. }
}
```

### M004 — SOLID: SRP

```
Dimensión: Solid
Proxy:     Responsabilidades por clase/módulo (fan-out normalizado)
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let graph = build_call_graph(ctx)?;
    // Contar dependencias salientes por símbolo
    let fan_outs: Vec<usize> = graph.symbols()
        .map(|s| graph.outgoing_count(&s.id))
        .collect();
    let avg_fan_out = fan_outs.iter().sum::<usize>() as f64 / fan_outs.len() as f64;
    let score = (1.0 - (avg_fan_out / 15.0)).clamp(0.0, 1.0);
    MetricValue { name: "solid_srp", score, .. }
}
```

### M005 — SOLID: OCP

```
Dimensión: Solid
Proxy:     Puntos de extensión vs modificaciones directas
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    // ¿Cuántos traits/interfaces hay vs cuántas dependencias directas a concretos?
    let abstractions = count_trait_impls(ctx)?;
    let concrete_deps = count_concrete_dependencies(ctx)?;
    let total = abstractions + concrete_deps;
    let score = if total > 0 {
        abstractions as f64 / total as f64
    } else {
        1.0
    };
    MetricValue { name: "solid_ocp", score, .. }
}
```

### M006 — SOLID: LSP

```
Dimensión: Solid
Proxy:     Profundidad del árbol de herencia
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let avg_depth = average_inheritance_depth(ctx)?;
    let score = (1.0 - (avg_depth / 5.0)).clamp(0.0, 1.0);
    MetricValue { name: "solid_lsp", score, .. }
}
```

### M007 — SOLID: ISP

```
Dimensión: Solid
Proxy:     Métodos por interfaz (interfaces pequeñas = mejor)
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let avg_methods = average_methods_per_interface(ctx)?;
    let score = (1.0 - (avg_methods / 10.0)).clamp(0.0, 1.0);
    MetricValue { name: "solid_isp", score, .. }
}
```

### M008 — SOLID: DIP

```
Dimensión: Solid
Proxy:     Dependencia de abstracciones vs implementaciones concretas
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let graph = build_call_graph(ctx)?;
    let trait_deps = graph.dependencies().filter(|d| d.is_abstraction()).count();
    let concrete_deps = graph.dependencies().filter(|d| !d.is_abstraction()).count();
    let total = trait_deps + concrete_deps;
    let score = if total > 0 {
        trait_deps as f64 / total as f64
    } else {
        1.0
    };
    MetricValue { name: "solid_dip", score, .. }
}
```

### M009 — Connascence: Name

```
Dimensión: Connascence
Proxy:     Strings literales compartidos entre módulos sin constante
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let instances = find_magic_strings(ctx)?;
    let score = (1.0 - (instances.len() as f64 / 100.0)).clamp(0.0, 1.0);
    MetricValue { name: "connascence_name", score, .. }
}
```

### M010 — Connascence: Algorithm

```
Dimensión: Connascence
Proxy:     Bloques de código duplicados (lógica repetida)
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let duplications = find_duplicated_blocks(ctx)?;
    let duplication_ratio = duplications.total_duplicated_lines as f64
        / duplications.total_lines as f64;
    let score = 1.0 - duplication_ratio;
    MetricValue { name: "connascence_algorithm", score, .. }
}
```

### M011 — Smells: Architecture

```
Dimensión: Smells
Proxy:     God components, cyclic dependencies, hub-like dependencies
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let arch_smells = detect_architecture_smells(ctx)?;
    // God components: símbolos con >20 dependencias entrantes
    // Cyclic deps: ciclos detectados por Tarjan SCC
    // Hub-like: símbolos con fan-in y fan-out ambos >10
    let total = arch_smells.len() as f64;
    let score = (1.0 - (total / 20.0)).clamp(0.0, 1.0);
    MetricValue { name: "smells_architecture", score, .. }
}
```

### M012 — Smells: Design

```
Dimensión: Smells
Proxy:     Feature envy, inappropriate intimacy, shotgun surgery, divergent change
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let design_smells = detect_design_smells(ctx)?;
    // Feature envy: método que accede más a otra clase que a la propia
    // Inappropriate intimacy: clases que acceden a campos privados de otras
    let total = design_smells.len() as f64;
    let score = (1.0 - (total / 20.0)).clamp(0.0, 1.0);
    MetricValue { name: "smells_design", score, .. }
}
```

### M013 — Smells: Implementation

```
Dimensión: Smells
Proxy:     Long method, long parameter list, deep nesting, commented code
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let impl_smells = detect_implementation_smells(ctx)?;
    // Long method: >30 líneas
    // Long parameter list: >5 parámetros
    // Deep nesting: >4 niveles
    // Commented code: bloques de código comentados
    let total = impl_smells.len() as f64;
    let score = (1.0 - (total / 30.0)).clamp(0.0, 1.0);
    MetricValue { name: "smells_implementation", score, .. }
}
```

### M014 — Coverage

```
Dimensión: Coverage
Fuente:    cargo llvm-cov / pytest-cov / jest --coverage / c8
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let coverage = measure_code_coverage(ctx)?;
    MetricValue {
        name: "coverage",
        score: coverage.line_coverage, // ya en [0.0, 1.0]
        raw_values: Some(json!({
            "line": coverage.line_coverage,
            "branch": coverage.branch_coverage,
            "function": coverage.function_coverage,
        })),
    }
}
```

### M015 — Documentation

```
Dimensión: Documentation
Proxy:     Porcentaje de API pública con doc comments
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let public_api = count_public_api_symbols(ctx)?;
    let documented = count_documented_symbols(ctx)?;
    let score = if public_api > 0 {
        documented as f64 / public_api as f64
    } else {
        1.0
    };
    MetricValue { name: "documentation", score, .. }
}
```

### M016 — Security

```
Dimensión: Security
Fuente:    cargo audit / bandit / npm audit
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let findings = security_audit(ctx)?;
    let score = 1.0
        - findings.critical as f64 * 0.2
        - findings.high as f64 * 0.1
        - findings.medium as f64 * 0.05;
    MetricValue { name: "security", score: score.clamp(0.0, 1.0), .. }
}
```

---

## 3. Métricas LLM-Assisted

Estas métricas usan un LLM para evaluar aspectos cualitativos del código.
Tienen **peso bajo** en el Health Score (5-10%) y siempre incluyen un
`confidence` score. Si divergen >20% de las métricas deterministas
equivalentes, se marcan para revisión humana.

### M017 — CleanCode (LLM)

```
Dimensión: CleanCode
Fuente:    LLM (MiniMax, Claude, GPT-4o) — revisión de estilo
```

```rust
fn evaluate(&self, ctx: &ProjectContext) -> MetricValue {
    let prompt = format!(
        "Review the following code for clean code principles. \
         Rate naming quality, function length, comment quality, \
         and overall readability on a scale of 0-100.\n\n{}",
        sample_code_snippets(ctx)?
    );
    let response = self.llm.ask(&prompt)?;
    let score = parse_numeric_score(&response)?;
    MetricValue {
        name: "llm_clean_code",
        score: score / 100.0,
        source: MetricSource::LlmAssisted,
        confidence: Some(self.llm.confidence()),
    }
}
```

### M018 — DesignQuality (LLM)

```
Dimensión: DesignQuality
Fuente:    LLM — evaluación de decisiones de diseño
```

Evalúa:
- ¿La arquitectura es apropiada para el dominio?
- ¿Los patrones de diseño están bien aplicados?
- ¿Las dependencias fluyen en la dirección correcta?

---

## 4. Tabla Resumen

| ID | Métrica | Tipo | Peso sugerido | Tiempo |
|----|---------|------|---------------|--------|
| M001 | Cyclomatic Complexity | Det | 5% | <1s |
| M002 | Cognitive Complexity | Det | 5% | <1s |
| M003 | Maintainability Index | Det | 5% | <1s |
| M004 | SOLID: SRP | Det | 3% | <1s |
| M005 | SOLID: OCP | Det | 3% | <1s |
| M006 | SOLID: LSP | Det | 3% | <1s |
| M007 | SOLID: ISP | Det | 3% | <1s |
| M008 | SOLID: DIP | Det | 3% | <1s |
| M009 | Connascence: Name | Det | 5% | <5s |
| M010 | Connascence: Algorithm | Det | 5% | <5s |
| M011 | Smells: Architecture | Det | 7% | <5s |
| M012 | Smells: Design | Det | 7% | <5s |
| M013 | Smells: Implementation | Det | 6% | <5s |
| M014 | Coverage | Det | 15% | 30s-5min |
| M015 | Documentation | Det | 5% | <1s |
| M016 | Security | Det | 10% | 5-30s |
| M017 | CleanCode | LLM | 7% | 5-30s |
| M018 | DesignQuality | LLM | 3% | 5-30s |

---

## Siguiente: [05 — Health Score](05-health-score.md)
