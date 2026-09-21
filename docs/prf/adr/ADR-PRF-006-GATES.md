# ADR-PRF-006 — Evidencia independiente para cada SHA y release
**Status:** PROPOSED · **Fecha:** 2026-09-21.

**Contexto.** `.github/workflows/ci.yml` es manual/local-first por decisión histórica; release.yml publica tarballs nativos Linux y con attestations. Algunas rutas antiguas de smoke son `|| true` o `continue-on-error`. No atribuir a Actions un gate de PR que no se ejecuta.

**Decisión propuesta.** Mantener validación local rica, pero exigir para cada SHA candidato una verificación independiente y bloqueante de core/compatibilidad, con recibos verificables. Preferencia: workflow remoto mínimo en PR + suites largas repetibles local/nightly; cualquier alternativa local-only debe demostrar imposibilidad de merge/release sin aprobación independiente del SHA. Cambiar política local-first explícitamente, no de forma implícita al crear este ADR.

**Validación:** U27 con test/manifest fallidos bloquea; release bundle realmente descargado y ejecutado en runners nativos de plataformas prometidas; signed hash/attestation. C6/C7. **Rollback:** no publicar un candidato sin recibo completo; conservar artefactos anteriores.
