# Agent Teams: paralelización segura y entregas seriadas

**Principio:** paralelizar investigación y piezas independientes, **no las fuentes de autoridad** (versión, `main`, `Cargo.lock`, catálogo de lenguajes, schemas, release). Un integrador conserva autoridad única por hito. Máximo recomendado tres implementadores + un verificador compartido. Cada subagente tiene scope de archivos, contratos, salida y fecha de integración; sin commits competidores sobre mismo archivo.

## Equipos y ownership

| Rol | Zona de responsabilidad | Salida verificable |
|---|---|---|
| Orquestador/Integrador | `docs/roadmap/ROADMAP.md`, `CURRENT.md`, `JOURNAL.md`, bump y release, reconciliar ramas y conflictos | Una unidad activa, PR serializado, release |
| A — AST/language | `domain/value_objects/language.rs`, `infrastructure/parser/{language_config,tree_sitter_parser,type_ref_walkers}.rs`, grammar dep | Corpus parser y hechos de símbolo |
| B — Grafo/coverage | `infrastructure/graph/{strategy,per_file_graph}.rs`, proyección/evidencias | Universo correcto y aristas/omisión |
| C — Providers/intelligence | `infrastructure/lsp/**`, `application/fact_bridge/**`, futuro importador S3 si disparado | S0/S2/S3 con diagnósticos y compatibilidad |
| D — Consumer/API/evals | CLI/MCP DTO/handlers, `product/{languages,tools}.json` y herramienta de generación; UAT externa | Respuesta consumidor real + schema snapshot |

**Regla de equipo:** se puede dividir por rol dentro del hito solo cuando no pisan los mismos seams; el agente B no reescribe el parser mientras A cambia sus invariantes. Un contrato de interfaz aprobado antes de paralelizar obliga a que todos importen el ADT **único**.

## Paralelismo por hito

| Hito | En paralelo | Secuencial/ownership único |
|---|---|---|
| K0 | B implementa inventario + reporte; D prepara fixture cliente negativo y matrices documentales | Integrador revisa invariantes, A verifica catálogo, versión/merge/release serializados |
| K1 | A gramática/AST; D prepara pruebas CLI/MCP y UX de símbolos con schema preliminar | `Language`/`LanguageConfig` editado SOLO por A; integración API tras fijar identidad y spans |
| K2 | B resolución/grafo con ADT acordado; D prepara fixture JVM de overload/interop | A termina extracción de callsite antes de integrar B; nadie compite en `per_file_graph.rs` |
| K3 | C implementa LSP; D prepara cliente/Unicode/conformance; A verifica spans AST existentes | Coordinador acepta handshake y tier; release cuando provider real y fallback están probados |
| K4 | C spike SCIP separado; D experiencia pregunta/evidencia; B mide graph consultas | S3 solo se activa si diferencial positivo; orquestador fusiona producto y hace release |

## Política worktree/commits

Cada agente trabaja en rama/worktree individual, commits atómicos `feat|fix|test|refactor|docs(scope): ...`, sin `Co-authored-by` automático. No push directo a `main`. Ramas nacen del SHA actual de la unidad; contratos compartidos se fijan en un commit común antes del trabajo paralelo. Preparar PRs aislados si el `merge-gate` soporta su integración; *no ejecutar dos releases del mismo producto simultáneamente*. Si una dependencia cambia, rebase/reconciliar y volver a correr gates del diff real.

**Entrega de cada subagente:** código de producción, tests afectados medidos, paths propietarios, cambios de contrato, diagnóstico de fallos, cualquier `BLOCKED`, cero promesas de PASS sin ejecución. No escribir nuevos roadmaps, duplicar estados ni añadir TODO dispersos. Integrador acepta únicamente tras test y evidencia de consumidor.
