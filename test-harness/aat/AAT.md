# AAT — Adversarial Acceptance Tests / pruebas de mutación

**Estado:** ejecutado en lo que el harness puede matar hoy. El criterio del AAT no es «el mutante compila», sino que el validador lo **MATA** con señal concreta. Conservar recibo por mutante y seed. No ejecutar mutaciones sobre main ni releases públicos.

La tabla «Ejecución» de abajo es la que manda: cada fila con estado `KILLED`
nombra el test que la mata, y el contrato del gate comprueba que ese test existe y
que el estado es uno de los tres admitidos. Las filas `NOT_RUN` llevan su motivo,
no un silencio: sandbox de campañas, adaptador nextest y artefactos de release
quedan fuera del alcance del harness en esta unidad.

| ID | Mutación inyectada | Debe producir |
|---|---|---|
| AAT-01 | Worker sandbox falla pero script ejecuta `exit 0` | TEST FAIL por rc y escenario fallido |
| AAT-02 | Worker rc=0 pero falta `result.json` de escenario requerido | `MissingEvidence`; gate rojo |
| AAT-03 | Quitar `--features evidence-kernel` | Inventario exige casos gated, rechaza `0 passed` |
| AAT-04 | Eliminar doctests de selector `--all-targets` | Gate distingue doctest obligation y rechaza reducción |
| AAT-05 | Deshabilitar serialización CLI con env compartido | Auditoría/locks detectan riesgo, no flake oculto |
| AAT-06 | Borrar `cargo clippy` de merge gate | Parity check rechaza ausencia de obligación |
| AAT-07 | Hacer que desconocido resulte `noop` | Selector fail-closed mata mutante |
| AAT-08 | Modificar código upstream de suite PASS | Fingerprint cambia e invalida |
| AAT-09 | Modificar sólo doc irrelevante con cobertura probada | No invalida suite ajena; justificación |
| AAT-10 | Alterar Cargo.lock, build.rs, test source o toolchain | Invalida clausuras relevantes |
| AAT-11 | Añadir test sin add Git | Discover incluye y requiere ejecutar |
| AAT-12 | Quitar test de binario | El inventario denuncia desaparición |
| AAT-13 | Corromper recibo o inventar campo status PASS | Digest/schema/ledger rechazan |
| AAT-14 | Cortar proceso después de `Running` sin terminal | Resume no convierte a PASS |
| AAT-15 | Ejecutar dos resume simultáneos | Lease protege, no duplicidad engañosa |
| AAT-16 | Recibo PASS de otro clone mismo nombre | Identidad+inputs no coinciden salvo equivalencia explícita |
| AAT-17 | Recibo Nextest anterior pero código modificado | Fingerprint externo invalida aunque -R omita test |
| AAT-18 | Grabar `-R latest` de otro agente/suite | Origen de grabación incompatible rechazado |
| AAT-19 | Señalar binario `target/release` obsoleto | Hash de binario/proveniencia rechaza |
| AAT-20 | Ruta con symlink que escapa del state root | Rechazo de escritura/lectura insegura |
| AAT-21 | Sustituir candidato tarball por otro tras tests | Reverify fail; no publish |
| AAT-22 | Publicar sólo tarballs sin bundle manifest o SHA256SUMS | UAT install consumer rechaza; no certificado |
| AAT-23 | Cambiar `known_failures.yaml` tras receipt | Baseline policy fingerprint invalida |
| AAT-24 | Incluir palabra PASS en logs de error | Resultado se basa en evento/rc/cases, no grep |
| AAT-25 | Cambiar `scripts/ci` mientras contrato runner no se reejecuta | Política e input fingerprint lo invalidan |
| AAT-26 | Etapa advisory falla | Reporta ADVISORY_FAIL, no PASS; conserva política actual de bloqueo |
| AAT-27 | Un test no determinista PASS una vez, falla otra | Política AlwaysFresh y stability classification evitan certificar como estable |
| AAT-28 | Interrumpir escritura del ledger | Recuperación rechaza entry parcial, conserva previos válidos |
| AAT-29 | Un puerto/PG de otro agent contamina resultado | ResourceLock + namespace/isolation prueba independencia |
| AAT-30 | Una suite sin test inventory retorna rc=0 y 0 tests | FAIL por vacuidad salvo waiver explícito aprobado |

## Protocolo de ejecución

1. Probar mutante RED contra implementación vieja (si es seguro), confirmar que detecta el defecto declarado.
2. Probar contra implementación nueva y certificar que se rechaza exactamente por el invariante correspondiente.
3. Restaurar mutante y ejecutar tests quirúrgicos, sin hacer commits de mutaciones al repo principal.
4. Documentar `MUTANT_KILLED / SURVIVED / NOT_RUN`, outputs/recibos y run SHA. Toda supervivencia de mutante de seguridad/reuse es BLOCKER.
5. En gate final, agregar todos los AAT críticos, no confundir con número de tests positivos.

## Ejecución (2026-10-15)

