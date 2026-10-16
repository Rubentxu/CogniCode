# Spec: release-consumer — publicar el conjunto que un usuario puede instalar

**Status:** PROPOSED. **Refs:** `docs/prf/specs/SPEC-DISTRIBUTION.md` PRF-DIST-01..07, `docs/prf/specs/SPEC-CI.md` PRF-CI-05/07, `docs/adr/ADR-034-cognicode-distribution-package.md`, `docs/adr/ADR-RELEASE-PROVENANCE-PIPELINEK.md`; `release-candidate.pipeline.kts`, `release.pipeline.kts`, `sandbox/scripts/test_release_asset_set.sh`.

### VDH-REL-01 — Completitud, integridad, provenance, alcanzabilidad

**MUST** comparar el conjunto esperado de assets (derivado de plan/manifiesto canónico y plataforma realmente certificada) con la release publicada. **MUST** verificar hashes y provenance; `sha256sum -c` por sí solo **no basta** para completitud. **MUST** verificar que `/releases/latest` resuelve el mismo tag instalable o declarar la versión como prerelease incompatible con `latest`. **MUST NOT** confiar en assets sólo locales ni ocultar errores de subida/limpieza. Para la distribución x86_64 del snapshot `v0.101.17`, la expectativa operacional observada es de ocho assets; obtener el conjunto exacto desde el contrato ejecutable vigente antes de publicar.

**Scenario A:** GIVEN release con sólo 3 assets y SHA válido de esos 3; WHEN se ejecuta publicación; THEN falla por faltantes, no PASS.

**Scenario B:** GIVEN release con 8 assets esperados, uno alterado; WHEN consumidor descarga y verifica; THEN falla por hash/provenance.

**Scenario C:** GIVEN draft o prerelease que no es visible por `/releases/latest`; WHEN instalador resuelve latest; THEN no se declara release estable utilizable.

### VDH-REL-02 — Mismo código, mismos bits

**MUST** tomar artefactos del candidato construido sobre SHA certificado, no recompilar silenciosamente para el verificador. **MUST** ejecutar instalación/doctor/CLI/MCP/update/rollback/uninstall desde paquetes publicados en HOME temporal, sin repositorio como dependencia.

**Scenario D:** GIVEN clone limpio sin `target/release` preexistente; WHEN se instala la release descargada; THEN completa el flujo y deja fuera del HOME temporal intacto.

### VDH-REL-03 — Cierre remoto por hito

**MUST** requerir PR aprobado, gate de merge verde, commit presente en `origin/main`, tag/version SemVer coherentes, release publicada verificada y recibo de ejecución por hito. **MUST** declarar `BLOCKED/NOT_RUN` cuando falta una evidencia; un commit local o PR abierto no equivale a release.

**Scenario E:** GIVEN gate verde y PR merged pero release no publicada; WHEN se evalúa cierre Bn; THEN estado `BLOCKED`, no DONE.
