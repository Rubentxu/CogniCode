# Referencia de CLI

Referencia completa para la interfaz de linea de comandos de CogniCode.

## Tabla de Contenidos

1. [Sinopsis](#sinopsis)
2. [Opciones Globales](#opciones-globales)
3. [Comandos](#comandos)
4. [Formatos de Salida](#formatos-de-salida)
5. [Codigos de Salida](#codigos-de-salida)
6. [Ejemplos](#ejemplos)

---

## Sinopsis

```bash
cognicode [OPCIONES] [COMANDO]
```

La CLI de CogniCode proporciona comandos para analisis de codigo, gestion del servidor MCP, y operaciones de refactorizacion.

---

## Opciones Globales

| Opcion | Descripcion | Valor por defecto |
|--------|-------------|-------------------|
| `-v, --verbose` | Habilita registro detallado (establece RUST_LOG=debug) | false |
| `-h, --help` | Muestra informacion de ayuda | - |
| `--version` | Muestra informacion de version | - |

---

## Comandos

### analyze

Analiza codigo en un directorio para simbolos, dependencias y metricas.

```bash
cognicode analyze [RUTA] [OPCIONES]
```

**Argumentos**:

| Argumento | Descripcion | Valor por defecto |
|-----------|-------------|-------------------|
| `RUTA` | Directorio a analizar | Directorio actual (`.`) |

**Opciones**:

| Opcion | Descripcion |
|--------|-------------|
| `-v, --verbose` | Habilita salida detallada |

**Formato de Salida**:

```
Analyzing: ./src
  Found 42 symbols
  3 cycles detected
  Average complexity: 4.2
Analysis completed in 125ms
```

---

### serve

Inicia el servidor MCP para conexiones de agentes de IA.

```bash
cognicode serve [OPCIONES]
```

**Opciones**:

| Opcion | Descripcion | Valor por defecto |
|--------|-------------|-------------------|
| `-p, --port <PUERTO>` | Puerto TCP para escuchar | 8080 |
| `-v, --verbose` | Habilita registro detallado | false |

**Formato de Salida**:

```
Starting MCP server...
Listening on port 8080
MCP server ready
```

**Modos de Conexion**:

1. **Modo TCP** (por defecto con --port):
   ```bash
   cognicode serve --port 8080
   # Se conecta via socket TCP
   ```

2. **Modo Stdio** (por defecto sin --port):
   ```bash
   cognicode serve
   # Escucha en stdin/stdout para el protocolo MCP
   ```

---

### refactor

Realiza operaciones de refactorizacion en simbolos de codigo.

```bash
cognicode refactor [OPCIONES]
```

**Opciones**:

| Opcion | Descripcion | Valor por defecto |
|--------|-------------|-------------------|
| `-o, --operation <OPERACION>` | Operacion de refactorizacion | rename |
| `-s, --symbol <SIMBOLO>` | Simbolo a refactorizar | - |
| `-n, --new-name <NOMBRE>` | Nuevo nombre (para renombrar) | - |
| `-v, --verbose` | Habilita salida detallada | false |

**Operaciones**:

| Operacion | Descripcion |
|-----------|-------------|
| `rename` | Renombra un simbolo en todo el codigo |
| `extract` | Extrae codigo a una nueva funcion |
| `inline` | Inline de una funcion en sus llamadores |
| `move` | Mueve un simbolo a una ubicacion diferente |
| `change-signature` | Modifica parametros de funcion |

---

### help

Muestra informacion de ayuda.

```bash
cognicode help [COMANDO]
```

**Argumentos**:

| Argumento | Descripcion |
|-----------|-------------|
| `COMANDO` | Comando especifico opcional para obtener ayuda |

---

## Formatos de Salida

### Salida Predeterminada (Legible para Humanos)

```
$ cognicode analyze ./src
Analyzing: ./src
  Processing files...  [####################] 100%
  Found 156 symbols in 23 files
  Complexity analysis complete
  Average cyclomatic complexity: 3.4

Results:
  High complexity functions (>10):
    - calculate_totals (src/order.rs:42) - complexity: 12
    - process_payment (src/payment.rs:88) - complexity: 15

Analysis completed in 342ms
```

### Salida JSON (Legible para Maquinas)

Cuando la salida es canalizada o redirigida, CogniCode puede generar JSON:

```json
{
  "status": "success",
  "command": "analyze",
  "results": {
    "symbols": 156,
    "files": 23,
    "complexity": {
      "average": 3.4,
      "max": 15,
      "high_risk": 2
    }
  },
  "timing_ms": 342
}
```

### Salida Detallada (Debug)

```
$ cognicode analyze ./src --verbose
DEBUG [cognicode] Initializing parser for Rust
DEBUG [cognicode] Loading workspace from ./src
DEBUG [cognicode] Found 47 .rs files
DEBUG [cognicode] Parsing file: src/main.rs
DEBUG [cognicode] Found 5 symbols in src/main.rs
DEBUG [cognicode] Building call graph...
DEBUG [cognicode] Running cycle detection...
INFO  [cognicode] Analysis complete
Analyzing: ./src
  Found 156 symbols
Analysis completed in 342ms
```

---

## Codigos de Salida

| Codigo | Descripcion |
|--------|-------------|
| `0` | Exito |
| `1` | Error general |
| `2` | Argumentos invalidos |
| `3` | Error de analisis |
| `4` | Error del servidor |
| `5` | Error de seguridad (path traversal, rate limit) |

**Ejemplos**:

```bash
# Exito
cognicode analyze ./src
echo $?  # 0

# Argumentos invalidos
cognicode analyze --nonexistent-flag
echo $?  # 2

# Fallo de analisis
cognicode analyze ./empty_dir
echo $?  # 3
```

---

## Ejemplos

### Analisis Basico

```bash
# Analizar directorio actual
cognicode analyze

# Analizar directorio especifico
cognicode analyze ./src

# Analizar con salida detallada
cognicode analyze ./src --verbose
```

### Gestion del Servidor

```bash
# Iniciar servidor MCP en puerto predeterminado (8080)
cognicode serve

# Iniciar servidor en puerto personalizado
cognicode serve --port 9090

# Iniciar servidor con registro de debug
cognicode serve --verbose
```

### Refactorizacion

```bash
# Renombrar un simbolo
cognicode refactor \
  --operation rename \
  --symbol "process_order" \
  --new-name "handle_order"

# Extraer una funcion
cognicode refactor \
  --operation extract \
  --symbol "order.total()"

# Cambiar firma de funcion
cognicode refactor \
  --operation change-signature \
  --symbol "create_user"
```

### Ayuda

```bash
# Mostrar ayuda general
cognicode --help

# Mostrar ayuda del comando analyze
cognicode analyze --help

# Mostrar ayuda del comando refactor
cognicode refactor --help

# Mostrar ayuda del comando serve
cognicode serve --help
```

### Canalizacion y Scripting

```bash
# Capturar codigo de salida
cognicode analyze ./src && echo "Success" || echo "Failed"

# Usar en scripts
#!/bin/bash
if cognicode analyze ./src --verbose; then
  echo "Analysis successful"
else
  exit 1
fi

# Encadenar comandos
cognicode analyze ./src -v | grep -i "complexity"
```

### Manejo de Errores

```bash
# Verificar codigo de salida
cognicode analyze ./nonexistent 2>/dev/null
exit_code=$?
if [ $exit_code -ne 0 ]; then
  echo "Analysis failed with code: $exit_code"
fi

# Capturar salida de error
cognicode analyze ./src 2>&1 | grep -i error
```

---

## Recursos Adicionales

- [Manual de Usuario](user-manual-es.md)
- [Guia de Configuracion de Agentes](agent-setup-es.md)
- [Referencia de Herramientas MCP](mcp-tools-reference-es.md)