Estados: `KILLED` = hay un test que inyecta la mutación y comprueba el rechazo
exacto; `PARTIAL` = el mecanismo existe y está probado, pero la fila pide además
algo que hoy no se puede ejercer (sandbox, servicio externo); `NOT_RUN` = fuera del
alcance actual, con motivo.

| ID | Estado | Test que lo mata | Motivo si no está KILLED |
|---|---|---|---|
| AAT-01 | NOT_RUN | — | Requiere campaña real de sandbox; fuera del alcance del harness |
| AAT-02 | KILLED | test_aat_02_a_required_suite_without_receipt_turns_the_gate_red | |
| AAT-03 | KILLED | test_aat_03_removing_features_changes_the_fingerprint_and_the_reuse | |
| AAT-04 | KILLED | test_the_critical_obligation_floor_applies_through_the_cli | (También la mata `test_aat_04_a_missing_critical_obligation_leaves_the_plan_indeterminate`.) Hasta 2026-10-16 la evidencia llamaba a `build_plan` **con el suelo cargado a mano**: probaba el mecanismo, no el cableado, y el cableado estaba roto (la CLI cargaba el suelo y no lo pasaba al `Context`, así que ningún plan real podía quedar indeterminado por perder una obligación). El segundo test ejerce la CLI sobre un repo cuyo suelo promete algo que sus pipelines no ejecutan |
| AAT-05 | PARTIAL | test_a_serial_suite_never_overlaps_even_with_workers_available | El mecanismo de exclusión está probado; la declaración de qué suite del CLI es serial es un dato de `config/suite-metadata.json` y su auditoría es manual |
| AAT-06 | KILLED | test_aat_06_removing_clippy_from_the_gate_is_detected_as_indeterminate | |
| AAT-07 | KILLED | test_unknown_path_forces_everything | |
| AAT-08 | KILLED | test_touching_a_suite_source_invalidates_only_its_fingerprint | |
| AAT-09 | KILLED | test_docs_change_alone_selects_nothing | |
| AAT-10 | KILLED | test_fingerprint_is_stable_and_input_sensitive | |
| AAT-11 | KILLED | test_working_tree_and_untracked_files_with_spaces | |
| AAT-12 | KILLED | test_the_pinned_inventory_minimum_applies_through_the_cli | (También la mata `test_inventory_pin_flags_a_suite_that_lost_cases`.) Ídem: esa evidencia era unitaria (`check_receipt_against_inventory`). El pin no llegaba al plan por el mismo defecto de cableado, así que un PASS con menos casos que el pinado se aceptaba. El segundo test pincha un mínimo imposible y comprueba, por la CLI, que el run lo rechaza con un hueco que lo nombra |
| AAT-13 | KILLED | test_corrupt_receipt_is_not_evidence | |
| AAT-14 | KILLED | test_sigkill_leaves_no_pass_and_resume_reexecutes | |
| AAT-15 | KILLED | test_two_runs_with_the_same_id_do_not_both_write | |
| AAT-16 | KILLED | test_aat_16_a_receipt_from_another_repository_is_not_reused | |
| AAT-17 | NOT_RUN | — | No hay adaptador nextest; su semántica de `-R` no se usa |
| AAT-18 | NOT_RUN | — | Igual que AAT-17: sin grabaciones de nextest no hay origen que validar |
| AAT-19 | NOT_RUN | — | Requiere artefactos release; vive en la lane de candidato |
| AAT-20 | KILLED | test_aat_20_a_hostile_suite_id_cannot_escape_the_state_root | |
| AAT-21 | NOT_RUN | — | Sustitución de candidato tras los tests: lane de release |
| AAT-22 | NOT_RUN | — | Publicación de tarballs y bundle manifest: lane de release |
| AAT-23 | NOT_RUN | — | `known_failures.yaml` lo evalúa `scripts/test-fast.sh` con su propio gate, no el harness |
| AAT-24 | KILLED | test_aat_24_the_word_pass_in_a_failing_log_does_not_pass | |
| AAT-25 | KILLED | test_aat_25_a_ci_change_forces_everything | |
| AAT-26 | KILLED | test_advisory_obligations_do_not_block | |
| AAT-27 | KILLED | test_aat_27_a_suite_that_flipped_with_the_same_fingerprint_is_not_stable | |
| AAT-28 | KILLED | test_aat_28_a_partial_receipt_does_not_hide_the_valid_ones | |
| AAT-29 | PARTIAL | test_a_serial_suite_never_overlaps_even_with_workers_available | Los cerrojos por recurso están probados; la independencia frente a un PG o puerto ajeno exige levantar el servicio y eso es sandbox |
| AAT-30 | KILLED | test_aat_30_rc_zero_with_zero_tests_is_not_a_pass | |

Resumen: 21 `KILLED`, 2 `PARTIAL`, 7 `NOT_RUN`. Ninguna fila `NOT_RUN` se cuenta
como cubierta: el README y el recibo de la unidad repiten el reparto.

Las mutaciones con AAT-17 a AAT-23 dependen de piezas que no existen todavía
(nextest, fábrica de release, `known_failures` dentro del harness). Cuando existan,
la fila cambia de estado en esta misma tabla y el contrato exige el test.
