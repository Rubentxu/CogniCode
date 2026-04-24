# CogniCode: Por qué construí un IDE para agentes de IA (y por qué debería importarte)

*Los agentes de IA leen código como humanos — línea por línea, secuencialmente, sin ver la estructura. CogniCode les da inteligencia estructural: el call graph, el índice de símbolos, el análisis de arquitectura. Esta es la historia de por qué lo construí y cómo cambia todo sobre cómo los agentes de IA trabajan con codebases.*

---

## El problema: Los agentes de IA vuelan sin visibilidad

Déjame pintarte un cuadro. Son las 2 AM. Estás depurando un incidente en producción en una empresa donde llevas tres meses. La alerta de on-call está gritando sobre un fallo en el procesamiento de pagos. Has aislado el problema a algo en la capa de autenticación, y le pides ayuda a tu asistente de código con IA.

Le preguntas: "¿Qué funciones llaman a `validate_session_token`? Necesito entender el radio de impacto antes de tocar algo."

Esto es lo que pasa en un mundo sin CogniCode:

Tu asistente de IA empieza a leer archivos. Uno por uno. Lee `auth/mod.rs` (312 líneas). Luego `auth/jwt.rs` (489 líneas). Luego `auth/session.rs` (267 líneas). Luego decide revisar `middleware/auth_check.rs` (156 líneas). Luego `services/user_service.rs` (423 líneas). Luego `api/routes/admin.rs` (198 líneas).

Doce archivos después — después de 40 segundos de lectura secuencial, quemando aproximadamente 18,000 tokens de contexto — tu IA te dice que cambiar `validate_session_token` "podría afectar algunos componentes relacionados con autenticación, particularmente alrededor de la validación de tokens en el módulo auth."

¿Tres de esos archivos "afectados"? Ni siquiera existen en el codebase. Dependencias alucinadas. Mientras tanto, la IA perdió completamente el llamador real en `api/routes/webhook.rs` que se rompería silenciosamente si cambiaras la firma de la función.

Esto no es que la IA sea tonta. Es que la IA está trabajando exactamente como fue diseñada: leyendo código como lo haría un humano. Secuencialmente. Sin ninguna comprensión estructural de qué llama a qué.

### El problema de la economía de tokens

Aquí está el problema más profundo del que nadie habla: **la economía de tokens**.

Cada archivo que tu IA lee cuesta dinero. No solo en créditos de API — aunque eso también es real — sino en espacio en la ventana de contexto. Un proyecto Rust de 50,000 líneas podría tener 2,000 funciones en 150 archivos. Si tu IA intenta "entender" el proyecto leyendo archivos, quema su ventana de contexto leyendo archivos que no necesita mientras sigue perdiendo las conexiones críticas.

Un desarrollador senior nunca haría esto. Haría clic derecho en `validate_session_token` y seleccionaría "Find Usages". En Visual Studio Code con la extensión Rust Analyzer, esto toma 200 milisegundos y devuelve una lista exacta de cada sitio de llamada. Sin adivinar. Sin alucinar. Solo hechos.

El desarrollador senior no está leyendo más código — está leyendo código *estructuralmente*. Tiene el IDE como multiplicador de fuerza que le da acceso instantáneo al call graph.

Así que aquí está mi pregunta: **¿Por qué estamos dejando que nuestros agentes de IA tropiecen a través de codebases como si fuera 1995, cuando resolvimos este problema para humanos hace treinta años?**

### Por qué los LLMs struggled con código

Los grandes modelos de lenguaje procesan texto secuencialmente. Son magníficos generando texto fluido, entendiendo lenguaje natural, e incluso escribiendo código que parece plausible. Pero cuando se trata de entender la *estructura* de un codebase — qué funciones llaman a cuáles, cuál es el punto de entrada, dónde están las dependencias circulares — el procesamiento secuencial de texto es fundamentalmente la herramienta equivocada para el trabajo.

Considera qué pasa cuando le preguntas a un LLM: "¿Hay alguna dependencia circular en este codebase?"

Sin herramientas estructurales, el LLM tiene que:
1. Leer cada archivo del codebase
2. Intentar recordar qué funciones llaman a cuáles
3. Construir mentalmente un grafo de dependencias
4. Buscar ciclos en ese grafo mental

Esto le está pidiendo al LLM hacer lo que las computadoras hacen excepcionalmente bien (algoritmos de grafos) usando lo que los LLMs hacen excepcionalmente mal (recordar relaciones estructurales precisas a través de miles de archivos).

El resultado es predecible: el LLM ya sea te dice "No veo dependencias circulares obvias" (que podría estar mal), o encuentra algunas y las presenta con baja confianza, o inventa dependencias que no existen.

Este es el problema que CogniCode resuelve.

---

## La visión: ¿Y si los agentes tuvieran superpoderes de IDE?

