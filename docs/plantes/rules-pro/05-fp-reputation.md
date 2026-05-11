# Sistema de Reputación de Falsos Positivos

> **Fecha**: 11 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Diseño de implementación

---

## 1. El Problema: El Ruido Mata la Adopción

### 1.1 False Positives Son Costosos

Los **falsos positivos** (findings que no son problemas reales) son el mayor obstáculo para la adopción de herramientas de análisis estático:

| Métrica | Impacto |
|---------|---------|
| **Tiempo perdido** | Desarrolladores investigan findings que no son reales |
| **Credibilidad** | Cuando 50% son FP, los otros 50% se ignoran |
| **Adopción** | Equipos dejan de usar la herramienta |
| **Costo** | Cada FP cuesta ~15 minutos de tiempo de desarrollador |

### 1.2 El Ciclo Vicioso

```
┌──────────────────────────────────────────────────────────────────┐
│                    CICLO VICIOSO DE FPs                          │
│                                                                  │
│  1. Herramienta reporta muchos FPs                               │
│           │                                                      │
│           ▼                                                      │
│  2. Desarrolladores ignoran los findings                        │
│           │                                                      │
│           ▼                                                      │
│  3. Bugs reales se pierden en el ruido                          │
│           │                                                      │
│           ▼                                                      │
│  4. Herramienta no parece útil                                  │
│           │                                                      │
│           ▼                                                      │
│  5. Uso de herramienta disminuye                               │
│           │                                                      │
│           ▼                                                      │
│  6. Volver a 1                                                  │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 1.3 Por Qué Ocurren los FPs

Los falsos positivos en CogniCode ocurren por varias razones:

| Causa | Ejemplo | Frecuencia |
|-------|---------|------------|
| **Regex sin contexto** | `TODO` en comentario es válido | Alta |
| **Strings como código** | `format!("SELECT * FROM")` no es SQL injection | Alta |
| **Code generation** | Archivos auto-generados no siguen las mismas reglas | Media |
| **Patrones demasiado amplios** | Detectar `eval()` cuando es `window.eval()` seguro | Media |
| **Legacy code** | Código antiguo con patrones que ahora son antipatterns | Baja |

---

## 2. La Solución: Sistema de Reputación

### 2.1 Concepto Central

El sistema de reputación permite que **los desarrolladores marquen findings como falsos positivos**, y el sistema **aprende** de ese feedback para reducir ruido en el futuro.

### 2.2 Principios de Diseño

1. **Feedback colectivo**: Si muchos desarrolladores marcan el mismo pattern como FP, probablemente es un FP
2. **Contexto específico**: Cada supresión es para un **nodo específico** del AST, no para una regla completa
3. **Expiración automática**: Las supresiones expiran cuando el código cambia (evita supresiones stale)
4. **Transparencia**: Los usuarios pueden ver por qué algo fue suprimido

### 2.3 Flujo de Uso

```
┌──────────────────────────────────────────────────────────────────┐
│                    FLUJO DE REPUTACIÓN                            │
│                                                                  │
│  1. ANALYZER reporta finding                                    │
│     finding = { rule_id, node_hash, file, line, message }       │
│                                                                  │
│  2. USUARIO ve el finding en la UI                              │
│     "S5122: SQL Injection en src/db.rs:42"                      │
│                                                                  │
│  3. USUARIO marca como FALSE POSITIVE                            │
│     → "This is a test file, SQL queries are expected"           │
│                                                                  │
│  4. Sistema registra supresión                                   │
│     suppression = { rule_id, node_hash, file, reason, expiry }   │
│                                                                  │
│  5. ANALYZER，下次运行时 skip finding si node_hash matchea     │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## 3. Estructura de Datos

### 3.1 Schema de Supresiones

