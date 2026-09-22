# PRF-MCP-02 — UAT binario real: stdout puro JSON-RPC + shutdown limpio

Fecha: 2026-09-22

`prf_mcp_02_uat` (1/1): sesión completa (initialize, tools/call
desconocido → error JSON-RPC válido, build_graph con logging denso en
stderr) — cada línea de stdout parsea como JSON-RPC; el servidor sale
solo al cerrar stdin (sin huérfanos).

Sin defectos de producto.
