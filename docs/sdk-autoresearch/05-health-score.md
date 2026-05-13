# 05 — Health Score

> El Health Score es la métrica de verdad absoluta del sistema. Equivalente al
> `evaluate_bpb()` de Karpathy: **inmutable para el agente, determinista en su
> cálculo, compuesto por Gates ponderados + Métricas ponderadas.**

---

## 1. Fórmula

```
Health Score = GATES_FLAG × Σ( W_i × metric_i )

Donde:
  GATES_FLAG ∈ {0, 1}
    = 0 si cualquier Gate bloqueante falla
    = 1 si todos los Gates bloqueantes pasan

  W_i = peso de la métrica i
    Σ W_i = 1.0
    Cada W_i está asociado a un QualityDimension

  metric_i ∈ [0.0, 1.0]
    Valor normalizado de la métrica i
```

---

## 2. Ejemplo de Cálculo

### Estado del Proyecto

```
GATES:
  ✅ compilation   (bloqueante)
  ✅ tests         (bloqueante, 837/837)
  ✅ lint          (bloqueante, 12 warnings)
  ✅ syntax        (bloqueante)
  ⚠️  fmt          (warning, 3 archivos sin formatear)

→ GATES_FLAG = 1 (todos los bloqueantes pasan)

MÉTRICAS:
  complexity      = 0.820 × 0.15 = 0.123
  solid_srp       = 0.640 × 0.03 = 0.019
  solid_ocp       = 0.550 × 0.03 = 0.016
  solid_lsp       = 0.900 × 0.03 = 0.027
  solid_isp       = 0.720 × 0.03 = 0.022
  solid_dip       = 0.600 × 0.03 = 0.018
  connascence_name    = 0.710 × 0.05 = 0.035
  connascence_algo    = 0.680 × 0.05 = 0.034
  smells_arch     = 0.580 × 0.07 = 0.041
  smells_design   = 0.520 × 0.07 = 0.036
  smells_impl     = 0.660 × 0.06 = 0.040
  coverage        = 0.720 × 0.15 = 0.108
  documentation   = 0.550 × 0.05 = 0.028
  security        = 0.900 × 0.10 = 0.090
  llm_clean_code  = 0.750 × 0.07 = 0.052
  llm_design      = 0.680 × 0.03 = 0.020

HEALTH SCORE = 1.0 × 0.709 = 0.709
```

### Desglose Visual

```
Health Score: 0.709
  ├─ Complexity:    0.820 ─███████████████████████████████████████░░░░░░░░ 0.123
  ├─ SOLID:         0.682 ─██████████████████████████████████░░░░░░░░░░░░░ 0.102
  ├─ Connascence:   0.695 ─███████████████████████████████████░░░░░░░░░░░░ 0.069
  ├─ Smells:        0.587 ─█████████████████████████████░░░░░░░░░░░░░░░░░░ 0.117
  ├─ Coverage:      0.720 ─██████████████████████████████████████░░░░░░░░░ 0.108
  ├─ Documentation: 0.550 ─███████████████████████████░░░░░░░░░░░░░░░░░░░░ 0.028
  ├─ Security:      0.900 ─█████████████████████████████████████████████░░ 0.090
  ├─ CleanCode:     0.750 ─████████████████████████████████████████░░░░░░░ 0.052
  └─ DesignQuality: 0.680 ─████████████████████████████████████░░░░░░░░░░░ 0.020
```

---

## 3. Pesos por Defecto (Fase Maintenance)

| Dimensión | Peso | Justificación |
|-----------|------|---------------|
| Complexity | 15% | Base de la calidad, bien cubierta por herramientas |
| Solid (5 × 3%) | 15% | Principios de diseño fundamentales |
| Connascence (2 × 5%) | 10% | Acoplamiento, la raíz de muchos bugs |
| Smells (3 niveles) | 20% | Detecta problemas concretos accionables |
| Coverage | 15% | Cobertura de tests da confianza para refactors |
| Documentation | 5% | Important pero secundario a funcionalidad |
| Security | 10% | Crítico para producción |
| CleanCode (LLM) | 7% | Criterio cualitativo, peso bajo |
| DesignQuality (LLM) | 3% | Criterio cualitativo, peso muy bajo |

---

## 4. Pesos por Fase SDLC

| Dimensión | Coding | Testing | Deploy | Maintenance |
|-----------|--------|---------|--------|-------------|
| Complexity | 25% | 10% | 5% | 15% |
| Solid | 20% | 10% | 5% | 15% |
| Connascence | 10% | 10% | 5% | 10% |
| Smells | 20% | 10% | 5% | 20% |
| Coverage | 10% | 35% | 15% | 15% |
| Documentation | 5% | — | 5% | 5% |
| Security | — | — | 40% | 10% |
| CleanCode (LLM) | 10% | 20% | 10% | 7% |
| DesignQuality (LLM) | — | 5% | 10% | 3% |

---

## 5. SAGA Rebalancing (Nivel 2)