Déjame llevarte a 1995. Eres un desarrollador C++. Quieres entender qué hace `processOrder` y dónde se llama. ¿Qué haces?

Usas "Find in Files". Buscas `processOrder`. Obtienes una lista de coincidencias. Abres cada archivo. Lees el código. Intentas mantenerlo todo en tu cabeza.

Luego, alrededor de 2000, algo cambió. Los IDEs empezaron a construir índices de símbolos. Empezaron a rastrear qué función llama a cuál. De repente, "Find Usages" y "Go to Definition" pasaron a ser herramientas indispensables. El IDE mantenía un modelo actualizado de la estructura de tu codebase, y podías consultarlo instantáneamente.

La ganancia de productividad no fue sobre leer más código. Fue sobre leer código *estructuralmente*. En lugar de buscar ciegamente, podías hacer preguntas precisas y obtener respuestas precisas.

**CogniCode trae este mismo cambio de paradigma a los agentes de IA.**

La idea clave es esta: ahora tenemos el MCP (Model Context Protocol) que permite a los agentes de IA llamar herramientas externas. Esto significa que podemos dar a los agentes de IA la misma inteligencia estructural que los IDEs dan a los desarrolladores humanos. No haciendo el LLM más inteligente (ya ha alcanzado su límite para razonamiento estructural), sino dándole herramientas que hacen razonamiento estructural *por* él.

El cambio de paradigma es de:

```
"Leer todo y adivinar"
```

a:

```
"Consultar el grafo y saber"
```

En lugar de pedirle a una IA que lea 50 archivos y espere que entienda el call graph, le damos una herramienta que *ya ha construido* el call graph y puede responder preguntas sobre él instantáneamente.

Esto no se trata de hacer la IA más inteligente. Se trata de darle a la IA la herramienta correcta para el trabajo.

---

## ¿Qué es CogniCode?

CogniCode es un servidor de inteligencia de código escrito en Rust que usa el protocolo MCP (Model Context Protocol). En español claro: es un servidor que los agentes de IA pueden llamar para obtener información estructural sobre codebases.

Esto es lo que ofrece:

**32 herramientas MCP** para inteligencia de código — todo, desde construir call graphs hasta analizar el impacto de cambios, pasando por buscar dependencias circulares.

**6 lenguajes soportados**: Rust, Python, TypeScript, JavaScript, Go y Java. Estos se parsean usando Tree-sitter, que produce ASTs (Abstract Syntax Trees) precisos en lugar de depender de coincidencias frágiles con regex.

**4 estrategias de grafo**: Puedes construir un call graph completo (comprehensivo pero más lento), un índice liviano (búsquedas rápidas solo de símbolos), un grafo on-demand (construido según se necesita), o un grafo por archivo (para análisis de un solo archivo).

**Cache persistente**: Los grafos se almacenan en una base de datos redb embebida (un almacén clave-valor embebido en Rust). Construyes una vez, consultas para siempre. El cache vive dentro de tu proyecto en `.cognicode/graph.redb`.

**Análisis de arquitectura**: Usa el algoritmo de Tarjan para detección de Componentes Fuertemente Conectados (SCC) para encontrar dependencias circulares. Proporciona un score de salud arquitectural que empieza en 100 y resta 5 puntos por cada símbolo involucrado en un ciclo.

**Exportación Mermaid**: Genera diagramas visuales de call graphs como SVG o código Mermaid.

### ¿Por qué Rust?

Construí CogniCode en Rust por razones específicas que importan para este caso de uso:

**Rendimiento**: Construir call graphs para codebases grandes requiere parsear miles de archivos. Las abstracciones de costo cero de Rust y la ausencia de pausas por garbage collection significan rendimiento consistente y rápido.

**Seguridad de memoria**: Las herramientas de inteligencia de código parsean código no confiable. El modelo de propiedad de Rust previene categorías enteras de bugs de memoria que serían catastróficos en una herramienta que procesa entrada arbitraria.

**Embedding**: La base de datos embebida redb significa que no hay proceso de servidor separado. La herramienta corre como un binario único que tu agente de IA spawnea como subproceso.

### ¿Por qué Tree-sitter?

Tree-sitter es un generador de parsers que produce ASTs incrementalmente. Esto es lo que importa:

**Precisión sobre regex**: El análisis de código basado en regex (piensa en "buscar todas las líneas que parecen llamadas a funciones") es frágil. Pierde casos donde el patrón está dividido en múltiples líneas, se confunde con strings que parecen código, y no puede entender la estructura real.

**Agnóstico del lenguaje**: Tree-sitter tiene gramáticas para todos los lenguajes soportados. El mismo análisis estructural funciona sin importar si estás parseando Rust o Python.

**Incrementabilidad**: Cuando un archivo cambia, Tree-sitter puede actualizar el AST solo para ese archivo en lugar de re-parsear todo el codebase.

---

## Ejemplos del Mundo Real: Antes y Después

