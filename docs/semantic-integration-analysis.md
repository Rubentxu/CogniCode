# Análisis de Integración: Capacidades Semánticas de codesearch → CogniCode

**Fecha**: 27 de abril de 2026  
**Objetivo**: Evaluar si vale la pena integrar búsqueda semántica con embeddings + BM25 + RRF en CogniCode

---

## 📋 Resumen Ejecutivo

**¿Vale la pena?** ⚠️ **DEPENDENCIA COMPLEJA - No es un sí/no simple**

**Recomendación inicial**: **NO integrar directamente** en CogniCode v0.4.0, pero considerar:
1. **Usar ambas herramientas complementariamente** (recomendación principal)
2. **Integración modular futura** en v1.0+ si la demanda justifica el coste
3. **Explorar alternativas técnicas** (extensión MCP, wrappers, etc.)

---

## 🎯 Donde codesearch GANA (Capacidades a Integrar)

### 1. Búsqueda Semántica Natural

| Aspecto | codesearch | CogniCode actual | Gap |
|-----------|-------------|-------------------|-----|
| **Tecnología** | Vector embeddings + BM25 + RRF | Fuzzy string matching | ⚠️ Significativo |
| **Entiende significado** | ✅ Embeddings capturan contexto | ❌ Solo coincidencias de strings | ⚠️ Critico |
| **Precisión** | Alta (0.85-0.96 scores) | Media (fuzzy matching) | ⚠️ Alto |
| **Recall** | Alta (encuentra código relacionado) | Baja (solo nombres similares) | ⚠️ Alto |

### 2. Eficiencia de Tokens

| Aspecto | codesearch | CogniCode actual | Gap |
|-----------|-------------|-------------------|-----|
| **Compact mode** | ✅ 90%+ reducción | ✅ 50% reducción (context compression) | ⚠️ Medio |
| **Metadata-only** | ✅ path, line, kind, signature, score | ✅ resúmenes en lenguaje natural | ⚠️ Medio |
| **Chunk retrieval** | ✅ `get_chunk` para código completo | ✅ `get_symbol_code` | ⚠️ Similar |

### 3. Multi-repo Search

| Aspecto | codesearch | CogniCode actual | Gap |
|-----------|-------------|-------------------|-----|
| **Cross-repo** | ✅ Groups + serve mode | ❌ Un workspace por instancia | ⚠️ Significativo |
| **Unified ranking** | ✅ RRF fusiona de todos los repos | ❌ No disponible | ⚠️ Significativo |
| **Path prefixes** | ✅ Alias prefijados (service-a/, service-b/) | ❌ No disponible | ⚠️ Significativo |

---

## 📦 Requisitos Técnicos de Integración

### Arquitectura Actual de CogniCode

```
┌──────────────────────────────────────────────────────┐
│              COGNICODE (Actual)               │
│                                               │
│  ┌────────────────┐  ┌────────────────┐        │
│  │   DOMAIN       │  │  APPLICATION   │        │
│  └───────┬────────┘  └───────┬────────┘        │
│          │                    │                   │
│          └────────────────────┼───────────────────┘        │
│                               │                          │
│                    ┌──────────┴──────────┐               │
│                    │     INTERFACE       │               │
│                    │   (MCP, LSP, CLI)  │               │
│                    └────────────────────┘               │
│                                               │
│  Storage: RedbGraphStore (redb)                  │
│  Graphs: petgraph (call graphs)                 │
│  Parsing: tree-sitter (AST)                    │
└───────────────────────────────────────────────────────┘
```

### Arquitectura Propuesta con Integración

```
┌──────────────────────────────────────────────────────┐
│            COGNICODE + SEMÁNTICA (Propuesta)    │
│                                               │
│  ┌────────────────┐  ┌────────────────┐        │
│  │   DOMAIN       │  │  APPLICATION   │        │
│  │  + EMBEDDINGS │  │  + SEARCH      │        │  │
│  └───────┬────────┘  └───────┬────────┘        │
│          │                    │                   │
│          └────────────────────┼───────────────────┘        │
│                               │                          │
│                    ┌──────────┴──────────┐               │
│                    │     INTERFACE       │               │
│                    │   (MCP, LSP, CLI)  │               │
│                    └────────────────────┘               │
│                                               │
│  Storage: RedbGraphStore + EmbeddingStore         │
│  Search Engine: BM25 + Vector + RRF             │
│  Models: MiniLM, BGE, Jina, Omi              │
└───────────────────────────────────────────────────────┘
```

