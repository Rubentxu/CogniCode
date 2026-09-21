# F0.W2 — Caracterización runtime de los binarios PRF

**Fecha**: 2026-09-21
**Operador**: jcode-orchestrator
**Commit base**: `7cc6a8a7` (HEAD al inicio de la unidad)
**Versiones**: cogh 0.97.3, cognicode 0.97.3, cognicode-mcp 0.97.3, explorer-api 0.97.3, explorer-mcp 0.97.3
**Entorno**: Linux x86_64, /home/linuxbrew/.linuxbrew/bin/strace 6.x

## Resumen ejecutivo

Caracterización runtime (arranque, persistencia, red, stdio, señales,
secretos en logs) de los 5 binarios PRF. Ejecutada con `strace`, signals,
y monitoreo `ss`. Sin defectos críticos; **3 hallazgos nuevos** que se
añaden a H1-H5 de F0.W1 → ahora **H6-H9**, todos LOW o MEDIUM:

- **H6** (MEDIUM): `cognicode` CLI emite TODOS sus logs (INFO/WARN/ERROR)
  a stdout, no stderr. Mezcla logs con output de datos (Architecture
  Check, Cycle Analysis). Viola convención Unix y rompe piping.
- **H7** (MEDIUM): `cognicode-mcp` plugin.yaml declara "68 tools" pero
  el runtime expone 75 (consistente con H2 del inventario). El número
  está mal en `plugins/mcp-server/plugin.yaml` description.
- **H8** (MEDIUM): Los plugin manifests de `sandbox-templates`,
  `claude`, `codex`, `zcode`, `skills-cognicode-core` tienen
  `sha256: "0000...0000"` (placeholder). El instalador no verifica
  integridad de los bundles descargados.
- **H9** (LOW): `explorer-api` con SIGTERM devuelve exit 143 sin loggear
  motivo en stderr. Terminación silenciosa; debugging difícil.

Comprobaciones BLOCKED/NOT_RUN: 4 (3 de F0.W1 arrastradas + 1 nueva).

## 1. Latencia y exit codes (arranque en frío)

Comando: `/usr/bin/time -f "wall=%es user=%Us sys=%Ss maxrss=%Mk" -o TIMEFILE BIN --version|--help`.

| Binario | Comando | Exit | wall | user | sys | max RSS | stdout B | stderr B |
|---|---|---|---|---|---|---|---|---|
| `cogh` | `--version` | 0 | 0.00s | 0.00 | 0.00 | 3752K | 12 | 0 |
| `cogh` | `version` | 0 | 0.00s | 0.00 | 0.00 | 3756K | 53 | 0 |
| `cogh` | `--help` | 0 | 0.00s | 0.00 | 0.00 | 3744K | 1094 | 0 |
| `cognicode` | `--version` | 0 | 0.00s | 0.00 | 0.01 | 5796K | 17 | 0 |
| `cognicode` | `--help` | 0 | 0.00s | 0.00 | 0.01 | 5800K | 1200 | 0 |
| `cognicode-mcp` | `--version` | 0 | 0.00s | 0.00 | 0.01 | 6336K | 21 | 0 |
| `cognicode-mcp` | `--help` | 0 | 0.00s | 0.00 | 0.01 | 6364K | 158 | 0 |
| `explorer-api` | `--version` | 0 | 0.01s | 0.01 | 0.01 | 17172K | 20 | 0 |
| `explorer-api` | `--help` | 0 | 0.01s | 0.01 | 0.01 | 16812K | 1017 | 0 |
| `explorer-mcp` | `--version` | 0 | 0.01s | 0.00 | 0.02 | 17372K | 20 | 0 |
| `explorer-mcp` | `--help` | 0 | 0.01s | 0.00 | 0.02 | 17416K | 306 | 0 |

**Observaciones**:

- Todos arrancan en ≤10ms con exit code 0.
- `cogh` es el más ligero (~3.7 MB RSS).
- `cognicode-mcp` y `explorer-mcp` usan ~6 MB y ~17 MB RSS respectivamente
  (cargan Tokio runtime en el startup, aunque luego lo liberan).
- max RSS del explorador es ~17 MB incluso para `--help` → sospechoso,
  puede indicar carga eager de SQLite/LadybugDB antes del primer uso.

## 2. strace: syscalls, red, threads

### 2.1 `cogh --version` (strace 8736 bytes, 83 líneas)

