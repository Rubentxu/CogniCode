# PRF-CLI-02 — stdout solo datos estructurados; stderr logs (captura completa)

Fecha: 2026-09-22 · binario real `cognicode` release reconstruido · corpus `/tmp/cli02-corpus`

## Requisito (SPEC-CLI)

> PRF-CLI-02 MUST: en operaciones con salida estructurada, stdout contiene solo datos
> con esquema/versión declarados; stderr contiene logs/avisos. No cambiar una salida
> histórica sin migración y test de cliente anterior.

## Hallazgo inicial (auditoría honesta)

La primera pasada de UAT clasificó `analyze`/`doctor` (modo texto) como violación.
Corrección del análisis: esos comandos publican salida humana histórica, no "salida
estructurada" según el contrato; cambiarla exigiría migración + test de cliente
anterior (prohibido hacerlo sin contrato). La ÚNICA operación estructurada declarada
era `doctor --format json` (cumple). `graph full` no tenía modo máquina.

## Cambio

`graph full --format json` (nuevo, additive, sin tocar la salida texto histórica):

- stdout: documento JSON único con `schema_version: "cognicode.graph.full/v1"`,
  path, elapsed_ms, symbols, dependencies.
- stderr: logs y texto de progreso (incl. "Building full project graph at:").
- Modo texto default sin cambios (contrato de cliente anterior preservado).

## UAT (captura completa, binario real)

```
doctor-json:    rc=1 stdout_json=yes stderr_leak=no   (rc=1 honesto: core/lsp missing)
graph-json:     rc=0 stdout_json=yes stderr_leak=no
verbose-json:   stdout pure JSON + logs on stderr (241B) con RUST_LOG=debug
graph-text:     historical text output preserved
schema_version: cognicode.graph.full/v1
RESULT: PASS (5/5)
```

## Verificación adicional

- `cognicode-core` lib: 2145 passed / 0 failed / 27 ignored.
- `cargo clippy -p cognicode-core --all-targets`: 0 errores. `cargo fmt` aplicado.

## Estado

PASS para el criterio CLI-02 sobre las operaciones estructuradas existentes/declaradas.
Deuda declarada (no bloqueante): el resto de subcomandos de `graph` (hot-paths,
trace-path, etc.) sigue en modo texto humano; añadir `--format json` a cada uno es
trabajo incremental sujeto al mismo patrón.
