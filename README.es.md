<p align="center">
  <h1 align="center">CogniCode</h1>
  <p align="center">
    <strong>Servidor de inteligencia de código Super-LSP para agentes de IA</strong>
  </p>
  <p align="center">
    <a href="#características">Características</a> · <a href="#instalación">Instalación</a> · <a href="#herramientas-mcp">Herramientas MCP</a> · <a href="#cli">CLI</a> · <a href="#arquitectura">Arquitectura</a> · <a href="README.md">English</a>
  </p>
</p>

---

CogniCode es un servidor de inteligencia de código escrito en Rust que proporciona análisis profundo, grafos de llamadas, búsqueda semántica y refactorización segura a agentes de IA a través del [Model Context Protocol (MCP)](https://modelcontextprotocol.io). Imagina las capacidades de IntelliJ IDEA — expuestas como herramientas que tu IA puede invocar.

Construido con **Domain-Driven Design** y **Clean Architecture**, soporta seis lenguajes de serie.

## Características

- **32 herramientas MCP** — grafos de llamadas, análisis de impacto, búsqueda semántica, refactorización segura y más
- **6 lenguajes** — Rust, Python, TypeScript, JavaScript, Go, Java (mediante Tree-sitter)
- **4 estrategias de grafo** — `full`, `lightweight`, `on_demand`, `per_file`
- **Caché persistente** — RedbGraphStore sobrevive entre sesiones (base de datos embebida `redb`)
- **Refactorización segura** — renombrar, extraer, inline, mover, cambiar firma con vista previa de impacto
- **Navegación LSP** — ir a definición, hover, buscar referencias mediante servidores de lenguaje
- **Análisis de arquitectura** — detección de ciclos (Tarjan SCC), evaluación de riesgo, identificación de caminos críticos
- **Exportación Mermaid** — genera diagramas de grafo de llamadas como código o SVG renderizado
- **Compresión de contexto** — devuelve resúmenes en lenguaje natural en vez de JSON crudo
- **Orquestador de sandbox** — testing automatizado y benchmarking de escenarios
- **Inicio sin configuración** — funciona directamente con `cognicode-mcp --cwd /tu/proyecto`

## Instalación

### Binario precompilado

Descarga la última release desde [GitHub Releases](https://github.com/Rubentxu/CogniCode/releases):

```bash
# Linux (x86_64)
chmod +x cognicode-mcp
./cognicode-mcp --cwd /ruta/a/tu/proyecto
```

### Desde fuente

```bash
git clone https://github.com/Rubentxu/CogniCode.git
cd CogniCode
cargo build --release -p cognicode-mcp
./target/release/cognicode-mcp --cwd /ruta/a/tu/proyecto
```

### Claude Desktop / Cursor / Windsurf

Añade CogniCode como servidor MCP en la configuración de tu cliente de IA:

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--cwd", "/ruta/a/tu/proyecto"]
    }
  }
}
```

## Inicio Rápido

1. **Construye el grafo** — CogniCode necesita analizar tu proyecto primero:

```json
{ "tool": "build_graph", "arguments": { "directory": "/ruta/al/proyecto" } }
```

2. **Analiza el impacto** de cambiar un símbolo:

```json
{ "tool": "analyze_impact", "arguments": { "symbol_name": "mi_funcion" } }
```

3. **Traza el camino de ejecución** entre dos símbolos:

```json
{ "tool": "trace_path", "arguments": { "source": "main", "target": "manjar_peticion" } }
```

4. **Encuentra caminos críticos** — funciones más llamadas:

```json
{ "tool": "get_hot_paths", "arguments": { "limit": 10, "min_fan_in": 3 } }
```

5. **Refactoriza de forma segura** con vista previa de impacto:

```json
{
  "tool": "safe_refactor",
  "arguments": {
    "action": "rename",
    "new_name": "nuevo_nombre_funcion",
    "file_path": "src/lib.rs",
    "line": 42,
    "column": 4
  }
}
```

## Herramientas MCP

### Análisis de Grafos

| Herramienta | Descripción |
|-------------|-------------|
| `build_graph` | Construye el grafo de llamadas de un proyecto. Persiste en disco automáticamente. |
| `get_call_hierarchy` | Recorre llamadores/llamados de un símbolo. |
| `analyze_impact` | Analiza el impacto de cambiar un símbolo. Devuelve nivel de riesgo. |
| `check_architecture` | Detecta ciclos y violaciones de arquitectura (Tarjan SCC). |
| `get_entry_points` | Encuentra puntos de entrada (sin aristas entrantes). |
| `get_leaf_functions` | Encuentra funciones hoja (sin aristas salientes). |
| `get_hot_paths` | Encuentra las funciones más llamadas por fan-in. |
| `trace_path` | Encuentra camino de ejecución entre dos símbolos (BFS). |
| `export_mermaid` | Exporta el grafo como diagrama Mermaid o SVG. |
| `build_lightweight_index` | Construye un índice rápido de solo símbolos. |
| `query_symbol_index` | Búsqueda de símbolos insensible a mayúsculas. |
| `build_call_subgraph` | Construye un subgrafo bajo demanda centrado en un símbolo. |
| `get_per_file_graph` | Obtiene el grafo de llamadas de un solo archivo. |
| `merge_file_graphs` | Fusiona grafos de múltiples archivos. |

### Símbolos y Semántica

| Herramienta | Descripción |
|-------------|-------------|
| `get_file_symbols` | Extrae símbolos de un archivo. Soporta resúmenes comprimidos. |
| `get_outline` | Esquema jerárquico de símbolos (estructura de árbol). |
| `get_symbol_code` | Obtiene el código fuente completo de un símbolo incluyendo documentación. |
| `get_complexity` | Métricas de complejidad ciclomática, cognitiva y de anidamiento. |
| `semantic_search` | Búsqueda difusa de símbolos con filtrado por tipo. |
| `find_usages` | Encuentra todos los usos de un símbolo en el proyecto. |
| `find_usages_with_context` | Encuentra usos con líneas de contexto circundante. |
| `structural_search` | Búsqueda estructural basada en AST (pattern matching). |
| `validate_syntax` | Valida la sintaxis de un archivo mediante Tree-sitter. |

### Navegación LSP

| Herramienta | Descripción |
|-------------|-------------|
| `go_to_definition` | Navega a la definición de un símbolo. |
| `hover` | Obtiene información de tipos y documentación. |
| `find_references` | Encuentra todas las referencias a un símbolo. |

### Operaciones de Archivos

| Herramienta | Descripción |
|-------------|-------------|
| `read_file` | Lector inteligente con modos outline/símbolos/comprimido. |
| `search_content` | Búsqueda por regex con conciencia de .gitignore. |
| `list_files` | Lista archivos del proyecto con filtrado por glob. |
| `write_file` | Crea o sobrescribe archivos dentro del workspace. |
| `edit_file` | Edita archivos con validación de sintaxis. |
| `safe_refactor` | Refactorización segura con validación y vista previa. |

## CLI

CogniCode incluye una CLI completa (`cognicode`) para uso directo en terminal:

```
cognicode analyze [ruta]                              # Análisis completo de código
cognicode doctor [--format text|json]                 # Verificar configuración del entorno

