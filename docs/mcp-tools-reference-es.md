# Referencia de Herramientas MCP

Referencia completa de las herramientas MCP (Model Context Protocol) de CogniCode.

## Tabla de Contenidos

1. [Resumen](#resumen)
2. [Lista de Herramientas](#lista-de-herramientas)
3. [Esquemas de Herramientas](#esquemas-de-herramientas)
4. [Ejemplos de Solicitudes](#ejemplos-de-solicitudes)
5. [Manejo de Errores](#manejo-de-errores)
6. [Seguridad](#seguridad)

---

## Resumen

CogniCode expone capacidades de analisis y refactorizacion de codigo a traves de herramientas MCP. Cada herramienta sigue el patron de solicitud/respuesta JSON-RPC 2.0.

### Transporte

- **Protocolo**: JSON-RPC 2.0 sobre stdio o TCP
- **Content-Type**: application/json

### Formato de Solicitud

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "tool_name",
    "arguments": {
      "param1": "value1"
    }
  },
  "id": 1
}
```

### Formato de Respuesta (Exito)

```json
{
  "jsonrpc": "2.0",
  "result": {
    "tool_specific_result": "..."
  },
  "id": 1
}
```

### Formato de Respuesta (Error)

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Error description"
  },
  "id": 1
}
```

---

## Lista de Herramientas

| Herramienta | Proposito |
|-------------|-----------|
| `get_call_hierarchy` | Navegar grafos de llamadas (entrantes/salientes) |
| `get_file_symbols` | Extraer todos los simbolos de un archivo |
| `find_usages` | Encontrar todos los usos de un simbolo |
| `structural_search` | Buscar por patron AST |
| `analyze_impact` | Analizar el impacto del cambio |
| `check_architecture` | Detectar ciclos y violaciones |
| `safe_refactor` | Ejecutar refactorizacion validada |
| `validate_syntax` | Validacion rapida de sintaxis |
| `get_complexity` | Calcular metricas de complejidad |

---

## Esquemas de Herramientas

### get_call_hierarchy

Navegar el grafo de llamadas para un simbolo.

**Esquema de Entrada**:

```json
{
  "symbol_name": "string",      // Requerido: nombre completamente calificado (ej., "module::function")
  "direction": "incoming|outgoing",  // Requerido: "incoming"=quien llama a esto, "outgoing"=que llama esto
  "depth": 1,                   // Opcional: profundidad de recorrido (predeterminado: 1, max: 10)
  "include_external": false      // Opcional: incluir deps externas (predeterminado: false)
}
```

**Esquema de Salida**:

```json
{
  "symbol": "string",
  "calls": [
    {
      "symbol": "string",
      "file": "string",
      "line": 42,
      "column": 5,
      "confidence": 1.0
    }
  ],
  "metadata": {
    "total_calls": 10,
    "analysis_time_ms": 15
  }
}
```

**Valores de Direccion**:

| Valor | Descripcion |
|-------|-------------|
| `incoming` | Encontrar todos los simbolos que llaman a este simbolo |
| `outgoing` | Encontrar todos los simbolos que este simbolo llama |

---

### get_file_symbols

Extraer todos los simbolos de un archivo.

**Esquema de Entrada**:

```json
{
  "file_path": "string"          // Requerido: ruta al archivo fuente
}
```

**Esquema de Salida**:

```json
{
  "file_path": "string",
  "symbols": [
    {
      "name": "string",
      "kind": "function|class|struct|...",
      "location": {
        "file": "string",
        "line": 42,
        "column": 5
      },
      "signature": "string|null"
    }
  ]
}
```

**Valores de SymbolKind**:

| Valor | Descripcion |
|-------|-------------|
| `module` | Definicion de modulo |
| `class` | Definicion de clase |
| `struct` | Definicion de estructura |
| `enum` | Definicion de enumeracion |
| `trait` | Definicion de trait |
| `function` | Definicion de funcion |
| `method` | Definicion de metodo |
| `field` | Definicion de campo |
| `variable` | Definicion de variable |
| `constant` | Definicion de constante |
| `constructor` | Definicion de constructor |
| `interface` | Definicion de interfaz |
| `type_alias` | Definicion de alias de tipo |
| `parameter` | Definicion de parametro |

---

### find_usages

Encontrar todos los usos de un simbolo.

**Esquema de Entrada**:

```json
{
  "symbol_name": "string",       // Requerido: simbolo a buscar
  "include_declaration": true     // Opcional: incluir definicion (predeterminado: true)
}
```

**Esquema de Salida**:

```json
{
  "symbol": "string",
  "usages": [
    {
      "file": "string",
      "line": 42,
      "column": 5,
      "context": "string",        // Contexto del codigo circundante
      "is_definition": false
    }
  ],
  "total": 10
}
```

---

### structural_search

Buscar patrones de codigo usando coincidencia basada en AST.

**Esquema de Entrada**:

```json
{
  "pattern_type": "function_call|type_definition|import_statement|annotation|custom",
  "query": "string",              // Requerido: consulta de busqueda
  "path": "string|null",          // Opcional: ruta de archivo/directorio
  "depth": 1                       // Opcional: profundidad de busqueda (predeterminado: 1)
}
```

**Esquema de Salida**:

```json
{
  "pattern": "string",
  "matches": [
    {
      "file": "string",
      "line": 42,
      "column": 5,
      "matched_text": "string",
      "context": "string"
    }
  ],
  "total": 5
}
```

**Valores de PatternType**:

| Valor | Descripcion |
|-------|-------------|
| `function_call` | Coincidir invocaciones de funciones |
| `type_definition` | Coincidir definiciones de tipo/clase |
| `import_statement` | Coincidir declaraciones de import/require |
| `annotation` | Coincidir anotaciones/decoradores |
| `custom` | Coincidencia de patrones personalizada |

---

### analyze_impact

Analizar el impacto de cambiar un simbolo.

**Esquema de Entrada**:

```json
{
  "symbol_name": "string"         // Requerido: simbolo a analizar
}
```

**Esquema de Salida**:

```json
{
  "symbol": "string",
  "impacted_files": ["string"],
  "impacted_symbols": ["string"],
  "risk_level": "low|medium|high|critical",
  "summary": "string"
}
```

**Valores de RiskLevel**:

| Valor | Descripcion |
|-------|-------------|
| `low` | El cambio tiene efectos en cascada minimos |
| `medium` | El cambio afecta varios simbolos |
| `high` | El cambio tiene impacto significativo |
| `critical` | El cambio puede romper multiples sistemas |

---

### check_architecture

Verificar la salud arquitectonica y detectar violaciones.

**Esquema de Entrada**:

```json
{
  "scope": "string|null"          // Opcional: alcance especifico a verificar
}
```

**Esquema de Salida**:

```json
{
  "cycles": [
    {
      "symbols": ["string"],
      "length": 3
    }
  ],
  "violations": [
    {
      "rule": "string",
      "from": "string",
      "to": "string",
      "severity": "string"
    }
  ],
  "score": 85.5,
  "summary": "string"
}
```

---

### safe_refactor

Realizar una operacion de refactorizacion con validacion.

**Esquema de Entrada**:

```json
{
  "action": "rename|extract|inline|move|change_signature",
  "target": "string",             // Requerido: simbolo objetivo
  "params": {}                     // Opcional: parametros especificos de la accion
}
```

**Esquema de Salida**:

```json
{
  "action": "string",
  "success": true,
  "changes": [
    {
      "file": "string",
      "old_text": "string",
      "new_text": "string",
      "location": {
        "file": "string",
        "line": 42,
        "column": 5
      }
    }
  ],
  "validation_result": {
    "is_valid": true,
    "warnings": ["string"],
    "errors": []
  },
  "error_message": null
}
```

**Valores de RefactorAction**:

| Valor | Descripcion |
|-------|-------------|
| `rename` | Renombrar un simbolo |
| `extract` | Extraer codigo en una funcion |
| `inline` | Inline de una funcion |
| `move` | Mover simbolo a otra ubicacion |
| `change_signature` | Modificar parametros de funcion |

---

### validate_syntax

Validar la sintaxis de un archivo fuente.

**Esquema de Entrada**:

```json
{
  "file_path": "string"           // Requerido: archivo a validar
}
```

**Esquema de Salida**:

```json
{
  "file_path": "string",
  "is_valid": true,
  "errors": [
    {
      "line": 42,
      "column": 5,
      "message": "string",
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "line": 42,
      "column": 5,
      "message": "string",
      "severity": "warning"
    }
  ]
}
```

---

### get_complexity

Calcular metricas de complejidad para el codigo.

**Esquema de Entrada**:

```json
{
  "file_path": "string",           // Requerido: archivo a analizar
  "function_name": "string|null"   // Opcional: funcion especifica
}
```

**Esquema de Salida**:

```json
{
  "file_path": "string",
  "complexity": {
    "cyclomatic": 5,
    "cognitive": 3,
    "lines_of_code": 150,
    "parameter_count": 3,
    "nesting_depth": 4,
    "function_name": "string|null"
  }
}
```

---

## Ejemplos de Solicitudes

### Ejemplo 1: Obtener Jerarquia de Llamadas

**Solicitud**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_call_hierarchy",
    "arguments": {
      "symbol_name": "order::process_order",
      "direction": "outgoing",
      "depth": 2
    }
  },
  "id": 1
}
```

**Respuesta**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "symbol": "order::process_order",
    "calls": [
      {
        "symbol": "order::validate_order",
        "file": "src/order.rs",
        "line": 25,
        "column": 1,
        "confidence": 1.0
      },
      {
        "symbol": "inventory::check_stock",
        "file": "src/order.rs",
        "line": 26,
        "column": 1,
        "confidence": 1.0
      }
    ],
    "metadata": {
      "total_calls": 2,
      "analysis_time_ms": 12
    }
  },
  "id": 1
}
```

### Ejemplo 2: Encontrar Simbolos de Archivo

**Solicitud**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_file_symbols",
    "arguments": {
      "file_path": "src/main.rs"
    }
  },
  "id": 2
}
```

**Respuesta**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "file_path": "src/main.rs",
    "symbols": [
      {
        "name": "main",
        "kind": "function",
        "location": {
          "file": "src/main.rs",
          "line": 1,
          "column": 1
        },
        "signature": "fn main()"
      },
      {
        "name": "Config",
        "kind": "struct",
        "location": {
          "file": "src/main.rs",
          "line": 10,
          "column": 1
        },
        "signature": null
      }
    ]
  },
  "id": 2
}
```