Cada 50 iteraciones, SAGA analiza los resultados y propone ajustar los pesos.
El objetivo: maximizar el `delta_health` esperado por iteración.

### Algoritmo

```rust
pub fn rebalance(
    current_weights: &HashMap<QualityDimension, f64>,
    recent_iterations: &[Iteration],
) -> WeightProposal {
    // 1. Calcular ganancias por dimensión
    let mut gains: HashMap<QualityDimension, Vec<f64>> = HashMap::new();
    for iter in recent_iterations {
        for (dim, delta) in &iter.component_deltas {
            gains.entry(*dim).or_default().push(*delta);
        }
    }

    // 2. Calcular headroom por dimensión
    let current_scores = latest_health_score()?.metric_values;
    let headroom: HashMap<QualityDimension, f64> = current_scores
        .iter()
        .map(|m| (m.dimension, 1.0 - m.score))
        .collect();

    // 3. Calcular eficiencia por dimensión
    let efficiency: HashMap<QualityDimension, f64> = gains
        .iter()
        .map(|(dim, deltas)| {
            let avg_gain = deltas.iter().sum::<f64>() / deltas.len() as f64;
            (*dim, avg_gain)
        })
        .collect();

    // 4. Proponer nuevos pesos:
    //    W'_i = headroom_i × efficiency_i
    //    normalizado para que Σ W' = 1.0
    let mut raw_weights: HashMap<QualityDimension, f64> = headroom
        .iter()
        .map(|(dim, hr)| {
            let eff = efficiency.get(dim).copied().unwrap_or(0.01);
            (*dim, hr * eff)
        })
        .collect();

    let total: f64 = raw_weights.values().sum();
    let proposed: HashMap<QualityDimension, f64> = raw_weights
        .iter()
        .map(|(dim, w)| (*dim, w / total))
        .collect();

    WeightProposal {
        current: current_weights.clone(),
        proposed,
        justification: format!(
            "Rebalance based on {} iterations: headroom × efficiency",
            recent_iterations.len()
        ),
        metrics: WeightProposalMetrics {
            iterations_analyzed: recent_iterations.len(),
            avg_gain_per_iteration: /* ... */,
            dimensions_stagnant: /* ... */,
            dimensions_active: /* ... */,
        },
    }
}
```

### Reglas de Rebalanceo

- **Si una dimensión está >0.90**: reducir su peso al 50% del actual
- **Si una dimensión está <0.40**: aumentar su peso al 150% del actual
- **Si una dimensión no ha mejorado en 20 iteraciones**: reducir peso
- **Si una dimensión genera mejoras consistentes**: mantener o aumentar
- **Nunca eliminar una dimensión**: peso mínimo = 0.02

### Formato de Propuesta

```markdown
# SAGA Weight Proposal — Iteration 250

## Analysis
- Analyzed: 50 iterations (201-250)
- Average health gain per iteration: 0.003
- Most improved dimension: Smells (+0.12 in 50 iters)
- Stagnant dimension: Documentation (no improvement in 20 iters)

## Proposed Changes
| Dimension | Current | Proposed | Change | Reason |
|-----------|---------|----------|--------|--------|
| Smells | 20% | 25% | +5% | Active improvement area, headroom 0.42 |
| Documentation | 5% | 3% | -2% | Stagnant, no gains in 20 iters |
| Security | 10% | 12% | +2% | Headroom 0.30 remains |

## Expected Impact
Projected health gain per iteration: 0.003 → 0.004 (+33%)

## Action Required
⚠️ HUMAN REVIEW NEEDED. Apply with: `saga apply proposals/saga_0250.md`
```

---

## 6. API del Health Score

```rust
impl HealthScore {
    /// Zero score (all gates failed)
    pub fn zero_with_gates(gate_results: Vec<GateResult>) -> Self;

    /// Calculate from metric values and weights
    pub fn calculate(
        metric_values: Vec<WeightedMetric>,
        gate_results: Vec<GateResult>,
    ) -> Self;

    /// Compare with another health score
    pub fn delta(&self, other: &HealthScore) -> f64;

    /// Format as colored terminal output
    pub fn display(&self) -> String;

    /// Serialize to JSON for MCP response
    pub fn to_mcp_response(&self) -> serde_json::Value;

    /// Get the breakdown by dimension
    pub fn breakdown(&self) -> HashMap<QualityDimension, f64>;
}
```

---

## 7. Configuración de Pesos

Los pesos se configuran en `program.md` o vía MCP:

```yaml
# En program.md
health_score:
  weights:
    complexity: 0.15
    solid: 0.15
    connascence: 0.10
    smells: 0.20
    coverage: 0.15
    documentation: 0.05
    security: 0.10
    clean_code: 0.07
    design_quality: 0.03
```

O programáticamente:

```rust
let harness = Harness::new(
    HarnessConfig::for_rust_project(".")?
        .with_weights(weights!{
            Complexity => 0.20,
            Solid => 0.15,
            Coverage => 0.30,
            // ... rest auto-distributed
        })
)?;
```

---

## Siguiente: [06 — SDLC Mapping](06-sdlc-mapping.md)
