# lsp-goto-definition-bug — `goto_definition` returns None con rust-analyzer instalado

## Goal

Investigar y arreglar el bug que `test_rust_analyzer_goto_definition` y
`test_pyright_goto_definition` pinean tras re-habilitar los `#[ignore]`
que silenciaban el path end-to-end.

## Problem

MEDIDO 2026-10-09 sobre `integrate/v1015` HEAD `15b5c68c` con
`rust-analyzer` instalado (`/home/rubentxu/.cargo/bin/rust-analyzer`):

    $ cargo test -p cognicode --test lsp_integration_test --no-fail-fast
    test test_extract_location_from_lsp_params ... ok
    test test_pyright_goto_definition ... FAILED    (pyright no instalado)
    test test_pyright_find_references ... ok
    test test_availability_detection_asks_the_binary_and_not_the_filesystem ... ok
    test test_rust_analyzer_goto_definition ... FAILED
    test test_availability_detection_of_the_lsp_servers_agrees_with_the_binaries ... ok
    test test_rust_analyzer_hover ... ok
    FAILED. 4 passed, 1 failed, 2 ignored

El panic (`none_is_not_a_verdict`, line 53):

> "`goto_definition` devolvio `None`. Eso NO es un resultado aceptable: con el
>  LSP disponible significa que el servidor arranco y no respondio, y sin el
>  LSP significa que el servicio degrado a tree-sitter sin decirlo."

El test `test_rust_analyzer_hover` pasa (LSP path funciona para hover),
pero `test_rust_analyzer_goto_definition` falla con None. La diferencia
entre hover y goto_definition es **el método LSP**, no el binario. Esto
apunta a uno de:

  (a) Request format incorrecto especificamente para goto_definition.
  (b) Response parsing incorrecto especificamente para goto_definition.
  (c) rust-analyzer genuinamente no encuentra la definicion (bug en el
      setup del test, p.ej. URI scheme, position 0-indexed vs 1-indexed).

## Why

El test fue endurecido a fail-closed en commit reciente (`7b37a35e` test(lsp):
dos tests que no podian fallar, y la propiedad que los salva). El autor
NO quito los `#[ignore]` al endurecerlos, porque la suposicion era que
los binarios no estaban. En esta maquina `rust-analyzer` SI esta,
asi que los tests ahora se ejecutan y el goto_definition falla-closed,
surfacing el bug que el `#[ignore]` original enmascaraba.

Misma leccion que M0.5/M0.6/M0.10/M0.12: un `#[ignore]` con razon
"requiere binario X" deja de ser honesto cuando el binario esta y el
test falla. La accion correcta es **re-habilitar y dejar el rojo
surfacing el bug**, no seguir silenciando.

## Scope

### S1 — Re-habilitar los tests rust-analyzer (DONE)
- Drop `#[ignore = "requires rust-analyzer binary"]` en
  `test_rust_analyzer_hover` y `test_rust_analyzer_goto_definition`.
- Mantener los `#[ignore = "requires pyright binary"]` (pyright NO
  esta instalado en esta maquina).
- Commit: `test(lsp): re-habilita los tests rust-analyzer; el
  goto_definition sale rojo honrando el bug real.`

### S2 — Diagnosticar el bug (TODO)
- Confirmar que `rust-analyzer` arranca con `route_operation("goto_definition", ...)`.
- Verificar request format (`textDocument/definition` con position 0-indexed).
- Verificar response parsing (`Option<Location>` vs `Vec<Location>` vs `LocationLink`).
- Si rust-analyzer responde correctamente y el proxy degrada: bug en
  el composite tier ordering o en `definition_chain`.

### S3 — Fix y cerrar (TODO)
- Fix minimo en el sitio correcto (no rebuild del LSP stack).
- Test verde end-to-end con rust-analyzer.
- Re-correr `cargo test -p cognicode --test lsp_integration_test` 6/6 verde.

## Constraints

- **No reabrir** el endurecimiento fail-closed (lesson 79 / commit
  `7b37a35e`). El panic sigue siendo la red minima.
- **No degradar** la ruta LSP a tree-sitter silenciosamente para
  silenciar el RED. El bug hay que mirarlo, no esconderlo.
- **No eliminar** el `#[ignore]` de los tests pyright hasta que
  pyright este instalado.

## Evidence

| Comando | Resultado |
|---|---|
| `cargo test -p cognicode --test lsp_integration_test --no-fail-fast` | 4 passed, 1 failed (test_rust_analyzer_goto_definition), 2 ignored (pyright) |
| `which rust-analyzer` | `/home/rubentxu/.cargo/bin/rust-analyzer` (presente) |
| `which pyright` | (ausente) |
| `cargo test --workspace` | Incluye el RED como senal al operador; el resto del workspace sigue verde |

## Diagnostico adicional 2026-10-09

Tracing desde `composite::attempt_lsp_definition` (line 576):

  1. `gate_lsp_for_location` pasa (rust-analyzer esta registrado, binario
     responde a `--version`).
  2. `self.lsp.get_definition(location)` se ejecuta.
  3. `process_manager.request(language, "textDocument/definition", params)`
     envia la peticion JSON-RPC.
  4. **Aqui se sospecha el bug**: `proc.request` en `process.rs:163` hace
     `Ok(result.result.unwrap_or(Value::Null))`. Si rust-analyzer
     responde con `result: null`, esto devuelve `Ok(Value::Null)`.
  5. `lsp.rs:177` mapea `Value::Null` → `Ok(None)` y termina.
  6. `attempt_lsp_definition:587` trata `Ok(None)` como `Attempt::Served(None)`
     con el comentario historico: 'LSP Ok(None) is an authoritative
     unresolved answer: no lower tier may second-guess it'.
  7. La chain retorna Served(None) en S2.
  8. `get_definition` retorna `Ok(None)` al caller.

**Comparacion con `attempt_lsp_hover`** (line 627): trata `Ok(None)` como
`Degraded`, cae a S1. Esa es la diferencia: hover degrada, goto_definition
"sirve" el None. Inconsistente y la causa del RED.

**Causa raiz probable (no confirmada)**: rust-analyzer arranca en cold
start, primer goto_definition llega antes de que el index este completo
(~10s), responde con null. El test da 30s pero el LSP server puede
haberse "estancado" sin reintentar. Hover pasa porque no requiere
resolucion semantica (solo signature del nodo actual).

**Hipotesis del fix** (a validar antes de aplicar):

  (A) En `attempt_lsp_definition`, mapear `Ok(None)` → `Attempt::Degraded`
      como hace hover. Cae a S1 (local resolver). Cambio pequeno, pero
      rompe la regla historica "LSP None es autoritativo".

  (B) Agregar warmup: esperar a que rust-analyzer termine el index antes
      del primer goto_definition. Cambia el LSP startup, no la chain.

  (C) Aumentar el timeout del test a 60s para dar tiempo al cold start.
      Workaround, no fix.

Recomendacion: empezar con (A) que es el cambio coherente con hover,
validar que los otros consumers de `get_definition` aceptan el cambio de
semantica (deben mirar la chain outcome en vez de None directo).

## Out of scope

- Re-arquitectura del LSP stack.
- Re-habilitar tests pyright (binario ausente).
- Cambiar el formato del request/response (sin diagnostico previo).