---

## 🏗️ Cambios Arquitectónicos Requeridos

### 1. Añadir Capa de Búsqueda Semántica

```rust
// Nuevo bounded context: SEARCH (o extender DOMAIN)
pub mod search {
    use domain::traits::*;
    use infrastructure::embeddings::*;
    use infrastructure::bm25::*;
    use infrastructure::rrf::*;

    pub struct SemanticSearchEngine {
        embeddings: Arc<dyn EmbeddingProvider>,
        bm25: Arc<dyn FullTextSearch>,
        vector_index: Arc<dyn VectorIndex>,
        rrf_fusioner: Arc<RRFFusioner>,
    }

    impl SemanticSearchEngine {
        pub async fn search(
            &self,
            query: &str,
            mode: SearchMode,
            limit: usize,
        ) -> Result<Vec<SearchResult>, SearchError> {
            // 1. Generar embedding de query
            let query_embedding = self.embeddings.embed(query).await?;

            // 2. Búsqueda vectorial
            let vector_results = self.vector_index.search(&query_embedding, limit)?;

            // 3. Búsqueda BM25
            let bm25_results = self.bm25.search(query, limit)?;

            // 4. RRF Fusion
            let fused = self.rrf_fusioner.fuse(
                &[vector_results, bm25_results],
                limit
            )?;

            // 5. Neural reranking (opcional)
            let ranked = if self.rerank_enabled {
                self.neural_reranker.rerank(&fused).await?
            } else {
                fused
            };

            Ok(ranked)
        }
    }
}
```

### 2. Nuevos Crates/Dependencias

```toml
# Crates nuevos necesarios
[workspace.dependencies]
# Embeddings
candle-core = "0.4"              # ONNX runtime para embeddings
burn = "0.14"                  # Alternative a candle
# Opción: descargar modelos pre-entrenados

# Búsqueda
tantivy = "0.22"                # BM25 full-text search (alternativa a implementación propia)
# Opción: implementar BM25 desde cero con lenguas

# Vector similarity
faiss-rs = "0.12"               # FAISS para búsqueda vectorial rápida
# Opción: qdrant-client, o implementación propia con HNSW

# Serialización de embeddings
bincode = { version = "2.0", features = ["serde"] }

# RRF
statrs = "0.16"                # Para ranking stats
```

### 3. Cambios en Storage

```rust
// infrastructure/persistence/mod.rs
pub mod redb_graph_store;
pub mod embedding_store;      // NUEVO
pub mod bm25_index;            // NUEVO
pub mod vector_index;           // NUEVO

// Nuevo: EmbeddingStore
use redb::{Database, Table, TableDefinition};
use candle_core::Tensor;

const EMBEDDINGS_TABLE: TableDefinition<&[u8], &[u8], &[u8]> =
    TableDefinition::new("embeddings");

pub struct EmbeddingStore {
    db: Database,
}

impl EmbeddingStore {
    pub fn store_embedding(
        &self,
        chunk_id: u64,
        embedding: Vec<f32>,  // 384 o 768 dims
    ) -> Result<(), StorageError> {
        // Almacenar embedding en redb
    }

    pub fn get_embedding(
        &self,
        chunk_id: u64,
    ) -> Result<Option<Vec<f32>>, StorageError> {
        // Recuperar embedding
    }

    pub fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SimilarChunk>>, StorageError> {
        // Búsqueda vectorial con HNSW o brute force
    }
}
```

### 4. Modelos de Embeddings

