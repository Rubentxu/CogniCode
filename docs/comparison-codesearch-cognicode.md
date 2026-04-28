# Análisis Comparativo: codesearch vs CogniCode

**Fecha**: 27 de abril de 2026  
**Versiones**: codesearch v0.1.207 vs CogniCode v0.4.0

---

## Resumen Ejecutivo

**codesearch** es un servidor MCP de búsqueda semántica local que utiliza embeddings vectoriales y ranking híbrido (BM25 + RRF) para permitir a agentes de IA buscar código sin enviar nada a APIs externas.

**CogniCode** es un servidor MCP de inteligencia de código "Super-LSP" que proporciona análisis profundo, grafos de llamadas, búsqueda semántica, y refactorización segura a agentes de IA.

**Similitudes principales:**
- Ambos son servidores MCP para Claude/OpenCode
- Ambos están escritos en Rust
- Ambos usan Tree-sitter para parsing
- Ambos tienen modo compacto para reducir consumo de tokens
- Ambos funcionan completamente offline

**Diferencias clave:**
- codesearch se enfoca en **búsqueda semántica** (vector embeddings)
- CogniCode se enfoca en **análisis de código y navegación** (grafos, refactorización)
- codesearch tiene 5 herramientas MCP vs 35+ de CogniCode
- codesearch usa SQLite + embeddings, CogniCode usa redb + grafos de petgraph

---

## Tabla Comparativa Detallada

### 📊 Información General