```rust
/// Archivo: .cognicode-suppressions.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuppressionsFile {
    pub version: String,
    pub suppressions: Vec<Suppression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suppression {
    /// ID de la regla (ej: "sec/sql-injection")
    pub rule_id: String,

    /// Hash del nodo AST que fue suprimido
    pub node_hash: String,

    /// Ruta del archivo donde se suprimió
    pub file: String,

    /// Fecha de supresión (ISO 8601)
    pub suppressed_at: String,

    /// Razón proporcionada por el usuario
    pub reason: String,

    /// Si true, la supresión expira cuando el código cambia
    pub expires_on_change: bool,

    /// Hash del contexto padre (para expiración)
    pub parent_hash: Option<String>,
}

impl SuppressionsFile {
    /// Carga supresiones desde archivo
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self { version: "1.0".into(), suppressions: Vec::new() });
        }
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(Into::into)
    }

    /// Guarda supresiones a archivo
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Agrega una nueva supresión
    pub fn add(&mut self, suppression: Suppression) {
        // Evitar duplicados
        if !self.suppressions.iter().any(|s| {
            s.rule_id == suppression.rule_id
            && s.node_hash == suppression.node_hash
            && s.file == suppression.file
        }) {
            self.suppressions.push(suppression);
        }
    }
}
```

### 3.2 Cálculo del Node Hash

```rust
/// Calcula un hash único para un nodo AST
/// El hash se basa en: tipo de nodo + contenido + posición relativa
pub struct NodeHasher;

impl NodeHasher {
    /// Genera un hash para un nodo específico
    pub fn hash_node(node: &Node, source: &str) -> String {
        let mut hasher = Sha256::new();

        // Incluir tipo de nodo
        hasher.update(node.kind().as_bytes());

        // Incluir texto del nodo (contenido)
        let node_text = node.text();
        hasher.update(node_text.as_bytes());

        // Incluir posición relativa (línea, columna) - no absoluta
        // Esto hace que el hash sea estable ante cambios lejanos
        hasher.update(format!("{}:{}", node.start_position().row, node.start_position().column).as_bytes());

        // Incluir hash del parent para detectar refactors
        if let Some(parent) = node.parent() {
            let parent_hash = Self::hash_parent_context(&parent, source);
            hasher.update(parent_hash.as_bytes());
        }

        let result = hasher.finalize();
        format!("{:x}", result)[..8].to_string()  // Primeros 8 chars
    }

    /// Hash del contexto del padre (para expiración)
    fn hash_parent_context(parent: &Node, source: &str) -> String {
        let mut hasher = Sha256::new();

        // Usar la estructura del padre, no su contenido
        hasher.update(parent.kind().as_bytes());

        // Solo los primeros 100 chars de contenido del padre
        let text = parent.text();
        hasher.update(text.chars().take(100).collect::<String>().as_bytes());

        format!("{:x}", hasher.finalize())
    }

    /// Verifica si el hash actual del nodo coincide con la supresión
    pub fn matches_suppression(node: &Node, source: &str, suppression: &Suppression) -> bool {
        let current_hash = Self::hash_node(node, source);

        if current_hash != suppression.node_hash {
            return false;
        }

        // Verificar expiración por cambio
        if suppression.expires_on_change {
            if let Some(parent_hash) = &suppression.parent_hash {
                let current_parent = node.parent();
                if let Some(parent) = current_parent {
                    let current_parent_hash = Self::hash_parent_context(&parent, source);
                    if current_parent_hash != *parent_hash {
                        // Código alrededor cambió, supresión expirada
                        return false;
                    }
                }
            }
        }

        true
    }
}
```

---

## 4. Integración con el Analyzer

### 4.1 Filtrado de Findings

```rust
pub struct Analyzer {
    suppressions: SuppressionsFile,
}

impl Analyzer {
    /// Procesa los findings y filtra los suprimidos
    pub fn filter_suppressed(&self, findings: Vec<Issue>) -> Vec<Issue> {
        findings
            .into_iter()
            .filter(|finding| {
                !self.is_suppressed(finding)
            })
            .collect()
    }

    /// Verifica si un finding está suprimido
    fn is_suppressed(&self, finding: &Issue) -> bool {
        for suppression in &self.suppressions.suppressions {
            if suppression.rule_id == finding.rule_id
                && suppression.file == finding.file.to_string_lossy()
                && finding.node.map_or(false, |span| {
                    // Necesitaríamos el nodo para calcular el hash
                    // Esta es una versión simplificada
                    true
                })
            {
                return true;
            }
        }
        false
    }
}
```

### 4.2 UI para Marcar FP