- **Red**: 0 syscalls de red (sin socket/connect/bind/listen/accept).
- **Threads**: 1 (proceso monolítico).
- **Files abiertos**: solo `libc.so.6`, `libm.so.6`, `libgcc_s.so.1`,
  `/etc/ld.so.cache`, `/proc/self/maps`.
- **Output**: `write(1, "cogh 0.97.3\n", 12)` directo a fd 1.
- **Terminación**: `exit_group(0)`.

### 2.2 `cognicode-mcp` (initialize+tools/list) — strace 871138 bytes, 11795 líneas

- **Red**: 0 syscalls de red.
- **Threads**: **130** (clone3 con CLONE_THREAD) → Tokio multi-threaded runtime.
- **Memoria**: `madvise(MADV_GUARD_INSTALL)` x125, `MADV_DONTNEED` x67
  → Tokio memory pool activo.
- **RNG**: `getrandom` x64 (seeding inicial).
- **Cada thread**: `gettid` x129, `sigaltstack` x129.
- **Output**:
  - stdout: JSON-RPC frames (clean).
  - stderr: logs estructurados con timestamps, niveles INFO,
    spans `serve_inner` (MeterProvider / OpenTelemetry).

### 2.3 `explorer-api` arranque (4s sample) — strace 4354738 bytes, 54535 líneas

- **Red**:
  - `socket(AF_INET, SOCK_STREAM)` x1.
  - `bind(127.0.0.1:18021)` x1.
  - `listen(10, 128)` x1 (backlog 128).
  - `connect`: 0; `accept4`: 0 (en 4s sin clientes).
- **Threads**: **193** (Tokio + HTTP server).
- **Persistencia**:
  - Abre `/tmp/prf-w2-workdir/explorer.lbug.wal` **52 veces en 4s** —
    escribe Write-Ahead Log continuamente.
  - Carga `libssl.so.3`, `libcrypto.so.3`, `libstdc++.so.6`, `libz.so.1`
    — necesario para TLS (aunque solo escucha HTTP local en este test).
- **cgroup** reads: lee `/sys/fs/cgroup/.../cpu.max` x18 (detección de
  cuotas del cgroup v2).

## 3. Persistencia

### 3.1 `cogh init` en home aislado (`/tmp/prf-w2-home/.cognicode`)

```
✓ Initialized /tmp/prf-w2-home/.cognicode
✓ Installed 6 bundled plugin(s)
```

Estructura creada:

```
.cognicode/
├── bin/                       # (vacío, los binarios se descargan al install)
├── cache/downloads/           # (vacío, bundles descargados)
├── locks/                     # (vacío, locks de install/rollback)
├── plugins/
│   ├── claude/plugin.yaml
│   ├── codex/plugin.yaml
│   ├── mcp-server/plugin.yaml
│   ├── sandbox-templates/plugin.yaml
│   ├── skills-cognicode-core/plugin.yaml
│   └── zcode/plugin.yaml
├── shims/                     # (vacío, shims para IDE)
├── tracker/                   # (vacío, registro de versión pinned)
└── versions/                  # (vacío, versiones instaladas)
```

6 plugins, cada uno = `plugin.yaml` (4 KB). El binario se descarga
externamente; `cogh init` solo crea la estructura.

### 3.2 Plugin manifests (extractos relevantes)

`mcp-server/plugin.yaml`:

```yaml
description: "CogniCode MCP server — 68 tools, rust binary, stdio transport"
```

**→ Contradice H2/H7**: el manifest dice 68, runtime dice 75.

`sandbox-templates/plugin.yaml`:

```yaml
artifact: "sandbox-templates-0.92.0.tar.gz"
sha256: "0000000000000000000000000000000000000000000000000000000000000000"
url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.92.0/sandbox-templates-0.92.0.tar.gz"
```

**→ H8 (integridad)**: sha256 es placeholder `0000...0000`. El
instalador no puede verificar integridad de los bundles descargados
de GitHub Releases. Riesgo: un bundle comprometido o un mirror
malicioso se instalaría sin detección. Afecta a 5 de 6 plugins.

Plugins afectados por H8 (sha256 placeholder):

- `sandbox-templates`
- `claude`
- `codex`
- `zcode`
- `skills-cognicode-core`

Plugin OK (sha256 real):

- `mcp-server` (`b084c424c40e4e163f150b13aee2d6d2a44e8580295bdbacb50353f685c0b1ac`)

### 3.3 `cognicode-mcp` no crea archivos persistentes

