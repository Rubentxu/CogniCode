# UAT PRF — catálogo obligatorio (27 escenarios)

**TODOS NOT_RUN a fecha de planificación.** El número de test de biblioteca no sustituye al UAT real. Para cada fila: source SHA, version/bin SHA256, plataforma/runner, HOME/XDG temporal, corpus revision/hash, argv/JSON-RPC sin secretos, hora, stdout/stderr, exit/status, oráculo y recibo firmado. Contrastar CLI/MCP por igualdad **semántica**, no por identidad de esquema externo.

**Fixtures:** F-RUST (proyecto anidado), F-MIXED (Rust/Python/TS + lenguaje no soportado), F-IDENT (rename/move/colisiones), F-ADV (symlinks, permisos, mtime, secretos señuelo), F-2WS (proyectos con símbolos homónimos), F-INDEX (backup/corrupción/migración), F-BIG (repo real de escala acordada), F-OLD (cliente e instalación anterior). Versionar manifest y sha256, no resultados falsos prefabricados.

| ID | Secuencia UAT | Oráculo / gate |
|---|---|---|
| U01 | Descargar bundle candidato, verificar digest/provenance, instalar con `cogh` en HOME limpio. | Binarios utilizables, identidad del candidato y cero escritura fuera de ownership; C0/C6. |
| U02 | Comparar `cognicode --help`, `cogh doctor`, MCP `tools/list` contra matriz. | Operaciones anunciadas, permisos, lenguajes y artefactos reales coinciden; C0. |
| U03 | Ejecutar baseline/goldens 2 veces en corpus fijado. | Identidad, cobertura, tiempos/RSS y diffs reproducibles; C0. |
| U04 | Apagar red y collector OTLP; iniciar CLI y MCP. | Core disponible, stdout MCP solo JSON-RPC, stderr logs; C1. |
| U05 | Cliente MCP externo: initialize, tools/list, llamada correcta, error, shutdown. | Protocolo válido, error tipado, proceso sale sin huérfanos; C1. |
| U06 | Workspace inexistente/sin permisos/vacío/lenguaje no soportado. | Error/Unknown/Partial/Unsupported, nunca Complete falso; C1/C2. |
| U07 | Workspace Unicode/espacios, cwd diferente y config inválida. | Rutas estables, mensajes útiles, códigos de salida; C1. |
| U08 | Una misma consulta real vía CLI y MCP sobre snapshot fijo. | Igual semántica de símbolos/relaciones/coverage/basis; C3. |
| U09 | Comparar frío/caliente y después editar un fichero. | Delta coherente, sin caché antigua presentada como nueva; C2. |
| U10 | Cliente anterior soportado contra binario candidato. | Mantiene contrato o deprecación aprobada y testada; C3. |
| U11 | Simular fallo parser/provider en ambas interfaces. | Mismo estado de completitud, error tipado y motivo; C2/C3. |
| U12 | Proyecto anidado, estrategias full/per_file y operaciones compartidas. | Subdirectorios incluidos y equivalencia para resultados prometidos; C2. |
| U13 | Cambiar bytes preservando tamaño y mtime; reescanear. | Detecta cambio o se declara incompleto; jamás Unchanged falso; C2. |
| U14 | Denegar lectura/eliminar archivo y superar presupuesto durante análisis. | Partial/Unknown/Failed con cobertura, tiempo/output acotados; C2. |
| U15 | Corpus mixto, LSP ausente y consulta de lenguaje no soportado. | Fallback y nivel de soporte explícitos, sin resolución inventada; C2. |
| U16 | Rename/move/colisiones y símbolos homónimos. | Identidad estable cuando se promete; ambiguo no equivale a match; C2. |
| U17 | Indexar, reiniciar, consultar historia y revisión actual. | Persistencia verificable o reconstrucción honesta, sin stale-complete; C4. |
| U18 | Dos proyectos, dos procesos bajo HOME común, cambios aislados. | 0 contaminación de grafos/snapshots/cachés; C4. |
| U19 | `../`, ruta absoluta, symlink externo, instrucciones de repo y write/exec sin permiso, secreto señuelo. | Rechazo sin fuga ni autoridad derivada de prompts; C5. |
| U20 | Instalar, doctor, CLI, MCP, actualizar, reinstalar idempotentemente. | Binarios disponibles y manifiestos/versiones coherentes; C6. |
| U21 | Cortar proceso durante escritura/migración; reiniciar. | Rollback/recovery seguro, datos previos conservados; C4. |
| U22 | Versión antigua crea datos; upgrade y downgrade. | Migración reversible o rechazo seguro explícito; C4. |
| U23 | Uninstall con HOME/XDG personalizados e IDE de prueba. | Solo recursos propiedad de `cogh` eliminados, sin contaminar config ajena; C6. |
| U24 | Instalación interrumpida/asset corrupto, luego rollback y reinstall. | No éxito sin binario, ni shim colgante, ni pérdida de datos; C6. |
| U25 | Tool costosa cancelada, desconexión cliente y ausencia de OTLP. | Recursos liberados, error/cancelación según contrato, sin corrupción/stdout sucio; C5. |
| U26 | Incorporar capacidad sintética read-only y ejecutar cliente previo. | Nueva capacidad registrada sin cambio del contrato de las existentes; C5. |
| U27 | Inyectar fallo crítico de test, manifest y publicación, ejecutar gates. | Cada fallo **bloquea** CI/release; no SKIP ni `|| true`; C6/C7. |

**Estados:** PASS (ejecutado, aserciones y evidencia completas); FAIL (reproducido); BLOCKED (prerrequisito); NOT_RUN (pendiente); SKIP_NOT_APPLICABLE (exclusión de alcance aprobada **antes** del test). **No usar `SKIP` para una funcionalidad core publicada.**

Ver [certificación](CERTIFICATION.md). El recibo de cada UAT se registra en `evidence/CERTIFICATES.md` y evidencia verificable adjunta al cambio/release, no en comentarios sin hash.
