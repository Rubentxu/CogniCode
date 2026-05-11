# 09 — program.md Reference

> Especificación completa del DSL `program.md`. Este archivo Markdown es lo que
> el humano edita para "programar" al agente. Es el equivalente directo al
> `program.md` de Karpathy, extendido para el SDLC completo.

---

## 1. Propósito

`program.md` es el **código fuente de la política del agente**. Define qué hacer,
cómo hacerlo, qué no hacer, cuándo parar, y qué considerar un éxito. Es lenguaje
natural estructurado, no código procedural. El agente lo lee al inicio de cada
sesión y sigue sus instrucciones.

> *"La idea central es que no estás tocando ninguno de los archivos Python como
> lo haría normalmente un investigador. En cambio, estás programando los archivos
> `program.md` que proporcionan contexto a los agentes de IA."*
> — Andrej Karpathy

---

## 2. Especificación de Secciones

### Sección 1: OBJECTIVE

```markdown
## Objective

Improve the Health Score of this project by making autonomous code changes.
The Health Score is a composite metric [0.0-1.0] measuring code quality across:
complexity, SOLID principles, connascence, code smells, test coverage,
documentation, security, and clean code practices.
```

**Obligatorio**: SÍ
**Formato**: Texto libre describiendo el objetivo general.

### Sección 2: SCOPE

```markdown
## Scope

### Files you CAN modify
- crates/**/*.rs (all Rust source files)
- tests/**/*.rs (test files)
- Cargo.toml (only to adjust versions within semver range)

### Files you CANNOT modify (EVER)
- harness/ (evaluation harness is sacred)
- Cargo.lock (auto-generated)
- .github/ (CI configuration)
- docs/sdk-autoresearch/ (this documentation)
```

**Obligatorio**: SÍ
**Formato**: Dos listas: `CAN modify` y `CANNOT modify`.

### Sección 3: PROTOCOL

```markdown
## Protocol

You must follow this exact protocol for every iteration:

### Step 1: Evaluate Baseline
```
autoresearch_evaluate(project_dir=".") → health_before
Record the health_before value and its breakdown.
```

### Step 2: Analyze and Choose Target
Look at the breakdown from Step 1:
- Which component has the LOWEST score?
- Which component has the HIGHEST weight?
- What changes have been tried before? (check git log)

```
autoresearch_suggest(project_dir=".", focus="lowest_scoring_component")
```

### Step 3: Formulate Hypothesis
State your hypothesis: "Changing X will improve Y by Z points."

### Step 4: Implement the Change
```
autoresearch_propose(suggestion_id="...")
# Then Edit the targeted files with the MINIMUM change needed
```

### Step 5: Pre-Validate
```
autoresearch_gates(project_dir=".")
```
If ANY gate fails → fix trivial issues or DISCARD immediately.

### Step 6: Commit
```
git add -A
git commit -m "experiment: <concise one-line description>"
```

### Step 7: Full Evaluation
```
autoresearch_evaluate(project_dir=".") → health_after
```

### Step 8: Decide
```
autoresearch_decide(
    health_before=<value>,
    health_after=<value>,
    change_description="...",
    lines_added=<N>,
    lines_removed=<N>
) → KEEP or DISCARD
```
If KEEP: the commit stays.
If DISCARD: git reset --hard HEAD~1.

### Step 9: Log
Append result to results.tsv.

### Step 10: Repeat from Step 1
```

**Obligatorio**: SÍ
**Formato**: 10 pasos numerados con comandos concretos.

### Sección 4: GATES

```markdown
## Quality Gates

These checks MUST pass before metrics are calculated.
If any blocking gate fails → Health Score = 0 → DISCARD immediately.

### Blocking Gates (always)
- compilation: code compiles without errors
- tests: all existing tests pass
- syntax: all regex/tree-sitter queries are valid

### Warning Gates (informative only)
- lint: no clippy/ruff/eslint warnings
- format: consistent formatting
```

**Obligatorio**: SÍ
**Formato**: Lista de gates con descripción.

### Sección 5: METRICS

```markdown
## Metrics

These contribute to the Health Score.

### Deterministic Metrics
| Metric | Weight | Description |
|--------|--------|-------------|
| complexity | 0.15 | Cyclomatic + cognitive + maintainability index |
| solid | 0.15 | 5 SOLID principles (3% each) |
| connascence | 0.10 | Name + algorithm coupling |
| smells | 0.20 | Architecture + design + implementation smells |
| coverage | 0.15 | Test coverage (lines, branches, functions) |
| documentation | 0.05 | Public API documentation coverage |
| security | 0.10 | Dependency vulnerabilities |

### LLM-Assisted Metrics
| Metric | Weight | Description |
|--------|--------|-------------|
| clean_code | 0.07 | Code style, naming, readability review |
| design_quality | 0.03 | Architecture and design pattern review |
```

**Obligatorio**: SÍ
**Formato**: Tabla con métricas y pesos.

### Sección 6: DECISION_RULES

```markdown
## Decision Rules (Karpathy Simplicity Criterion)

| Condition | Decision |
|-----------|----------|
| health_after > health_before by ≥ 0.001 | KEEP |
| health_after ≈ health_before AND code got simpler (≥5 net lines removed) | KEEP |
| health_after ≈ health_before AND no simplicity gain | DISCARD |
| health_after < health_before | DISCARD |
| ANY blocking gate fails | DISCARD immediately |
```