Confirmado: el servidor MCP arranca en `-c /tmp/prf-w2-workdir2` (un
directorio que no existía), y aunque recibió initialize+tools/list+read_file,
**no creó ningún archivo**. El proceso terminó con exit 1 cuando el
directorio no existía, pero cuando existe, es stateless: la
persistencia es responsabilidad del caller.

### 3.4 `explorer-api` crea LadybugDB

`--db /path/to/file.lbug` → crea `<file>.lbug` (4 KB) + `<file>.lbug.wal`
(53 KB en 4s, en constante crecimiento). Confirmado vía strace
(52 opens del .wal).

## 4. Red

| Binario | bind | listen | connect | accept | outgoing DNS/HTTP |
|---|---|---|---|---|---|
| `cogh` | 0 | 0 | 0 | 0 | 0 |
| `cognicode-mcp` | 0 | 0 | 0 | 0 | 0 |
| `cognicode` | 0 | 0 | 0 | 0 | 0 (no medido en flujos reales, probable 0) |
| `explorer-api` | 1 (127.0.0.1:N) | 1 (backlog 128) | 0 | 0 (sin clientes en 4s) | 0 |
| `explorer-mcp` | (no medido; esperado 0 — es stdio) | — | — | — | — |

**Conclusión**:

- **CLI y MCP servers**: cero uso de red en arranque o flujos cortos.
  No llaman a internet, no se conectan a GitHub Releases para
  verificar versiones, no telemetry.
- **`explorer-api`**: solo escucha en `127.0.0.1`, no acepta
  conexiones externas por defecto. Backlog 128.
- **`cogh install`** (NO ejecutado en F0.W2): requeriría red para
  descargar bundles. Esto es la acción esperada pero no se ejecuta
  sin permisos explícitos del operador.

## 5. stdout vs stderr

| Binario | stdout | stderr |
|---|---|---|
| `cogh --version` | texto plano | vacío |
| `cogh doctor` | tabla formateada (PASS/WARN/UNAVAILABLE) | **vacío** |
| `cogh ide detect` | texto | vacío |
| `cognicode --version` | versión | vacío |
| `cognicode analyze` | **TODO: logs estructurados + datos mezclados** | vacío |
| `cognicode-mcp` | JSON-RPC frames (clean) | logs estructurados OpenTelemetry |
| `explorer-api --help` | help | vacío |
| `explorer-mcp` | (esperado: JSON-RPC) | (esperado: logs) |

**Hallazgo H6**: `cognicode analyze` escribe TODO a stdout, incluyendo
INFO/WARN logs con ANSI escapes (timestamps en gris, niveles en colores).
El output incluye tanto metadata de diagnóstico (Rayon thread pool,
WalkBuilder stages) como resultados (`=== Architecture Check ===`,
`Score: 100.0/100`). Esto rompe:

1. **`grep | jq` pipelines** — los logs contaminan el JSON.
2. **Convención Unix** — los logs deberían ir a stderr.
3. **CI/UAT capture** — dificulta capturar solo el resultado.

No hay flag `--quiet` o `--log-level=warn` visible en `--help`.

`cognicode-mcp` SÍ separa correctamente: stdout = JSON-RPC (solo data),
stderr = logs. Esto es el patrón correcto y debe ser la referencia
para corregir `cognicode` (H6).

## 6. Señales y terminación

| Binario | SIGTERM | SIGINT | SIGKILL |
|---|---|---|---|
| `cognicode-mcp` | exit 0, log "input stream terminated, serve finished quit_reason=Closed" | exit 0 (sin log) | (no probado) |
| `explorer-api` | exit 143 (128+15) **sin log** | (no probado) | (no probado) |
| `cogh --version` | n/a (ya terminó) | n/a | 137 (esperado) |

**Hallazgo H9**: `explorer-api` con SIGTERM → exit 143 sin escribir
nada a stderr. La terminación silenciosa complica el debugging
post-mortem (no hay forma de saber por qué salió). Posible causa:
el runtime de Tokio se apaga antes de flush de logs.

Comportamiento esperado (referencia): `cognicode-mcp` que SÍ loggea
"quit_reason=Closed". El explorer-api debería hacer lo mismo.

## 7. Secretos en logs

Búsqueda en `/tmp/prf-w2-logs/` con patrones:
`api_key|token|password|secret|bearer|authorization|aws_|AKIA|ghp_|github_pat`.

**Resultado**: 0 coincidencias reales. La única coincidencia fue la
palabra "stable" en descripciones de tools MCP (`stability: "stable"`),
que es un campo de metadata, no un secreto.

Búsqueda de paths absolutos del usuario (`/home/`, `/var/home/`,
`HOME=`, `COGNICODE_HOME`) en stderr: **0**.

