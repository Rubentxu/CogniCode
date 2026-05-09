# Framework de Métricas y Evaluación

## 1. KPIs Multi-Nivel

### 1.1 Nivel Regla (Rule-Level)

Evalúa efectividad individual de cada regla.

| KPI | Fórmula | Ideal | Aceptable | Pobre |
|-----|---------|-------|-----------|-------|
| **Precision** | `TP / (TP + FP)` | ≥ 0.95 | ≥ 0.85 | < 0.85 |
| **Recall** | `TP / (TP + FN)` | ≥ 0.90 | ≥ 0.75 | < 0.75 |
| **F1 Score** | `2 × (P × R) / (P + R)` | ≥ 0.92 | ≥ 0.80 | < 0.80 |
| **False Positive Rate** | `FP / (FP + TN)` | < 0.01 | < 0.05 | ≥ 0.05 |
| **Execution Time** | `ms / KLOC` | < 50 | < 200 | ≥ 200 |
| **Issue Density** | `issues / KLOC` | — | — | > 2σ del mean |

**Casos edge**:
- `TP + FP = 0`: Precision undefined → `null`, excluir de agregados
- `TP + FN = 0`: Recall undefined → `null`, excluir de agregados

### 1.2 Nivel Fichero (File-Level)

Evalúa salud del código a granularidad de archivo.

| KPI | Umbrales |
|-----|----------|
| **Issues per File** | > 20 warning, > 50 critical |
| **Cyclomatic Complexity** | < 10 good, 10-20 warning, > 20 critical |
| **Cognitive Complexity** | < 15 good, 15-30 warning, > 30 critical |
| **Technical Debt Ratio** | > 5% warning, > 15% critical |
| **Duplication %** | > 3% warning, > 10% critical |

### 1.3 Nivel Proyecto (Project-Level)

Ratings A-E compatibles con SonarQube.

| Rating | Maintainability | Security | Reliability |
|--------|-----------------|----------|-------------|
| **A** | TDR < 3%, CC avg < 5 | 0 vulns | < 0.1 bugs/KLOC |
| **B** | TDR < 5%, CC avg < 8 | < 0.1/KLOC | < 0.3/KLOC |
| **C** | TDR < 10%, CC avg < 12 | < 0.5/KLOC | < 1.0/KLOC |
| **D** | TDR < 20%, CC avg < 20 | < 1.0/KLOC | < 3.0/KLOC |
| **E** | TDR ≥ 20% | ≥ 1.0/KLOC | ≥ 3.0/KLOC |

### 1.4 Nivel Ecosistema (Ecosystem-Level)

Evalúa efectividad del sistema de reglas completo.

| KPI | Fórmula | Target |
|-----|---------|--------|
| **Signal-to-Noise Ratio (SNR)** | `TP / (TP + FP + FN)` | > 0.70 |
| **Rule Effectiveness Score (RES)** | `fixed_issues / reported_issues` | > 60% |
| **Developer Adoption Rate (DAR)** | `projects_using / total_projects` | > 50% |
| **Mean Time to Fix (MTTF)** | `median(closed - created)` | < 24h (P0), < 7d (P1) |

---

## 2. Estrategias de Ground Truth

Cuatro estrategias complementarias. **Todas deben usarse en conjunto** — ninguna es suficiente sola.

### 2.1 Multi-Tool Consensus (Estrategia A)

```
3+ herramientas independientes → Consenso → Clasificación

┌──────────┐  ┌──────────┐  ┌──────────┐
│SonarQube │  │ ESLint   │  │ CogniCode│
└────┬─────┘  └────┬─────┘  └────┬─────┘
     │              │              │
     └──────────────┼──────────────┘
                    │
                    ▼
          ┌─────────────────┐
          │ CONSENSUS ENGINE │
          │ Match → Classify │
          └────────┬─────────┘
                   │
      ┌────────────┼────────────┐
      ▼            ▼            ▼
   3/3 tools    2/3 tools    1/3 tools
   Strong TP    Likely TP    Human Review
```

**Acuerdo mínimo**: 2/3 herramientas para clasificar como "Likely TP".

**Pesos por herramienta** (calibrados trimestralmente):

| Herramienta | Peso |
|-------------|------|
| SonarQube | 0.30 |
| ESLint / Clippy / Ruff | 0.25 |
| CogniCode | 0.20 |
| SpotBugs / staticcheck | 0.15 |

### 2.2 Historical Git Mining (Estrategia B)

