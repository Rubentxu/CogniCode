# Patrones de Contratos y Reflexión para Alineación de Reglas en Agentes IA

## Índice

1. [Introducción](#introducción)
2. [Sistemas Axiomáticos de Métodos Formales](#1-sistemas-axiomáticos-de-métodos-formales)
3. [Preservación de Invariantes en Generación de Código](#2-preservación-de-invariantes-en-generación-de-código)
4. [ADRs como Restricciones](#3-adrs-como-restricciones)
5. [Límites Hexagonales/DDD como Reglas](#4-límites-hexagonalddd-como-reglas)
6. [Entropía SOLID/Connascence](#5-entropía-solidconnascence)
7. [Arquitectura de Bucle de Retroalimentación Reflexión](#6-arquitectura-de-bucle-de-retroalimentación-reflexión)
8. [Design by Contract para IA](#7-design-by-contract-para-ia)
9. [Conclusiones y Líneas Futuras](#conclusiones-y-líneas-futuras)

---

## Introducción

Este documento explora patrones arquitectónicos para la alineación de reglas en sistemas de IA agentica. A medida que los agentes conversacionales y de código evolucionan hacia agentes autónomos capaces de ejecutar tareas complejas, la necesidad de mecanismos robustos para definir, verificar y hacer cumplir reglas de comportamiento se vuelve crítica. Los patrones presentados aquí emergen de la intersección de múltiples disciplinas: métodos formales, ingeniería de software, teoría de lenguajes de programación y arquitectura de sistemas distribuidos.

La **alineación de reglas** (rule alignment) se refiere al proceso de garantizar que el comportamiento de un agente IA sea consistente con un conjunto de restricciones definidas por el sistema, los desarrolladores o los usuarios. Estas restricciones pueden ser de naturaleza diversa: reglas de seguridad, convenciones de código, principios arquitectónicos, políticas de negocio o invariants de dominio.

El documento sigue una progresión lógica desde los fundamentos teóricos hasta las implementaciones prácticas, conectando conceptos de métodos formales con patrones de ingeniería de software que pueden ser implementados y verificados de manera automatizada.

---

## 1. Sistemas Axiomáticos de Métodos Formales

### 1.1 Fundamentos de Axiomas en Sistemas Formales

Un **sistema axiomático** es un aparato formal constituido por un lenguaje formal, un conjunto de axiomas y reglas de inferencia. Los axiomas son verdades fundamentales que se aceptan sin demostración y que sirven como base para derivar todo el conocimiento del sistema. Esta aproximación, heredada de Euclid y refinada por Hilbert, Peano y otros, ofrece un modelo poderoso para pensar sobre restricciones en sistemas de software.

En el contexto de agentes IA, podemos adaptar este modelo formal de la siguiente manera:

- **Axiomas (Hard Rules)**: verdades absolutas e inquebrantables que nunca deben violarse
- **Teoremas derivados (Soft Rules)**: guías y convenciones que emergen de los axiomas pero pueden adaptarse
- **Reglas de inferencia**: mecanismos para aplicar los axiomas a casos concretos
- **Demostraciones**: verificaciones formales de que una acción cumple los axiomas

### 1.2 Propiedades de Safety vs Liveness

Los métodos formales distinguen entre dos categorías fundamentales de propiedades:

**Propiedades de Safety** (seguridad): especifican que "algo malo nunca sucede". Son propiedades de tipo "nunca": nunca se revelará información sensible, nunca se accederá a memoria no autorizada, nunca se violará una restricción de dominio. Las propiedades de safety son **absolutas** — su violación representa un fracaso completo del sistema.

**Propiedades de Liveness** (vivacidad): especifican que "algo bueno eventualmente sucede". Son propiedades de tipo "eventualmente": eventualmente el sistema procesará todas las solicitudes, eventualmente se completará la tarea, eventualmente el agente responderá. Las propiedades de liveness son **graduales** — su cumplimiento se mide en términos de progreso.

```
┌─────────────────────────────────────────────────────────────────┐
│                  PROPIEDADES DE SISTEMAS                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   SAFETY (Safety Properties)          LIVENESS (Liveness)       │
│   ════════════════════════           ════════════════════       │
│                                                                 │
│   "Algo malo NUNCA ocurre"          "Algo bueno EVENTUALMENTE  │
│                                        ocurre"                  │
│                                                                 │
│   ┌─────────────────────┐            ┌─────────────────────┐   │
│   │  Ejemplos:          │            │  Ejemplos:          │   │
│   │                     │            │                     │   │
│   │  • No SQL injection │            │  • El sistema       │   │
│   │  • No acceso a      │            │    procesará la     │   │
│   │    archivos no      │            │    request          │   │
│   │    autorizados      │            │  • El agente        │   │
│   │  • No circ. deps    │            │    eventualmente    │   │
│   │  • Transactions     │            │    responderá      │   │
│   │    atómicas         │            │  • Se completará    │   │
│   │                     │            │    la tarea         │   │
│   └─────────────────────┘            └─────────────────────┘   │
│                                                                 │
│   Características:                Características:              │
│   • Absolutas (0/1)              • Graduales (gradiente)        │
│   • Verificables en tiempo       • Verificables en límite     │
│     de compilación                • Problema: "eventually     │
│   • Violación = falla             "     puede tomar forever   │
│     catastrófica                                               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

En el contexto de gobierno de agentes IA, esta distinción es crucial:

| Categoría | Agente IA | Ejemplo |
|-----------|-----------|---------|
| Safety hard | Nunca acceder a archivos fuera del workspace | Intentar leer `/etc/passwd` debe ser bloqueado |
| Safety hard | Nunca revelar secrets a usuarios no autorizados | Credenciales en logs deben ser redacted |
| Liveness soft | El agente eventualmente debe completar la tarea | Timeout después de N intentos |
| Liveness soft | El agente eventualmente debe pedir clarificaciones | Después de 3 intentos fallidos |

### 1.3 Invariants como Verdades Inquebrantables

Un **invariant** es una condición que debe ser verdadera en todos los estados accesibles del sistema. Los invariantes capturan verdades fundamentales sobre el dominio o el sistema que nunca deben violarse. A diferencia de las precondiciones y postcondiciones (que se evalúan en fronteras), los invariantes deben mantenerse durante toda la ejecución.

En el contexto de agentes IA, los invariantes pueden definirse a múltiples niveles:

```
┌─────────────────────────────────────────────────────────────────┐
│                    NIVELES DE INVARIANTES                        │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────┐
│ INVARIANTES DE  │  Ejemplo: "Todas las respuestas incluyen
│ DOMINIO         │  citation para información factual"
│                 │
│ Dominio =      │  Se mantiene durante toda la interacción
│ mundo real del  │  con el usuario
│ problema        │
└─────────────────┘
         │
         ▼
┌─────────────────┐
│ INVARIANTES DE  │  Ejemplo: "El agente siempre puede explicar
│ COMPORTAMIENTO  │  por qué tomó una decisión"
│                 │
│ Reglas de      │  Se mantiene durante toda la sesión
│ operación del   │  del agente
│ agente          │
└─────────────────┘
         │
         ▼
┌─────────────────┐
│ INVARIANTES DE  │  Ejemplo: "El código generado siempre tiene
│ CÓDIGO          │  tests unitarios"
│                 │
│ Reglas de      │  Se mantiene durante la generación
│ ingeniería      │  y verificación
└─────────────────┘
         │
         ▼
┌─────────────────┐
│ INVARIANTES DE  │  Ejemplo: "El estado de la sesión del agente
│ SISTEMA         │  es serializable"
│                 │
│ Reglas de      │  Se mantiene durante toda la
│ infraestructura │  ejecución del sistema
└─────────────────┘
```

### 1.4 Separación: Axiomas (Hard Rules) vs Guidelines (Soft Rules)

La distinción entre axiomas y guidelines es fundamental para construir sistemas de gobierno escalables. Los axiomas son **obligatorios y verificables automáticamente**; las guidelines son **recomendaciones que pueden violarse bajo circunstancias específicas**.

```
┌─────────────────────────────────────────────────────────────────┐
│        SEPARACIÓN AXIOMAS vs GUIDELINES                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   AXIOMAS (Hard Rules)              GUIDELINES (Soft Rules)     │
│   ══════════════════               ══════════════════         │
│                                                                 │
│   • Obligatorias                     • Recomendaciones          │
│   • Verificables automático         • Evaluables con peso      │
│   • Violación = DENEGAR             • Violación = WARN/ADVISORY │
│   • Sin excepciones                 • Con excepciones marcadas  │
│   • Ejemplo:                        • Ejemplo:                  │
│     "No execute() de                │   "Prefiere composición   │
│      strings controlados"            │    sobre herencia"        │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   EJEMPLO PRÁCTICO:                                            │
│                                                                 │
│   AXIOMA: "El agente no puede acceder a archivos                 │
│   fuera del workspace definido"                                │
│   → Verificación: AST analysis + path canonicalization         │
│   → Violación: Denegar ejecución, notificar                    │
│                                                                 │
│   GUIDELINE: "El código generado debería seguir                │
│   la convención de nombres del proyecto"                       │
│   → Verificación: Linting con weight 0.3                        │
│   → Violación: Warning, no bloquante                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 1.5 Mapeo a Gobernanza de Agentes IA

El modelo axiomático se mapea naturalmente a sistemas de gobierno de agentes IA:

```
┌─────────────────────────────────────────────────────────────────┐
│          MAPEO MÉTODOS FORMALES → GOBIERNO AGENTES IA           │
└─────────────────────────────────────────────────────────────────┘

    SISTEMA FORMAL                  AGENTE IA
    ══════════════                   ══════════

    Axioma                    →      Hard Rule (Security Policy)
    ─────────                        ───────────────────────────
    "√2 es irracional"                "No acceder a /etc/passwd"

    Teorema Derivado           →      Soft Rule (Best Practice)
    ───────────────────               ─────────────────────────
    "√2² = 2"                        "Usar prepared statements"

    Regla de Inferencia        →      Policy Engine
    ──────────────────               ─────────────────
    "De A y A→B inferir B"          "De 'es SQL' y 'usa concat'
                                     inferir 'violación de SQLi'"

    Demostración               →      Verification Report
    ─────────────                    ───────────────────
    "∀n, n² es par ↔ n es par"       "El código cumple la política
                                     de seguridad (verificado)"

    Consistencia              →      Alignment
    ───────────                     ──────────
    "No contradicción"               "Comportamiento consistente
                                     con valores humanos"
```

---

## 2. Preservación de Invariantes en Generación de Código

### 2.1 Patrones para Definir Invariantes

Los invariantes en sistemas de generación de código deben ser definidos de manera **formal, inequívoca y verificable**. Un patrón efectivo es usar **lenguajes de especificación** que puedan ser procesados por herramientas automatizadas.

**Invariantes de Seguridad**:

```yaml
# Ejemplo: Definición de invariantes de seguridad como políticas
security_invariants:
  - id: "SQL_INJECTION_PREVENTION"
    description: "Todas las queries SQL deben usar parameterized queries"
    pattern: "query.*\+\s*["']"  # Detecta concatenación
    severity: "CRITICAL"
    action: "DENY"

  - id: "NO_EVAL_USER_INPUT"
    description: "Nunca hacer eval() de input del usuario"
    pattern: "eval\s*\(\s*request\."
    severity: "CRITICAL"
    action: "DENY"

  - id: "CREDENTIALS_NOT_LOGGED"
    description: "Credenciales no deben aparecer en logs"
    pattern: "log\.(info|debug|warn).*(password|secret|key|token)"
    severity: "HIGH"
    action: "DENY"
```

**Invariantes de Documentación**:

```yaml
# Ejemplo: Invariantes de documentación OpenAPI
documentation_invariants:
  - id: "OPENAPI_SPEC_REQUIRED"
    description: "Todos los endpoints REST deben estar documentados"
    pattern: "@app\.(get|post|put|delete|patch)"
    requires:
      - "summary"
      - "parameters"
      - "responses"
    severity: "MEDIUM"
    action: "DENY"

  - id: "PARAMETER_DOCUMENTATION"
    description: "Parámetros deben tener descripción"
    pattern: "def .*\([^)]*\)"
    check: "docstring includes parameter descriptions"
    severity: "LOW"
    action: "WARN"
```

**Invariantes de Arquitectura**:

```yaml
# Ejemplo: Invariantes arquitectónicas
architecture_invariants:
  - id: "NO_CIRCULAR_DEPS"
    description: "No dependencias circulares entre módulos"
    check: "import graph is DAG"
    severity: "CRITICAL"
    action: "DENY"

  - id: "DOMAIN_INFRASTRUCTURE_SEPARATION"
    description: "Domain no puede depender de Infrastructure"
    check: "domain imports contain only domain + shared"
    severity: "CRITICAL"
    action: "DENY"

  - id: "NO_SIDE_EFFECTS_IN_PURE_FUNCTIONS"
    description: "Funciones marcadas como @pure no deben tener side effects"
    check: "@pure function contains no I/O"
    severity: "HIGH"
    action: "DENY"
```

### 2.2 El Bucle Invariant Checker → Feedback

El mecanismo de preservación de invariantes opera como un **bucle de control feedback** que intercepta, verifica y corrige antes de que se materialicen violaciones.

```
┌─────────────────────────────────────────────────────────────────┐
│              BUCLE DE VERIFICACIÓN DE INVARIANTES                │
└─────────────────────────────────────────────────────────────────┘

   ┌──────────────────────────────────────────────────────────┐
   │                                                          │
   │   AGENTE GENERA          INVARIANT CHECKER               │
   │   CÓDIGO                 ┌─────────────────┐             │
   │        │                 │                 │             │
   │        ▼                 │  1. PARSE        │             │
   │   ┌─────────┐            │     AST del      │             │
   │   │ Código  │───────────▶ │     código       │             │
   │   │ source  │            │                 │             │
   │   └─────────┘            │  2. VERIFY       │             │
   │                          │     invariantes  │             │
   │                          │                 │             │
   │                          │  3. CLASSIFY     │             │
   │                          │     violations   │             │
   │                          │                 │             │
   │                          └────────┬────────┘             │
   │                                   │                      │
   │                    ┌──────────────┼──────────────┐       │
   │                    ▼              ▼              ▼       │
   │               ┌─────────┐   ┌─────────┐   ┌─────────┐   │
   │               │ PASS    │   │ WARN    │   │ FAIL    │   │
   │               │ ✓        │   │ ⚠️      │   │ ✗       │   │
   │               │ Continua │   │ Advisory│   │ Bloquea │   │
   │               └─────────┘   └─────────┘   └────┬────┘   │
   │                                                  │        │
   │   ┌──────────────────────────────────────────────┘        │
   │   │                                                         │
   │   ▼                                                         │
   │  ┌─────────────────────────────────────────────────────┐   │
   │  │               FEEDBACK LOOP                          │   │
   │  │                                                     │   │
   │  │  4. EXPLAIN: "El código viola SQL_INJECTION_PREV    │   │
   │  │  5. SUGGEST: "Usa db.query('SELECT * FROM users     │   │
   │  │              WHERE id = ?', [user_id])"            │   │
   │  │  6. WAIT: Agent revisa y genera código corregido    │   │
   │  │                                                     │   │
   │  └─────────────────────────────────────────────────────┘   │
   │                            │                               │
   │                            │ Ciclo se repite              │
   │                            ▼                               │
   │                      [Verificación                          │
   │                       completa]                            │
   │                                                          │
   └──────────────────────────────────────────────────────────┘
```

### 2.3 Implementación de un Verificador de Invariantes

A continuación se presenta un patrón de implementación para un verificador de invariantes:

```python
# Patrón: InvariantChecker
class InvariantChecker:
    """
    Motor de verificación de invariantes para código generado.
    Implementa el patrón Strategy para diferentes tipos de verificación.
    """

    def __init__(self, policy_engine: PolicyEngine):
        self.policy_engine = policy_engine
        self.invariants = []
        self.violations = []

    def register_invariant(self, invariant: Invariant):
        """Registra un invariante a verificar."""
        self.invariants.append(invariant)

    def verify(self, code: str, context: GenerationContext) -> VerificationResult:
        """
        Verifica todos los invariantes contra el código dado.
        Retorna resultado estructurado con violaciones categorizadas.
        """
        self.violations = []
        ast = self.parse(code)

        for invariant in self.invariants:
            result = invariant.check(ast, context)
            if result.is_violation:
                self.violations.append(Violation(
                    invariant_id=invariant.id,
                    severity=invariant.severity,
                    location=result.location,
                    explanation=result.explanation,
                    suggestion=result.suggestion
                ))

        return self.classify_result()

    def parse(self, code: str) -> AST:
        """Parsing del código según el lenguaje detectado."""
        # Implementación simplificada
        pass

    def classify_result(self) -> VerificationResult:
        """
        Clasifica el resultado según la severidad de violaciones.
        """
        has_critical = any(v.severity == "CRITICAL" for v in self.violations)
        has_high = any(v.severity == "HIGH" for v in self.violations)

        if has_critical:
            return VerificationResult(
                status="DENIED",
                violations=self.violations,
                feedback=self.generate_feedback()
            )
        elif has_high:
            return VerificationResult(
                status="WARNED",
                violations=self.violations,
                feedback=self.generate_feedback()
            )
        else:
            return VerificationResult(
                status="PASSED",
                violations=[],
                feedback=[]
            )

    def generate_feedback(self) -> List[Feedback]:
        """Genera retroalimentación estructurada para el agente."""
        feedback = []
        for v in self.violations:
            feedback.append(Feedback(
                message=f"Violación de invariante: {v.invariant_id}",
                explanation=v.explanation,
                suggestion=v.suggestion,
                severity=v.severity,
                location=f"{v.location.file}:{v.location.line}"
            ))
        return feedback
```

---

## 3. ADRs como Restricciones

### 3.1 Arquitectura Decision Records (ADRs)

Un **Architecture Decision Record (ADR)** es un documento que captura una decisión arquitectónica importante, incluyendo su contexto, la decisión misma y sus consecuencias. Los ADRs fueron popularizados por Michael Nygard en su libro "Release It" y se han convertido en un estándar de facto para documentar decisiones técnicas.

En el contexto de agentes IA, los ADRs representan **restricciones heredadas** — decisiones tomadas por desarrolladores humanos que los agentes deben respetar para mantener la coherencia arquitectónica.

```
┌─────────────────────────────────────────────────────────────────┐
│                    ANATOMÍA DE UN ADR                           │
└─────────────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────────┐
│ ADR-0042: Uso de PostgreSQL como base de datos primaria       │
├───────────────────────────────────────────────────────────────┤
│                                                               │
│ CONTEXTO                                                      │
│ ─────────                                                     │
│ Necesitamos una base de datos para persistir el estado del    │
│ sistema. Tenemos opciones: PostgreSQL, MySQL, MongoDB,       │
│ DynamoDB.                                                     │
│                                                               │
│ DECISIÓN                                                       │
│ ───────                                                       │
│ Usaremos PostgreSQL 14+ como base de datos primaria.         │
│                                                               │
│ JUSTIFICACIÓN                                                  │
│ ─────────────                                                 │
│ • Soporte nativo para JSONB (semi-estructured data)           │
│ • Row-level security para multi-tenant                        │
│ • Madurez y comunidad establecida                             │
│ • Excelente performance para queries complejas                 │
│                                                               │
│ CONSECUENCI                                                    │
│ ───────────                                                   │
│ • Positivo: Eliminamos la complejidad de ORM para queries     │
│   complejas                                                   │
│ • Positivo: Row-level security nativo                         │
│ • Negativo: Requiere schema migrations más cuidadas          │
│ • Negativo: Menos flexible que NoSQL para cambios de schema   │
│                                                               │
│ CUMPLIMIENTO PARA AGENTES                                      │
│ ─────────────────────────────                                 │
│ REGLA: "Al generar código de acceso a datos, usar SQL         │
│ directo con psycopg2, NO usar ORM (SQLAlchemy está prohibido  │
│ para queries complejas)"                                      │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

### 3.2 Agente Lee ADRs y Genera Código Consistente

El patrón de **agente lee ADRs** implica que antes de generar código, el agente consulta la base de decisiones arquitectónicas y ajusta su generación соответственно.

```
┌─────────────────────────────────────────────────────────────────┐
│              FLUJO: AGENTE LEE ADRs ANTES DE GENERAR            │
└─────────────────────────────────────────────────────────────────┘

   USUARIO
      │
      │ "Genera el endpoint para crear usuarios"
      ▼
   ┌─────────────────────────────────────────────────────────┐
   │                      AGENTE IA                          │
   │                                                         │
   │  1. PARSE Request                                       │
   │     └── Entender: "Crear endpoint POST /users"         │
   │                                                         │
   │  2. QUERY ADRs                                          │
   │     ┌─────────────────────────────────────────────┐    │
   │     │ SELECT * FROM adrs                           │    │
   │     │ WHERE applicability MATCHES "API endpoint"   │    │
   │     │    OR applicability MATCHES "data access"    │    │
   │     └─────────────────────────────────────────────┘    │
   │                                                         │
   │     Resultado:                                          │
   │     • ADR-0015: "API REST con FastAPI"                  │
   │     • ADR-0023: "Validación con Pydantic"               │
   │     • ADR-0042: "PostgreSQL con psycopg2"               │
   │                                                         │
   │  3. EXTRACT CONSTRAINTS                                  │
   │     └── De ADR-0015: "Usar decorators @app.post()"     │
   │     └── De ADR-0023: "Modelos heredar de BaseModel"      │
   │     └── De ADR-0042: "NO SQLAlchemy, usar psycopg2"     │
   │                                                         │
   │  4. GENERATE CODE                                       │
   │     Genera código que cumple todas las constraints     │
   │                                                         │
   └─────────────────────────────────────────────────────────┘
      │
      │ Código generado con constraints aplicadas
      ▼
   VERIFICATION PASS
```

### 3.3 Violaciones de ADR y Feedback Estructurado

Cuando el agente genera código que viola un ADR, el sistema debe proporcionar retroalimentación estructurada que cite el ADR específico y explique la violación.

```python
# Ejemplo de feedback estructurado por violación de ADR
class ADRViolationFeedback:
    """
    Feedback estructurado cuando código generado viola un ADR.
    """

    def __init__(self, adr: ADR, violation: Violation):
        self.adr = adr
        self.violation = violation

    def format(self) -> str:
        return f"""
╔══════════════════════════════════════════════════════════════╗
║                    VIOLACIÓN DE ADR                          ║
╠══════════════════════════════════════════════════════════════╣
║                                                              ║
║  ADR #{self.adr.number}: {self.adr.title}                    ║
║  ─────────────────────────────────────────────────────────   ║
║                                                              ║
║  REGLA VIOLADA:                                              ║
║  {self.adr.constraint_rule}                                  ║
║                                                              ║
║  CÓDIGO GENERADO:                                            ║
║  {self.violation.code_snippet}                               ║
║  {self.violation.location}                                   ║
║                                                              ║
║  EXPLICACIÓN:                                                ║
║  {self.violation.explanation}                                ║
║                                                              ║
║  SUGERENCIA DE CORRECCIÓN:                                   ║
║  {self.violation.suggestion}                                 ║
║                                                              ║
║  DECISIÓN ORIGINAL (Contexto):                              ║
║  {self.adr.decision}                                        ║
║                                                              ║
║  JUSTIFICACIÓN:                                              ║
║  {self.adr.justification}                                   ║
║                                                              ║
╚══════════════════════════════════════════════════════════════╝
"""
```

### 3.4 Formato y Herramientas ADR

Formato estándar para ADRs con campos extendidos para consumo por agentes:

```markdown
# ADR-0042: PostgreSQL como base de datos primaria

## Estado
Aceptado (2024-01-15)

## Contexto
Necesitamos una base de datos para persistir el estado del sistema de agentes.
Las alternativas consideradas fueron PostgreSQL, MySQL, MongoDB y DynamoDB.

## Decisión
Usaremos PostgreSQL 14+ como base de datos primaria.

## Justificación
- Soporte nativo para JSONB para datos semi-estructurados
- Row-level security para multi-tenant
- Madurez y comunidad establecida
- Excelente performance para queries complejas

## Consecuencias
### Positivas
- Eliminamos complejidad de ORM para queries complejas
- Row-level security nativo

### Negativas
- Schema migrations más cuidadosas
- Menos flexible que NoSQL

## Metadata para Agentes
```yaml
agent_constraints:
  - rule: "Usar psycopg2 para queries SQL directas"
    prohibits:
      - "SQLAlchemy"
      - "Django ORM"
      - "ANY ORM para queries complejas"
    exception: "Simple CRUD puede usar SQLAlchemy"
  - rule: "Migrations con Alembic"
  - rule: "Connection pooling con psycopg2.pool"
```

## Herramientas Recomendadas

Para gestionar ADRs de manera efectiva:

| Herramienta | Propósito | Integración Agente |
|-------------|-----------|-------------------|
| adr-tools | CLI para crear/administrar ADRs | Compatible |
| ADR Viewer | Visualización de ADRs |阅 |
| adr-log | Historial de decisiones |触 |
| DocToolkit | Templates personalizados |触 |
```

---

## 4. Límites Hexagonales/DDD como Reglas

### 4.1 Bounded Contexts y su Importancia

El **Bounded Context** es un patrón fundamental de Domain-Driven Design (DDD) que define los límites lógicos dentro de los cuales existe un modelo de dominio específico. Cada bounded context tiene su propio modelo de dominio, su propio lenguaje ubicuo y es responsable de una preocupación específica del negocio.

En el contexto de agentes IA y rule alignment, los bounded contexts definen **fronteras de responsabilidad** que el agente no debe cruzar violando dependencias.

```
┌─────────────────────────────────────────────────────────────────┐
│                  BOUNDED CONTEXTS EN DDD                       │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                    MICROSERVICIO                            │
│                    "Order Management"                        │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              BOUNDED CONTEXT                         │  │
│  │                   "Orders"                             │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │              DOMAIN CORE                       │  │  │
│  │  │                                               │  │  │
│  │  │   Order ──────► OrderLine                     │  │  │
│  │  │     │            │                            │  │  │
│  │  │     │            ▼                            │  │  │
│  │  │     └──────► OrderService                     │  │  │
│  │  │                   │                            │  │  │
│  │  │                   ▼                            │  │  │
│  │  │           Domain Events                        │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │                                                       │  │
│  │  PUERTOS (Interfaces)         ADAPTADORES             │  │
│  │  ┌──────────────────┐        ┌──────────────────┐   │  │
│  │  │ IOrderRepository  │        │ PostgreSQLAdapter │   │  │
│  │  │ IEventPublisher  │        │ KafkaAdapter       │   │  │
│  │  └──────────────────┘        └──────────────────┘   │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ FRONTERA IMPERMEABLE
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    MICROSERVICIO                            │
│                    "User Management"                        │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              BOUNDED CONTEXT                         │  │
│  │                   "Users"                             │  │
│  │  ...                                                 │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 La Regla: "Domain Must Not Depend on Infrastructure"

Esta es quizás la regla más importante de la arquitectura hexagonal: **el dominio (核心 de negocio) nunca debe depender de infraestructura** (frameworks, bases de datos, APIs externas). Esta regla garantiza que el código de negocio sea testeable, reutilizable y independiente de tecnológicas específicas.

```
┌─────────────────────────────────────────────────────────────────┐
│     REGLA: DOMAIN ⊄ INFRASTRUCTURE                              │
│     "El dominio nunca debe depender de infraestructura"         │
└─────────────────────────────────────────────────────────────────┘

                    CORRECTO                          INCORRECTO
                    ════════                          ═══════════

            ┌───────────────┐                ┌───────────────┐
            │    DOMAIN     │                │    DOMAIN     │
            │               │                │               │
            │  OrderService │                │  OrderService │
            │       │       │                │       │       │
            │       │       │                │       │       │
            │       ▼       │                │       ▼       │
            │ IOrderRepo    │                │ PostgreSQL    │ ◄─── VIOLACIÓN
            │ (interface)    │                │RepoImpl       │
            └───────────────┘                └───────────────┘
                   │                                  ▲
                   │                                  │
                   ▼                                  │
            ┌───────────────┐                        │
            │ INFRASTRUCTURE│                        │
            │               │                        │
            │ PostgreSQL     │────────────────────────┘
            │ RepoImpl      │
            └───────────────┘


    ¿Por qué es importante?
    ═══════════════════════
    1. Testabilidad: El dominio puede testearse sin base de datos
    2.复用: El dominio es independiente de frameworks
    3. Evolución: Puedo cambiar PostgreSQL por DynamoDB
                   sin modificar el dominio
    4. Claridad: Las dependencias son explícitas
```

### 4.3 AST Analysis para Verificación de Dependencias

La verificación automatizada de esta regla puede implementarse mediante **AST (Abstract Syntax Tree) analysis**. El siguiente patrón muestra cómo verificar que el dominio no importe directamente módulos de infraestructura.

```python
# Verificador de dependencias usando AST
class DomainInfrastructureChecker:
    """
    Verifica que el dominio no dependa de infraestructura
    usando análisis de imports del AST.
    """

    def __init__(self, domain_package: str, infrastructure_packages: List[str]):
        self.domain_package = domain_package
        self.infrastructure_packages = infrastructure_packages

    def check_file(self, file_path: str) -> List[Violation]:
        """
        Analiza un archivo Python y reporta violaciones
        de la regla domain-no-infra.
        """
        violations = []
        tree = ast.parse(Path(file_path).read_text())

        # Detectar si el archivo pertenece al dominio
        is_domain_file = file_path.startswith(self.domain_package)

        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    if self._is_infrastructure(alias.name):
                        if is_domain_file:
                            violations.append(Violation(
                                type="DOMAIN_INFRASTRUCTURE_DEPENDENCY",
                                file=file_path,
                                import_name=alias.name,
                                message=f"Domain importa infraestructura: {alias.name}"
                            ))

            elif isinstance(node, ast.ImportFrom):
                if node.module and self._is_infrastructure(node.module):
                    if is_domain_file:
                        violations.append(Violation(
                            type="DOMAIN_INFRASTRUCTURE_DEPENDENCY",
                            file=file_path,
                            import_name=node.module,
                            message=f"Domain importa infraestructura: {node.module}"
                        ))

        return violations

    def _is_infrastructure(self, module_name: str) -> bool:
        """Verifica si un módulo pertenece a infraestructura."""
        return any(
            infra in module_name
            for infra in self.infrastructure_packages
        )

    def check_project(self, project_root: str) -> List[Violation]:
        """Verifica todo el proyecto."""
        all_violations = []
        for py_file in Path(project_root).rglob("*.py"):
            violations = self.check_file(str(py_file))
            all_violations.extend(violations)
        return all_violations


# Uso típico
checker = DomainInfrastructureChecker(
    domain_package="src/domain",
    infrastructure_packages=[
        "django",
        "sqlalchemy",
        "psycopg2",
        "redis",
        "boto3",
        "requests",
        "fastapi",  # Solo si se considera infraestructura
    ]
)

violations = checker.check_project("src/")
for v in violations:
    print(f"✗ {v.file}: {v.message}")
```

### 4.4 Verificación de Arquitectura con Import Graphs

La verificación de dependencias puede formalizarse mediante **grafos de imports** que revelan visualmente las violaciones arquitectónicas.

```
┌─────────────────────────────────────────────────────────────────┐
│                    GRAFO DE DEPENDENCIAS                        │
│                    (Verificación Automatizada)                   │
└─────────────────────────────────────────────────────────────────┘

    src/
    ├── domain/
    │   ├── models/
    │   │   ├── Order.py         ┌─────────────────────┐
    │   │   └── User.py          │  models/             │
    │   ├── services/            │  ├── Order.py        │
    │   │   └── OrderService.py  │  │   └── imports: []  │
    │   └── ports/               │  │        (✓ Clean)   │
    │       └── IOrderRepo.py    │  └─────────────────────┘
    │                               │          │
    └── infrastructure/                │          │
        ├── adapters/                 │          ▼
        │   └── PostgresRepo.py       │  ┌─────────────────────┐
        └── database/                 │  │  services/          │
            └── connection.py         │  │  OrderService.py    │
                                     │  │  └── imports:      │
                                     │  │    - models.Order   │
                                     │  │    - ports.IOrderRepo│
                                     │  └─────────────────────┘
                                     │          │
                                     │          ▼
                                     │  ┌─────────────────────┐
                                     │  │  infrastructure/    │
                                     │  │  adapters/          │
                                     │  │  PostgresRepo.py   │
                                     │  │  └── imports:      │
                                     │  │    - psycopg2       │
                                     │  │    - ports.IOrderRepo│
                                     │  └─────────────────────┘
                                     │
                                     ▼
                              REGLA VERIFICADA:
                              "infraestructura importa dominio ✓"
                              "dominio NO importa infraestructura ✓"
```

---

## 5. Entropía SOLID/Connascence

### 5.1 Cuantificación de Calidad de Diseño

La **entropía de diseño** es una medida de la degradación de la calidad del software a lo largo del tiempo. Así como la entropía termodinámica mide el desorden, la entropía de software mide el caos estructural: dependencias circulares, acoplamiento estrecho, responsabilidad difusa.

El enfoque **SOLID/Connascence** proporciona métricas cuantificables para evaluar y hacer cumplir la calidad del diseño.

```
┌─────────────────────────────────────────────────────────────────┐
│                    PRINCIPIOS SOLID                             │
│                    (Mnemotecnia para Buenas Prácticas)          │
└─────────────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────────┐
│                                                               │
│   S - Single Responsibility Principle (SRP)                   │
│   ──────────────────────────────────────────────              │
│   "Una clase debe tener una sola razón para cambiar"          │
│                                                               │
│   Ejemplo bueno:                                              │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│   │ UserAuth    │  │ UserProfile │  │ UserRepository│       │
│   │ (login)     │  │ (datos)     │  │ (persist)    │         │
│   └─────────────┘  └─────────────┘  └─────────────┘         │
│                                                               │
│   Ejemplo malo:                                               │
│   ┌─────────────────────────────┐                            │
│   │ UserManager                 │ ◄─── Múltiples razones     │
│   │ - login()                   │      para cambiar          │
│   │ - updateProfile()           │                             │
│   │ - saveToDatabase()          │                             │
│   │ - sendEmail()               │                             │
│   └─────────────────────────────┘                            │
│                                                               │
├───────────────────────────────────────────────────────────────┤
│                                                               │
│   O - Open/Closed Principle (OCP)                            │
│   ─────────────────────────────────                          │
│   "Entidades de software deben estar abiertas para           │
│    extensión pero cerradas para modificación"                │
│                                                               │
│   Strategy Pattern:                                          │
│   ┌─────────────────┐                                        │
│   │ Context         │◄────────────────┐                      │
│   │ - algorithm     │                 │                      │
│   │ + execute()     │         ┌───────┴───────┐             │
│   └─────────────────┘         │   Algorithm    │             │
│                               │   (interface)  │             │
│                               └───────┬───────┘             │
│                               ┌───────┴───────┐             │
│                      ┌────────┤         ├────────┐         │
│                      ▼                 ▼                  │
│               ┌──────────┐    ┌──────────┐              │
│               │ QuickSort │    │ MergeSort │              │
│               └──────────┘    └──────────┘              │
│                                                               │
├───────────────────────────────────────────────────────────────┤
│                                                               │
│   L - Liskov Substitution Principle (LSP)                   │
│   ────────────────────────────────────────                   │
│   "Objetos de una superclase deben poder sustituir          │
│    objetos de una subclase sin alterar correctitud"           │
│                                                               │
│                          ┌─────────────┐                     │
│                          │  Rectangle  │                     │
│                          │ +w          │                     │
│                          │ +h          │                     │
│                          │ +setWidth()  │                     │
│                          │ +setHeight() │                     │
│                          └─────────────┘                     │
│                                 ▲                             │
│                                 │ extends                    │
│                          ┌─────────────┐                     │
│                          │   Square    │                     │
│                          │             │                     │
│                          │ LSP VIOLADO │ ◄── setWidth()      │
│                          │ si w!=h     │     cambia h       │
│                          └─────────────┘                     │
│                                                               │
├───────────────────────────────────────────────────────────────┤
│                                                               │
│   I - Interface Segregation Principle (ISP)                   │
│   ─────────────────────────────────────────                  │
│   "Los clientes no deben depender de interfaces              │
│    que no usan"                                              │
│                                                               │
│   MAL (fat interface):        BIEN (role interfaces):        │
│   ┌─────────────────┐         ┌───────────┐                  │
│   │ IMachine       │         │ IPrinter  │                  │
│   │ + print()      │         │ +print()  │                  │
│   │ + scan()       │         └───────────┘                  │
│   │ + fax()        │         ┌───────────┐                  │
│   │ + copy()       │         │ IScanner  │                  │
│   └────────┬────────┘         │ +scan()   │                  │
│            │                  └───────────┘                  │
│    ┌───────┴───────┐         ┌───────────┐                  │
│    │ OldPrinter    │         │ MultiFunc │                  │
│    │ (solo print)  │         │ +print()  │                  │
│    │ VIOLACIÓN     │         │ +scan()   │                  │
│    └───────────────┘         └───────────┘                  │
│                                                               │
├───────────────────────────────────────────────────────────────┤
│                                                               │
│   D - Dependency Inversion Principle (DIP)                   │
│   ─────────────────────────────────────────                  │
│   "Módulos de alto nivel no deben depender de                │
│    módulos de bajo nivel"                                    │
│                                                               │
│                    ┌─────────────────┐                       │
│                    │  High-Level      │                       │
│                    │  Policy          │                       │
│                    │        │        │                       │
│                    │        ▼        │                       │
│                    │  IRepository    │ ◄── abstracción      │
│                    │  (interface)    │                       │
│                    └────────┬────────┘                       │
│                             │                                │
│              ┌──────────────┼──────────────┐                 │
│              ▼                             ▼                  │
│     ┌─────────────────┐       ┌─────────────────┐          │
│     │  MySQLRepo      │       │  MongoRepo      │          │
│     │ (low-level)     │       │ (low-level)      │          │
│     └─────────────────┘       └─────────────────┘          │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

### 5.2 Métricas de Connascence

**Connascence** (acoplamiento connascente) es una métrica formalizada por Meilir Page-Jones que mide el acoplamiento entre componentes. Existen dos categorías principales:

**Connascence Estática** ( puede detectarse en tiempo de compilación):
- **Connascence de Nombre (Nm)**: Múltiples componentes usan el mismo nombre
- **Connascence de Posición (Np)**: Múltiples componentes usan el mismo orden de parámetros
- **Connascence de Algoritmo (Na)**: Componentes deben usar el mismo algoritmo

**Connascence Dinámica** (solo detectable en tiempo de ejecución):
- **Connascence de Ejecución (E)**: Orden de ejecución es crítico
- **Connascence de Tiempo (T)**: Timing de ejecución es crítico
- **Connascence de Valor (V)**: Restricciones de valor entre componentes
- **Connascence de Identidad (I)**: Deben compartir identidad de objeto

```
┌─────────────────────────────────────────────────────────────────┐
│                    TIPOS DE CONNASCENCE                         │
│                    (Ordenados porseveridad)                     │
└─────────────────────────────────────────────────────────────────┘

    MENOS SEVERO                                          MÁS SEVERO
    ══════════                                            ════════════

    ┌─────────────────────────────────────────────────────────────┐
    │  CONNASCENCE ESTÁTICO (Compilación)                        │
    │  ───────────────────────────────────────                    │
    │                                                             │
    │  Nm - Nombre:  ┌────────────────┐                          │
    │                │ func foo(x, y) │                          │
    │                │ func bar(x, y) │  ◄── mismo nombre        │
    │                └────────────────┘    pero diferentes       │
    │                                      módulos               │
    │                                                             │
    │  Np - Posición: ┌────────────────┐                        │
    │                 │ foo(userId,    │                        │
    │                 │         name)  │  ◄── mismo orden       │
    │                 │ bar(userId,    │      obligatorio       │
    │                 │         name)  │                        │
    │                 └────────────────┘                        │
    │                                                             │
    │  Na - Algoritmo:┌────────────────┐                         │
    │                │  calculateTax() │                        │
    │                │  must use same   │ ◄── mismo algoritmo    │
    │                │  formula as     │      requerido         │
    │                │  BillingModule  │                        │
    │                └────────────────┘                        │
    └─────────────────────────────────────────────────────────────┘

    ┌─────────────────────────────────────────────────────────────┐
    │  CONNASCENCE DINÁMICO (Runtime)                            │
    │  ─────────────────────────────────                          │
    │                                                             │
    │  E - Ejecución: ┌──────────────────────────────────┐       │
    │                 │ createOrder() debe ejecutarse     │       │
    │                 │ ANTES de calculateTotal()         │       │
    │                 └──────────────────────────────────┘       │
    │                                                             │
    │  T - Tiempo:    ┌──────────────────────────────────┐       │
    │                 │ El callback debe ejecutarse       │       │
    │                 │ en < 100ms o el timeout expira   │       │
    │                 └──────────────────────────────────┘       │
    │                                                             │
    │  V - Valor:     ┌──────────────────────────────────┐       │
    │                 │ balance >= 0 siempre que         │       │
    │                 │ creditLimit > 0                  │       │
    │                 └──────────────────────────────────┘       │
    │                                                             │
    │  I - Identidad: ┌──────────────────────────────────┐       │
    │                 │ Ambas funciones deben recibir     │       │
    │                 │ la MISMA instancia de db_conn    │       │
    │                 └──────────────────────────────────┘       │
    │                                                             │
    │  ★ I es la más severa porque requiere compartir            │
    │    identidad de objeto específico                          │
    └─────────────────────────────────────────────────────────────┘


    FÓRMULA DE SEVERIDAD:
    ════════════════════
    Severity = Connascence_Type_Weight × Number_of_Components

    Donde:
    - Nm tiene weight = 1 (menos severo)
    - I tiene weight = 10 (más severo)
```

### 5.3 Violaciones SOLID como Quality Gates

Las violaciones de principios SOLID pueden transformarse en **quality gates** que bloquean la integración de código o generan retroalimentación automática.

```
┌─────────────────────────────────────────────────────────────────┐
│                    QUALITY GATES                               │
│            (Bloqueo basado en Principios SOLID)                  │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  GATE 1: SRP Checker                                           │
│  ────────────────────────                                       │
│  Analiza: "Número de razones para cambiar por clase"           │
│  Threshold: Max 1 razón por clase                              │
│  Métrica: Counting method responsibilities                     │
│                                                               │
│  Violación → DENY con理由:                                     │
│  "UserService tiene 5 razones para cambiar:                    │
│   1. Autenticación 2. Perfil 3. Persistencia 4. Email 5. Cache  │
│   Debe dividirse en: UserAuth, UserProfile, UserRepo,         │
│   EmailService, UserCache"                                     │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  GATE 2: OCP Checker                                           │
│  ────────────────────────                                       │
│  Analiza: "Extensibilidad sin modificación"                    │
│  Threshold: 0 violaciones de "open for extension"              │
│  Métrica: Detección de instanceof/type checks                  │
│                                                               │
│  Violación → DENY con理由:                                     │
│  "PaymentProcessor usa if/elif/else para tipos de pago.        │
│   Usa Strategy pattern: IPaymentStrategy con implementaciones  │
│   CreditCardStrategy, PayPalStrategy, CryptoStrategy"          │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  GATE 3: LSP Checker                                           │
│  ────────────────────────                                       │
│  Analiza: "Subtypes sustituíbles por supertypes"               │
│  Threshold: 0 violaciones de контра actual                    │
│  Métrica: Behavioral subtyping test                            │
│                                                               │
│  Violación → DENY con理由:                                     │
│  "Square.setWidth() viola LSP: altera height que Rectangle     │
│   no hace. Rectangle invariants incluyen w!=f(w) para         │
│   setWidth pero Square los viola"                              │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  GATE 4: ISP Checker                                           │
│  ────────────────────────                                       │
│  Analiza: "Interfaces cohesionadas"                            │
│  Threshold: Max 3 métodos por interfaz                         │
│  Métrica: Interface size analysis                              │
│                                                               │
│  Violación → DENY con理由:                                     │
│  "IMachine tiene 15 métodos. Divídela en: IPrinter (3),       │
│   IScanner (2), IFax (4), ICopier (2), IStapler (1)"          │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  GATE 5: DIP Checker                                           │
│  ────────────────────────                                       │
│  Analiza: "Dependencias sobre abstracciones"                   │
│  Threshold: Domain no depende de Infrastructure                │
│  Métrica: Import graph analysis                                │
│                                                               │
│  Violación → DENY con理由:                                     │
│  "domain/services/OrderService.py importa                     │
│   infrastructure/database/RedisCache.py. Domain debe           │
│   depender solo de ports (interfaces)"                          │
└─────────────────────────────────────────────────────────────────┘
```

### 5.4 De Métricas a Reglas Enforzables

La transformación de métricas cualitativas en reglas automáticas requiere definir **threshold boundaries** y **actions** para cada violación.

```yaml
# Configuración de Quality Gates
quality_gates:
  solid:
    SRP:
      metric: "responsibility_count_per_class"
      threshold: 1
      action: "DENY"
      severity: "HIGH"
      description: "Cada clase debe tener exactamente una responsabilidad"

    OCP:
      metric: "type_check_count_in_methods"
      threshold: 0
      action: "DENY"
      severity: "HIGH"
      description: "No usar type checks para extender comportamiento"

    LSP:
      metric: "liskov_violation_count"
      threshold: 0
      action: "DENY"
      severity: "CRITICAL"
      description: "Subclases deben mantener контра de superclase"

    ISP:
      metric: "method_count_per_interface"
      threshold: 5
      action: "WARN"
      description: "Interfaces grandes deben dividirse"

    DIP:
      metric: "domain_imports_infrastructure"
      threshold: 0
      action: "DENY"
      severity: "CRITICAL"
      description: "Domain no puede depender de Infrastructure"

  connascence:
   Nm:
      metric: "shared_names_cross_modules"
      threshold: 0
      action: "WARN"
      description: "Nombres compartidos deben evitarse"

    Np:
      metric: "parameter_order_coupling"
      threshold: 3
      action: "ADVISORY"
      description: "Orden de parámetros acoplado"

    Na:
      metric: "algorithm_coupling_count"
      threshold: 0
      action: "DENY"
      severity: "MEDIUM"
      description: "Mismo algoritmo no debe requerirse"

    E:
      metric: "execution_order_violations"
      threshold: 0
      action: "DENY"
      severity: "HIGH"
      description: "Orden de ejecución debe garantizarse"
```

---

## 6. Arquitectura de Bucle de Retroalimentación Reflexión

### 6.1 El Patrón Reflexion: Action → Evaluate → Deny → Reflect → Revise

El patrón **Reflexion** (self-reflection) es un bucle de control donde un agente examina su propiooutput y lo compara contra estándares definidos, corrigiendo automáticamente cuando detecta desviaciones. A diferencia de linting simple, reflexion implica **razonamiento metacognitivo** sobre el proceso de generación.

```
┌─────────────────────────────────────────────────────────────────┐
│            ARQUITECTURA DE BUCLE REFLEXION                      │
│            (Detailed Flow)                                      │
└─────────────────────────────────────────────────────────────────┘

   ┌─────────────────────────────────────────────────────────────┐
   │                                                             │
   │                    ┌──────────────┐                         │
   │                    │    START     │                         │
   │                    │   User Task  │                         │
   │                    └──────┬───────┘                         │
   │                           │                                 │
   │                           ▼                                 │
   │              ┌─────────────────────────┐                    │
   │              │      ACTION            │                    │
   │              │  Agent generates code   │                    │
   │              │  or executes command    │                    │
   │              └────────────┬────────────┘                    │
   │                           │                                 │
   │                           ▼                                 │
   │              ┌─────────────────────────┐                    │
   │              │      EVALUATE           │                    │
   │              │  Compare against:       │                    │
   │              │  - ADRs                 │                    │
   │              │  - Invariants           │                    │
   │              │  - SOLID principles     │                    │
   │              │  - Connascence metrics  │                    │
   │              │  - Policy engine rules   │                    │
   │              └────────────┬────────────┘                    │
   │                           │                                 │
   │                           ▼                                 │
   │              ┌─────────────────────────┐                    │
   │              │      DECISION           │                    │
   │              │  All checks pass?      │                    │
   │              └────────────┬────────────┘                    │
   │                           │                                 │
   │              ┌────────────┴────────────┐                     │
   │              │ YES                     │ NO                  │
   │              ▼                        ▼                     │
   │    ┌─────────────────┐      ┌─────────────────┐              │
   │    │     ALLOW      │      │      DENY       │              │
   │    │  Execute/      │      │  Stop execution │              │
   │    │  Commit        │      │  Return error   │              │
   │    └─────────────────┘      └────────┬────────┘              │
   │                                        │                       │
   │                                        ▼                       │
   │                            ┌─────────────────────┐            │
   │                            │      REFLECT        │            │
   │                            │                     │            │
   │                            │ 1. What went wrong? │            │
   │                            │ 2. Why did it       │            │
   │                            │    violate rule?   │            │
   │                            │ 3. How to fix?     │            │
   │                            │                     │            │
   │                            │ Generates:         │            │
   │                            │ - Explanation      │            │
   │                            │ - Corrected output │            │
   │                            │ - Learning for     │            │
   │                            │   future similar   │            │
   │                            │   situations       │            │
   │                            └──────────┬──────────┘            │
   │                                       │                       │
   │                                       ▼                       │
   │                            ┌─────────────────────┐            │
   │                            │      REVISE         │            │
   │                            │  Agent generates    │            │
   │                            │  corrected version  │            │
   │                            └──────────┬──────────┘            │
   │                                       │                       │
   │                                       ▼                       │
   │                            ┌─────────────────────┐            │
   │                            │    RE-EVALUATE      │            │
   │                            │  Loop back to       │            │
   │                            │  EVALUATE step      │            │
   │                            └──────────┬──────────┘            │
   │                                       │                       │
   │                                       │                       │
   │                    ┌──────────────────┴───────────────┐      │
   │                    │        MAX ITERATIONS?           │      │
   │                    └──────────────────┬───────────────┘      │
   │                              ┌────────┴────────┐             │
   │                              │ YES             │ NO           │
   │                              ▼                 ▼             │
   │                   ┌─────────────────┐  ┌─────────────────┐  │
   │                   │    ESCALATE     │  │ Continue loop   │  │
   │                   │  to human/user  │  │                 │  │
   │                   │  with full      │  │                 │  │
   │                   │  context        │  │                 │  │
   │                   └─────────────────┘  └─────────────────┘  │
   │                                                             │
   └─────────────────────────────────────────────────────────────┘
```

### 6.2 Diferenciación con Linting Simple

El bucle de reflexión difiere fundamentalmente del linting tradicional en varios aspectos:

| Característica | Linting Simple | Bucle Reflexión |
|----------------|----------------|------------------|
| **Feedback** | Static report | Interactive dialogue |
| **Adaptación** | No aprende | Aprende de errores |
| **Contexto** | Individual file | Full session context |
| **Resolución** | User fixes manually | Agent self-corrects |
| **Metacognición** | No | Yes (razona sobre su razonamiento) |
| **Estado** | Stateless | Maintains state across iterations |
| **IA Integration** | Rule-based only | AI-powered analysis |

```
┌─────────────────────────────────────────────────────────────────┐
│         LINING SIMPLE vs BUCLE DE REFLEXION                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  LINTING SIMPLE:                                               │
│  ═══════════════                                               │
│                                                                 │
│  Code → [Linter] → Report → Human reads → Human fixes         │
│                         │                                      │
│                         ▼                                      │
│                   ┌─────────────┐                              │
│                   │ Errors: 5   │                              │
│                   │ Warnings: 12│                              │
│                   │ Line 42:... │                              │
│                   └─────────────┘                              │
│                                                                 │
│  ────────────────────────────────────────────────────────────  │
│                                                                 │
│  BUCLE DE REFLEXION:                                           │
│  ════════════════════                                         │
│                                                                 │
│  Code → [Agent] → [Evaluate] → [Deny] → [Reflect] → [Revise]  │
│                    ▲                                           │
│                    │                                           │
│                    └────────────────────────────────────        │
│                        (loop until pass)                       │
│                                                                 │
│  Dialogue:                                                     │
│  ─────────                                                     │
│  Agent: "Generé código que usa string concatenation en SQL"   │
│  System: "Violación: SQL_INJECTION_PREVENTION. Usa parameter- │
│           ized queries."                                       │
│  Agent: "Entendido. Revisando... Usaré psycopg2 con ?"        │
│  System: "Verificado: PASS. Código cumple SQL_INJECTION_PREV"  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 6.3 Integración con Policy Engines

El bucle de reflexión puede integrarse con **policy engines** como Open Policy Agent (OPA), Casbin, o AWS IAM para crear sistemas de gobierno escalables y auditables.

```
┌─────────────────────────────────────────────────────────────────┐
│          INTEGRACIÓN REFLEXION + POLICY ENGINE                  │
└─────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                     AGENTE IA                                 │
│                     ═════════                                 │
│                                                              │
│    User Task                                                   │
│        │                                                       │
│        ▼                                                       │
│  ┌──────────────┐                                             │
│  │    ACTION    │                                             │
│  │  Generate    │                                             │
│  │  code/cmd    │                                             │
│  └──────┬───────┘                                             │
│         │                                                      │
│         ▼                                                      │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │                  EVALUATION LAYER                        │ │
│  │  ┌─────────────────┐    ┌─────────────────┐              │ │
│  │  │   LOCAL RULES   │    │  POLICY ENGINE  │              │ │
│  │  │  (SOLID, Invars)│    │     (OPA)       │              │ │
│  │  │                 │    │                 │              │ │
│  │  │ • SRP check     │    │ rego: allow if  │              │ │
│  │  │ • OCP check     │    │   input.user.   │              │ │
│  │  │ • DIP check     │    │   role in       │              │ │
│  │  │ • Connascence   │    │   allowed_roles │              │ │
│  │  │                 │    │                 │              │ │
│  │  └────────┬────────┘    └────────┬────────┘              │ │
│  │           │                      │                        │ │
│  │           │    ┌─────────────────┘                        │ │
│  │           └────►│                                           │ │
│  │                ▼                                           │ │
│  │         ┌────────────────────────────────┐               │ │
│  │         │     POLICY DECISION POINT     │               │ │
│  │         │                                │               │ │
│  │         │  ALL_LOCAL_PASSED &&           │               │ │
│  │         │  OPA_ALLOWS ?                  │               │ │
│  │         │                                │               │ │
│  │         │  YES → ALLOW                   │               │ │
│  │         │  NO  → DENY + EXPLANATION      │               │ │
│  │         └────────────────┬─────────────────┘               │ │
│  │                          │                                │ │
│  └──────────────────────────┼────────────────────────────────┘ │
│                             │                                  │
│              ┌──────────────┴──────────────┐                   │
│              │ YES                         │ NO                │
│              ▼                             ▼                   │
│    ┌─────────────────┐          ┌─────────────────┐         │
│    │     ALLOW       │          │      DENY       │         │
│    │  Execute/       │          │  Return error   │         │
│    │  Commit         │          │  with policy    │         │
│    │                 │          │  explanation     │         │
│    └─────────────────┘          └────────┬────────┘         │
│                                           │                   │
│                                           ▼                   │
│                                 ┌─────────────────┐          │
│                                 │     REFLECT     │          │
│                                 │  Analyze why    │          │
│                                 │  policy denied  │          │
│                                 └────────┬────────┘          │
│                                          │                   │
│                                          ▼                   │
│                                 ┌─────────────────┐          │
│                                 │     REVISE      │          │
│                                 │  Adjust code    │          │
│                                 │  to comply      │          │
│                                 └────────┬────────┘          │
│                                          │                   │
│                                          │ Loop back         │
│                                          ▼                   │
│                                   [RE-EVALUATE]              │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 6.4 Ejemplo de Integración OPA

```rego
# Ejemplo de política OPA para governance de agentes
package agent.governance

# Reglas básicas de seguridad
allow {
    not denied
    input.action != " BLOCKED_ACTION"
}

denied {
    some violation
    violations[_].severity == "CRITICAL"
}

denied {
    input.action == "execute_external_code"
    not whitelisted_binary[input.binary_path]
}

denied {
    input.action == "read_file"
    startswith(input.path, "/etc")
}

denied {
    input.action == "sql_query"
    contains(input.query, "DROP TABLE")
}

# Reglas de acceso a archivos
whitelisted_binary = {
    "/usr/bin/python3",
    "/usr/local/bin/node",
    "/bin/bash"
}

# Reglas de workspace
allowed_workspace_paths[path] {
    path := input.workspace_root
    endswith(path, "project/src")
}

# Reglas de rate limiting
rate_limited {
    count(requests_this_minute) < 60
}

requests_this_minute[req] {
    req := input.recent_requests[_]
    req.timestamp > time.now_ns() - 60000000000
}
```

---

## 7. Design by Contract para IA

### 7.1 Precondiciones: "Agent Must Read CLAUDE.md"

En **Design by Contract (DbC)**, las precondiciones son condiciones que deben cumplirse **antes** de que un método o servicio ejecute su lógica principal. Para agentes IA, las precondiciones definen el **contexto requerido** antes de comenzar trabajo.

```
┌─────────────────────────────────────────────────────────────────┐
│              PRECONDICIONES PARA AGENTES IA                     │
└─────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────┐
│                                                                │
│  CONTRATO: AgentEditsCode                                      │
│  ════════════════════════════════════                          │
│                                                                │
│  PRECONDICIONES (antes de editar):                            │
│  ─────────────────────────────────────                         │
│                                                                │
│  1. "Agent debe leer CLAUDE.md antes de editar"               │
│     └── Verificación: session.has_read("CLAUDE.md")           │
│                                                                │
│  2. "Agent debe entender el dominio del proyecto"             │
│     └── Verificación: context.has_domain_context              │
│                                                                │
│  3. "Agent debe conocer las convenciones de código"           │
│     └── Verificación: rules.has_been_loaded                    │
│                                                                │
│  4. "Si edita security-critical code, debe haber contexto"      │
│     └── Verificación: context.has_security_clearance           │
│                                                                │
│  EJEMPLO PRÁCTICO:                                            │
│  ═══════════════════                                          │
│                                                                │
│  Before editing:                                              │
│  ┌─────────────────────────────────────────────────────┐       │
│  │ CLAUDE.md exists? ──NO──► Agent reads CLAUDE.md    │       │
│  │        │                                      │    │       │
│  │       YES                                     │    │       │
│  │        │                                      │    │       │
│  │        ▼                                      │    │       │
│  │ preconditions_met? ───NO──► Return error:     │    │       │
│  │        │                        "Read CLAUDE.md│    │       │
│  │       YES                        first"        │    │       │
│  │        │                                      │    │       │
│  │        ▼                                      │    │       │
│  │ ┌───────────────────┐                       │    │       │
│  │ │ Proceed with edit  │◄──────────────────────┘    │       │
│  │ └───────────────────┘                              │       │
│  └─────────────────────────────────────────────────────┘       │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### 7.2 Postcondiciones: "Generated Code Must Pass Linting"

Las **postcondiciones** son condiciones que deben cumplirse **después** de que una operación complete. Para agentes IA, las postcondiciones garantizan que el output cumpla estándares mínimos.

```python
# Patrón: Contrato para generación de código
class CodeGenerationContract:
    """
    Design by Contract para generación de código por agentes.
    """

    @staticmethod
    def verify_postconditions(
        generated_code: str,
        context: GenerationContext
    ) -> ContractResult:
        """
        Verifica postcondiciones después de generar código.
        """

        violations = []

        # Postcondición 1: Código pasa linting
        lint_result = run_linter(generated_code)
        if not lint_result.passed:
            violations.append(ContractViolation(
                type="POSTCONDITION",
                rule="linting_pass",
                message=f"Generated code failed linting: {lint_result.errors}",
                severity="HIGH"
            ))

        # Postcondición 2: Código tiene tests
        if not has_test_coverage(generated_code):
            violations.append(ContractViolation(
                type="POSTCONDITION",
                rule="has_tests",
                message="Generated code must include unit tests",
                severity="MEDIUM"
            ))

        # Postcondición 3: Código no viola invariantes de seguridad
        security_check = security_invariant_checker.verify(generated_code)
        if not security_check.passed:
            violations.append(ContractViolation(
                type="POSTCONDITION",
                rule="security_invariants",
                message=f"Security invariant violated: {security_check.violations}",
                severity="CRITICAL"
            ))

        # Postcondición 4: Documentación existe
        if not has_docstring(generated_code):
            violations.append(ContractViolation(
                type="POSTCONDITION",
                rule="documented",
                message="Generated code must include docstrings",
                severity="LOW"
            ))

        return ContractResult(
            passed=len(violations) == 0,
            violations=violations
        )
```

### 7.3 Invariantes: "All API Handlers Include Input Validation"

Las **invariantes de clase** deben mantenerse verdaderas durante toda la existencia de objetos de una clase. Para agentes, definimos invariantes de sesión que deben mantenerse durante toda la interacción.

```
┌─────────────────────────────────────────────────────────────────┐
│              INVARIANTES PARA HANDLERS DE API                    │
└─────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────┐
│  INVARIANTE: "Todos los API handlers incluyen validación        │
│              de input"                                         │
│                                                                │
│  Verificación:                                                 │
│  ─────────────                                                 │
│                                                                │
│  Para cada endpoint en la OpenAPI spec:                        │
│                                                                │
│  1. ¿Tiene validación de tipos?                               │
│     → Si el schema define type: string, debe validar type      │
│                                                                │
│  2. ¿Tiene validación de rangos?                               │
│     → Si maxLength, minLength, minimum, maximum → validar      │
│                                                                │
│  3. ¿Tiene validación de patterns?                             │
│     → Si pattern definido → verificar regex match              │
│                                                                │
│  4. ¿Tiene validación de required fields?                       │
│     → Si schema.markers['required'] → verificar presencia      │
│                                                                │
│  IMPLEMENTACIÓN:                                               │
│  ══════════════                                                │
│                                                                │
│  @app.post("/users")                                           │
│  async def create_user(user: UserCreate):  # ← Pydantic valida │
│      # Si llega aquí, UserCreate invariants se cumplieron     │
│      pass                                                      │
│                                                                │
│  @app.post("/search")                                          │
│  async def search(query: str = Query(...)):  # ← FastAPI valida│
│      # Query invariants: min_length=1, max_length=100          │
│      pass                                                      │
│                                                                │
│  INVARIANT VIOLATION → DENY:                                   │
│  "Handler /users no valida input contra schema.               │
│   Usa Pydantic models o dependencias de Query/Body"           │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### 7.4 Contratos Formales para Agentes

Los contratos pueden formalizarse usando lenguajes de especificación como **TLA+** o **Dafny** para verificar corrección en tiempo de diseño.

```tla
-------------------------- MODULE AgentContract --------------------------

(* Contrato formal para un agente que edita archivos *)

VARIABLES
    agent_state,
    session_context,
    file_system

CONSTANTS
    WORKSPACE_ROOT,
    PROHIBITED_PATHS

\* Precondición: El agente debe haber leído las reglas del proyecto
PreconditionReadRules == 
    session_context.rules_read = TRUE

\* Precondición: El agente debe estar en workspace válido  
PreconditionValidWorkspace ==
    LET target = session_context.current_target IN
    \E path \in WORKSPACE_ROOT:
        path = target \/ SubSeq(path, 1, Len(path)) = target

\* Precondición: Archivo objetivo no está prohibido
PreconditionNotProhibited ==
    \A prohibited \in PROHIBITED_PATHS:
        session_context.current_target # prohibited

\* Postcondición: Archivo modificado mantiene sintaxis válida
PostconditionValidSyntax ==
    file_system[session_context.current_target].valid = TRUE

\* Postcondición: Archivo no creció más allá de límite razonable
PostconditionSizeConstraint ==
    file_system[session_context.current_target].size 
        <= file_system[session_context.current_target].original_size * 2

\* Postcondición: Timestamp actualizado
PostconditionTimestampUpdated ==
    file_system[session_context.current_target].last_modified 
        > session_context.action_start_time

\* Invariante: Archivo nunca debe estar en estado inconsistente
TypeInvariant ==
    \A file \in DOMAIN file_system:
        file_system[file].valid \/ file_system[file].locked

\* El contrato se cumple si todas las condiciones son válidas
ContractFulfilled ==
    /\ PreconditionReadRules
    /\ PreconditionValidWorkspace
    /\ PreconditionNotProhibited
    /\ PostconditionValidSyntax
    /\ PostconditionSizeConstraint
    /\ PostconditionTimestampUpdated

============================================================================
```

---

## 8. Conclusiones y Líneas Futuras

### 8.1 Síntesis de Patrones

Los patrones presentados en este documento forman un **ecosistema coherente** para la alineación de reglas en agentes IA:

1. **Sistemas Axiomáticos** proporcionan el fundamento teórico: la distinción entre hard rules (axiomas) y soft rules (guidelines), entre propiedades de safety y liveness.

2. **Invariantes** son las verdades inquebrantables que se preservan a través de bucles de verificación automatizada.

3. **ADRs** capturan decisiones pasadas como constraints heredadas, permitiendo que agentes respeten el contexto histórico de decisiones arquitectónicas.

4. **Arquitectura Hexagonal/DDD** define fronteras físicas y lógicas que los agentes no deben cruzar, implementables mediante AST analysis.

5. **SOLID/Connascence** cuantifica la calidad de diseño, transformando principios cualitativos en métricas verificables y quality gates blokqueantes.

6. **Bucle Reflexión** es el mecanismo de ejecución que conecta todos los componentes: acción, evaluación, negación, reflexión, revisión.

7. **Design by Contract** formaliza las expectativas sobre comportamiento de agentes mediante precondiciones, postcondiciones e invariantes.

### 8.2 Líneas de Investigación Futura

| Área | Pregunta de Investigación | Enfoque |
|------|--------------------------|---------|
| **Verificación Formal** | ¿Cómo verificar automáticamente que un agente cumple contratos formales? | TLA+, Dafny, model checking |
| **Aprendizaje de Reglas** | ¿Pueden los agentes inferir ADRs implícitos del código existente? | Mining de decisiones, pattern recognition |
| **Contratos Evolutivos** | ¿Cómo manejar cambios de reglas sin romper sesiones activas? | Versioning de contratos, rollback strategies |
| **Multi-Agente** | ¿Cómo coordinar contratos entre múltiples agentes con roles distintos? | Contract negotiation protocols, trust frameworks |
| **Auditoría** | ¿Cómo mantener logs inmutables de decisiones contractuales? | Blockchain-style logging, zero-knowledge proofs |
| **Adaptación** | ¿Pueden las reglas evolucionar basadas en feedback del entorno? | Reinforcement learning para policy optimization |

### 8.3 Recomendaciones de Implementación

Para organizaciones que buscan implementar estos patrones:

1. **Comenzar con Invariantes de Seguridad**: Las propiedades de safety son las más críticas y las más verificables automáticamente.

2. **Adoptar ADRs Incrementalmente**: Comenzar documentando decisiones nuevas, gradualmente migrar decisiones históricas.

3. **Implementar Quality Gates Graduales**: Primero warnings, luego denials para violations de nivel medio, finalmente denials automáticos para CRITICAL.

4. **Invertir en AST Analysis**: La capacidad de verificar dependencias y estructura de código es fundamental para todos los patrones.

5. **Diseñar para Retroalimentación**: Cada violación debe generar feedback actionable, no solo logs de error.

6. **Mantener Humanos en el Loop**: Para decisiones de alto impacto, mantener puntos de escalación humanos.

---

## Referencias y Lecturas Adicionales

- Nygard, M. - *Release It!* (Architecture Decision Records)
- Meyer, B. - *Object-Oriented Software Construction* (Design by Contract)
- Page-Jones, M. - *Fundamentals of Object-Oriented Design in UML* (Connascence)
- Martin, R. - *Clean Architecture* (SOLID principles)
- Evans, E. - *Domain-Driven Design* (Bounded Contexts)
- Open Policy Agent Documentation - *Rego Language Reference*
- Holtman, K. - *Mutex Linting: Static Analysis for Concurrency Bugs*

---

*Documento generado como parte de la investigación en patrones arquitectónicos para alineación de reglas en sistemas de IA agentica. Versión 1.0*

## Integración con CogniCode: Patrones Implementables

Este documento presenta patrones teóricos para alineación de reglas. CogniCode proporciona las herramientas concretas para implementarlos. La siguiente tabla muestra cómo cada patrón se mapea a capacidades existentes o nuevas en CogniCode:

**Patrones implementables con CogniCode:**

| Patrón | CogniCode existente | Axiom añade |
|--------|---------------------|-------------|
| **Connascence analysis** | Call graph (petgraph) ya tiene aristas de llamada | Algoritmos de peso de connascence sobre el grafo |
| **LCOM (SRP)** | Symbol → method → attribute mapping ya existe | Cálculo de cohesión por clase |
| **CycleDetector** | Tarjan SCC ya existe en `check_architecture` | Políticas Cedar que DENY si se encuentran ciclos |
| **ImpactAnalyzer** | Ya proporciona risk levels (Low/Medium/High) | `block if risk > threshold` como policy action |
| **DDD boundary validation** | Module dependency graph disponible | Reglas: domain ⊄ infrastructure |
| **Quality delta** | Snapshot call graph before/after | Comparación de métricas delta |
| **Reflection pattern** | Proporciona "qué cambió" | Axiom añade "fue correcto" evaluation |

**Connascence desde call graph existente:**

```rust
// El call graph de CogniCode (petgraph) ya tiene:
// - Nodos: symbols (funciones, métodos)
// - Aristas: llamadas entre símbolos

// Connascence de Nombre (Nm) = símbolos con mismo nombre en módulos distintos
// Connascence de Posición (Np) = parámetros con mismo orden en interfaces
// Connascence Algorítmica (Na) = algoritmos diferentes para mismo propósito

// El grafo de llamadas contiene toda la información necesaria para calcular
// las métricas de connascence estática sin análisis adicional
```

**LCOM desde symbol mapping:**

```rust
// Symbol → methods → attributes mapping ya existe
// LCOM = 1 - (sum(method_shared_attrs) / sum(method_total_attrs))

// Ejemplo:
// class UserService
//   - login(): attrs usados = {credentials}
//   - updateProfile(): attrs usados = {userData, credentials}
//   - getOrders(): attrs usados = {userData}
//
// shared_attrs = 1 (credentials) + 1 (userData) = 2
// total_attrs = 1 + 2 + 1 = 4
// LCOM = 1 - (2/4) = 0.5 (violación si threshold < 0.3)
```

**CycleDetector + Cedar policies:**

```cedar
// check_architecture ya detecta ciclos con Tarjan SCC
// Axiom añade la policy que convierte detección en enforcement

forbid(
    principal,
    action == Action::Commit,
    resource
)
when {
    cycle_detected(resource)
};
```

**ImpactAnalyzer + blocking threshold:**

```rust
// ImpactAnalyzer ya existe con risk levels
// Axiom añade el enforcement

match impact.risk_level {
    RiskLevel::Low => PolicyDecision::Allow,
    RiskLevel::Medium => PolicyDecision::Warn,
    RiskLevel::High if context.approval_required => PolicyDecision::Deny,
    RiskLevel::High => PolicyDecision::Warn,
}
```

**Quality delta snapshot:**

```rust
// Antes del cambio del agente
let before_metrics = snapshot_call_graph(&workspace);

// Agente ejecuta cambio

// Después del cambio
let after_metrics = snapshot_call_graph(&workspace);

// Comparar métricas
let delta = compare_metrics(before_metrics, after_metrics);

// Si delta viola thresholds → policy denial
if delta.cyclomatic_complexity_change > 5 {
    return PolicyDecision::Deny("Complexity increased by > 5");
}
```

**El patrón Reflection completo:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    REFLECTION PATTERN EN CogniCode                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  AGENTE PROPONE        CogniCode ANALIZA        Axiom EVALÚA     │
│  ──────────────        ────────────────        ──────────────    │
│                                                                  │
│  "Voy a hacer X"   →   check_architecture()  →  evaluate(X)    │
│                             │                      │             │
│                             ▼                      ▼             │
│                       Violations?           Decision:            │
│                             │              Allow/Deny/Warn      │
│                             ▼                      │             │
│                      Metrics report            │             │
│                                                  ▼             │
│                       "Hay ciclo en           "Violación:      │
│                        módulo domain"         cycle detected"   │
│                                                                  │
│                              ◀────────────────────────────────  │
│                              │                                   │
│                              ▼                                   │
│                        AGENTE REVISA                            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Conclusión: CogniCode + axiom = sistema completo**

CogniCode proporciona:
- **Datos:** call graphs, complexity metrics, symbol analysis, impact assessment
- **Detección:** violations, cycles, quality issues
- **Snapshots:** before/after comparison

Axiom añade:
- **Evaluación:** policy engine (Cedar) que decide Allow/Deny
- **Enforcement:** hooks que bloquean si Deny
- **Explicación:** reasons + suggestions basadas en policies

Juntos proporcionan el ciclo completo: Action → Analyze → Evaluate → Deny/Allow → Reflect → Revise que este documento describe como el patrón Reflexion.
