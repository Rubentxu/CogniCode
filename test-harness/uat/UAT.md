# UAT — User Acceptance Tests del harness incremental

**Estado:** plan, **no ejecutado**. Ejecutar contra el checkout y binarios actuales; registrar SHA, toolchains, env allowlist, comando, log digest, exit code, mediciones y la razón de cada reutilización. Para cada test citar recibo real en informe de hito.

| ID | Dado | Cuando | Entonces |
|---|---|---|---|
| UAT-01 | Worktree sin cambios y base Git válida | `plan --lane change` | Plan reproducible con obligaciones reales o noop expresamente fundamentado |
| UAT-02 | Rama diverge de base, checkout limpio | plan | Incluye cambios de commits de rama, no cae en `HEAD~1` |
| UAT-03 | Código core usado por MCP cambia | plan | Selecciona dependientes y explica clausura |
| UAT-04 | Existe test sin `git add` | plan | Lo descubre e incluye; no falsa cobertura |
| UAT-05 | Un test es eliminado | plan | Invalida inventario anterior y informa ausencia |
| UAT-06 | Suite A pasa, suite B falla, C pendiente | resume tras fix local de B | A se reutiliza sólo si huella sigue igual; B repite; C continúa |
| UAT-07 | Falló B y el arreglo cambia dependencia de A | resume | A queda invalidada y repite; explicación explícita |
| UAT-08 | Recibo PASS previo existe con mismo nombre pero feature distinto | resume | No se reutiliza; se ejecuta perfil requerido |
| UAT-09 | Cargo.lock/toolchain cambia | plan/resume | Rechaza reuse de suites afectadas y justifica |
| UAT-10 | Cargo target externo en HOME/config | ejecutar CLI test | Resuelve binarios con autoridad Cargo; no depende de `target/` supuesto |
| UAT-11 | Baseline `known_failures` coincide con salida observada | run | Compara sin segunda ejecución redundante |
| UAT-12 | El baseline cambia o uno de sus tests pasa inesperadamente | run | Gate detecta drift, exige reconciliar baseline |
| UAT-13 | Suite `--features evidence-kernel` | run | Se ejecutan casos gated esperados; 0 no es verde |
| UAT-14 | `cargo test --workspace --all-targets` | run | Doctests se cubren por obligación separada si definidos |
| UAT-15 | CLI test env global | run parallel | Se serializa o aísla procesos, sin flaky nuevos |
| UAT-16 | Nextest no instalado/recording incompatible | run | Fallback Cargo, o INFRA_ERROR explícito sin reducción de cobertura |
| UAT-17 | Nextest run recording habilitada | rerun un test failed | Identidad de grabación ligada a suite; no `-R latest` ajeno; invalidación externa manda |
| UAT-18 | Sandbox 4 escenarios, tercero falla | campaña | Exit != 0; resultado `COMPLETED_WITH_FAILURES`; demás resultados reportados |
| UAT-19 | Repetición de campaña con worker fallido | repeat | Estado no verde y fallos sumados; ningún falso terminal `PASS` |
| UAT-20 | Sandbox scenario recibe fixture y container | resume | Solo se reutiliza si inputs verificados o policy AlwaysFresh obliga reejecución |
| UAT-21 | Dos agents usan mismo worktree | run con locks | No hay corrupción, resultado consistente y resource locks efectivas |
| UAT-22 | SIGKILL durante test | resume | PASS previos durables y test running repetido; gate no engañado |
| UAT-23 | `known_failures` y contratos Python | comparar legado vs nuevo | Igual conjunto de casos, fallos y rc; glob vacío falla |
| UAT-24 | Worktree de harness evoluciona en paralelo | ejecutar main gate | `main` no se modifica, compat `scripts` permanece |
| UAT-25 | Lane merge vieja y adaptada | shadow run | Paridad de targets, flags, rc y test inventory; diff documentado |
| UAT-26 | Integration matrix | run | Default/no-default/all-features de crates y doc/coverage conservados |
| UAT-27 | Candidato release real | certify + release | Install smoke y `cognicode-release verify` contra bytes finales, no recibo de build previo |
| UAT-28 | Repetir certificación idéntica | plan | No reejecuta lo cacheable permitido; tareas AlwaysFresh siguen exigidas |
| UAT-29 | Artefacto faltante o SHA alterado en candidate | certify | Gate rojo, no se publica |
| UAT-30 | Estado persistente en `$XDG_STATE_HOME` | plan/resume | No aparecen artefactos de test en checkout ni se modifica el control de Cargo |

## Cierre

Para cada UAT registrar `PASS/FAIL/BLOCKED/NOT_RUN`, número de casos descubiertos, referencia a requisito TH-S*, log sanitizado, recibo verificado y tiempos; no escribir `PASS` por evidencia ajena o por exclusión.

**UAT transversal: exposición pública rota**. Reproducir el fallo de la release `v0.101.16` por ausencia de bundle manifest; garantizar que un harness reanudable jamás marque la instalación real como PASS por haber pasado tests de Rust. El caso requiere entorno controlado y artefacto de prueba, sin alterar release pública.
