# Proposal: e30-sandbox-infra — Reparación de infraestructura del sandbox (Fase 0)

**Idioma**: Español (documento efímero, no se pushea a remoto)
**Fecha**: 2026-08-06
**Contexto**: ADR-031, ADR-032, RELEASE-1.0.0-PLAN.md, explore-report (6 gaps), spec `sandbox-validation-system`

---

## Intent

La infraestructura del sandbox de validación para el scorecard G3–G9 (ADR-031) tiene **buenos huesos** (orquestador compila, scoring 5D implementado, 22 repos clonados, 60+ manifests) pero **los contenedores están rotos**: 6 imágenes con digests falsos/ausentes, go.container no endurecido, setup despliega solo 3/6 containers, Maven documentado como missing, y sin workflow CI nocturno. Sin estos arreglos, `sandbox-ci-smoke` no puede pasar y la Fase 0 del release 1.0.0 está bloqueada.

**Este cambio repara los 6 gaps identificados en explore para que el sandbox sea funcional, reproducible y automatizado.**

---

## Scope

### In Scope

| # | Gap | Acción | Archivos |
|---|-----|--------|----------|
| 1 | **Digests falsos** en rust/python/java (patrón `e5e5e5...` inválido) y floating tags en js/ts/go | Pull real de cada imagen → extraer SHA-256 real → reescribir `.container` + `justfile: sandbox-pull` | 6 `.container` + `justfile` |
| 2 | **go.container no endurecido** (`Network=host`, `AutoUpdate=registry`) | Alinear con spec: `Network=none`, `AutoUpdate=no`, `MemoryMax=2g`, `PidsLimit=128` | `go.container` |
| 3 | **Setup despliega solo 3/6** containers (rust/python/java; falta go/js/ts) | Merge `sandbox-setup-js-ts` + go en `sandbox-setup` principal | `justfile: sandbox-setup` |
| 4 | **Contradicción Maven/Gradle** — ADR-032 y RELEASE-1.0.0-PLAN mandan Maven; `java_repos.yaml` usa `./gradlew` | Decisión: instalar Maven en imagen Java derivada; migrar manifiestos a `mvn`. Ver Decisión abajo. | `java.container`, `java_repos.yaml`, `SETUP_REQUIREMENTS.md` |
| 5 | **spring-petclinic pinned a `main`** (branch, no SHA) | Re-pin a commit SHA concreto (ya registrado en `java_repos.yaml`: `edf4db28affcc4741c79850a3d95bc3f177b5ff9`) | `clone_repos.sh` línea 192 |
| 6 | **Sin workflow nightly** | Nuevo `sandbox-nightly.yml`: schedule diario, setup podman, smoke + probe lanes, upload artifacts | `.github/workflows/sandbox-nightly.yml` |

### Out of Scope

- **Tier-3 repos** (tokio, clap, rust-analyzer, typescript, react) — son Fase 2 (`e30-corpus-expansion`)
- **Coverage matrix 43 tools** — Fase 2
- **Ground-truth fixtures nuevos** — Fase 2
- **Baseline congelado + scorecard** — Fase 3 (`e30-metric-baseline`)
- **Derived images con tools pre-instaladas** (eslint, jest, typescript) — los `.container` de js/ts ya documentan el procedimiento; la decisión de SI construir imágenes derivadas ahora o en Fase 2 es de implementación, no de propuesta
- **MCP server binary funcional contra repos reales** — este cambio garantiza que el sandbox *pueda* ejecutar escenarios (infra verde); que los escenarios *pasen* (producto verde) es scope de fases posteriores

---

## Decisión: Maven vs Gradle para Java

### Evidencia

| Fuente | Dice |
|--------|------|
| **ADR-032 §1** | `cognicode-java \| eclipse-temurin:17-jammy + Maven \| spring-petclinic` |
| **RELEASE-1.0.0-PLAN §3.1** | `cognicode-java \| eclipse-temurin:17-jammy + maven \| spring-petclinic` |
| **SETUP_REQUIREMENTS.md** | `Java \| maven \| 3.8+ \| spring-petclinic` — y marca Maven como ❌ MISSING |
| **java_repos.yaml** | Usa `./gradlew compileJava -q` y `./gradlew test -q` |
| **spring-petclinic repo** | Tiene AMBOS: `pom.xml` + `mvnw` (Maven wrapper) Y `build.gradle` + `gradlew` (Gradle wrapper). El README dice que soporta ambos. |
| **clone_repos.sh** | Pinnea spring-petclinic a rama `main` (no SHA), usa `pin_repo` con ref_type vacío (auto-detect) |

### Decisión

**Instalar Maven en la imagen Java derivada y migrar `java_repos.yaml` a comandos `mvn`.**

### Justificación

