# ADR-CU-004 — Identidad de proveedor, snapshot y release consumible

**Estado:** PROPOSED · **Bloque:** CU-4

## Contexto

Guard puede etiquetar su caché como core/v{versión de Guard}, y un source-code ZIP de GitHub no constituye producto. CogniCode ya tiene caches/hashes e inventario de distribución. Mantener dos pipelines para certificar lo mismo elevaría coste y riesgo.

## Decisión propuesta

- La identidad durable del grafo se compone de workspace canónico, proveedor
  real (versión, commit y feature-set) y schema. El `FileManifest` persistido
  junto al grafo es la autoridad de paths y contenido; no se duplica dentro de
  `SnapshotKey`.
- `load_or_build_graph` mantiene una sola decisión de reutilización: valida
  proveedor/schema, decodificación e integridad, y después compara el manifest
  contra el árbol actual mediante hashes de contenido. Cambios de identidad o
  del manifest causan miss, nunca «falso hit».
- La configuración publicada (`BasisDto.config_digest`) y el contexto de
  consumidor (`AnalysisBasis.profile`, budget y scope de consulta) describen
  la respuesta o la operación que la consume. No forman parte del snapshot del
  grafo mientras no cambien la semántica del grafo producido. Si una futura
  capability sí cambia esa semántica, se incorpora al proveedor/features y se
  versiona el schema según corresponda; no se crea una segunda caché.
- Reutilizar cache/incremental existentes y evitar persistir cachés dentro del
  workspace cuando la operación está tipada `Read`, salvo permiso/ubicación
  explícitos.
- Entrega: PipelineK actual (`merge-gate`, `integration`,
  `release-candidate`, `release`) sin un orquestador paralelo. Candidate
  immutable → release assets → descarga+verificación; presencia de todo el
  set exigido además de integridad de cada asset.
- Gate proporcional: inner loop afectado; full antes de PR/tag según
  `AGENTS.md`; aceptación de distribución solo sobre binarios del release.
  Retirar UAT de sesión con `delivery-first-testing` tras conservar evidencia.
- En caso de incompatibilidad N−1 o Guard aún no migrado, el estado es
  `BLOCKED_EXTERNAL`, no PASS. Publicar rollback/changelog claros.

## Trazabilidad de la decisión

La inspección de `save_durable_snapshot`/`load_durable_snapshot` confirma que
la tupla durable actual solo contiene schema, `SnapshotKey`, grafo y manifest.
La inspección de `compute_basis` confirma que `config_digest` se calcula para
la respuesta a partir de `HandlerContext.log_level`, no para la decisión de
reutilización. Esta frontera permite elegir B sin migrar bincode ni cambiar la
superficie serializada: la aceptación se concentra en el seam existente de
validación de snapshots y en la provenance publicada.

## Alternativas rechazadas

- Cache key = `mtime` y tamaño: no detecta ciertas ediciones.
- Cache key = hash de contenido sin path/version: puede reutilizar semántica equivocada.
- Reconstruir en publish lo certificado en RC: riesgo de mismatch.
- Publicar sources.zip como release de producto: no instalable.

**Resultado buscado:** rendimiento medido + compatibilidad observable + assets correctos. Reversión limitada por hito y versión.
