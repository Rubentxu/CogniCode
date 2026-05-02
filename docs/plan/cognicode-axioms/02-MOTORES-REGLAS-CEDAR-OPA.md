# Motores de Reglas para Gobernanza de Agentes IA

## Cedar Policy y Open Policy Agent (OPA): Análisis Comparativo e Integración con Agentes IA

---

## 1. Cedar Policy

### 1.1 El Modelo PARC: Principio Fundamental

**Cedar Policy** es un motor de políticas de código abierto desarrollado por Amazon Web Services, diseñado desde cero para ser eficiente, seguro y utilizable en entornos de producción. El corazón de Cedar radica en su modelo conceptual denominado **PARC**, que proporciona una abstracción elegante para expresar políticas de autorización.

El acrónimo PARC representa los cuatro elementos fundamentales de toda política en Cedar:

- **Principal**: La entidad que solicita realizar una acción. En el contexto de agentes IA, esto típicamente representa la identidad del agente, su rol, o el usuario en cuyo nombre opera el agente.
- **Action**: La operación específica que se desea realizar. Las acciones describen qué puede hacer el principal sobre un recurso determinado.
- **Resource**: El objeto sobre el cual se realiza la acción. Puede ser un archivo, una API, un servicio, o cualquier entidad que necesite protección.
- **Context**: El contexto adicional de la solicitud, que incluye metadatos como el timestamp, la fuente de la petición, atributos del ambiente, o información contextual que ayuda a tomar decisiones más refinadas.

Este modelo resulta particularmente poderoso porque separa claramente las preocupaciones de identidad, acción, objeto y circunstancias, permitiendo políticas expresivas sin caer en la complejidad descontrolada de otros sistemas basados en atributos.

### 1.2 Sintaxis de Políticas: permit y forbid

Cedar utiliza una sintaxis declarativa inspirada en **RFC 8707** (HTTP API Verb Normalization) que resulta tanto legible para humanos como procesable por máquinas. Las políticas en Cedar siguen una estructura jerárquica donde las decisiones principales son de dos tipos:

```cedar
// Política de ejemplo: Permitir lectura de documentos a analistas
permit(
    principal in AnalystGroup,
    action in [Action::Read, Action::List],
    resource in DocumentFolder
);
```

La construcción `permit` indica que la solicitud está autorizada cuando todas las condiciones se satisfacen. De manera análoga, `forbid` expresa prohibiciones explícitas que tienen precedencia sobre cualquier `permit`:

```cedar
// Política de ejemplo: Prohibir eliminación de documentos en carpeta raíz
forbid(
    principal,
    action == Action::Delete,
    resource in RootFolder
);
```

Es importante destacar que **forbid siempre tiene prioridad sobre permit** en Cedar. Si alguna política aplica un `forbid` a una solicitud, el resultado final será `deny` independientemente de cuántas políticas `permit` la matcheen. Esta semántica de negación por defecto es un principio de seguridad fundamental en el diseño de Cedar.

### 1.3 El Crate Rust: cedar-policy v4.10.0

Cedar está implementado en Rust, lo que le confiere propiedades de seguridad de memoria y alto rendimiento. El crate `cedar-policy` (versión 4.10.0, licenciada bajo Apache-2.0) proporciona una API completa para integrar la evaluación de políticas en aplicaciones Rust.

#### Dependencia en Cargo.toml

```toml
[dependencies]
cedar-policy = "4.10.0"
cedar-policy-core = "4.10.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

#### Evaluación Básica en Rust

```rust
use cedar_policy::{Authorizer, Policy, PolicySet};
use cedar_policy_core::evaluator::EvaluationOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Crear el autorizador
    let authorizer = Authorizer::new();

    // Definir una política en formato JSON
    let policy_json = serde_json::json!({
        "effect": "permit",
        "principal": {
            "id": "agent-001",
            "type": "Agent"
        },
        "action": {
            "type": "Action",
            "id": "file:write"
        },
        "resource": {
            "type": "File",
            "id": "/project/src/*.rs"
        }
    });

    // Parsear y crear el policy set
    let pset: PolicySet = PolicySet::from_json(policy_json)?;

    // Definir el contexto de la solicitud
    let request = Request::new(
        Principal::from_str("Agent::\"agent-001\""),
        Action::from_str("Action::\"file:write\""),
        Resource::from_str("File::\"/project/src/main.rs\""),
        Context::empty()
    );

    // Evaluar la solicitud
    let response = authorizer.is_authorized(&request, &pset);

    match response.decision() {
        Decision::Allow => println!("Acceso PERMITIDO"),
        Decision::Deny => println!("Acceso DENEGADO"),
    }

    Ok(())
}
```

### 1.4 Definición de Esquemas

Cedar soporta la definición de esquemas que describen la estructura de tipos en el sistema. Los esquemas son opcionales pero altamente recomendados porque permiten validación estática de políticas y mejor documentación.

```json
{
  " CEDAR ':': {
    "types": {
      "Agent": {
        "memberOfTypes": ["User", "Service"],
        "shape": {
          "type": "Record",
          "attributes": {
            "clearanceLevel": { "type": "Long" },
            "owner": { "type": "String" }
          }
        }
      },
      "File": {
        "shape": {
          "type": "Record",
          "attributes": {
            "owner": { "type": "String" },
            "classification": { "type": "String" },
            "path": { "type": "String" }
          }
        }
      },
      "Action": {
        "type": "String",
        "watched": true
      }
    }
  }
}
```

### 1.5 Attribute-Based Access Control (ABAC)

Una de las características más potentes de Cedar es su soporte nativo para **ABAC** (Attribute-Based Access Control). A diferencia de RBAC puro, ABAC permite decisiones de acceso basadas en atributos dinámicos tanto del principal como del recurso y el contexto.

```cedar
// Permitir escritura si el agente tiene nivel de autorización suficiente
// y el archivo no está clasificado como secreto
permit(
    principal,
    action == Action::Write,
    resource
)
when {
    principal.clearanceLevel >= 3 &&
    resource.classification != "top-secret"
};
```

Este ejemplo demuestra cómo Cedar permite expresiones condicionales ricas usando atributos, lo cual resulta especialmente útil en escenarios de gobernanza de agentes IA donde los niveles de autorización pueden cambiar dinámicamente y las clasificaciones de recursos pueden variar.

### 1.6 Caso de Uso: AWS Verified Permissions

**AWS Verified Permissions** es un servicio gestionado de AWS que utiliza Cedar como su motor de políticas subyacente. Este servicio permite a las aplicaciones implementar autorización fina con políticas basadas en roles y atributos.

Casos de uso típicos incluyen:

- Autorización en aplicaciones SaaS multi-tenant
- Control de acceso a recursos basados en atributos de usuario
- Políticas de colaboración entre usuarios con diferentes permisos
- Gobernanza de acceso a APIs y microservicios

### 1.7 Ejemplo Completo de Política Cedar para Agente IA

```cedar
// === Políticas de Gobernanza para Agente IA ===