```rust
// infrastructure/embeddings/mod.rs
pub mod models;
pub mod providers;

#[derive(Debug, Clone)]
pub enum EmbeddingModel {
    MiniLmL6,      // 384 dims, fastest, default
    BGESmall,       // 384 dims, good quality
    BGEBase,        // 768 dims, higher quality
    JinaCode,       // 768 dims, code-optimized
    OmiV1_5,        // 768 dims, long context
}

impl EmbeddingModel {
    pub fn dims(&self) -> usize {
        match self {
            Self::MiniLmL6 | Self::BGESmall => 384,
            Self::BGEBase | Self::JinaCode | Self::OmiV1_5 => 768,
        }
    }

    pub fn model_file(&self) -> &'static str {
        match self {
            Self::MiniLmL6 => "models/minilm-l6-q.onnx",
            Self::BGESmall => "models/bge-small-en-q.onnx",
            // ...
        }
    }
}

pub struct EmbeddingProvider {
    model: EmbeddingModel,
    session: candle_core::Session,  // O burn::Session
}

impl EmbeddingProvider {
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        // 1. Tokenizar (usando tokenizer del modelo)
        let tokens = self.tokenize(text)?;

        // 2. Pasar por modelo ONNX
        let output = self.session.run(
            &self.model,
            &tokens,
        )?;

        // 3. Extraer embedding (mean pooling o [CLS] token)
        let embedding = self.extract_embedding(&output)?;

        Ok(embedding)
    }
}
```

### 5. BM25 Implementation

```rust
// infrastructure/search/bm25.rs
use std::collections::HashMap;

pub struct BM25Index {
    doc_freqs: HashMap<String, u32>,
    doc_lengths: HashMap<u64, usize>,
    avg_doc_length: f64,
    total_docs: u32,
}

impl BM25Index {
    pub fn add_document(&mut self, doc_id: u64, text: &str) {
        // Tokenizar texto
        let tokens = self.tokenize(text);

        // Contar frecuencias en documento
        for token in tokens {
            *self.doc_freqs.entry(token.clone()).or_insert(0) += 1;
        }

        // Guardar longitud del documento
        self.doc_lengths.insert(doc_id, tokens.len());
        self.total_docs += 1;
    }

    pub fn build(&mut self) {
        // Calcular avg_doc_length
        let total: usize = self.doc_lengths.values().sum();
        self.avg_doc_length = total as f64 / self.total_docs as f64;
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<BM25Result> {
        let mut scores: HashMap<u64, f64> = HashMap::new();

        for term in self.tokenize(query) {
            if let Some(&df) = self.doc_freqs.get(&term) {
                let idf = ((self.total_docs as f64 - df as f64 + 0.5)
                    / (df as f64 + 0.5)).ln_1p();

                for (&doc_id, doc_len) in &self.doc_lengths {
                    let tf = 1.0; // TF simplificado (usar freq real sería mejor)
                    let doc_len_norm = 1.0 - 0.1 * (*doc_len as f64 / self.avg_doc_length);
                    let score = tf * idf * doc_len_norm;

                    *scores.entry(*doc_id).or_insert(0.0) += score;
                }
            }
        }

        // Ordenar y top-K
        let mut results: Vec<_> = scores.into_iter()
            .map(|(doc_id, score)| BM25Result { doc_id, score })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.truncate(limit);

        results
    }
}
```

### 6. RRF Fusion

```rust
// infrastructure/search/rrf.rs
use statrs::statistics::{mean, Data};

pub struct RRFFusioner;

impl RRFFusioner {
    pub fn fuse(
        &self,
        ranked_lists: &[Vec<SearchResult>],
        k: usize,  // Default 60, pero usar 20 es suficiente
    ) -> Vec<SearchResult> {
        let mut rrf_scores: HashMap<u64, f64> = HashMap::new();

        for (rank, list) in ranked_lists.iter().enumerate() {
            for (position, result) in list.iter().enumerate() {
                let doc_id = result.chunk_id;
                let rrf_score = 1.0 / (k + position + 1) as f64;

                *rrf_scores.entry(doc_id).or_insert(0.0) += rrf_score as f64;
            }
        }

        // Normalizar scores
        let scores: Vec<f64> = rrf_scores.values().cloned().collect();
        let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, |a, b| a.max(b));

        let mut fused: Vec<SearchResult> = rrf_scores.into_iter()
            .map(|(chunk_id, score)| SearchResult {
                chunk_id,
                score: score / max_score,
                sources: vec!["vector", "bm25"],
            })
            .collect();

        fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        fused
    }
}
```

