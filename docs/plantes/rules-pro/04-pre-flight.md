# Layer 0: Pre-Flight con Aho-Corasick

> **Fecha**: 11 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Diseño de implementación

---

## 1. El Problema: Parsing AST es Caro

### 1.1 Costo del Parsing

Parsear un archivo a un AST es una operación **relativamente costosa**:

| Operación | Tiempo Aproximado |
|-----------|-------------------|
| Leer archivo | ~0.1ms |
| Lexing (tokenizing) | ~0.5ms |
| Parsing a AST | ~1-5ms |
| **Total por archivo** | **~2-6ms** |

Para un proyecto con **1000 archivos**, el parsing completo toma **2-6 segundos**.

### 1.2 No Todos los Archivos Necesitan Todas las Reglas

En la práctica, la mayoría de los archivos son relevantes solo para un **subconjunto** de reglas:

- Un archivo `src/main.rs` probablemente necesita reglas de SQL injection? **Probablemente no**
- Un archivo `src/crypto.rs` definitivamente necesita reglas de crypto? **Sí**
- Un archivo de tests `tests/integration_test.rs` necesita todas las reglas? **Depende**

### 1.3 La Solución: Filtrar Antes de Parsear

**Pre-flight** es una capa de filtrado ultra-rápida que actúa **antes** de parsear el AST, descartando reglas irrelevantes basándose en keywords de texto plano.

```
┌──────────────────────────────────────────────────────────────────┐
│                    SOURCE CODE                                    │
│                (archivo.rs, 10KB)                                 │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│              LAYER 0: PRE-FLIGHT (Aho-Corasick)                   │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  Escaneo O(n) en ~0.01ms                                   │  │
│  │ Resultado: 50 de 854 reglas son aplicables                 │  │
│  └────────────────────────────────────────────────────────────┘  │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│                    LAYER 1: STRUCTURAL                           │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  Parsear AST (~2-6ms por archivo)                          │  │
│  │  Solo para las 50 reglas seleccionadas                     │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

---

## 2. Algoritmo Aho-Corasick

### 2.1 Qué es Aho-Corasick

Aho-Corasick es un algoritmo de **búsqueda de múltiples patrones simultáneos** desarrollado por Alfred Aho y Margaret Corasick en 1975.

**Propiedad fundamental**: Encuentra todas las ocurrencias de **todos los patrones** en **O(n + m + z)** donde:
- n = longitud del texto
- m = suma de longitudes de todos los patrones
- z = número de matches

### 2.2 Por Qué Aho-Corasick para Pre-Flight

| Algoritmo | Tiempo (1M patrones, 1KB texto) | Memoria |
|-----------|----------------------------------|---------|
| Naive | O(1B) operaciones | - |
| Boyer-Moore | O(n * m) | - |
| **Aho-Corasick** | **O(n + m + z)** | Moderada |
| Regex compilation | Lento | Alta |

**Aho-Corasick es óptimo** para nuestro caso de uso porque:
1. Construye **un solo automaton** para todas las keywords
2. Lo usa para **todos los archivos**
3. El escaneo es **lineal** en la longitud del archivo

### 2.3 Comparación Visual

```
ARCHIVO: "SELECT * FROM users WHERE id = 1"

