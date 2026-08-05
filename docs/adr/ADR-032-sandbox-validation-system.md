# ADR-032 — Sandbox Validation System: Podman Quadlets + Real Repos + Scoring

**Estado**: PROPUESTO
**Fecha**: 2026-08-05
**Decisores**: CogniCode Architecture Team
**Contexto**: ADR-031 (Release 1.0.0 gate); infraestructura sandbox existente en `sandbox/` (orquestador compila, scoring 5D implementado, 22 repos clonados, 40+ manifests) pero **contenedores rotos** (digest pins falsos en `sandbox/containers/*.container`) y sin workflow CI

---

## Resumen ejecutivo

El sistema de validación para el gate 1.0.0 (ADR-031) se construye sobre la infraestructura sandbox **ya existente** — `sandbox-orchestrator` (Rust, compila), scoring de 5 dimensiones con ground-truth matchers, manifests YAML con schema, repos reales clonados, history JSONL con trends. La decisión aquí es: **reparar y completar lo existente** en lugar de reconstruir, con 5 componentes concretos:

1. **Quadlets reales** por lenguaje (podman 5.8.4, ya hay `cognicode-postgres` corriendo) — reemplazar las plantillas con digest pins reales.
2. **Corpus de repos pinneados** (22 existentes + 3 nuevos Tier-1 Rust + 3 nuevos Tier-3 stress).
3. **Scoring 5D wireado al release gate** (ya implementado en `sandbox_core::scoring`).
4. **Automatización CI** — workflow `sandbox-nightly.yml` + `just release-scorecard`.
5. **Coverage matrix** — 43 MCP tools × repos.

---

## Contexto

### Lo que YA existe (no reconstruir)

| Componente | Ubicación | Estado |
|------------|-----------|--------|
| `sandbox-orchestrator` binary | `crates/cognicode-sandbox` | ✅ Compila (6 warnings) |
| Scoring 5D (correctitud, latencia, escalabilidad, consistencia, robustez) | `cognicode-core/src/sandbox_core/scoring.rs` | ✅ Implementado con ground-truth matchers (13 tipos) |
| History JSONL + trends | `sandbox_core/history.rs` | ✅ `RunEntry`, `TrendDirection`, health score |
| Manifests YAML + schema | `sandbox/manifests/*.yaml` + `schema.json` | ✅ 40+ archivos, tiers A/B/C |
| Repos clonados | `sandbox/repos/` | ✅ 22 repos (ripgrep, serde, anyhow, cobra, bubbletea, lo, chalk, express, commander, zod, spring-petclinic, click, urllib3, requests, hiredis, json, spectre-console, elixir, slim, sinatra, argument-parser) |
| Stability runs | `scripts/run_campaign.sh --repeat N` | ✅ `stability.json` |
| Reports HTML | `scripts/generate_html_report.py` | ✅ |
| Quadlets plantilla | `sandbox/containers/*.container` | ❌ **Fake digest pins** |
| Maven | — | ❌ Missing |
| Sandbox CI workflow | `.github/workflows/` | ❌ Solo `ci.yml` (unit tests) |

### Problemas detectados (2026-08-05)

1. `sandbox/containers/rust.container` usa `docker.io/library/rust@sha256:16b31a5e2b37d0e1c9c0e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5` — digest falso (formato inválido, longitud incorrecta).
2. `sandbox/justfile:sandbox-pull` referencia los mismos digests falsos.
3. Los últimos results-runs son de 2026-06-19 — infra desusada desde hace ~7 semanas.
4. `SETUP_REQUIREMENTS.md` documenta Maven como missing.
5. No hay workflow GitHub Actions para la matriz nocturna.

---

## Decisión

### 1. Quadlets reales por lenguaje (fuente de verdad en repo)

`sandbox/containers/*.container` pasa a ser la **fuente de verdad** versionada. Deploy a `~/.config/containers/systemd/` vía `just sandbox-setup`.