### 7. Neural Reranking (Opcional)

```rust
// infrastructure/search/reranker.rs
use candle_core::{Tensor, Module};

pub struct NeuralReranker {
    model: Module,
    // Modelo pequeño para reranking (cross-encoder)
}

impl NeuralReranker {
    pub async fn rerank(
        &self,
        query: &str,
        candidates: &[SearchResult],
    ) -> Result<Vec<SearchResult>, RerankError> {
        // 1. Embed query
        let query_emb = self.embed(query).await?;

        // 2. Embed each candidate
        let mut query_expanded = Vec::new();
        let mut doc_embeddings = Vec::new();

        for candidate in candidates {
            let doc = self.get_chunk_text(candidate.chunk_id)?;
            query_expanded.push(query.clone());
            doc_embeddings.push(self.embed(&doc).await?);
        }

        // 3. Compute similarity scores
        let similarities = self.compute_similarities(&query_emb, &doc_embeddings)?;

        // 4. Merge with existing scores and re-sort
        let mut reranked = candidates.clone();
        for (i, sim) in similarities.iter().enumerate() {
            reranked[i].score = reranked[i].score * 0.7 + sim * 0.3; // Weighted fusion
        }

        reranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        Ok(reranked)
    }
}
```

---

## 💰 Coste de Integración

### 1. Dependencias Externas (Tamaño de Binario)

| Dependencia | Versión Estimada | Tamaño Extra | Crítico |
|-------------|-----------------|-------------|----------|
| candle-core | ~15 MB | 🟡 Mediano | ⚠️ Alto |
| o burn | ~20 MB | 🟡 Mediano | ⚠️ Alto |
| tantivy | ~5 MB | 🟢 Pequeño | ⚡ Bajo |
| faiss-rs | ~10 MB | 🟡 Mediano | ⚡ Medio |
| Modelos de embeddings | ~100-200 MB | 🔴 Muy Grande | ⚠️ Muy Alto |

**Tamaño actual de CogniCode:**
- Binary: ~8-12 MB
- Incluyendo dependencias: ~30 MB

**Tamaño estimado después de integración:**
- Binary: ~20-30 MB (+150-250%)
- Incluyendo dependencias: ~150-200 MB (+400-600%)

**⚠️ IMPACTO SIGNIFICATIVO EN DISTRIBUCIÓN**

### 2. Complejidad de Código

| Componente | Líneas Estimadas | Complejidad | Tiempo Implementación |
|-------------|------------------|--------------|---------------------|
| Embedding provider | 800-1,200 | 🟡 Alta | 2-3 semanas |
| BM25 index | 600-1,000 | 🟢 Media | 1-2 semanas |
| Vector index (HNSW) | 1,200-1,800 | 🔴 Muy Alta | 3-4 semanas |
| RRF fusion | 400-600 | 🟢 Media | 1 semana |
| Neural reranker | 800-1,200 | 🔴 Muy Alta | 2-3 semanas |
| Integration y testing | 500-800 | 🟡 Alta | 2-3 semanas |
| **TOTAL** | **4,300-6,600** | **🔴 Muy Alta** | **11-16 semanas (3-4 meses)** |

### 3. Coste Operacional

| Aspecto | Actual | Con Integración | Impacto |
|---------|--------|----------------|---------|
| **Memoria RAM** | ~100-200 MB | ~300-500 MB | 🔴 +150-300% |
| **CPU indexado** | 1-5 s | 10-30 s | 🔴 +500-600% |
| **Disk space** | ~50 MB | ~150-250 MB | 🟡 +200-400% |
| **Cold start** | ~1 s | ~3-5 s | 🟡 +300-500% |
| **Mantenimiento modelos** | 0 | Requiere descarga y actualización | 🟢 Nuevo overhead |

---

## ✅ Pros de Integración