1. **Consistencia documental**: ADR-032, RELEASE-1.0.0-PLAN y SETUP_REQUIREMENTS.md —tres documentos de autoridad— coinciden en Maven. Mantener Gradle requeriría enmendar los tres, mientras que instalar Maven solo requiere un paso de build.
2. **El repo lo soporta nativamente**: spring-petclinic tiene `pom.xml` + `mvnw` (Maven wrapper). `mvn compile` y `mvn test` funcionan sin cambios en el repo.
3. **Cierra el gate "Maven MISSING"**: SETUP_REQUIREMENTS.md marca explícitamente Maven como bloqueante. Instalarlo resuelve ese gate sin ambigüedad.
4. **Costo bajo**: eclipse-temurin:17-jammy no trae Maven preinstalado, pero se puede derivar con `apt-get install -y maven` (~30 MB adicionales). Alternativa: usar `mvnw` (Maven wrapper) que YA existe en el repo — cero instalación adicional, solo cambiar el comando en el manifiesto de `./gradlew` a `./mvnw`.
5. **Recomendación táctica**: usar `./mvnw` (Maven wrapper incluido en el repo) en lugar de instalar Maven en la imagen. Esto evita el build de imagen derivada y mantiene la compatibilidad exacta con la versión de Maven que el proyecto spring-petclinic espera. Si `mvnw` no funciona en el contenedor (requiere Java en PATH, que eclipse-temurin ya proporciona), se cae al plan B: derivar imagen con `apt-get install -y maven`.

**No se requiere ADR nuevo** — es una corrección de implementación que alinea el manifiesto con los ADRs existentes. Se actualiza `SETUP_REQUIREMENTS.md` para reflejar Maven ✅ DISPONIBLE vía wrapper.

---

## Capabilities

> CONTRACT con sddk-spec. Investigado `openspec/specs/` — existe `sandbox-validation-system/spec.md`.

### New Capabilities

None — todas las requirements ya están en `sandbox-validation-system`.

### Modified Capabilities

- **`sandbox-validation-system`**: 
  - Escenario "Six language services exist" — clarificar que el conteo es de 6 language `.container` files (rust, python, go, java, js, ts); postgres es infraestructura preexistente no gestionada por este cambio. Si se necesita, refundir js+ts en un solo `cognicode-node` para cuadrar con "rust, python, go, java, node, postgres = 6".
  - Escenario "Every quadlet pins a real digest" — ahora con enforcement real (todos los 6 digests validados con `podman inspect`).
  - Escenario "Containers are isolated" — extender verificación a go.container.

---

## Approach

### Paso 1 — Digests reales (gap 1)
1. `podman pull` cada imagen base: `rust:1.80-slim`, `python:3.12-slim`, `golang:1.23-alpine`, `eclipse-temurin:17-jammy`, `node:22-slim`.
2. `podman inspect --format '{{.Digest}}'` → extraer SHA-256 real.
3. Reescribir `Image=` en los 6 `.container` + `sandbox-pull` en `justfile`.

### Paso 2 — Hardening go.container (gap 2)
4. Cambiar `Network=host` → `Network=none`.
5. Cambiar `AutoUpdate=registry` → `AutoUpdate=no`.
6. Subir `MemoryMax=2g`, `PidsLimit=128` para alinear con spec.

### Paso 3 — Setup unificado (gap 3)
7. Merge `sandbox-setup-js-ts` dentro de `sandbox-setup`: copiar los 6 `.container` (no solo 3) a `~/.config/containers/systemd/`.
8. `systemctl --user start` para los 6 servicios.

### Paso 4 — Maven/Gradle (gap 4)
9. Cambiar `java_repos.yaml`: `./gradlew compileJava -q` → `./mvnw compile -q`; `./gradlew test -q` → `./mvnw test -q`.
10. Verificar que `./mvnw` existe en `sandbox/repos/java/spring-petclinic/` y es ejecutable.
11. Actualizar `SETUP_REQUIREMENTS.md`: Maven pasa de ❌ MISSING a ✅ DISPONIBLE (wrapper).
12. Si `mvnw` falla en contenedor, plan B: derivar `eclipse-temurin:17-jammy` con `apt-get install -y maven`.

### Paso 5 — Re-pin spring-petclinic a SHA (gap 5)
13. Cambiar `clone_repos.sh` línea 192: `"main"` → `"edf4db28affcc4741c79850a3d95bc3f177b5ff9"`.

### Paso 6 — Workflow nightly (gap 6)
14. Crear `.github/workflows/sandbox-nightly.yml` con:
    - Trigger: `schedule: cron(0 3 * * *)` + `workflow_dispatch`.
    - Steps: checkout → setup podman → `just sandbox-pull && just sandbox-setup` → `just sandbox-ci-smoke` → upload artifacts.
    - Si el runner hosted (ubuntu-latest) no soporta rootless podman + systemd, el job se marca como `continue-on-error: true` con un warning documentado. La ejecución real requiere self-hosted runner o migración a Docker.

### Paso 7 — Verificación
15. `just sandbox-ci-smoke` → iterar hasta exit 0.
16. `systemctl --user is-active cognicode-{rust,python,go,java,js,ts}` → todos deben reportar `active`.

---

## Affected Areas

