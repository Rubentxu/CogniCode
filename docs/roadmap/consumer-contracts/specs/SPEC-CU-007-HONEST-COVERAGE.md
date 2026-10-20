# SPEC-CU-007 — Cobertura honesta: archivos leídos y referencias resueltas son dos hechos

**Estado:** PROPOSED · **Bloques:** CU-3, CU-4 · **ADR:** `ADR-CU-004`

## Intención

Un grafo puede haber leído todos los ficheros de un proyecto y aun así no
haber resuelto todas las referencias que hay en ellos. Son dos hechos
distintos, y el producto los confunde: responde `complete` cuando ha leído
todo, como si también lo hubiera resuelto todo.

La diferencia no es cosmética. Un grafo que leyó un fichero y no encontró
`aws_s3_bucket.app_bucket` **no sabe qué depende de qué**, y quien pregunta
por el impacto de un cambio recibe una respuesta que parece completa y no lo
es. El silencio no es honestidad: es la forma más difícil de detectar de
mentir.

Este contrato no pide resolver más referencias de las que se pueden
resolver. Pide que las que no se resuelven **se digan**.

## Contrato

- **CU-007.1** El grafo distingue dos hechos: la **cobertura de archivos**
  (qué ficheros leyó) y la **resolución de relaciones** (qué referencias
  pudo resolver). Ninguno se deduce del otro.
- **CU-007.2** Una referencia sin resolver es un **hecho**, no un silencio:
  se conserva con su origen (fichero y línea), su motivo y su clase —
  `Unresolved` cuando no hay candidato, `Ambiguous` cuando hay más de uno.
  Descartarla sin contarla no es una opción.
- **CU-007.3** `Complete` sólo se dice cuando las dos coberturas están
  completas. Si queda alguna referencia pendiente, la respuesta dice cuál y
  por qué, en el mismo sitio donde dice lo que sí resolvió.
- **CU-007.4** No se fabrica ninguna arista: una relación sin destino
  demostrado no se inventa. La resolución pendiente se resuelve después,
  entre ficheros, cuando aparecen los candidatos.
- **CU-007.5** El **error del recorrido** y la **exclusión legítima** son
  cosas distintas. Un fichero que no se pudo inspeccionar no es un fichero
  excluido: se cuenta aparte, con su motivo, y no desaparece del
  inventario.
- **CU-007.6** El grafo, el manifiesto de invalidación y el vigilante
  nombran **el mismo conjunto de ficheros de origen**. Una lista literal de
  extensiones en cualquiera de los tres es un defecto, no una decisión.

## Ejemplo de contrato de cobertura (esquema orientativo)

```json
{"state":"partial",
 "coverage":{"files_read":12,"files_pending":0,"relations_total":30,"relations_resolved":28},
 "gaps":[{"kind":"unresolved_reference","origin":"main.tf:42","reason":"no_candidate","target":"aws_s3_bucket.app_bucket"}]}
```

## Garantías mínimas

- Un consumidor que pregunta "¿lo sabe todo?" recibe dos respuestas
  separadas, no una respuesta que significa "leí todo".
- Una referencia que no se pudo resolver se puede localizar: fichero, línea y
  motivo.
- Un error del recorrido es visible; una exclusión legítima es visible como
  tal.
- El manifiesto describe el conjunto de ficheros que el grafo leyó, no otro.

**Pruebas:** un test permanente sobre el fixture Terraform real con una
referencia a un símbolo inexistente (la respuesta la declara pendiente y no
dice `complete`); un test del inventario que diferencie error de recorrido y
exclusión; un test que compare el conjunto de extensiones que reconocen el
grafo, el manifiesto y el vigilante. Referencias:
`crates/cognicode-core/src/infrastructure/graph/semantic_layer.rs`,
`crates/cognicode-core/src/infrastructure/graph/source_files.rs`,
`crates/cognicode-core/src/application/ingest/watcher.rs`,
`crates/cognicode-core/src/application/services/analysis_service.rs`,
`sandbox/fixtures/terraform-iac`.
