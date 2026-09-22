# PRF-CLI-01 — recorrido exhaustivo de comandos stable (binario real)

Fecha: 2026-09-22 · binario fresco

## Resultado: 7/7 PASS

- `--help` por subcomando (analyze/index/graph/navigate/doctor): exit 0
- `analyze` en dir válido: exit 0
- `index build` en dir válido: exit 0
- `graph full --path` y `graph on-demand --path` inexistentes: exit != 0
- `navigate` con símbolo inexistente: sin crash (sin señales)
- `doctor`: 0/1 honesto (1 = entorno sin componentes opcionales, nunca crash)
- comando desconocido: exit != 0

## Hallazgos menores (sin defecto de producto)

- `index` exige subcomando (clap) — contrato correcto.
- `graph` exige `--path` (el path posicional se ignora con exit 0 en
  modo help-less) — ya pineado con --path en prf_cli_01_uat.
- `doctor` exit 1 en entorno incompleto es un informe honesto.

## Disposición

CLI-01: escenario crítico (§38) + barrido exhaustivo (este) → PASS.
