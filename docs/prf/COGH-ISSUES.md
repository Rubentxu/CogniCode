# COGH-ISSUES — Bugs del binario `cogh` v0.98.1

Documentación de comportamiento inesperado del binario instalador `cogh` v0.98.1
(encontrado durante B3 §146 y B5 §148). NO son bugs del runtime CogniCode; son del
instalador. **Recovery paths incluidos**.

## ISSUE-1 — `cogh rollback --to <same-version>` deja estado parcial irrecuperable

**Severidad**: media (caso de borde — rollback contra misma versión instalada).
**Workaround**: re-install desde red.
**Estado upstream**: pendiente reportar.

### Reproducción

```bash
# Estado inicial: v0.98.1 ya instalado (tracker/version = 0.98.1)
$ cogh rollback --home "$HOME/.cognicode" --to 0.98.1
rolling back version 0.98.1
restored tracker pin to 0.98.1
Error: rollback partially applied: tracker restored to 0.98.1 but shim resurrection failed
  (read bundle.yaml /home/user/.cognicode/versions/0.98.0/manifest.yaml);
  the journal at /home/user/.cognicode/journal/0.98.0.json has been PRESERVED so
  the rollback can be retried with `cogh rollback --to 0.98.1`
```

### Estado del filesystem tras el rollback fallido

- `tracker/version` → restaurado a `0.98.1` ✓
- `versions/0.98.1/` → **borrado** ✗
- `shims/` → **borrado** ✗
- `bundle.yaml` → presente ✓
- `journal/0.98.1.json` → **preservado** ✓ (con lista completa de efectos a reaplicar)
- `cache/` → preservado ✓

### Reintento del rollback: mismo error

```bash
$ cogh rollback --home "$HOME/.cognicode" --to 0.98.1
Error: resume pending rollback for 0.98.1: shim resurrection failed (read bundle.yaml
  /home/user/.cognicode/versions/0.98.0/manifest.yaml);
  journal at /home/user/.cognicode/journal/0.98.0.json preserved for another retry
  (restore the underlying manifest of 0.98.0 and re-run `cogh rollback --to 0.98.0`)
```

El reintento falla con el mismo error. El journal existe pero el binario no puede
regenerar `versions/0.98.1/manifest.yaml` desde él.

### Recovery path

```bash
# 1. Limpiar el pending-rollback state manualmente
$ rm -rf ~/.cognicode/versions/0.98.1
$ rm -f  ~/.cognicode/journal/0.98.1.json

# 2. Re-instalar desde red (funciona, deja doctor healthy)
$ cogh install --home ~/.cognicode cognicode --version 0.98.1
$ cogh init   --home ~/.cognicode
$ cogh install --home ~/.cognicode cognicode --version 0.98.1 --profile reviewer --ide opencode
$ cogh doctor --home ~/.cognicode
==> overall: healthy
```

### Análisis

La release body de v0.98.1 dice:

> `fix(cli): cmd_rollback owns shim resurrection, refuses Ok on partial apply`

El "refuses Ok on partial apply" sí funciona (no reporta éxito falsamente). Pero el
"shim resurrection" no se completa: el journal preserva la lista de efectos, pero
`cogh` no puede reescribir `versions/<v>/manifest.yaml` desde esa lista sin
re-descargar de red.

### Contrato cumplido

- ✅ Estado **observable** (no destructivo): versions/ borrado pero cache/ y journal/ preservados.
- ✅ Refuses Ok on partial apply (no miente sobre éxito).
- ❌ "Shim resurrection" incompleta (debería regenerar desde el journal sin red).

## ISSUE-2 — `cogh install --ide all` no soportado

**Severidad**: baja (cosmetic).
**Workaround**: usar `--ide <name>` por separado.

```bash
$ cogh install --ide all
Error: IDE 'all' is not supported by cogh yet (opencode/zcode/claude/codex in E32-D/E/F/G)
```

`--ide` acepta un nombre por invocación. Para configurar los 4 IDEs soportados:

```bash
cogh install ... --ide opencode
cogh install ... --ide zcode
cogh install ... --ide claude
cogh install ... --ide codex
```

## ISSUE-3 — `cogh install` no inicializa `~/.cognicode/` markers

**Severidad**: baja (documentación; no es bug en sí).
**Workaround**: ejecutar `cogh init` después de `cogh install`.

`cogh install` deja `tracker/version`, `versions/<v>/`, `shims/`, journal, `bundle.yaml`.
Pero NO crea `.init`, ni `plugins/{codex,claude,zcode,mcp-server,sandbox-templates,skills-cognicode-core}/`,
ni `bin/`. Estos markers los crea `cogh init`.

Sin `cogh init`, `cogh doctor` reporta `FAIL Core health missing: bin/`.

Ver [`docs/prf/INSTALL-ORDER.md`](INSTALL-ORDER.md) para el orden correcto.

## Refs

- `docs/prf/JOURNAL.md` §146 (B3) y §148 (B5)
- `docs/prf/INSTALL-ORDER.md` (orden install + init)
- `docs/prf/RELEASE-CANDIDATE.md` (frentes abiertos)