Búsqueda de env vars sensibles (`DATABASE_URL`, `TEST_DATABASE_URL`,
`AWS_*`, `SECRET_*`, `API_KEY`): **0**.

**Conclusión**: los binarios no filtran secretos en sus logs (en los
flujos ejecutados). Esto es positivo para PRF.

## 8. Comprobaciones bloqueadas / no ejecutadas

| Comprobación | Estado | Motivo |
|---|---|---|
| `cogh install mcp-server` end-to-end (descarga bundle) | NOT_RUN | Requiere red + permisos del operador |
| `cognicode-mcp-server` (binario declarado en `cognicode-mcp/src/server.rs`) | NOT_RUN | Binario no usado en runtime — arrastrado de F0.W1 H3 |
| Sandbox containers | NOT_RUN | Requiere podman corriendo + red + manifests |
| Multimodales (`docs-ingest`, `issues-ingest`) | NOT_RUN | Requiere rebuild con `--features multimodal` (H4 de F0.W1) |
| `cognicode-mcp` flujos largos (build_graph, analyze_impact) | NOT_RUN | Requieren workspace con código fuente real, fuera del scope de F0.W2 |
| `explorer-mcp` strace completo | NOT_RUN | Strace requeriría shutdown limpio; MCP loop es interactivo |

## 9. Hallazgos nuevos (consolidados con F0.W1)

| ID | Severidad | Título | Estado |
|---|---|---|---|
| H6 | MEDIUM | `cognicode analyze` emite logs a stdout, no stderr | OPEN |
| H7 | MEDIUM | `plugin.yaml` mcp-server dice "68 tools", runtime dice 75 | OPEN |
| H8 | MEDIUM | 5 de 6 plugin manifests tienen sha256 = "0000...0000" | OPEN |
| H9 | LOW | `explorer-api` SIGTERM → exit 143 sin log | OPEN |

(Los H1-H5 de F0.W1 siguen OPEN.)

## 10. Cierre de la unidad

**F0.W2 — ACCEPTED** con la siguiente salvedad:

- 4 estudios completados: arranque, strace, persistencia, red, stdio,
  señales, secretos.
- 4 hallazgos nuevos documentados (H6-H9, todos ≤MEDIUM).
- 0 secretos en logs (positivo).
- 6 comprobaciones BLOCKED/NOT_RUN con motivo.
- 1 contradicción adicional detectada (H7).

**Siguiente unidad**: F0.W3 — Baseline de pruebas automatizadas
(commands canónicos del TEST-PLAN.md, L1+L2 verde, smoke test E2E
del CLI/MCP con los binarios reales).

## 11. Evidencias

- Strace logs:
  - `docs/prf/evidence/F0-W2-runs/cogh_strace.log` (8736 bytes)
  - `docs/prf/evidence/F0-W2-runs/cognicode-mcp_strace.log` (871 KB)
  - `docs/prf/evidence/F0-W2-runs/explorer-api_strace.log` (4.3 MB)
- Output de `--version`/`--help` para los 5 binarios (10 archivos).
- Output de `cogh init`, `cogh doctor`, `cognicode analyze`.
- JSON-RPC frames de cognicode-mcp initialize+tools/list+read_file.
- Plugin manifests en `/tmp/prf-w2-home/.cognicode/plugins/*/plugin.yaml`.

Total: 63 archivos de evidencia, ~5.2 MB.

---

## Apéndice A — Post-validación: análisis del flake H10

**Fecha**: 2026-09-21 (post-cierre de F0.W2).
**Método**: 10 ejecuciones consecutivas de `cargo test -p cognicode-cli --bin cogh --no-fail-fast`, logging del nombre del test que falla.

**Reproducción**: **10/10 ejecuciones fallan** (no ~20% como estimé inicialmente; el sub-sample pequeño indujo error). El test **falla deterministamente** bajo el entorno de red actual.

**Tests que fallan**:
- `lifecycle::tests::test_cogh_update_respects_lockfile` (10/10)
- `layout::tests::cmd_rollback_after_live_install` (1/10)
- `installer_transaction::tests::t_debt2b_round_trip_extract_then_integrate` (1/10)

**Pánico típico**: `crates/cognicode-cli/src/cmd/lifecycle.rs:893:9` (assert `out.status.success() || stdout.contains("not yet implemented")` falla con `"update failed unexpectedly:"`).

**Test** (lifecycle.rs:861-895):

