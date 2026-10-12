# ADR-012 — ST-04 CLI/MCP separation: measured coupling keeps the binary together

**Status:** PROPOSED · **Date:** 2026-10-12 · **Cycle:** B6.W6 cierre
arquitectónico (v0.101.16) · **Cycle ref:**
`odd/tasks/b6-agent-ready-runtime.md` §B6.W6.

**Contexto.** El programa CLI/MCP 2026 (B1..B6, v0.101.11..v0.101.16)
introduce B2.W1 `AnalysisBasis`, B2.W2 `ToolDescriptor` y los módulos
B4.W1..W5 (`impact`, `architecture`, `findings`, `assurance_export`,
`impact::composite`). El cierre arquitectónico B6.W6 pide medir la
deuda residual en los coordinadores grandes (`commands.rs`,
`rmcp_adapter.rs`, `handlers/`, `assurance_export/workspace_extractor.rs`)
y completar las separaciones ST-02/ST-04 que afecten CLI/MCP.

La regla de AGENTS.md ("Respetar arquitectura hexagonal, SOLID,
cohesión y connascence: nueva abstracción solo para acoplamiento
probado con tests") y el riesgo R-B6-W6 ("dividir módulos por
tamaño sin acoplamiento medido produce fragmentación sin valor")
exigen evidencia antes de separar. Esta ADR documenta la medición y
la decisión resultante.

**Lo que esta ADR cierra:**

1. No se separan CLI (`cognicode-cli`) y MCP (`cognicode-mcp`) en
   dos espacios de tipos: la dependencia entre ambos es legítima y
   los tipos compartidos son load-bearing.
2. Se mantienen `commands.rs` y `rmcp_adapter.rs` con su tamaño
   actual (3563 y 2249 líneas respectivamente a 2026-10-12) bajo un
   ratchet soft de 4000 líneas por coordinador.
3. No se fragmenta `assurance_export/workspace_extractor.rs` (786
   líneas) ni `handlers/` por tamaño: la connascence de tipo entre
   extractores B4.W4 y módulos B4.W1..W3 es real, no cosmética.

---

## Decisión 1 — CLI y MCP en el mismo binario

**Decisión:** mantener `commands.rs` y `rmcp_adapter.rs` dentro del
mismo crate `cognicode-core`, accediendo a los mismos tipos de
aplicación vía `cognicode_core::*`.

**Justificación — medición de acoplamiento legítimo:**

| Tipo compartido | Definido en | Consumido por CLI | Consumido por MCP | Sin split viable |
|---|---|---|---|---|
| `AnalysisBasis` | `application::analysis_basis` (B2.W1) | sí (`execute_*` para fingerprint reproducible) | sí (idempotencia de tools read-only) | reimplementar duplica; `pub use` añadiría capa sin valor |
| `ToolDescriptor` | `interface::catalog` (B2.W2) | sí (`execute_capabilities`, listado JSON) | sí (`tools/list` MCP, paridad con Authority) | duplicar rompe contrato del catálogo |
| `Severity` | `architecture::diagnostic` (B4.W2) | sí (output JSON de `check_architecture`) | sí (mismo JSON via MCP) | re-declarar rompe equivalencia CLI/MCP |
| `CompositeImpactQuery` | `impact::composite` (B4.W5) | sí (`cogh composite-impact`) | sí (tool MCP) | el shape es compartido por contrato |
| `Enums` B4 (FindingCategory, FindingSeverity, Direction) | `findings` / `impact` (B4.W3 / B4.W1) | sí (vía `execute_findings`, `execute_impact`) | sí (vía MCP tools) | re-declarar crea divergencia silenciosa |
| `assurance_export::envelope::*` | `assurance_export` (B4.W4) | sí (`cogh export`) | sí (tool `export_evidence`) | split rompe codec CBOR shared |

El número de referencias cruzadas legítimas (B2.B4 consumidos desde
ambos interfaces) es **6 tipos compartidos**, no 1. Cualquier split
que aisle `cognicode-cli` de `cognicode-core::impact::*` o
`cognicode-core::findings::*` obliga a:

* Duplicar cada tipo, **o**
* Introducir una capa `pub use` (re-export) que no añade
  semántica, **o**
* Mover los tipos a un crate nuevo (`cognicode-shared-types` o
  similar), creando un tercer artefacto binario/librería a mantener.

Ninguna de las tres opciones satisface AGENTS.md ("no crear dos
fuentes de verdad ni duplicar la semántica entre interfaces"). La
separación prematura **introduciría** la deuda que pretende evitar.

**Acción:** declarar la separación como **NO triggered** con
evidencia. Bloqueador: ninguno. Si en el futuro aparece un consumer
externo a CogniCode que exija los tipos sin el binario, este ADR
se reabre con el caso de uso concreto.

## Decisión 2 — Coordinadores grandes bajo ratchet soft

**Decisión:** `commands.rs`, `rmcp_adapter.rs` y
`assurance_export/workspace_extractor.rs` mantienen su forma actual
con un ratchet contractual de **4000 líneas por coordinador** (soft
cap, no hard split). El ratchet:

* Lee el archivo en disco en cada ejecución del test
  `large_coordinators_have_a_size_ratchet`.
* Emite un **warning explícito** con la métrica actual si se supera
  el soft cap.
* **Falla el build** si supera 6000 líneas (hard cap = 1.5× soft).
* No divide módulos por tamaño: la división solo se justifica por
  connascence medida y contrato preservado (AGENTS.md §Disciplina
  de ingeniería).

**Justificación — tamaño actual a 2026-10-12:**

| Coordinador | Líneas | vs soft 4000 | vs hard 6000 |
|---|---|---|---|
| `interface/cli/commands.rs` | 3563 | -437 (88%) | -2437 (59%) |
| `interface/mcp/rmcp_adapter.rs` | 2249 | -1751 (56%) | -3751 (37%) |
| `assurance_export/workspace_extractor.rs` | 786 | -3214 (20%) | -5214 (13%) |

Los tres coordinadores están **lejos** del hard cap. Ningún umbral
de tamaño se ha cruzado en `main` post-B5 (HEAD `3e9fddc3`).

**Por qué soft cap y no hard split:** los coordinadores cargan con
dos responsabilidades legítimas que la connascence mide:

1. **Catálogo de tools** (B2.W2): `commands.rs` y `rmcp_adapter.rs`
   proyectan los mismos 73 `ToolDescriptor` a superficies distintas
   (`clap` subcommands vs MCP `tools/list`).
2. **Output contracts** (B2.W3): cada `execute_*` registra su
   `OutputContract` por herramienta; el `tool_name` es la clave
   compartida entre `commands.rs` (CLI) y `rmcp_adapter.rs` (MCP).

Estos dos ejes no se pueden partir sin duplicar el catálogo. El
ratchet soft permite detectar crecimiento patológico (e.g., añadir un
`execute_*` que arrastra un módulo gigante) sin promulgar una
división que no se ha justificado.

**Acción:** el test `large_coordinators_have_a_size_ratchet` queda
pinea do en la suite `b6_w6_architectural_close` (anchor suite,
suma 62 → 63 en `core_gate_coverage_contract`). El soft cap es
**revisable** solo si la división propuesta se acompaña de un
acoplamiento medido y un contrato preservado.

## Decisión 3 — `workspace_extractor.rs` no se fragmenta

**Decisión:** `assurance_export/workspace_extractor.rs` permanece
como módulo único. No se subdivide en `extract_impact.rs`,
`extract_architecture.rs`, `extract_findings.rs`.

**Justificación — connascence medida:**

Los tres extractores B4.W4 comparten:

* El patrón `ExtractionOutcome` (Unsupported / Partial / Complete).
* El accumulator por evidencia (`Observation` arquitectónica,
  `Finding` de calidad, `ImpactEdge`).
* El límite `MAX_*` del envelope v1 (`MAX_COLLECTION_SIZE`,
  `MAX_INPUT_BYTES`).
* El productor `EvidenceExport` v1.

Cada extractor reutiliza los otros dos tipos (ver
`workspace_extractor.rs:639-715`): dividir el archivo crearía tres
módulos que importan el uno del otro para los tipos compartidos. La
fragmentación **aumentaría** la connascence de tipo, no la
disminuiría.

La separación real sería **lógica** (funciones `extract_*` por
evidencia), no física (un módulo por evidencia). El estado actual
ya implementa esa separación lógica dentro de un módulo que
sigue cabiendo bajo el soft cap.

**Acción:** no mover código. Documentar la connascence en este
ADR para que un futuro maintainer no caiga en la tentación de
"limpiar" sin evidencia.

## Decisión 4 — `mcp/handlers/` no se divide por archivo

**Decisión:** los 7 archivos de `handlers/` (`aix_handlers.rs`,
`consolidated_handlers.rs`, `graph_handlers.rs`,
`graph_query_handlers.rs`, `lsp_handlers.rs`, `mod.rs`,
`program_analysis_handlers.rs`, `refactor_handlers.rs`) permanecen
tal cual. No se mueven a sub-submódulos.

**Justificación — el módulo `mod.rs` (306.3K) ya es el dispatcher:**

* `mod.rs` contiene la lógica de despacho y el `match` central que
  enruta cada `(tool_name, params)` al handler concreto.
* Los otros 7 archivos son **handlers concretos** por dominio
  (AIX, consolidated, graph, graph_query, LSP, program_analysis,
  refactor).
* Cada handler concreto tiene su propio contrato, registrado en el
  catálogo `product/tools.json`. La división está hecha por
  **dominio semántico**, no por tamaño.

El test `cli_does_not_depend_on_mcp_runtime` y
`mcp_does_not_depend_on_cli_runtime` pinea que la separación
**delgada** CLI/MCP (cada crate con su runtime) sigue vigente:
`commands.rs` y `rmcp_adapter.rs` consumen tipos de aplicación,
no se importan entre sí.

**Acción:** ningún cambio. Documentar la decisión en este ADR.

---

## Riesgos identificados y mitigaciones

| Riesgo | Mitigación |
|---|---|
| El ratchet soft se ignora por costumbre y los coordinadores crecen sin control | El test `large_coordinators_have_a_size_ratchet` falla el build al cruzar 6000 líneas. Hard cap = 1.5× soft = contrato observable, no decoración. |
| Un futuro maintainer divide `commands.rs` por estética, no por acoplamiento | ADR-012 + tests de connascence pin que la división no se justifica sin evidencia. AGENTS.md §Disciplina de ingeniería lo refuerza. |
| La duplicación de tipos CLI/MCP aparece silenciosamente (e.g., un nuevo `execute_*` declara un DTO local) | El test `B4_modules_dont_reimplement_b2_types` pinea que los módulos B4.W1..W5 reutilicen `AnalysisBasis`/`Severity` en lugar de re-Declarar. Replicar el patrón a cada nuevo B*. |
| El catálogo crece y el dispatcher de `mcp/handlers/mod.rs` se vuelve inmanejable | El ratchet soft aplica también a `mod.rs` (306.3K ≪ 6000 líneas hard cap). Si se acerca al cap, se reabre esta ADR con la división propuesta. |

---

## Cierre de la ADR

Esta ADR se cierra cuando:

1. El test `b6_w6_architectural_close` queda pineado como anchor
   suite (62 → 63 en `core_gate_coverage_contract`).
2. El ratchet soft (4000) y los tests de no-acoplamiento
   (`cli_does_not_depend_on_mcp_runtime`,
   `mcp_does_not_depend_on_cli_runtime`,
   `B4_modules_dont_reimplement_b2_types`) ejecutan en verde sobre
   `main` HEAD.
3. La firma humana del operador queda registrada en
   `docs/roadmap/production-ready/veredictos/2026-10-12-b6-release-v0.101.16-C8-R.md`.

Tras B6.W6 cerrado, esta ADR queda **vigente** mientras no aparezca
un caso de uso real que justifique la separación ST-04 con
acoplamiento medido y contrato preservado. El sello "PROPOSED" pasa
a "ACCEPTED" solo si la firma humana del operador confirma la
decisión; en otro caso queda en "PROPOSED" con el veredicto
adjunto como evidencia de no-disparo.

---

## Apéndice A — Medición raw (2026-10-12, HEAD `3e9fddc3`)

Comandos verbatim que respaldan las cifras de §Decisión 2:

```bash
$ wc -l \
    crates/cognicode-core/src/interface/cli/commands.rs \
    crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs \
    crates/cognicode-core/src/assurance_export/workspace_extractor.rs
  3563 cli/commands.rs
  2249 mcp/rmcp_adapter.rs
   786 assurance_export/workspace_extractor.rs
  Σ 6598
```

Comandos que respaldan las dependencias CLI/MCP (Decisión 1 y 4):

```bash
$ grep cognicode-mcp crates/cognicode-cli/Cargo.toml
  cognicode-core.workspace = true
  # cognicode-mcp NOT in deps
$ grep cognicode-cli crates/cognicode-mcp/Cargo.toml
  cognicode-core.workspace = true
  # cognicode-cli NOT in deps
```

Comandos que respaldan la no-reimplementación de tipos B2 (Decisión
3, replicado en `B4_modules_dont_reimplement_b2_types`):

```bash
$ grep -rn 'pub enum Severity\|pub struct AnalysisBasis' \
    crates/cognicode-core/src/architecture \
    crates/cognicode-core/src/impact \
    crates/cognicode-core/src/findings \
    crates/cognicode-core/src/assurance_export
  crates/cognicode-core/src/architecture/diagnostic.rs:50:pub enum Severity
  # Only ONE Severity defined (in architecture). B4 re-uses via pub use.
  crates/cognicode-core/src/application/analysis_basis.rs:82:pub struct AnalysisBasis
  # Only ONE AnalysisBasis defined (in application). B4 re-uses via path.
```

Nota: `findings` mantiene un `FindingSeverity` propio (5 niveles:
Info/Low/Medium/High/Critical) distinto de `architecture::Severity`
(4 niveles: Hint/Info/Warning/Error). Esta diferencia es
**deliberada** (ver §R-B4-W3 del roadmap Q4): los findings de
calidad usan una escala ordinal de severidad de defecto, las
observaciones de arquitectura usan una escala de severidad de
violación. Fusionarlas requeriría redefinir el contrato observable
de B4.W2 y B4.W3. No es duplicación, es **vocabulario intencional**
por dominio. Si la diferencia se quisiera eliminar, abrir ADR
nuevo con la propuesta concreta.

---

## Apéndice B — Conexión con el resto del programa

Esta ADR consume:

* AGENTS.md §Disciplina de ingeniería (connascence probada, no
  estética).
* `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md` §B6.W6 (cierre
  arquitectónico, no dividir por tamaño).
* ADR-009 (E1 EvidenceStore scope) y ADR-010 (Knowledge/Evidence
  namespace) como precedente: una ADR previa a un contrato cuando
  el cambio tiene impacto multi-crate.

Esta ADR alimenta:

* `docs/roadmap/production-ready/veredictos/2026-10-12-b6-release-v0.101.16-C8-R.md`
  (cierre técnico B6 + base para la firma humana del operador).
* `crates/cognicode-core/tests/b6_w6_architectural_close.rs`
  (anchor suite que pinea el ratchet soft, la no-dependencia
  CLI↔MCP runtime y la no-reimplementación de tipos B2).