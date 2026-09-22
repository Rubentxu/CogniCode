# PRF-CLI-03 — UAT binario real: workspace sin Explorer/RPC/cloud/OTLP

Fecha: 2026-09-22

3/3 PASS sobre binario real con OTEL endpoint apuntando a puerto
loopback cerrado (127.0.0.1:1 = collector ausente):

1. `analyze .` funciona sin collector/Explorer/RPC (exit 0, output).
2. Selección de workspace por ruta canónica: exit 0.
3. Archivo con extensión no soportada dentro del workspace: sin crash,
   el contenido parseable sigue disponible — fallo parcial no se
   convierte en fatal ni se oculta (honestidad parcial).

Sin defectos de producto. Evidencia y test:
`crates/cognicode-cli/tests/prf_cli_03_workspace_uat.rs`.
