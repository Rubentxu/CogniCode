# Kotlin evolution — INTEGRATION

**Fecha de integración:** 2026-10-17
**Baseline al integrar:** `main@755e97a6` (v0.101.21, post-CU-2.5 #376)
**Estado del paquete upstream:** `docs/cognicode-kotlin-roadmap/` (no
tracked; vive en working tree bajo `docs/*` y `docs/roadmap/*` gitignore,
decisión operador 2026-06-24).
**PR de K0 en curso:** #377 (`k0/honest-coverage` → `main`).

## Por qué este directorio existe

El paquete upstream declara en su `README.md`:

> **Tipo:** propuesta de evolución incremental, NO una segunda agenda.
> **Autoridad en destino:** `docs/roadmap/ROADMAP.md` y `AGENTS.md`.

Su `apply.py` deposita los documentos en
`docs/roadmap/kotlin-evolution/`, ruta **gitignored** (línea 200 de
`.gitignore`, decisión operador 2026-06-24). Aplicarlo produciría
diecisiete archivos huérfanos visibles solo en working tree y un único
puntero "KOTLIN-EVOLUTION [PROPOSED]" en `docs/ROADMAP.md` apuntando a
contenido que no viaja con el repo (404 en clone limpio).

En su lugar, este directorio `kotlin-evolution/` bajo
`docs/roadmap/consumer-contracts/` (que **sí** está en la whitelist
`!/docs/roadmap/consumer-contracts/**/` desde la línea 207) recibe
los 17 archivos del paquete con tres adaptaciones honestas:

1. **Baseline actualizado.** El upstream declaraba
   `baseline_main_sha = 88f6d02a` y `last_release_verified = v0.101.17`.
   La realidad al integrar es `main@755e97a6` y v0.101.21; los
   MILESTONES se anclan a esa verdad.
2. **K0 marcado `[en PR #377]`** (no `[x]`) hasta que el merge-gate
   apruebe y squash-merge a `main`. Si el gate falla, el doc no
   miente. Esto es la diferencia entre "el código está en una rama"
   y "el código está en `main`".
3. **`tools/apply.py` y `README.md` del upstream, omitidos.** El
   primero queda reemplazado por este `INTEGRATION.md` (es el
   integrador vivo); el segundo es descripción del upstream, no
   del estado integrado. La identidad del paquete upstream se
   preserva en el campo `pack-manifest-hash` abajo para
   auditabilidad.

## Lo que cambió de upstream → integrado

| Archivo upstream (en `docs/cognicode-kotlin-roadmap/`) | Destino integrado | Diff |
|---|---|---|
| `docs/roadmap/kotlin-evolution/MILESTONES.md` | `kotlin-evolution/MILESTONES.md` | Baseline al `main@755e97a6`; K0 marcado `[en PR #377]`; añade bloque "K0 — qué se cerró y qué queda" |
| `docs/roadmap/kotlin-evolution/AGENT-HANDOFF.md` | `kotlin-evolution/AGENT-HANDOFF.md` | Sin cambios; las instrucciones se leen tal cual |
| `docs/roadmap/kotlin-evolution/AGENT-TEAMS.md` | `kotlin-evolution/AGENT-TEAMS.md` | Sin cambios |
| `docs/roadmap/kotlin-evolution/GLOSSARY.md` | `kotlin-evolution/GLOSSARY.md` | Sin cambios |
| `docs/roadmap/kotlin-evolution/RELEASE-RUNBOOK.md` | `kotlin-evolution/RELEASE-RUNBOOK.md` | Sin cambios |
| `docs/roadmap/kotlin-evolution/TEST-STRATEGY.md` | `kotlin-evolution/TEST-STRATEGY.md` | Sin cambios |
| `docs/roadmap/kotlin-evolution/TRACEABILITY.md` | `kotlin-evolution/TRACEABILITY.md` | Sin cambios |
| `docs/roadmap/kotlin-evolution/adr/ADR-K00{1..4}.md` | `kotlin-evolution/adr/ADR-K00{1..4}.md` | Sin cambios; el estado `PROPOSED` se conserva hasta aceptación del maintainer |
| `docs/roadmap/kotlin-evolution/specs/SPEC-*.md` (5) | `kotlin-evolution/specs/SPEC-*.md` | Sin cambios |
| `docs/roadmap/kotlin-evolution/uat/ACCEPTANCE.md` | `kotlin-evolution/uat/ACCEPTANCE.md` | Sin cambios |
| `tools/apply.py` (upstream) | — | **No integrado.** Offline por defecto, idempotente pero gitignored. Este INTEGRATION.md es su reemplazo vivo. |
| `README.md` (upstream) | — | **No integrado.** Describe el upstream, no el estado integrado. |
| `PACKAGE-MANIFEST.json` (upstream) | — | **No integrado.** Manifiesto del upstream, no del estado integrado. |

## Cómo se reconcilia con `KOTLIN-LINE-K0-K3.md`

[`docs/roadmap/consumer-contracts/kotlin/KOTLIN-LINE-K0-K3.md`](../kotlin/KOTLIN-LINE-K0-K3.md)
es la **línea de trabajo** dentro del programa Consumer-First (CU-2.x).
Este directorio `kotlin-evolution/` es la **propuesta de subprograma
Kotlin** (K0–K4) con SPECs, ADRs, TEST-STRATEGY y UAT. La línea es un
hilo de trabajo; el subprograma es su marco contractual. El primero se
ejecuta dentro del roadmap principal; el segundo es el **anexo
técnico** del que la línea bebe.

Reglas de autoridad:

- `docs/roadmap/ROADMAP.md` §11 (CU) es la agenda; el estado, prioridad
  y work item siguiente se actualizan solo allí.
- `docs/roadmap/JOURNAL.md` recibe los recibos append-only.
- `docs/roadmap/consumer-contracts/ROADMAP.md` es el roadmap operativo
  del programa CU; lista las WUs de la línea Kotlin.
- Este directorio es la **propuesta de plan**, no la agenda.
- `docs/prf/` permanece histórico, no se reabre.

## K0 — qué se cerró y qué queda (status honesto)

`MILESTONES.md` lista K0 con seis criterios. El K0 commitado en
`k0/honest-coverage` cierra tres de seis; los tres restantes requieren
K0.1 o K1:

| # | Criterio K0 (upstream) | Estado al integrar | Comentario |
|---|---|---|---|
| 1 | Reconciliar `main`, tag, `CURRENT`, roadmap, enlace CLI/MCP 2026-Q4 | **Cerrado (parcial)** | Main y `CURRENT` reconciliados a `755e97a6`; el enlace `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md` sigue ausente y NO se crea (decisión del upstream: no inventar). |
| 2 | Selección única de fuentes vía `LanguageConfig` + política de exclusión | **Cerrado** | `per_file_graph::classify_walk_extension` único, autoridad única. `strategy.rs` ×3 sitios y `analysis_service.rs` derivados. |
| 3 | Per-file `Indexed, Unsupported, IgnoredByPolicy, ParseFailed, ReadFailed, Cancelled, Unknown` con razones acotadas | **Cerrado (parcial)** | K0 introduce `WalkClassification::{Analyze, KnownUnsupportedGap, LegitExclusion}`. La taxonomía completa (7+ variantes) del upstream es más granular; integrar la taxonomía completa queda como **K0.1**. |
| 4 | `Complete` solo relativo a alcance explícito; no inferir de `Vec<SkippedFile>::is_empty()` si se filtró el universo antes | **Abierto** | El actual `BuildStatus::Complete` sigue siendo ingenuo. Defecto que K0.1 cierra con la taxonomía del punto 3. |
| 5 | `SkippedUnchanged` incremental ≠ `SkippedUnsupported`; hashes/identidad y `PRF-ANA-03` | **Abierto** | Sin cambios; preservado por status quo. |
| 6 | Diagnóstico CLI/MCP vía DTO existente con conteos y rutas omitidas | **Abierto** | Sin cambios; queda como K0.1. |
| 7 | `product/languages.json` por contrato; Kotlin `unsupported/not_advertised` | **Cerrado** | `ALL_TREE_SITTER_LANGS` ya no incluye `kotlin`; el Lint `--strict` pinea el constant contra `CAPABILITIES-MATRIX.md`. |

**Cierre de K0 (el commit)**: criterios 2, 3 (parcial) y 7. El gate
`merge-gate` lo acepta si el lint de capabilities pasa (veredicto
`2026-10-17-k0-honest-coverage.md`).

**K0.1 propuesto (no iniciado)**: criterios 3 (resto), 4, 5, 6. Una
sola WU acotada que sella los tres huecos que el K0 deja entreabiertos.
Dependencias: K0 cerrado y `main` con K0. Ver `MILESTONES.md` §K0.1.

## Paquete upstream: estado y huella

| Atributo | Valor |
|---|---|
| Path upstream | `docs/cognicode-kotlin-roadmap/` |
| Tracking | **No tracked** (working tree only; `docs/*` línea 153 + `docs/roadmap/*` línea 200) |
| Estado declarado | `PROPOSED_NOT_EXECUTED` (mantenido por upstream) |
| Baseline declarado | `88f6d02afc9c90fe7ac3c5c4de975d9b9f8c8abb` (v0.101.17, obsoleto) |
| `apply.py` | Idempotente, dry-run por defecto, aborta si encuentra una autoridad distinta (comprobación `AGENTS.md` y `## 7. Referencias` en ROADMAP.md); rechaza destinos con symlink o ficheros distintos |

Si en el futuro el operador decide relajar la política `.gitignore` para
abrir un subdirectorio `kotlin-evolution/` bajo `docs/roadmap/` (con
whitelist explícito), `apply.py` queda preparado para correr `--apply`
sin sobrescribir nada distinto. **No es prerrequisito de K0 ni K1.**

## Veredicto de integración

K0 cierra el headline de "cobertura honesta" sin reabrir PRF, sin
crear segundo roadmap y sin tocar el authority de `ROADMAP.md`. Lo que
el upstream llama "K0" se cierra en dos pasadas: K0 (este commit) y
K0.1 (taxonomía completa + `CompleteForScope` + diagnóstico CLI/MCP).
K1 sigue siendo la dependencia dura del primer Kotlin utilizable; sin
K1, K0–K0.1 son honestos pero parciales.
