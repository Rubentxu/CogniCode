# Production-Ready Foundation (PRF) — TEST-PLAN

## Niveles de prueba

PRF adopta la pirámide de pruebas clásica con cuatro niveles, cada uno
con un propósito distinto:

### L1 — Pruebas unitarias

- **Qué prueban**: funciones, métodos, módulos aislados.
- **Cómo**: `cargo test --lib`, `cargo test --bin <bin>`.
- **Velocidad**: < 1 s por crate.
- **Cobertura**: la mayor parte del código debe tener tests L1.
- **Cuándo**: en cada cambio de código.

### L2 — Pruebas de integración en proceso

- **Qué prueban**: interacción entre módulos dentro del mismo crate.
- **Cómo**: `cargo test --tests` (incluye `tests/*.rs`).
- **Velocidad**: < 30 s por crate.
- **Cobertura**: flujos completos del crate (por ejemplo, ciclo
  install→uninstall de `cogh`, ciclo read→edit de file ops).
- **Cuándo**: en cada cambio que afecte a más de un módulo.

### L3 — Pruebas sobre el binario real

- **Qué prueban**: el binario compilado responde a sus argumentos y
  entorno como se espera.
- **Cómo**: se invoca el binario (built con `just build` o
  `cargo build --release`) con argumentos controlados.
- **Velocidad**: segundos a minutos por flujo.
- **Cobertura**: las interfaces públicas de cada binario.
- **Cuándo**: en cada UAT documentada en `evidence/UAT-*.md`.

### L4 — Pruebas UAT con escenarios reales

- **Qué prueban**: el binario funciona en escenarios reales
  (instalar el bundle, configurar un IDE, ejecutar el daemon, etc.).
- **Cómo**: combinación de L3 + verificación contra especificación.
- **Velocidad**: minutos.
- **Cobertura**: las UAT obligatorias definidas por cada requisito.
- **Cuándo**: en cada promoción a `ACCEPTED` o `RELEASED`.

## Asignación por binario

| Binario | L1 (cargo test) | L2 (tests/) | L3 (binario) | L4 (UAT) |
|---|---|---|---|---|
| `cognicode` | `cargo test -p cognicode-core --lib` | `cargo test -p cognicode-core --tests` | `cargo run -p cognicode -- ...` | `evidence/UAT-cognicode-*.md` |
| `cognicode-mcp` | (covered by cognicode-core lib tests) | (covered by core tests) | `cargo run -p cognicode-mcp -- ...` | `evidence/UAT-cognicode-mcp-*.md` |
| `cogh` | `cargo test -p cognicode-cli --bin cogh` | `cargo test -p cognicode-cli --tests` | `cargo run -p cognicode-cli --bin cogh -- ...` | `evidence/UAT-cogh-*.md` |
| `explorer-mcp` | (covered by core tests) | (covered by core tests) | `cargo run -p cognicode-explorer --bin explorer-mcp -- ...` | `evidence/UAT-explorer-mcp-*.md` |
| `explorer-api` | (covered by core tests) | (covered by core tests) | `cargo run -p cognicode-explorer --bin explorer-api -- ...` | `evidence/UAT-explorer-api-*.md` |

## Convenciones

- Los tests L1 deben ser deterministas. Sin sleeps fijos ni orden
  implícito.
- Los tests L2 que tocan el filesystem deben usar `tempfile::TempDir`.
- Los tests L3 deben ejecutarse en un `TMPDIR` aislado cuando usen
  temporales.
- Los tests L4 deben documentar la receta exacta usada para que sean
  reproducibles.

## Comandos canónicos

```bash
# L1 — unit tests del crate principal
cargo test -p cognicode-core --lib

# L2 — integration tests del crate principal
cargo test -p cognicode-core --tests

# L1+L2 — todo el crate
cargo test -p cognicode-core

# L1 — CLI unit
cargo test -p cognicode-cli --bin cogh

# L2 — CLI integration (cognicode_ide_adapter)
cargo test -p cognicode-cli --test cognicode_ide_adapter

# L3 — build del binario
just build-server
# o
cargo build -p cognicode-cli --bin cogh --release
cargo build -p cognicode-core --bin cognicode
cargo build -p cognicode-mcp
cargo build -p cognicode-explorer

# L4 — UAT manual
./target/release/cogh --version
./target/release/cogh --help
./target/release/cognicode --help
./target/release/cognicode-mcp --help
# etc.
```

## Política de fallos

- Un fallo en L1 o L2 bloquea el commit. Se corrige antes de seguir.
- Un fallo en L3 bloquea la promoción a `INTEGRATED`.
- Un fallo en L4 bloquea la promoción a `ACCEPTED` y `RELEASED`.
- Los tests flaky no se eliminan: se investiga la causa raíz y se
  corrige (ver disciplina de implementación, sección 4 de la
  instrucción principal).
