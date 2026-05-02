# Rule Engine — CogniCode Axiom

> Capa de análisis de calidad de código basada en AST para CogniCode.

**Versión**: 0.1.0
**Estado**: Experimental
**Última actualización**: Abril 2026

---

## 1. Visión General

El Rule Engine es la capa de análisis de calidad de código de **cognicode-axiom**, el crate de gobernanza y calidad dentro del workspace de CogniCode. Su responsabilidad es responder a la pregunta: *"¿El código cumple con los estándares de calidad?"*, en contraste con Cedar Policy que responde *"¿Está autorizado para realizar esta acción?"*.

### 1.1 Arquitectura del Sistema

```
┌─────────────────────────────────────────────────────────────┐
│                      CogniCode Workspace                     │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────┐    ┌─────────────────────────────┐ │
│  │   Cedar Policy      │    │       Rule Engine           │ │
│  │   (Gobernanza)     │    │       (Calidad)             │ │
│  │                     │    │                             │ │
│  │  "¿Puedo hacer      │    │  "¿Lo hice bien?"           │ │
│  │   esto?"            │    │                             │ │
│  └──────────┬──────────┘    └──────────────┬──────────────┘ │
│             │                               │                │
│             │      ┌───────────────────────┘                │
│             │      │                                         │
│             ▼      ▼                                         │
│     ┌─────────────────────────────────────────────┐          │
│     │              Bridge Layer                    │          │
│     │  Rule Findings → Cedar Context (futuro)     │          │
│     └─────────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Relación con Cedar Policy

| Aspecto | Cedar Policy | Rule Engine |
|---------|-------------|-------------|
| **Pregunta** | "¿Tengo permiso?" | "¿Cumplo con los estándares?" |
| **Tiempo** | Pre-ejecución | Post-análisis |
| ** focus** | Autorización | Calidad, bugs, security hotspots |
| **Output** | Permitido/Denegado | Lista de issues categorizados |

Los findings del Rule Engine pueden convertirse en contexto para Cedar Policy en fases futuras, permitiendo reglas como: *"Un archivo con más de 5 issues críticos no puede ser mergeado"*.

### 1.3 Infraestructura Compartida

El Rule Engine aprovecha la infraestructura existente de CogniCode:

- **tree-sitter**: Parsing multi-lenguaje (Rust, TypeScript, Python, Go, Java, C)
- **CallGraph**: Grafo de llamadas basado en PetGraph + Redb
- **ComplexityCalculator**: Métricas de complejidad (cyclomatic, cognitive, nesting)
- **CycleDetector**: Detección de dependencias cíclicas
- **ImpactAnalyzer**: Análisis de impacto de cambios

```
┌─────────────────────────────────────────────────────────────┐
│                     Rule Engine                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   tree-sitter     CallGraph     Complexity      CycloMat    │
│        │              │            Calculator       │       │
│        └──────────────┴─────────────┬────────────────┘       │
│                                     │                        │
│                                     ▼                        │
│                            ┌─────────────────┐               │
│                            │  RuleContext    │               │
│                            │  (datos para    │               │
│                            │   cada regla)   │               │
│                            └────────┬────────┘               │
│                                     │                        │
│                    ┌────────────────┼────────────────┐      │
│                    ▼                ▼                ▼      │
│              ┌──────────┐    ┌──────────┐    ┌──────────┐   │
│              │  Rust    │    │  Type    │    │  Python  │   │
│              │  Rules   │    │  Script  │    │  Rules   │   │
│              │          │    │  Rules   │    │          │   │
│              └──────────┘    └──────────┘    └──────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Core Types

### 2.1 El Trait `Rule`

El trait `Rule` es la interfaz central que toda regla debe implementar:

```rust
/// Trait principal que define el comportamiento de una regla de calidad.
///
/// Cada regla encapsula:
/// - Identificación (id, name)
/// - Clasificación (severity, category)
/// - Alcance (language)
/// - Lógica de detección (check)
pub trait Rule: Send + Sync {
    /// Identificador único de la regla (ej. "S138", "S3776")
    fn id(&self) -> &str;

    /// Nombre descriptivo de la regla
    fn name(&self) -> &str;

    /// Severidad del issue detectado
    fn severity(&self) -> Severity;

    /// Categoría del issue
    fn category(&self) -> Category;

    /// Lenguaje que esta regla analiza (ej. "rust", "typescript")
    fn language(&self) -> &str;

    /// Lógica principal de la regla.
    /// Recibe el contexto con el AST y metadata, retorna issues encontrados.
    fn check(&self, ctx: &RuleContext) -> Vec<Issue>;
}
```

**Nota sobre `Send + Sync`**: El trait requiere que las implementaciones sean thread-safe porque el Rule Engine ejecuta reglas en paralelo usando Rayon. Esto permite que cientos de reglas se ejecuten concurrently sin bloqueos.

### 2.2 `RuleContext`

El contexto de ejecución proporciona acceso a todos los datos que una regla necesita:

```rust
/// Contexto compartido passado a todas las reglas durante el análisis.
///
/// Contiene:
/// - El AST parseado (tree-sitter)
/// - El código fuente completo
/// - Ruta del archivo
/// - Lenguaje detectado
/// - Grafo de llamadas (desde cognicode-core)
/// - Métricas de archivo (desde cognicode-core)
pub struct RuleContext<'a> {
    /// Árbol AST del archivo analizar
    pub tree: &'a Tree,

    /// Código fuente completo como string
    pub source: &'a str,

    /// Ruta absoluta del archivo
    pub file_path: &'a Path,

    /// Lenguaje del archivo (detectado por tree-sitter)
    pub language: Language,

    /// Grafo de llamadas del proyecto (PetGraph)
    pub graph: &'a CallGraph,

    /// Métricas pre-calculadas del archivo
    pub metrics: &'a FileMetrics,
}
```

**Lifetime `'a`**: El lifetime asegura que todas las referencias dentro de `RuleContext` vivan al menos tanto como el contexto mismo, evitando uso después de free.

### 2.3 `Issue`

Representa un problema detectado por una regla:

```rust
/// Representa un issue (problema) encontrado durante el análisis.
///
/// Incluye información de ubicación, severidad, categoría
/// y datos para generar un remediation suggestion.
#[derive(Debug, Clone)]
pub struct Issue {
    /// ID de la regla que generó este issue
    pub rule_id: String,

    /// Mensaje descriptivo del problema
    pub message: String,

    /// Severidad del issue
    pub severity: Severity,

    /// Categoría del issue
    pub category: Category,

    /// Ruta del archivo donde se encontró el issue
    pub file: PathBuf,

    /// Número de línea donde inicia el issue (1-indexed)
    pub line: usize,

    /// Número de columna donde inicia (None si no disponible)
    pub column: Option<usize>,

    /// Número de línea donde termina (None si es single-line)
    pub end_line: Option<usize>,

    /// Sugerencia de remediación (opcional)
    pub remediation: Option<Remediation>,
}

impl Issue {
    /// Crea un nuevo issue con ubicación básica
    pub fn new(
        rule_id: impl Into<String>,
        message: impl Into<String>,
        severity: Severity,
        category: Category,
        file: impl Into<PathBuf>,
        line: usize,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            message: message.into(),
            severity,
            category,
            file: file.into(),
            line,
            column: None,
            end_line: None,
            remediation: None,
        }
    }

    /// Añade ubicación de columna
    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    /// Añade línea de fin para issues multi-línea
    pub fn with_end_line(mut self, end_line: usize) -> Self {
        self.end_line = Some(end_line);
        self
    }

    /// Añade sugerencia de remediación
    pub fn with_remediation(mut self, remediation: Remediation) -> Self {
        self.remediation = Some(remediation);
        self
    }
}
```

