# SPEC-CU-008 — La consulta de infraestructura responde con el tipo, la arista y la profundidad que le corresponden

**Estado:** PROPOSED · **Bloques:** CU-3 · **ADR:** `ADR-CU-002`

## Intención

La herramienta que responde sobre infraestructura hace tres preguntas y las
tres tienen una respuesta enriquecida: **qué es esto**, **de qué depende** y
**hasta dónde**. Hoy las tres se contestan con atajos: el tipo se toma del
tipo interno que guarda la declaración, la relación de los dependientes es
una constante, y la profundidad es un tope de resultados.

Un consumidor que lee esas respuestas no puede distinguirlas de un análisis
real: parecen datos y son constantes. Este contrato dice qué tiene que
contestar cada pregunta.

## Contrato

- **CU-008.1** Una declaración de infraestructura **nombra su tipo**: el
  kind que le dio el lector (`resource`, `data`, `variable`, `module`,
  `output`, `play`, `task`, `handler`) y, cuando lo lleva, el tipo que la
  declaración nombra (`aws_s3_bucket`). La respuesta publica el tipo cuando
  existe y el kind cuando no, y nunca el nombre de una variante interna de
  la implementación.
- **CU-008.2** Una relación **conserva los metadatos reales de su arista**:
  tipo de dependencia, procedencia y confianza. Una constante no es un
  metadato: si la arista no lleva confianza, la confianza se dice
  desconocida, no se invierte en `1.0`.
- **CU-008.3** `depth` es la **profundidad del recorrido**, no un tope de
  resultados. Doshops significa "a dos saltos", no "las veinte primeras
  aristas". Un tope de resultados, si hace falta, es otro parámetro y se
  llama como lo que es.
- **CU-008.4** Cuando **varias declaraciones responden al mismo nombre**,
  la herramienta declara la ambigüedad —o pide la identidad canónica—; no
  elige una en silencio. La identidad canónica (`tf:fichero:tipo.nombre`)
  sigue siendo la forma inequívoca de preguntar.
- **CU-008.5** Los dos caminos de la herramienta **hablan el mismo
  vocabulario**: la respuesta no depende de qué backend contestó. Un camino
  que no tiene implementación no es una segunda definición de la respuesta.

## Ejemplo de contrato de infraestructura (esquema orientativo)

```json
{"resource_id":"tf:/p/main.tf:aws_s3_bucket.app_bucket",
 "resource_type":"aws_s3_bucket",
 "dependencies":[{"id":"tf:/p/main.tf:aws_s3_bucket.app_bucket","name":"aws_s3_bucket.app_bucket",
                  "kind":"aws_s3_bucket","edge_type":"references","confidence":1.0}]}
```

## Garantías mínimas

- "Qué es esto" tiene una respuesta que el lenguaje del producto usaría.
- "De qué depende" trae la arista que existe, con sus metadatos, no una
  constante.
- "Hasta dónde" es una profundidad, y aumentar `depth` trae más aristas,
  no las mismas con otro corte.
- Un nombre ambiguo no se resuelve por casualidad.

**Pruebas:** un test permanente sobre los fixtures Terraform y Ansible
reales que compruebe el tipo, el kind sin tipo, los metadatos de la arista
y la monotonía de `depth`; un test que compruebe que un nombre ambiguo se
declara ambiguo. Referencias:
`crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs`,
`crates/cognicode-core/src/domain/aggregates/call_graph.rs`,
`crates/cognicode-core/src/domain/value_objects/declaration.rs`,
`sandbox/fixtures/terraform-iac`, `sandbox/fixtures/ansible-playbook`.
