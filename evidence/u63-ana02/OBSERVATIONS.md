# PRF-ANA-02 — UAT binario real: sin descarte silencioso de archivos

Fecha: 2026-09-22

- Corrección estructural: los 6 call-sites de extracción
  (`find_all_symbols_with_path` / `find_call_relationships`) en
  analysis_service.rs (estrategias full, filtered y async) ya no usan
  `unwrap_or_default()` (que convertía errores de parseo en resultados
  vacíos silenciosos). Ahora: full → SkippedFile clasificado;
  filtered/async → exclusión de cobertura parsed + warn! en stderr.
  Los 3 `unwrap_or_default` restantes son fallbacks de mtime a epoch,
  documentados y benignos.

UAT (`prf_ana_02_uat`, binario real): corpus con `locked/secret.rs`
chmod-000 → `build_graph` responde `status: partial` con
`skipped_files` nombrando el archivo, reason_kind=read,
"Permission denied (os error 13)". Nunca `complete` silencioso.

Regresión: core lib 2145/0; graph subset 401/0; analysis+workspace 118/0;
MCP prf_ana_05 5/0; continuation_e2e 1/0.

Contrato SPEC-ANALYSIS: PRF-ANA-02 — "los errores de lectura/parseo no
se descartan silenciosamente" → verificado.
