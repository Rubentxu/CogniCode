# Manual de Usuario de CogniCode

Este manual proporciona orientacion practica para usuarios finales que desean instalar, configurar y usar CogniCode.

## Tabla de Contenidos

1. [Instalacion](#instalacion)
2. [Inicio Rapido](#inicio-rapido)
3. [Uso de CLI](#uso-de-cli)
4. [Configuracion](#configuracion)
5. [Solucion de Problemas](#solucion-de-problemas)

---

## Instalacion

### Requisitos Previos

- **Rust 1.70+**: Requerido para compilar desde el codigo fuente
- **Cargo**: Gestor de paquetes de Rust (viene con Rust)

### Compilar desde el Codigo Fuente

```bash
# Clone the repository
git clone https://github.com/your-org/cognicode.git
cd cognicode

# Build release version
cargo build --release

# The binaries will be in target/release/
ls target/release/
# cognicode      - Main CLI
# cognicode-mcp  - MCP server binary
# cognicode-lsp  - LSP server binary
```

### Verificar la Instalacion

```bash
# Check CLI version
./target/release/cognicode --version

# Expected output:
# cognicode 0.1.0
```

### Configuracion de Directorios

Crea un directorio de trabajo para CogniCode:

```bash
# Create a bin directory in your home
mkdir -p ~/bin

# Copy binaries
cp target/release/cognicode ~/bin/
cp target/release/cognicode-mcp ~/bin/
cp target/release/cognicode-lsp ~/bin/

# Add to PATH (add to ~/.bashrc or ~/.zshrc)
export PATH="$HOME/bin:$PATH"

# Reload shell
source ~/.bashrc
```

---

## Inicio Rapido

### Uso Basico de CLI

```bash
# Analyze a directory
cognicode analyze ./src

# Start MCP server (for AI agents)
cognicode serve --port 8080

# Perform a refactoring check
cognicode refactor --operation rename "OldName" --new-name "NewName"
```

### Inicio Rapido del Servidor MCP

Para integracion con agentes de IA:

```bash
# Start the MCP server
cognicode-mcp --workspace /path/to/your/project

# Server will listen on stdin/stdout by default
# Configure your AI agent to connect to this server
```

---

## Uso de CLI

### Opciones Globales

| Opcion | Descripcion |
|--------|-------------|
| `-v, --verbose` | Habilitar registro detallado/depuracion |
| `--help` | Mostrar informacion de ayuda |
| `--version` | Mostrar informacion de version |

### Resumen de Comandos

```
cognicode [OPTIONS] [COMMAND]

Commands:
  analyze    Analyze code in the given directory
  serve      Start the MCP server
  refactor   Perform a refactoring operation
  help       Print this help message
```

### Comando Analyze

Analiza codigo en un directorio para simbolos, dependencias y complejidad.

```bash
cognicode analyze [PATH] [OPTIONS]

Arguments:
  PATH    Directory to analyze (default: current directory)

Options:
  -v, --verbose    Enable verbose output
```

**Ejemplos**:

```bash
# Analyze current directory
cognicode analyze

# Analyze specific directory
cognicode analyze ./src

# Analyze with verbose output
cognicode analyze ./src --verbose
```

### Comando Serve

Inicia el servidor MCP para conexiones de agentes de IA.

```bash
cognicode serve [OPTIONS]

Options:
  -p, --port <PORT>    Port to listen on (default: 8080)
  -v, --verbose        Enable verbose output
```

**Ejemplos**:

```bash
# Start server on default port
cognicode serve

# Start server on custom port
cognicode serve --port 9090

# Start with verbose logging
cognicode serve --verbose
```

### Comando Refactor

Realiza operaciones de refactorizacion en simbolos de codigo.

```bash
cognicode refactor [OPTIONS]

Options:
  -o, --operation <OPERATION>    Refactoring operation (default: rename)
  -s, --symbol <SYMBOL>          Symbol to refactor
  -n, --new-name <NAME>          New name (for rename operation)
```

**Operaciones**:

| Operacion | Descripcion |
|-----------|-------------|
| `rename` | Renombrar un simbolo |
| `extract` | Extraer codigo en una funcion |
| `inline` | Integrar una funcion |
| `move` | Mover un simbolo |
| `change-signature` | Cambiar parametros de funcion |

**Ejemplos**:

```bash
# Rename a symbol
cognicode refactor --operation rename --symbol "process_order" --new-name "handle_order"

# Extract a function
cognicode refactor --operation extract --symbol "order.total()"

# Change function signature
cognicode refactor --operation change-signature --symbol "create_user"
```

---

## Configuracion

### Variables de Entorno

| Variable | Descripcion | Valor Predeterminado |
|----------|-------------|----------------------|
| `COGNICODE_WORKSPACE` | Directorio raiz del espacio de trabajo | Directorio actual |
| `COGNICODE_MAX_FILE_SIZE` | Tamano maximo de archivo en bytes | 10485760 (10MB) |
| `COGNICODE_MAX_RESULTS` | Maximo de resultados por consulta | 10000 |
| `COGNICODE_MAX_QUERY_LENGTH` | Longitud maxima de consulta | 1000 |
| `COGNICODE_RATE_LIMIT` | Solicitudes por minuto | 100 |
| `RUST_LOG` | Nivel de registro | info |

### Archivo de Configuracion

CogniCode lee la configuracion desde `cognicode.toml` en la raiz del proyecto:

```toml
[workspace]
path = "/path/to/project"
max_file_size = 10485760

[security]
rate_limit = 100
max_query_length = 1000
max_results = 10000

[logging]
level = "info"
```

### Configuracion del Workspace

El workspace es el directorio raiz para el analisis de codigo:

```bash
# Via environment variable
export COGNICODE_WORKSPACE=/path/to/project
cognicode analyze

# Via cognicode.toml
[workspace]
path = "/path/to/project"
```

### Configuracion de Seguridad

```toml
[security]
# Path traversal prevention is always enabled
# Rate limiting
rate_limit = 100  # requests per minute

# Query limits
max_query_length = 1000  # characters
max_results = 10000       # per query

# File size limits
max_file_size = 10485760  # 10MB
```

---

## Solucion de Problemas

### Problemas Comunes

#### Problema: "Binary not found"

**Sintoma**: `bash: cognicode: command not found`

**Solucion**:
```bash
# Check if binary exists
ls -la ~/bin/cognicode

# If not, copy from build directory
cp target/release/cognicode ~/bin/

# Ensure ~/bin is in PATH
echo $PATH | grep -q ~/bin || export PATH="$HOME/bin:$PATH"
```

#### Problema: "Connection refused" al servir

**Sintoma**: No se puede conectar al servidor MCP en el puerto especificado

**Soluciones**:
1. Verificar si otro proceso esta usando el puerto:
   ```bash
   lsof -i :8080
   # or
   netstat -an | grep 8080
   ```

2. Intentar un puerto diferente:
   ```bash
   cognicode serve --port 9090
   ```

3. Verificar configuracion del firewall:
   ```bash
   # Allow port 8080
   sudo ufw allow 8080/tcp
   ```

#### Problema: Errores de "Path traversal attempt"

**Sintoma**: Error de seguridad al usar rutas de archivo

**Solucion**: Usar rutas relativas dentro de tu workspace:
```bash
# Instead of absolute paths
cognicode analyze ./src

# Or set workspace and use relative paths
export COGNICODE_WORKSPACE=/home/user/project
cd /home/user/project
cognicode analyze ./src
```

#### Problema: "Rate limit exceeded"

**Sintoma**: Error de demasiadas solicitudes

**Solucion**:
1. Esperar 60 segundos para el reinicio del limite de tasa
2. O aumentar el limite en el entorno:
   ```bash
   export COGNICODE_RATE_LIMIT=200
   ```

#### Problema: Resultados vacios del analisis

**Sintoma**: El analisis se completa pero no devuelve simbolos

**Posibles Causas**:
1. Workspace no configurado correctamente
2. Tipo de archivo no soportado
3. Errores de parsing en archivos fuente

**Depuracion**:
```bash
# Run with verbose logging
cognicode analyze ./src --verbose

# Check output for parsing errors
# Look for "Parse error" or "Symbol not found" messages
```

#### Problema: Desconexion del servidor MCP

**Sintoma**: El agente de IA se desconecta del servidor MCP

**Soluciones**:
1. Revisar los logs del servidor para errores
2. Verificar la conexion stdin/stdout
3. Intentar modo TCP en su lugar:
   ```bash
   cognicode-mcp --port 8080
   # Then connect via TCP instead of stdio
   ```

### Registro (Logging)

Habilitar registro detallado para solucion de problemas:

```bash
# Debug level logging
export RUST_LOG=debug
cognicode analyze ./src --verbose

# Or for a specific module
export RUST_LOG=cognicode=debug
cognicode serve --verbose
```

### Problemas de Rendimiento

#### Problema: Analisis lento en codigo grande

**Soluciones**:
1. Limitar el alcance del analisis:
   ```bash
   # Analyze specific subdirectory
   cognicode analyze ./src/module1
   ```

2. Aumentar el limite de tasa para MCP:
   ```bash
   export COGNICODE_RATE_LIMIT=200
   ```

3. Excluir directorios innecesarios:
   ```toml
   [workspace]
   exclude = ["**/tests/**", "**/target/**", "**/node_modules/**"]
   ```

### Obtener Ayuda

```bash
# Show all available commands
cognicode --help

# Show help for specific command
cognicode analyze --help
cognicode serve --help
cognicode refactor --help
```

### Reportar Errores

Cuando reportes problemas, incluye:

1. **Version**: `cognicode --version`
2. **Comando**: El comando exacto que fallo
3. **Entorno**: SO, version de Rust, ruta del workspace
4. **Logs**: Salida detallada con `RUST_LOG=debug`
5. **Reproduccion minima**: Ejemplo de codigo mas pequeno que demuestre el problema

---

## Recursos Adicionales

- [Documentacion de Arquitectura](architecture-es.md)
- [Vision Conceptual](concept-es.md)
- [Guia de Configuracion de Agentes](agent-setup-es.md)
- [Referencia de CLI](cli-reference-es.md)
- [Referencia de Herramientas MCP](mcp-tools-reference-es.md)