| Área | Impacto | Descripción |
|------|---------|-------------|
| `sandbox/containers/rust.container` | Modified | Digest real |
| `sandbox/containers/python.container` | Modified | Digest real |
| `sandbox/containers/go.container` | Modified | Digest real + hardening (`Network=none`, `AutoUpdate=no`, `MemoryMax=2g`, `PidsLimit=128`) |
| `sandbox/containers/java.container` | Modified | Digest real + posible derivación para Maven |
| `sandbox/containers/js.container` | Modified | Digest real |
| `sandbox/containers/ts.container` | Modified | Digest real |
| `sandbox/justfile` | Modified | `sandbox-pull` (digests reales), `sandbox-setup` (6 containers en vez de 3), eliminar `sandbox-setup-js-ts` standalone |
| `sandbox/manifests/java_repos.yaml` | Modified | `./gradlew` → `./mvnw` (21 líneas de comandos de validación) |
| `sandbox/scripts/clone_repos.sh` | Modified | spring-petclinic: `"main"` → SHA concreto |
| `sandbox/SETUP_REQUIREMENTS.md` | Modified | Maven: ❌ MISSING → ✅ DISPONIBLE (mvnw) |
| `.github/workflows/sandbox-nightly.yml` | **New** | Workflow nocturno + smoke lane |
| `openspec/specs/sandbox-validation-system/spec.md` | Modified | Clarificación escenario "Six language services" |
| `docs/ROADMAP.md` | Modified | Fase 0 status: PROPOSED → IN PROGRESS |

---

## Riesgos

| Riesgo | Probabilidad | Mitigación |
|--------|-------------|------------|
| **CI runner sin podman rootless + systemd** — ubuntu-latest no soporta `systemctl --user` en GitHub Actions | Alta | El workflow nocturno se configura con `continue-on-error: true` inicialmente. Documentar requisito de self-hosted runner con podman. La smoke lane manual (`just sandbox-ci-smoke`) funciona en dev. |
| **`mvnw` no ejecutable en contenedor** — el Maven wrapper requiere Java en PATH y permisos de red para bajar dependencias primera vez | Media | Verificar en contenedor real antes de commit. Plan B: derivar imagen Java con `apt-get install -y maven` (~30 MB, sin dependencia de wrapper). |
| **Digest pins se vuelven obsoletos** — las imágenes upstream se actualizan | Media | `AutoUpdate=no` previene drift automático. Procedimiento de re-pin documentado en ADR-032 (manual, cada ~mes). |
| **Smoke lane falla por producto, no por infra** — `sandbox-ci-smoke` requiere MCP server binary funcional; si el binary tiene bugs, el smoke sale 1 (product-fail) en vez de 0 o 2 (infra-fail) | Media | Documentado en spec: exit 1 = unexpected failure (producto), exit 2 = infra failure. La verificación de Fase 0 acepta exit 1 documentado como "infra verde, producto pendiente". |
| **Confusión "6 servicios activos"** — spec dice "rust, python, go, java, node, postgres" pero hay 7 contenedores (js y ts separados) | Baja | Propuesta: interpretar como "6 language containers desplegados y activos". Clarificar en spec. Si se requiere cuadrar exactamente con spec, refundir js+ts en un solo `cognicode-node`. |

---

## Rollback Plan

1. `git revert` del commit que introduce los cambios.
2. Restaurar `.container` files desde `main` (los digests falsos son funcionalmente equivalentes para desarrollo — no rompen nada que ya no estuviera roto).
3. `sandbox-nightly.yml` se desactiva con `git revert` o eliminando el archivo.
4. `java_repos.yaml` revierte a `./gradlew` si `mvnw` resultó inviable.
5. Los contenedores desplegados en `~/.config/containers/systemd/` no se tocan (son estado local del host, no del repo).

---

## Dependencias

- **Podman 5.8.4+ funcional** en el entorno de desarrollo (ya verificado: `cognicode-postgres` corriendo).
- **Acceso a Docker Hub** para `podman pull` de las 5 imágenes base (rust, python, golang, eclipse-temurin, node).
- **`sandbox-orchestrator` compila** (ya verificado: compila en `main`).
- **`cognicode-mcp` binary compila** (necesario para smoke lane; ya compila en `main`).

---

## Success Criteria

- [ ] `just sandbox-ci-smoke` → exit 0 (o exit 1 documentado como product-fail, no infra-fail)
- [ ] `systemctl --user is-active cognicode-{rust,python,go,java,js,ts}` → 6/6 `active`
- [ ] Los 6 `.container` tienen `Image=...@sha256:<64-hex-chars>` válido (verificable con `podman inspect`)
- [ ] `go.container`: `Network=none` y `AutoUpdate=no` (ya no `host` ni `registry`)
- [ ] `java_repos.yaml` usa `./mvnw` (no `./gradlew`)
- [ ] `clone_repos.sh` pinnea spring-petclinic a SHA concreto (no a `main`)
- [ ] `.github/workflows/sandbox-nightly.yml` existe con schedule diario
- [ ] `SETUP_REQUIREMENTS.md` muestra Maven ✅ DISPONIBLE (wrapper)
