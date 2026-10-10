# Goal — T1-1c SymbolKind caller migration a canonical vía `From`

> **Estado**: PENDING (opcional, 3 call-sites). Autorizado en el goal
> `v1015-consolidation-and-promote` como T1-1c, tras la re-planificación
> "consolidación con pérdida acotada". Estimación: ~30 min.

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `odd/tasks/v1015-consolidation-and-promote.md` §T1-1c | origen: la subdivisión de T1-1 que rechazó `Other(String)` y aceptó 4 orphans a `Unknown` |
| `crates/cognicode-core/src/sandbox_core/ground_truth.rs:2183,2188,2559` | los **3 call-sites** que aún comparan con variantes legacy (`Impl`, `Static`, `Macro`, `Other(s)`) |
| `crates/cognicode-core/src/sandbox_core/ground_truth.rs` `From<SymbolKind>` | ya añadido en `21fe4e5d` por T1-1c parcial; mapea 11 variantes directo + 4 orphans a `CanonicalSymbolKind::Unknown` con string preservado en `ground_truth`'s propio `Other(s)` |
| `odd/tasks/t1-1b-symbolkind-serde-shim.md` | paralelo en la frontera MCP; T1-1c es el paralelo en la frontera de scoring |

## 1. Objetivo

Migrar los 3 call-sites de `scoring.rs` / ground_truth que aún comparan con
las variantes `Impl`/`Static`/`Macro`/`Other(String)` del canónico
(que ahora son `Unknown` con string preservado en `ground_truth`'s propio
`Other(s)`) para que usen la conversión canónica vía `From<SymbolKind>` en
lugar de matchear variantes legacy.

## 2. Problema

T1-1c en su forma original proponía añadir `Other(String)` al canónico
`SymbolKind`. Eso rompía 15 call-sites (todos los que tenían `match` con
la versión fija de variantes). Se rechazó y se re-planificó como
"consolidación con pérdida acotada": 4 orphans (`Impl`, `Static`, `Macro`,
`Other(String)`) se mapean a `CanonicalSymbolKind::Unknown`, y el
`ground_truth` preserva el string en su propio `Other(s)`.

Quedan 3 call-sites en `ground_truth.rs:2183,2188,2559` (o la numeración
actual) que aún hacen pattern match contra el canónico asumiendo las
variantes legacy existen. Esos 3 sites son el residuo y deben migrarse
para usar la conversión canónica.

## 3. Por qué ahora

T1-1a cerró la consolidación de 4 enums. T1-1b cierra el serde shim en
MCP. T1-1c cierra el lado simétrico en scoring/ground_truth. Sin esto,
los 3 sites quedan en un estado que compila por suerte (gracias a la
representación `Unknown` que T1-1c parcial ya añadió vía `From`) pero
mantiene lógica de comparación con variantes que el dominio dice que no
deberían existir.

## 4. Scope

### 4.1 In-scope

* Identificar los 3 call-sites exactos (grep + read) que comparan contra
  `SymbolKind::Impl`/`Static`/`Macro`/`Other(_)` en `ground_truth.rs`.
* Sustituirlos por la conversión canónica: canonical `From<SymbolKind>`
  ya existe (`21fe4e5d`); los sites deben usar el resultado de esa
  conversión en lugar del `SymbolKind` original.
* Verificar que los scores no cambian en tests de caracterización.

### 4.2 Out-of-scope

* T1-1b (MCP serde shim) — otro goal, paralelo.
* Cambiar la decisión de T1-1c de que `Other(String)` queda en
  `ground_truth` (no en canónico) — eso es ADR-implícito y se mantiene.

## 5. Constraints

* **Sin cambio de comportamiento observable**: los scores antes y después
  de la migración deben ser idénticos (los 4 orphans van a `Unknown` con
  string preservado, así que `CanonicalSymbolKind::Unknown + Other(s)`
  sigue dando el mismo peso).
* TDD: añadir un test de caracterización que pinea el score de un
  fixture que cubre los 4 orphans ANTES de migrar, GREEN después.
