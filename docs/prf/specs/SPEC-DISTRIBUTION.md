# PRF-DIST — Instalación y distribución

- **PRF-DIST-01 MUST:** publicar un manifiesto canónico con artefacto, versión, plataforma, SHA256 y composición. `cogh` (Layer 0) instala/versiona Layer 1 `cognicode` y `cognicode-mcp`; no mezclar propietario externo de `cogh` con datos gestionados por `cogh`.
- **PRF-DIST-02 MUST:** `install → doctor → CLI → MCP → update → rollback → uninstall` se ejecuta contra tarballs reales con HOME/XDG limpios y personalizados, de forma idempotente y sin escrituras fuera de ownership.
- **PRF-DIST-03 MUST:** binario ausente, fallo en descarga, manifiesto incompleto, SHA inválido o migración interrumpida produce error y reversión; ningún shim a binario inexistente ni éxito absorbido por `|| true`.
- **PRF-DIST-04 MUST:** archivos de usuario y config IDE preexistente sobreviven desinstalación/rollback; la instalación modifica solo ámbitos aceptados y mantiene versiones anteriores soportadas.
- **PRF-DIST-05 MUST:** soporte de plataforma se anuncia solo con build/ejecución en runner nativo, corpus y UAT realmente superados. Linux GNU x86_64/aarch64 son candidatas, no certificaciones actuales. MUSL/macOS/Windows permanecen sin promesa hasta certificar.
- **PRF-DIST-06 MUST:** hashes, inventario, procedencia de build y assets de la release coinciden; README/versión y ruta de actualización se prueban desde la release candidata, no un checkout.
- **PRF-DIST-07 MUST:** `explorer-mcp`, `explorer-api` y clientes anteriores son clasificados con pruebas y deprecación antes de cambiar distribución; nada se suprime por simplificar el roadmap.

**Certificación:** C1/C4/C6/C7; U01,U02,U20–U24,U27.
