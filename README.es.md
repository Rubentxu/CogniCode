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

Construido con **Domain-Driven Design** y **Clean Architecture**, analiza 31
lenguajes de serie: 20 con evidencia de aceptación completa y 11 experimentales.

## Características

- **88 herramientas MCP** — 78 herramientas y los 10 goals que las responden: grafos de llamadas, análisis de impacto, búsqueda semántica, refactorización segura, métricas de complejidad y más
- **31 lenguajes** — 20 `supported` (Rust, Python, TypeScript, JavaScript, Java, Go, C, C++, C#, Ruby, PHP, Swift, Scala, Haskell, Lua, Bash, JSON, YAML, Kotlin, Groovy) y 11 `experimental`, todos mediante Tree-sitter. El desglose exacto, con la evidencia de cada nivel, está en [`product/languages.json`](product/languages.json)
- **3 estrategias de grafo** — `full`, `lightweight`, `on_demand`
- **Caché persistente** — RedbGraphStore sobrevive entre sesiones (base de datos embebida `redb`)
- **Refactorización segura** — renombrar, extraer, inline, mover, cambiar firma con vista previa de impacto
- **Navegación LSP** — ir a definición, hover, buscar referencias
- **Análisis de arquitectura** — detección de ciclos (Tarjan SCC), evaluación de riesgo, identificación de caminos críticos, detección de código muerto
- **Exportación Mermaid** — genera diagramas de grafo de llamadas como código o SVG renderizado
- **Compresión de contexto** — devuelve resúmenes en lenguaje natural en vez de JSON crudo
- **Orquestador de sandbox** — testing automatizado y benchmarking de escenarios
- **Inicio sin configuración** — funciona directamente con `cognicode-mcp --cwd /tu/proyecto`
- **Integración OpenTelemetry** — soporte de métricas y observabilidad

## Instalación

### Bootstrap de una línea (Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/Rubentxu/CogniCode/main/install.sh | sh
```

Instala solo el ejecutable `cogh` en `~/.cognicode/bin` (verificado contra los checksums de la release oficial de GitHub). Fija una versión con `COGNICODE_VERSION=v0.102.1`. Después ejecuta `cogh init && cogh install mcp-server --profile reviewer --ide opencode` para instalar CLI, MCP y las skills portables. Consulta [Distribución e instalación](docs/distribution/INSTALL.md) para actualización, rollback, desinstalación, plataformas y recuperación.

### mise

```bash
mise install "github:Rubentxu/CogniCode[matching=cogh-]"
```

Fija una versión con `@v0.102.1`. Ambos canales instalan el mismo asset de la release publicada — la prueba de identidad por digest ya no está versionada en este árbol (limitación conocida). Contrato de propiedad: instalado vía mise, mise es dueño del binario `cogh` de Capa 0 (actualiza con `mise upgrade`); `cogh` solo es dueño del runtime de Capa 1 (`~/.cognicode`).

### Binario precompilado publicado (recuperación independiente)

Para una instalación no gestionada, extrae el tarball de tu plataforma desde
[GitHub Releases](https://github.com/Rubentxu/CogniCode/releases) **después**
de verificarlo con el `SHA256SUMS` de la release. El ejecutable está dentro
del archivo, en `bin/cognicode-mcp`; no es un fichero suelto en el directorio
de descargas. No utilices `target/release/cognicode-mcp` en una configuración
MCP persistente: es un build local de desarrollo. Utiliza preferentemente
`~/.cognicode/shims/cognicode-mcp`, instalado y gestionado por `cogh`.

### Desde fuente

> **No es el camino recomendado.** Para el uso normal, usa `install.sh` (Capa 0) + `cogh install mcp-server` (Capa 1).
> Desde fuente es para quien contribuye y para compilaciones reproducibles.

```bash
git clone https://github.com/Rubentxu/CogniCode.git
cd CogniCode
# Binarios del CLI (cogh, cognicode, cognicode-release)
cargo build --release -p cognicode-cli --features cognicode-core/evidence-kernel
# Daemon MCP
cargo build --release -p cognicode-mcp
./target/release/cogh --version         # esperado: cogh 0.102.1
./target/release/cognicode --version    # esperado: cognicode 0.102.1
./target/release/cognicode-mcp --version # esperado: cognicode-mcp 0.102.1
```

> **Nota:** los binarios publicados también incluyen los de la cadena de herramientas cruzada
> que el host no ejecuta de forma nativa (p. ej. `aarch64-unknown-linux-gnu`); los produce
> el pipeline de release. Compilarlos en local requiere el target correspondiente con
> `rustup target add <triple>` y un linker configurado. Las compilaciones desde fuente
> son nativas del host por defecto.

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
{ "tool": "trace_path", "arguments": { "source": "main", "target": "manejar_peticion" } }
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
    "target": "my_function",
    "params": { "new_name": "new_function_name", "file_path": "src/lib.rs" }
  }
}
```

## Herramientas MCP

CogniCode publica **88 herramientas MCP**: 78 herramientas y los 10 goals que
las responden. Las tablas siguientes son una
**selección curada**, agrupada por la capacidad que más le importa al lector,
no la superficie completa. La lista autoritativa, generada desde el código y
verificada en CI, está en [`crates/cognicode-core/product/tools.json`](crates/cognicode-core/product/tools.json); los
esquemas de entrada y salida por herramienta están en
[`product/README.md`](product/README.md).

### Análisis de Grafos (11 herramientas)

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

### Operaciones de Grafos (3 herramientas)

| Herramienta | Descripción |
|-------------|-------------|
| `build_call_subgraph` | Construye un subgrafo bajo demanda centrado en un símbolo. |
| `get_per_file_graph` | Obtiene el grafo de llamadas de un solo archivo. |
| `merge_graphs` | Fusiona grafos de múltiples archivos. |

### Símbolos y Semántica (4 herramientas)

| Herramienta | Descripción |
|-------------|-------------|
| `get_file_symbols` | Extrae símbolos de un archivo. Soporta resúmenes comprimidos. |
| `get_symbol_code` | Obtiene el código fuente completo de un símbolo incluyendo documentación. |
| `get_complexity` | Métricas de complejidad ciclomática, cognitiva y de anidamiento. |
| `find_usages` | Encuentra todos los usos de un símbolo en el proyecto. |

### Navegación LSP (3 herramientas)

| Herramienta | Descripción |
|-------------|-------------|
| `go_to_definition` | Navega a la definición de un símbolo. |
| `hover` | Obtiene información de tipos y documentación. |
| `find_references` | Encuentra todas las referencias a un símbolo. |

### Operaciones de Archivos (5 herramientas)

| Herramienta | Descripción |
|-------------|-------------|
| `read_file` | Lector inteligente con modos outline/símbolos/comprimido. |
| `search_content` | Búsqueda por regex con conciencia de .gitignore. |
| `list_files` | Lista archivos del proyecto con filtrado por glob. |
| `write_file` | Crea o sobrescribe archivos dentro del workspace. |
| `edit_file` | Edita archivos con validación de sintaxis. |

### Refactorización (1 herramienta)

| Herramienta | Descripción |
|-------------|-------------|
| `safe_refactor` | Refactorización segura con validación y vista previa (rename, extract, inline, move, change signature). |

## CLI

CogniCode incluye una CLI completa (`cognicode`) para uso directo en terminal:

```
cognicode analyze [ruta]                              # Análisis completo de código
cognicode doctor [--format text|json]                 # Verificar configuración del entorno

cognicode index build [ruta] [--strategy full|lightweight|on_demand]
cognicode index query <símbolo> [ruta]
cognicode index outline <archivo>
cognicode index symbol-code <archivo> <línea> <col>

cognicode graph full [--rebuild] [ruta]
cognicode graph hot-paths [-n 10] [ruta]
cognicode graph trace-path <desde> <hasta> [ruta]
cognicode graph mermaid [ruta] [--format svg|txt]
cognicode graph complexity [ruta]
cognicode graph impact <símbolo> [ruta]

cognicode refactor <símbolo> [nuevo_nombre] [--operation rename|extract|inline|move]

cognicode navigate definition|hover|references <archivo:línea:col> [ruta]
```

## Estrategias de Grafo

Elige la estrategia adecuada para tu caso de uso:

| Estrategia | Velocidad | Aristas | Mejor para |
|------------|-----------|---------|------------|
| `lightweight` | La más rápida | Ninguna | Búsquedas de símbolos |
| `on_demand` | Rápida | Dirigidas | Analizar funciones específicas |
| `full` | La más lenta | Completas | Análisis de impacto, caminos críticos, verificación de arquitectura |

## Lenguajes Soportados

Se analizan 31 lenguajes. **20 son `supported`**, con evidencia de aceptación
enfocada para las rutas de análisis; **11 son `experimental`**, es decir, se
parsean e indexan pero todavía no se someten al mismo listón. La lista
autoritativa por lenguaje, con la evidencia de cada nivel, está en
[`product/languages.json`](product/languages.json).

| Lenguaje | Extensiones | Soporte |
|----------|-------------|---------|
| Rust | `.rs` | supported |
| Python | `.py` | supported |
| TypeScript | `.ts`, `.tsx` | supported |
| JavaScript | `.js`, `.jsx` | supported |
| Java | `.java` | supported |
| Go | `.go` | supported |
| C | `.c`, `.h` | supported |
| C++ | `.cpp`, `.hpp`, `.cc` | supported |
| C# | `.cs` | supported |
| Ruby | `.rb` | supported |
| PHP | `.php` | supported |
| Swift | `.swift` | supported |
| Scala | `.scala` | supported |
| Haskell | `.hs` | supported |
| Lua | `.lua` | supported |
| Bash | `.sh`, `.bash` | supported |
| JSON | `.json` | supported |
| YAML | `.yaml`, `.yml` | supported |
| Dart | `.dart` | experimental |
| Elixir | `.ex`, `.exs` | experimental |
| Erlang | `.erl` | experimental |
| Fortran | `.f90` | experimental |
| Groovy | `.groovy` | supported |
| HCL | `.hcl`, `.tf` | experimental |
| Julia | `.jl` | experimental |
| Kotlin | `.kt`, `.kts` | supported |
| PowerShell | `.ps1` | experimental |
| R | `.r` | experimental |
| SystemVerilog | `.sv` | experimental |
| Verilog | `.v` | experimental |
| Zig | `.zig` | experimental |

## Arquitectura

CogniCode sigue **Domain-Driven Design** con Clean Architecture y 4 bounded contexts:

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

**Contexto de Dominio** (Lógica de negocio central):
- Agregados: `Symbol`, `CallGraph`, `Refactor`
- Value Objects: `Location`, `SourceRange`, `DependencyType`
- Servicios de Dominio: `ImpactAnalyzer`, `CycleDetector`, `ComplexityCalculator`
- Traits: `CodeIntelligenceProvider`, `DependencyRepository`, `RefactorStrategy`

**Contexto de Aplicación** (Orquestación):
- Servicios: `NavigationService`, `RefactorService`, `AnalysisService`
- DTOs: Contratos de solicitud/respuesta
- Comandos: Orquestadores de casos de uso

**Contexto de Infraestructura** (Implementaciones):
- Parsers: `TreeSitterParser`
- Almacenamiento de Grafos: `PetGraphStore`, `RedbGraphStore`
- LSP: `LspClient`
- Persistencia: `RedbGraphStore` (base de datos embebida `redb`)

**Contexto de Interfaz** (Protocolos externos):
- Servidor MCP (Model Context Protocol)
- Comandos CLI
- Servidor LSP

**Decisiones de diseño clave:**

- **Estrategias basadas en traits** — La construcción de grafos, refactorización y parsing son extensibles mediante traits
- **Caché de grafos con ArcSwap** — Lecturas atómicas sin bloqueos entre tareas async
- **Paralelismo con Rayon** — El cómputo pesado se ejecuta en un thread pool dedicado (8MB de stack por thread)
- **Sandbox del workspace** — Todas las operaciones de archivos están restringidas al workspace declarado
- **Propagación de cancelación** — Los tokens de cancelación de MCP fluyen por todos los handlers
- **Métricas OpenTelemetry** — Observabilidad integrada con exportación OTLP

## Crates del Workspace

| Crate | Descripción |
|-------|-------------|
| `cognicode` | Tipos compartidos y utilidades |
| `cognicode-core` | Lógica de dominio, servicios de aplicación, infraestructura |
| `cognicode-mcp` | Servidor MCP (`cognicode-mcp`) y cliente de test (`mcp-client`) |
| `cognicode-cli` | Interfaz de terminal (`cognicode`) |
| `cognicode-sandbox` | Testing automatizado de escenarios y benchmarking |
| `rcode-debug` | Integración de debugging time-travel (Chronos MCP) |

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

## Seguridad y límites

CogniCode es una herramienta local: analiza un repositorio de tu máquina y no
necesita red. Los límites que importan al ejecutarla, descritos completos en
[`SECURITY.md`](SECURITY.md):

- **El repositorio analizado es dato, no instrucciones.** Los ficheros, los
  comentarios y los resultados de herramientas nunca se tratan como órdenes.
- **Modo de solo lectura.** Arranca el servidor con `--read-only` para ocultar
  las herramientas que modifican de `tools/list` y rechazarlas.
- **Sin telemetría por defecto.** `COGNICODE_TELEMETRY=1` activa la exportación
  de métricas; nunca se envía código fuente. `COGNICODE_LINK_TRACE=<fichero>`
  activa un registro local de llamadas; puede contener rutas y nombres de
  símbolos, así que trata el fichero como sensible.
- **Flujos.** `cognicode-mcp` escribe JSON-RPC por stdout y los logs por
  stderr, nunca al revés.

Límites: un resultado `Partial`, `Unknown`, `Unsupported` o `Failed` no es uno
completo, y toda respuesta de análisis declara su cobertura. Reporta una
vulnerabilidad en privado con GitHub Security Advisories, no en una issue
pública.

## Desarrollo

```bash
# Construir todos los crates
cargo build --workspace

# Ejecutar tests
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

Un vistazo a los escenarios de prompts, con sus cadenas de razonamiento y
secuencias de tool calls:

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

## Versionado

- **Release vigente:** `v0.102.1`, cortada desde este árbol con ese tag. La
  release publicada desde él es la que sirve un `install.sh` sin versión
  fijada. La anterior publicada es
  [`v0.101.30`](https://github.com/Rubentxu/CogniCode/releases/tag/v0.101.30);
  `v0.101.24`, `v0.101.25` y `v0.101.26` tienen tag pero nunca se publicaron,
  y un tag que no se publicó no se mueve. v0.102.1 es una versión menor: el
  grafo se publica en orden canónico, el sobre de la respuesta (resultado,
  base, enlaces siguientes) tiene un solo dueño y cada pregunta declara la
  cobertura con la que se respondió.
- **Los binarios informan:** `0.102.1` (`cogh`, `cognicode`, `cognicode-mcp`).
- **Tag:** [`origin/v0.102.1`](https://github.com/Rubentxu/CogniCode/releases/tag/v0.102.1).
- **Tags sin release:** `v0.99.2`, `v0.100.0`, `v0.101.6`, `v0.101.7`,
  `v0.101.8`, `v0.101.18`, `v0.101.19`, `v0.101.24`, `v0.101.25` y
  `v0.101.26` existen y nunca se publicaron; cada uno tiene una entrada en el
  changelog que explica el motivo.
- **Releases publicadas con un conjunto de assets incompleto:** `v0.101.11` a
  `v0.101.16` publican 3 de 8 assets. La causa está en la entrada de
  v0.101.17 del changelog.
- **Autoridad de seguimiento:** la fila `PRODUCT-1.0` de [`docs/roadmap/ROADMAP.md` §2](docs/roadmap/ROADMAP.md).
- **Changelog:** [CHANGELOG.md](CHANGELOG.md).

Las releases se derivan de `[workspace.package] version` en `Cargo.toml` y las
comprueba `scripts/ci/release-tag-coherence.sh` (R8).

## Licencia

Publicado bajo **MIT OR Apache-2.0**, a tu elección. Los textos completos están
en [`LICENSE-MIT`](LICENSE-MIT) y [`LICENSE-APACHE`](LICENSE-APACHE); la
concesión está en [`LICENSE`](LICENSE) y el registro de la decisión en
[`docs/adr/ADR-053-cp1-license-publication.md`](docs/adr/ADR-053-cp1-license-publication.md).

## Contribuciones

Consulta [`CONTRIBUTING.md`](CONTRIBUTING.md) para el flujo que este proyecto
exige de verdad, [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) para el
comportamiento esperado de quienes participan, y [`SECURITY.md`](SECURITY.md)
antes de reportar una vulnerabilidad. Los canales de soporte están en
[`SUPPORT.md`](SUPPORT.md).
