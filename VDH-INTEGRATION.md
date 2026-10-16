# Overlay VDH para Rubentxu/CogniCode

**Entrega:** roadmap complementario de 5 hitos B0..B4, 4 ADR propuestas, 5 specs OpenSpec Given/When/Then + proposal/design/tasks, aceptación efímera UAT/E2E/AAT y un inventario de 125 archivos para retiro por hito. Incluye herramientas Python read-only y ejemplo Rust funcional (no se compila como parte del producto).

**Base auditada:** `main`, `88f6d02afc9c90fe7ac3c5c4de975d9b9f8c8abb`, 2026-10-15. Cuando se integre, verificar `origin/main`: esta referencia no es una orden de checkout/revert.

## Integrar al checkout existente sin sobrescribir

```bash
cd /ruta/a/CogniCode
# Comprobar permisos y trabajo local antes de desempaquetar:
git status --short --branch
unzip -l /ruta/cognicode-vdh-2026-10.zip
unzip -n /ruta/cognicode-vdh-2026-10.zip -d .
python3 test-harness/tools/package_check.py --root .
python3 test-harness/tools/verify_overlay_manifest.py --root .
python3 test-harness/tools/ephemeral_guard.py --root . --phase merge
python3 -m unittest discover -s test-harness/tools -p 'test_*.py'
```

Después **añadir una sola fila de índice** en `docs/roadmap/ROADMAP.md` usando texto propuesto en `docs/roadmap/initiatives/VDH-2026-10/START-HERE.md`. Registro append-only al journal. Las ADR vienen marcadas `PROPOSED` y no quedan aprobadas por descomprimir el ZIP. Revisar `.gitignore` para ignorar localmente `/test-harness/.cycles/`, **sin sobrescribir** reglas actuales.

## Estructura

- `docs/roadmap/initiatives/VDH-2026-10/`: orden de hitos, criterios, 15 playbooks UAT/E2E/AAT, retirada y cierre de release.
- `docs/adr/ADR-VDH-001..004`: propuesta de ontología, FP, Explorer real y pruebas efímeras.
- `openspec/changes/2026-10-15-value-delivery-functional-core/`: proposal/design/tasks + specs formales.
- `test-harness/inventory/`: lista exacta de candidatas, snapshot auditoría, plantilla de receipt.
- `test-harness/tools/`: guard contra temporales, validadores de paquete y recibos, tests Python.
- `test-harness/examples/analysis_outcome.rs`: ejemplo Rust puro que necesita adaptación a los ADTs de CogniCode antes de producción.

## Política de aceptación y borrado

Se borran obligatoriamente, **al final de cada Bn y antes de su PR**, los tests UAT/E2E/AAT propios del ciclo, después de demostrar PASS y trasladar cualquier propiedad crítica a contrato permanente. Tests UAT/E2E que ya pertenecen a `main` no son efímeros por etiqueta y solo se eliminan tras reemplazo/obsolescencia probada (ver inventario de 125 y `LEGACY-BY-BLOCK.md`).

## Restricciones

- ZIP solo contiene archivos nuevos, no `Cargo.toml` ni `AGENTS.md` existentes, no cambia el repositorio remoto.
- El paquete no aporta resultados reales de pruebas o releases; `NOT_RUN` no es `PASS`.
- Requisitos actuales `AGENTS.md`, `docs/roadmap/ROADMAP.md`, `docs/prf/specs/*`, ADR aceptadas y branch protection prevalecen sobre propuestas.
- Release remota de cada bloque exige PR integrado, SemVer correcto, build y pruebas, artefactos completos, publicación y consumer proof.