// Política base: Agentes con rol 'developer' pueden leer código fuente
permit(
    principal in Role::Developer,
    action in [Action::Read, Action::List],
    resource in CodeRepository
);

// Política: Agentes solo pueden modificar archivos en su directorio asignado
permit(
    principal,
    action == Action::Modify,
    resource
)
when {
    principal.assignedDirectory == resource.parentDirectory
};

// Política: Prohibir eliminación de archivos de configuración del sistema
forbid(
    principal,
    action == Action::Delete,
    resource
)
when {
    resource.path.startsWith("/etc/") ||
    resource.path.startsWith("/system/")
};

// Política: Requerir confirmación humana para acciones destructivas
forbid(
    principal,
    action in [Action::Delete, Action::Truncate, Action::Overwrite],
    resource
)
when {
    resource.classification == "critical"
};

permit(
    principal,
    action == Action::ApproveDestructiveAction,
    resource
)
when {
    principal.canApproveDestructiveActions == true
};
```

---

## 2. Open Policy Agent (OPA) y Rego

### 2.1 Overview de OPA

**Open Policy Agent (OPA)** es un motor de políticas de propósito general desarrollado por la Cloud Native Computing Foundation (CNCF). A diferencia de Cedar, que está optimizado para autorización, OPA es un policy engine general que puede evaluar cualquier tipo de política sobre datos estructurados.

OPA se ha convertido en un estándar de facto en el ecosistema cloud-native, particularmente para escenarios donde las políticas necesitan ser expresadas de manera flexible y donde múltiples fuentes de datos influyen en la decisión.

### 2.2 El Lenguaje Rego

**Rego** es el lenguaje de políticas declarativo de OPA, diseñado para expresar políticas sobre documentos JSON arbitrarios. Rego es Turing-incomplete pero suficientemente expresivo para la mayoría de casos de uso de políticas empresariales.

#### Estructura Básica de Rego

```rego
package kubernetes.admission

# Deny pod creation if not in allowed namespaces
deny[msg] {
    input.request.kind.kind == "Pod"
    input.request.namespace != "production"
    input.request.namespace != "staging"
    msg := "Pods can only be created in production or staging namespaces"
}

# Deny privileged containers
deny[msg] {
    input.request.kind.kind == "Pod"
    some i
    input.request.object.spec.containers[i].securityContext.privileged == true
    msg := sprintf("Container %v cannot run in privileged mode", [input.request.object.spec.containers[i].name])
}
```

### 2.3 Input Documents y Data Documents

OPA distingue entre dos tipos principales de documentos que pueden ser consultados en las políticas:

- **Input Document**: El documento de entrada que representa la solicitud o evento a evaluar. Típicamente contiene información transaccional como una solicitud HTTP, un evento de auditoría, o una operación de usuario.

- **Data Document**: Datos de referencia que persisten entre evaluaciones. Pueden representar listas de usuarios válidos, catálogos de recursos, configuraciones, o cualquier información estática o lentamente cambiante.

```rego
# Ejemplo: Verificar si el usuario tiene acceso basado en roles usando data document
allow {
    user := input.user
    data.allowed_users[user]
}

# Ejemplo: Combinar input y data
allow {
    input.action == "read"
    user_has_read_permission[input.user]
}

user_has_read_permission[user] {
    data.roles[user].permissions[_] == "read"
}
```

### 2.4 Decisiones allow y deny

OPA típicamente produce decisiones en forma de `allow` o `deny` (o `deny[msg]` para incluir mensajes de explicación). La convención es que una política `allow` retorna `true` cuando la acción está permitida, mientras que `deny` retorna un mensaje de rechazo cuando debe ser bloqueada.

```rego
# Política principal de allow
default allow = false

allow {
    input.method == "GET"
    input.path == "/api/public"
}

allow {
    input.method == "GET"
    input.path == "/api/protected"
    data.users[input.user].active == true
}

allow {
    input.user == "admin"
}

