# Test harness VDH: control central, sin desplazar Cargo/Playwright

- `inventory/test_inventory.csv`: 106 candidatas UAT/E2E y 19 adicionales. `REVIEW` hasta evidencia real. No confundir fichero con caso de prueba.
- `tools/ephemeral_guard.py`: comprueba ausencia de pruebas de ciclo `__vdh_cycle_*` y de cualquier contenido bajo `test-harness/.cycles`. **No borra nada**. Al cerrar cada Bn, --phase merge debe devolver 0.
- `tools/package_check.py`: comprueba rutas y recuento, en checkout real o `--overlay-only` para validar el ZIP.
- `tools/acceptance_receipt.py`: valida recibos de UAT/E2E/AAT completos y comprueba que rutas efímeras ya no existen; **solo chequeo estructural, no prueba que los comandos se ejecutaron**.
- `inventory/ACCEPTANCE-RECEIPT-TEMPLATE.json`: plantilla NOT_RUN para rellenar con SHA/oráculos reales; por diseño no puede dar PASS.
- `tools/test_*.py`: ocho tests de los bordes del guard y de los recibos sin conectividad externa.
- `examples/analysis_outcome.rs`: ejemplo idiomático de ADTs, pure map y leyes; **no es código productivo ni compatible automáticamente con los DTO existentes**.
- `.cycles/` = directorio efímero privado por sesión, a ignorar en `.gitignore` **después de revisar el patrón existente**:

```gitignore
/test-harness/.cycles/
```

Rust UAT temporal puede requerir `crates/<crate>/tests/__vdh_cycle_<id>_<name>.rs`, y UI temporal `apps/explorer-ui/e2e/__vdh_cycle_<id>_<name>.spec.ts`. **No commitear** esos tests; borrarlos tras capturar recibo, antes de merge. El guard los detecta estén trackeados o no. No guardar tokens/secretos, snapshots enormes o DB de usuario en receipts.

**Centralización incremental**: mantener tests permanentes bajo rutas estándar de compiladores/runners; catalogar contratos/capacidades y ejecutores aquí para no crear dos orquestadores. Cualquier `--full` sigue las normas de `AGENTS.md`. Si esta herramienta no se integra en PipelineK real, no protege un merge: incorporar invocación en el gate y probar negativo.
