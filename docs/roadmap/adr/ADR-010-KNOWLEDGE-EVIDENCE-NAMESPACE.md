# ADR-010 — Namespace split entre `RunLineageStore.Evidence` y `EvidenceStore.KnowledgeEvidence`

- **Estado**: ACEPTADO
- **Fecha**: 2026-09-25
- **Contexto**: Post-PRF / F0.* / E1 (LadybugDB-backed Durable Knowledge)
- **Decisor**: arquitecto (operador) sobre propuesta del agente L4.4
- **Refs**: ADR-009 (E1 scope), `docs/prf/FINAL-STATE.md` §31, `crates/cognicode-ladybug/src/init_schema.rs::evidence_ddls`

## 1. Contexto

El cierre de PRF (FINAL-STATE §31) advirtió:

> *Dos nombres iguales en dominios distintos no se fusionan. `EvidenceStore` en `domain::evidence_kernel::ports` ≠ `domain::ports::evidence_store`. Un ADR previo a la fusión.*

Cuando E1.W1 implementó la primera versión real del `EvidenceStore` (puerto de lectura del Durable Knowledge) sobre LadybugDB, se topó con este conflicto exactamente en el plano de la tabla Cypher:

| Tabla (Cypher) | Schema | Propietario | Significado |
|---|---|---|---|
| `Evidence` | `id SERIAL`, `revision_id INT64`, `source_kind STRING`, `source_ref STRING`, `valid_from INT64`, `valid_to INT64`, `properties MAP(STRING,STRING)` | `RunLineageStore` (`cognicode_ladybug::run_lineage`) | Provenance de ejecuciones / runs |
| (a crear) | `id STRING PRIMARY KEY`, `workspace_id STRING`, `evidence_id STRING`, `title STRING`, `kind STRING`, `source_path STRING`, `excerpt STRING`, `confidence DOUBLE` | `EvidenceStore` (E1.W1, `cognicode_ladybug::evidence_store`) | Snapshots resumibles para el Spotter |

Ambos son reales, ambos legítimos, ambos viven en el mismo grafo LadybugDB. **No se pueden fusionar** porque:

1. **PK incompatible**: SERIAL (auto-increment numérico) vs STRING sintético (`evidence:{ws}::{eid}`). Forzar un tipo común rompe el contrato de uno de los dos puertos.
2. **Semántica incompatible**: el `Evidence` del `RunLineageStore` modela eventos temporales con `valid_from/to` (provenance / linaje); el `EvidenceStore` modela snapshots resumibles (sin linaje temporal, con `confidence`).
3. **API Rust incompatible**: el dominio `cognicode_core::domain::ports::evidence_store::EvidenceStore` (puerto de E1) y `cognicode_core::domain::evidence_kernel` (puerto del evidence-kernel, gated por `feature = "evidence-kernel"`) son traits distintos que viven en módulos distintos. Ya están separados a nivel Rust — el ADR documenta que **la separación Rust se mantiene en el plano de persistencia**.

## 2. Decisión

La tabla Cypher backing del `EvidenceStore` (E1.W1) se llama **`KnowledgeEvidence`**, NO `Evidence`. Esta elección está implementada en `crates/cognicode-ladybug/src/init_schema.rs::evidence_ddls()` (línea 81) y pineada por los 12 tests inline de `crates/cognicode-ladybug/src/evidence_store.rs`.

Mapeo puerto → tabla:

| Puerto (Rust trait) | Tabla (Cypher) | Schema propietario |
|---|---|---|
| `cognicode_core::domain::ports::evidence_store::EvidenceStore` (default build) | `KnowledgeEvidence` | E1.W1 |
| `cognicode_core::domain::ports::run_lineage_store::RunLineageStore` (default build) | `Evidence` | pre-existente |
| `cognicode_core::domain::evidence_kernel::ports::EvidenceStore` (gated `feature = "evidence-kernel"`, off) | (in-memory; persistencia pendiente) | n/a |

## 3. Consecuencias

### Positivas

1. **Cero colisión de nombres en el grafo LadybugDB**: los `MATCH (n:Evidence)` existentes siguen resolviendo al `RunLineageStore`; los nuevos `MATCH (n:KnowledgeEvidence)` resuelven al `EvidenceStore` de E1 sin ambigüedad.
2. **Aislamiento semántico**: un cambio en el schema de provenance (p. ej. añadir `valid_event_count`) no rompe el trait del Spotter, y viceversa.
3. **Honestidad arquitectónica**: el nombre `KnowledgeEvidence` refleja su rol ("evidence attached to a knowledge graph node") mejor que `Evidence` a secas, que es genérico y confunde dos dominios.
4. **Cumple FINAL-STATE §31**: la lección explícita del cierre de PRF se aplica a tiempo, antes de que se fusione código de los dos puertos.