| Patrón en git history | Clasificación |
|----------------------|---------------|
| Línea cambiada en bug-fix commit | **TP** (el issue era real) |
| Línea intacta en 50+ commits | **FP** probable |
| Issue marcado "won't fix" | **FP** (decisión de diseño) |
| Issue marcado "false positive" | **FP** (error de herramienta) |
| Issue desaparece tras refactor | **TP** (era real, ya irrelevante) |

**Time Decay**: `survival_score = e^(-0.01 × days)` — issues viejos sin tocar → FP probable.

### 2.3 Manual Annotation (Estrategia C)

**Golden Corpus**: 500 archivos anotados manualmente, 3 expertos por archivo.

| Métrica | Valor |
|---------|-------|
| Anotadores por archivo | 3 |
| Inter-rater reliability | Krippendorf α |
| α ≥ 0.80 | Usar majority vote |
| α < 0.67 | Retrain, revisar schema |
| Actualización | Trimestral (+50 archivos, -50 viejos) |
| Rotación máxima | 18 meses sin re-anotación |

### 2.4 Differential Testing (Estrategia D)

Compara versiones OLD vs NEW de una regla:

| OLD → NEW | Interpretación |
|-----------|---------------|
| TP → TP | Sin cambio (correcto) |
| TP → FP | **Regresión** (perdimos un hallazgo real) |
| FP → TP | **Mejora** (encontramos algo nuevo) |
| FN → TP | **Mejora mayor** (detectamos lo que antes no) |
| TP → FN | **Regresión mayor** (perdimos un hallazgo real) |

**Improvement Score**: `ΔTPR - 2.0 × ΔFPR` (FPs cuestan 2× más que perder TPs)

---

## 3. Pipeline de Extracción

```
┌────────────────────────────────────────────────────────────────┐
│ PHASE 1: CORPUS PREPARATION                                     │
│   Clone repo @ git hash → Filter files → Extract metadata      │
├────────────────────────────────────────────────────────────────┤
│ PHASE 2: MULTI-TOOL ANALYSIS                                    │
│   SonarQube Scanner │ ESLint │ Clippy │ Ruff │ CogniCode       │
├────────────────────────────────────────────────────────────────┤
│ PHASE 3: NORMALIZATION                                          │
│   Rule ID mapping → Severity normalization → Location normalize │
├────────────────────────────────────────────────────────────────┤
│ PHASE 4: CONSENSUS CLASSIFICATION                              │
│   Match by (file, line±1, rule_id) → Classify TP/FP/FN/TN     │
├────────────────────────────────────────────────────────────────┤
│ PHASE 5: METRICS COMPUTATION                                    │
│   Rule metrics → File metrics → Project metrics → Ecosystem     │
├────────────────────────────────────────────────────────────────┤
│ PHASE 6: STORAGE                                               │
│   SQLite (time series) + JSONL (raw replay) + TSV (audit log)  │
└────────────────────────────────────────────────────────────────┘
```

### 3.1 File Filtering

**Excluir**:
- `*_test.*`, `*.test.*` (test files)
- `*.min.js`, `*.bundle.js` (generated)
- `vendor/`, `node_modules/`, `target/` (dependencies)
- `*.pb.go`, `*.g.cs` (generated code)

### 3.2 Severity Normalization

| SonarQube | CogniCode | ESLint | Clippy | Normalized |
|-----------|-----------|--------|--------|------------|
| BLOCKER | CRITICAL | error | error | **P0** |
| CRITICAL | HIGH | error | error | **P1** |
| MAJOR | MEDIUM | warn | warning | **P2** |
| MINOR | LOW | info | note | **P3** |
| INFO | INFO | info | help | **P4** |

### 3.3 Rule ID Canonicalization

Formato canónico: `S{NUMBER}` (convención SonarQube)

**Tiers de mapping**:

| Tier | Definición | Confianza | Ejemplo |
|------|-----------|-----------|---------|
| **T1: Exact** | IDs equivalentes | 1.0 | `S2068` = `S2068` |
| **T2: Semantic** | Misma categoría | 0.85 | `S5122` = `sql-injection` |
| **T3: Fuzzy** | > 80% similar | 0.70 | `S107` ≈ `max-params` |
| **T4: Unmapped** | Sin equivalente | N/A | — |

---

## 4. Herramientas por Lenguaje