### 1. Ventajas Funcionales

✅ **Búsqueda semántica nativa en CogniCode**
- Agentes pueden hacer queries naturales sin herramienta adicional
- Experiencia unificada en el servidor MCP
- No requiere configuración de dos servidores separados

✅ **Mejor comprensión de código por parte de agentes**
- Encuentra código relacionado por significado
- Reduce tokens de prompts de agentes (90%+ compact mode)
- Mejora calidad de respuestas de agentes

✅ **Cross-repo search unificado**
- Si se extiende CogniCode con multi-repo, se beneficia de RRF unificado
- Búsqueda en múltiples proyectos desde una sola herramienta MCP
- Ranking inteligente combinando resultados de todos los repos

✅ **Mejor token efficiency**
- Compact mode de codesearch (90%+ reducción)
- Agentes pueden usar CogniCode para búsquedas semánticas eficientes
- Reduce coste de uso de APIs de LLMs

### 2. Ventajas Técnicas

✅ **Arquitectura consolidada**
- Un solo servidor MCP con todas las capacidades
- Menor superficie de ataque (un solo proceso vs dos)
- Simplifica deployment y configuración

✅ **Shared infrastructure**
- Reutiliza storage (Redb) para embeddings
- Reutiliza tree-sitter parsing existente
- Reutiliza MCP handlers existentes

✅ **Mejor user experience**
- Agentes no necesitan decidir qué herramienta usar
- Configuración simplificada en Claude/OpenCode
- Menor fricción en flujo de trabajo

---

## ❌ Contras de Integración

### 1. Riesgos Técnicos

❌ **Aumento dramático de tamaño de binario**
- 150-250% más grande (8-12 MB → 20-30 MB)
- Tiempos de descarga más largos
- Mayor uso de disco en deployment

❌ **Complejidad arquitectónica significativa**
- 4,300-6,600 líneas de código nuevas
- 3-4 meses de desarrollo mínimo
- Aumenta deuda técnica significativamente
- Más bugs potenciales, más testing requerido

❌ **Dependencias externas pesadas**
- candle/burn/ONNX runtime (15-20 MB)
- tantivy/faiss (5-10 MB)
- Modelos de embeddings (100-200 MB)
- Posibles vulnerabilidades en nuevas dependencias
- Updates más frecuentes y complejos

❌ **Coste operacional alto**
- RAM: +150-300% más memoria (300-500 MB vs 100-200 MB)
- CPU: Indexado 5-6x más lento (10-30 s vs 1-5 s)
- Storage: +200-400% más espacio (150-250 MB vs 50 MB)
- Cold start: +300-500% más lento (3-5 s vs 1 s)

❌ **Mantenimiento de modelos**
- Requiere descargar modelos ONNX (100-200 MB)
- Versionado complejo de modelos
- Actualizaciones de modelos no trivial
- Posible drift de calidad entre modelos

### 2. Riesgos de Diseño

❌ **Foco diluido**
- CogniCode actualmente está focalizado en "inteligencia de código"
- Añadir búsqueda semántica lo convierte en "hágalo todo"
- Riesgo de ser "jack of all trades, master of none"
- Más difícil mantener y mejorar cada área

❌ **Complexity Creep**
- Cada nueva capa añade complejidad
- Testing más complejo (unit tests + integration tests + e2e tests)
- Debugging más difícil (embeddings + graphs + refactor)

❌ **Team overwhelm**
- Equipo de desarrollo pequeño vs 3-4 meses de trabajo
- Requiere expertise en ML/embeddings (posible skill gap)
- Distracción de mejoras en capacidades core (refactorización, análisis)

### 3. Riesgos de Producto

❌ **Overengineering para casos de uso reales**
- ¿Realmente los agentes necesitan búsqueda semántica en CogniCode?
- codesearch ya funciona perfectamente bien en standalone
- ¿Valdrá la pena duplicar funcionalidad?

❌ **Adopción limitada**
- Usuarios pueden preferir usar codesearch standalone
- Integración forzada puede ser rechazada
- Complejidad adicional sin valor claro percibido

