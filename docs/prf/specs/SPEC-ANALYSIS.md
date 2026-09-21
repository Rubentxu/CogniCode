# PRF-ANA — Correctitud y contrato de análisis

- **PRF-ANA-01 MUST:** cada operación stable declara soporte por lenguaje y precisión semántica (p. ej. AST/LSP/heurística), incompletitud, límites y exclusiones del corpus. `Unsupported` no es grafo vacío válido.
- **PRF-ANA-02 MUST:** `full` y `per_file` cubren todos los archivos del árbol soportado cuando se anuncian como equivalentes; los errores de lectura/parseo no se descartan silenciosamente. `lightweight` no promete relaciones que no calcula.
- **PRF-ANA-03 MUST:** cambiar bytes conservando mtime/tamaño no genera `Unchanged` certificado falso. Manifest, hashes o degradación `Unknown` salvaguardan la correctitud incremental; `mtime` solo optimización.
- **PRF-ANA-04 MUST:** si falta un archivo/provider, la consulta es `Partial/Unknown/Failed` con causa, cobertura y referencias a la base de análisis; nunca `clean/no impact` sin cobertura suficiente.
- **PRF-ANA-05 MUST:** repetir consulta con el mismo repo/config/basis produce outputs semánticamente equivalentes y orden estable donde se publica; frío/caliente no altera respuesta lógica.
- **PRF-ANA-06 MUST:** para consultas con identidad histórica, `basis` incluye workspace canónico+config digest+source manifest/revisión y provenance de provider/graph según capacidad; si la identidad no puede establecerse se declara incompleto.
- **PRF-ANA-07 MUST:** renames, moves, nombres repetidos y colisiones mantienen identidad cuando se promete o devuelven ambigüedad visible, sin vinculación especulativa.
- **PRF-ANA-08 MUST:** lectura, parser y consultas costosas tienen budgets y limitaciones cuantificadas; la superación de budget produce error/estado tipado y salida acotada.
- **PRF-ANA-09 MUST:** al integrar LSI se compara el camino nuevo con el histórico mediante golden corpus, se registra drift y no se realiza cutover sin equivalencia o cambio de contrato aprobado.

**Certificación:** C2/C3/C4; U03,U06,U08,U09,U11–U17. Precondición: fixture y oracle independientes de la propia implementación.