### 2.4 Enums de Clasificación

```rust
/// Severidad de un issue.
/// Orden de gravedad: Blocker > Critical > Major > Minor > Info
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Problema crítico que impide el funcionamiento
    Blocker = 5,

    /// Problema grave que puede causar fallos en producción
    Critical = 4,

    /// Problema significativo que debe ser corregido
    Major = 3,

    /// Problema menor que no afecta funcionalidad
    Minor = 2,

    /// Información o sugerencia
    Info = 1,
}

/// Categoría del issue.
/// Agrupa issues por tipo de problema detectado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Defecto que probablemente produce comportamiento incorrecto
    Bug,

    /// Vulnerabilidad de seguridad
    Vulnerability,

    /// Patrón de código que indica baja calidad
    CodeSmell,

    /// Área de código que es potencialmente insegura
    SecurityHotspot,
}
```

### 2.5 `Remediation`

Información para sugerir al desarrollador cómo corregir el issue:

```rust
/// Sugerencia de remediación para un issue.
///
/// Cuantifica el esfuerzo de corrección y proporciona
/// una descripción de la solución recomendada.
#[derive(Debug, Clone)]
pub struct Remediation {
    /// Estimación del tiempo necesario para corregir (minutos)
    pub effort_minutes: u32,

    /// Descripción de la solución recomendada
    pub description: String,
}

impl Remediation {
    /// Crea una nueva remediación
    pub fn new(effort_minutes: u32, description: impl Into<String>) -> Self {
        Self {
            effort_minutes,
            description: description.into(),
        }
    }

    /// Remediación rápida (menos de 5 minutos)
    pub fn quick(description: impl Into<String>) -> Self {
        Self::new(5, description)
    }

    /// Remediación moderada (5-30 minutos)
    pub fn moderate(description: impl Into<String>) -> Self {
        Self::new(15, description)
    }

    /// Remediación significativa (más de 30 minutos)
    pub fn substantial(description: impl Into<String>) -> Self {
        Self::new(60, description)
    }
}
```

---

## 3. The `declare_rule!` Macro

El macro `declare_rule!` elimina el boilerplate para definir reglas. Genera automáticamente la estructura, la implementación del trait `Rule`, y el registro en el inventory compile-time.

### 3.1 Definición del Macro

```rust
/// Macro para declarar una regla con registro automático.
///
/// # Uso
///
/// ```ignore
/// declare_rule! {
///     /// ID de la regla (ej. "S138")
///     id: "S138"
///     /// Nombre descriptivo
///     name: "Functions should not be too long"
///     /// Severidad por defecto
///     severity: Major
///     /// Categoría
///     category: CodeSmell
///     /// Lenguaje objetivo
///     language: "rust"
///     /// Parámetros configurables (opcional)
///     params: {
///         /// Umbral máximo de líneas (default: 50)
///         threshold: usize = 50
///     }
///     /// Lógica de la regla
///     check: |ctx| {
///         // Tu código aquí
///         vec![]
///     }
/// }
/// ```
///
/// El macro genera:
/// - Struct `S138Rule` con campos para cada parámetro
/// - Implementación de `Rule` trait
/// - Registro en `inventory::collect!` al compile-time
#[proc_macro]
pub fn declare_rule(input: TokenStream) -> TokenStream {
    // El macro procesa el token stream y genera:
    // 1. Struct con defaults para cada parámetro
    // 2. impl Rule para la regla
    // 3. inventory::submit!(RuleEntry { factory })
}
```

### 3.2 Ejemplo: Regla Simple (S1135 - TODO Tags)

La regla más simple posible: detección por regex en comentarios:

```rust
use regex::Regex;
use crate::{declare_rule, Severity, Category, Issue};

declare_rule! {
    /// Detecta comentarios TODO/FIXME que quedaron en el código.
    ///
    /// # Justificación
    ///
    /// Los TODOs pendientes indican trabajo incompleto que no debe
    /// llegar a producción sin atención.
    id: "S1135"
    name: "TODO tags should be completed or removed"
    severity: Minor
    category: CodeSmell
    language: "*"  // Aplica a todos los lenguajes
    params: {}
    check: |ctx| {
        let mut issues = Vec::new();
        let re = Regex::new(r"(?i)(TODO|FIXME|HACK|XXX):?\s*(.*)").unwrap();

        // Buscar en comentarios usando tree-sitter
        let query = tree_sitter::Query::new(
            ctx.language.comment_query(),
            "(comment) @comment"
        ).unwrap();

        let mut cursor = tree_sitter::QueryCursor::new();
        for match_ in cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes()) {
            for capture in match_.captures {
                if let Some(text) = capture.node.utf8_text(ctx.source.as_bytes()) {
                    if let Some(m) = re.find(text) {
                        let (start_row, start_col) = capture.node.start_position();
                        issues.push(Issue::new(
                            "S1135",
                            format!("TODO comment found: {}", m.as_str()),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            start_row + 1,  // 1-indexed
                        ).with_column(start_col));
                    }
                }
            }
        }

        issues
    }
}
```

### 3.3 Ejemplo: Regla con Parámetros (S138 - Long Method)

```rust
use crate::{declare_rule, Severity, Category, Issue, RuleContext};

declare_rule! {
    /// Detecta funciones que exceden un umbral de líneas.
    ///
    /// # Parámetros
    ///
    /// - `threshold`: Número máximo de líneas en una función (default: 50)
    ///
    /// # Justificación
    ///
    /// Las funciones largas son difíciles de leer, testear y mantener.
    /// El código legible típicamente cabe en una pantalla (~50 líneas).
    id: "S138"
    name: "Functions should not be too long"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {
        /// Líneas máximas antes de reportar como issue
        threshold: usize = 50
    }
    check: |ctx| {
        let mut issues = Vec::new();

        // Query para encontrar todas las funciones
        let query_str = ctx.language.function_query();
        let query = tree_sitter::Query::new(ctx.language.language(), query_str)
            .unwrap();
        let cursor = tree_sitter::QueryCursor::new();

        for match_ in cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes()) {
            for capture in match_.captures {
                let func_node = capture.node;

                // Calcular líneas usando helper del contexto
                let line_count = ctx.line_count(func_node);

                // Verificar contra threshold (acceso al parámetro)
                if line_count > *ctx.params.get::<usize>("threshold").unwrap_or(&50) {
                    let (start_row, start_col) = func_node.start_position();
                    let func_name = ctx.function_name(func_node)
                        .unwrap_or_else(|| "anonymous".to_string());

                    issues.push(Issue::new(
                        "S138",
                        format!(
                            "Function `{}` has {} lines, exceeds threshold of {}",
                            func_name, line_count,
                            ctx.params.get::<usize>("threshold").unwrap_or(&50)
                        ),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_row + 1,
                    ).with_column(start_col));
                }
            }
        }

        issues
    }
}
```

### 3.4 Ejemplo: Regla Compleja (S3776 - Cognitive Complexity)

```rust
use crate::{declare_rule, Severity, Category, Issue, RuleContext};

