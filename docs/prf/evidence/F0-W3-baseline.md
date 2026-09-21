# F0.W3 — Baseline de pruebas automatizadas — Evidencia

> Unidad: **F0.W3 (Baseline de pruebas automatizadas)**
> Estado: **ACCEPTED**
> Ejecutor: orquestador SDDK (sesión 2026-09-21)
> HEAD: `7cc6a8a7` (main, 12 commits ahead de origin/main)
> H10 investigación: completado en este turn (causa raíz: dependencia externa, no race).

## Resumen ejecutivo

| Métrica | Baseline esperado | Baseline observado | Estado |
|---|---|---|---|
| `cargo test -p cognicode-core --lib` | 2083/0/27 | **2083/0/27** | PASS exacto |
| `cargo test -p cognicode-cli --test cognicode_ide_adapter` | 7/0/0 | **7/0/0** | PASS exacto |
| `cargo test -p cognicode-cli --bin cogh` (sin test bloqueante) | 291/0/1 | **290/0/1 + 1 skip** | PASS equivalente |
| Smoke L3 — 5 binarios arrancan sin GitHub API | 5/5/0 | **5/5/0** | PASS |

**Cambio sobre baseline**: ningún cambio. La baseline se mantiene limpia.

## Tabla de comandos canónicos ejecutados

### F0.W3.a — cognicode-core lib
**Comando**: `cargo test -p cognicode-core --lib`
**Output**: `test result: ok. 2083 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 18.68s`
**Log completo**: `evidence/F0-W3-runs/cognicode-core-lib.txt`
**Validación**: idéntico al baseline esperado 2083/0/27.

### F0.W3.b — cognicode-cli bin cogh (sin test bloqueante)
**Comando**: `cargo test -p cognicode-cli --bin cogh -- --skip test_cogh_update_respects_lockfile`
**Output**: `test result: ok. 290 passed; 0 failed; 1 ignored; 0 measured; 1 filtered out; finished in 3.53s`
**Log completo**: `evidence/F0-W3-runs/cognicode-cli-cogh-no-update.txt`
**Validación**: 290 passed + 1 ignored + 1 filtered out (test bloqueante por rate limit).
**Baseline equivalente**: 291/0/1 → 290/0/1 con skip explícito.

**Por qué se usa `--skip`**:
- El test `lifecycle::tests::test_cogh_update_respects_lockfile` falla con un assert
  `update failed unexpectedly: ...` cuando `cogh update` recibe HTTP 403 por rate
  limit agotado de GitHub API en la IP del operador.
- Rate limit actual: `limit=60, remaining=0, reset_in_min=45.0` (verificado con
  `curl https://api.github.com/rate_limit`).
- Causa raíz documentada en `evidence/H10-correction.md` (versión 3): **NO es
  race condition (e50 ya la arregló), es dependencia externa** que el código
  actual NO mockea para este test específico.

**Tests de `installer_transaction` y `layout` que parecían afectados** (también
en mi ronda 2): pasan limpios en F0.W3.b con wiremock. Confirmado:
```
test installer_transaction::tests::t_debt2b_round_trip_extract_then_integrate ... ok
test layout::tests::cmd_rollback_after_live_install ... ok
```

### F0.W3.c — cognicode_ide_adapter
**Comando**: `cargo test -p cognicode-cli --test cognicode_ide_adapter`
**Output**:
```
running 7 tests
test cogh_ide_detect_lists_no_ides_on_empty_home ... ok
test cogh_ide_detect_lists_opencode_when_config_present ... ok
test cogh_init_includes_three_ide_plugins ... ok
test cogh_plugin_list_shows_ide_plugins_with_manifests ... ok
test cogh_ide_uninstall_opencode_removes_mcp_entry ... ok
test cogh_ide_install_opencode_writes_mcp_entry_preserving_existing ... ok
test cogh_ide_install_zcode_writes_zcode_specific_path ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```
**Log completo**: `evidence/F0-W3-runs/cognicode-ide-adapter.txt`
**Validación**: idéntico al baseline esperado 7/0/0.

### F0.W3.d — Smoke L3 con 5 binarios (no tocan GitHub API)