### Negativas / trade-offs

1. **El nombre `KnowledgeEvidence` es más largo y añade ruido visual** en cualquier log Cypher que lo mencione. Aceptable — la alternativa (prefijo `evidence_knowledge_`) era peor en queries parametrizadas.
2. **Cualquier consumidor que asuma "Evidence = la tabla del nuevo puerto E1"** se va a sorprender. Mitigación: este ADR queda citado desde `evidence_ddls()` (línea 65) para que un `grep ADR-010 init_schema` devuelva contexto inmediato.
3. **Si en el futuro el `evidence-kernel` (feature gated) llega a persistir**, tendrá que usar OTRO nombre distinto de `KnowledgeEvidence` (candidato natural: `EvidenceKernelRecord`). Esto se documenta como **riesgo explícito** en §6.

## 4. Alternativas consideradas

### 4.1 Renombrar la tabla `Evidence` del `RunLineageStore` a `RunLineageEvidence`

Rechazada. El `RunLineageStore` ya tiene docenas de tests, queries pre-existentes en producción y un grafo con datos reales (`revision_id` SERIAL usado en claves foráneas Cypher). Renombrar la tabla requeriría una migración destructiva. PRF cerrada: no hay presupuesto para una migración de datos en v0.98.x.

### 4.2 Renombrar el `EvidenceStore` de E1 a `KnowledgeEvidenceStore`

Rechazada. El puerto `domain::ports::evidence_store::EvidenceStore` ya está publicado en la doc pública y consumido por `cognicode-runtime` (L4.2 E1.W2), `cognicode-cli` (L4.3 E1.W3) y `cognicode-mcp` (L4.3 E1.W3). Renombrar el trait rompería la API pública. ADR-009 ya eligió el nombre `EvidenceStore` — este ADR sólo elige el nombre de la tabla backing, que es interno a `cognicode-ladybug`.

### 4.3 Bases de datos separadas (un `.lbdb` por dominio)

Rechazada. El modelo de LadybugDB está pensado como un **grafo unificado por workspace**; partir el grafo en archivos distintos destruye la propiedad principal (queries cross-domain: "muéstrame la evidence de un run específico del RunLineage"). Además, complica el contrato `bootstrap_ladybug` (L4.2): ahora solo se abre un archivo.

### 4.4 Fusionar los dos `Evidence` en una tabla polimórfica con discriminator

Rechazada. La diferencia semántica (provenance con `valid_from/to` vs snapshot con `confidence`) es demasiado grande para un solo esquema. Una tabla con `kind STRING` + campos NULL-sprawl degrada la legibilidad de las queries y la integridad de los datos.

## 5. Implementación

- `crates/cognicode-ladybug/src/init_schema.rs::evidence_ddls` (línea 81) — DDL `CREATE NODE TABLE IF NOT EXISTS KnowledgeEvidence(...)`. Implementado en commit `7611a589` (E1.W1).
- `crates/cognicode-ladybug/src/evidence_store.rs` — 12 tests inline pinean el nombre y el shape de la tabla.
- Documentación inline (líneas 58-68 de `init_schema.rs`) cita este ADR para contexto inmediato.

## 6. Riesgos abiertos

1. **`evidence-kernel` (feature OFF)**: si alguna vez se materializa con persistencia, NO puede usar `KnowledgeEvidence` como tabla — el nombre está cogido. Acción futura: el ADR correspondiente (ADR-011+ cuando proceda) deberá documentar el tercer nombre antes de empezar la implementación.
2. **Migración de datos legacy**: si una versión futura necesita unificar los dos `Evidence`, esa migración es destructiva (mover filas + cambiar PK). Por ahora, esa unificación está **explícitamente fuera del scope del roadmap**.
3. **Documentación cruzada**: el comando `cognicode-cli` debería exponer, en una versión futura, un listado de las tablas Cypher que toca (`cognicode doctor --schema` o similar). Esto NO es trabajo de E1.W1-E1.W3; queda registrado como idea en el backlog Post-PRF.

## 7. Cierre

ADR-010 cierra el colgajo que E1.W1 dejó explícitamente abierto (ver `init_schema.rs` líneas 65-68: *"the future ADR-010 will document the namespace split"*). Con este ADR:

- `KnowledgeEvidence` queda como nombre canónico del backing de `EvidenceStore` (E1.W1).
- El `Evidence` del `RunLineageStore` queda protegido de cualquier conflicto por nombre.
- La lección FINAL-STATE §31 se aplica preventivamente, no reactivamente.
