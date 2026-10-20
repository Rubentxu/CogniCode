# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda fuente
> de estado: donde este documento y el ROADMAP diverjan, manda el ROADMAP y
> este snapshot se considera obsoleto hasta su próxima regeneración.
>
> **Snapshot**: 2026-10-20, sobre `d8d60d48757e9d57b10457612a6044b8a232be11`
> (`main`). Versión del workspace: `0.101.23`. Última release publicada:
> `v0.101.23` (2026-10-18, 8 assets). `main` está **26** commits por delante
> de ese tag: hay estabilizaciones mergeadas que todavía no están publicadas.
>
> Este fichero se regeneró desde 519 líneas a esta. La versión anterior mezclaba
> un snapshot con la historia de ocho ciclos: narrativas de programas cerrados,
> una lista de "próximo trabajo" que ya estaba hecha, y un estado de release
> que describía `v0.101.16`. **La historia vive en el Journal**; aquí sólo
> vive el estado. Reconstruirlo cuesta una lectura del Journal, no 519 líneas.

## Unidades abiertas

| Unidad | Qué es | Detalle |
|---|---|---|
| **H3** (K1) | gramática tree-sitter de Kotlin: `Language::Kotlin`, `.kt`/`.kts` | `ROADMAP.md` §11 |
| **H4.4** (K2) | relaciones Kotlin↔Java, sólo cuando se pueden probar | `ROADMAP.md` §11 |
| **H5** | `SnapshotKey` e incrementalidad honesta (CU-005) | `consumer-contracts/specs/SPEC-CU-005-SNAPSHOT-INCREMENTAL.md` |
| **H23** | poda por obligaciones y coste de la fase de tests | Journal, entrada del 2026-10-20 |
| **H24** | el producto dice la verdad: manifiesto, CLI=MCP, ontología | Journal, entrada del 2026-10-20 |

## La misión

Misión `cognicode` declarada (`goal-compass.json`), con **13** candidatos que
llevan evidencia —una ruta que existe— y un criterio de cierre observable.
La cadena encadena sola el siguiente objetivo en cada cierre de turno.

El siguiente trabajo es **C2**: que el manifiesto de invalidación del grafo se
derive del registro de lenguajes, en vez de una lista literal de nueve
extensiones. Hoy `build_graph` puede responder `status="complete"` con un
grafo viejo, porque el manifiesto no ve `.tf`, `.yml`, `.cs` ni `.rb`.

Detalle de la auditoría que abrió la backlog: `docs/roadmap/auditoria-2026-10-20.md`
(copia local) y la entrada del Journal del 2026-10-20.

## Gates

| Gate | Comando | Cuándo |
|---|---|---|
| fast | `bash scripts/test-fast.sh` | durante el evolutivo, con `--filter` mientras la unidad sea una |
| full | `bash scripts/test-full.sh` | antes de PR, antes de tag, antes de bump de SemVer |
| merge-gate | `bash scripts/ci/publish-merge-gate.sh` | publica el veredicto que `main` exige |

`full` incluye el gate de contratos. `merge-gate` corre sobre el árbol de
trabajo, así que **no se edita código mientras corre**: el resultado
correspondería a otro árbol.

## Bloqueos abiertos

- **C8-R (CR-01)**: la firma *contractual* del operador sobre un clon limpio.
  La firma operativa está hecha; la contractual no. Las citas al dossier del
  programa cerrado (`docs/prf/`) son históricas por contrato y no se
  reescriben, pero ese dossier ya no está en el árbol: la re-verificación
  tiene que apoyarse en la certificación vigente y en el Journal.
- **SDDK storage**: `~/.local/share/sddk/data/ledger.sqlite` está vacío y el
  ciclo no arranca. Es operator-gated, no un bug de CogniCode.
- **Binarios release**: algunas suites de CLI requieren
  `$CARGO_TARGET_DIR/release`. Si el binario falta, el fallo es de
  infraestructura local, no una regresión: `cargo build --release --bin cogh`.
- **Release pendiente**: `main` lleva 26 commits sin publicar. Publicar exige
  gates sobre un candidato concreto, binarios instalados verificados y tag
  anotado.

## Dónde está cada cosa

| Qué | Dónde |
|---|---|
| Agenda (la única autoridad) | `docs/roadmap/ROADMAP.md`, §11 |
| Recibo de cada unidad, append-only | `docs/roadmap/JOURNAL.md` |
| Decisiones | `docs/adr/` y `docs/roadmap/consumer-contracts/**/adr/` |
| Contratos publicados del consumidor | `docs/roadmap/consumer-contracts/specs/` |
| Certificaciones | `docs/roadmap/certifications/` |
| Guía de distribución | `docs/distribution/INSTALL.md` |
| Identidad de acciones de la comunidad | `docs/cognicode-community-productization/` |
| Backlog de la misión | `goal-compass.json` |
| Cómo se decide qué se toca | `AGENTS.md` |
