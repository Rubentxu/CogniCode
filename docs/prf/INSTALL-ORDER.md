# Install order — `cogh` v0.98.1

**Orden de operaciones obligatorio** para llegar a un `cogh doctor` con `MCP: PASS`.

## TL;DR

```bash
cogh install --home "$HOME/.cognicode" --staging <staging-dir-or-omit> cognicode --version <v>
cogh init   --home "$HOME/.cognicode"
cogh install --home "$HOME/.cognicode" --staging <staging-dir-or-omit> cognicode --version <v> --profile reviewer --ide opencode
# (repeat --ide clause for each IDE: opencode, zcode, claude, codex — NOT --ide all, see ISSUE-1)
cogh doctor --home "$HOME/.cognicode"
```

## Por qué dos `cogh install` separados

`cogh install` materializa el runtime (`tracker/version`, `versions/<v>/cognicode/bin/`,
`versions/<v>/cognicode-mcp/bin/`, shims, journal, `bundle.yaml`). Pero **no** crea los
bundled plugin markers (`~/.cognicode/plugins/{codex,claude,zcode,mcp-server,sandbox-templates,skills-cognicode-core}/`,
`~/.cognicode/bin/`). Esos markers los crea `cogh init`.

`cogh doctor` reporta `FAIL Core health missing: bin/` si no se ejecutó `cogh init`.

`cogh doctor` reporta `UNAVAILABLE MCP active installation does not include the daemon capability`
si el primer install no se hizo con `--profile reviewer`.

## Diagnóstico post-install

```bash
cogh doctor --home "$HOME/.cognicode"
```

| Estado | Significado | Acción |
|---|---|---|
| `FAIL Core health missing: bin/` | Faltan bundled plugin markers | `cogh init` |
| `FAIL Core health missing: shims/` | Faltan shims | `cogh reshim` o re-install |
| `UNAVAILABLE MCP — active installation does not include the daemon capability` | Faltó `--profile reviewer` en install | re-install con `--profile reviewer` |
| `PASS MCP — cognicode-mcp shim present (declared by active install)` | OK | (proceder) |

## ISSUE-1 — `--ide all` no soportado

```
$ cogh install --ide all
Error: IDE 'all' is not supported by cogh yet (opencode/zcode/claude/codex in E32-D/E/F/G)
```

`--ide` acepta **un nombre por invocación**. Para configurar varios IDEs, ejecutar `cogh install` con
cada `--ide` por separado, o repetir con `cogh init` que internamente configura los 6 bundled plugins
pero no toca las configs de los IDEs reales.

Alternativa:

```bash
cogh install ... --profile reviewer --ide opencode
cogh install ... --profile reviewer --ide zcode
cogh install ... --profile reviewer --ide claude
cogh install ... --profile reviewer --ide codex
```

O configurar el IDE manualmente después del install base:

```bash
cogh ide detect           # ver qué IDEs detecta
cogh ide install --ide opencode cognicode --version <v>
```

## Issue de downgrade / rollback parcial

`cogh rollback --to <same-version>` deja estado parcial observable (ver
[`docs/prf/COGH-ISSUES.md`](COGH-ISSUES.md)). Recovery: re-install desde red.

## Smoke verificado

Sobre `v0.98.1` real, el ciclo completo está documentado en `docs/prf/JOURNAL.md` §148
(búsqueda: "DIST-02 PASS contra v0.98.1"). SHA256 reproduce, MCP JSON-RPC stdio devuelve
20 tools con `authority=read` consistente con `cognicode_meta.cognicode.authority`.

## Refs

- `docs/prf/JOURNAL.md` §146 (B3 sobre v0.98.0), §148 (B5 sobre v0.98.1)
- `docs/prf/RELEASE-CANDIDATE.md` (Sección "Cierre de PRF")
- `docs/prf/COGH-ISSUES.md` (ISSUE-1)
- https://github.com/Rubentxu/CogniCode/releases/tag/v0.98.1
