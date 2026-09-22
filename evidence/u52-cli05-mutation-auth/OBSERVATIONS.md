# PRF-CLI-05 — Mutación con autorización separada; read-only por defecto

Fecha: 2026-09-22 · binarios `cognicode` (debug+release) · corpus `/tmp/cli05-corpus`

## Requisito (SPEC-CLI)

> PRF-CLI-05 MUST: los comandos de mutación/refactor exigen autorización separada y
> previsualización/rollback donde se anuncien; CLI estable core read-only por defecto,
> sin romper cliente histórico publicado.

## Hallazgos (RED real, dos defectos)

1. **Crash del comando refactor**: `cognicode refactor rename <sym> <new>` abortaba
   (SIGABRT, "corrupted size vs. prev_size" en release; panic de clap debug_asserts en
   debug): `Refactor` declaraba un posicional OPCIONAL (`operation`, con default)
   antes de un posicional REQUERIDO (`symbol`) — inválido para clap. El comando de
   mutación nunca funcionó: no hay cliente histórico publicado que romper (argv
   corregido: `refactor <SYMBOL> [NEW_NAME] --operation <op>`).
2. **Sin autorización separada**: no existía gate apply/preview; el camino de mutación
   no estaba implementado (rename solo genera preview, nunca escribe), pero tampoco
   se declaraba honestamente.

## Cambio

- `Refactor` argv corregido: symbol requerido primero, `--operation` como flag.
- `--apply` añadido como autorización separada explícita: **se rechaza** con exit 1 y
  motivo claro ("apply requires preview + rollback, which is pending") hasta que
  exista camino de aplicación con rollback. Honestidad > simulacro.
- `--format json` para preview (`cognicode.refactor.preview/v1`: action, applied:false,
  success, changes, error); progreso y logs a stderr.
- Default (sin --apply): preview-only, nunca escribe; aviso visible en stderr.

## UAT (binario real)

```
1. preview default: corre sin crash, archivo sin cambios      PASS
2. preview --format json: documento schema-versioned          PASS
3. --apply: exit 1 + motivo explícito                         PASS
4. integridad del fuente: SHA sin cambios en todos los casos  PASS
```

## Verificación

- Test de regresión nuevo: `cli05_refactor_is_preview_only_and_refuses_apply`
  (prf_cli_01_uat.rs): 6/6 passed.
- core lib 2145/0/27; cogh bin serial 304/304; clippy 0 errores.

## Estado

PASS para el criterio CLI-05 en su estado actual: core read-only por defecto
demostrado, autorización separada (`--apply`) presente y honestamente no disponible
hasta existir rollback. Deuda declarada: implementar apply con rollback (trabajo
futuro), y rename preview exige contexto de fichero real (hoy placeholder
`<unknown>` produce fallo honesto visible).