❌ **Soporte multi-modelo**
- ¿Soportar TODOS los modelos de codesearch? (MiniLM, BGE, Jina, Omi)
- O solo uno? (limita utilidad)
- Decisiones de producto complejas

---

## 🔄 Alternativas a Integración Directa

### Alternativa 1: Usar Ambas Complementariamente (RECOMENDADA)

**Cómo funciona:**
```json
// Configuración MCP actual
{
  "mcpServers": {
    "codesearch": {
      "command": "codesearch",
      "args": ["mcp"]
    },
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--cwd", "/path/to/project"]
    }
  }
}
```

**Pros:**
- ✅ **CERO código adicional** en CogniCode
- ✅ **CERO dependencias nuevas** (tamaño binario sin cambios)
- ✅ **CERO tiempo de desarrollo**
- ✅ Mejor de ambos mundos
- ✅ Cada herramienta hace lo que hace mejor
- ✅ Desarrollo paralelo (codesearch team y CogniCode team)
- ✅ Updates independientes
- ✅ Testing independiente
- ✅ Users pueden elegir cuál usar

**Contras:**
- ⚡ Dos procesos separados (más memoria total)
- ⚡ Configuración un poco más compleja
- ⚡ Agentes deben elegir herramienta apropiada
- ⚡ Cross-repo solo disponible en codesearch

**Coste:** $0 (solo configuración)
**Tiempo:** 0 semanas
**Riesgo:** 🟢 Bajo

---

### Alternativa 2: Wrapper MCP en CogniCode

**Cómo funciona:**
```rust
// CogniCode MCP wrapper que delega a codesearch
interface/mcp/codesearch_wrapper.rs

pub struct CodeSearchWrapper {
    codesearch_path: PathBuf,
    child_process: Option<tokio::process::Child>,
}

impl CodeSearchWrapper {
    pub async fn search_semantic(
        &self,
        query: &str,
    ) -> Result<Vec<SearchResult>, WrapperError> {
        // Lanzar codesearch mcp --mode local como child process
        let mut child = Command::new(&self.codesearch_path)
            .args(["mcp", "--mode", "local"])
            .stdout(Stdio::piped())
            .spawn()?;

        // Enviar MCP request vía stdin
        let request = json!({
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {
                    "query": query,
                    "mode": "semantic"
                }
            }
        });

        child.stdin.as_mut().unwrap()
            .write_all(request.as_bytes())?;

        // Leer respuesta via stdout
        let response = child.wait_with_output().await?;

        // Parsear y devolver resultado
        Ok(serde_json::from_slice(&response.stdout)?)
    }
}
```

**Pros:**
- ✅ Exposición unificada desde CogniCode
- ✅ Cero dependencias de embeddings en CogniCode
- ✅ Mantenimiento delegado a codesearch team
- ✅ Tamaño binario sin cambios (+ wrapper code ~500 LOC)
- ✅ 1-2 semanas de desarrollo

**Contras:**
- ⚡ Overhead de child process
- ⚡ Latencia adicional (~100-200ms)
- ⚡ Debugging más complejo (cross-process)
- ⚡ Versión de codesearch debe estar sincronizada
- ⚡ Debe instalar codesearch adicionalmente

**Coste:** Desarrollo (1-2 semanas)
**Tiempo:** 1-2 semanas
**Riesgo:** 🟡 Medio

---

### Alternativa 3: Extensión Plugin en CogniCode

**Cómo funciona:**
```rust
// Sistema de plugins para capacidades opcionales
domain/traits/search_provider.rs

#[async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str, limit: usize)
        -> Result<Vec<SearchResult>, SearchError>;
}

// Implementación nativa (actual)
pub struct CogniCodeSearch { ... }

// Implementación wrapper (nueva)
pub struct CodeSearchAdapter {
    codesearch: Arc<dyn SearchProvider>,
}

impl CodeSearchAdapter {
    pub async fn search(&self, query: &str, limit: usize)
        -> Result<Vec<SearchResult>, SearchError> {
        // Delegar a codesearch si disponible
        self.codesearch.search(query, limit).await
    }
}
```

