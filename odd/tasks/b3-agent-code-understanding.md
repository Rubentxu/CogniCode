# B3 — Agent Code Understanding (v0.101.13)

> **Estado**: PENDING. Desbloqueado por B2.
> Detalle espeficado: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b3--agent-code-understanding).

## Valor

Un agente que entra en un repositorio desconocido puede encontrar los puntos de entrada, localizar símbolos, entender relaciones, leer fragmentos relevantes y navegar sin reconstruir repetidamente el contexto.

## Prerrequisitos

- B2 cerrado: modelo común de ejecución + capabilities unificadas.

## Work units

- [ ] **B3.W1** — Navegación semántica unificada. Consolidar `get_file_symbols`, `query_symbol_index`, `go_to_definition`, `find_references`, `find_usages`, `get_call_hierarchy`, `get_implementors`, `get_type_references`. Identidades no ambiguas para homónimos, módulos distintos, providers de precisión variable. NO devolver AST heurística como LSP exacta.
- [ ] **B3.W2** — Contexto progresivo. Encadenar `codebase_map`, `smart_search`, `get_symbol_code`, `read_file`, `search_content`, `retrieve_and_verify`. Posible CLI `cognicode inspect` o `cognicode context` (primero contrato, después compatibilidad).
- [ ] **B3.W3** — Descubrimiento orientado a agentes. El catálogo responde: qué puedo hacer, qué operación resuelve la intención, qué argumentos faltan, qué análisis previo se necesita, qué resultado consultar después, qué limitación impide continuar. Metadatos tipados/affordances, NO prompts en el dominio.
- [ ] **B3.W4** — Análisis incremental fiable. Invalidación dependiente del contenido (no solo mtime). Pruebas en frío, caliente, post-edit, post-cambio de rama Git. NO publicar resultado stale como actual.

## Acceptance criteria (resumen)

- UAT-B3-01..04: agente navega sin indicaciones manuales sobre qué tool usar.
- AAT-B3-01..04: LSP ausente degrada explícitamente; edits sin mtime detectado; workspaces aislados; Unicode/rutas con espacios manejados.
- `bash scripts/test-full.sh` exit 0.

## Out of scope

- Reescribir el grafo de llamadas (eso es B4.W1).
- Cambiar el formato de evidence (B4.W4).

## Riesgos

- **R-B3-W2**: crear `cognicode inspect` o `cognicode context` sin verificar uso real puede saturar el namespace CLI. Mitigación: estudio de uso antes del commit.
- **R-B3-W4**: invalidación por contenido es correcta pero cara. Mitigación: solo para los artefactos donde realmente importa; no para todo.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| _pendiente_ | B3.W1 | _pendiente_ | _pendiente_ |
| _pendiente_ | B3.W2 | _pendiente_ | _pendiente_ |
| _pendiente_ | B3.W3 | _pendiente_ | _pendiente_ |
| _pendiente_ | B3.W4 | _pendiente_ | _pendiente_ |

## Cross-block signals

B3.W1 + B3.W4 son prerrequisito de B4 (impact analysis consume identidades no ambiguas y datos incrementales fiables).