### Ejemplo 3: Analizar Impacto

**Solicitud**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "analyze_impact",
    "arguments": {
      "symbol_name": "order::calculate_total"
    }
  },
  "id": 3
}
```

**Respuesta**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "symbol": "order::calculate_total",
    "impacted_files": [
      "src/order.rs",
      "src/checkout.rs",
      "tests/order_test.rs"
    ],
    "impacted_symbols": [
      "checkout::finalize",
      "order::apply_discount",
      "report::generate_summary"
    ],
    "risk_level": "high",
    "summary": "Impact analysis completed in 8ms - 3 files affected, 12 symbols depend on this function"
  },
  "id": 3
}
```

### Ejemplo 4: Refactorizacion Segura

**Solicitud**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "safe_refactor",
    "arguments": {
      "action": "rename",
      "target": "order::process_order",
      "params": {
        "new_name": "order::handle_order"
      }
    }
  },
  "id": 4
}
```

**Respuesta**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "action": "rename",
    "success": true,
    "changes": [
      {
        "file": "src/order.rs",
        "old_text": "process_order",
        "new_text": "handle_order",
        "location": {
          "file": "src/order.rs",
          "line": 42,
          "column": 5
        }
      }
    ],
    "validation_result": {
      "is_valid": true,
      "warnings": ["Consider updating related documentation"],
      "errors": []
    },
    "error_message": null
  },
  "id": 4
}
```

