# LCPG — Lightweight Code Property Graph

> **Fecha**: 11 de Mayo de 2026  
> **Versión**: 1.0  
> **Estado**: Diseño de implementación

---

## 1. Qué es el LCPG

El **Lightweight Code Property Graph** (LCPG) es un grafo de propiedades de código **ligero** que conecta símbolos dentro de un archivo individual. Su propósito es permitir análisis semántico sin el overhead de un análisis inter-procedural completo.

### 1.1 Definición Informal

El LCPG responde preguntas como:
- **¿Esta función es llamada desde algún lugar?**
- **¿Esta variable se lee después de asignarla?**
- **¿Este import se utiliza en alguna parte del código?**
- **¿Hay código que nunca puede ser ejecutado (dead code)?**

### 1.2 Qué NO es el LCPG

Es importante entender las limitaciones deliberadas del LCPG:

| Característica | LCPG | Análisis Inter-procedural Completo |
|---------------|------|-----------------------------------|
| **Alcance** | Un archivo | Proyecto completo |
| **Tipos** | No resuelve | Resuelve tipos completos |
| **Inter-procedural** | No | Sí |
| **Punteros/Refs** | Limitado | Completo |
| **Rendimiento** | O(n) por archivo | Puede ser O(n²) o peor |
| **Memoria** | Bajo | Alto |
| **Paralelizable** | Sí (por archivo) | Limitado |

### 1.3 Analogía Simple

```
LCPG ≈ Tabla de símbolos mejorada con un grafo de referencias

┌─────────────────────────────────────────────────────┐
│  Symbol Table Tradicional:                          │
│  ┌─────────┬─────────────────────────────────────┐  │
│  │ Symbol  │ Definición                          │  │
│  ├─────────┼─────────────────────────────────────┤  │
│  │ foo     │ src/main.rs:10                       │  │
│  │ bar     │ src/main.rs:25                       │  │
│  │ x       │ src/main.rs:30                       │  │
│  └─────────┴─────────────────────────────────────┘  │
│                                                      │
│  LCPG = Symbol Table + References Graph:            │
│  ┌─────────────────────────────────────────────────┐ │
│  │ foo ──calls──→ bar                             │ │
│  │ x ────────references──→ foo                    │ │
│  │ bar ──────references──→ x                      │ │
│  └─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

---

## 2. Estructura de Datos

### 2.1 Tipos Fundamentales

```rust
/// Identificador único para un símbolo
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub usize);

/// Identificador de archivo (para referencias cross-file)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub usize);

/// Span en el source code
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub offset: usize,
}
```

### 2.2 Tipos de Símbolos

```rust
/// Kind de símbolo
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    /// Función o método
    Function,
    /// Variable local
    Variable,
    /// Parámetro de función
    Parameter,
    /// Import o use statement
    Import,
    /// Definición de tipo (struct, enum, trait)
    Type,
    /// Constante
    Constant,
    /// Estructura
    Struct,
    /// Enumeración
    Enum,
    /// Trait
    Trait,
    /// Bloque impl
    Impl,
    /// Módulo
    Module,
    /// Label (para loops)
    Label,
    /// Campo de struct
    Field,
}

/// Visibilidad del símbolo
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// Público (pub)
    Public,
    /// Accesible desde el crate
    Crate,
    /// Accesible desde super (padre)
    Super,
    /// Privado
    Private,
}

impl Visibility {
    /// Retorna true si el símbolo es accesible desde fuera del módulo
    pub fn is_exported(&self) -> bool {
        matches!(self, Visibility::Public)
    }
}
```

### 2.3 Estructura Principal

```rust
/// El grafo de propiedades de código ligero
pub struct SymbolTable {
    /// Mapa de SymbolId → Symbol
    symbols: HashMap<SymbolId, Symbol>,

    /// Edges de llamadas (caller → callee)
    calls: Vec<CallEdge>,

    /// Edges de referencias (reference → definition)
    references: Vec<RefEdge>,

    /// Lookup rápido: nombre → SymbolIds
    by_name: HashMap<String, Vec<SymbolId>>,

    /// Lookup por file
    by_file: HashMap<FileId, Vec<SymbolId>>,

    /// Contador para asignar IDs únicos
    next_id: usize,
}

