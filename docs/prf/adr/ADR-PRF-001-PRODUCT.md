# ADR-PRF-001 — Producto CLI + MCP local-first
**Status:** PROPOSED · **Fecha:** 2026-09-21 · **Decisión esperada:** F0.

**Contexto.** Existen `cognicode` CLI, `cognicode-mcp`, `explorer-mcp`, `explorer-api` y `cogh` gestor, con contratos no necesariamente equivalentes. El roadmap anterior los mezcla con servicios remotos y UI.

**Decisión propuesta.** La primera promesa productiva se limita a un CLI de análisis y un servidor MCP stdio instalados/versionados por cogh; local-first, sin red/OTLP/daemon obligatorio. La matriz de F0 decide qué comandos/tools/lenguajes son `stable`/preview. Conservar wrappers y versiones antiguas mientras existan clientes publicados.

**Alternativas.** Mantener toda la UI/API como condición de lanzamiento (más superficie de fallos); migrar de golpe a `explorer-mcp` (rompe compatibilidad desconocida).

**Consecuencias.** Cada capability stable exige UAT en ambos transportes cuando aplique, pero `cogh` queda fuera del análisis. No prometer paridad para funcionalidades que solo existen en uno de ellos; documentar matriz. La elección de CLI core debe fundamentarse en F0 y puede corregirse sin borrar binarios.

**Validación:** PRF-CLI/MCP/DIST, C0/C1/C3/C7, U01–U10. **Rollback:** restaurar soporte/manifest anterior sin borrar datos.