| Servicio | Imagen (digest pinneado) | Propósito |
|----------|--------------------------|-----------|
| `cognicode-postgres` | postgres:16 (ya corriendo) | Store canónico (hasta migración lbug completa) |
| `cognicode-rust` | rust:1.80-slim | Repos Rust (ripgrep, serde, anyhow, tokio, clap) |
| `cognicode-python` | python:3.12-slim | Repos Python (click, urllib3, requests) |
| `cognicode-go` | golang:1.23-alpine | Repos Go (cobra, bubbletea, lo) |
| `cognicode-java` | eclipse-temurin:17-jammy + Maven | spring-petclinic |
| `cognicode-node` | node:22-slim | Repos JS/TS (chalk, express, commander, zod) |

Hardening por quadlet: `Network=none`, `MemoryMax=2g`, `PidsLimit=128`, `ReadOnly=yes` con mounts rw para `/workspace` y `/repos`, `NoNewPrivileges=yes`, `Tmpfs=/tmp`, digest pinneado + `AutoUpdate=no`.

Procedimiento de pinning: `podman pull <img>` → `podman inspect --format '{{.ImageDigest}}'` → escribir digest en quadlet → `systemctl --user daemon-reload && systemctl --user start cognicode-*`.

### 2. Corpus de repos (pinneados por SHA)

`clone_repos.sh` ya implementa pinning por commit con verificación de HEAD. Se extiende:

**Tier 1 — Rust (validación profunda)**: ripgrep, serde, anyhow (existentes) + **tokio**, **clap** (nuevos).
**Tier 2 — Multi-lenguaje (amplitud)**: cobra, bubbletea, lo, zod, commander, express, spring-petclinic, click, urllib3, requests (existentes).
**Tier 3 — Stress (escala)**: **rust-analyzer**, **typescript**, **react** (nuevos; validan G8 escalabilidad 100k+ LOC).

Todo repo pinneado a un commit SHA concreto. Drift detectado por `clone_repos.sh` (WARNING + re-pin).

### 3. Scoring → release gate (sin código nuevo, solo wiring)

El scoring 5D y el health score YA existen. El plan solo los expone:

- `just release-scorecard` → ejecuta campaña → agrega `ReleaseReadiness` con los 12 gates de ADR-031 → emite `scorecard.json` + `scorecard.md`.
- El scorecard lee: `health.json` (G3), scoring por repo Tier-1 (G4), benchmark percentiles (G5), `stability.json` (G6), failure classes (G7), scenarios scale (G8), diff vs baseline (G9).
- Gates G1/G2/G10/G11/G12 se alimentan de fuentes no-sandbox (git, openspec, docs) y se añaden al mismo scorecard por un script `release_scorecard.py`.

### 4. Automatización CI

Nuevo `.github/workflows/sandbox-nightly.yml` (nightly, self-hosted runner con podman o ubuntu-latest):

```yaml
nightly:
  - just sandbox-pull && just sandbox-setup
  - just sandbox-ci-full        # matriz completa, JSONL
  - just sandbox-stability <manifests> 5
  - just sandbox-benchmark <tools>
  - just release-scorecard      # → scorecard.md
  - upload artifacts: results-runs/, scorecard.md
```

Lane PR rápida: `sandbox-ci-smoke` (< 5 min) en el workflow PR existente.

### 5. Coverage matrix MCP tools × repos

Script `generate_tool_coverage.py`: parsea `docs/MCP-TOOLS.md` (43 tools) + scenarios en manifests → matriz tool × repo → detecta tools sin scenario → gate G2.

---

## Alternativas consideradas

| Alternativa | Rechazada porque |
|-------------|------------------|
| Docker + docker-compose | Podman ya instalado y `cognicode-postgres` corriendo con quadlets; systemd user units dan auto-start y hardening declarativo |
| Reconstruir el orquestador en otro lenguaje | El Rust ya compila y el scoring 5D está implementado y testeado — reconstruir sería descartar trabajo validado |
| Correr escenarios en el host sin contenedores | Riesgo de contaminación del entorno de desarrollo; sin límites de recursos; sin reproducibilidad |
| Sin pinning de imágenes | Digest pins + `AutoUpdate=no` garantizan reproducibilidad (requisito del gate) |

---

## Trazabilidad

- Especificación ejecutable: `openspec/specs/sandbox-validation-system/spec.md`
- Gate de release: ADR-031 + `openspec/specs/release-readiness-gate/spec.md`
- Plan maestro: `docs/RELEASE-1.0.0-PLAN.md`
- ROADMAP: sección `Release 1.0.0 Program (E30)`