# Políticas de deny con mensajes
deny[msg] {
    input.method == "POST"
    not input.content_type == "application/json"
    msg := "POST requests must have Content-Type: application/json"
}
```

### 2.5 Kubernetes y Gatekeeper

**OPA Gatekeeper** es un proyecto que integra OPA con Kubernetes como un **Dynamic Admission Controller**. Gatekeeper permite definir políticas como Constraints y ConstraintTemplates usando Rego.

#### Ejemplo de ConstraintTemplate

```yaml
apiVersion: templates.gatekeeper.sh/v1beta1
kind: ConstraintTemplate
metadata:
  name: k8srequiredlabels
spec:
  crd:
    spec:
      names:
        kind: K8sRequiredLabels
      validation:
        openAPIV3Schema:
          properties:
            labels:
              type: array
              items:
                type: string
  targets:
    - target: admission.k8s.gatekeeper.sh
      rego: |
        package k8srequiredlabels

        violation[{"msg": msg, "details": {"missing_labels": missing}}] {
          provided := {label | input.request.object.metadata.labels[label]}
          required := {label | label := input.parameters.labels[_]}
          missing := required - provided
          count(missing) > 0
          msg := sprintf("Missing labels: %v", [missing])
        }
```

### 2.6 Conftest para CI/CD

**Conftest** es una herramienta que utiliza OPA/Rego para validar configuraciones en pipelines de CI/CD. Soporta Kubernetes manifests, Terraform, Docker, INI, y muchos otros formatos.

```bash
# Instalar Conftest
pip install conftest

# Validar un manifest de Kubernetes
conftest test kubernetes/deployment.yaml

# Validar configuración de Terraform
conftest test terraform/main.tf --policy terraform/policy
```

#### Ejemplo de política para Terraform

```rego
package main

deny[msg] {
    input.resource.aws_s3_bucket[_].acl == "public-read"
    msg := "S3 buckets must not be public-read"
}

deny[msg] {
    input.resource.aws_instance[_].instance_type == "t2.micro"
    msg := "Production instances should not be t2.micro for cost reasons"
}

warn[msg] {
    input.resource.aws_db_instance[_].storage_encrypted == false
    msg := "Database instances should have encryption at rest enabled"
}
```

### 2.7 Ecosistema OPA

El ecosistema OPA incluye múltiples componentes y extensiones:

- **OPA Server**: Servidor REST para evaluación de políticas centralizada
- **Bundle API**: Sistema de distribución de políticas en bundle
- **Status API**: Reportes de salud y métricas de evaluación
- **Default Bundle**: Políticas predefinidas para casos comunes
- **OPA Envoy Plugin**: Integración como filtro de Envoy para microservicios
- **IDQL**: Lenguaje para políticas sobre bases de datos

### 2.8 Ejemplo Completo de Política Rego para Agente IA

```rego
package agent_governance

# ============================================
# Políticas de Autorización para Agentes IA
# ============================================

# Función helper: verificar nivel de autorización del agente
agent_has_clearance(agent, required_level) {
    agent.clearanceLevel >= required_level
}

# Función helper: verificar que el recurso está en directorio asignado
resource_in_assigned_directory(agent, resource) {
    startswith(resource.path, agent.assignedDirectory)
}

# Función helper: clasificaciones sensibles
is_classified(resource) {
    resource.classification in ["top-secret", "confidential", "restricted"]
}

# ALLOW: Lectura de archivos de código fuente para desarrolladores
allow {
    input.action == "read"
    input.resource.type == "source_code"
    input.agent.role == "developer"
    not is_classified(input.resource)
}

# ALLOW: Escritura en archivos dentro del directorio asignado
allow {
    input.action == "write"
    input.agent.role == "developer"
    resource_in_assigned_directory(input.agent, input.resource)
    not is_classified(input.resource)
}

# ALLOW: Lectura de archivos clasificados para agentes con nivel >= 5
allow {
    input.action == "read"
    is_classified(input.resource)
    agent_has_clearance(input.agent, 5)
}

# DENY: Escritura en archivos clasificados sin supervisión
deny[msg] {
    input.action == "write"
    is_classified(input.resource)
    not agent_has_clearance(input.agent, 5)
    msg := sprintf("Agent %v requires clearance level 5 to modify classified resource %v", [input.agent.id, input.resource.id])
}

# DENY: Acciones destructivas sin aprobación explícita
deny[msg] {
    input.action in ["delete", "truncate", "overwrite"]
    input.agent.approvalStatus != "approved"
    msg := "Destructive actions require explicit approval before execution"
}

# DENY: Rate limiting - máximo de operaciones por minuto
deny[msg] {
    input.action == "execute"
    count(input.agent.recentOperations[_]) > input.agent.rateLimit
    msg := sprintf("Agent %v exceeded rate limit of %v operations/minute", [input.agent.id, input.agent.rateLimit])
}

# Mensaje de evaluación final
evaluate_and_decide[decision] {
    allow
    decision := {"status": "allowed", "reason": "All policy conditions satisfied"}
}

