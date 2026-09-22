# U21 — PRF-DIST-04: Kill -9 mid-install (SIGKILL recovery UAT)

Fecha: 2026-09-22 · RC 0.97.3 · entorno local Fedora x86-64 · asset mirror local `http://127.0.0.1:37527` sirviendo `/tmp/dist03` (ruta `<base>/v0.97.3/`)

## Escenario

HOME desechable `/tmp/dist03/u21home`. Se lanza `cogh install mcp-server --version 0.97.3 --profile reviewer` y se mata con SIGKILL a los 30 ms (en plena descarga/extracción).

## Ejecución 1 — RED (binario RC 0.97.3 sin fix)

Tras SIGKILL:

- `versions/` vacío, `journal/` vacío, `shims/` vacío → rollback de estado correcto.
- `cache/` conserva tars parciales (esperado, cache no es estado activo).
- `cogh doctor` tras reinicio: MCP UNAVAILABLE, overall healthy → honesto.
- **FALLO REAL**: la reinstalación falla con `cannot acquire CogniCode install lock ... File exists (os error 17); another installation may be in progress`. El lock del proceso muerto bloquea la recuperación **para siempre**. Ni doctor ni install lo detectan como huérfano.

## Corrección (commit de esta unidad)

`install_lock.rs`: el lock registra `pid:timestamp`. En colisión:

- Si el PID registrado no existe (`/proc/<pid>` ausente) o el contenido es ilegible/pid=0 → lock huérfano (stale): se elimina y se reintenta una adquisición atómica (create_new). Si otro proceso lo recrera en medio, se pierde la carrera y se falla honestamente.
- Si el PID está vivo → error nuevo y explícito: `another live installation is in progress`.
- En no-Linux sin sonda de vida: se trata el holder como vivo (nunca se roba un lock que no se puede probar huérfano).

Tests añadidos (3, todos serial): dead-pid takeover, corrupt contents stale, live-pid refusal.

## Ejecución 2 — GREEN (binario reconstruido)

Tras SIGKILL a 30 ms:

- versions/journal/shims vacíos (rollback correcto).
- doctor: UNAVAILABLE MCP + overall healthy (honesto).
- **Reinstalación tiene éxito**: toma el lock huérfano, instala 0.97.3, shim PASS, doctor overall healthy, `shims/cognicode-mcp --version` → `cognicode-mcp 0.97.3`.

## Verificación

- `cargo test -p cognicode-cli --bin cogh -- --test-threads=1` → **304 passed, 0 failed** (incluye los 7 de install_lock).
- `cargo test -p cognicode-cli --test cognicode_lifecycle` → 7 passed.
- `cargo clippy --release -p cognicode-cli` → 0 errores.

## Estado

PASS para el escenario U21 (kill mid-install + recuperación). Deuda menor: `cache/` conserva descargas parciales sin GC (no bloquea; espacio en disco solo).