┌─────────────────────────────────────────────────────────────────┐
│                    REGEX NAIVE                                  │
│                                                                  │
│  Regex para S5122: (?<!['"`])sql|SELECT|INSERT...                │
│  ═══════════════════════════════════                            │
│  ✗ Fallo en "SELECT" - no coincide con pattern                   │
│  ═══════════════════════════════════                            │
│  ✗ Fallo en "FROM" - no coincide                                 │
│  ...                                                             │
│  Resultado: 1 match encontrado, pero O(n*m) búsquedas           │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                   AHO-CORASICK                                   │
│                                                                  │
│  automaton con keywords: [sql, SELECT, INSERT, UPDATE, DELETE]  │
│  ════════════════════════════════════════════════════════════   │
│  ✓ "SELECT" encontrado @ posicion 0                             │
│  ✓ "FROM" no es keyword                                         │
│  ✓ "UPDATE" no es keyword                                       │
│  ✓ "WHERE" no es keyword                                         │
│  ════════════════════════════════════════════════════════════   │
│  Resultado: 1 keyword (SELECT) en O(n)                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Implementación

### 3.1 Estructuras de Datos

```rust
use aho_corasick::AhoCorasick;
use serde::{Deserialize, Serialize};

/// Registro de una keyword asociada a reglas
#[derive(Debug, Clone)]
pub struct KeywordEntry {
    /// Índice de la keyword en el automaton
    pub pattern_idx: usize,
    /// Lista de rule_ids que requieren esta keyword
    pub rule_ids: Vec<RuleId>,
}

/// Filter de pre-flight basado en Aho-Corasick
pub struct PreflightFilter {
    /// Automaton precompilado con todas las keywords
    automaton: AhoCorasick,
    /// Mapping de pattern_idx → rule_ids
    keyword_to_rules: Vec<Vec<RuleId>>,
    /// Todas las keywords en orden
    keywords: Vec<String>,
}

impl PreflightFilter {
    /// Construye el filter desde el catálogo de reglas
    pub fn new(rules: &[Box<dyn Rule>]) -> Self {
        // 1. Recolectar todas las keywords únicas
        let mut all_keywords: Vec<String> = Vec::new();
        let mut keyword_to_rules: Vec<Vec<RuleId>> = Vec::new();

        for rule in rules {
            for kw in rule.required_keywords() {
                all_keywords.push(kw.to_string());
            }
        }

        // Deduplicar
        all_keywords.sort();
        all_keywords.dedup();

        // 2. Construir automaton
        let automaton = AhoCorasick::new(&all_keywords)
            .expect("Failed to build Aho-Corasick automaton");

        // 3. Crear mapping vacío (se llena en register_rules)
        let keyword_to_rules = vec![Vec::new(); all_keywords.len()];

        Self {
            automaton,
            keyword_to_rules,
            keywords: all_keywords,
        }
    }

    /// Registra las reglas asociadas a cada keyword
    pub fn register_rules(&mut self, rules: &[Box<dyn Rule>]) {
        for rule in rules {
            for kw in rule.required_keywords() {
                if let Some(idx) = self.keywords.iter().position(|k| k == kw) {
                    // Asumimos que self.rule_id_map da el RuleId
                    let rule_id = self.get_rule_id(rule.id());
                    self.keyword_to_rules[idx].push(rule_id);
                }
            }
        }
    }

    fn get_rule_id(&self, rule_id: &str) -> RuleId {
        // Implementation-dependent: map rule string ID to numeric RuleId
        RuleId(0) // placeholder
    }
}
```

### 3.2 Filtrado de Reglas

```rust
impl PreflightFilter {
    /// Filtra las reglas aplicables basándose en keywords presentes en el source
    pub fn filter_rules<'a>(
        &self,
        source: &str,
        all_rules: &'a [Box<dyn Rule>],
    ) -> Vec<&'a Box<dyn Rule>> {
        // 1. Encontrar todas las keywords presentes en el source
        let present_keyword_indices: HashSet<usize> = self.automaton
            .find_iter(source)
            .map(|m| m.pattern().as_usize())
            .collect();

        // 2. Encontrar todas las reglas que pueden aplicar
        let mut eligible_rules: HashSet<RuleId> = HashSet::new();

        for kw_idx in &present_keyword_indices {
            for rule_id in &self.keyword_to_rules[*kw_idx] {
                eligible_rules.insert(*rule_id);
            }
        }

        // 3. Filtrar el catálogo de reglas
        // Si una regla no tiene keywords requeridas, siempre aplica
        all_rules
            .iter()
            .filter(|rule| {
                // Reglas sin keywords: siempre aplican
                if rule.required_keywords().is_empty() {
                    return true;
                }
                // Verificar si alguna keyword requerida está presente
                eligible_rules.contains(&self.get_rule_id(rule.id()))
            })
            .collect()
    }
}
```

### 3.3 Optimización: Case Insensitivity

```rust
/// Variante case-insensitive para keywords
pub struct PreflightFilterCaseInsensitive {
    automaton: AhoCorasick,
    // ... resto igual pero con строука lookup
}

impl PreflightFilterCaseInsensitive {
    pub fn new(rules: &[Box<dyn Rule>]) -> Self {
        let mut all_keywords: Vec<String> = Vec::new();

        for rule in rules {
            for kw in rule.required_keywords() {
                // Normalizar a lowercase
                all_keywords.push(kw.to_lowercase());
            }
        }

        // Construir automaton case-insensitive
        let automaton = AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .build(&all_keywords)
            .expect("Failed to build Aho-Corasick automaton");

        // ... resto
    }

    pub fn filter_rules<'a>(
        &self,
        source: &str,
        all_rules: &'a [Box<dyn Rule>],
    ) -> Vec<&'a Box<dyn Rule>> {
        // El automaton ya hace case-insensitive matching internamente
        let present_keyword_indices: HashSet<usize> = self.automaton
            .find_iter(source.to_lowercase())  // Solo lowercase una vez
            .map(|m| m.pattern().as_usize())
            .collect();

        // ... resto igual
    }
}
```

---

## 4. Integración con el Sistema de Capas

### 4.1 Flujo Completo

```
┌──────────────────────────────────────────────────────────────────┐
│                        ANALYZER                                   │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Construir PreflightFilter al iniciar                         │
│     ┌─────────────────────────────────────────────────────────┐  │
│     │  all_rules ──→ PreflightFilter                          │  │
│     │              ├── Recolecta keywords                     │  │
│     │              ├── Construye Aho-Corasick automaton       │  │
│     │              └── Registra rule → keywords mapping      │  │
│     └─────────────────────────────────────────────────────────┘  │
│                                                                  │
│  2. Para cada archivo:                                            │
│     ┌─────────────────────────────────────────────────────────┐  │
│     │  source ──→ filter_rules() ──→ eligible_rules          │  │
│     │            ├── Aho-Corasick scan O(n)                   │  │
│     │            └── Retorna reglas candidatas               │  │
│     └─────────────────────────────────────────────────────────┘  │
│                                                                  │
│  3. Parsear AST (solo si hay reglas aplicables)                  │
│     ┌─────────────────────────────────────────────────────────┐  │
│     │  source ──→ tree-sitter ──→ AST                         │  │
│     │            (~2-6ms por archivo)                        │  │
│     └─────────────────────────────────────────────────────────┘  │
│                                                                  │
│  4. Ejecutar reglas por capa                                      │
│     ┌─────────────────────────────────────────────────────────┐  │
│     │  Layer 1: Structural (todas las eligible_rules)         │  │
│     │  Layer 2: Semantic (si hay rules layer=2)               │  │
│     │  Layer 3: Flow (si hay rules layer=3)                   │  │
│     └─────────────────────────────────────────────────────────┘  │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 4.2 Implementación en Analyzer

```rust
pub struct Analyzer {
    catalog: RuleCatalog,
    preflight: PreflightFilter,
}

impl Analyzer {
    pub fn new() -> Self {
        let catalog = RuleCatalog::load_all();
        let preflight = PreflightFilter::new(catalog.all_rules());

        Self { catalog, preflight }
    }

    pub fn analyze_file(&self, source: &str, path: &Path) -> Vec<Issue> {
        // 1. Pre-flight: filtrar reglas aplicables
        let eligible_rules = self.preflight.filter_rules(
            source,
            self.catalog.all_rules(),
        );

        if eligible_rules.is_empty() {
            return Vec::new();
        }

        // 2. Parsear AST
        let language = self.detect_language(path);
        let tree = parse(source, language);

        // 3. Ejecutar reglas por capa
        let mut all_issues = Vec::new();

        // Layer 1: Structural
        let layer1_rules: Vec<_> = eligible_rules.iter()
            .filter(|r| r.layer() == 1)
            .collect();

        for rule in layer1_rules {
            let ctx = RuleContext::new(source, &tree, language, path);
            let issues = rule.check(&ctx);
            all_issues.extend(issues);
        }

        // Layer 2: Semantic (requiere LCPG)
        let layer2_rules: Vec<_> = eligible_rules.iter()
            .filter(|r| r.layer() == 2)
            .collect();

        if !layer2_rules.is_empty() {
            let symbol_table = LcpqBuilder::new(FileId(0)).build(&tree);
            let ctx = RuleContext::with_symbol_table(source, &tree, &symbol_table, language, path);

            for rule in layer2_rules {
                let issues = rule.check(&ctx);
                all_issues.extend(issues);
            }
        }

        // Layer 3: Flow
        let layer3_rules: Vec<_> = eligible_rules.iter()
            .filter(|r| r.layer() == 3)
            .collect();

        for rule in layer3_rules {
            let ctx = RuleContext::new(source, &tree, language, path);
            let issues = rule.check(&ctx);
            all_issues.extend(issues);
        }

        all_issues
    }
}
```

---

## 5. Ejemplos Prácticos

### 5.1 Keywords para Reglas de SQL

```rust
// Regla S5122: SQL Injection
struct SqlInjectionRule;

impl Rule for SqlInjectionRule {
    fn id(&self) -> &str { "security/sql-injection" }

    fn required_keywords(&self) -> &[&str] {
        &[
            // Palabras SQL
            "sql", "SELECT", "INSERT", "UPDATE", "DELETE", "DROP",
            "CREATE", "ALTER", "TRUNCATE",
            // Funciones de query
            "query", "execute", "fetch", "cursor",
            // APIs de base de datos
            "db", "database", "postgres", "mysql", "sqlite",
            // String formatting (para interpolation)
            "format!", "format_args!", "concat!",
            // ORM patterns
            ".filter(", ".where(", ".find(",
        ]
    }

    fn layer(&self) -> u8 { 3 }
}
```

### 5.2 Keywords para Reglas de Crypto

```rust
// Regla: Weak Crypto
struct WeakCryptoRule;

impl Rule for WeakCryptoRule {
    fn id(&self) -> &str { "security/weak-crypto" }

    fn required_keywords(&self) -> &[&str] {
        &[
            // Algoritmos débiles
            "md5", "sha1", "sha256", "sha384", "sha512",
            "des", "rc4", "rc2", "dsa",
            // Funciones hash
            "hash", "Hash", "digest", "Digest",
            // Módulos crypto
            "crypto", "Crypto", "cryptography",
            "hashlib", "pycryptodome", "openssl",
            // Nombres de funciones comunes
            "encrypt", "decrypt", "sign", "verify",
        ]
    }

    fn layer(&self) -> u8 { 1 }
}
```

### 5.3 Efecto del Pre-Flight

```
ESCENARIO: Proyecto con 1000 archivos Rust

┌──────────────────────────────────────────────────────────────────┐
│                    SIN PRE-FLIGHT                                │
│                                                                  │
│  Por cada archivo (1000):                                        │
│  - Parsear AST: ~3ms                                             │
│  - Ejecutar TODAS las reglas (854): ~10ms                       │
│                                                                  │
│  Total: 1000 × (3ms + 10ms) = 13 segundos                       │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                    CON PRE-FLIGHT                                │
│                                                                  │
│  Pre-flight por archivo (~0.01ms):                              │
│  - Encontrar keywords presentes                                  │
│  - Filtrar a ~50 reglas aplicables                              │
│                                                                  │
│  Parsear AST: ~3ms (solo para archivos con reglas)              │
│  Ejecutar reglas filtradas: ~0.5ms                              │
│                                                                  │
│  Estimación (asumiendo 30% de archivos tienen reglas):         │
│  - Pre-flight: 1000 × 0.01ms = 10ms                             │
│  - Parseo + análisis: 300 × 3.5ms = 1050ms                     │
│                                                                  │
│  Total: ~1.06 segundos (12x más rápido)                         │
└──────────────────────────────────────────────────────────────────┘
```

---

## 6. Edge Cases y Consideraciones

### 6.1 Keywords en Comentarios

**Problema**: Una keyword en un comentario no debería activar una regla.

```rust
// Este archivo contiene:
// SELECT * FROM users -- pero es solo un comentario, no SQL real
```

**Solución**: El pre-flight es solo un **filtro grueso** (recall alto, precision baja). La Layer 1 (AST matching) filtra los falsos positivos reales.

### 6.2 Keywords en Strings

**Problema similar**: `let sql = "SELECT * FROM"` contiene SQL keywords pero no es vulnerable.

**Mismo enfoque**: La Layer 1/3 hace el análisis fino que distingue contexto.

### 6.3 Archivos Muy Grandes

**Consideración**: Para archivos >1MB, el escaneo Aho-Corasick sigue siendo O(n), pero puede tomar ~10ms.

**Optimización**: Para archivos muy grandes,可以考虑 hacer pre-flight por chunks o saltar si el tiempo de parsing sería mayor.

### 6.4 Unicode y Binary Files

**Manejo**: Aho-Corasick funciona con bytes, pero las keywords son ASCII typical.

**Recomendación**: Ignorar archivos binary (skip binary detection) antes de pre-flight.

---

## 7. Métricas de Rendimiento

### 7.1 Benchmarks Típicos

| Operación | Tiempo |
|-----------|--------|
| Construcción del automaton (854 reglas) | ~5ms (una vez) |
| Escaneo de archivo 10KB | ~0.01ms |
| Escaneo de archivo 100KB | ~0.1ms |
| Escaneo de archivo 1MB | ~1ms |

### 7.2 Savings Estimados

| Escenario | Sin Pre-flight | Con Pre-flight | Speedup |
|-----------|----------------|---------------|---------|
| 100 archivos | 1.3s | 0.1s | 13x |
| 1000 archivos | 13s | 1.1s | 12x |
| 10000 archivos | 130s | 11s | 12x |

### 7.3 Costo de Memoria

| Componente | Memoria |
|------------|---------|
| Automaton (854 keywords) | ~50KB |
| keyword_to_rules mapping | ~100KB |
| HashSet temporal | ~10KB por archivo |
| **Total** | **~160KB + overhead** |
