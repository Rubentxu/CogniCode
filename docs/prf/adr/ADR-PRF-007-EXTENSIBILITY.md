# ADR-PRF-007 — Extensibilidad mínima, justificada por consumidor
**Status:** PROPOSED · **Fecha:** 2026-09-21.

**Decisión propuesta.** Reutilizar puertos, descriptor de capability versionado, composición por backend y un use case único. La primera extensión prueba una capability sintética read-only con clientes CLI/MCP, compatibilidad y test fail-closed. Registry dinámico, carga de packs, DI container/event bus no se crean mientras una tabla tipada simple sirva.

**Alternativas.** Framework universal de plugins desde el principio (complejidad y superficie de ataque) o copy/paste por tool (connascence).

**Validación:** diff estructural reduce sitios de cambio conjunto y conserva contratos; C3/C5, U08/U10/U26. **Rollback:** retirar capability nueva sin alterar stable.