Esta es la carne del post. Déjame mostrarte cuatro escenarios donde CogniCode transforma cómo un agente de IA trabaja con código. Cada ejemplo muestra la conversación completa: la pregunta del usuario, el razonamiento del agente, las tool calls que hace, y la respuesta que obtiene.

### Ejemplo 1: Onboarding a un Nuevo Codebase

**La situación**

Es lunes por la mañana. Acabas de unirte a una nueva empresa. Te han dado acceso a un proyecto Rust de 200 archivos que ha estado en desarrollo por dos años. Tu tarea: entender la arquitectura lo suficiente como para agregar una nueva característica para el viernes.

Tu manager dice: "Solo pregúntale a la IA que te ayude a entender el codebase."

Le preguntas a tu asistente de IA:

> "Acabo de clonar este repo. Ayúdame a entender qué hace, cuáles son los puntos de entrada principales, y qué funciones se llaman más."

**Lo que hace un agente de IA tradicional**

Sin CogniCode, la IA empieza a leer archivos. Lee `main.rs`, luego `lib.rs`, luego archivos `mod.rs` a través del proyecto. Intenta armar una comprensión de lo que lee. Esto toma 2-3 minutos y te da una imagen vaga, posiblemente inaccurate, de la arquitectura.

**Lo que hace un agente con CogniCode**

El agente razona: "Necesito una vista 360° de este codebase. Los entry points muestran la superficie pública de la API. Las leaf functions muestran los primitivos de bajo nivel. Los hot paths muestran dónde los cambios ripple con mayor impacto. Déjame llamar a estas tres herramientas en secuencia, luego sintetizar."

Tool call 1 — Construir el grafo:
```json
{
  "tool": "build_graph",
  "arguments": {
    "strategy": "full"
  }
}
```
Respuesta:
```json
{
  "status": "success",
  "nodes": 1247,
  "edges": 3891,
  "languages": ["rust"],
  "cache_hit": false,
  "build_time_ms": 1247
}
```

Tool call 2 — Obtener entry points:
```json
{
  "tool": "get_entry_points",
  "arguments": {
    "compressed": true
  }
}
```
Respuesta:
```json
{
  "entry_points": [
    "handle_request (src/api/mod.rs:12)",
    "process_payment (src/payment/mod.rs:34)",
    "authenticate (src/auth/mod.rs:8)",
    "generate_report (src/reports/mod.rs:19)"
  ],
  "summary": "4 puntos de entrada públicos. El handler HTTP delega a la capa de servicios. El procesamiento de pagos es un subsistema separado. Auth es un componente de middleware."
}
```

Tool call 3 — Obtener leaf functions:
```json
{
  "tool": "get_leaf_functions",
  "arguments": {
    "compressed": true
  }
}
```
Respuesta:
```json
{
  "leaf_functions": [
    "hash_password (src/auth/crypto.rs:45) - utilidad pura, sin llamadas salientes",
    "format_currency (src/utils/money.rs:12) - formateo puro, sin llamadas salientes",
    "parse_timestamp (src/utils/time.rs:8) - utilidad de parsing ISO8601"
  ],
  "summary": "Primitivos de bajo nivel. Estos son leaves en el call graph — no llaman a nada más. Seguros de entender pero no críticos para la comprensión de la arquitectura."
}
```

Tool call 4 — Obtener hot paths:
```json
{
  "tool": "get_hot_paths",
  "arguments": {
    "limit": 10,
    "min_fan_in": 3
  }
}
```
Respuesta:
```json
{
  "hot_paths": [
    {"symbol": "validate_token", "fan_in": 47, "location": "src/auth/jwt.rs:89"},
    {"symbol": "log_request", "fan_in": 31, "location": "src/middleware/logging.rs:34"},
    {"symbol": "check_permission", "fan_in": 28, "location": "src/auth/permissions.rs:56"},
    {"symbol": "db_query", "fan_in": 24, "location": "src/db/mod.rs:112"}
  ],
  "summary": "validate_token se llama desde 47 lugares — esta es tu función más crítica. Cualquier cambio aquí tendrá un radio de impacto masivo. log_request se llama desde virtualmente cada handler."
}
```

**La síntesis del agente**

El agente sintetiza esto en una imagen coherente:

> "Este es un proyecto de API web con 4 puntos de entrada públicos. `handle_request` es el gateway HTTP — delega a handlers de la capa de servicios. `process_payment` es un subsistema separado de pagos. `authenticate` es middleware que valida tokens antes de ejecutar los handlers.
>
> La función más crítica es `validate_token` (src/auth/jwt.rs:89) — se llama desde 47 lugares. Esta es la función que mejor debes entender y menos debes cambiar. Si necesitas modificar auth, `check_permission` (llamada desde 28 lugares) también tiene alto impacto.
>
> Las funciones primitivas de bajo nivel como `hash_password` y `format_currency` son leaf functions — son utilidades que no llaman a nada más. Son seguras de leer para entender pero los cambios ahí no ripple hacia arriba.
>
> Empieza leyendo `src/api/mod.rs:12` para entender cómo fluyen las solicitudes HTTP, luego `src/auth/jwt.rs:89` para entender la capa de auth. Esos dos archivos te darán el 80% de lo que necesitas."