evaluate_and_decide[decision] {
    deny[msg]
    decision := {"status": "denied", "reason": msg}
}
```

---

## 3. Comparación Cedar vs OPA

### 3.1 Tabla Comparativa de Características

| Característica | Cedar Policy | Open Policy Agent (OPA) |
|----------------|--------------|--------------------------|
| **Lenguaje de políticas** | Cedar Policy Language | Rego |
| **Paradigma** | Declarativo, orientado a autorización | Declarativo, propósito general |
| **Complejidad sintáctica** | Baja, optimizado para legibilidad | Media-alta, muy expresivo |
| **Tipado** | Estático con schema | Dinámico sobre JSON |
| **Motor de evaluación** | Rust nativo | Go (puede compilar a WASM) |
| **Curva de aprendizaje** | Moderada | pronunciada |
| **Extensibilidad** | Limitada | Alta (custom functions, OTA) |
| **Casos de uso primarios** | Autorización, RBAC/ABAC | Cualquier tipo de política |
| **Tamaño de Policieset óptimo** | Pequeño-mediano (<1000) | Grande (miles) |
| **Performance** | Muy alta | Alta |
| **Runtime overhead** | Mínimo | Moderado |

### 3.2 Cuándo Usar Cedar Policy

**Cedar es la elección correcta cuando:**

1. **Autorización es el caso de uso primario**: Si necesitas principalmente controlar quién puede hacer qué sobre qué recurso, Cedar proporciona una abstracción más limpia y enfocada.

2. **Simplicidad es prioritaria**: La sintaxis de Cedar es más accesible para equipos que no tienen experiencia previa con engines de políticas. Las políticas son autodescriptivas.

3. **Performance es crítica**: El runtime de Cedar en Rust tiene overhead mínimo, lo que lo hace ideal para hot paths donde cada milisegundo cuenta.

4. **Integración AWS**: Si usas AWS Verified Permissions o necesitas integrarte con servicios AWS, Cedar es la opción nativa.

5. **Políticas pequeñas a medianas**: Cedar está optimizado para conjuntos de políticas de tamaño pequeño a mediano. Si tienes miles de políticas, OPA puede manejar mejor la carga.

6. **Type safety es importante**: El sistema de tipos estático de Cedar con schema validation previene errores en tiempo de desarrollo.

### 3.3 Cuándo Usar OPA

**OPA es la elección correcta cuando:**

1. **Políticas complejas con múltiples fuentes de datos**: Cuando la decisión de política requiere combinar información de múltiples fuentes (databases, APIs externas, sistemas de configuración), OPA es más flexible.

2. **Necesitas policy-as-code general**: Si tus políticas no son solo de autorización (e.g., compliance, validación de config, reglas de negocio), OPA puede manejarlas todas.

3. **Ecosistema cloud-native**: Para Kubernetes, Terraform, y otras herramientas cloud-native, OPA tiene integraciones maduras y preconstruidas.

4. **WASM embedding es necesario**: OPA compila a WASM de manera nativa, permitiendo embedding en cualquier runtime que soporte WebAssembly.

5. **Gran volumen de políticas**: OPA maneja eficientemente miles de políticas organizadas en namespaces y bundles.

6. **Debugging avanzado es necesario**: OPA tracer y testing framework son más maduros para políticas complejas.

### 3.4 Performance Characteristics

#### Cedar Performance

- **Evaluación**: O(1) para match de políticas usando matching eficiente
- **Memoria**: Footprint muy bajo gracias a Rust
- **Latencia**: Diseñado para latencias sub-milisegundo
- **Threading**: Totalmente thread-safe, optimizado para concurrent evaluation

#### OPA Performance

- **Evaluación**: Indexación profunda de documentos para queries eficientes
- **Memoria**: Bundle caching con configurable size limits
- **Latencia**: Latencias típico de 1-5ms para evaluación, optimizable con caching
- **Threading**: goroutines para evaluación paralela

### 3.5 Expressiveness de Políticas

#### Lo que Cedar Expressa Bien

```cedar
// Expresiones booleanas sobre atributos
permit(
    principal.age >= 18,
    action == Action::Access,
    resource
);

// Membership en grupos
permit(
    principal in Group::Engineering,
    action,
    resource
);

// Condiciones complejas sobre atributos numéricos
when {
    resource.value > principal.creditLimit
}
```

#### Lo que OPA/Rego Expressa Bien

```rego
# Queries sobre estructuras anidadas arbitrarias
allow {
    input.request.headers["Authorization"] == data.apiKeys[key]
    key == input.request.user
}

# Agregaciones sobre colecciones
violation[msg] {
    some k
    data.clusters[k].unhealthy_nodes > 3
    msg := sprintf("Cluster %v has too many unhealthy nodes", [k])
}

# Recursión (limitada)
path_exists(graph, node, path) {
    graph[node][next]
    path := [node] + path
    path_exists(graph, next, path)
}
```

---

## 4. Mapping del Modelo PARC a Gobernanza de Agentes IA

### 4.1 Correspondencia Conceptual Directa

El modelo PARC de Cedar se mapea naturalmente al dominio de gobernanza de agentes IA:

| Elemento PARC | Significado General | Mapping a Agentes IA |
|---------------|---------------------|----------------------|
| **Principal** | Entidad que solicita acceso | `Agent` (identidad del agente, rol, capabilities) |
| **Action** | Operación a realizar | `File::Write`, `Code::Generate`, `API::Call`, `Command::Execute` |
| **Resource** | Objeto sobre el cual actúa | Archivos destino, código a modificar, APIs a invocar |
| **Context** | Metadatos de la solicitud | `TaskMetadata`, `UserContext`, `EnvironmentState`, `Timestamp` |

### 4.2 Ejemplo de Mapeo Completo

Considérese un agente IA que necesita escribir código en un archivo:

```cedar
// Mapeo directo al modelo PARC
permit(
    principal: Agent::"agent-llm-001" with attributes {
        role: "developer",
        clearanceLevel: 4,
        assignedDirectory: "/workspace/project/src"
    },
    action: Action::"file:write",
    resource: File::"/workspace/project/src/main.rs",
    context: {
        taskId: "task-12345",
        userId: "user-alice",
        timestamp: 2024-01-15T10:30:00Z,
        taskType: "feature implementation"
    }
)
```

### 4.3 Context como Fuente de Inteligencia

El componente `Context` en el modelo PARC es particularmente valioso para agentes IA porque permite enriquecer las decisiones de política con información situacional:

- **Historial del agente**: Número de operaciones recientes, historial de errores, tasa de éxito
- **Metadatos de la tarea**: User story asociada, sprint, prioridad, deadline
- **Estado del ambiente**: Branch actual, estado de CI/CD, reviewers asignados
- **Información del usuario**: Rol del usuario que solicitó la tarea, nivel de autorización

```cedar
// Ejemplo: Policy que usa contexto rico
permit(
    principal,
    action == Action::Modify,
    resource
)
when {
    // El agente solo puede modificar si la tarea está aprobada
    context.taskStatus == "approved" &&
    // Y el usuario que solicitó tiene autoridad suficiente
    context.userAuthorization >= resource.requiredAuthLevel &&
    // Y no estamos en periodo de freeze
    context.currentSprint.freeze != true
};
```

### 4.4 Jerarquía de Agentes y Herencia

Los agentes IA típicamente tienen una jerarquía organizacional que se mapea bien a grupos y roles:

```cedar
// Grupo base de todos los agentes
entity Agent = {
    id: String,
    type: String,  // "llm", "rule-based", "hybrid"
    parentAgent: Option[Agent],
    clearanceLevel: Long,
    capabilities: Set[String]
};

