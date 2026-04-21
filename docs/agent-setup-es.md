# Guia de Configuracion de Agentes

Esta guia explica como configurar agentes de IA (como Claude Desktop) para usar CogniCode a traves del Protocolo de Contexto de Modelo (MCP).

## Descripcion General

CogniCode proporciona un servidor **Super-LSP** que ofrece capacidades avanzadas de analisis de codigo y refactorizacion a agentes de IA a traves de MCP. Esto permite a los asistentes de IA:

- Navegar jerarquias de llamadas y relaciones entre simbolos
- Realizar operaciones de refactorizacion seguras con analisis de impacto
- Analizar complejidad del codigo y salud de la arquitectura
- Buscar patrones de codigo estructuralmente

## Arquitectura

```
┌─────────────────────┐     MCP (stdin/stdout)     ┌─────────────────────┐
│   Agente de IA      │◄───────────────────────────►│   Servidor MCP      │
│   (Claude Desktop)  │                            │   de CogniCode      │
└─────────────────────┘                            └─────────┬───────────┘
                                                              │
                                                    ┌─────────▼───────────┐
                                                    │   Nucleo de          │
                                                    │   CogniCode          │
                                                    │   - Dominio          │
                                                    │   - Aplicacion       │
                                                    │   - Infraestructura  │
                                                    └─────────────────────┘
```

## Configuracion del Servidor MCP

### Metodo de Conexion

CogniCode usa **stdio** (stdin/stdout) para la comunicacion MCP, que es el mecanismo de transporte estandar para integraciones locales de agentes.

### Binario del Servidor

El binario del servidor MCP se construye junto con el binario principal:

```bash
# Construir todos los binarios incluyendo el servidor MCP
cargo build --release

# El binario del servidor MCP estara en:
# target/release/cognicode-mcp
```

### Configuracion de Claude Desktop

Para conectar Claude Desktop a CogniCode, edita el archivo de configuracion de Claude Desktop:

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
**Linux:** `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "/ruta/a/cognicode-mcp",
      "args": [],
      "env": {
        "COGNICODE_WORKSPACE": "/ruta/a/tu/proyecto"
      }
    }
  }
}
```

### Variables de Entorno

| Variable | Descripcion | Valor por defecto |
|----------|-------------|-------------------|
| `COGNICODE_WORKSPACE` | Directorio raiz del espacio de trabajo para analisis | Directorio actual |
| `COGNICODE_MAX_FILE_SIZE` | Tamano maximo de archivo a procesar (bytes) | 10485760 (10MB) |
| `COGNICODE_MAX_RESULTS` | Maximo de resultados por consulta | 10000 |
| `COGNICODE_MAX_QUERY_LENGTH` | Longitud maxima de cadena de consulta | 1000 |
| `COGNICODE_RATE_LIMIT` | Solicitudes por minuto | 100 |
| `RUST_LOG` | Nivel de registro | info |

### Argumentos del Servidor MCP

| Argumento | Descripcion |
|-----------|-------------|
| `--port N` | Puerto TCP para modo TCP (opcional, stdin/stdout es el predeterminado) |
| `--workspace PATH` | Establecer directorio del espacio de trabajo |
| `--verbose` | Habilitar registro detallado |

## Detalles del Protocolo MCP

### Transporte

- **Predeterminado**: stdio (stdin/stdout)
- **Opcional**: Modo socket TCP con `--port`

### JSON-RPC 2.0

CogniCode implementa la especificacion JSON-RPC 2.0:

```json
// Solicitud
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_call_hierarchy",
    "arguments": {
      "symbol_name": "module::function",
      "direction": "outgoing",
      "depth": 1
    }
  },
  "id": 1
}

// Respuesta (Exito)
{
  "jsonrpc": "2.0",
  "result": { ... },
  "id": 1
}

// Respuesta (Error)
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Security error: Path traversal attempt"
  },
  "id": 1
}
```

### Codigos de Error

| Codigo | Significado |
|--------|-------------|
| -32600 | Solicitud Invalida |
| -32601 | Metodo No Encontrado |
| -32602 | Parametros Invalidos |
| -32603 | Error Interno |
| -32000 | Error de Seguridad |
| -32001 | Error de Aplicacion |
| -32002 | Entrada Invalida |
| -32003 | No Encontrado |

## Herramientas Disponibles

Una vez conectado, las siguientes herramientas estan disponibles:

| Herramienta | Proposito |
|-------------|-----------|
| `get_call_hierarchy` | Navegar grafos de llamadas (entrantes/salientes) |
| `get_file_symbols` | Extraer simbolos de un archivo |
| `find_usages` | Encontrar todos los usos de un simbolo |
| `structural_search` | Buscar por patron AST |
| `analyze_impact` | Analizar impacto de cambios |
| `check_architecture` | Detectar ciclos y violaciones |
| `safe_refactor` | Ejecutar refactorizacion validada |
| `validate_syntax` | Validacion rapida de sintaxis |
| `get_complexity` | Calcular metricas de complejidad |

Consulta [referencia-de-herramientas-mcp-es.md](mcp-tools-reference-es.md) para documentacion detallada de las herramientas.

## Modelo de Seguridad

CogniCode implementa varias medidas de seguridad:

### Validacion de Entrada

- **Prevencion de path traversal**: Bloquea patrones `..`, `~/`, `$`, `${`
- **Aplicacion de limites del espacio de trabajo**: Los archivos deben estar dentro del espacio de trabajo configurado
- **Limites de longitud de consulta**: Previene DoS via consultas demasiado grandes
- **Limites de tamano de resultados**: Limita el maximo de resultados devueltos

### Limitacion de Tasa

- Algoritmo de token bucket: 100 solicitudes por minuto (configurable)
- Validacion por solicitud de todas las entradas

### Limites de Tamano de Archivo

- Maximo predeterminado: 10MB por archivo
- Configurable via entorno

## Solucion de Problemas

### Problemas de Conexion

**Problema**: El agente reporta "connection refused" o timeout

**Soluciones**:
1. Verifica que la ruta del binario es correcta y ejecutable
2. Comprueba que el binario `cognicode-mcp` existe en la ruta especificada
3. Intenta ejecutar el binario manualmente para ver mensajes de error:
   ```bash
   ./target/release/cognicode-mcp --verbose
   ```

### Errores de Seguridad

**Problema**: Errores de "Path traversal attempt"

**Soluciones**:
1. Usa rutas relativas dentro del espacio de trabajo
2. Asegurate de que `COGNICODE_WORKSPACE` esta configurado correctamente
3. Evita rutas absolutas a menos que el espacio de trabajo incluya `/`

**Problema**: "Rate limit exceeded"

**Solucion**: Espera 60 segundos o aumenta `COGNICODE_RATE_LIMIT`

### Problemas de Analisis

**Problema**: Resultados vacios para simbolos validos

**Soluciones**:
1. Verifica que el nombre del simbolo esta completamente cualificado (ej. `module::function`)
2. Comprueba que el archivo esta en el espacio de trabajo
3. Usa registro `--verbose` para ver detalles del analisis

## Configuraciones Ejemplo de Claude Desktop

### Configuracion Basica

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "/usr/local/bin/cognicode-mcp"
    }
  }
}
```

### Con Espacio de Trabajo Personalizado

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "/usr/local/bin/cognicode-mcp",
      "env": {
        "COGNICODE_WORKSPACE": "/Usuarios/me/Proyectos/miapp"
      }
    }
  }
}
```

### Con Registro Detallado

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "/usr/local/bin/cognicode-mcp",
      "env": {
        "RUST_LOG": "debug"
      }
    }
  }
}
```

### Configuracion Multi-Proyecto

Para agentes que trabajan en multiples proyectos:

```json
{
  "mcpServers": {
    "cognicode-backend": {
      "command": "/usr/local/bin/cognicode-mcp",
      "env": {
        "COGNICODE_WORKSPACE": "/Usuarios/me/Proyectos/backend"
      }
    },
    "cognicode-frontend": {
      "command": "/usr/local/bin/cognicode-mcp",
      "env": {
        "COGNICODE_WORKSPACE": "/Usuarios/me/Proyectos/frontend"
      }
    }
  }
}
```

## Probando la Conexion

Despues de la configuracion, reinicia Claude Desktop e intenta:

```
List the symbols in src/main.rs
```

O:

```
Find all usages of the function calculate_total
```

Si es exitoso, CogniCode devolvera la informacion solicitada.

## Recursos Adicionales

- [Documentacion de Arquitectura](architecture-es.md)
- [Vision Conceptual](concept-es.md)
- [Referencia de Herramientas MCP](mcp-tools-reference-es.md)
- [Referencia de CLI](cli-reference-es.md)