**La diferencia**

El enfoque tradicional: 2-3 minutos de lectura, comprensión vaga, posibles alucinaciones.

El enfoque con CogniCode: 1.2 segundos de construcción de grafo, 4 tool calls precisas, comprensión comprehensiva con números reales.

---

### Ejemplo 2: Análisis de Impacto Antes de un PR

**La situación**

Eres un desarrollador backend. Te han pedido cambiar la firma de `UserRepository::find_by_email` para que devuelva `Result<User, UserNotFound>` en lugar de `Option<User>`. Este es un patrón de manejo de errores más idiomático en Rust.

Pero antes de hacer este cambio, necesitas saber: ¿qué se rompe?

Le preguntas a tu asistente de IA:

> "Estoy a punto de cambiar la firma de `UserRepository::find_by_email`. ¿Cuál es el radio de impacto? ¿Qué archivos necesitaré actualizar?"

**Lo que hace un agente de IA tradicional**

La IA lee archivos que parecen que podrían usar el patrón repository. Busca `find_by_email` en archivos probables. Hace猜测 sobre qué podría romperse. Podría perder algunos llamadores. Podría alucinar algunos. Tarda 30 segundos y te da una lista que puede o no ser precisa.

**Lo que hace un agente con CogniCode**

El agente razona: "Necesito encontrar cada llamador de esta función y evaluar el riesgo. La herramienta `analyze_impact` me dará un nivel de riesgo y lista de archivos. Luego puedo profundizar en el call hierarchy para entender el panorama completo."

Tool call 1 — Analizar impacto:
```json
{
  "tool": "analyze_impact",
  "arguments": {
    "symbol_name": "UserRepository::find_by_email"
  }
}
```
Respuesta:
```json
{
  "risk_level": "High",
  "impacted_files": [
    "src/application/auth_service.rs",
    "src/application/user_service.rs",
    "src/api/routes/auth.rs",
    "src/api/routes/admin.rs",
    "src/api/routes/user_profile.rs",
    "tests/integration/auth_test.rs",
    "tests/integration/user_service_test.rs",
    "tests/unit/user_repository_test.rs"
  ],
  "total_affected_symbols": 12,
  "estimated_change_surface": "8 archivos en 3 módulos"
}
```

Tool call 2 — Obtener call hierarchy (incoming):
```json
{
  "tool": "get_call_hierarchy",
  "arguments": {
    "symbol_name": "UserRepository::find_by_email",
    "direction": "incoming",
    "depth": 4
  }
}
```
Respuesta:
```json
{
  "hierarchy": [
    {
      "symbol": "login_user (src/api/routes/auth.rs:45)",
      "depth": 1,
      "callers": ["authenticate_request"]
    },
    {
      "symbol": "get_user_profile (src/api/routes/user_profile.rs:23)",
      "depth": 1,
      "callers": ["extract_user_id"]
    },
    {
      "symbol": "update_user (src/api/routes/admin.rs:67)",
      "depth": 2,
      "callers": ["admin_update_email"]
    }
  ],
  "total_callers": 8
}
```

**La síntesis del agente**

> "Alto riesgo — este cambio afecta 8 archivos en 3 módulos. Aquí está el radio de impacto completo:
>
> **Archivos que necesitarás actualizar:**
> - `src/application/auth_service.rs` — llama `find_by_email` para buscar usuarios durante login
> - `src/application/user_service.rs` — lo usa para búsquedas de perfil de usuario
> - `src/api/routes/auth.rs` — handler del endpoint de login
> - `src/api/routes/admin.rs` — endpoints de administración de usuarios
> - `src/api/routes/user_profile.rs` — recuperación de perfil
> - 3 archivos de test que mockean el repository
>
> **El patrón en todos los llamadores:** Actualmente manejan `Option<User>` con `.unwrap()` o `.expect()`. Necesitarás actualizar cada sitio de llamada para manejar el caso `Result<User, UserNotFound>` — típicamente cambiando `user.unwrap()` a `user?` o `match user { Ok(u) => u, Err(_) => return Err(...) }`.
>
> **Recomendación:** Antes de cambiar la firma, te sugiero agregar un método wrapper `find_by_email_opt` que preserve el comportamiento antiguo. Luego actualizar los sitios de llamada uno por uno. Esto es más seguro que cambiar todo de una vez.
>
> **Alternativa:** Considera usar el operador `?` en todos los sitios de llamada. Como `UserNotFound` implementa `std::error::Error`, se convertirá a un trait object de error boxed si es necesario."