/// Edge de llamada: quien llama → quien es llamado
#[derive(Clone, Copy, Debug)]
pub struct CallEdge {
    pub caller: SymbolId,
    pub callee: SymbolId,
    pub span: Span,
}

/// Edge de referencia: quien referencia → a quien referencia
#[derive(Clone, Copy, Debug)]
pub struct RefEdge {
    pub source: SymbolId,
    pub target: SymbolId,
    pub span: Span,
}

/// Un símbolo en el grafo
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub visibility: Visibility,
    pub span: Span,
    pub file: FileId,
    /// Símbolos que referencian a este
    pub references_in: Vec<SymbolId>,
    /// Símbolos que este referencia
    pub references_out: Vec<SymbolId>,
    /// metadata adicional específica por kind
    pub metadata: SymbolMetadata,
}

#[derive(Clone, Debug)]
pub enum SymbolMetadata {
    Function(FunctionMetadata),
    Variable(VariableMetadata),
    Import(ImportMetadata),
    Type(TypeMetadata),
    None,
}

#[derive(Clone, Debug)]
pub struct FunctionMetadata {
    pub params: Vec<SymbolId>,
    pub return_type: Option<String>,
    pub is_async: bool,
    pub is_method: bool,
}

#[derive(Clone, Debug)]
pub struct VariableMetadata {
    pub is_mutable: bool,
    pub initial_value: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ImportMetadata {
    pub path: String,
    pub is_qualified: bool,
    pub is_wildcard: bool,
}

#[derive(Clone, Debug)]
pub struct TypeMetadata {
    pub fields: Vec<SymbolId>,
    pub methods: Vec<SymbolId>,
}
```

---

## 3. Cómo se Construye el LCPG

### 3.1 Arquitectura del Builder

```
┌─────────────────────────────────────────────────────────────────┐
│                      LCPG Builder                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  Pass 1: RECOLECTAR DEFINICIONES                          │ │
│  │  - Visitar AST una vez                                    │ │
│  │  - Para cada nodo de definición, crear Symbol            │ │
│  │  - Guardar en symbols HashMap                             │ │
│  │  - Indexar por nombre y por archivo                        │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                            │                                     │
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  Pass 2: RECOLECTAR REFERENCIAS                           │ │
│  │  - Visitar AST nuevamente                                  │ │
│  │  - Para cada identificador, intentar resolver             │ │
│  │  - Crear RefEdge o CallEdge según contexto                │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                            │                                     │
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  Pass 3: RESOLVER REFERENCIAS                             │ │
│  │  - Para cada referencia pendiente                         │ │
│  │  - Buscar definición por nombre + scope                   │ │
│  │  - Actualizar references_in / references_out              │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Implementación del Visitor

```rust
/// Visitor para construir el LCPG
pub struct LcpqBuilder {
    symbols: HashMap<SymbolId, Symbol>,
    pending_calls: Vec<(SymbolId, String, Span)>,
    pending_refs: Vec<(SymbolId, String, Span)>,
    by_name: HashMap<String, Vec<SymbolId>>,
    file_id: FileId,
    next_id: usize,
}

impl LcpqBuilder {
    pub fn new(file_id: FileId) -> Self {
        Self {
            symbols: HashMap::new(),
            pending_calls: Vec::new(),
            pending_refs: Vec::new(),
            by_name: HashMap::new(),
            file_id,
            next_id: 0,
        }
    }

    /// Obtiene el siguiente ID único
    fn next_id(&mut self) -> SymbolId {
        let id = SymbolId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Registra una nueva definición de símbolo
    fn define(&mut self, name: String, kind: SymbolKind, span: Span) -> SymbolId {
        let id = self.next_id();
        let symbol = Symbol {
            id,
            name: name.clone(),
            kind,
            visibility: Visibility::Private, // default
            span,
            file: self.file_id,
            references_in: Vec::new(),
            references_out: Vec::new(),
            metadata: SymbolMetadata::None,
        };
        self.symbols.insert(id, symbol);
        self.by_name.entry(name).or_default().push(id);
        id
    }
}
```

### 3.3 Visitando el AST

```rust
impl<'a> Visitor for LcpqBuilder {
    fn on_function(&mut self, node: &Node) {
        let name = extract_function_name(node);
        let visibility = extract_visibility(node);
        let span = node.span();

        let id = self.define(name.clone(), SymbolKind::Function, span);
        let symbol = self.symbols.get_mut(&id).unwrap();
        symbol.visibility = visibility;

        // Metadata de función
        let params = self.visit_params(node);
        symbol.metadata = SymbolMetadata::Function(FunctionMetadata {
            params,
            return_type: extract_return_type(node),
            is_async: is_async_fn(node),
            is_method: is_method(node),
        });

        // Registrar calls dentro de la función
        self.visit_children(node);

        // Al terminar, resolver referencias pendientes
        self.resolve_pending_calls(id);
        self.resolve_pending_refs(id);
    }

    fn on_call(&mut self, node: &Node) {
        // Extraer nombre de la función llamada
        if let Some(func_name) = extract_callee_name(node) {
            let span = node.span();
            let caller_id = self.current_function_id();

            // No sabemos el target todavía, guardar para resolver después
            if let Some(caller) = caller_id {
                self.pending_calls.push((caller, func_name, span));
            }
        }
    }

    fn on_identifier(&mut self, node: &Node) {
        let name = node.text().to_string();
        let span = node.span();

        if let Some(parent) = node.parent() {
            // Solo registrar si es una referencia (no definición)
            if !is_definition_context(parent) {
                if let Some(ref_id) = self.current_scope_id() {
                    self.pending_refs.push((ref_id, name, span));
                }
            }
        }
    }
}
```

### 3.4 Resolución de Referencias

```rust
impl LcpqBuilder {
    /// Resuelve las llamadas pendientes usando scope tracking
    fn resolve_pending_calls(&mut self, caller: SymbolId) {
        let to_remove: Vec<usize> = Vec::new();

        for (i, (call_caller, callee_name, span)) in self.pending_calls.iter().enumerate() {
            if *call_caller != caller {
                continue;
            }

            // Buscar en scope actual y scopes superiores
            if let Some(callee_id) = self.lookup_in_scope(callee_name) {
                // Encontrado: crear edge de llamada
                self.symbols.get_mut(&caller).unwrap()
                    .references_out.push(callee_id);

                self.symbols.get_mut(&callee_id).unwrap()
                    .references_in.push(caller);

                self.calls.push(CallEdge {
                    caller,
                    callee: callee_id,
                    span: *span,
                });

                to_remove.push(i);
            }
        }

        // Limpiar los resueltos
        for i in to_remove.into_iter().rev() {
            self.pending_calls.remove(i);
        }
    }

    /// Busca un símbolo por nombre en el scope actual y superiores
    fn lookup_in_scope(&self, name: &str) -> Option<SymbolId> {
        // Primero buscar en el scope actual
        if let Some(ids) = self.by_name.get(name) {
            return ids.last().copied();
        }
        // TODO: Implementar búsqueda en scopes superiores
        None
    }
}
```

---

## 4. Reglas Habilitadas por LCPG

### 4.1 Función Pública Sin Llamadas Externas

```rust
/// Regla: `unused/public-function`
/// Detecta funciones públicas que nunca son llamadas desde fuera del módulo
pub struct UnusedPublicFunctionRule;

impl Rule for UnusedPublicFunctionRule {
    fn id(&self) -> &str { "unused/public-function" }
    fn severity(&self) -> Severity { Severity::Minor }
    fn category(&self) -> Category { Category::CodeSmell }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let symbol_table = ctx.symbol_table
            .expect("LCPG required for unused/public-function");

        for symbol in symbol_table.symbols.values() {
            if symbol.kind == SymbolKind::Function
                && symbol.visibility == Visibility::Public
                && symbol.references_in.is_empty()
            {
                issues.push(Issue {
                    rule_id: self.id(),
                    severity: self.severity(),
                    node: Some(symbol.span),
                    message: format!(
                        "Public function '{}' is never called from outside its module. \
                         Consider making it private or removing it if unused.",
                        symbol.name
                    ),
                    fix: None,
                });
            }
        }

        issues
    }

    fn layer(&self) -> u8 { 2 }
    fn required_keywords(&self) -> &[&str] {
        &["fn", "pub", "pub(crate)", "pub(super)"]
    }
}
```

### 4.2 Variable Asignada Pero No Leída

```rust
/// Regla: `dead/assignment-no-read`
/// Detecta variables que se asignan pero nunca se leen
pub struct AssignmentNoReadRule;

impl Rule for AssignmentNoReadRule {
    fn id(&self) -> &str { "dead/assignment-no-read" }
    fn severity(&self) -> Severity { Severity::Minor }
    fn category(&self) -> Category { Category::CodeSmell }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let symbol_table = ctx.symbol_table
            .expect("LCPG required for dead/assignment-no-read");

        for symbol in symbol_table.symbols.values() {
            if symbol.kind == SymbolKind::Variable
                && !symbol.references_out.is_empty()  // Asignada
                && symbol.references_in.is_empty()    // Pero nunca leída
            {
                issues.push(Issue {
                    rule_id: self.id(),
                    severity: self.severity(),
                    node: Some(symbol.span),
                    message: format!(
                        "Variable '{}' is assigned but its value is never used.",
                        symbol.name
                    ),
                    fix: None,
                });
            }
        }

        issues
    }

    fn layer(&self) -> u8 { 2 }
}
```

### 4.3 Import No Utilizado

```rust
/// Regla: `unused/import`
/// Detecta imports que no son utilizados
pub struct UnusedImportRule;

impl Rule for UnusedImportRule {
    fn id(&self) -> &str { "unused/import" }
    fn severity(&self) -> Severity { Severity::Minor }
    fn category(&self) -> Category { Category::CodeSmell }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let symbol_table = ctx.symbol_table
            .expect("LCPG required for unused/import");

        for symbol in symbol_table.symbols.values() {
            if symbol.kind == SymbolKind::Import
                && symbol.references_out.is_empty()
            {
                let msg = if let SymbolMetadata::Import(import_meta) = &symbol.metadata {
                    format!("Import '{}' is unused", import_meta.path)
                } else {
                    format!("Import '{}' is unused", symbol.name)
                };

                issues.push(Issue {
                    rule_id: self.id(),
                    severity: self.severity(),
                    node: Some(symbol.span),
                    message: msg,
                    fix: None,
                });
            }
        }

        issues
    }

    fn layer(&self) -> u8 { 2 }
}
```

### 4.4 Código Muerto (Island Code)

```rust
/// Regla: `dead/code-island`
/// Detecta funciones que no pueden ser llamadas desde ningún punto de entrada
pub struct DeadCodeIslandRule;

impl Rule for DeadCodeIslandRule {
    fn id(&self) -> &str { "dead/code-island" }
    fn severity(&self) -> Severity { Severity::Info }
    fn category(&self) -> Category { Category::CodeSmell }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let symbol_table = ctx.symbol_table
            .expect("LCPG required for dead/code-island");

        // Build reverse call graph
        let mut callers: HashMap<SymbolId, Vec<SymbolId>> = HashMap::new();
        for call in &symbol_table.calls {
            callers.entry(call.callee).or_default().push(call.caller);
        }

        // DFS desde funciones exportadas para encontrar reachable
        let mut reachable: HashSet<SymbolId> = HashSet::new();
        let entry_points: Vec<SymbolId> = symbol_table.symbols.values()
            .filter(|s| s.kind == SymbolKind::Function && s.visibility == Visibility::Public)
            .map(|s| s.id)
            .collect();

        for entry in entry_points {
            self.mark_reachable(entry, &callers, &mut reachable);
        }

        // Encontrar no-reachable functions
        let mut issues = Vec::new();
        for symbol in symbol_table.symbols.values() {
            if symbol.kind == SymbolKind::Function
                && !reachable.contains(&symbol.id)
                && !symbol.name.starts_with("_")  // allow #[allow(unused)]
            {
                issues.push(Issue {
                    rule_id: self.id(),
                    severity: self.severity(),
                    node: Some(symbol.span),
                    message: format!(
                        "Function '{}' appears to be dead code. \
                         It cannot be reached from any public entry point.",
                        symbol.name
                    ),
                    fix: None,
                });
            }
        }

        issues
    }

    fn layer(&self) -> u8 { 2 }

    fn mark_reachable(
        &self,
        id: SymbolId,
        callers: &HashMap<SymbolId, Vec<SymbolId>>,
        reachable: &mut HashSet<SymbolId>,
    ) {
        if reachable.contains(&id) {
            return;
        }
        reachable.insert(id);

        if let Some(callers) = callers.get(&id) {
            for caller in callers {
                self.mark_reachable(*caller, callers, reachable);
            }
        }
    }
}
```

---

## 5. Integración con RuleContext

### 5.1 Acceso al SymbolTable

```rust
impl RuleContext<'_> {
    /// Retorna el SymbolTable construido para el archivo actual
    pub fn symbol_table(&self) -> Option<&SymbolTable> {
        self.symbol_table
    }

    /// Busca un símbolo por nombre en el archivo actual
    pub fn find_symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbol_table.and_then(|st| {
            st.by_name.get(name)
                .and_then(|ids| ids.last())
                .and_then(|id| st.symbols.get(id))
        })
    }

    /// Retorna todas las referencias a un símbolo
    pub fn get_references(&self, symbol_id: SymbolId) -> Vec<Span> {
        self.symbol_table
            .and_then(|st| st.symbols.get(&symbol_id))
            .map(|s| s.references_in.iter().filter_map(|ref_id| {
                st.symbols.get(ref_id).map(|r| r.span)
            }).collect())
            .unwrap_or_default()
    }
}
```

### 5.2 Construcción Condicional

El LCPG se construye **solo cuando es necesario** (Layer 2+ rules):

```rust
pub struct Analyzer {
    rules: Vec<Box<dyn Rule>>,
    need_lcpg: bool,  // true si alguna regla requiere LCPG
}

impl Analyzer {
    pub fn analyze(&self, source: &str, path: &Path) -> Vec<Issue> {
        let layer0_issues = self.run_preflight(source);
        if layer0_issues.is_empty() && self.rules.iter().all(|r| r.layer() == 0) {
            return layer0_issues;
        }

        let tree = parse(source);
        let layer1_issues = self.run_structural(&tree);

        // Solo construir LCPG si alguna regla lo necesita
        let symbol_table = if self.need_lcpg {
            Some(LcpqBuilder::new(FileId(0)).build(&tree))
        } else {
            None
        };

        let ctx = RuleContext {
            source,
            ast: &tree,
            symbol_table: symbol_table.as_ref(),
            language: self.detect_language(path),
            file_path: path,
        };

        let layer2_issues = self.run_semantic(&ctx);
        let layer3_issues = self.run_flow(&ctx);

        // ... combinar issues
    }
}
```

---

## 6. Métricas y Estadísticas

### 6.1 Construcción de Métricas

```rust
pub struct LcpqStats {
    /// Tiempo de construcción del LCPG
    pub build_time_ms: u64,
    /// Número de símbolos encontrados
    pub symbol_count: usize,
    /// Número de edges de llamada
    pub call_edge_count: usize,
    /// Número de edges de referencia
    pub ref_edge_count: usize,
    /// Memoria estimada en bytes
    pub memory_bytes: usize,
}

impl SymbolTable {
    pub fn stats(&self) -> LcpqStats {
        LcpqStats {
            build_time_ms: 0, // medido externamente
            symbol_count: self.symbols.len(),
            call_edge_count: self.calls.len(),
            ref_edge_count: self.references.len(),
            memory_bytes: self.estimate_memory(),
        }
    }

    fn estimate_memory(&self) -> usize {
        // HashMap overhead + symbols + edges
        std::mem::size_of::<Self>()
            + self.symbols.capacity() * std::mem::size_of::<Symbol>()
            + self.calls.capacity() * std::mem::size_of::<CallEdge>()
            + self.references.capacity() * std::mem::size_of::<RefEdge>()
    }
}
```

### 6.2 Benchmark Típico

| Métrica | Valor Típico |
|---------|--------------|
| Símbolos por archivo | 50-500 |
| Tiempo de construcción | < 1ms por archivo |
| Memoria por archivo | < 100KB |
| Edge count | ~2x symbol count |

---

## 7. Limitaciones y Futuras Mejoras

### 7.1 Limitaciones Conocidas

1. **No resuelve shadowing correctamente**: Variables con el mismo nombre en scopes anidados
2. **No soporta closures como First-Class**: Las referencias desde closures pueden no trackearse
3. **No hace inference de tipos**: No puede saber si `x.foo()` es un método o función
4. **Alcance por archivo**: No puede detectar dead code cross-file

### 7.2 Mejoras Planeadas

1. **Scope tracking mejorado**: Árbol de scopes en lugar de lista plana
2. **Cross-file references**: FileId + SymbolId para referencias inter-archivo
3. **Type inference ligera**: Para distinguir métodos de funciones
4. **Pattern matching en LCPG**: Queries como "todos los setters que no son usados"