| Lenguaje | Primaria | Secundaria | Terciaria |
|----------|----------|------------|-----------|
| **Rust** | Clippy (0.40) | CogniCode (0.30) | SonarQube (0.20) |
| **Python** | Ruff (0.35) | Pylint (0.30) | CogniCode (0.25) |
| **JavaScript** | ESLint (0.40) | SonarQube (0.30) | CogniCode (0.20) |
| **TypeScript** | TS ESLint (0.40) | SonarQube (0.30) | CogniCode (0.20) |
| **Java** | SonarQube (0.50) | SpotBugs (0.30) | CogniCode (0.15) |
| **Go** | staticcheck (0.45) | golangci-lint (0.30) | CogniCode (0.15) |

*Pesos entre paréntesis = weight en consensus engine*

---

## 5. Health Score (Decisión Keep/Discard)

### Fórmula

```
Health Score = 0.35×F1 + 0.25×SNR + 0.20×RES + 0.10×DAR - 0.10×relative_cost

where:
  F1 = harmonic mean of precision and recall
  SNR = Signal-to-Noise Ratio
  RES = Rule Effectiveness Score
  DAR = Developer Adoption Rate
  relative_cost = execution_time / baseline_execution_time
```

### Umbrales de Decisión

| Health Score | Clasificación | Acción |
|--------------|---------------|--------|
| ≥ 0.80 | **Healthy** | KEEP — optimizaciones menores OK |
| 0.60 — 0.79 | **Acceptable** | MONITOR — mejorar KPI más débil |
| 0.40 — 0.59 | **At Risk** | IMPROVE — plan de mejora o probation |
| < 0.40 | **Discard Candidate** | DISCARD — salvo regla estratégica |

### Árbol de Decisión

```
                 ┌──────────────┐
                 │ F1 ≥ 0.50?   │
                 └──────┬───────┘
                        │
                  YES   │   NO
                  ┌─────┴─────┐
                  ▼           ▼
             ┌─────────┐  ┌──────────┐
             │ SNR ≥   │  │ 6-month  │
             │ 0.40?   │  │ probation│
             └────┬────┘  └────┬─────┘
                  │            │
                YES │          │
                  ┌┴──────────┴┐
                  ▼            ▼
             ┌─────────┐  ┌──────────┐
             │Health   │  │Improve to│
             │≥ 0.60?  │  │≥ 0.60?   │
             └────┬────┘  └────┬─────┘
                  │            │
               YES │       YES │  NO
                ┌─┴──┐     ┌──┴──┐
                ▼    ▼     ▼     ▼
              KEEP  MONITOR KEEP DISCARD
```

### Casos Especiales

| Escenario | Regla |
|-----------|-------|
| F1 = 0, regla única sin equivalente | KEEP en perfil "experimental" |
| Alta precisión, recall ≈ 0 | Mejorar detección (demasiado estricta) |
| Alto recall, precisión = 0.50 | Auditoría inmediata de FPs |
| Regla nueva (< 3 meses) | Umbrales ×0.80 (conservador) |
| Regla de seguridad (P0) | NUNCA descartar, siempre mejorar |

---

## 6. Salvaguardas de Fiabilidad

### 6.1 Corpus Inmutable

```yaml
corpus_config.yaml:
  rust-corpus-v2:
    commit: abc123def456  # NUNCA cambia durante evaluación
    pinned_at: 2026-05-01
```

### 6.2 Blind Evaluation

El agente que propone cambios NO ve el corpus de evaluación ni los baselines. Solo recibe los resultados después de la evaluación.

### 6.3 Third-Party Baseline

Las métricas primarias DEBEN venir de herramientas externas (SonarQube, ESLint, Clippy, Ruff). CogniCode solo no puede validarse a sí mismo.

### 6.4 Periodic Human Audit

| Frecuencia | Muestra | Propósito |
|-----------|---------|-----------|
| Cada 100 iteraciones | 20 decisiones | Verificar accuracy |
| Cada 1000 iteraciones | 50 + anomalías | Revisión completa |
| Triggered | P0/P1 anomalías | Revisión inmediata |

### 6.5 Cross-Validation Split

- **Dev set (60%)**: Para mejorar reglas (tuning)
- **Test set (40%)**: Para evaluación final (NUNCA se usa para tuning)
- Rotación trimestral entre sets

### 6.6 Anomaly Detection Triggers

| Trigger | Umbral |
|---------|--------|
| Precision drop | > 20% en un solo run |
| Recall drop | > 15% en un solo run |
| FPR spike | > 5× media histórica |
| Issue count spike | > 10× media histórica |
| Rating drop | > 2 niveles (ej: A→C) |

### 6.7 Reproducibilidad

- Seeds fijas para splits aleatorios
- Versiones de herramientas pineadas en cada run
- Docker container para entorno de evaluación
- Audit log completo (actor, timestamp, inputs)