**La diferencia**

El enfoque tradicional:猜测 vaga, probablemente pierde algunos llamadores, ninguna guía específica sobre cómo actualizar cada sitio de llamada.

El enfoque con CogniCode: lista exacta de archivos, evaluación de riesgo, guía específica sobre el patrón de cambio necesario en cada sitio de llamada.

---

### Ejemplo 3: Revisión de Salud de Arquitectura

**La situación**

Eres tech lead. Tu equipo ha estado construyendo rápido, y te preocupa que haya entrado algo de deuda arquitectural. Específicamente, quieres saber: ¿tenemos dependencias circulares? ¿Cuál es nuestro score de salud arquitectural?

Le preguntas a tu IA:

> "¿Hay alguna dependencia circular en este codebase? Dame un score de salud arquitectural."

**Lo que hace un agente de IA tradicional**

La IA lee la estructura de módulos, intentando trazar los imports. Podría encontrar algunos ciclos, pero esencialmente está haciendo el algoritmo de Tarjan mentalmente, lo cual es propenso a errores. Te da una respuesta con baja confianza y sin score cuantitativo.

**Lo que hace un agente con CogniCode**

El agente razona: "La herramienta `check_architecture` usa el algoritmo de Tarjan para detección de SCC (Strongly Connected Components). Este es un algoritmo de grafos bien definido — no algo que un LLM deba intentar hacer mentalmente. Déjame llamarlo directamente."

Tool call 1 — Construir el grafo (prerrequisito):
```json
{
  "tool": "build_graph",
  "arguments": {
    "strategy": "full"
  }
}
```

Tool call 2 — Verificar arquitectura:
```json
{
  "tool": "check_architecture",
  "arguments": {}
}
```
Respuesta:
```json
{
  "architecture_score": 85,
  "cycles_detected": 3,
  "cycle_details": [
    {
      "symbols": ["Order -> Payment -> Billing -> Order"],
      "length": 3,
      "severity": "minor"
    },
    {
      "symbols": ["User -> Auth -> Session -> User"],
      "length": 3,
      "severity": "minor"
    },
    {
      "symbols": ["Config -> Settings -> Preferences -> Config"],
      "length": 3,
      "severity": "minor"
    }
  ],
  "algorithm": "Tarjan SCC",
  "analysis_time_ms": 156
}
```

**La síntesis del agente**

> "Score de arquitectura: 85 sobre 100. Tienes 3 dependencias circulares menores — no crítico, pero vale la pena atender.
>
> **Los ciclos:**
>
> 1. **Order → Payment → Billing → Order** — Este es un ciclo en la lógica de dominio. El módulo `Order` llama a `Payment`, que llama a `Billing`, que llama de vuelta a `Order`. Esto sugiere que estos tres módulos están demasiado acoplados. Solución recomendada: introduce un trait `BillingService` compartido del que tanto `Order` como `Payment` dependan, en lugar de llamarse directamente.
>
> 2. **User → Auth → Session → User** — Gestión de usuarios, auth y gestión de sesión formando un ciclo. Esto es común en sistemas con mucha auth. Solución recomendada: extrae un trait `SessionStore` del que `Auth` dependa, y haz que `User` dependa de `Session` a través de una interfaz en lugar de directamente.
>
> 3. **Config → Settings → Preferences → Config** — Ciclo del sistema de configuración. Menor, probablemente solo organización de módulos.
>
> **Interpretación del score:**
> - 100 = arquitectura limpia, sin ciclos
> - 80-99 = ciclos menores, vale la pena atender cuando sea conveniente
> - 50-79 = acoplamiento significativo, planifica refactoring
> - <50 = problemas arquitecturales serios
>
> Con 85, estás en buena forma. Te recomiendo atender el ciclo Order/Payment/Billing antes de que crezca — los ciclos tienen tendencia a atraer más código hacia ellos con el tiempo."

**La diferencia**

El enfoque tradicional: "No veo dependencias circulares obvias" (probablemente wrong).

El enfoque con CogniCode: algoritmo exacto, ciclos exactos, severidad exacta, recomendaciones de refactoring específicas.

---

### Ejemplo 4: Safe Rename Refactoring

**La situación**

Estás refactorizando un codebase legacy. Has encontrado una función llamada `calc_total` que se usa en todo el proyecto. Debería llamarse `calculate_order_total` — el nombre se acortó hace 10 años y nadie lo corrigió.

Tienes 47 archivos en este proyecto. Necesitas renombrar en todas partes, pero no quieres romper nada.

Le preguntas a tu IA:

> "Renombra `calc_total` a `calculate_order_total` en todas partes del proyecto. Asegúrate de que nada se rompa."

**Lo que hace un agente de IA tradicional**