| Aspecto | codesearch | CogniCode |
|-----------|-------------|-------------|
| **Propósito principal** | Búsqueda semántica de código | Inteligencia de código (grafos, análisis, refactorización) |
| **Versión** | v0.1.207 (19 abr 2026) | v0.4.0 |
| **Lenguaje** | Rust | Rust |
| **Fork de** | [demongrep](https://github.com/yxanul/demongrep) | Original |
| **Estrellas GitHub** | 16 | No disponible |
| **Licencia** | Apache 2.0 | Apache 2.0 |
| **Tipo de datos** | Vector embeddings + BM25 + RRF | Grafos de llamadas + AST + métricas |

---

### 🛠️ Herramientas MCP

| Categoría | codesearch | CogniCode |
|------------|-------------|-------------|
| **Total herramientas** | **5** consolidadas | **35+** distribuidas |
| **Búsqueda** | 2 (`search`, `explore`) | 3 (`semantic_search`, `structural_search`, `find_usages`) |
| **Navegación de símbolos** | 3 (`find`, `get_chunk`, `explore`) | 6 (`get_call_hierarchy`, `go_to_definition`, `hover`, `find_references`, `get_outline`, `get_symbol_code`) |
| **Análisis de código** | 0 | 10 (`analyze_impact`, `check_architecture`, `get_complexity`, `get_entry_points`, `get_leaf_functions`, `get_hot_paths`, `trace_path`, `find_dead_code`, `validate_syntax`, `get_module_dependencies`) |
| **Grafos** | 0 | 5 (`build_graph`, `build_lightweight_index`, `build_call_subgraph`, `get_per_file_graph`, `merge_graphs`) |
| **Archivos** | 0 | 5 (`read_file`, `write_file`, `edit_file`, `search_content`, `list_files`) |
| **Refactorización** | 0 | 1 (`safe_refactor`) |
| **Exportación** | 0 | 1 (`export_mermaid`) |
| **Estado/índices** | 1 (`status`) | 1 (`get_all_symbols`) |

#### Descripción de herramientas de codesearch:

1. **`search`**: Búsqueda unificada de código
   - Modos: `semantic` (vector + BM25 + RRF) o `literal` (FTS con regex/phrase)
   - Soporta compact mode (solo metadatos)
   - Soporta multi-repo via `group`

2. **`find`**: Navegación de símbolos
   - Kinds: `definition`, `usages`, `imports`, `dependents`
   - Soporta project y group

3. **`explore`**: Exploración de código
   - Kinds: `outline` (símbolos en archivo) o `similar` (chunks similares por chunk_id)
   - Soporta target, project, group

4. **`get_chunk`**: Obtener código fuente completo de un chunk
   - Devuelve contenido con líneas de contexto opcionales (0-20)

5. **`status`**: Estado del índice
   - Kinds: `index` (salud y estadísticas) o `projects` (lista de repos)

---

### 🏗️ Arquitectura

#### codesearch - Arquitectura

```
┌─────────────────────────────────────────────────────────────┐
│                    codesearch                          │
│  Búsqueda semántica con vector embeddings           │
└─────────────────────────────────────────────────────────────┘
                         │
                         ├──► LOCAL/AUTO (modo único repo)
                         │    └──► .codesearch.db (SQLite local)
                         │
                         └──► CLIENT+SERVE (modo multi-repo)
                              ├──► codesearch serve (HTTP server)
                              │    ├──► repos.json (registro de repos)
                              │    └──► /project-a/.codesearch.db
                              │    └──► /project-b/.codesearch.db
                              │
                              └──► codesearch mcp --mode client
                                   └──► Proxy stdio→HTTP
```

**Características arquitectónicas de codesearch:**
- **3 modos de operación**: LOCAL/AUTO, CLIENT+SERVE, Direct HTTP
- **Servidor HTTP persistente**: `codesearch serve` para Claude Desktop
- **File watcher**: Mantiene índice actualizado automáticamente
- **Multi-repo**: Un servidor maneja múltiples proyectos
- **Groups**: Búsqueda cross-repo con alias prefijados
- **Base de datos**: SQLite con embeddings vectoriales

#### CogniCode - Arquitectura

```
┌──────────────────────────────────────────────────────────────┐
│                      COGNICODE                               │
│                                                               │
│  ┌────────────────┐  ┌────────────────┐  ┌──────────────┐  │
│  │   DOMAIN       │  │  APPLICATION   │  │ INFRASTRUCTURE│ │
│  │   (Core)       │  │   (Services)   │  │  (Impl)       │ │
│  └───────┬────────┘  └───────┬────────┘  └──────┬───────┘  │
│          │                    │                   │          │
│          └────────────────────┼───────────────────┘          │
│                               │                              │
│                    ┌──────────┴──────────┐                    │
│                    │     INTERFACE       │                    │
│                    │   (MCP, LSP, CLI)  │                    │
│                    └────────────────────┘                    │
└──────────────────────────────────────────────────────────────┘
```

**Características arquitectónicas de CogniCode:**
- **4 bounded contexts** (DDD): Domain, Application, Infrastructure, Interface
- **ArcSwap graph cache**: Lecturas atómicas sin bloqueos
- **Rayon parallelism**: Thread pool dedicado (8MB stack por thread)
- **Workspace sandboxing**: Operaciones restringidas al workspace declarado
- **OpenTelemetry**: Métricas y observabilidad con exportación OTLP
- **Base de datos**: RedbGraphStore (redb embebida)
- **Persistencia**: Grafo persistente entre sesiones

---

### 💾 Storage y Datos

| Aspecto | codesearch | CogniCode |
|-----------|-------------|-------------|
| **Base de datos** | SQLite | Redb (embebida) |
| **Ubicación** | .codesearch.db en root de repo | .cognicode/data/ (configurable) |
| **Datos almacenados** | Chunks + embeddings + índice BM25 | Grafos de llamadas + símbolos + métricas |
| **Modelos de embedding** | MiniLM-L6 (384 dims, default), BGE Small (384), BGE Base (768), Jina Code (768), Omic v1.5 (768) | No usa embeddings |
| **Incremental updates** | ✅ File watcher + git branch detection | ✅ RedbGraphStore persistente |
| **Multi-repo** | ✅ Via groups en modo serve | ❌ Un solo workspace por instancia |

---

### 🌳 Lenguajes Soportados

#### codesearch - Lenguajes

**Full AST chunking** (tree-sitter):
- Rust
- Python
- JavaScript
- TypeScript
- C
- C++
- C#
- Go
- Java

**Line-based chunking**:
- Ruby, PHP, Swift, Kotlin, Shell, Markdown, JSON, YAML, TOML, SQL, HTML, CSS/SCSS

#### CogniCode - Lenguajes

**Soporte completo** (tree-sitter):
- Rust
- Python
- TypeScript
- JavaScript
- Go
- Java

---

### 🔍 Búsqueda Semántica

#### codesearch - Enfoque

```
Query → Vector embedding → BM25 + RRF fusion → Ranked results
```

**Algoritmo de ranking:**
1. **Vector similarity**: Cosine similarity con embeddings
2. **BM25**: Full-text search con ponderación probabilística
3. **RRF (Reciprocal Rank Fusion)**: Fusión de resultados de ambos métodos
4. **Neural reranking**: Opcional con modelo (~1.7s extra)

**Compact mode:**
- Devuelve solo metadatos: path, line range, kind, signature, score
- Usa `get_chunk` para obtener código completo
- Reduce consumo de tokens en 90%+

#### CogniCode - Enfoque

```
Symbol lookup → Graph traversal → Call hierarchy / Impact analysis
```

**Tipos de búsqueda:**
1. **Semantic search**: Fuzzy symbol search con filtrado por tipo
2. **Structural search**: Búsqueda de patrones AST
3. **Call hierarchy**: Navegación de grafos de llamadas
4. **Impact analysis**: Análisis de impacto de cambios
5. **Usages**: Encuentra todos los usos de un símbolo

**Context compression:**
- Devuelve resúmenes en lenguaje natural en vez de JSON crudo
- Reducción de tokens del 50%+

---

### 📈 Casos de Uso

#### codesearch - Mejor para

✅ **Búsqueda semántica de código**
- "Encuentra dónde se valida el token de autenticación"
- "Busca código relacionado con manejo de errores HTTP"

✅ **Búsqueda cross-repo**
- Búsqueda simultánea en múltiples repositorios relacionados
- Ideal para monorepos o microservicios

✅ **Claude Desktop**
- Modo CLIENT+SERVE diseñado específicamente para Claude Desktop
- Proceso persistente que maneja múltiples proyectos

✅ **Prototipado rápido**
- Indexado rápido con file watcher automático
- Compact mode para minimizar tokens

#### CogniCode - Mejor para

✅ **Análisis de arquitectura**
- Detección de ciclos con Tarjan SCC
- Análisis de dependencias a nivel de módulo
- Identificación de caminos críticos (hot paths)

✅ **Refactorización segura**
- Rename, extract, inline, move, change signature
- Preview de impacto antes de ejecutar
- Validación de sintaxis con tree-sitter

✅ **Análisis de impacto de cambios**
- "¿Qué pasa si cambio esta función?"
- Evaluación de riesgo (low/medium/high/critical)
- Archivos y símbolos afectados

✅ **Navegación de código profunda**
- Jerarquía de llamadas (callers/callees)
- Traza de caminos de ejecución entre símbolos
- Go-to-definition, hover, find references

✅ **Métricas de complejidad**
- Complejidad ciclomática
- Complejidad cognitiva
- Anidamiento máximo

✅ **Generación de diagramas**
- Exportación de grafos como Mermaid
- Visualización de arquitectura de llamadas

---

### 🔧 Configuración MCP

#### codesearch - Configuración

```json
// Claude Code (single repo)
{
  "mcpServers": {
    "codesearch": {
      "command": "codesearch",
      "args": ["mcp"]
    }
  }
}

// Claude Desktop (via serve)
{
  "mcpServers": {
    "codesearch": {
      "command": "codesearch",
      "args": ["mcp", "--mode", "client"]
    }
  }
}

// Direct HTTP (Streamable MCP)
{
  "mcp": {
    "codesearch": {
      "type": "remote",
      "url": "http://127.0.0.1:39725/mcp",
      "enabled": true
    }
  }
}
```

**Modos:**
- `--mode auto` (default): Probes for serve instance, fallback to local
- `--mode local`: Siempre usa base de datos local
- `--mode client`: Siempre se conecta a serve

#### CogniCode - Configuración

```json
// Claude Desktop / Cursor / Windsurf
{
  "mcpServers": {
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--cwd", "/path/to/your/project"]
    }
  }
}
```

**Opciones:**
- `--cwd`: Directorio de trabajo (obligatorio)
- `RUST_LOG`: Nivel de logging (trace/debug/info/warn/error)
- `OTEL_EXPORTER_OTLP_ENDPOINT`: Endpoint de métricas (default: http://localhost:4317)

---

### ⚡ Performance y Escalabilidad

| Aspecto | codesearch | CogniCode |
|-----------|-------------|-------------|
| **Indexado inicial** | Moderado (procesa embeddings) | Rápido (sin embeddings) |
| **Incremental updates** | ✅ File watcher automático | ✅ Grafo persistente |
| **Multi-repo** | ✅ Groups + serve mode | ❌ Un workspace por instancia |
| **Concurrent access** | ✅ Single-writer lock | ✅ ArcSwap (lock-free reads) |
| **Thread pool** | No especificado | Rayon (8MB stack por thread) |
| **Métricas** | Básicas (index stats) | OpenTelemetry completo |

---

### 🔒 Seguridad

| Aspecto | codesearch | CogniCode |
|-----------|-------------|-------------|
| **Validación de paths** | No especificado | ✅ InputValidator con workspace sandboxing |
| **Path traversal** | No especificado | ✅ Prevención de path traversal |
| **Rate limiting** | No especificado | ✅ Token bucket (100 req/min default) |
| **Symlink protection** | No especificado | ✅ Detección de symlinks |
| **Tamaño máximo de archivos** | No especificado | ✅ 10MB default |
| **Workspace restriction** | No especificado | ✅ Todas las ops restringidas al workspace |

---

### 📦 Instalación y Distribución

#### codesearch

```bash
# Pre-built binaries
# Windows x86_64
codesearch-windows-x86_64.zip

# Linux x86_64
codesearch-linux-x86_64.tar.gz

# macOS Apple Silicon
codesearch-macos-arm64.tar.gz

# Build from source
git clone https://github.com/flupkede/codesearch.git
cd codesearch
cargo build --release
```

**Uso:**
```bash
# Indexar código
codesearch index

# Iniciar servidor (para Claude Desktop)
codesearch serve

# Buscar CLI
codesearch search "database connection pooling"
```

#### CogniCode

```bash
# Pre-built binary
chmod +x cognicode-mcp
./cognicode-mcp --cwd /path/to/your/project

# Build from source
git clone https://github.com/Rubentxu/CogniCode.git
cd CogniCode
cargo build --release -p cognicode-mcp
```

**Uso:**
```bash
# Verificar entorno
cargo run -p cognicode-cli -- doctor

# Ejecutar tests
cargo test --workspace
```

---

## 🎯 Recomendaciones

### Usar codesearch cuando:

1. **Necesitas búsqueda semántica natural**
   - "Encuentra código relacionado con autenticación"
   - "Busca funciones que manejan archivos CSV"

2. **Trabajas con Claude Desktop**
   - Modo CLIENT+SERVE está optimizado para Claude Desktop
   - No requiere contexto de proyecto

3. **Tienes múltiples repositorios relacionados**
   - Groups permiten búsqueda cross-repo
   - Ideal para monorepos o microservicios

4. **Quieres prototipar rápido**
   - Indexado automático con file watcher
   - Compact mode minimiza consumo de tokens

### Usar CogniCode cuando:

1. **Necesitas análisis profundo de código**
   - Detección de ciclos arquitectónicos
   - Análisis de dependencias
   - Métricas de complejidad

2. **Quieres refactorizar código de forma segura**
   - Rename, extract, inline, move, change signature
   - Preview de impacto antes de ejecutar
   - Validación de sintaxis automática

3. **Necesitas entender el flujo de ejecución**
   - Jerarquía de llamadas completa
   - Traza de caminos entre símbolos
   - Análisis de impacto de cambios

4. **Quieres generar documentación**
   - Exportación de grafos como Mermaid
   - Visualización de arquitectura

5. **Usas OpenCode o Claude Code (con contexto de proyecto)**
   - CogniCode espera estar dentro del directorio del proyecto
   - Funciona perfectamente con CLI tools

---

## 🚀 Complementariedad

Ambas herramientas pueden complementarse:

```
┌─────────────────────────────────────────────────────────┐
│             Agente IA (Claude/OpenCode)           │
└─────────────────────────────────────────────────────────┘
                         │
             ┌───────────┴───────────┐
             ▼                       ▼
┌─────────────────────┐    ┌─────────────────────┐
│   codesearch       │    │    CogniCode      │
│  Búsqueda semántica │    │  Análisis profundo  │
│  Vector + BM25     │    │  Grafos + Refactor │
└─────────────────────┘    └─────────────────────┘
```

**Workflow recomendado:**
1. Usar codesearch para encontrar código relevante semánticamente
2. Usar CogniCode para analizar dependencias y refactorizar
3. Combinar: búsqueda natural + análisis estructural

---

## 📊 Comparación Rápida

| Criterio | codesearch | CogniCode | Ganador |
|-----------|-------------|-------------|----------|
| Búsqueda semántica natural | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | codesearch |
| Análisis de arquitectura | ⭐⭐ | ⭐⭐⭐⭐⭐ | CogniCode |
| Refactorización segura | ❌ | ⭐⭐⭐⭐⭐ | CogniCode |
| Multi-repo | ⭐⭐⭐⭐⭐ | ⭐ | codesearch |
| Grafo de llamadas | ⭐ | ⭐⭐⭐⭐⭐ | CogniCode |
| Métricas de complejidad | ⭐ | ⭐⭐⭐⭐ | CogniCode |
| Generación de diagramas | ⭐ | ⭐⭐⭐ | CogniCode |
| Seguridad y validación | ⭐⭐ | ⭐⭐⭐⭐ | CogniCode |
| Claude Desktop soporte | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | codesearch |
| Observabilidad | ⭐⭐ | ⭐⭐⭐⭐ | CogniCode |
| **Herramientas MCP** | **5** | **35+** | CogniCode |

---

## 💡 Conclusión

**codesearch** es la mejor opción para:
- Búsqueda semántica natural de código
- Trabajo con Claude Desktop
- Proyectos multi-repo
- Prototipado rápido

**CogniCode** es la mejor opción para:
- Análisis profundo de arquitectura
- Refactorización segura con impacto preview
- Navegación de grafos de llamadas
- Métricas de complejidad
- Generación de documentación

**Recomendación**: Ambas herramientas son complementarias. Para la máxima efectividad, usa codesearch para encontrar código y CogniCode para analizar y refactorizar.

---

## 📚 Referencias

- **codesearch**: https://github.com/flupkede/codesearch
- **CogniCode**: https://github.com/Rubentxu/CogniCode
- **MCP Protocol**: https://modelcontextprotocol.io
- **Tree-sitter**: https://tree-sitter.github.io/

---

**Documento creado**: 27 de abril de 2026  
**Autor**: Análisis comparativo basado en documentación pública y arquitectura conocida
