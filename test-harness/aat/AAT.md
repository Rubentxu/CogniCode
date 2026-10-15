# AAT — Adversarial Acceptance Tests / pruebas de mutación

**Estado:** especificación. El criterio del AAT no es «el mutante compila», sino que el validador lo **MATA** con señal concreta. Conservar recibo por mutante y seed. No ejecutar mutaciones sobre main ni releases públicos.

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