La IA usa buscar-y-reemplazar a través del codebase. Podría perder algunos casos. Podría accidentalmente reemplazar `calc_total` dentro de strings o comentarios donde no debería. No sabe cuáles usages son llamadas reales de código versus documentación. Podría perder archivos de test. Podría no actualizar declaraciones de import/export correctamente.

**Lo que hace un agente con CogniCode**

El agente razona: "Necesito seguir la secuencia de safe refactoring: (1) analizar impacto para entender el alcance, (2) preview del rename para ver exactamente qué cambiará, (3) esperar confirmación del usuario, (4) aplicar. Nunca renombrar a ciegas."

Tool call 1 — Analizar impacto:
```json
{
  "tool": "analyze_impact",
  "arguments": {
    "symbol_name": "calc_total"
  }
}
```
Respuesta:
```json
{
  "risk_level": "Medium",
  "impacted_files": [
    "src/domain/order.rs",
    "src/application/order_service.rs",
    "src/api/routes/checkout.rs",
    "src/api/routes/invoice.rs"
  ],
  "total_affected_symbols": 6,
  "estimated_change_surface": "4 archivos"
}
```

Tool call 2 — Encontrar usages:
```json
{
  "tool": "find_usages",
  "arguments": {
    "symbol_name": "calc_total",
    "include_declaration": true
  }
}
```
Respuesta:
```json
{
  "usages": [
    {"file": "src/domain/order.rs", "line": 45, "context": "pub fn calc_total(&self) -> Money"},
    {"file": "src/application/order_service.rs", "line": 78, "context": "let total = self.order.calc_total()"},
    {"file": "src/application/order_service.rs", "line": 112, "context": "order.calc_total().into()"},
    {"file": "src/api/routes/checkout.rs", "line": 34, "context": "total: order.calc_total()"},
    {"file": "src/api/routes/invoice.rs", "line": 56, "context": "calc_total(order)"}
  ],
  "total_usages": 5
}
```

Tool call 3 — Preview del rename:
```json
{
  "tool": "safe_refactor",
  "arguments": {
    "action": "rename",
    "symbol_name": "calc_total",
    "new_name": "calculate_order_total",
    "file_path": "src/domain/order.rs",
    "line": 45,
    "column": 8,
    "preview": true
  }
}
```
Respuesta:
```json
{
  "preview": true,
  "action": "rename",
  "affected_files": [
    {"file": "src/domain/order.rs", "changes": ["line 45: definición de función"]},
    {"file": "src/application/order_service.rs", "changes": ["line 78: sitio de llamada", "line 112: sitio de llamada"]},
    {"file": "src/api/routes/checkout.rs", "changes": ["line 34: sitio de llamada"]},
    {"file": "src/api/routes/invoice.rs", "changes": ["line 56: sitio de llamada"]}
  ],
  "files_to_modify": 4,
  "total_changes": 5
}
```

**La síntesis del agente**

> "Rename de riesgo medio — 4 archivos afectados, 5 cambios totales. Esto es lo que se renombrará:
>
> **src/domain/order.rs:45** — Definición de función
> - `pub fn calc_total(&self) -> Money` → `pub fn calculate_order_total(&self) -> Money`
>
> **src/application/order_service.rs** — 2 sitios de llamada
> - Línea 78: `self.order.calc_total()` → `self.order.calculate_order_total()`
> - Línea 112: `order.calc_total().into()` → `order.calculate_order_total().into()`
>
> **src/api/routes/checkout.rs:34** — 1 sitio de llamada
> - `total: order.calc_total()` → `total: order.calculate_order_total()`
>
> **src/api/routes/invoice.rs:56** — 1 sitio de llamada
> - `calc_total(order)` → `calculate_order_total(order)`
>
> Noto algo: `invoice.rs` la llama como función independiente `calc_total(order)` en lugar de método `order.calc_total()`. Esto sugiere que podría haber una función libre además del método en Order. ¿Quieres que renombre ambas, o solo el método?
>
> **Para proceder:** Di 'sí' y aplicaré el rename a las 5 ubicaciones. Di 'no' y me detengo aquí."

**Lo que pasa después (el usuario confirma)**

Tool call 4 — Aplicar el rename:
```json
{
  "tool": "safe_refactor",
  "arguments": {
    "action": "rename",
    "symbol_name": "calc_total",
    "new_name": "calculate_order_total",
    "file_path": "src/domain/order.rs",
    "line": 45,
    "column": 8,
    "preview": false
  }
}
```
Respuesta:
```json
{
  "status": "success",
  "files_modified": 4,
  "total_changes": 5,
  "validation": {
    "syntax_valid": true,
    "no_breaks": true
  }
}
```

**La confirmación del agente**

> "Hecho. Renombré `calc_total` a `calculate_order_total` en 4 archivos (5 ubicaciones). La validación de sintaxis pasó — no hay referencias rotas.
>
> NO renombré la función libre `calc_total(order)` en `invoice.rs:56` ya que parece ser una función diferente (independiente en lugar de método). Si quieres que también la renombre, avísame."

