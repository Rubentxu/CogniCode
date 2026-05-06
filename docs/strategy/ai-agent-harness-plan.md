# CogniCode Strategic Vision — Action Plan

## Estado Actual vs Propuestas

| Propuesta | ¿Existe? | Estado | Acción |
|-----------|---------|--------|--------|
| 1. Indexación Estructural-Semántica | ✅ Parcial | `semantic_search.rs` tiene BM25 + filtrado por SymbolKind | Añadir tagging Tree-sitter a tokens BM25 |
| 2. Detector de Deriva de Intención | ❌ | No existe | Nueva capability: docstring vs cuerpo |
| 3. Compilador de Contexto | ✅ Parcial | `sandbox_core/` tiene ground_truth y scoring | Expandir para "contratos de ejecución" |
| 4. MCP Server Alta Velocidad | ✅ | `cognicode-mcp` + `interface/mcp/` ya funcionan | Añadir incremental parsing |

## Infraestructura Existente Aprovechable

```
crates/cognicode-core/src/
├── infrastructure/semantic/
│   ├── semantic_search.rs    ← BM25 + fuzzy search + kind filtering
│   ├── symbol_code.rs        ← Extracción de código de símbolos
│   └── outline.rs            ← Outline jerárquico del archivo
├── infrastructure/parser/
│   ├── tree_sitter_parser.rs ← Parser incremental (ya soporta incremental parsing!)
│   └── ast_scanner.rs        ← Scanner sobre AST
├── interface/mcp/
│   ├── rmcp_adapter.rs       ← Adaptador MCP TCP
│   ├── handlers/             ← Handlers MCP (incluyendo semantic_search)
│   └── schemas.rs            ← Schemas MCP
└── sandbox_core/
    ├── ground_truth.rs       ← Verificación de corrección
    └── scoring.rs            ← Scoring de calidad
```

## Plan de Implementación por Prioridad

### 🔴 P1: Intent-Drift Detector (la propuesta más disruptiva)

**Qué hace:** Compara el docstring/comentario de una función con su implementación real usando BM25 + Tree-sitter. Si hay divergencia semántica, emite una alerta.

**Valor:** Ninguna herramienta actual (SonarQube, CodeQL, Semgrep) hace esto. Es un diferenciador absoluto.

**Implementación:**

```rust
// Nueva regla: S7000 — Semantic Intent Drift
struct IntentDriftRule;

impl SubscriptionRule for IntentDriftRule {
    fn subscribed_nodes(&self) -> Vec<&'static str> {
        vec!["function_item", "method_definition"]
    }
    
    fn visit_node(&self, node: Node, ctx: &RuleContext) -> Vec<Issue> {
        let doc = extract_docstring(node, ctx.source);  // Tree-sitter
        let body = extract_function_body(node, ctx.source); // Tree-sitter
        
        // BM25: compara el "vocabulario" del docstring vs el cuerpo
        let doc_terms = tokenize_bm25(&doc);
        let body_terms = tokenize_bm25(&body);
        
        let similarity = compute_bm25_similarity(&doc_terms, &body_terms);
        
        if similarity < 0.3 {
            // El docstring habla de algo que el código no hace
            vec![Issue::new("S7000", "Semantic drift: docstring describes different behavior than implementation")]
        } else {
            vec![]
        }
    }
}
```

**Esfuerzo:** 3h

### 🟠 P2: Indexación Estructural-Semántica (Queryable Context)

**Qué hace:** Enriquecer el índice BM25 con metadatos de Tree-sitter: `role:function_name`, `in_struct:User`, `visibility:pub`.

**Valor:** El agente puede hacer queries como `find: "auth" role:function in_struct: "User"` → resultados precisos, sin alucinaciones.

**Implementación:** Extender `semantic_search.rs` para añadir campos de metadatos al índice.

**Esfuerzo:** 2h

### 🟡 P3: MCP Server de Alta Velocidad con Parseo Incremental

**Qué hace:** Cuando el agente modifica un archivo, Tree-sitter incremental re-parsea solo los nodos cambiados. El MCP server notifica al agente en milisegundos.

**Valor:** El agente "ve" los cambios al instante. Experiencia tipo LSP pero vía MCP.

**Ya disponible:** `tree_sitter_parser.rs` tiene `parse_incremental()`. Solo falta exponerlo vía MCP.

**Esfuerzo:** 2h

### 🟢 P4: Contratos de Ejecución (Verifiable RAG)

**Qué hace:** Para funciones Rust, intenta compilar el fragmento de forma aislada y extrae la firma de tipos garantizada.

**Valor:** El agente recibe "este código compila y acepta `(String, u32) -> Result<User>`".

**Implementación:** Usar `sandbox_core` + `rustc` para compilar fragmentos.

**Esfuerzo:** 4h (más complejo, depende de rustc)

## Timeline

| Semana | Entregable | Valor |
|--------|-----------|-------|
| 1 | Intent-Drift Detector (S7000) | 🔴 Diferenciador |
| 1 | Structural-Semantic Indexing | 🟠 Precisión |
| 2 | MCP Incremental Parsing | 🟡 Velocidad |
| 2 | MCP tools para cognitive_load, architecture_health | 🟡 Utilidad |
| 3 | Verifiable RAG Contracts | 🟢 Confianza |
| 3 | Dashboard: intent-drift visualization | 🟢 UX |
