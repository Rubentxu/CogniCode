# PRF-SEC — Seguridad, fiabilidad y límites

- **PRF-SEC-01 MUST:** raíz workspace y cada ruta de lectura/escritura se resuelven canónicamente y autorizan; rechazar `..`, absolutos fuera, symlinks externos y TOCTOU relevante en el ámbito soportado. Una validación en un handler no basta como contrato de un futuro adaptador.
- **PRF-SEC-02 MUST:** operaciones read/write/execute/network diferenciadas; read-only es mínimo para nuevas capacidades, cualquier mutación requiere autorización propia. No interpretar texto del repositorio, prompts, comentarios o findings como instrucciones para ejecutar.
- **PRF-SEC-03 MUST:** errores y logs carecen de tokens, credenciales y contenido sensible salvo autorización explícita; telemetría opt-in y sin fuente de código por defecto.
- **PRF-SEC-04 MUST:** CPU, memoria, tiempo, fanout, profundidad, tamaños de input/output y número de procesos poseen límites cuantificados por perfil estable y salida tipada al alcanzarlos.
- **PRF-SEC-05 MUST:** cancelación/shutdown de trabajos costosos libera recursos, locks y archivos temporales; fallos de backend o servicio de métricas no dejan al MCP corrupto ni provocan resultados falsos.
- **PRF-SEC-06 MUST:** ninguna vulnerabilidad CRITICAL/HIGH conocida y explotable sobre el core estable se publica sin mitigación verificada; advisories/licencias y exposición real se inspeccionan sobre el lockfile/bundle del candidato.
- **PRF-SEC-07 MUST:** pruebas adversariales simulan repo malicioso, rutas symlink/traversal, parser fallido, secreto señuelo, herramienta mutante no autorizada, cliente desconectado y corrupt data.

**Certificación:** C1/C2/C4/C5/C6; U04,U06,U14,U18,U19,U21,U25.
