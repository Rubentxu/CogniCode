# Sistema de certificación por hitos

**Situación inicial: C0..C7 NOT_RUN/BLOCKED.** Aceptación basada en *evidencia observada*, nunca en casillas, textos de autores o tests de una capa aislada. Un solo ledger de certificados: `evidence/CERTIFICATES.md`. Cada certificado enlaza acta y anexos inmutables con SHA, binario/digest, entorno, corpus/manifest y comandos.

## Niveles (acumulativos)

| Certificado | Condiciones obligatorias | Evidencia/UAT mínima |
|---|---|---|
| C0 Inventario | Matriz cerrada de capacidades publicadas y candidatas: comandos, tool IDs/schema, lenguajes, permisos, errores, BIN SHA, backend/flags, versión; corpus y baseline p50/p95 RSS por perfil; defectos P0 con reproducciones. | U01–U03; inventario firmado; NO prometer comportamiento no probado. |
| C1 Arranque | Install/doctor/core CLI y MCP funcionar en HOME limpio sin red/OTLP; stderr separado, JSON-RPC puro en stdout; errores de root/flags explicados; shutdown limpio. | U04–U07; clientes externos y salida real. |
| C2 Correctitud | Corpus anidado, mtime preservado, lectura/cobertura parcial, lenguajes, identidad ambigua; resultados semánticos reproducibles; ninguna vía de análisis anuncia Completo falso. | U06,U09,U11–U16; goldens antes/después, cero false-clean. |
| C3 Casos compartidos | Una operación end-to-end real del mismo use case para CLI/MCP; contrato versionado y paridad semántica con clientes anteriores; límites del dominio/aplicación preservados. | U08,U10,U11; diagrama de dependencias y pruebas contractuales. |
| C4 Estado | Identidad workspace+config+revisión, sin mezcla entre procesos; recuperación, migración, corrupción controlada, upgrade/downgrade y datos de usuario intactos. | U17,U18,U21,U22. |
| C5 Seguridad y extensibilidad | Permisos por capability, ruta canónica, ausencia de fuga, cancelación/timeout/budget, observabilidad opt-in, al menos una extensión read-only sin romper cliente anterior. | U19,U25,U26 + amenazas revisadas. |
| C6 CI/distribución | Gate bloqueante e independiente sobre el SHA candidato; tests feature matrix, auditoría dependencias, benchmark comparativo; tarballs + manifiesto/sha/attestation comprobados en runners nativos prometidos. | U01,U20,U23,U24,U27; no `|| true` ni `continue-on-error` en pasos obligatorios. |
| C7 Production ready | C0–C6 PASS sobre artefactos del **mismo commit**; todos UAT obligatorios por plataforma/lenguaje/capability anunciados PASS; historial de repetición sin regresión; docs y soporte/rollback aprobados por maintainer. | Certificado final firmado, release candidate, soporte Tier-1 x86_64/aarch64 validado por separado. |

## Reglas de calidad bloqueantes

- **P0/P1:** ninguna vulnerabilidad de seguridad explotable conocida sin mitigación probada; cero fallos core sin clasificar; ningún resultado false-complete del corpus.
- **Cobertura funcional:** 100% de capabilities **anunciadas estables** con positivo, negativo, error, permiso y compatibilidad donde corresponda. `SKIP` solo para capacidades no anunciadas/plataformas fuera del contrato, decidido ANTES del corte.
- **Cobertura de código:** medir líneas, ramas y riesgo tras F0; umbral por módulo fijado ANTES de F6, sin dar por acreditado un porcentaje sin ejecutarlo.
- **Rendimiento:** límites por capacidad y corpus establecidos con p50/p95, RSS pico, tamaño salida, dos condiciones frío/caliente y >=3 repeticiones; comparar misma máquina/corpus/config SHA. Una regresión >10% sobre baseline de la vertical migrada requiere justificación y aceptación **previa al gate**, nunca silencio.
- **CI:** el repo actual tiene una decisión local-first; F6 debe decidir cómo demostrar gate independiente, inalterable y obligatorio por SHA. Propuesta: PR gate remoto mínimo + suites largas local/nightly con recibos firmados. Si se preserva local-only, se exige protección equivalente documentada; no proclamar automatización remota inexistente.
- **Release:** artefacto publicado, checksum, procedencia y prueba de usuario final en HOME/XDG limpios. Lista de fallos conocidos no equivale a PASS; fallos P0 core jamás se justifican con baseline.
- **Trazabilidad:** requirement ID → implementación → contrato → UAT → certificado → release. Todo `N/D`, `NOT_RUN`, `BLOCKED`, `FAIL` o excepción caducada en gate obligatorio ⇒ **NO-GO**.

Los umbrales de latencia y memoria no fijados numéricamente antes de ejecutar F0 quedan `UNDEFINED`, no arbitrariamente `PASS`; para C7 deben estar fijados y medidos con un presupuesto de producto revisado.
