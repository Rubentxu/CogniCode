# Estrategia de pruebas por capas — PRF

La base no se certifica por el conteo de tests; cada nivel tiene oráculos y gatillos definidos.

| Nivel | Ejecución | Propósito | Gate |
|---|---|---|---|
| T0 dominio | tests unitarios de puertos/adts/identidad | Propiedades, invariantes, errores tipados y connascence | cada WU |
| T1 adaptadores | tests de parser/store/fs/LSP por backend | Errores de IO y límites, in-memory vs Ladybug, fault injection | cada WU |
| T2 contratos | golden fixtures de CLI/MCP, snapshots, clients old/new | Compatibilidad, same meaning, no schema drift | F0/F2/F3 |
| T3 procesos | binarios instalados + cliente MCP externo | stdout/stderr, startup, lifecycle, clean HOME, offline | C1/C3 |
| T4 adversariales | symlinks, mtime, permisos, secret, timeout, crash, concurrency | Fallo seguro, no false-complete y recuperación | C2/C4/C5 |
| T5 sistemas | upgrade/migration/downgrade, 2 WS, corpus real por lenguaje | Semántica durable y no mezcla | C4/C6 |
| T6 supply chain | instalador externo, hash, provenance, release payload | Identidad exacta del candidato y rollback | C6/C7 |
| T7 rendimiento | métricas repeatables sobre corpus pinned, en runner comparable | p50/p95, RSS y límites, frío/caliente, regresión | C0/C6/C7 |

**Evidencia por test:** ID, source SHA, binary SHA256, features, config digest, corpus digest, runner, comandos, salida/exit/status, falla inicial y éxito posterior (si fix), timestamp y ubicación de log. No regenerar golden automáticamente ante divergencia.

**Secuencia TDD:** primero reproducir (RED), aislar causa, corregir lo mínimo, GREEN focos, comprobar compatibilidad, UAT de binario real, ejecutar gate del hito. Nunca convertir `SKIP`/test filtrado en PASS global. Conserva `docs/TEST-PLAN.md` histórico en su ruta porque scripts y ADR lo pueden usar; este plan rige PRF.
