# PRODUCT-1.0 — Community Productization Roadmap

## Integración con roadmap principal

`docs/roadmap/ROADMAP.md` sigue siendo única autoridad. Añadir sólo un outcome/puntero:

```markdown
| PRODUCT-1.0 | Community Productization | ACTIVE | docs/roadmap/community-productization/ROADMAP.md |
```

Este documento puede vivir en ese directorio.

## CP0 — Product Truth

- CP0.1 canonical product manifest.
- CP0.2 generated tool catalog.
- CP0.3 language support matrix.
- CP0.4 public profiles/stability.
- CP0.5 repo metadata/README reconciliation.
- CP0.6 version/release drift reconciliation.

**Gate PT:** toda claim pública derivable o verificable.

## CP1 — Open Source Foundation

- CP1.1 license decision + files.
- CP1.2 CONTRIBUTING.
- CP1.3 SECURITY.
- CP1.4 CODE_OF_CONDUCT.
- CP1.5 SUPPORT.
- CP1.6 issue/PR templates.
- CP1.7 Discussions/community categories.

**Gate OSS:** community health completo.

## CP2 — Public Contract Hardening

- CP2.1 agent-safe profile/read-only posture.
- CP2.2 tool authority audit.
- CP2.3 structured outputs for stable reviewer profile.
- CP2.4 schema snapshots.
- CP2.5 MCP conformance.
- CP2.6 N-1 compatibility.
- CP2.7 cancellation/stress/lifecycle.
- CP2.8 performance budgets.

**Gate HARD:** MCP/CLI contratos reproducibles.

## CP3 — Documentation Product

- IA + site.
- quickstart.
- user manual.
- MCP/CLI guide.
- integrations.
- security/limitations.
- generated reference.
- cheatsheets.
- troubleshooting.
- agent-hardness cookbook.

**Gate DOC:** usuario nuevo obtiene evidencia útil sin leer código interno.

## CP4 — Distribution

### Wave A

- Linux certified.
- install.sh/cogh.
- mise.
- MCPB + MCP Registry.
- OCI/GHCR.

### Wave B

- macOS arm64/x64.
- Aqua.
- Homebrew.
- Nix.

### Wave C

- Windows x64.
- PowerShell.
- Scoop.
- WinGet.
- asdf compatibility.

**Gate DIST:** install lifecycle UAT por canal publicado.

## CP5 — Skills

- root `cognicode`.
- agent-hardness.
- PR review.
- quality investigator alignment.
- evals.
- skills.sh discovery.
- pack.

**Gate SKILL:** agent puede descubrir y usar CogniCode sin tool-name guessing.

## CP6 — Landing + Launch Assets

- `Rubentxu/cognicode-site`.
- `cognicode.rubentxu.dev`.
- demo.
- social preview.
- reproducible benchmark.
- examples.
- launch article drafts.

## CP7 — Public Beta

- beta release.
- external testers.
- feedback loop.
- stabilization.

## CP8 — GA 1.0

- final certification.
- v1.0.0.
- registries/channels sync.
- coordinated launch.

## CP9 — Growth

- content cadence.
- case studies.
- integrations.
- community.
- adoption metrics.

## Dependencias resumidas

```text
CP0 ─┬─> CP1
     ├─> CP2
     └─> CP3 skeleton
CP1 + CP2 ─> CP3 complete
CP3 ─┬─> CP4
     ├─> CP5
     └─> CP6
CP4 + CP5 + CP6 ─> CP7
CP7 stabilization ─> CP8
CP8 ─> CP9
```

## Trabajo paralelo

CP1, CP2 y site skeleton pueden ejecutarse en paralelo si no modifican los mismos contratos. CP4 package-manager waves no deben frenar CP2.

## Regla de reparto entre el eje técnico y el eje CP

PRODUCT-1.0 no sustituye al roadmap técnico: lo consume. Las dos líneas son
paralelas y ambas pueden tener un P0 ejecutable al mismo tiempo, así que hace
falta un criterio escrito para decidir cuál absorbe capacidad. Sin esta regla el
reparto se decide por costumbre, y la costumbre con la que se ha venido
extrayendo deuda técnica indefinidamente es "seguir con el eje técnico".

**Regla: el P0 del eje CP gana si y solo si su gate bloquea a otro P0 del
roadmap. En cualquier otro caso, el eje técnico gana.**

Es decir, la pregunta que decide no es "qué es más valioso" sino "qué está
bloqueando a qué":

1. Si un item del eje CP tiene una dependencia P0 sin resolver en el eje
   técnico, se resuelve primero el técnico. Ejemplo real: la cobertura de
   aceptación m06/m10 es la que permite a A-005 clasificar los lenguajes como
   supported en vez de experimental. Sin tests, la matriz habría sido una
   declaración, no una derivación.
2. Si el item del eje CP es independiente y su gate no depende de nada técnico,
   avanza. Ejemplo real: A-009 (authority audit) se apoya en el catálogo de
   A-004, no en deuda técnica abierta.
3. La deuda técnica que no bloquea ningún P0 no se ejecuta de forma
   anticipada. Se registra con su disparador —el P0 que la desbloquee— y
   espera. CP1-DEBT-01, CP1-DEBT-02 y CP1-DEBT-03 están en esa categoría.

**Corolario sobre lo que cuenta como bloqueo.** Una dependencia se declara
bloqueante solo si el item CP no puede verificarse sin ella. "Sería más
consistente con el estilo del repo" no es un bloqueo; es una preferencia, y
una preferencia no se satisface con trabajo de otro eje.

**Corolario sobre el mantenimiento inline.** Un item del eje CP que necesite
corregir un defecto técnico lo hace dentro de su propio ciclo y lo registra
como debt con severidad y prioridad. No abre un ciclo de mantenimiento
paralelo por un defecto que su propio gate ya expone. La excepción es un
defecto que afecte al contrato público sin estar cubierto por el gate del
item: ese sí abre ciclo propio, porque el item CP no puede certificar sobre
un contrato que no sabe medir.