declare_rule! {
    /// Analiza la complejidad cognitiva de funciones.
    ///
    /// # Parámetros
    ///
    /// - `threshold`: Puntuación máxima de complejidad cognitiva (default: 15)
    ///
    /// # Algoritmo
    ///
    /// La complejidad cognitiva mide qué tan difícil es entender una función.
    /// Incrementa por:
    /// - Recursión (+1 por nivel)
    /// - Conditional operators (if, while, for, case) (+1)
    /// - Logical operators (&&, ||) (+1)
    /// - Jump statements (return, break, continue) con condiciones (+1)
    /// - Recursión (+1 por nivel)
    /// - Nested structures (cada nivel anidado +1)
    id: "S3776"
    name: "Cognitive complexity should not be too high"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {
        /// Puntuación máxima de complejidad cognitiva
        threshold: u32 = 15
    }
    check: |ctx| {
        let mut issues = Vec::new();
        let threshold = *ctx.params.get::<u32>("threshold").unwrap_or(&15);

        let query_str = ctx.language.function_query();
        let query = tree_sitter::Query::new(ctx.language.language(), query_str).unwrap();
        let cursor = tree_sitter::QueryCursor::new();

        for match_ in cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes()) {
            for capture in match_.captures {
                let func_node = capture.node;
                let complexity = ctx.cognitive_complexity(func_node);

                if complexity > threshold as i32 {
                    let (start_row, start_col) = func_node.start_position();
                    let func_name = ctx.function_name(func_node)
                        .unwrap_or_else(|| "anonymous".to_string());

                    issues.push(Issue::new(
                        "S3776",
                        format!(
                            "Function `{}` has cognitive complexity of {}, exceeds threshold of {}",
                            func_name, complexity, threshold
                        ),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_row + 1,
                    ).with_column(start_col)
                    .with_remediation(crate::Remediation::substantial(
                        "Consider extracting helper functions or simplifying logic flow"
                    )));
                }
            }
        }

        issues
    }
}
```

### 3.5 Ejemplo: Regla de Seguridad (S2068 - Hard-coded Password)

```rust
use crate::{declare_rule, Severity, Category, Issue, RuleContext};
use regex::Regex;

declare_rule! {
    /// Detecta contraseñas o secrets hard-coded en el código.
    ///
    /// # Justificación
    ///
    /// Las credenciales hard-coded son un riesgo de seguridad crítico.
    /// Pueden ser filtradas en repositorios públicos o extraídas por atacantes.
    ///
    /// # Patrones detectados
    ///
    /// - `password = "..."`
    /// - `api_key = "..."`
    /// - `secret = "..."`
    /// - `token = "..."`
    /// - Credenciales en URLs
    id: "S2068"
    name: "Hard-coded passwords are security hotspots"
    severity: Blocker
    category: SecurityHotspot
    language: "*"
    params: {
        /// Lista de palabras clave adicionales a detectar
        extra_keywords: Vec<String> = vec![]
    }
    check: |ctx| {
        let mut issues = Vec::new();

        // Patrones de credenciales
        let patterns = [
            r"(?i)(password|passwd|pwd)\s*[=:]\s*[\"'][^\"']+[\"']",
            r"(?i)(api[_-]?key|apikey)\s*[=:]\s*[\"'][^\"']+[\"']",
            r"(?i)(secret|token)\s*[=:]\s*[\"'][^\"']+[\"']",
            r"(?i)(bearer|auth)\s+[A-Za-z0-9._-]+",
        ];

        // Compilar patrones dinámicamente si hay keywords extra
        let mut all_patterns = patterns.to_vec();
        if let Some(extra) = ctx.params.get::<Vec<String>>("extra_keywords") {
            for kw in extra {
                all_patterns.push(&format!(r"(?i){}\s*[=:]\s*[\"'][^\"']+[\"']", kw));
            }
        }

        let regexes: Vec<Regex> = all_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        // Buscar en el código fuente completo
        for re in &regexes {
            for m in re.find_iter(ctx.source) {
                let start = ctx.source[..m.start()].chars().count();
                let line = ctx.source[..m.start()].lines().count() + 1;

                issues.push(Issue::new(
                    "S2068",
                    format!("Hard-coded secret detected: {}", m.as_str()),
                    Severity::Blocker,
                    Category::SecurityHotspot,
                    ctx.file_path,
                    line,
                ).with_remediation(crate::Remediation::moderate(
                    "Use environment variables or a secrets manager instead of hard-coded values"
                )));
            }
        }

        issues
    }
}
```

### 3.6 Ejemplo: Regla de Arquitectura (DDD Boundary)

```rust
use crate::{declare_rule, Severity, Category, Issue, RuleContext};

declare_rule! {
    /// Verifica que los bounded contexts de DDD respeten sus límites.
    ///
    /// # Justificación
    ///
    /// En DDD, los bounded contexts definen límites de modelos de dominio.
    /// Cruzar estos límites directamente (imports entre contextos) viola
    /// la encapsulación del dominio.
    ///
    /// # Configuración
    ///
    /// Requiere que se defina `domain_boundaries` en la configuración
    /// del proyecto, mapeando contextos a sus módulos autorizados.
    id: "DDD001"
    name: "Bounded context boundaries should be respected"
    severity: Critical
    category: Bug  // Violación de invariantes de dominio
    language: "rust"
    params: {
        /// Mapa de contextos a módulos autorizados (JSON en config)
        boundaries: std::collections::HashMap<String, Vec<String>> = default
    }
    check: |ctx| {
        let mut issues = Vec::new();
        let boundaries = ctx.params.get::<std::collections::HashMap<String, Vec<String>>>("boundaries");

        // Obtener imports del archivo actual
        let imports = ctx.query_imports();

        // Determinar a qué contexto pertenece este archivo
        let current_context = ctx.detect_bounded_context();
        if current_context.is_none() {
            return issues;  // No es código de dominio
        }

        let current = current_context.unwrap();

        for import in imports {
            // Extraer el módulo destino del import
            if let Some(target_context) = ctx.resolve_import_context(&import) {
                if current != target_context {
                    // Cross-context import: verificar si está autorizado
                    let authorized = boundaries
                        .and_then(|b| b.get(&current))
                        .map(|allowed| allowed.contains(&target_context))
                        .unwrap_or(false);

                    if !authorized {
                        let (line, col) = import.location;

                        issues.push(Issue::new(
                            "DDD001",
                            format!(
                                "Cross-context import from `{}` to `{}` violates bounded context boundary",
                                current, target_context
                            ),
                            Severity::Critical,
                            Category::Bug,
                            ctx.file_path,
                            line,
                        ).with_column(col)
                        .with_remediation(crate::Remediation::substantial(
                            "Use an anti-corruption layer or shared kernel pattern instead of direct cross-context imports"
                        )));
                    }
                }
            }
        }

        issues
    }
}
```

---

## 4. Auto-Registration con `inventory`

El Rule Engine usa el crate `inventory` para registro automático compile-time. Esto significa que **no hay pasos manuales de registro**: cualquier struct que implemente `Rule` y esté visible en el módulo correcto se descubre automáticamente.

### 4.1 Cómo Funciona `inventory::collect!`

El crate `inventory` implementa un patrón de plugin registry usando macros de Rust:

```rust
// En el crate inventory (simplificado):
mod Registry {
    use std::sync::RwLock;

    pub struct RuleEntry {
        pub factory: fn() -> Box<dyn Rule>,
    }

    // Storage estático con impl de Rust para sincronización
    static RULES: RwLock<Vec<RuleEntry>> = RwLock::new(Vec::new());

    pub fn submit(entry: RuleEntry) {
        RULES.write().unwrap().push(entry);
    }

    pub fn collect() -> Vec<RuleEntry> {
        RULES.read().unwrap().clone()
    }
}

