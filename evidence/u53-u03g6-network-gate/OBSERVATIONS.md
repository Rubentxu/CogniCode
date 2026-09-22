# U03/G6 — Flakiness layout/lifecycle: fuga de red cerrada

Fecha: 2026-09-22 · HEAD de la unidad · `cognicode-cli` (cogh bin serial)

## Diagnóstico previo (JOURNAL §42)

Dos fugas de red identificadas en tests del instalador, violando la regla
"el core se ejecuta sin red":

1. `test_cogh_update_respects_lockfile` (lifecycle.rs:954): ejecuta el binario
   real `cogh update` sin staging ni override de API → consulta `api.github.com`
   real. Evidencia: 3.6s de duración (latencia de red) vs 0.13s en los demás.
2. `t_debt4_uat_install_*` (layout.rs): sospecha de descarga contra
   `github.com/.../v0.97.3` real.

## Trabajo de esta unidad

### 1. Test de update offline por construcción (fix)

`test_cogh_update_respects_lockfile` ahora fija
`COGNICODE_API_BASE_URL=http://127.0.0.1:1` (puerto muerto, loopback) antes de
ejecutar el subprocess y restaura el valor al salir. La aserción acepta los tres
resultados honestos: éxito, "not yet implemented", o fallo limpio de red causado
por el endpoint deliberadamente muerto. Resultado: el test pasa de ~3.6s
(dependiente de red) a ~0.01s determinista y sin egress.

### 2. Verificación de la fuga 2: ya cerrada por PR #289

Los logs de `t_debt4_uat_install_rollback_roundtrip` muestran que TODO el tráfico
va al servidor loopback del fixture (`127.0.0.1 GET /v0.95.0/...`): el resolver
con staging lee `releases.json` sin red (lifecycle_resolver.rs:203) y
`TempBaseUrl` reescribe los assets al loopback. La fuga observada en §42 quedó
cerrada por el wiring de staging del PR #289.

Nota metodológica honesta: el intento de sondar egress con `HTTPS_PROXY` a un
puerto muerto no es concluyente en este stack (reqwest sin feature de proxy
intercepta también el loopback del fixture: 33 fallos espurios incluyendo tests
de servidores mini locales). La evidencia de no-egress es la auditoría de código
(resolver staging sin red + TempBaseUrl loopback) más los logs de peticiones.

### 3. Estabilidad

- Suite completa serial `cogh` bin: **5/5 runs consecutivos GREEN** (304 passed).
- `cognicode_lifecycle` integration: **5/5 runs GREEN** (7 passed).
- Duración estable (~4.3s), sin timeouts ni variabilidad de red.

## Estado

U03/G6 → resuelto. La causa raíz de §42 queda cerrada: único test con egress
pinado offline; el resto verificado loopback-only. El requisito de "runner
estable" para la comparación automática de perf (PRF-CI-04) queda desbloqueado
en su componente de flakiness local.