// Agentes de desarrollo tienen acceso a repositorios
permit(
    principal in AgentGroup::Developers,
    action in [Action::Read, Action::Write],
    resource in CodeRepository
);

// Agentes de solo-lectura no pueden modificar
forbid(
    principal in AgentGroup::ReadOnly,
    action in [Action::Write, Action::Delete],
    resource
);

// Agentes subordinados heredan restricciones del agente padre
permit(
    principal,
    action,
    resource
)
when {
    principal.parentAgent != null &&
    principal.parentAgent.clearanceLevel >= 3
};
```

---

## 5. Policy-as-Code para Agentes IA

### 5.1 El Ciclo de Evaluación: Propose → Evaluate → Feedback → Revise

La gobernanza de agentes IA mediante políticas sigue un ciclo iterativo que difiere fundamentalmente de la autorización estática tradicional:

```
┌─────────────────────────────────────────────────────────────────┐
│                    AGENT GOVERNANCE LOOP                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌──────────┐     ┌───────────┐     ┌──────────┐             │
│   │ PROPOSE  │────▶│ EVALUATE  │────▶│ FEEDBACK │             │
│   │          │     │           │     │          │             │
│   └──────────┘     └───────────┘     └──────────┘             │
│        ▲                                   │                   │
│        │                                   ▼                   │
│        │       ┌───────────┐     ┌──────────────┐             │
│        └───────│  REVISE   │◀────│   APPLY?     │             │
│                │           │     │              │             │
│                └───────────┘     └──────────────┘             │
│                                                             │
└─────────────────────────────────────────────────────────────────┘
```

1. **Propose**: El agente propone una acción basada en su comprensión de la tarea (e.g., "Voy a modificar este archivo para implementar la característica X")

2. **Evaluate**: El motor de políticas evalúa la propuesta contra las reglas activas, consultando el contexto apropiado

3. **Feedback**: Si la acción es denegada, se proporciona una explicación al agente con el motivo del rechazo y potencialmente sugerencias de corrección

4. **Revise**: El agente revisa su propuesta basándose en el feedback, potencialmente solicitando aprobación adicional o modificando el scope de la acción

### 5.2 Qué Pueden y No Pueden Evaluar Cedar/OPA

#### ✓ LO QUE PUEDEN EVALUAR

| Aspecto | Ejemplo | Motor |
|---------|---------|-------|
| **Autorización básica** | ¿Tiene el agente permiso para esta acción? | Ambos |
| **Restricciones de recursos** | ¿Está el recurso en un directorio permitido? | Ambos |
| **Atributos de clasificación** | ¿El archivo está clasificado como secreto? | Ambos |
| **Rate limiting** | ¿Ha excedido el agente su cuota de operaciones? | Ambos |
| **Validación de contexto** | ¿Está la tarea en estado approved? | Ambos |
| **Compliance de formato** | ¿El nombre del archivo sigue las convenciones? | OPA |
| **Dependencies válidas** | ¿Las dependencias referenciadas existen? | OPA |

#### ✗ LO QUE NO PUEDEN EVALUAR

| Aspecto | Limitación | Por qué |
|---------|------------|---------|
| **Calidad del código** | ¿El código es idiomático? | Requiere análisis semántico de código |
| **Corrección algorítmica** | ¿El algoritmo es correcto? | Requiere verificación formal o testing |
| **Seguridad del código generado** | ¿Tiene vulnerabilities? | Requiere análisis de seguridad dedicado |
| **Naming appropriateness** | ¿Los nombres son descriptivos? | Juicio subjetivo que requiere contexto |
| **Performance implications** | ¿Hay problemas de performance? | Requiere profiling o análisis asintótico |
| **Business logic correctness** | ¿Implementa el requerimiento correcto? | Requiere especificación formal |

### 5.3 Policy-as-Code como Capa de Gobernanza

```
┌─────────────────────────────────────────────────────────────────────┐
│                    AGENT GOVERNANCE LAYERS                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    AGENT LAYER                              │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐        │   │
│  │  │ LLM     │  │ Tools   │  │ Memory  │  │ Planner │        │   │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │              POLICY ENFORCEMENT LAYER                       │   │
│  │  ┌─────────────────────┐    ┌─────────────────────────┐   │   │
│  │  │   Cedar/OPA Engine  │    │   MCP Server (PEP)       │   │   │
│  │  │   Authorization     │    │   Context Enrichment    │   │   │
│  │  └─────────────────────┘    └─────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                  INFRASTRUCTURE LAYER                       │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐        │   │
│  │  │ Files   │  │ APIs    │  │ Shell   │  │ Network │        │   │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.4 Integración con Quality Gates