// Macro submit! del usuario:
// inventory::submit!(RuleEntry { factory: || Box::new(MyRule::new()) });
```

### 4.2 Estructura de `RuleEntry`

```rust
/// Entrada de registro para una regla.
///
/// El macro `declare_rule!` genera automáticamente la creación
/// de esta estructura y su registro.
#[derive(Clone)]
pub struct RuleEntry {
    /// Factory function que crea una nueva instancia de la regla
    factory: fn() -> Box<dyn Rule>,
}

impl RuleEntry {
    /// Crea una nueva entrada con la factory especificada
    pub fn new<R: Rule + 'static>() -> Self {
        Self {
            factory: || Box::new(R::new()),
        }
    }

    /// Crea una instancia de la regla
    pub fn instantiate(&self) -> Box<dyn Rule> {
        (self.factory)()
    }
}
```

### 4.3 `RuleRegistry::discover()`

```rust
use inventory::collect;
use std::collections::HashMap;

/// Registro central de todas las reglas descubiertas.
pub struct RuleRegistry {
    /// Todas las reglas registradas
    rules: Vec<Box<dyn Rule>>,

    /// Índice por lenguaje
    by_language: HashMap<String, Vec<usize>>,

    /// Índice por categoría
    by_category: HashMap<Category, Vec<usize>>,

    /// Índice por severidad
    by_severity: HashMap<Severity, Vec<usize>>,
}

impl RuleRegistry {
    /// Descubre y registra todas las reglas disponibles.
    ///
    /// Debe llamarse una vez al inicio de la aplicación.
    /// Recopila todas las entradas del inventory y las instancia.
    pub fn discover() -> Self {
        let entries = collect::<RuleEntry>();

        let rules: Vec<Box<dyn Rule>> = entries
            .into_iter()
            .map(|e| e.instantiate())
            .collect();

        // Construir índices
        let mut by_language: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_category: HashMap<Category, Vec<usize>> = HashMap::new();
        let mut by_severity: HashMap<Severity, Vec<usize>> = HashMap::new();

        for (idx, rule) in rules.iter().enumerate() {
            // Por lenguaje
            let lang = rule.language().to_string();
            by_language
                .entry(lang)
                .or_default()
                .push(idx);

            // Por categoría
            by_category
                .entry(rule.category())
                .or_default()
                .push(idx);

            // Por severidad
            by_severity
                .entry(rule.severity())
                .or_default()
                .push(idx);
        }

        Self {
            rules,
            by_language,
            by_category,
            by_severity,
        }
    }

    /// Retorna todas las reglas
    pub fn all(&self) -> &[Box<dyn Rule>] {
        &self.rules
    }

    /// Filtra reglas por lenguaje
    pub fn for_language(&self, language: &str) -> Vec<&dyn Rule> {
        self.by_language
            .get(language)
            .map(|indices| indices.iter().map(|i| self.rules[*i].as_ref()).collect())
            .unwrap_or_default()
    }

    /// Filtra reglas por categoría
    pub fn for_category(&self, category: Category) -> Vec<&dyn Rule> {
        self.by_category
            .get(&category)
            .map(|indices| indices.iter().map(|i| self.rules[*i].as_ref()).collect())
            .unwrap_or_default()
    }

    /// Filtra reglas por severidad mínima
    pub fn with_min_severity(&self, min_severity: Severity) -> Vec<&dyn Rule> {
        self.rules
            .iter()
            .filter(|r| r.severity() >= min_severity)
            .map(|r| r.as_ref())
            .collect()
    }

    /// Combina múltiples filtros
    pub fn query(
        &self,
        language: Option<&str>,
        categories: Option<&[Category]>,
        min_severity: Option<Severity>,
    ) -> Vec<&dyn Rule> {
        self.rules
            .iter()
            .filter(|r| {
                // Filtro por lenguaje
                if let Some(lang) = language {
                    if r.language() != lang && r.language() != "*" {
                        return false;
                    }
                }

                // Filtro por categoría
                if let Some(cats) = categories {
                    if !cats.contains(&r.category()) {
                        return false;
                    }
                }

                // Filtro por severidad mínima
                if let Some(min) = min_severity {
                    if r.severity() < min {
                        return false;
                    }
                }

                true
            })
            .map(|r| r.as_ref())
            .collect()
    }
}
```

### 4.4 Flujo de Registro Completo

```
┌──────────────────────────────────────────────────────────────────┐
│                     Compilación del crate                         │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  1. macro_rules! declare_rule expande en:                        │
│     ┌─────────────────────────────────────────────────────┐     │
│     │ struct S138Rule {                                    │     │
│     │     threshold: usize,                                │     │
│     │ }                                                    │     │
│     │                                                      │     │
│     │ impl Rule for S138Rule { ... }                       │     │
│     │                                                      │     │
│     │ inventory::submit!(RuleEntry {                       │     │
│     │     factory: || Box::new(S138Rule::new())             │     │
│     │ });                                                  │     │
│     └─────────────────────────────────────────────────────┘     │
│                                                                   │
│  2. inventory::collect! al runtime.collect():                    │
│     ┌─────────────────────────────────────────────────────┐     │
│     │ RULES = [                                            │     │
│     │     RuleEntry { factory: S138::new },                │     │
│     │     RuleEntry { factory: S3776::new },                │     │
│     │     RuleEntry { factory: S1135::new },                │     │
│     │     ...                                              │     │
│     │ ]                                                    │     │
│     └─────────────────────────────────────────────────────┘     │
│                                                                   │
│  3. RuleRegistry::discover() consume RULES:                      │
│     ┌─────────────────────────────────────────────────────┐     │
│     │ registry = RuleRegistry {                            │     │
│     │     rules: [S138, S3776, S1135, ...],                │     │
│     │     by_language: { "rust" -> [S138, S3776], ... },   │     │
│     │     by_category: { CodeSmell -> [...], ... },        │     │
│     │ }                                                    │     │
│     └─────────────────────────────────────────────────────┘     │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### 4.5 Beneficios del Sistema

1. **Cero boilerplate**: Agregar una regla solo requiere crear el archivo y declararla con `declare_rule!`
2. **Compile-time safety**: Si la regla no compila, el crate no compila
3. **Descubrimiento automático**: No hay lista manual de reglas a mantener
4. **Plugin-like**: Las reglas son efectivamente plugins compilados estáticamente

---

## 5. Build Script para Auto-Discovery (Opcional)

Para proyectos que quieren generación automática de módulos, el build script escanea el directorio de reglas y genera declaraciones `mod`.

### 5.1 `build.rs` Estándar

```rust
// build.rs
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let rules_dir = Path::new("src/rules/catalog");

    if !rules_dir.exists() {
        println!("cargo:warning=No rules directory found at {:?}", rules_dir);
        return;
    }

    // Encontrar todos los archivos .rs en el directorio de reglas
    let mut modules = Vec::new();
    collect_modules(rules_dir, "", &mut modules);

    // Generar contenido del módulo auto-generado
    let module_decls: String = modules
        .iter()
        .map(|(path, name)| format!("mod {};", name))
        .collect::<Vec<_>>()
        .join("\n");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("rules_catalog.rs");

    fs::write(&dest_path, &module_decls).unwrap();

    // Rerun si cambia cualquier archivo de reglas
    println!("cargo:rerun-if-changed={}", rules_dir.display());

    // Incluir el archivo generado en build
    println!("cargo:rustc-cfg=rules_auto_generated");
}
```

### 5.2 Recolección Recursiva de Módulos