* Sin nuevos `unimplemented!()`.

## 6. Work items

| ID | Tarea | Verificación |
|----|-------|--------------|
| **T1-1c.1** | Localizar los 3 call-sites en `ground_truth.rs` que comparan con variantes legacy del canónico | `grep -n "SymbolKind::Impl\|SymbolKind::Static\|SymbolKind::Macro\|Other(" crates/cognicode-core/src/sandbox_core/ground_truth.rs` |
| **T1-1c.2** | RED test de caracterización: pin el score actual para un fixture que cubra los 4 orphans (`Impl`, `Static`, `Macro`, `Other("…")`) | `cargo test` RED si cambia el score |
| **T1-1c.3** | Migrar los 3 call-sites a usar la conversión canónica vía `From<SymbolKind>` (que T1-1c parcial ya añadió en `21fe4e5d`) | test GREEN, `cargo test --workspace` verde |
| **T1-1c.4** | Verificación final + update del v1015 doc T1-1c row | commit, recibo |

## 7. Acceptance criteria

1. T1-1c.1..4 en DONE.
2. `cargo test --workspace` exit 0 sin regresión.
3. Test de caracterización pinea los scores pre-migración.
4. Update a `odd/tasks/v1015-consolidation-and-promote.md` T1-1c row.

## 8. Risks

* **R-1**: los 3 call-sites pueden no ser los que asumimos (la numeración
  de líneas puede haber cambiado tras T1-1c parcial). Mitigación: T1-1c.1
  usa grep, no número de línea fijo.
* **R-2**: si el score cambia por error, el test RED lo detecta. Mitigación:
  caracterización antes de migrar.

## 9. Recibo (append-only)

| Fecha | WU | Commit | Notas |
|-------|----|--------|-------|
| 2026-10-10 | T1-1c.1 | (pendiente commit) | **No había 3 call-sites que migrar**. El grep `SymbolKind::Impl\|Static\|Macro` en `crates/cognicode-core/src/` solo encuentra 9 ocurrencias, **todas dentro de `ground_truth.rs` mismo**: las 3 en el `From<SymbolKind> for CanonicalSymbolKind` (líneas 63, 70, 72) y las 6 en `from_str`/`as_str` (líneas 90, 97, 99, 115, 122, 124) que son el round-trip string-intentional del local `SymbolKind`. **El commit `21fe4e5d` (T1-1c parcial) ya hizo la migración** — los 4 orphans (`Impl`, `Static`, `Macro`, `Other(_)`) se mapean a `CanonicalSymbolKind::Unknown` con string preservado en `SymbolKind::Other(s)`. No quedan sitios fuera de `ground_truth.rs` que comparen contra variantes legacy del canónico. |
| 2026-10-10 | T1-1c.2 | (pendiente commit) | Contract test `t1_1c_ground_truth_orphan_mapping` creado (4/4 verde). Pinea: (1) los 4 orphans colapsan a `CanonicalSymbolKind::Unknown`; (2) los 11 directos van a su variante canónica específica; (3) los 3 "extras" (Variant→EnumVariant, Property→Property, Variable→Variable) van a su contraparte; (4) `Other("...")` preserva el string en el `ground_truth` space. Si este RED, el mapping drift cambia los scores de scoring silenciosamente. |

**Estado**: T1-1c DONE — la migración ya estaba hecha en `21fe4e5d`; este goal solo añadía el contract test que pinea el mapping. Sin código de producción que cambiar.

**Diferencia con el goal doc original**: el goal doc asumía 3 call-sites específicos a 2183, 2188, 2559 que había que migrar. Esos números correspondían al snapshot de líneas en el momento de escribir el goal doc, pero en ese momento los sites ya habían sido migrados (o nunca existieron como tales — la decisión "consolidación con pérdida acotada" en T1-1c.original ya preveía que los 4 orphans van a `Unknown` desde el From, no que habría que migrar matches uno a uno). El work real era el contract test, que ahora existe.
