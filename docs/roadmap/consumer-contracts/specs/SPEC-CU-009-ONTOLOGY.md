# SPEC-CU-009 — Un recurso de infraestructura no es un símbolo de programación

**Estado:** PROPOSED · **Bloques:** CU-3 · **ADR:** `ADR-CU-002`

## Intención

El grafo modela símbolos, y un símbolo es un nombre con una clase: una
función, una clase, una variable. El extractor genérico decide esa clase a
partir de la forma del nodo del árbol sintáctico, y esa forma no siempre
significa lo que el lenguaje significa.

Un `command` de Bash no es una importación: es una orden. Un
`function_definition` de Bash no es además una clase: es una función. Un
`block` de HCL no es una función: puede ser un recurso, un módulo, una
variable o una salida. Cuando esa clase genérica llega al consumidor, el
producto afirma algo que la lengua no afirma.

Los lectores que entienden el lenguaje ya lo saben —el de Terraform
distingue recurso, dato, variable, módulo y salida; el de Ansible, play,
task y handler—, pero **no todas las rutas los consultan**, así que la
misma pregunta recibe dos respuestas según por dónde se pregunte.

## Contrato

- **CU-009.1** El grafo **distingue un símbolo de programación de una
  declaración de infraestructura**, y dice cuál es. Una declaración de
  infraestructura nombra su kind; un símbolo de programación nombra el suyo.
- **CU-009.2** La clase que el extractor genérico deduce de la forma del
  nodo **no contradice la semántica del lenguaje**. Si el lenguaje tiene un
  lector que sabe lo que la cosa es, el lector manda; si no lo tiene, el
  grafo dice lo que sabe y lo que no.
- **CU-009.3** Una categoría que el lector no puede defender **no se
  publica**. Un `command` de Bash no es una importación y una función de
  Bash no es además una clase.
- **CU-009.4** La ontología está **declarada en un sitio** —el dominio— y
  no se vuelve a derivar en cada consumidor. Cada consumidor que publica
  una clase usa la misma definición.
- **CU-009.5** Un lenguaje **sin lector semántico declara su precisión**:
  qué puede afirmar y qué no, en la matriz de capacidades. Lo que no sabe,
  lo dice.

## Ejemplo de contrato de ontología (esquema orientativo)

```json
{"id":"tf:/p/main.tf:aws_s3_bucket.app_bucket","kind":"declaration","declaration_kind":"resource","resource_type":"aws_s3_bucket"}
{"id":"src/app.rs:handle","kind":"symbol","symbol_kind":"function"}
```

## Garantías mínimas

- Nadie lee un recurso de infraestructura como si fuera una función.
- Las rutas que no pasan por el lector semántico dicen lo mismo que las que
  sí, o dicen que no lo saben.
- La matriz de capacidades declara la precisión por lenguaje, y lo que no
  puede afirmarse está escrito allí.

**Pruebas:** un test permanente por lenguaje con lector semántico (Terraform
y Ansible) que compruebe que la categoría publicada no es la del extractor
genérico; un test que compruebe que Bash no publica `command` como
importación; una fila por lenguaje en la matriz de capacidades. Referencias:
`crates/cognicode-core/src/infrastructure/parser/language_config.rs`,
`crates/cognicode-core/src/infrastructure/parser/terraform_handler.rs`,
`crates/cognicode-core/src/infrastructure/parser/ansible_handler.rs`,
`crates/cognicode-core/src/domain/value_objects/declaration.rs`,
`docs/roadmap/consumer-contracts/specs/CAPABILITIES-MATRIX.md`.