### Ejemplo 5: Obtener Complejidad

**Solicitud**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_complexity",
    "arguments": {
      "file_path": "src/order.rs",
      "function_name": "calculate_total"
    }
  },
  "id": 5
}
```

**Respuesta**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "file_path": "src/order.rs",
    "complexity": {
      "cyclomatic": 8,
      "cognitive": 5,
      "lines_of_code": 45,
      "parameter_count": 2,
      "nesting_depth": 4,
      "function_name": "calculate_total"
    }
  },
  "id": 5
}
```

---

## Manejo de Errores

### Formato de Respuesta de Error

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Human-readable error message",
    "data": {}
  },
  "id": 1
}
```

### Codigos de Error

| Codigo | Nombre | Descripcion |
|--------|--------|-------------|
| -32600 | InvalidRequest | Solicitud JSON-RPC mal formada |
| -32601 | MethodNotFound | Nombre de herramienta desconocido |
| -32602 | InvalidParams | Parametros de herramienta invalidos |
| -32603 | InternalError | Error del lado del servidor |
| -32000 | SecurityError | Validacion de entrada fallida |
| -32001 | AppError | Error de logica de aplicacion |
| -32002 | InvalidInput | Datos de entrada invalidos |
| -32003 | NotFound | Recurso solicitado no encontrado |

### Errores Comunes

**Intento de Recorrido de Ruta**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Security error: Path traversal attempt detected: '../etc/passwd'",
    "data": null
  },
  "id": 1
}
```

