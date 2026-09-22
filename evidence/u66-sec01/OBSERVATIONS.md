# PRF-SEC-01 — UAT binario real: build_graph valida el directorio

Fecha: 2026-09-22

Defecto REAL encontrado: `handle_build_graph` no validaba el argumento
`directory` contra el allowlist del workspace (los demás handlers sí).
Corrección: `ctx.validator.validate_path(&directory)` en el handler.

UAT (`prf_sec_01_uat`, binario real, 4 vectores):
- `../outside` (traversal) → RECHAZADO: Path traversal attempt
- ruta absoluta fuera del workspace → RECHAZADO: outside allowed workspace
- symlink a directorio externo → RECHAZADO: Symlink detected
- `.` (raíz workspace) → ALLOWED, build normal

Hallazgo adicional documentado (no corregido aquí): los argumentos
desconocidos se ignoran silenciosamente por serde (p.ej. enviar `path`
en vez de `directory` ejecuta con default). Registrado como deuda de
contrato MCP-04 (deny_unknown_fields / honestidad de esquema).

Regresión: MCP suites verdes (5+1+1+1+1).