**Obligatorio**: SÍ
**Formato**: Tabla de condiciones → decisiones.

### Sección 7: NEVER_DO

```markdown
## Anti-Patterns (NEVER DO)

- NEVER change the same threshold back and forth (check git log first)
- NEVER make changes that affect the evaluation harness
- NEVER add new crate dependencies without human approval
- NEVER modify Cargo.toml dependency versions outside semver range
- NEVER skip pre-validation — it wastes time on doomed experiments
- NEVER modify more than 50 lines in a single iteration
- NEVER "fix" a failing test by deleting it or commenting it out
- NEVER modify program.md or this file
```

**Obligatorio**: SÍ
**Formato**: Lista de prohibiciones.

### Sección 8: ESCALATION

```markdown
## When to Escalate to Human

Write a proposal file and STOP in these cases:

1. SAGA suggests weight changes → write proposals/saga_weights_NNNN.md
2. Meta-analysis suggests protocol changes → write proposals/protocol_NNNN.md
3. Dependency update available with security fix → write proposals/dep_update.md
4. 10 consecutive DISCARD iterations → write proposals/stuck.md
5. Architecture change needed (affects >3 crates) → write proposals/arch_NNNN.md
6. New language support needed → write proposals/new_lang.md
```

**Obligatorio**: SÍ
**Formato**: Lista de condiciones de escalación.

### Sección 9: MULTI_AGENT (Opcional)

```yaml
multi_agent:
  enabled: true
  merge_strategy: tournament
  sync_interval: 25
  agents:
    - name: rules-health
      branch: auto/rules-health
      focus: [smells, solid]
      max_iterations: 50
```

### Sección 10: SDLC_PHASES (Opcional)

```yaml
sdlc_phases:
  planning:
    autonomy: ai_assisted
  coding:
    autonomy: full_auto
  testing:
    autonomy: full_auto
  deployment:
    autonomy: ai_led
    require_human_approval: true
  maintenance:
    autonomy: full_auto
```

---

## 3. Ejemplo Completo

```markdown
# CogniCode Autonomous Improvement Program

## Objective
Improve the CogniCode static analysis platform Health Score autonomously.

## Scope
### CAN modify
- crates/cognicode-axiom/src/**/*.rs
- crates/cognicode-core/src/**/*.rs
- crates/cognicode-mcp/src/**/*.rs
- crates/cognicode-sandbox/src/**/*.rs
- tests/**/*.rs

### CANNOT modify
- harness/ (sacred)
- Cargo.lock
- .github/

## Protocol
[10 steps as defined above]

## Quality Gates
- compilation: cargo check
- tests: cargo test --workspace
- syntax: all regex/tree-sitter queries valid
- lint: cargo clippy -- -D warnings

## Metrics
| Metric | Weight |
|--------|--------|
| complexity | 0.15 |
| solid | 0.15 |
| connascence | 0.10 |
| smells | 0.20 |
| coverage | 0.15 |
| security | 0.10 |
| clean_code (LLM) | 0.10 |
| design_quality (LLM) | 0.05 |

## Decision Rules
[as defined above]

## Anti-Patterns
[as defined above]

## Escalation
[as defined above]
```

---

## 4. Validación del program.md

```rust
pub struct ProgramMdValidator;

impl ProgramMdValidator {
    pub fn validate(content: &str) -> Result<Vec<ValidationWarning>> {
        let mut warnings = Vec::new();

        // Validar secciones obligatorias
        let required = &["Objective", "Scope", "Protocol", "Quality Gates",
                         "Metrics", "Decision Rules", "Anti-Patterns"];
        for section in required {
            if !content.contains(&format!("## {}", section)) {
                warnings.push(ValidationWarning {
                    severity: Severity::Error,
                    message: format!("Missing required section: {}", section),
                });
            }
        }

        // Validar que los pesos suman ~1.0
        let weights = extract_weights(content);
        let total: f64 = weights.values().sum();
        if (total - 1.0).abs() > 0.01 {
            warnings.push(ValidationWarning {
                severity: Severity::Warning,
                message: format!("Metrics weights sum to {:.2} (should be 1.0)", total),
            });
        }

        // Validar que no hay gates duplicados
        // ...

        warnings
    }
}
```

---

## 5. Evolución del program.md

El propio `program.md` puede mejorar con el tiempo. El meta-agente (Nivel 3)
analiza la eficiencia del protocolo y propone cambios:

```
Iteración 200:
  Meta-agent detecta: 30% de los DISCARD son por "lint gate failed"
  Propone: añadir lint gate al paso 5 (pre-validate) para detectar antes
  Ahorro estimado: ~45 minutos por cada 100 iteraciones

Iteración 400:
  Meta-agent detecta: sugerencias de "pattern_extend" tienen 40% keep rate,
  "threshold_tune" solo 15%
  Propone: añadir guía al protocolo priorizando pattern_extend sobre threshold_tune

Iteración 600:
  Meta-agent detecta: el agente pasa 30% del tiempo en el paso 2 (analyze)
  Propone: añadir caché de análisis por componente para evitar re-evaluar
```

---

## Siguiente: [10 — Implementation Plan](10-implementation-plan.md)