```rust
/// Comando CLI para suprimir un finding
pub struct SuppressCommand {
    pub rule_id: String,
    pub file: String,
    pub line: Option<u32>,
    pub reason: String,
    pub expires_on_change: bool,
}

impl SuppressCommand {
    pub fn execute(&self, suppressions: &mut SuppressionsFile) -> Result<()> {
        // Calcular node_hash (requiere acceso al AST)
        let node_hash = self.calculate_node_hash()?;

        let suppression = Suppression {
            rule_id: self.rule_id.clone(),
            node_hash,
            file: self.file.clone(),
            suppressed_at: chrono::Utc::now().to_rfc3339(),
            reason: self.reason.clone(),
            expires_on_change: self.expires_on_change,
            parent_hash: Some(self.calculate_parent_hash()?),
        };

        suppressions.add(suppression);
        Ok(())
    }
}
```

---

## 5. Sistema de Feedback Loop

### 5.1 Tracking de Tasa de FP

```rust
/// Estadísticas de una regla
#[derive(Debug, Clone)]
pub struct RuleStats {
    pub rule_id: String,
    /// Total de findings reportados
    pub total_findings: u64,
    /// Número marcado como FP
    pub false_positives: u64,
    /// Número confirmado como true positive
    pub true_positives: u64,
    /// Tasa de FP (0.0 - 1.0)
    pub fp_rate: f64,
}

impl RuleStats {
    pub fn fp_rate(&self) -> f64 {
        if self.total_findings == 0 {
            return 0.0;
        }
        self.false_positives as f64 / self.total_findings as f64
    }

    /// Retorna true si la regla tiene demasiados FPs
    pub fn is_noisy(&self) -> bool {
        self.fp_rate() > 0.4  // > 40% FP rate
    }
}

/// Tracker de estadísticas por regla
pub struct ReputationTracker {
    stats: HashMap<String, RuleStats>,
}

impl ReputationTracker {
    pub fn record_finding(&mut self, rule_id: &str, is_false_positive: bool) {
        let stats = self.stats.entry(rule_id.to_string()).or_insert_with(|| RuleStats {
            rule_id: rule_id.to_string(),
            total_findings: 0,
            false_positives: 0,
            true_positives: 0,
            fp_rate: 0.0,
        });

        stats.total_findings += 1;
        if is_false_positive {
            stats.false_positives += 1;
        } else {
            stats.true_positives += 1;
        }
    }

    /// Sugiere ajustar el pattern si hay demasiados FPs
    pub fn suggest_improvement(&self, rule_id: &str) -> Option<String> {
        let stats = self.stats.get(rule_id)?;

        if stats.is_noisy() {
            Some(format!(
                "Rule '{}' has {:.1}% false positive rate. Consider: \
                 1) Making the pattern more specific, \
                 2) Adding exclusion patterns, \
                 3) Using semantic analysis (Layer 2/3) instead of regex.",
                rule_id,
                stats.fp_rate() * 100.0
            ))
        } else {
            None
        }
    }
}
```

### 5.2 Dashboard de Reputación

```rust
/// Genera reporte de reputación para la UI
#[derive(Serialize)]
pub struct ReputationReport {
    pub overall_fp_rate: f64,
    pub rules_by_fp_rate: Vec<RuleStats>,
    pub top_noisy_rules: Vec<RuleStats>,
    pub recently_suppressed: Vec<Suppression>,
}

impl ReputationTracker {
    pub fn generate_report(&self, suppressions: &SuppressionsFile) -> ReputationReport {
        let mut rules: Vec<_> = self.stats.values().collect();
        rules.sort_by(|a, b| b.fp_rate.partial_cmp(&a.fp_rate).unwrap());

        let total_fp: u64 = rules.iter().map(|s| s.false_positives).sum();
        let total_findings: u64 = rules.iter().map(|s| s.total_findings).sum();

        ReputationReport {
            overall_fp_rate: if total_findings > 0 {
                total_fp as f64 / total_findings as f64
            } else {
                0.0
            },
            rules_by_fp_rate: rules.clone(),
            top_noisy_rules: rules.iter().filter(|r| r.is_noisy()).cloned().collect(),
            recently_suppressed: suppressions.suppressions.iter().rev().take(10).cloned().collect(),
        }
    }
}
```