cognicode index build [ruta] [--strategy full|lightweight|per_file|on_demand]
cognicode index query <símbolo> [ruta]
cognicode index outline <archivo>
cognicode index symbol-code <archivo> <línea> <col>

cognicode graph full [--rebuild] [ruta]
cognicode graph hot-paths [-n 10] [ruta]
cognicode graph trace-path <desde> <hasta> [ruta]
cognicode graph mermaid [ruta] [--format svg|txt]
cognicode graph complexity [ruta]
cognicode graph impact <símbolo> [ruta]

cognicode refactor rename|extract|inline|move <símbolo> [nuevo_nombre]

cognicode navigate definition|hover|references <archivo:línea:col> [ruta]
```

## Estrategias de Grafo

Elige la estrategia adecuada para tu caso de uso:

| Estrategia | Velocidad | Aristas | Mejor para |
|------------|-----------|---------|------------|
| `lightweight` | La más rápida | Ninguna | Búsquedas de símbolos |
| `on_demand` | Rápida | Dirigidas | Analizar funciones específicas |
| `per_file` | Media | Por archivo | Análisis modular |
| `full` | La más lenta | Completas | Análisis de impacto, caminos críticos, verificación de arquitectura |

## Lenguajes Soportados

| Lenguaje | Extensiones |
|----------|-------------|
| Rust | `.rs` |
| Python | `.py` |
| TypeScript | `.ts`, `.tsx` |
| JavaScript | `.js`, `.jsx` |
| Go | `.go` |
| Java | `.java` |

## Arquitectura

CogniCode sigue **Domain-Driven Design** con una arquitectura limpia por capas:

```
┌─────────────────────────────────────────┐
│             Capa de Interfaz            │
│   Handlers MCP │ CLI │ LSP               │
├─────────────────────────────────────────┤
│            Capa de Aplicación           │
│   WorkspaceSession │ DTOs │ Servicios    │
├─────────────────────────────────────────┤
│              Capa de Dominio            │
│   Agregados │ Traits │ Value Objects     │
│   Eventos │ Servicios de Dominio        │
├─────────────────────────────────────────┤
│          Capa de Infraestructura        │
│   Tree-sitter │ Grafos │ Persistencia   │
│   Semántica │ Refactorización │ LSP     │
└─────────────────────────────────────────┘
```

**Decisiones de diseño clave:**

- **Estrategias basadas en traits** — La construcción de grafos, refactorización y parsing son extensibles mediante traits
- **Caché de grafos con ArcSwap** — Lecturas atómicas sin bloqueos entre tareas async
- **Paralelismo con Rayon** — El cómputo pesado se ejecuta en un thread pool dedicado
- **Sandbox del workspace** — Todas las operaciones de archivos están restringidas al workspace declarado
- **Propagación de cancelación** — Los tokens `on_cancelled` de MCP fluyen por todos los handlers

## Crates del Workspace

| Crate | Descripción |
|-------|-------------|
| `cognicode-core` | Lógica de dominio, servicios de aplicación, infraestructura |
| `cognicode-mcp` | Servidor MCP (`cognicode-mcp`) y cliente de test (`mcp-client`) |
| `cognicode-cli` | Interfaz de terminal (`cognicode`) |
| `cognicode-sandbox` | Testing automatizado de escenarios y benchmarking |

## Configuración

### Variables de Entorno

| Variable | Por defecto | Descripción |
|----------|-------------|-------------|
| `RUST_LOG` | `info` | Nivel de log (`trace`, `debug`, `info`, `warn`, `error`) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | Endpoint de métricas OpenTelemetry |

### Flags de Características (`cognicode-core`)

| Feature | Por defecto | Descripción |
|---------|-------------|-------------|
| `persistence` | activado | RedbGraphStore para caché persistente de grafos |
| `rig` | desactivado | Integración con el framework de agentes `rig-core` |

## Desarrollo

```bash
# Construir todos los crates
cargo build --workspace