```rust
fn collect_modules(dir: &Path, prefix: &str, modules: &mut Vec<(String, String)>) {
    if !dir.is_dir() {
        return;
    }

    // Leer entradas del directorio
    let entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            // Directorio: recursionar con nuevo prefijo
            let new_prefix = if prefix.is_empty() {
                file_name.clone()
            } else {
                format!("{}_{}", prefix, file_name)
            };
            collect_modules(&path, &new_prefix, modules);
        } else if file_name.ends_with(".rs") {
            // Archivo .rs: generar módulo
            let module_name = file_name.trim_end_matches(".rs");
            let full_path = if prefix.is_empty() {
                module_name.to_string()
            } else {
                format!("{}_{}", prefix, module_name)
            };

            modules.push((full_path.clone(), full_path));
        }
    }
}
```

### 5.3 Uso del Módulo Generado

```rust
// src/rules/mod.rs

#[cfg(rules_auto_generated)]
include!(concat!(env!("OUT_DIR"), "/rules_catalog.rs"));

#[cfg(not(rules_auto_generated))]
mod manual_catalog;
```

### 5.4 Fallback a Declaración Manual

Si el build script no es deseado, las declaraciones manuales funcionan igual:

```rust
// src/rules/catalog/rust/mod.rs

pub mod s138_long_method;
pub mod s3776_cognitive_complexity;
pub mod s134_deep_nesting;
pub mod s107_too_many_params;
pub mod s2068_hardcoded_password;
pub mod s1135_todo_tags;

// Agregar nuevas reglas aquí...
```

**Recomendación**: Usar el build script para proyectos con más de 20 reglas. Para proyectos más pequeños, las declaraciones manuales ofrecen mejor claridad y debugabilidad.

---

## 6. Rule Context Helpers

`RuleContext` proporciona métodos helper que abstraen operaciones comunes sobre el AST, el grafo de llamadas, y las métricas.

### 6.1 Helpers de Extracción de AST

```rust
impl<'a> RuleContext<'a> {
    /// Obtiene todas las funciones/métodos en el archivo actual.
    ///
    /// Retorna un vector de nodos del AST que representan funciones.
    pub fn query_functions(&self) -> Vec<tree_sitter::Node<'a>> {
        let query_str = self.language.function_query();
        let query = tree_sitter::Query::new(self.language.language(), query_str).unwrap();
        let cursor = tree_sitter::QueryCursor::new();

        cursor
            .matches(&query, self.tree.root_node(), self.source.as_bytes())
            .flat_map(|m| m.captures.iter().map(|c| c.node))
            .collect()
    }

    /// Obtiene todas las declaraciones de import/use/require.
    pub fn query_imports(&self) -> Vec<ImportInfo<'a>> {
        let query_str = self.language.import_query();
        let query = tree_sitter::Query::new(self.language.language(), query_str).unwrap();
        let cursor = tree_sitter::QueryCursor::new();

        cursor
            .matches(&query, self.tree.root_node(), self.source.as_bytes())
            .flat_map(|m| {
                m.captures
                    .iter()
                    .filter_map(|c| self.parse_import_node(c.node))
            })
            .collect()
    }

    /// Obtiene todas las clases/impls/structs del archivo.
    pub fn query_classes(&self) -> Vec<ClassInfo<'a>> {
        let query_str = self.language.class_query();
        let query = tree_sitter::Query::new(self.language.language(), query_str).unwrap();
        let cursor = tree_sitter::QueryCursor::new();

        cursor
            .matches(&query, self.tree.root_node(), self.source.as_bytes())
            .flat_map(|m| {
                m.captures
                    .iter()
                    .filter_map(|c| self.parse_class_node(c.node))
            })
            .collect()
    }

    /// Ejecuta una tree-sitter query custom.
    ///
    /// Útil para reglas que necesitan buscar patrones específicos.
    ///
    /// # Ejemplo
    ///
    /// ```ignore
    /// let matches = ctx.query_patterns(
    ///     "(call_expression function: (identifier) @fn)"
    /// );
    /// for m in matches {
    ///     println!("Call: {:?}", m.captures[0].node.text(ctx.source));
    /// }
    /// ```
    pub fn query_patterns(&self, query_str: &str) -> Vec<QueryMatch<'a>> {
        let query = tree_sitter::Query::new(self.language.language(), query_str).unwrap();
        let cursor = tree_sitter::QueryCursor::new();

        cursor
            .matches(&query, self.tree.root_node(), self.source.as_bytes())
            .map(|m| QueryMatch {
                captures: m.captures
                    .iter()
                    .map(|c| CaptureInfo {
                        node: c.node,
                        index: c.index,
                    })
                    .collect(),
            })
            .collect()
    }
}

/// Información de un import parseado
#[derive(Debug)]
pub struct ImportInfo<'a> {
    pub path: &'a str,
    pub alias: Option<&'a str>,
    pub location: (usize, usize),  // (line, column)
}

/// Información de una clase/struct/impl
#[derive(Debug)]
pub struct ClassInfo<'a> {
    pub name: &'a str,
    pub node: tree_sitter::Node<'a>,
    pub location: (usize, usize),
}

/// Match resultante de una query
#[derive(Debug)]
pub struct QueryMatch<'a> {
    pub captures: Vec<CaptureInfo<'a>>,
}

#[derive(Debug)]
pub struct CaptureInfo<'a> {
    pub node: tree_sitter::Node<'a>,
    pub index: u32,
}
```

### 6.2 Helpers de Métricas

```rust
impl<'a> RuleContext<'a> {
    /// Cuenta las líneas que ocupa un nodo en el código fuente.
    ///
    /// Incluye la línea de inicio y fin del nodo.
    pub fn line_count(&self, node: tree_sitter::Node) -> usize {
        let start = node.start_position().row;
        let end = node.end_position().row;
        end - start + 1
    }

    /// Calcula la profundidad máxima de anidamiento en un nodo.
    ///
    /// Recorre el árbol y encuentra el camino más profundo.
    pub fn nesting_depth(&self, node: tree_sitter::Node) -> usize {
        self.max_nesting_impl(node, 0)
    }

    fn max_nesting_impl(&self, node: tree_sitter::Node, current_depth: usize) -> usize {
        let mut max_depth = current_depth;

        // Nodos que incrementan el anidamiento
        let nesting_kinds = self.language.nesting_kinds();

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let kind = child.kind();
                let new_depth = if nesting_kinds.contains(kind) {
                    current_depth + 1
                } else {
                    current_depth
                };

                let child_max = self.max_nesting_impl(child, new_depth);
                max_depth = max_depth.max(child_max);
            }
        }

        max_depth
    }

    /// Calcula la complejidad ciclomática de un nodo.
    ///
    /// Usa el ComplexityCalculator de cognicode-core.
    pub fn cyclomatic_complexity(&self, node: tree_sitter::Node) -> i32 {
        // Delegar al ComplexityCalculator existente
        self.metrics
            .cyclomatic_complexity(node)
            .unwrap_or(1)
    }

    /// Calcula la complejidad cognitiva de un nodo.
    ///
    /// Implementación del algoritmo de SonarSource.
    pub fn cognitive_complexity(&self, node: tree_sitter::Node) -> i32 {
        use crate::metrics::CognitiveComplexity;

        let visitor = CognitiveComplexity::new(self.language);
        visitor.calculate(node, self.source)
    }
}
```

### 6.3 Helpers del CallGraph

```rust
impl<'a> RuleContext<'a> {
    /// Retorna los símbolos que llaman a la función dada.
    ///
    /// Busca en el CallGraph (basado en PetGraph) los nodos
    /// que tienen edges hacia el símbolo especificado.
    pub fn callers_of(&self, symbol: &str) -> Vec<&str> {
        let node_idx = self
            .graph
            .find_node(symbol)
            .map(|n| n.id())
            .unwrap_or_default();

        self.graph
            .edges_directed(node_idx, petgraph::EdgeDirection::Incoming)
            .filter_map(|e| {
                self.graph
                    .node_weight(e.source())
                    .map(|n| n.name.as_str())
            })
            .collect()
    }

