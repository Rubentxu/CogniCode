# ADR-TH-005 — Una suite, un dueño: las construcciones release salen del gate de merge

- **Estado:** ACCEPTED (2026-10-15). Decisión de B1 de la iniciativa de evolución
  del test harness; autorizada por el mandato del operador del 2026-10-15.
- **Contexto:** `merge-gate.pipeline.kts` tenía 73 declaraciones de stage y
  cuatro construcciones en perfil release (`cognicode-mcp`,
  `cognicode-control-plane`, `cognicode-cli` y el bin `cognicode`) más su
  verificación. El operador midió aproximadamente 676 s en esas cuatro
  construcciones, y son el mayor coste de una iteración de merge. El único
  consumidor verificable de esos artefactos en esa lane era un conjunto de
  suites que taran, generan y verifican **bytes de perfil release**:
  `prf_dist_01_06_release_candidate_uat`, `prf_dist_workflow_flatten_uat`,
  `prf_f6_w1_release_coherence`, `prf_f6_w2_staging_contract`,
  `published_skill_bundles_are_required` y `prf_cli_04_two_process_uat`.
  El resto de la lane ejecuta binarios que `cargo test` resuelve por
  `CARGO_BIN_EXE_*` en perfil debug.
- **Decisión:** una suite pertenece a la lane que puede producir sus
  precondiciones.
  1. `merge-gate.pipeline.kts` construye los binarios de perfil **debug** que
     sus propias suites ejecutan (`cognicode-mcp`, y `--bins` de
     `cognicode-cli`), y verifica que existen y arrancan. No construye nada en
     release.
  2. `release-candidate.pipeline.kts` ejecuta las seis suites de perfil release
     después de construir el candidato, y las apunta explícitamente a los
     artefactos de esa lane con `COGNICODE_RELEASE_BIN` (la herramienta
     `cognicode-release`) y `COGNICODE_RELEASE_BIN_DIR` (el directorio con
     `cogh`, `cognicode` y `cognicode-mcp` del target anfitrión).
  3. Las suites de perfil release fallan en voz alta cuando el artefacto que
     se les señala no existe o no es de perfil release. No hay modo de
     reutilizar un binario debug "que resultó estar ahí".
- **Invariantes que NO cambian:**
  - PipelineK sigue siendo la única autoridad de CI; no se introducen GitHub
    Actions, ni un segundo orquestador, ni una capa de caché o reanudación.
  - Ninguna suite se elimina y ninguna aserción se sustituye por un skip. El
    conjunto de suites de `crates/cognicode-cli/tests/*.rs` y
    `crates/cognicode-mcp/tests/*.rs` se compara contra el sistema de ficheros:
    exhaustivo entre las dos lanes y sin que la lane de merge nombre una suite
    que no puede ejecutar.
  - Las suites que ejecutan binarios por ruta fallan en voz alta si el binario
    falta, y la lane de merge les entrega el binario debug que ella misma
    construyó, no el que aparezca en el directorio compartido de artefactos.
  - La garantía de `verify-release-binaries` ("el artefacto existe, es
    ejecutable y arranca") se conserva: en la lane de candidato ya existía como
    `binary-smoke-$target`, que además sabe ejecutar un target extranjero con
    su emulador y su sysroot.
- **Fundamento:** `ADR-TH-003` (PipelineK como autoridad) y
  `docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md` (lanes con responsabilidades
  distinguibles). La lane de merge es "lo que un cambio debe pasar para
  fusionarse"; la lane de candidato es "lo que un artefacto debe probar antes
  de publicarse". Una suite que necesita el candidato pertenece a la segunda.
- **Consecuencias:**
  - El coste por iteración de merge baja: se dejan de construir cuatro
    artefactos release y se construyen dos binarios debug que el propio gate ya
    necesitaba compilar para sus suites.
  - El candidato **no** paga una construcción extra: reutiliza los binarios que
    ya produce para empaquetar. Las suites pasan a ejecutarse contra los bytes
    del candidato, que es más fiel que un artefacto construido en otra lane.
  - Un fallo de esas seis suites deja de bloquear el merge y pasa a bloquear la
    lane de candidato, que es obligatoria antes de publicar:
    `release.pipeline.kts` no reconstruye, publica lo que el candidato verificó.
  - El riesgo aceptado es explícito: entre el merge y la publicación, un cambio
    que rompa el perfil release se detecta en la lane de candidato y no antes.
    Eso ya era así para todo lo que la lane de candidato construye y verifica.
- **Prueba que la sostiene:** los contratos de cobertura de CLI y core
  (propiedad exhaustiva por lane, detección medida desde el código de las
  suites que resuelven artefactos release) y las mutaciones adversariales
  registradas en el recibo de B1: quitar una suite obligatoria, quitar la
  construcción debug o cambiar el dueño de una suite debe poner el gate rojo.
- **Límites:** este ADR no altera la política de seguridad ni de certificación,
  no cambia el significado de `full` en `AGENTS.md`, y no autoriza solape,
  reutilización de resultados ni certificación de artefactos sin verificar.