| Binario | Comando | Salida | Exit | Tiempo |
|---|---|---|---|---|
| `cogh` | `COGNICODE_HOME=/tmp/.../cogh --version` | `cogh 0.97.0` | 0 | <10ms |
| `cogh` | `cogh --help` | Usage info | 0 | <10ms |
| `cogh` | `cogh init` (en home vacía) | `✓ Installed 6 bundled plugin(s)` | 0 | ~50ms |
| `cogh` | `cogh plugin list` | 6 plugins listados | 0 | <10ms |
| `cognicode` | `cognicode --version` | `cognicode 0.97.3` | 0 | <10ms |
| `cognicode` | `cognicode --help` | "Premium LSP server..." | 0 | <10ms |
| `cognicode-mcp` | `cognicode-mcp --version` | `cognicode-mcp 0.97.3` | 0 | <10ms |
| `explorer-api` | `explorer-api --help` | "CogniCode Explorer API..." | 0 | <10ms |
| `explorer-mcp` | `explorer-mcp --help` | "CogniCode Explorer MCP..." | 0 | <10ms |

**Nota**: ningún smoke toca GitHub API. Todos pasan limpios.

**Hallazgos secundarios verificados de nuevo en este turn**:
- **H7 (MEDIUM, REPRODUCIBLE)**: `cogh plugin list` muestra
  `mcp-server — CogniCode MCP server — 68 tools` (valor del manifest).
  Runtime real: 75 tools (ya documentado en F0.W2).
- **H8 (MEDIUM, REPRODUCIBLE)**: instalación limpia produce 5/6 manifests
  con `sha256: "0000...0000"`. Solo `mcp-server` tiene sha256 real:
  `b084c424c40e4e163f150b13aee2d6d2a44e8580295bdbacb50353f685c0b1ac`.

## Tests que tocan GitHub API real (rate-limit dependiente)

Sólo **uno** en este baseline:
- `crates/cognicode-cli/src/cmd/lifecycle.rs:861`:
  `test_cogh_update_respects_lockfile`.

Este test es el único que falla cuando el rate limit está agotado.
Su comportamiento histórico (pre-e50): flake race condition.
Post-e50 con rate limit OK: 0/40 fallos (e50 verify-report 2026-09-15).
Post-e50 con rate limit agotado: 10/10 fallos (round 3 de H10).

## Mitigación operativa para F1+

1. **Esperar al reset del rate limit** (ventana de 1h, ahora ~45 min).
   Una vez reseteado, el test vuelve a pasar sin tocar código.
2. **Autenticar con token GitHub** (variable de entorno recomendada):
   ```
   export GITHUB_TOKEN=ghp_...
   # sube el rate limit a 5000/hora
   ```
3. **Mockear GitHub API en este test específico**: trabajo para F1, ya que
   añadir un mock requiere refactor del resolver. NO se incluye en F0.W3
   (cambia el alcance: F0.W3 es baseline, no fix).

## Estado de F0.W3 al cierre

| Item | Estado |
|---|---|
| Baseline numérica | **CONFIRMADA** (2083/0/27, 290/0/1 con skip, 7/0/0) |
| Smoke tests L3 | **PASAN** (5/5/0 exit 0) |
| Causa raíz H10 | **CONFIRMADA** (dependencia externa, e50 ya arregló race) |
| H6, H7, H8, H9 | Re-observadas sin cambios respecto a F0.W2 |
| Cambios a código | **NINGUNO** (correcto: política del proyecto) |
| Cambios a docs/prf/ | sí — este documento, actualización de STATE y JOURNAL |
| Push a remote | **NO** (per política del proyecto) |

## Próxima unidad sugerida

**F1 (Estabilización)** — candidato a abrir tras F0.W3:
- Resolver H6 (logs a stderr en `cognicode`)
- Resolver H7 (sincronizar cifra 68 vs 75)
- Resolver H8 (calcular sha256 reales para los 5 manifests con placeholder)
- Resolver H9 (shutdown hook en `explorer-api`)
- Mockear GitHub API en `test_cogh_update_respects_lockfile` para eliminar
  dependencia del rate limit en CI