    /// Retorna los símbolos que la función dada llama.
    pub fn callees_of(&self, symbol: &str) -> Vec<&str> {
        let node_idx = self
            .graph
            .find_node(symbol)
            .map(|n| n.id())
            .unwrap_or_default();

        self.graph
            .edges_directed(node_idx, petgraph::EdgeDirection::Outgoing)
            .filter_map(|e| {
                self.graph
                    .node_weight(e.target())
                    .map(|n| n.name.as_str())
            })
            .collect()
    }

    /// Detecta código muerto: símbolos sin llamadores.
    ///
    /// Útil para reglas como S1484 (Unused function parameters)
    /// o para detectar funciones nunca llamadas.
    pub fn dead_code(&self) -> Vec<&str> {
        self.graph
            .nodes()
            .filter(|node| {
                // Filtrar por lenguaje del archivo actual
                // (para no marcar símbolos de otros archivos como dead)
                node.metadata
                    .file_path
                    .map(|p| p == self.file_path)
                    .unwrap_or(false)
            })
            .filter(|node| {
                // Sin incoming edges = nunca llamado
                self.graph
                    .edges_directed(node.id(), petgraph::EdgeDirection::Incoming)
                    .count() == 0
            })
            .map(|n| n.name.as_str())
            .collect()
    }
}
```

### 6.4 Helper de Nombre de Función

```rust
impl<'a> RuleContext<'a> {
    /// Extrae el nombre de una función de su nodo AST.
    ///
    /// Maneja diferencias entre lenguajes:
    /// - Rust: `fn foo()` → "foo"
    /// - TypeScript: `function foo()` o `const foo = () =>`
    /// - Python: `def foo():` → "foo"
    pub fn function_name(&self, node: tree_sitter::Node) -> Option<&'a str> {
        let kind = node.kind();

        match kind {
            // Rust: function_declaration o method_declaration
            "function_declaration" | "function_item" | "method_declaration" => {
                node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
            }

            // TypeScript/JavaScript
            "function" | "method_definition" | "arrow_function" => {
                node.child_by_field_name("name")
                    .or_else(|| node.child(0))
                    .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
            }

            // Python
            "function_definition" | "async_function_definition" => {
                node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
            }

            // Go
            "function_declaration" => {
                node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
            }

            // Java
            "method_declaration" | "constructor_declaration" => {
                node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
            }

            _ => None,
        }
    }
}
```

---

## 7. Performance Considerations

El Rule Engine está diseñado para analizar bases de código grandes (miles de archivos) en tiempo razonable.

### 7.1 Ejecución Paralela con Rayon

```rust
/// Analiza múltiples archivos en paralelo usando Rayon.
///
/// # Performance
///
/// - I/O-bound: parsing y lectura de archivos paralelizado
/// - CPU-bound: reglas se ejecutan en thread pool de Rayon
/// - Escalabilidad:lineal con número de cores disponibles
pub fn analyze_files(paths: &[PathBuf], registry: &RuleRegistry) -> Vec<Issue> {
    paths
        .par_iter()  // Parallel iterator
        .flat_map(|path| {
            let issues = analyze_single_file(path, registry);
            issues
        })
        .collect()
}

/// Analiza un archivo individual
fn analyze_single_file(path: &Path, registry: &RuleRegistry) -> Vec<Issue> {
    // 1. Parsear archivo (tree-sitter)
    let source = std::fs::read_to_string(path).unwrap();
    let language = Language::detect(path, &source).unwrap();
    let tree = language.parse(&source).unwrap();

    // 2. Cargar métricas (cachadas)
    let metrics = FileMetrics::for_file(path);

    // 3. Crear contexto
    let ctx = RuleContext {
        tree: &tree,
        source: &source,
        file_path: path,
        language,
        graph: &CallGraph::for_project(),
        metrics: &metrics,
    };

    // 4. Obtener reglas aplicables
    let applicable_rules = registry.for_language(ctx.language.name());

    // 5. Ejecutar reglas en paralelo
    applicable_rules
        .par_iter()
        .flat_map(|rule| rule.check(&ctx))
        .collect()
}
```

### 7.2 Caching de Parsing

```rust
/// Cache global de árboles parseados.
///
/// Evita re-parsear archivos que no cambiaron entre análisis.
pub struct ParseCache {
    cache: RwLock<LruCache<PathBuf, (Tree, Language)>>,
}

impl ParseCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: RwLock::new(LruCache::new(capacity)),
        }
    }

    /// Obtiene o parsea un archivo.
    pub fn get_or_parse(&self, path: &Path, source: &str) -> (Tree, Language) {
        // Intentar leer del cache primero
        if let Some(cached) = self.cache.read().unwrap().get(path) {
            return cached.clone();
        }

        // Parsear y cachear
        let language = Language::detect(path, source).unwrap();
        let tree = language.parse(source).unwrap();

        self.cache
            .write()
            .unwrap()
            .put(path.to_path_buf(), (tree.clone(), language.clone()));

        (tree, language)
    }

    /// Invalida entradas del cache.
    pub fn invalidate(&self, path: &Path) {
        self.cache.write().unwrap().pop(path);
    }
}
```

### 7.3 Caching de Query Results

```rust
impl RuleContext<'_> {
    // Cache interno para resultados de queries frecuentes
    thread_local! {
        static QUERY_CACHE: RefCell<HashMap<String, Vec<QueryMatch>>> =
            RefCell::new(HashMap::new());
    }

    /// Versión cached de query_patterns.
    ///
    /// Los resultados se cachean por query string para evitar
    /// re-ejecutar tree-sitter queries idénticas.
    pub fn query_patterns_cached(&self, query_str: &str) -> Vec<QueryMatch<'_>> {
        QUERY_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();

            if let Some(cached) = cache.get(query_str) {
                return cached.clone();
            }

            let result = self.query_patterns(query_str);
            cache.insert(query_str.to_string(), result.clone());

            result
        })
    }
}
```

### 7.4 Análisis Incremental

```rust
/// Analiza solo archivos que cambiaron desde el último análisis.
///
/// Requiere mantener estado del último análisis:
/// - timestamps de archivos
/// - hashes de contenido
/// - issues previos
pub struct IncrementalAnalyzer {
    previous_state: HashMap<PathBuf, FileState>,
    cache: ParseCache,
}

struct FileState {
    mtime: std::time::SystemTime,
    content_hash: u64,
    previous_issues: Vec<Issue>,
}

impl IncrementalAnalyzer {
    /// Analiza archivos, retornando solo issues de archivos cambiados.
    pub fn analyze_changed(&mut self, paths: &[PathBuf]) -> Vec<Issue> {
        let changed: Vec<PathBuf> = paths
            .iter()
            .filter(|p| self.has_changed(p))
            .cloned()
            .collect();

        let new_issues = self.analyze_files(&changed);

        // Actualizar estado
        for path in &changed {
            self.update_state(path);
        }

        new_issues
    }

