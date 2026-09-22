# U24 / PRF-DIST-03 — Instalación corrupta → rollback + reinstall (2026-09-22)

Entorno: HOME sandbox desechable (/tmp/dist03/home), servidor HTTP local
sirviendo RC 0.97.3 (x86-64). HEAD 69481cf2, binario cogh release local.

## Intento 1 — asset corrupto (truncado a 500000 bytes)
- Descarga 200 OK desde el mirror local (COGNICODE_ASSET_BASE_URL reescrito).
- `Error: install failed: SHA256 mismatch: downloaded file does not match
  expected hash` — fallo LIMPIO, exit no-cero de la transacción, sin
  `|| true`.
- Estado posterior verificado: `versions/` vacío, `journal/` vacío, `shims/`
  vacío → rollback completo, cero estado parcial.
- `cogh doctor` posterior: MCP UNAVAILABLE con mensaje honesto ("no active
  runtime (no version pinned in tracker)") + guía de recuperación.

## Intento 2 — asset correcto (reinstall)
- Misma versión, mismo staging: instalación completa, shims creados,
  skills dual integradas, doctor `overall: healthy`, MCP PASS.
- `cognicode-mcp --version` → 0.97.3.

## Contrato PRF-DIST-03
- Binario ausente/URL rota → cubierto por los 404 observados en U05/U24
  setup (fallo limpio, sin estado parcial).
- SHA inválido → SHA256 mismatch, rollback verificado.
- Reinstalación tras fallo → éxito sin limpiar manualmente.

Estado: PASS. Evidencia: este documento + logs del servidor local
(200/404 series) + transcripción de los comandos en JOURNAL §46.