**Limite de Tasa Excedido**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Security error: Rate limit exceeded",
    "data": null
  },
  "id": 1
}
```

**Simbolo No Encontrado**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32003,
    "message": "Not found: Symbol 'nonexistent::function' not found in workspace",
    "data": null
  },
  "id": 1
}
```

**Parametros Invalidos**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32602,
    "message": "Invalid params: 'direction' must be 'incoming' or 'outgoing'",
    "data": null
  },
  "id": 1
}
```

---

## Seguridad

Todas las herramientas MCP estan protegidas por el `InputValidator` que enforces:

### Validacion de Rutas

- Previene recorrido de rutas (`..`, `~/`, `$`)
- Valida que las rutas esten dentro del espacio de trabajo
- Verifica bytes nulos y caracteres invalidos
- Limita la profundidad de los componentes de la ruta

### Limites de Tamanio

| Limite | Predeterminado | Descripcion |
|--------|----------------|-------------|
| `max_file_size` | 10MB | Tamanio maximo del contenido del archivo |
| `max_query_length` | 1000 | Longitud maxima de la cadena de consulta |
| `max_results` | 10000 | Maximo de resultados por consulta |

### Limitacion de Tasa

- Algoritmo de cubo de tokens
- Predeterminado: 100 solicitudes por minuto
- Configurable a traves de variables de entorno

### Mejores Practicas para Clientes

1. **Siempre validar respuestas**: Verificar campos `is_valid`
2. **Manejar errores con gracia**: Implementar reintento con retroceso
3. **Limitar tamanos de resultados**: Usar paginacion para conjuntos grandes de resultados
4. **Sanitizar entradas**: Escapar caracteres especiales en consultas

---

## Recursos Adicionales

- [Guia de Configuracion del Agente](agent-setup-es.md)
- [Resumen Conceptual](concept-es.md)
- [Documentacion de Arquitectura](architecture-es.md)