    fn has_changed(&self, path: &Path) -> bool {
        let current_mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok();

        let previous = self.previous_state.get(path);

        match (current_mtime, previous) {
            (Some(mtime), Some(prev)) => mtime != prev.mtime,
            _ => true,  // Nuevo archivo o sin estado previo
        }
    }
}
```

### 7.5 Budget y Early Termination

```rust
/// Configuración de budget para análisis.
///
/// Previene que reglas problemáticos dominen el tiempo de análisis.
#[derive(Debug, Clone)]
pub struct AnalysisBudget {
    /// Máximo de issues por archivo
    pub max_issues_per_file: usize,

    /// Máximo de issues críticos antes de abortar
    pub max_critical_before_abort: usize,

    /// Timeout por archivo (millisegundos)
    pub timeout_per_file_ms: u64,
}

impl Default for AnalysisBudget {
    fn default() -> Self {
        Self {
            max_issues_per_file: 100,
            max_critical_before_abort: 10,
            timeout_per_file_ms: 5000,
        }
    }
}

impl RuleRegistry {
    /// Versión de analyze con budget.
    pub fn analyze_with_budget(
        &self,
        ctx: &RuleContext,
        budget: &AnalysisBudget,
    ) -> Vec<Issue> {
        let mut all_issues = Vec::new();
        let mut critical_count = 0;

        for rule in self.rules.iter() {
            // Early termination: demasiados críticos
            if critical_count >= budget.max_critical_before_abort {
                break;
            }

            let start = std::time::Instant::now();
            let issues = rule.check(ctx);
            let elapsed = start.elapsed();

            // Timeout check
            if elapsed.as_millis() > budget.timeout_per_file_ms as u128 {
                eprintln!(
                    "WARNING: Rule {} exceeded timeout ({}ms > {}ms)",
                    rule.id(),
                    elapsed.as_millis(),
                    budget.timeout_per_file_ms
                );
                continue;
            }

            // Contar críticos
            critical_count += issues
                .iter()
                .filter(|i| i.severity == Severity::Critical || i.severity == Severity::Blocker)
                .count();

            // Budget de issues por archivo
            if all_issues.len() + issues.len() > budget.max_issues_per_file {
                let remaining = budget.max_issues_per_file - all_issues.len();
                all_issues.extend(issues.into_iter().take(remaining));
                break;
            }

            all_issues.extend(issues);
        }

        all_issues
    }
}
```

---

## 8. Testing Rules

El Rule Engine incluye infraestructura de testing para validar que las reglas detectan correctamente los issues esperados.

### 8.1 Estructura de Test Fixtures

```
tests/
├── fixtures/
│   ├── rust/
│   │   ├── s138_long_method/
│   │   │   ├── valid.rs         # Función corta, sin issues
│   │   │   └── invalid.rs       # Función larga, 1 issue esperado
│   │   ├── s3776_cognitive_complexity/
│   │   │   ├── simple.rs
│   │   │   └── complex.rs
│   │   └── s2068_hardcoded_password/
│   │       ├── clean.rs
│   │       └── with_secrets.rs
│   └── typescript/
│       └── ...
└── rule_tests.rs
```

### 8.2 `RuleTestCase` Struct

```rust
/// Caso de prueba para una regla individual.
pub struct RuleTestCase {
    /// ID de la regla bajo prueba
    pub rule_id: &'static str,

    /// Contenido del fixture
    pub fixture_path: PathBuf,

    /// Issues esperados (vacío si se espera que no haya issues)
    pub expected_issues: Vec<ExpectedIssue>,
}

/// Issue esperado en un test.
#[derive(Debug)]
pub struct ExpectedIssue {
    /// Línea donde se espera el issue
    pub line: usize,

    /// Columna opcional
    pub column: Option<usize>,

    /// Mensaje esperado (substrings aceptados)
    pub message_contains: &'static str,
}
```

### 8.3 Macro para Declaración de Tests

```rust
/// Macro para declarar tests de reglas de forma concisa.
///
/// # Uso
///
/// ```ignore
/// rule_tests! {
///     s138_long_method {
///         valid: "tests/fixtures/rust/s138_long_method/valid.rs",
///         invalid: "tests/fixtures/rust/s138_long_method/invalid.rs"
///             => ExpectedIssue { line: 5, message_contains: "too long" }
///     }
///     s3776_cognitive_complexity {
///         simple: "tests/fixtures/rust/s3776_cognitive_complexity/simple.rs"
///             => 0  // 0 issues esperados
///         complex: "tests/fixtures/rust/s3776_cognitive_complexity/complex.rs"
///             => ExpectedIssue { line: 1, message_contains: "cognitive complexity" }
///     }
/// }
/// ```
#[macro_export]
macro_rules! rule_tests {
    (
        $(
            $rule_id:ident {
                $(
                    $name:ident : $fixture:literal => $expected:expr
                ),* $(,)?
            }
        )*
    ) {
        #[cfg(test)]
        mod rule_tests {
            use super::*;

            $(
                #[test]
                fn $rule_id() {
                    // Genera un test por cada caso
                    test_rule_fixture!(
                        stringify!($rule_id),
                        vec![
                            $(
                                TestCase {
                                    name: stringify!($name),
                                    fixture: $fixture,
                                    expected: $expected,
                                }
                            ),*
                        ]
                    );
                }
            )*
        }
    }
}
```

### 8.4 Ejemplo: Test para S138 (Long Method)

```rust
#[cfg(test)]
mod s138_tests {
    use super::*;

    #[test]
    fn s138_valid_function() {
        let source = r#"
fn short_function() {
    let x = 1;
    let y = 2;
    assert_eq!(x + y, 3);
}
"#;

        let rule = S138Rule::new();
        let issues = execute_rule(&rule, source, "test.rs");

        assert!(issues.is_empty(), "Expected no issues for short function");
    }

    #[test]
    fn s138_long_function() {
        let source = r#"
fn very_long_function() {
    let a = 1;  // line 2
    let b = 2;  // line 3
    let c = 3;  // line 4
    let d = 4;  // line 5
    let e = 5;  // line 6
    let f = 6;  // line 7
    let g = 7;  // line 8
    let h = 8;  // line 9
    let i = 9;  // line 10
    let j = 10; // line 11
    let k = 11; // line 12
    let l = 12; // line 13
    let m = 13; // line 14
    let n = 14; // line 15
    let o = 15; // line 16
    let p = 16; // line 17
    let q = 17; // line 18
    let r = 18; // line 19
    let s = 19; // line 20
    let t = 20; // line 21
    let u = 21; // line 22
    let v = 22; // line 23
    let w = 23; // line 24
    let x = 24; // line 25
    let y = 25; // line 26
    let z = 26; // line 27
    let aa = 27; // line 28
    let bb = 28; // line 29
    let cc = 29; // line 30
    let dd = 30; // line 31
    let ee = 31; // line 32
    let ff = 32; // line 33
    let gg = 33; // line 34
    let hh = 34; // line 35
    let ii = 35; // line 36
    let jj = 36; // line 37
    let kk = 37; // line 38
    let ll = 38; // line 39
    let mm = 39; // line 40
    let nn = 40; // line 41
    let oo = 41; // line 42
    let pp = 42; // line 43
    let qq = 43; // line 44
    let rr = 44; // line 45
    let ss = 45; // line 46
    let tt = 46; // line 47
    let uu = 47; // line 48
    let vv = 48; // line 49
    let ww = 49; // line 50
    let xx = 50; // line 51
    let yy = 51; // line 52
    // threshold is 50 lines
}
"#;

        let rule = S138Rule::new();
        let issues = execute_rule(&rule, source, "test.rs");

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "S138");
        assert!(issues[0].message.contains("very_long_function"));
        assert_eq!(issues[0].line, 2);  // Function starts at line 2
    }

    /// Helper para ejecutar una regla sobre source y retornar issues.
    fn execute_rule(rule: &dyn Rule, source: &str, file_path: &str) -> Vec<Issue> {
        let language = Language::Rust;
        let tree = language.parse(source).unwrap();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language,
            graph: &CallGraph::empty(),
            metrics: &FileMetrics::default(),
        };

        rule.check(&ctx)
    }
}
```

### 8.5 Test de Integración

```rust
#[cfg(test)]
mod integration_tests {
    use crate::RuleRegistry;