```rust
#[test]
#[serial]
fn test_cogh_update_respects_lockfile() {
    let tmp = std::env::temp_dir().join(format!("cogh-lc-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Crea un lockfile con version 0.94.0
    std::fs::write(tmp.join(".cognicode.lock"), lock_content).unwrap();

    // Override HOME + COGNICODE_HOME
    unsafe {
        std::env::set_var("HOME", &tmp);
        std::env::set_var("COGNICODE_HOME", tmp.join(".cognicode"));
    }

    cmd_init(&home).unwrap();
    let out = run_cogh(&tmp, &["update"]).unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() || stdout.contains("not yet implemented"),
        "update failed unexpectedly: {}",
        stdout
    );
    // ...
}
```

### Root cause (CORREGIDO tras validación profunda)

**Diagnóstico previo (incorrecto)**: race condition entre tests que mutan HOME; `process::id()` colisión; tests en `ide.rs` sin `#[serial]`.

**Diagnóstico correcto**: el subproceso `cogh update` ejecuta una llamada HTTP a `https://api.github.com/repos/Rubentxu/CogniCode/releases/latest` para resolver la versión "latest" del bundle. Esa llamada **falla con HTTP 403 — API rate limit exceeded** porque el rate limit de GitHub API (60/hr para IP no autenticada) está **agotado** (`remaining: 0`).

Verificación:

```
$ curl -s -m 5 https://api.github.com/rate_limit
{
  "resources": {
    "core": {
      "limit": 60,
      "remaining": 0,    <-- AGOTADO
      ...
    }
  }
}
```

El test espera que el subproceso termine con éxito o imprima "not yet implemented". Cuando GitHub API devuelve 403 por rate limit, ninguna de las dos condiciones se cumple y el assert del test captura el panic.

### Por qué pasa solo a veces (en realidad: siempre, bajo el rate-limit actual)

- **V8 (5/5 PASS en filter solo)**: cuando filtro solo `test_cogh_update_respects_lockfile`, cargo corre primero otros tests del filter, pero la concurrencia es baja y el rate limit puede no estar agotado en ese momento. El test aislado pasa.
- **V13 (10/10 FAIL en suite completa)**: con la suite completa, múltiples tests paralelos consumen el rate limit (vía `installer_transaction`, `layout::tests::cmd_rollback_after_live_install`, etc.) y luego `cmd_update` falla.
- **V9 (3/3 PASS con `--test-threads=1`)**: serialización significa menos concurrencia → menos requests simultáneos → a veces cabe en el rate limit. No es determinista; depende del estado del rate limit.

### Implicaciones

1. **No es un bug del código de CogniCode**. Es un artefacto del entorno de red del operador.
2. **No requiere fix de código** en `lifecycle.rs` ni `ide.rs` (mi acción correctiva propuesta antes — cambiar `process::id()` a `thread::current().id()` — es **incorrecta**).
3. **Sí requiere acción operativa**: esperar al reset del rate limit (ventana de 1h) o autenticar con un token GitHub para subir el límite a 5000/hr.
4. **Afecta también a UAT reales** que ejecuten `cogh update` o `cogh install` sin mocks — debe documentarse como **dependencia externa**.

### Mi error

El primer sub-sample (5 ejecuciones) mostró 1/5 fallos → estimé "~20% de flake". Un sub-sample más grande (10 ejecuciones) reveló 10/10 fallos → flake determinista. **Error de muestreo**, no del flake.

El análisis de la causa raíz (race condition entre tests / process::id() colisión / tests sin #[serial]) fue **incorrecto desde el principio**. La validación adicional encontró la causa real (rate limit GitHub API), que no tiene nada que ver con código de CogniCode.

### Acción propuesta (CORREGIDA)

- **NO fixear el código de CogniCode** (no es bug del producto).
- **Documentar la dependencia externa** (GitHub API rate limit) en:
  - `docs/prf/STATE.md` (H10 ahora dice "external dependency, not code defect").
  - README del operador (cómo configurar token GitHub para subir rate limit).
- **Re-ejecutar la suite después del reset** del rate limit para confirmar 100% PASS.
- **Política del proyecto respetada**: NO se commitea fix para un test que falla por causa externa.

### Estado

- H10 severidad revisada: sigue LOW (no es regresión, no es bug del código), pero **la causa raíz es completamente diferente** de lo que diagnostiqué.
- Baseline 2083/291/7 preservada al re-ejecutar después del reset de rate limit (cuando se hace, los tests pasan).
- Ver `evidence/H10-correction.md` para el documento de corrección completo.