---

## 6. Anti-Abuso y Expiración

### 6.1 Expiración Automática

El sistema de supresión **no es permanente**. Las supresiones expiran cuando el código alrededor cambia.

```rust
/// Verifica si una supresión sigue siendo válida
pub fn is_suppression_valid(
    suppression: &Suppression,
    current_source: &str,
    node: &Node,
) -> bool {
    // Si no expira en cambio, es permanente (dentro del archivo)
    if !suppression.expires_on_change {
        return true;
    }

    // Verificar que el código alrededor no cambió
    if let Some(expected_parent_hash) = &suppression.parent_hash {
        if let Some(parent) = node.parent() {
            let current_parent_hash = NodeHasher::hash_parent_context(&parent, current_source);
            return current_parent_hash == *expected_parent_hash;
        }
    }

    false
}
```

### 6.2 Razones de Expiración

| Situación | Comportamiento |
|-----------|---------------|
| Código alrededor cambió (misma función) | Supresión expira |
| Nodo completo se modificó | Supresión expira |
| Archivo se borró | Supresión se limpia automáticamente |
| Archivo se movió | Supresión se limpia (paths diferentes) |
| Refactor grande | Todas las supresiones del archivo expiran |

### 6.3 Límites de Supresión

Para evitar abuso, se pueden imponer límites:

```rust
/// Límites de supresión
#[derive(Debug)]
pub struct SuppressionLimits {
    /// Máximo de supresiones activas por archivo
    pub max_per_file: usize,
    /// Máximo de supresiones activas por regla
    pub max_per_rule: usize,
    /// Máximo de supresiones totales
    pub max_total: usize,
}

impl Default for SuppressionLimits {
    fn default() -> Self {
        Self {
            max_per_file: 100,
            max_per_rule: 50,
            max_total: 1000,
        }
    }
}

impl SuppressionsFile {
    pub fn can_add(&self, limits: &SuppressionLimits, rule_id: &str, file: &str) -> bool {
        let per_file = self.suppressions.iter()
            .filter(|s| s.file == file)
            .count();

        let per_rule = self.suppressions.iter()
            .filter(|s| s.rule_id == rule_id)
            .count();

        let total = self.suppressions.len();

        per_file < limits.max_per_file
            && per_rule < limits.max_per_rule
            && total < limits.max_total
    }
}
```

---

## 7. Formato del Archivo de Supresiones

### 7.1 Ejemplo `.cognicode-suppressions.json`

```json
{
  "version": "1.0",
  "suppressions": [
    {
      "rule_id": "sec/sql-injection",
      "node_hash": "a4f3b2c1",
      "file": "src/tests/fixtures.rs",
      "suppressed_at": "2026-05-11T10:30:00Z",
      "reason": "This is a test file, SQL queries are expected",
      "expires_on_change": true,
      "parent_hash": "e5d6c7b8"
    },
    {
      "rule_id": "convention/todo-comment",
      "node_hash": "b3c4d5e6",
      "file": "src/main.rs",
      "suppressed_at": "2026-05-10T15:45:00Z",
      "reason": "TODO is used intentionally as a marker for the documentation generator",
      "expires_on_change": false,
      "parent_hash": null
    },
    {
      "rule_id": "security/weak-crypto",
      "node_hash": "c7d8e9f0",
      "file": "src/legacy/md5_compat.rs",
      "suppressed_at": "2026-05-09T09:00:00Z",
      "reason": "Legacy compatibility module, upgrading would break clients",
      "expires_on_change": true,
      "parent_hash": "f1e2d3c4"
    }
  ]
}
```

---

## 8. Comparación con Otros Sistemas

| Aspecto | ESLint disable comments | SonarQube | CogniCode FP System |
|---------|------------------------|-----------|---------------------|
| Scope | Línea/archivo | Global | Nodo específico |
| Expiración | Nunca | Manual | Automática en cambio |
| Tracking de FP rate | No | Parcial | Sí |
| Feedback loop | No | Limitado | Sí |
| Supresión coletiva | No | Sí (quality gate) | Sí |
| Hash de contexto | No | No | Sí |