Las políticas pueden implementar "quality gates" que van más allá de la autorización pura:

```rego
package agent_quality_gates

# Quality gate: Verificar que el código propuesto tiene tests
deny[msg] {
    input.action == "commit"
    input.files_changed[_].type == "source"
    not has_associated_tests(input.files_changed)
    msg := "Source code changes require associated test files"
}

# Quality gate: Verificar convenciones de nombre
deny[msg] {
    input.action == "create_file"
    endswith(input.resource.name, ".go")
    not matches_naming_convention(input.resource.name)
    msg := "Go files must follow naming conventions: lowercase with underscores"
}

has_associated_tests(changes) {
    source_files := [f | changes[_]; f.type == "source"]
    test_files := [f | changes[_]; f.type == "test"]
    count(test_files) >= count(source_files) / 2
}

matches_naming_convention(name) {
    regex.match("^[a-z][a-z0-9_]*\\.go$", name)
}
```

---

## 6. Rust Ecosystem para Policy Evaluation

### 6.1 El Crate cedar-policy

El crate `cedar-policy` proporciona acceso programático completo al motor de evaluación de Cedar. Las características principales incluyen:

```rust
// Características del crate cedar-policy
// - Parsing de políticas Cedar
// - Evaluación de requests contra policy sets
// - Validación de políticas contra schemas
// - Serialización/deserialización JSON

use cedar_policy::{Authorizer, PolicySet, Request};
use cedar_policy_core::entities::Entities;
use cedar_policy_core::evaluator::EvaluationOptions;

pub struct PolicyEngine {
    authorizer: Authorizer,
    policies: PolicySet,
    entities: Entities,
}

impl PolicyEngine {
    pub fn new(policies_json: &str, entities_json: &str) -> Result<Self> {
        let policies: PolicySet = serde_json::from_str(policies_json)?;
        let entities: Entities = serde_json::from_str(entities_json)?;

        Ok(Self {
            authorizer: Authorizer::new(),
            policies,
            entities,
        })
    }

    pub fn evaluate(&self, request: Request) -> Decision {
        self.authorizer.is_authorized(&request, &self.policies, &self.entities).decision()
    }
}
```

### 6.2 Alternativas en el Ecosystem Rust

#### rhea

**rhea** es una alternativa más ligera para policy evaluation,focusada en políticas basadas en reglas con sintaxis similar a Datalog. Es útil cuando se necesitan políticas más expresivas con recursión.

```rust
// Ejemplo conceptual de rhea
use rhea::{Policy, Rule};

let policy = Policy::parse(r#"
    allow(user, resource) :-
        user.clearance >= resource.required_clearance,
        resource.type == "document".

    deny(resource) :-
        resource.classification == "top-secret",
        not user.has_special_permission.
"#)?;
```

#### cider

**cider** es un motor de políticas experimental que intenta combinar la simplicidad de Cedar con mayor expresividad. Todavía está en desarrollo activo.

### 6.3 Por Qué Rust es Ideal para MCP Server Embedding

La elección de Rust como lenguaje base para motores de políticas de gobernanza ofrece ventajas significativas para integración con MCP (Model Context Protocol) servers:

1. **Seguridad de memoria**: Sin garbage collector, ideal para long-running servers
2. **BAJA latencia**: Overhead mínimo en hot paths de evaluación
3. **Thread safety**: Compartmentalización natural de múltiples agentes evaluando en paralelo
4. **WASM compilation**: Posibilidad de compilar a WebAssembly para embedding universal
5. **FFI clara**: Interfaz bien definida para integración con otros lenguajes
6. **Tamaño de binario**: Binarios estáticos pequeños, ideales para containers

```rust
// Ejemplo de embedding de Cedar en un MCP server
use cedar_policy::{Authorizer, PolicySet, Request};
use cedar_policy_core::entities::Entities;

pub struct PolicyEnforcer {
    authorizer: Authorizer,
    policy_set: PolicySet,
    entities: Entities,
}

impl PolicyEnforcer {
    /// Evaluate a tool call request from an agent
    pub fn evaluate_tool_call(
        &self,
        agent_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ToolCallDecision> {
        let request = Request::builder()
            .principal(Agent::from_str(agent_id)?)
            .action(tool_name.parse()?)
            .resource(arguments)
            .context(Context::empty())
            .build();

        let response = self.authorizer.is_authorized(
            &request,
            &self.policy_set,
            &self.entities,
        );

        Ok(ToolCallDecision {
            allowed: response.decision() == Decision::Allow,
            errors: response.errors().to_vec(),
        })
    }
}
```

### 6.4 Comparación de Crates Rust para Policy Evaluation

| Crate | Maturity | Expressiveness | Performance | WASM Support |
|-------|----------|----------------|-------------|--------------|
| cedar-policy | Production (AWS-backed) | Media | Muy alta | Limitado |
| rhea | Beta | Alta (Datalog-like) | Alta | No |
| cider | Experimental | Alta | Media | Parcial |
| Casbin combos | Varies | Alta | Media | Variable |

---

## 7. Patrones de Integración

### 7.1 WASM Embedding de OPA

OPA puede compilarse a WebAssembly, permitiendo embedding en cualquier runtime que soporte WASM. Este patrón es particularmente útil para:

- **Edge computing**: Evaluación de políticas en edge locations
- **Browser-based**: Políticas evaluadas client-side
- **Plugin systems**: Políticas como plugins seguros
- **Sandboxing**: Aislamiento fuerte entre motor de políticas y aplicación

```bash
# Compilar OPA a WASM
opa build -t wasm -o policy.wasm policy.rego

# Ejecutar con wasmtime
wasmtime policy.wasm eval --format pretty --bundle policy.bundle 'input={"user": "alice"}'
```

### 7.2 FFI con Cedar

Para integrar Cedar en aplicaciones no-Rust, se pueden usar Foreign Function Interfaces:

```c
// Ejemplo de header FFI para Cedar
typedef struct {
    const char* principal;
    const char* action;
    const char* resource;
    const char* context_json;
} CedarRequest;

typedef struct {
    int decision; // 0 = deny, 1 = allow
    const char* error_message;
} CedarResponse;

// Funciones FFI exportadas
CedarResponse evaluate_policy(const char* policy_json, CedarRequest* request);
void free_response(CedarResponse* response);
```

```rust
// Implementación Rust con exportacros
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn evaluate_policy(
    policy_json: *const c_char,
    request: *const CedarRequest,
) -> *mut CedarResponse {
    // Implementación...
}

#[no_mangle]
pub extern "C" fn free_response(response: *mut CedarResponse) {
    // Cleanup...
}
```

### 7.3 Arquitectura Sidecar

Un patrón común para integrar policy engines con agentes IA es la arquitectura sidecar:

```
┌─────────────────────────────────────────────────────────────────────┐
│                      AGENT POD                                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Agent Container                            │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐                       │  │
│  │  │ LLM     │  │ Tools   │  │ Memory  │                       │  │
│  │  └────┬────┘  └────┬────┘  └────┬────┘                       │  │
│  │       │             │             │                            │  │
│  │       └─────────────┼─────────────┘                            │  │
│  │                     │                                          │  │
│  │                     ▼                                          │  │
│  │              ┌────────────┐                                   │  │
│  │              │ MCP Client │                                   │  │
│  │              └─────┬──────┘                                   │  │
│  └────────────────────┼──────────────────────────────────────────┘  │
│                       │ gRPC                                       │
│  ┌────────────────────┼──────────────────────────────────────────┐  │
│  │                    ▼           Policy Sidecar                 │  │
│  │  ┌─────────────────────────────────────────────────────────┐ │  │
│  │  │              Policy Engine (Cedar/OPA)                  │ │  │
│  │  │  ┌───────────┐  ┌───────────┐  ┌───────────┐            │ │  │
│  │  │  │ Policies  │  │ Entities  │  │  Cache    │            │ │  │
│  │  │  └───────────┘  └───────────┘  └───────────┘            │ │  │
│  │  └─────────────────────────────────────────────────────────┘ │  │
│  │  ┌─────────────────────────────────────────────────────────┐ │  │
│  │  │              gRPC Server (PEP Interface)               │ │  │
│  │  └─────────────────────────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 7.4 MCP Server como Policy Enforcement Point

En el contexto del Model Context Protocol, el servidor MCP puede actuar como Policy Enforcement Point (PEP), interceptando y evaluando cada operación del agente:

```rust
// Conceptual MCP Policy Enforcement
pub struct PolicyMcpServer {
    policy_engine: PolicyEngine,
    tools: Vec<ToolDefinition>,
}

