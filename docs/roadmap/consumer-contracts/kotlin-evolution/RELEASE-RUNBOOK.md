# Release real por cada K0–K4 — sin inventar gates

## No asumir comandos: descubrir runner existente

El root contiene `release-candidate.pipeline.kts`, `release.pipeline.kts`, `merge-gate.pipeline.kts`, `product-fast.pipeline.kts`, `scripts/test-fast.sh`, `scripts/test-full.sh`, y `AGENTS.md` obliga full antes de PR/tag. **Usar la versión instalada y la configuración real de PipelineK**; no inventar `pipelinek release`, subcomandos de Cargo, flags de CogniCode o un GitHub Actions nuevo. Ejecutar tareas de release a través del orquestador *existente* solo tras inspeccionar su contract y obtener autorización.

## Secuencia rígida de admisión por hito

1. **BASELINE**: `git status --short --branch`, `git rev-parse HEAD`, `git log -5 --oneline`; comprobar `origin/main`, la mayor release remota, `Cargo.toml`, el manifiesto binario y el roadmap. No reutilizar snapshot `CURRENT.md` obsoleto. Registrar el objetivo de cliente en el work item Kx.
2. **BUILD**: código funcional integrado en rama; tests quirúrgicos `bash scripts/test-fast.sh`, UAT del consumidor; revisión de arquitectura y compatibilidad pública.
3. **PR**: `bash scripts/test-full.sh` con resultados reales y número no nulo de pruebas, `merge-gate` sobre HEAD real, sin `|| true`, without cache masquerade; obtener review y **merge** bajo la política de repo.
4. **RC**: generar binarios exactos del commit integrado; comprobar identidad product version/build commit/digest e inventario esperado. Correr `release-assurance` sobre esos binarios, no sobre `cargo run` de source. Verificar comportamiento del hito por CLI/MCP real, incluyendo fallo negativo.
5. **VERSION**: determinar SemVer desde tag remota mayor **en ese momento**; K0 patch si compatible, K1/K2/K3/K4 minor cuando exponen feature pública. **No reservar 0.101.18/0.102.0/... si otro equipo publicó antes.** Bump, changelog y tag anotado según flujo del repo.
6. **PUBLISH**: release remota, assets redistribuibles y checksums. En baseline v0.101.17 hubo 8 assets; el conjunto exacto de la versión Kx se deriva de `release-inventory-<version>.json`, matriz de distribución y scripts de publicación actuales. **Completitud ≠ integridad**: descargar y listar assets publicados, contrastar número/nombres/tamaños/digests y SHA256SUMS; no asumir que seguirán siendo 8 al añadir plataformas.
7. **CONSUMER**: instalar `cognicode`, `cognicode-mcp`, `cogh` si aplica desde la release real en HOME efímero, invocar capacidad Kx, comprobar versión y ref/caché. Repetir con protocolo MCP real; no certificar únicamente test del crate.
8. **CLOSE**: verificar tag remoto resuelve SHA integrado, `origin/main` contiene trabajo, release realmente visible y reinstalable, journal append-only actualizado, `CURRENT` regenerado sin drift, decisión `PROMOTE|REPLACE|RETIRE` y **borrado de UAT/E2E/AAT temporales** de la sesión; siguiente Kx habilitado.

## Bifurcaciones honestas

- Falla `merge-gate` o `full`: **NO** tag; triaje, corregir, nuevo candidato. El resultado anterior queda FAIL.
- Falta binario o inventario: `BLOCKED`, no release vacía ni sources.zip como sustituto; mantener binario/JAR/bundle reales.
- `source.zip` generado automáticamente por GitHub no se considera producto, no justifica tag; el consumidor usa artefactos versionados.
- No es posible publicar por credenciales/firma humana: `PAUSED_OPERATOR` con PR, HEAD, evidencia y comandos; el hito **no está CLOSED**.
- Existen avisos de seguridad/permisos o distros no publicadas: declarar en limitaciones; no atribuirles cobertura.
- Un proyecto mixto no tiene LSP/Kotlin index: no declarar compatibilidad completa, servir operaciones S0 con precisión honesta.

**Reporte final breve:** `Kx | SHA main | tag | release URL | assets esperados/reales | digests | UAT CLI/MCP | test full | merge-gate | limpieza temp | gaps`.
