# Ejemplos de Prompts para Agentes de IA — CogniCode MCP

Este documento contiene ejemplos de prompts listos para usar por agentes de IA
que integran CogniCode vía MCP. Los prompts están organizados por escenario
— lo que el agente intenta lograr — en lugar de por herramienta.

Cada ejemplo incluye:
- El **prompt en lenguaje natural** que el agente recibe del usuario
- La **cadena de razonamiento** que el agente debe seguir
- Las **llamadas a herramientas MCP** a realizar, en orden
- Una **interpretación de ejemplo** de los resultados

> **Requisitos previos:** El servidor MCP debe estar en ejecución y `build_graph`
> debe haberse llamado al menos una vez antes de que la mayoría de las
> herramientas de análisis devuelvan resultados significativos.

---

## Tabla de Contenidos

1. [Explorando un Repositorio Nuevo](#1-explorando-un-repositorio-nuevo)
2. [Entender una Funcionalidad Antes de Modificarla](#2-entender-una-funcionalidad-antes-de-modificarla)
3. [Refactoring Seguro con Rename](#3-refactoring-seguro-con-rename)
4. [Revisión de Salud Arquitectónica](#4-revisión-de-salud-arquitectónica)
5. [Encontrar Quién Llama a una Función](#5-encontrar-quién-llama-a-una-función)
6. [Análisis de Impacto Antes de un PR](#6-análisis-de-impacto-antes-de-un-pr)
7. [Navegar a una Definición](#7-navegar-a-una-definición)
8. [Detectar Código Muerto](#8-detectar-código-muerto)
9. [Medir la Complejidad del Código](#9-medir-la-complejidad-del-código)
10. [Extraer una Función de Forma Segura](#10-extraer-una-función-de-forma-segura)
11. [Mover un Símbolo a Otro Archivo](#11-mover-un-símbolo-a-otro-archivo)
12. [Búsqueda Semántica de Símbolos](#12-búsqueda-semántica-de-símbolos)
13. [Generar un Diagrama del Grafo de Llamadas](#13-generar-un-diagrama-del-grafo-de-llamadas)
14. [Entender un Archivo de un Vistazo](#14-entender-un-archivo-de-un-vistazo)
15. [Validar Sintaxis Antes de un Commit](#15-validar-sintaxis-antes-de-un-commit)
16. [Encontrar las Funciones Más Críticas](#16-encontrar-las-funciones-más-críticas)
17. [Trazar una Ruta de Ejecución](#17-trazar-una-ruta-de-ejecución)
18. [Buscar un Patrón en el Código](#18-buscar-un-patrón-en-el-código)
19. [Revisar la Implementación Completa de un Símbolo](#19-revisar-la-implementación-completa-de-un-símbolo)
20. [Re-análisis Incremental Tras Cambios](#20-re-análisis-incremental-tras-cambios)

---

## 1. Explorando un Repositorio Nuevo

**Prompt del usuario:**
> "Acabo de clonar este repo. Ayúdame a entender qué hace, cuáles son los
> puntos de entrada principales y qué funciones se llaman más."

**Razonamiento del agente:**
Construir primero el grafo completo, luego obtener los entry points (superficie
pública de la API), las leaf functions (primitivos de bajo nivel) y los hot
paths (código más interconectado). Los tres juntos ofrecen una visión 360° de
cualquier base de código desconocida.

**Llamadas a herramientas:**

```
1. build_graph
   directory: "/ruta/al/proyecto"
   strategy: "full"
```

```
2. get_entry_points
   compressed: true
```

```
3. get_leaf_functions
   compressed: true
```

```
4. get_hot_paths
   limit: 10
   min_fan_in: 3
```

**Cómo interpretar los resultados:**

- **Entry points** → superficie pública; son las funciones que llaman usuarios
  u otros sistemas. Empieza a leer por aquí.
- **Leaf functions** → primitivos; código de utilidad puro sin dependencias
  adicionales. Generalmente se pueden ignorar al principio.
- **Hot paths** → funciones llamadas desde muchos puntos. Los cambios aquí se
  propagan ampliamente. Presta especial atención a estas.

---

## 2. Entender una Funcionalidad Antes de Modificarla

**Prompt del usuario:**
> "Necesito modificar la función `process_payment`. Antes de tocar nada,
> muéstrame todo lo que la llama y todo lo que ella llama."

**Razonamiento del agente:**
Obtener tanto la jerarquía de llamadas entrantes (quién depende de ella) como
la saliente (de qué depende ella). Luego obtener el código fuente real de la
función para leerla en contexto.

**Llamadas a herramientas:**

```
1. get_call_hierarchy
   symbol_name: "process_payment"
   direction: "incoming"
   depth: 3
```

```
2. get_call_hierarchy
   symbol_name: "process_payment"
   direction: "outgoing"
   depth: 2
```

```
3. find_usages
   symbol_name: "process_payment"
   include_declaration: true
```

```
4. get_symbol_code
   file: "src/payments/processor.rs"
   line: 42
   col: 0
```

**Cómo interpretar los resultados:**

- Si `incoming` a profundidad 1 muestra 8 o más callers, esta es una función de
  alto riesgo — considera el enfoque de extracción en su lugar.
- Si `outgoing` muestra llamadas a funciones de I/O, red o base de datos, cambiar
  la firma requerirá actualizar los mocks en los tests.
- `find_usages` revela los sitios de llamada en los tests — ahí está el contrato
  esperado.

---

## 3. Refactoring Seguro con Rename

**Prompt del usuario:**
> "Renombra `calc_total` a `calculate_order_total` en todo el proyecto.
> Asegúrate de que nada se rompa."

**Razonamiento del agente:**
Primero analizar el impacto para entender el alcance, luego previsualizar el
refactoring antes de aplicarlo. Nunca renombrar a ciegas.

**Llamadas a herramientas:**

```
1. analyze_impact
   symbol_name: "calc_total"
```

```
2. find_usages
   symbol_name: "calc_total"
   include_declaration: true
```

```
3. safe_refactor
   action: "rename"
   symbol_name: "calc_total"
   new_name: "calculate_order_total"
   file_path: "src/order/calculator.rs"
   line: 15
   column: 7
   preview: true
```

```
4. safe_refactor
   action: "rename"
   symbol_name: "calc_total"
   new_name: "calculate_order_total"
   file_path: "src/order/calculator.rs"
   line: 15
   column: 7
   preview: false
```

**Cómo interpretar los resultados:**

- Paso 1: Si `risk_level` es `High` o `Critical` (más de 5–10 sitios de llamada),
  avisar al usuario antes de continuar.
- Paso 3 (preview): Leer la lista `affected_files` y mostrarla al usuario para
  confirmación.
- Paso 4: Solo ejecutar tras la confirmación del usuario de que la preview es
  correcta.

---

## 4. Revisión de Salud Arquitectónica

**Prompt del usuario:**
> "¿Hay alguna dependencia circular en esta base de código? Dame una puntuación
> de salud arquitectónica."

**Razonamiento del agente:**
Ejecutar `check_architecture`, que usa el algoritmo de Tarjan SCC para detectar
ciclos. La puntuación empieza en 100 y baja 5 puntos por cada símbolo involucrado
en un ciclo.

**Llamadas a herramientas:**

```
1. build_graph
   directory: "/ruta/al/proyecto"
   strategy: "full"
```

```
2. check_architecture
```

**Cómo interpretar los resultados:**

| Puntuación | Significado |
|------------|-------------|
| 100 | Sin ciclos. Arquitectura limpia. |
| 80–99 | Ciclos menores, vale la pena resolverlos. |
| 50–79 | Problemas de acoplamiento significativos. |
| < 50 | La arquitectura necesita refactoring. |

- Cada `cycle` en el resultado lista los símbolos que forman el bucle.
- Para cada ciclo: usar `trace_path` entre los dos extremos para entender
  por qué existe la dependencia.
- Corrección recomendada: introducir un trait o una abstracción compartida para
  romper el ciclo.

---

## 5. Encontrar Quién Llama a una Función

**Prompt del usuario:**
> "¿Quién llama a `authenticate_user`? Quiero saber todos los callers y si
> alguno está en la capa de API pública."

**Razonamiento del agente:**
Obtener la jerarquía de llamadas entrantes con suficiente profundidad para
llegar a los callers de nivel superior. Luego cruzar con los entry points para
ver cuáles están en la superficie pública.

**Llamadas a herramientas:**

```
1. get_call_hierarchy
   symbol_name: "authenticate_user"
   direction: "incoming"
   depth: 5
   compressed: false
```

```
2. get_entry_points
   compressed: false
```

**Cómo interpretar los resultados:**

- Intersectar los callers del paso 1 con los entry points del paso 2.
- Cualquier caller que también sea un entry point significa que `authenticate_user`
  es alcanzable desde la API pública — los cambios deben ser retrocompatibles.
- Si no se encuentran callers a profundidad 1, la función podría ser código muerto.

---

## 6. Análisis de Impacto Antes de un PR

**Prompt del usuario:**
> "Voy a cambiar la firma de `UserRepository::find_by_email`. ¿Cuál es el
> radio de explosión? ¿Qué archivos tendré que actualizar?"

**Razonamiento del agente:**
`analyze_impact` devuelve una lista categorizada por riesgo de todos los símbolos
que dependen transitivamente del objetivo. Esta es la herramienta más importante
a ejecutar antes de cualquier cambio no trivial.

**Llamadas a herramientas:**

```
1. analyze_impact
   symbol_name: "UserRepository::find_by_email"
```

```
2. get_call_hierarchy
   symbol_name: "UserRepository::find_by_email"
   direction: "incoming"
   depth: 4
```

**Cómo interpretar los resultados:**

- Valores de `risk_level`: `Low` (≤2 símbolos), `Medium` (≤5), `High` (≤10),
  `Critical` (>10).
- `impacted_files` en el resultado da la lista exacta de archivos a actualizar.
- Si `risk_level` es `Critical`, sugerir al usuario introducir un shim de
  compatibilidad en lugar de un cambio directo de firma.

---

## 7. Navegar a una Definición

**Prompt del usuario:**
> "¿Dónde está definido `PaymentGateway`? Llévame al fuente."

**Razonamiento del agente:**
Usar `go_to_definition` con soporte LSP. Si el usuario indica una ubicación en
un archivo, usarla directamente. De lo contrario, usar `query_symbol_index`
para encontrar primero un sitio de uso.

**Llamadas a herramientas:**

```
1. query_symbol_index
   symbol_name: "PaymentGateway"
   directory: "/ruta/al/proyecto"
```

```
2. go_to_definition
   file_path: "src/payments/gateway.rs"
   line: 8
   column: 4
```

**Cómo interpretar los resultados:**

- El paso 1 devuelve un archivo y línea donde aparece `PaymentGateway`.
- El paso 2 salta a su definición real (que puede estar en un archivo diferente
  si proviene de un crate importado).
- Si `go_to_definition` no devuelve nada, el símbolo es de una dependencia
  externa — sugerir usar `cargo doc --open`.

---

## 8. Detectar Código Muerto

**Prompt del usuario:**
> "¿Hay funciones en este proyecto que nadie llama? Quiero limpiar código
> sin usar."

**Razonamiento del agente:**
Las leaf functions con cero callers son candidatas a código muerto. Cruzar
con entry points (que no tienen callers por diseño, pero no son código muerto).

**Llamadas a herramientas:**

```
1. build_graph
   directory: "/ruta/al/proyecto"
   strategy: "full"
```

```
2. get_leaf_functions
   compressed: false
```

```
3. get_entry_points
   compressed: false
```

**Lógica de post-procesado del agente:**

```
candidatos_codigo_muerto = leaf_functions - entry_points

Para cada candidato:
  - Si es una función de test (#[test], test_*) → omitir
  - Si es una implementación de trait → verificar con get_call_hierarchy incoming
  - Si get_call_hierarchy incoming devuelve vacío → código muerto confirmado
```

```
4. get_call_hierarchy
   symbol_name: "<función_candidata>"
   direction: "incoming"
   depth: 1
```

**Cómo interpretar los resultados:**

- Las funciones con 0 callers entrantes que no son entry points, no son tests
  y no son implementaciones de trait son seguras para eliminar.
- Confirmar siempre con el usuario antes de borrar — algunas funciones pueden
  llamarse por reflexión, FFI o dispatch dinámico.

---

## 9. Medir la Complejidad del Código

**Prompt del usuario:**
> "¿Qué funciones de `src/billing/invoicer.rs` son las más complejas?
> Quiero priorizar qué refactorizar."

**Razonamiento del agente:**
Obtener métricas de complejidad para el archivo completo, luego ordenar por
complejidad ciclomática. Las funciones por encima de 10 son prioridad de
refactoring.

**Llamadas a herramientas:**

```
1. get_complexity
   file_path: "src/billing/invoicer.rs"
```

**Cómo interpretar los resultados:**

| Complejidad ciclomática | Riesgo |
|------------------------|--------|
| 1–5 | Baja. Simple y fácil de testear. |
| 6–10 | Moderada. Considera simplificar. |
| 11–20 | Alta. Difícil de testear completamente. Refactorizar. |
| > 20 | Crítica. Debe descomponerse. |

- Ordenar `functions` por `cyclomatic` descendente.
- Para cada función por encima del umbral, llamar a `get_symbol_code` para
  leerla y sugerir oportunidades de extracción.

```
2. get_symbol_code
   file: "src/billing/invoicer.rs"
   line: <línea_de_función_compleja>
   col: 0
```

---

## 10. Extraer una Función de Forma Segura

**Prompt del usuario:**
> "La función `generate_invoice` tiene 200 líneas. Extrae la parte del cálculo
> de impuestos a una función separada llamada `calculate_tax`."

**Razonamiento del agente:**
Leer la función primero para identificar el rango de extracción. Previsualizar
la extracción y luego aplicarla.

**Llamadas a herramientas:**

```
1. get_symbol_code
   file: "src/billing/invoicer.rs"
   line: 88
   col: 0
```

```
2. get_complexity
   file_path: "src/billing/invoicer.rs"
   function_name: "generate_invoice"
```

```
3. safe_refactor
   action: "extract"
   symbol_name: "generate_invoice"
   new_name: "calculate_tax"
   file_path: "src/billing/invoicer.rs"
   line: 88
   column: 0
   preview: true
```

```
4. safe_refactor
   action: "extract"
   symbol_name: "generate_invoice"
   new_name: "calculate_tax"
   file_path: "src/billing/invoicer.rs"
   line: 88
   column: 0
   preview: false
```

**Cómo interpretar los resultados:**

- La preview del paso 3 muestra la nueva firma de la función y dónde se insertará
  el sitio de llamada.
- Tras aplicar, ejecutar `validate_syntax` en el archivo para confirmar que el
  resultado es sintácticamente válido.

```
5. validate_syntax
   file_path: "src/billing/invoicer.rs"
```

---

## 11. Mover un Símbolo a Otro Archivo

**Prompt del usuario:**
> "Mueve el struct `EmailFormatter` de `utils.rs` a un nuevo archivo dedicado
> `email.rs`."

**Razonamiento del agente:**
Verificar qué depende de `EmailFormatter` primero. Luego previsualizar el
movimiento para ver todos los imports `use` que se actualizarán.

**Llamadas a herramientas:**

```
1. analyze_impact
   symbol_name: "EmailFormatter"
```

```
2. safe_refactor
   action: "move"
   symbol_name: "EmailFormatter"
   source_path: "src/utils.rs"
   target_path: "src/email.rs"
   preview: true
```

```
3. safe_refactor
   action: "move"
   symbol_name: "EmailFormatter"
   source_path: "src/utils.rs"
   target_path: "src/email.rs"
   preview: false
```

**Cómo interpretar los resultados:**

- La preview lista cada archivo donde `use crate::utils::EmailFormatter` debe
  cambiar a `use crate::email::EmailFormatter`.
- Si el archivo destino no existe, la herramienta lo creará.
- Tras el movimiento, confirmar con `find_usages` que todas las referencias
  se resuelven correctamente.

---

## 12. Búsqueda Semántica de Símbolos

**Prompt del usuario:**
> "Encuentra todos los structs y traits relacionados con repositorios en este
> proyecto."

**Razonamiento del agente:**
Usar `semantic_search` con una consulta fuzzy y filtrar por tipo. Intentar
múltiples consultas si la primera no cubre todos los resultados.

**Llamadas a herramientas:**

```
1. semantic_search
   query: "repository"
   kinds: ["struct", "trait"]
   max_results: 30
```

```
2. semantic_search
   query: "repo"
   kinds: ["struct", "trait", "impl"]
   max_results: 20
```

**Cómo interpretar los resultados:**

- Los resultados incluyen ruta del archivo, línea y tipo para cada coincidencia.
- Deduplicar por nombre de símbolo entre ambas consultas.
- Para cada trait encontrado, llamar a `get_call_hierarchy outgoing` para ver
  qué operaciones concretas define.

---

## 13. Generar un Diagrama del Grafo de Llamadas

**Prompt del usuario:**
> "Dame un diagrama visual de todo lo que llama a `OrderService`."

**Razonamiento del agente:**
Construir un subgrafo enfocado alrededor de `OrderService`, luego exportarlo
como Mermaid. El subgrafo es más legible que el grafo completo del proyecto.

**Llamadas a herramientas:**

```
1. build_call_subgraph
   symbol_name: "OrderService"
   direction: "both"
   depth: 2
   directory: "/ruta/al/proyecto"
```

```
2. export_mermaid
   root_symbol: "OrderService"
   max_depth: 2
   format: "code"
   include_external: false
```

**Cómo interpretar los resultados:**

- Pega el código Mermaid en cualquier renderizador (GitHub markdown,
  mermaid.live, extensión de VS Code) para visualizarlo.
- Usa `format: "svg"` para obtener una imagen renderizada directamente.
- Si el diagrama es demasiado grande (>30 nodos), reducir `max_depth` a 1.

---

## 14. Entender un Archivo de un Vistazo

**Prompt del usuario:**
> "Dame un resumen rápido de qué contiene `src/api/handlers.rs`."

**Razonamiento del agente:**
Usar `get_outline` para la estructura y `get_file_symbols` con `compressed: true`
para un resumen en lenguaje natural. Ambos juntos dan una imagen completa sin
necesidad de leer el fuente en bruto.

**Llamadas a herramientas:**

```
1. get_outline
   file_path: "src/api/handlers.rs"
   include_private: false
   include_tests: false
```

```
2. get_file_symbols
   file_path: "src/api/handlers.rs"
   compressed: true
```

**Cómo interpretar los resultados:**

- `get_outline` muestra el árbol de símbolos — qué funciones están dentro de
  qué structs o módulos.
- `get_file_symbols` comprimido devuelve una descripción en prosa adecuada para
  presentar al usuario sin abrumarlo con JSON.

---

## 15. Validar Sintaxis Antes de un Commit

**Prompt del usuario:**
> "Acabo de editar tres archivos. Asegúrate de que todos se parsean
> correctamente antes de hacer commit."

**Razonamiento del agente:**
Ejecutar `validate_syntax` en cada archivo modificado. Más barato que una
compilación completa y detecta errores estructurales de inmediato.

**Llamadas a herramientas:**

```
1. validate_syntax
   file_path: "src/domain/order.rs"
```

```
2. validate_syntax
   file_path: "src/application/order_service.rs"
```

```
3. validate_syntax
   file_path: "src/interface/api/order_handler.rs"
```

**Cómo interpretar los resultados:**

- `valid: true` → el archivo se parsea correctamente.
- `valid: false` → el array `errors` contiene la línea, columna y descripción
  de cada error de sintaxis.
- Mostrar los errores al usuario con la ubicación exacta antes de que haga commit.

---

## 16. Encontrar las Funciones Más Críticas

**Prompt del usuario:**
> "¿Qué funciones nunca debería romper? Quiero saber el código más dependido
> del proyecto."

**Razonamiento del agente:**
Los hot paths por fan-in son las funciones con más callers — romperlas tiene
el mayor radio de explosión. Combinar con análisis de impacto en las principales.

**Llamadas a herramientas:**

```
1. get_hot_paths
   limit: 15
   min_fan_in: 2
```

```
2. analyze_impact
   symbol_name: "<función_top_del_paso_1>"
```

**Cómo interpretar los resultados:**

- Ordenar los resultados por `fan_in` descendente.
- Las 3–5 funciones principales son las más críticas — añadirlas a una lista de
  símbolos "nunca romper".
- Para cada función top, verificar que tiene cobertura de test buscando su nombre
  con `find_usages` y comprobando callers con prefijo `test_`.

---

## 17. Trazar una Ruta de Ejecución

**Prompt del usuario:**
> "¿Cómo llega el código desde `main` hasta `send_notification`? Muéstrame
> la ruta de ejecución."

**Razonamiento del agente:**
`trace_path` usa BFS para encontrar la cadena de llamadas más corta entre
dos símbolos. Es invaluable para entender cómo una acción del usuario
desencadena comportamiento profundo.

**Llamadas a herramientas:**

```
1. trace_path
   source: "main"
   target: "send_notification"
   max_depth: 15
```

**Cómo interpretar los resultados:**

- El resultado es una lista ordenada de nombres de funciones que forman la
  cadena de llamadas.
- Si no se encuentra ninguna ruta, las funciones están en componentes
  desconectados, o `send_notification` se llama indirectamente (vía objeto
  trait, callback o tarea async).
- En ese caso, usar `find_usages` para encontrar quién llama a
  `send_notification` directamente, luego trazar desde `main` hasta ese
  caller intermedio.

---

## 18. Buscar un Patrón en el Código

**Prompt del usuario:**
> "Encuentra todos los lugares en el código donde llamamos a `unwrap()` sobre
> un Result. Quiero auditar el manejo de errores."

**Razonamiento del agente:**
`structural_search` hace coincidencia de patrones con conciencia del AST, que
es más preciso que la búsqueda de texto para construcciones de código.
Usar `search_content` como fallback para patrones de texto más simples.

**Llamadas a herramientas:**

```
1. search_content
   pattern: "\\.unwrap()"
   path: "/ruta/al/proyecto"
   case_sensitive: true
```

```
2. structural_search
   pattern: "?.unwrap()"
   directory: "/ruta/al/proyecto"
```

**Cómo interpretar los resultados:**

- Cada resultado incluye la ruta del archivo, línea y el contenido de la línea
  que coincide.
- Agrupar los resultados por archivo.
- Marcar cualquier `unwrap()` que no esté dentro de una función `#[test]` ni
  sobre un valor probablemente `Some`/`Ok` — estos son posibles panics en
  producción.

---

## 19. Revisar la Implementación Completa de un Símbolo

**Prompt del usuario:**
> "Muéstrame la implementación completa de `TokenValidator::validate`, incluida
> su documentación."

**Razonamiento del agente:**
Usar `query_symbol_index` para localizar el símbolo, luego `get_symbol_code`
para recuperar su fuente completo con docstrings.

**Llamadas a herramientas:**

```
1. query_symbol_index
   symbol_name: "TokenValidator"
   directory: "/ruta/al/proyecto"
```

```
2. get_symbol_code
   file: "src/auth/token_validator.rs"
   line: 34
   col: 4
```

```
3. hover
   file_path: "src/auth/token_validator.rs"
   line: 34
   column: 4
```

**Cómo interpretar los resultados:**

- `get_symbol_code` devuelve el fuente en bruto entre las llaves de inicio y
  fin de la función, incluyendo los comentarios `///` de documentación.
- `hover` (LSP) devuelve la firma de tipo y la documentación renderizada tal
  como la ve el language server.
- Juntos ofrecen la imagen completa: implementación + documentación.

---

## 20. Re-análisis Incremental Tras Cambios

**Prompt del usuario:**
> "Acabo de modificar varios archivos. Actualiza el análisis sin reconstruir
> todo desde cero."

**Razonamiento del agente:**
Usar `build_graph` con `strategy: "lightweight"` para una re-indexación rápida
que actualiza la tabla de símbolos sin el coste de la computación completa de
aristas. Luego lanzar una reconstrucción completa solo si el usuario necesita
análisis de impacto o hot paths.

**Llamadas a herramientas:**

```
1. build_lightweight_index
   directory: "/ruta/al/proyecto"
   strategy: "lightweight"
```

```
2. query_symbol_index
   symbol_name: "<símbolo_modificado_recientemente>"
   directory: "/ruta/al/proyecto"
```

**Si se necesita un análisis más profundo tras el pase ligero:**

```
3. build_graph
   directory: "/ruta/al/proyecto"
   strategy: "full"
```

**Cómo interpretar los resultados:**

- El índice ligero es siempre rápido (segundos vs. decenas de segundos para
  el completo).
- Usarlo para preguntas rápidas de "¿existe este símbolo / dónde está ahora?"
  tras editar archivos.
- Solo lanzar `strategy: "full"` cuando el usuario pida explícitamente análisis
  de impacto, hot paths, revisión de arquitectura o traversal del grafo de
  llamadas.

---

## Combinando Herramientas: Flujo Completo de Refactoring

El siguiente es un ejemplo completo de cómo un agente debe encadenar herramientas
para una tarea de refactoring no trivial.

**Prompt del usuario:**
> "Quiero refactorizar `UserService` — está haciendo demasiado. Ayúdame a
> dividirlo de forma segura."

**Flujo de trabajo recomendado para el agente:**

```
Paso 1 — Entender el estado actual
  → get_file_symbols("src/services/user_service.rs", compressed=false)
  → get_outline("src/services/user_service.rs")
  → get_complexity("src/services/user_service.rs")

Paso 2 — Entender las dependencias
  → analyze_impact("UserService")
  → get_call_hierarchy("UserService", direction="incoming", depth=3)
  → get_call_hierarchy("UserService", direction="outgoing", depth=2)

Paso 3 — Identificar candidatos para extracción
  Ordenar métodos por complejidad ciclomática.
  Agrupar métodos por preocupación de dominio (auth vs. perfil vs. notificaciones).

Paso 4 — Planificar la división
  Presentar al usuario una propuesta de división:
    UserService → AuthService + UserProfileService + NotificationService

Paso 5 — Ejecutar las extracciones (una a la vez, con validación)
  Para cada método extraído:
    → safe_refactor(action="extract", preview=true)
    → [usuario confirma]
    → safe_refactor(action="extract", preview=false)
    → validate_syntax(file_path)

Paso 6 — Mover a nuevos archivos
  → safe_refactor(action="move", preview=true) para cada nuevo servicio
  → [usuario confirma]
  → safe_refactor(action="move", preview=false)

Paso 7 — Verificar el estado final
  → check_architecture()
  → find_usages("UserService")  ← debería mostrar menos usos directos
  → get_hot_paths()             ← confirmar que los nuevos servicios tienen
                                   un fan-in razonable
```

---

## Consejos para Agentes de IA

### Construye siempre el grafo primero

La mayoría de las herramientas requieren un grafo construido. Al inicio de
cualquier sesión, llamar a:

```
build_graph(directory=".", strategy="full")
```

Si el tiempo es una restricción, usa `strategy: "lightweight"` para operaciones
solo de símbolos y actualiza a `"full"` solo cuando sea necesario.

### Usa `compressed: true` para ahorrar contexto

Al explorar bases de código grandes, prefiere la salida comprimida:

```
get_file_symbols(file_path="...", compressed=true)
get_entry_points(compressed=true)
get_leaf_functions(compressed=true)
```

Esto devuelve resúmenes en prosa en lugar de JSON completo, preservando la
ventana de contexto.

### Secuencia para cambios seguros

1. `analyze_impact` → entender el alcance
2. `safe_refactor preview=true` → mostrar al usuario qué cambiará
3. Confirmación del usuario
4. `safe_refactor preview=false` → aplicar
5. `validate_syntax` → confirmar que el resultado es válido

### Umbrales de riesgo

| Nivel de riesgo | Acción recomendada |
|----------------|--------------------|
| `Low` | Aplicar directamente tras la preview |
| `Medium` | Mostrar la lista de impacto, pedir confirmación |
| `High` | Avisar al usuario, sugerir incrementos más pequeños |
| `Critical` | Recomendar un shim de compatibilidad en su lugar |

### Usa subgrafos para mayor legibilidad

Para proyectos grandes, `build_call_subgraph` centrado en un símbolo de interés
ofrece una vista enfocada en lugar del grafo completo, que puede ser abrumador:

```
build_call_subgraph(symbol_name="objetivo", direction="both", depth=2)
```