# Ejecutar tests (746 tests)
cargo test --workspace

# Construir binario de release
cargo build --release -p cognicode-mcp

# Verificar entorno
cargo run -p cognicode-cli -- doctor
```

## Uso con Agentes de IA

CogniCode está diseñado para ser la **columna vertebral de inteligencia de
código** de los agentes de IA. En lugar de pedirle al agente que lea archivos
e intuya la estructura, le das herramientas que devuelven respuestas precisas
y estructuradas.

📖 **[docs/agent-prompts.md](docs/agent-prompts.md)** contiene 20 escenarios
de prompts listos para usar, con cadenas de razonamiento completas y secuencias
de tool calls. Un vistazo:

---

### Explorando un Repositorio Nuevo

> *"Acabo de clonar este repo. Ayúdame a entender qué hace, cuáles son los
> puntos de entrada principales y qué funciones se llaman más."*

**Razonamiento del agente:** Primero construir el grafo completo, luego obtener
los entry points (superficie pública de la API), las leaf functions
(primitivos de bajo nivel) y los hot paths (código más interconectado). Los
tres juntos dan una visión 360° de cualquier base de código desconocida.

```
1. build_graph        → strategy: "full"
2. get_entry_points   → la superficie pública de la API
3. get_leaf_functions → los primitivos de bajo nivel
4. get_hot_paths      → min_fan_in: 3  (las funciones carga-crítica)
```

---

### Análisis de Impacto Antes de un PR

> *"Voy a cambiar `UserRepository.find_by_email`. ¿Qué puede romperse?"*

```
1. build_lightweight_index
2. query_symbol_index  → symbol_name: "find_by_email"
3. analyze_impact      → symbol_name: "UserRepository.find_by_email"
4. get_call_hierarchy  → direction: "incoming", depth: 3
```

El agente obtiene un nivel de riesgo (`low` / `medium` / `high`), una lista de
archivos impactados y la cadena de llamadas completa — antes de tocar una sola
línea de código.

---

Estos son solo 2 de los 20 escenarios. La guía completa cubre **detección de
código muerto, refactoring seguro con rename, auditorías de complejidad,
trazado de rutas de ejecución** y mucho más.

👉 [Lee la Guía Completa de Prompts para Agentes →](docs/agent-prompts.md)

## Licencia

Ver [LICENSE](LICENSE) para más detalles.

## Contribuciones

¡Las contribuciones son bienvenidas! No dudes en enviar un Pull Request.
