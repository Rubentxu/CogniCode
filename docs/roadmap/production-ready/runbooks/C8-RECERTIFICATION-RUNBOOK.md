# Runbook — C8-R Reproducible Recertification

## Objetivo
Reemitir C8 sobre un SHA cuyo contenido sea exactamente reproducible desde Git.

## Preflight
1. Seleccionar SHA candidato y registrarlo como `C8-R-CANDIDATE`.
2. No modificar ese SHA durante la campaña.
3. Crear fresh clone en directorio temporal.
4. Checkout detached del SHA.
5. Verificar `git status --porcelain` vacío.
6. Validar con `git ls-files`:
   - source de todos los `[[bin]]`;
   - expediente C8-R;
   - scripts usados por la campaña.

## Gates

### G1 — Source integrity
- todos los bin targets resuelven a ficheros trackeados;
- ninguna ruta contractual depende de `.gitignore`/global ignore.

### G2 — Static quality
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`.

### G3 — Test campaign
- `cargo test --workspace --all-targets`;
- suites adversariales obligatorias;
- tests CP reales.

### G4 — Release binaries
Construir release de:
- `cognicode`;
- `cognicode-mcp`;
- `cognicode-control-plane`.

Verificar `--version`/`--help` según contrato.

### G5 — CP live
Arrancar `cognicode-control-plane` contra source root del clone y comprobar:
- HTTP 200;
- `status=evaluated`;
- canonical constraint IDs completos;
- no unevaluated constraints;
- rutas fuera de scope 404.

### G6 — Evidence integrity
Guardar:
- SHA;
- comandos exactos;
- conteos de tests;
- outputs resumidos;
- hashes de binarios si se empaquetan;
- timestamp;
- versión toolchain.

## Regla de invalidación
Cualquier corrección durante la campaña produce un nuevo SHA y reinicia los gates afectados. Nunca se “continúa” el certificado como si el SHA fuese el mismo.

## Criterio de cierre
C8-R queda **técnicamente PASS** solo si todos los gates se ejecutan desde el fresh clone. La firma humana sigue siendo una decisión posterior del release operator.