    #[test]
    fn all_rules_have_unique_ids() {
        let registry = RuleRegistry::discover();
        let mut ids: Vec<_> = registry.all().iter().map(|r| r.id()).collect();
        ids.sort();
        ids.dedup();

        assert_eq!(
            ids.len(),
            registry.all().len(),
            "Rule IDs must be unique"
        );
    }

    #[test]
    fn rules_for_each_language_have_valid_language() {
        let registry = RuleRegistry::discover();
        let languages = ["rust", "typescript", "python", "go", "java"];

        for lang in languages {
            let rules = registry.for_language(lang);
            for rule in rules {
                assert_eq!(
                    rule.language(),
                    lang,
                    "Rule {} claims to be for {} but language() returns {}",
                    rule.id(),
                    lang,
                    rule.language()
                );
            }
        }
    }

    #[test]
    fn no_rules_with_blocker_severity_are_sleeping() {
        // Reglas Blocker deben tener check implementado
        let registry = RuleRegistry::discover();

        for rule in registry.all() {
            if rule.severity() == Severity::Blocker {
                // Verificar que no es una regla de stub
                assert!(
                    !rule.name().contains("[STUB]"),
                    "Blocker rule {} has stub name",
                    rule.id()
                );
            }
        }
    }
}
```

---

## 9. Catalog Organization

El catálogo de reglas está organizado por lenguaje para facilitar el mantenimiento y la contribución de nuevas reglas.

### 9.1 Estructura de Directorios

```
rules/
├── catalog/
│   ├── rust/                    # Reglas específicas de Rust
│   │   ├── mod.rs               # Re-exports públicos
│   │   ├── s138_long_method.rs  # Detección de funciones largas
│   │   ├── s3776_cognitive_complexity.rs
│   │   ├── s107_too_many_params.rs
│   │   ├── s001_import_order.rs
│   │   ├── s1064_too_many_returns.rs
│   │   ├── s1130_empty_lines.rs
│   │   ├── s1172_dead_code.rs
│   │   └── ... (25+ reglas Rust)
│   │
│   ├── typescript/              # Reglas específicas de TypeScript
│   │   ├── mod.rs
│   │   ├── s138_long_method.rs
│   │   ├── s3776_cognitive_complexity.rs
│   │   ├── s1172_dead_code.rs
│   │   ├── s1854_unused_variable.rs
│   │   └── ... (30+ reglas TypeScript)
│   │
│   ├── python/                  # Reglas específicas de Python
│   │   ├── mod.rs
│   │   ├── s138_long_method.rs
│   │   ├── s001_no_tab_indentation.rs
│   │   ├── s1135_todo_tags.rs
│   │   └── ... (20+ reglas Python)
│   │
│   ├── go/                     # Reglas específicas de Go
│   │   ├── mod.rs
│   │   ├── s138_long_method.rs
│   │   ├── s1005_error_naming.rs
│   │   └── ... (15+ reglas Go)
│   │
│   └── java/                    # Reglas específicas de Java
│       ├── mod.rs
│       ├── s138_long_method.rs
│       ├── s1135_todo_tags.rs
│       └── ... (20+ reglas Java)
│
├── engine.rs          # RuleRegistry, discover(), analyze()
├── trait.rs           # Trait Rule
├── context.rs         # RuleContext con helpers
├── types.rs           # Issue, Severity, Category, Remediation
├── macros.rs          # declare_rule! macro
└── testing.rs         # Test infrastructure (RuleTestCase, etc.)
```

### 9.2 Módulo Público de Reglas

```rust
// src/rules/catalog/rust/mod.rs

//! Reglas específicas para el lenguaje Rust.
//!
//! Estas reglas aprovechan las particularidades del AST de Rust
//! y las convenciones del ecosistema Rust.

// Re-export todas las reglas públicas
pub use super::s138_long_method::S138Rule;
pub use super::s3776_cognitive_complexity::S3776Rule;
pub use super::s107_too_many_params::S107Rule;
pub use super::s001_import_order::S001Rule;
pub use super::s1064_too_many_returns::S1064Rule;
pub use super::s1130_empty_lines::S1130Rule;
pub use super::s1172_dead_code::S1172Rule;
pub use super::s134_deep_nesting::S134Rule;
pub use super::s2068_hardcoded_password::S2068Rule;
pub use super::s1135_todo_tags::S1135Rule;

// Lista completa para registry
inventory::collect! {
    pub static RUST_RULES: crate::rules::RuleEntry = [
        RuleEntry::new::<S138Rule>,
        RuleEntry::new::<S3776Rule>,
        RuleEntry::new::<S107Rule>,
        // ... todas las reglas
    ]
}
```

### 9.3 Archivo Principal del Rule Engine

```rust
// src/rules/mod.rs

//! Rule Engine - Capa de análisis de calidad de código.
//!
//! # Arquitectura
//!
//! El Rule Engine analiza código usando reglas declaradas con
//! el macro `declare_rule!`. Cada regla implementa el trait `Rule`
//! y es auto-registrada al compile-time usando `inventory`.
//!
//! # Uso
//!
//! ```ignore
//! let registry = RuleRegistry::discover();
//! let issues = registry.analyze_file("src/main.rs", &source);
//! ```
//!
//! # Agregar una Nueva Regla
//!
//! 1. Crear archivo en `src/rules/catalog/{language}/sXXX_name.rs`
//! 2. Implementar la regla usando `declare_rule!`
//! 3. Agregar `pub mod sXXX_name;` en el mod.rs del lenguaje
//! 4. La regla se descubre automáticamente al recompilar

pub mod catalog;
pub mod context;
pub mod engine;
pub mod macros;
pub mod testing;
pub mod trait;
pub mod types;

// Re-exports públicos
pub use context::RuleContext;
pub use engine::RuleRegistry;
pub use trait::Rule;
pub use types::{Category, Issue, Remediation, Severity};

/// Versión del Rule Engine
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

---

## Anexo: Catálogo de Reglas Implementadas

| ID | Nombre | Lenguaje | Categoría | Severidad |
|----|--------|-----------|-----------|-----------|
| S107 | Too many parameters |Todos | CodeSmell | Major |
| S113 | Line length | Todos | CodeSmell | Minor |
| S1135 | TODO tags | Todos | CodeSmell | Minor |
| S117 | Variable naming | Rust | CodeSmell | Minor |
| S134 | Deep nesting | Todos | CodeSmell | Major |
| S138 | Long method | Todos | CodeSmell | Major |
| S1854 | Unused variable | TS/JS | Bug | Major |
| S2068 | Hard-coded secrets | Todos | Security | Blocker |
| S3776 | Cognitive complexity | Todos | CodeSmell | Major |
| DDD001 | Bounded context boundary | Rust | Bug | Critical |

---

*Documento generado automáticamente. Para contribuir reglas, ver CONTRIBUTING.md.*