impl PolicyMcpServer {
    pub async fn handle_tool_call(
        &self,
        agent_id: &str,
        tool_name: &str,
        arguments: JsonValue,
    ) -> Result<ToolCallResponse> {
        // 1. Construir request de evaluación
        let request = PolicyRequest {
            principal: agent_id.into(),
            action: tool_name.into(),
            resource: Resource::ToolArguments(tool_name, arguments.clone()),
            context: Context::from_current_task(),
        };

        // 2. Evaluar contra políticas
        match self.policy_engine.evaluate(request) {
            PolicyDecision::Allow => {
                // 3a. Si permitido, ejecutar la herramienta
                self.execute_tool(tool_name, arguments).await
            }
            PolicyDecision::Deny(reason) => {
                // 3b. Si denegado, retornar error con explicación
                Ok(ToolCallResponse::Denied {
                    reason: reason.clone(),
                    suggestion: self.get_suggestion(tool_name, &reason),
                })
            }
        }
    }
}
```

### 7.5 Patrón de Cache Distribuido

Para escenarios con múltiples agentes evaluando políticas, un cache distribuido mejora performance:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    DISTRIBUTED POLICY EVALUATION                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Agent-1 ──┐                                                       │
│   Agent-2 ─┼──▶ ┌─────────────┐      ┌────────────────┐           │
│   Agent-3 ─┤    │   Policy    │─────▶│  Redis Cache   │           │
│             │    │   Gateway   │      │  (Decisions)   │           │
│   Agent-N ──┘    └──────┬──────┘      └────────────────┘           │
│                         │                                            │
│                         ▼                                            │
│                  ┌─────────────┐                                     │
│                  │   Cedar/    │                                     │
│                  │   OPA       │                                     │
│                  └─────────────┘                                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Conclusiones y Recomendaciones

### Síntesis de la Comparación

Ambos motores de políticas, Cedar y OPA, ofrecen capacidades robustas para la gobernanza de agentes IA, pero con perfiles distintos:

**Cedar Policy** brilla en escenarios donde la autorización es el caso de uso primario, se requiere alta performance con bajo overhead, y el equipo valora la simplicidad y legibilidad de las políticas. Su integración nativa con AWS y su diseño optimizado para Rust lo hacen especialmente adecuado para MCP servers embebidos en aplicaciones Rust.

**Open Policy Agent** es la elección cuando se necesita flexibilidad máxima, integración con el ecosistema cloud-native (Kubernetes, Terraform), o cuando las políticas requieren expresiones complejas sobre datos estructurados. Su capacidad de compilación a WASM abre posibilidades de embedding universal.

### Recomendaciones para Agentes IA

1. **Para governance de agentes simples**: Comenzar con Cedar por su simplicidad y performance
2. **Para escenarios cloud-native**: OPA con Gatekeeper para Kubernetes admission control
3. **Para CI/CD pipelines**: Conftest con OPA para validación de configuraciones
4. **Para embedding en Rust MCP servers**: Cedar directamente o OPA compilado a WASM
5. **Para escenarios híbridos**: Considerar una capa de abstracción que permita evaluar con ambos motores

### Consideraciones de Implementación

- **Esquemas estrictos**: Usar schemas en Cedar para validación temprana
- **Versioning de políticas**: Tratar las políticas como código fuente con reviews obligatorios
- **Testing de políticas**: Implementar unit tests para cada política con casos edge
- **Monitoring**: Registrar todas las evaluaciones de política para auditoría
- **Gradual rollout**: Para políticas nuevas, comenzar en modo "advisory" (solo log) antes de enforcement

---

## Referencias y Recursos

- **Cedar Policy**: https://www.cedarpolicy.com/
- **cedar-policy Rust Crate**: https://docs.rs/cedar-policy/
- **Open Policy Agent**: https://www.openpolicyagent.org/
- **OPA Rego Language**: https://www.openpolicyagent.org/docs/latest/policy-language/
- **AWS Verified Permissions**: https://docs.aws.amazon.com/verifiedpermissions/
- **OPA Gatekeeper**: https://open-policy-agent.github.io/gatekeeper/
- **Conftest**: https://www.conftest.dev/

---

*Documento de investigación preparado para el proyecto NotebookAI*
*Versión: 1.0*
*Fecha: Abril 2026*

## Integración con CogniCode: Motor de Políticas

La comparación Cedar vs OPA de este documento concluye que **Cedar es la elección correcta cuando autorización es el caso de uso primario y se requiere alta performance**. Esta conclusión se alinea perfectamente con la integración en CogniCode.

**¿Por qué Cedar sobre OPA para CogniCode?**

| Criterio | Cedar | OPA | Ventaja para CogniCode |
|----------|-------|-----|------------------------|
| Rust-native | ✓ | ✗ (Go) | Compila con cognicode-core |
| Crate de primer nivel | ✓ | ✗ | Sin dependencias externas |
| Performance | Muy alta | Alta | Runtime overhead mínimo |
| Type safety | Estático con schema | Dinámico | Validación temprana |

Cedar se integra directamente en el `Cargo.toml` del workspace de CogniCode como cualquier otra dependencia Rust.

**Mapeo del modelo PARC a CogniCode:**

```
┌─────────────────────────────────────────────────────────────────┐
│  CEDAR PARC                │  CogniCode                         │
├────────────────────────────┼────────────────────────────────────┤
│  Principal                 │  Agent (identidad del agente,       │
│                            │  rol, capabilities del workspace)  │
│  Action                    │  Tool Call (file:write, cmd:exec,  │
│                            │  api:call)                        │
│  Resource                  │  File/Module (el archivo o módulo  │
│                            │  que se está analizando)           │
│  Context                   │  Project metadata (branch, user,   │
│                            │  task_type, session context)       │
└─────────────────────────────────────────────────────────────────┘
```

**Ejemplo: `check_architecture` → Cedar evaluation:**

```rust
// check_architecture ya existe en CogniCode
// Detecta ciclos, viola DIP, detecta внедрения

// axiom añade el Policy Enforcement Point
pub fn evaluate_architecture_violation(
    violation: &ArchitectureViolation,
    context: &PolicyContext,
) -> PolicyDecision {
    // Elviolation se convierte en request Cedar
    let request = Request::builder()
        .principal(Agent::from_context(context))
        .action(Action::ModifyArchitecture)
        .resource(Resource::from_module(&violation.module))
        .context(Context::from_violation(&violation))
        .build();

    // Cedar evalúa contra las políticas del workspace
    authorizer.is_authorized(&request, &policy_set, &entities)
}
```

**Políticas Cedar típicas para un workspace CogniCode:**

```cedar
// Los ciclos de dependencias violan la arquitectura
forbid(
    principal,
    action == Action::ModifyFile,
    resource
)
when {
    would_create_cycle(resource, principal)
};

// Domain no puede depender de infrastructure
forbid(
    principal,
    action == Action::Import,
    resource
)
when {
    resource.inModule == "infrastructure" &&
    principal.inModule == "domain"
};

// Archivos críticos requieren approval antes de modificar
forbid(
    principal,
    action == Action::Modify,
    resource
)
when {
    resource.classification == "critical" &&
    context.taskApproval != "approved"
};
```

**Flujo de integración completo:**

```
Agent propone cambio
        │
        ▼
CogniCode: check_architecture()  ──▶ Detecta violación (ciclo/DIP)
        │
        ▼
Axiom: evaluate(violation)      ──▶ Cedar policy engine
        │
        ├──▶ Allow  ──▶ Ejecutar cambio
        │
        └──▶ Deny   ──▶ Bloquear + explanation + suggestion
```

Cedar proporciona el motor de políticas; CogniCode proporciona los datos de análisis; axiom une ambos en un sistema de governance enforceable.
