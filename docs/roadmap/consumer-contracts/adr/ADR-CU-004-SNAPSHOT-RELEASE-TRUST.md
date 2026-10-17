# ADR-CU-004 — Identidad de proveedor, snapshot y release consumible

**Estado:** PROPOSED · **Bloque:** CU-4

## Contexto

Guard puede etiquetar su caché como core/v{versión de Guard}, y un source-code ZIP de GitHub no constituye producto. CogniCode ya tiene caches/hashes e inventario de distribución. Mantener dos pipelines para certificar lo mismo elevaría coste y riesgo.

## Decisión propuesta

- Identidad de análisis = función pura sobre manifest canónico con rutas, contenido, proveedor real, feature-set, scope, esquema y configuración. La versión procede del **build del proveedor**, no de consumidor.
- Reutilizar cache/incremental existentes. Cambios de identidad causan miss, nunca «falso hit». Evitar persistir cachés dentro del workspace cuando la operación está tipada `Read` salvo permiso/ubicación explícitos.
- Entrega: PipelineK actual (`merge-gate`, `integration`, `release-candidate`, `release`) sin un orquestador paralelo. Candidate immutable → release assets → descarga+verificación; presencia de **todo** el set exigido además de integridad de cada asset.
- Gate proporcional: inner loop afectado; full antes de PR/tag según `AGENTS.md`; aceptación de distribución solo sobre binarios del release. Retirar UAT de sesión con `delivery-first-testing` tras conservar evidencia.
- En caso de incompatibilidad N−1 o Guard aún no migrado, el estado es `BLOCKED_EXTERNAL`, no PASS. Publicar rollback/changelog claros.

## Alternativas rechazadas

- Cache key = `mtime` y tamaño: no detecta ciertas ediciones.
- Cache key = hash de contenido sin path/version: puede reutilizar semántica equivocada.
- Reconstruir en publish lo certificado en RC: riesgo de mismatch.
- Publicar sources.zip como release de producto: no instalable.

**Resultado buscado:** rendimiento medido + compatibilidad observable + assets correctos. Reversión limitada por hito y versión.
