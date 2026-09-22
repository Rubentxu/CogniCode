# PRF-CLI-06 — UAT binario real: determinismo, unicode, permisos, secretos

Fecha: 2026-09-22

4/4 PASS sobre binario real:

1. Unicode + espacios en ruta (`árbol con espacios/ñandú.rs`):
   análisis determinista exit 0, reporte completo.
2. cwd distinto sobre workspaces idénticos → reportes idénticos
   (línea "Analyzing" excluida; contiene la ruta, legítimamente distinta).
3. Permiso 000 en subdirectorio: sin crash (0 o 1), permisos restaurados
   para cleanup.
4. `-v` con COGNICODE_API_TOKEN / MY_SECRET_KEY / DB_PASSWORD en env:
   ningún valor secreto aparece en stdout+stderr.

Sin defectos de producto.