**Configuración:**
```toml
# ~/.config/cognicode/config.toml
[search]
provider = "native"  # o "codesearch", o "both"
codesearch_path = "/usr/local/bin/codesearch"
codesearch_mode = "local"  # o "client"
```

**Pros:**
- ✅ Modular, no acoplado al core
- ✅ Opcional (users pueden no usarlo)
- ✅ Desarrollo incremental
- ✅ Mantenimiento separado
- ✅ 2-3 semanas de desarrollo
- ✅ Tamaño binario sin cambios si no instalado

**Contras:**
- ⚡ Complejidad de configuración
- ⚡ Requires instalación de codesearch
- ⚡ Version coupling risk
- ⚡ Testing cross-platform complejo

**Coste:** Desarrollo (2-3 semanas)
**Tiempo:** 2-3 semanas
**Riesgo:** 🟡 Medio

---

### Alternativa 4: Integración Futura en v1.0+

**Fase 1: v0.5.x (sin embeddings)**
- Añadir BM25 al actual fuzzy search
- Añadir RRF fusion para vector + BM25 cuando esté disponible
- Preparar abstracción SearchProvider

**Fase 2: v0.6.x (opcional embeddings)**
- Añadir soporte de embeddings como feature flag
- Implementar EmbeddingProvider con un solo modelo (MiniLM-L6)
- Añadir vector index básico

**Fase 3: v1.0 (completo)**
- Soporte múltiple modelos
- Neural reranking
- RRF unificado

**Pros:**
- ✅ Incremental, reduce riesgo
- ✅ Testing gradual de cada fase
- ✅ Feedback de users real
- ✅ Puede abortar si no hay demanda

**Contras:**
- ⚡ 6-9 meses a v1.0
- ⚡ Requiere recursos sostenidos
- ⚡ Complejidad de versionado

**Coste:** Desarrollo escalonado (6-9 meses)
**Tiempo:** 6-9 meses
**Riesgo:** 🟡 Medio (más alto con más fases)

---

## 📊 Matriz de Decisión

| Alternativa | Coste Desarrollo | Coste Operacional | Riesgo | Beneficio | Recomendación |
|-------------|------------------|-------------------|----------|-----------|----------------|
| **1. Usar ambos complementariamente** | $0 | 🟡 Bajo | 🟢 Bajo | ⭐⭐⭐⭐⭐ | 🥇 RECOMENDADA |
| **2. Wrapper MCP** | 1-2 semanas | 🟡 Bajo | 🟡 Medio | ⭐⭐⭐ | 🥈 BUENA OPCIÓN |
| **3. Plugin/Extensión** | 2-3 semanas | 🟢 Medio | 🟡 Medio | ⭐⭐ | 🥈 ACEPTABLE |
| **4. Integración directa completa** | 3-4 meses | 🔴 Alto | 🔴 Alto | ⭐ | 🥉 NO RECOMENDADA |
| **5. Integración escalonada (v1.0)** | 6-9 meses | 🔴 Alto | 🟡 Medio | ⭐⭐ | 🥉 OPCIONAL FUTURA |

---

## 🎯 Recomendación Final

### FASE CORTO PLAZO (1-2 semanas): DOCUMENTAR INTEGRACIÓN

**Acción:** Crear guía oficial de uso combinado

```
1. Documentar workflow recomendado en docs/guides/codesearch-integration.md
2. Crear ejemplos de configuración en examples/mcp-config/
3. Actualizar README.md con sección "Integración con codesearch"
4. Crear cheatsheet para agents
```

**Resultado:** Users pueden usar ambas herramientas de forma efectiva sin cambios en código.

**Coste:** $0
**Riesgo:** 🟢 Bajo

---

### FASE MEDIO PLAZO (1-3 meses): WRAPPER MCP

**Si hay DEMANDA CLARA de users** por integración unificada:

```
1. Implementar wrapper CodeSearchWrapper en CogniCode (1-2 semanas)
2. Añadir configuración de codesearch_path y mode (1 semana)
3. Testing cross-platform (1 semana)
4. Documentación (1 semana)
5. Release como v0.5.0 con feature flag "codesearch-integration"
```