**La diferencia**

El enfoque tradicional: búsqueda y reemplazo a ciegas, probablemente pierde algo, posible romper cosas, sin validación.

El enfoque con CogniCode: preview primero, confirmación del usuario, lista exacta de cambios, validación de sintaxis después, reconocimiento de casos edge (la función libre).

---

## Cómo Funciona Bajo el Capó

Te podrías estar preguntando: ¿cómo funciona CogniCode realmente? Déjame levantar el telón.

### Parsing con Tree-sitter

El primer paso es el parsing. CogniCode usa Tree-sitter para parsear archivos fuente en Abstract Syntax Trees (ASTs). Un AST es una representación de árbol de tu código donde cada nodo representa un elemento estructural: definiciones de funciones, llamadas a funciones, declaraciones de tipos, imports, etc.

Por ejemplo, cuando Tree-sitter parsea este código Rust:

```rust
pub fn calculate_order_total(&self) -> Money {
    self.items.iter().map(|i| i.price).sum()
}
```

Produce un AST donde el nodo de función tiene hijos para:
- El nombre de la función (`calculate_order_total`)
- Los parámetros (`&self`)
- El tipo de retorno (`Money`)
- El cuerpo (la cadena del iterador)

Esto es importante porque significa que CogniCode puede distinguir entre:
- Una *definición* de función (donde `calc_total` es declarada)
- Una *llamada* a función (donde `calc_total()` es invocada)
- Un string que casualmente contiene "calc_total" (que no es una referencia en absoluto)

Regex no puede hacer esta distinción. Solo un parser adecuado puede.

### Construyendo el Call Graph

Una vez que los archivos son parseados en ASTs, CogniCode extrae el call graph. Para cada definición de función, encuentra todas las llamadas a funciones dentro de ella. Esto crea bordes en el grafo: "función A llama a función B."

El call graph se almacena en una base de datos redb embebida. Este es un almacén clave-valor persistente respaldado por disco, escrito en Rust. Una vez que el grafo está construido, las consultas subsecuentes no necesitan re-parsear ni re-analizar — simplemente consultan el grafo pre-construido.

### Detección de Estaleness

Aquí hay un problema: si editas un archivo, el grafo cacheado se vuelve obsoleto. CogniCode maneja esto con un `FileManifest` que rastrea el `mtime` (tiempo de modificación) de cada archivo. Antes de devolver resultados, CogniCode verifica si algún archivo ha cambiado desde que el grafo fue construido. Si han cambiado, re-parsea solo esos archivos y actualiza solo las partes afectadas del grafo.

Esto se llama re-análisis incremental, y es crítico para el rendimiento. No quieres reconstruir un grafo de 2,000 archivos cada vez que haces un pequeño edit.

### Las Cuatro Estrategias de Grafo

CogniCode ofrece cuatro estrategias para construir el grafo, porque diferentes tareas tienen diferentes tradeoffs:

**Full (`strategy: "full"`)**: Parsea cada archivo del proyecto, construye call graph completo con todos los bordes. Este es el más comprehensivo pero el más lento. Usar para análisis de arquitectura, evaluación de impacto, encontrar código muerto.

**Lightweight (`strategy: "lightweight"`)**: Parsea archivos solo para definiciones de símbolos, sin análisis de bordes. Rápido (segundos vs. decenas de segundos). Usar para búsquedas de símbolos, encontrar dónde está definido un símbolo.

**On-demand (`strategy: "on_demand"`)**: Construye el grafo para un símbolo específico y su vecindad inmediata. Usar cuando solo te importa una función y sus llamadores/llamados directos.

**Per-file (`strategy: "per_file"`)**: Construye grafo para un solo archivo únicamente. Usar para análisis a nivel de archivo.

### El Architecture Score

El score de salud arquitectural se calcula usando el algoritmo de Tarjan para encontrar Componentes Fuertemente Conectados (SCCs). Un SCC es un conjunto de nodos donde cada nodo es alcanzable desde cada otro nodo — en otras palabras, un ciclo.

El algoritmo de Tarjan corre en tiempo O(V+E) (lineal en el número de vértices y bordes) y encuentra todos los SCCs en una sola pasada. El score de arquitectura empieza en 100 y resta 5 puntos por cada símbolo involucrado en un ciclo. Esto te da una medida cuantitativa de salud arquitectural.

---

## Empezando

CogniCode está diseñado para no requerir configuración. Lo apuntas a un directorio y empiezas a consultar.

### Configuración del Servidor MCP

Agrega CogniCode a tu configuración de servidor MCP. El formato exacto depende de tu asistente de IA:

**Claude Desktop:**
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

**Cursor:**
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

**Windsurf:**
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

**OpenCode:**
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

### Tu Primera Sesión

Lo primero que deberías hacer en cualquier sesión de CogniCode es construir el grafo:

```json
{
  "tool": "build_graph",
  "arguments": {
    "strategy": "full"
  }
}
```

Esto puede tomar unos segundos para proyectos grandes, pero es un costo único por sesión. Después de eso, todas las consultas son instantáneas.

Luego prueba:
```json
{
  "tool": "analyze_impact",
  "arguments": {
    "symbol_name": "TU_FUNCION_MAS_LLAMADA"
  }
}
```

Reemplaza `TU_FUNCION_MAS_LLAMADA` con la función que crees que es crítica en tu codebase. La herramienta te dirá exactamente qué se rompería si la cambiaras.

---

## Qué Viene Después — La Visión Adelante

CogniCode está listo para producción hoy con 32 herramientas en 6 lenguajes. Pero esto es solo el comienzo.

### Roadmap a Corto Plazo

**Más lenguajes**: Soporte para C++ y Kotlin está en el roadmap. Estos son ampliamente usados en ambientes empresariales, y agregarlos expande significativamente el mercado direccionable de CogniCode.

**Semantic diff**: Imagina preguntar "muéstrame qué cambió arquitecturalmente entre la versión 1.2 y la 1.3 de este codebase." No solo qué líneas cambiaron, sino cómo evolucionó el call graph. ¿Qué funciones gained llamadores? ¿Qué dependencias se introdujeron? Esto es hacia dónde vamos.

**Integración con cobertura de tests**: Imagina mergear el call graph con datos de cobertura de código. "Esta función tiene 94% de cobertura de tests pero se llama desde 47 lugares y no tiene tests de integración." La combinación de análisis estructural y datos de cobertura permite priorización de tests más inteligente.

**Plugin de IDE**: Actualmente CogniCode funciona a través de MCP. Un plugin nativo de IDE (VS Code, IntelliJ) daría a los desarrolladores humanos acceso a la misma inteligencia, sin necesidad de ingeniería de prompts.

### El Panorama General

Aquí es donde creo que todo esto se dirige.

Los asistentes de codificación con IA se están convirtiendo en herramientas estándar en desarrollo de software. GitHub Copilot, Claude Code, Cursor — estos ya no son experimentos; son herramientas de producción que los desarrolladores usan diariamente.

Pero aquí está el problema: estos asistentes de IA todavía leen código como los humanos leen código — secuencialmente, sin comprensión estructural. Están mejorando en generar código, pero no están mejorando en entender codebases.

Esta es una limitación fundamental que ventanas de contexto más grandes no resolverán. Puedes darle a un LLM una ventana de contexto de 1 millón de tokens, y aún así no entenderá el call graph tan bien como una herramienta que lo ha explícitamente construido e indexado.

**El futuro es agentes que tienen inteligencia estructural integrada.** No porque el LLM en sí se vuelva un mejor razonador estructural, sino porque tiene herramientas que hacen razonamiento estructural por él.

CogniCode es el comienzo de esto. Cuando cada asistente de codificación con IA venga con herramientas de inteligencia de código, la calidad del desarrollo asistido por IA sube dramáticamente. Agentes que pueden evaluar impacto con precisión antes de hacer cambios. Agentes que pueden encontrar dependencias circulares antes de que causen problemas. Agentes que pueden refactorizar de forma segura sin romper cosas.

Esto no es ciencia ficción. Es lo que CogniCode habilita hoy.

### Llamado a Contribuidores

CogniCode es open source. Si esta visión resuena contigo — si crees que los agentes de IA merecen mejores herramientas — te invito a contribuir.

- Dale una estrella al repo
- Pruébalo en tu codebase
- Reporta issues para bugs y requests de características
- Envía PRs para lenguajes que quieras ver soportados
- Corre la voz

El repo está en: [https://github.com/Rubentxu/CogniCode](https://github.com/Rubentxu/CogniCode)

---

## Pruébalo Hoy

No tienes nada que perder y todo que ganar. Tu asistente de codificación con IA está volando sin visibilidad ahora. Dale el regalo de la inteligencia estructural.

Pasa de esto: "Leí 12 archivos y creo que cambiar esta función podría afectar algunas cosas."

A esto: "Esta función se llama desde 47 lugares en 8 archivos. Nivel de riesgo: Alto. Aquí están los sitios de llamada exactos que necesitas actualizar."

La diferencia no es solo velocidad. Es precisión. Es confianza. Es saber en lugar de adivinar.

CogniCode le da a tus agentes de IA los mismos superpoderes de navegación de código que los desarrolladores senior dan por sentados en su IDE. Deja de dejar que lean código como si fuera 1995.

**GitHub**: [https://github.com/Rubentxu/CogniCode](https://github.com/Rubentxu/CogniCode)

Dale una estrella al proyecto. Pruébalo en tu codebase. Déjame saber qué construyes.

---

*Este post también está disponible en: [English](blog-post-en.md)*
