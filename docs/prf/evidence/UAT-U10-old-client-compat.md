# UAT — U10 (old client vs candidate, EXT-06/CLI/MCP)

**Fecha**: 2026-09-22
**Binarios**: cognicode v0.97.3 (tag `daabf848`, worktree build) vs HEAD `4368367c`
**Entorno**: local, corpus versionado `docs/prf/fixtures/u10_compat_corpus/`
**Operador**: jcode-orchestrator (AUTO, preautorizado)

## Escenario

Given el binario publicado v0.97.3 y el candidato HEAD, When ambos ejecutan
los subcomandos `graph full/entry-points/leaf-functions/hot-paths/per-file`
sobre el mismo corpus versionado, Then el conjunto semántico publicado es
idéntico y el candidato es determinista run-over-run.

## Resultados

| Comando | stdout semántico | Notas |
|---|---|---|
| graph full | IDÉNTICO | |
| graph hot-paths | IDÉNTICO | |
| graph per-file | IDÉNTICO | |
| graph entry-points | mismo SET, orden canónico nuevo | defecto #5: orden HashMap no determinista en AMBOS binarios; corregido en HEAD (`4368367c`) |
| graph leaf-functions | mismo SET, orden canónico nuevo | idem |

## Hallazgos

1. **Defecto (corregido):** `CallGraph::roots/leaves` iteraban HashMap →
   salida no determinista (md5 distinto por ejecución, verificado x4/x5).
   Fix: orden canónico por SymbolId. Test RED→GREEN
   `roots_and_leaves_are_canonically_ordered`. Core 2147/0.
2. **Hallazgo compat:** el binario del tag v0.97.3 escribe logs INFO en
   **stdout** (viola PRF-CLI-02); HEAD lo dirige a stderr (PRF-F1.W1 H6).
   El candidato es compatible hacia adelante; el defecto era del cliente
   viejo. Comparación semántica hecha filtrando INFO del viejo.
3. stdout/exit codes idénticos para todo lo demás. No hay breaking change
   de esquema CLI en 0.97.3 → HEAD.

## Veredicto

PASS para U10 en su alcance ejecutable sin publicar release (old client =
tag v0.97.3). EXT-06: la mitad de compatibilidad queda cubierta; la
partiente de "contract tests schema/version/capabilities" ya está pinada
por PRF-CLI-04/SEC-02 UATs y MCP tools/list paginada.