**Resultado:** Exposición unificada desde CogniCode con overhead mínimo.

**Coste:** 1-3 semanas desarrollo
**Riesgo:** 🟡 Medio
**Recomendación:** Solo si hay demanda clara de users.

---

### FASE LARGO PLAZO (6-9 meses): NO PERSEGUIR

**Justificación:**
- Coste/beneficio desfavorable (3-4 meses vs usar complementariamente)
- Complejidad dramática de código
- Aumento 400-600% de tamaño de binario
- Coste operacional alto (RAM +150-300%)
- Riesgo de overengineering
- codesearch ya funciona bien standalone

**Alternativa:** Invertir esos recursos en capacidades CORE donde CogniCode ya es fuerte:
- Mejorar análisis de impacto
- Añadir más tipos de refactorización
- Mejorar métricas de calidad
- Añadir análisis de dead code más robusto
- Mejorar generación de documentación
- Añadir soporte LSP real (no solo wrappers)

---

## 📋 Checklist de Decisión

### ¿Debes integrar directamente? RESPUESTA: NO

❌ **NO integrar directamente** porque:
- 3-4 meses de desarrollo vs $0 para usar ambos
- 400-600% aumento de tamaño binario
- RAM +150-300% más memoria requerida
- Complejidad arquitectónica dramática
- Riesgo de overengineering
- codesearch ya funciona bien standalone

### ¿Cuándo reconsiderar integración?

✅ **Reconsiderar si:**
- [ ] Hay DEMANDA CLARA de users por integración unificada
- [ ] User interviews indican fricción significativa usando dos herramientas
- [ ] Métricas de uso muestran codesearch NO se usa con CogniCode
- [ ] Competidores integran búsqueda semántica (pressure de mercado)
- [ ] Hay equipo y recursos dedicados disponibles (3-4+ devs)
- [ ] Técnicamente viable integrar con overhead aceptable

Si 3+ de estas condiciones son verdaderas, **entonces considerar Wrapper MCP (no integración completa)**.

### ¿Qué hacer AHORA?

✅ **ACCIONES INMEDIATAS:**
1. [ ] Crear guía de integración con codesearch en docs/
2. [ ] Actualizar README con sección "Usando con codesearch"
3. [ ] Crear ejemplos de configuración MCP con ambas herramientas
4. [ ] Documentar workflow recomendado para agentes

✅ **ACCIONES FUTURAS (solo si hay demanda):**
1. [ ] Monitorizar métricas de uso de codesearch vs CogniCode
2. [ ] Encuestar users sobre fricción de usar dos herramientas
3. [ ] Evaluar demanda de integración unificada
4. [ ] Si demanda clara, implementar Wrapper MCP (no integración completa)

---

## 🏁 Conclusión

### RESPUESTA FINAL: **NO INTEGRAR DIRECTAMENTE**

**Razones principales:**

1. **Coste/Beneficio desfavorable**
   - 3-4 meses desarrollo vs $0 configuración
   - Complejidad masiva vs beneficio marginal

2. **Riesgo de Overengineering**
   - Dilución de foco (hágalo todo vs especialización)
   - Complejidad arquitectónica explosiva

3. **Impacto Operacional**
   - 400-600% tamaño binario
   - RAM +150-300% más memoria
   - Indexado 5-6x más lento

4. **Alternativa Mejor Disponible**
   - Usar ambas complementariamente
   - Wrapper MCP si hay demanda
   - Coste $0-3 semanas vs 3-4 meses

### RUTA RECOMENDADA:

```
HOY → Documentar integración complementaria
      → Guía de configuración
      → Ejemplos de workflows

MES FUTURO → Si hay demanda clara:
      → Implementar Wrapper MCP (1-3 semanas)
      → Feature flag, opcional

NO HACER → Integración directa completa
             → 3-4 meses desarrollo
             → Complejidad masiva
             → Sobredimensionamiento
```

---

**Documento creado**: 27 de abril de 2026  
**Próxima revisión**: Cuando se tenga métricas reales de uso de codesearch + CogniCode
