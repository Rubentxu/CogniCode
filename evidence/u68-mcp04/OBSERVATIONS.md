# PRF-MCP-04 — argumentos desconocidos: error tipado

Fecha: 2026-09-22

Defecto: `BuildGraphInput` ignoraba silenciosamente argumentos
desconocidos (enviar `path` en vez de `directory` reconstruía el
workspace por defecto sin avisar). Fix: `#[serde(deny_unknown_fields)]`
→ error tipado que nombra el campo desconocido y el esperado.

UAT binario real: `path` → "unknown field `path`, expected
`directory`"; campo extra `bogus` → error; `directory` correcto → OK.
Regresión: MCP completa verde (los tests que usaban la clave incorrecta
`path` fueron corregidos — su "verde" anterior era falso positivo por
el silent-ignore).
