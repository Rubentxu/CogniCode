# Design: e30-sandbox-infra — Reparación de infraestructura del sandbox

## Technical Approach

Este diseño implementa las 6 acciones de reparación identificadas en el proposal para dejar el sandbox funcional, reproducible y automatizado. La estrategia es incremental sobre lo existente: quadlets ya endurecidos (rust, python, java) se mantienen como referencia; los defectuosos (go, js, ts) se completan con el mismo patrón. Tres decisiones arquitectónicas dominan: formato de digest pinning, reconciliación de Maven con `Network=none`, y la estrategia de setup unificado.

## Architecture Decisions

### Decision 1: Digest Pinning Procedure

**Choice**: Digests SHA-256 obtenidos via `podman pull` + `podman inspect --format '{{.ImageDigest}}'` (no skopeo). Prefix en quadlets: `docker.io/library/<image>:<tag>@sha256:<64-hex>`.

**Alternatives considered**:
- `skopeo inspect` — rechazado porque requiere registry authentication para docker.io y añade dependencia adicional. `podman inspect` ya funciona sin auth para imágenes públicas.
- `:latest` tag sin digest — rechazado. ADR-032 exige `AutoUpdate=no` + digest pinneado para reproducibilidad. Tag flotante viola el gate G9.

**Rationale**: `podman pull` garantiza que la imagen existe y es accesible antes de extraer el digest. `podman inspect` sobre la imagen ya en local devuelve `ImageDigest` (no `Digest` que es otra cosa). El formato quadlet `Image=` acepta `registry/path/image:tag@sha256:...`.

**Procedimiento exacto** (documentado en cada `.container` actualizado):

```bash
# Para cada imagen: rust, python, golang, eclipse-temurin, node
podman pull docker.io/library/<image>:<tag>
podman inspect docker.io/library/<image>:<tag> --format '{{.ImageDigest}}'
# Resultado: sha256:<64-hex-chars>
# Editar el .container: Image=docker.io/library/<image>:<tag>@sha256:<64-hex>
```

Digests reales a obtener (orden recomendado para并行 pull):

| Imagen | Tag | Justfile `sandbox-pull` line |
|--------|-----|------------------------------|
| `docker.io/library/rust` | `1.80-slim` | L45 — fake `e5e5...` → real |
| `docker.io/library/python` | `3.12-slim` | L47 — fake `e5e5...` → real |
| `docker.io/library/eclipse-temurin` | `17-jammy` | L49 — fake `e5e5...` → real |
| `docker.io/library/golang` | `1.23-alpine` | L53 — floating tag → real digest |
| `docker.io/library/node` | `22-slim` | L51 — floating tag → real digest (js + ts comparten imagen) |

---

### Decision 2: go.container Hardening — valores exactos

**Choice**: Alineación total con el spec ADR-032 y el patrón existente en rust.container.

**Hardening completo go.container** (reemplazar contenidos actual):

```ini
[Container]
Image=docker.io/library/golang:1.23-alpine@sha256:<real-digest>   # ← ADD DIGEST PIN
ContainerName=cognicode-go
Volume=%t/containers/cognicode-go-workspace:/workspace:z
Volume=%h/Proyectos/rust/CogniCode/sandbox/repos:/repos:z
Environment=COGNICODE_WORKSPACE=/workspace
Network=none                    # ← CAMBIO: host → none
MemoryMax=2g                    # ← CAMBIO: 1g → 2g (spec ADR-032)
MemorySwap=2g                  # ← AÑADIR: swap igual a MemoryMax
PidsLimit=128                  # ← CAMBIO: 64 → 128
CPUWeight=50
ReadOnly=yes
Tmpfs=/tmp:rw,noexec,nosuid,size=64m
NoNewPrivileges=yes
SupplementaryGroups=

[Service]
Restart=on-failure
AutoUpdate=no                   # ← CAMBIO: registry → no
TimeoutStopSec=30

[Install]
WantedBy=default.target
```

**Note**: `golang:1.23-alpine` no trae `skopeo` ni `sha256sum` pre-instalados en todas las variantes. Si se necesita verificar firmas, la imagen alpine slim es preferible a la estándar alpine.

---

### Decision 3: Setup Unificado — merge sandbox-setup + sandbox-setup-js-ts

**Choice**: Merge completo de `sandbox-setup-js-ts` dentro de `sandbox-setup`. Una sola recipe expone todos los 6 containers. No se usa `podman generate systemd` — se mantienen plantillas estáticas en `sandbox/containers/`.

**Rationale**:
- `podman generate systemd` requiere que el container esté corriendo para generar la unit, lo cual es circular para un workflow de setup idempotente. Las plantillas estáticas son preferibles porque son versionables, diffables, y predecibles.
- La separate recipe `sandbox-setup-js-ts` existe por razones históricas (Phase 2 provisional). Unificarla reduce la superficie de drift entre recipes.

**recipe unificada**:

```just
sandbox-setup: sandbox-pull
    bash {{justfile_directory()}}/scripts/clone_repos.sh
    # Copy ALL 6 containers (not just 3)
    cp {{justfile_directory()}}/containers/rust.container \
       {{justfile_directory()}}/containers/python.container \
       {{justfile_directory()}}/containers/java.container \
       {{justfile_directory()}}/containers/go.container \
       {{justfile_directory()}}/containers/js.container \
       {{justfile_directory()}}/containers/ts.container \
       ~/.config/containers/systemd/ 2>/dev/null || true
    systemctl --user daemon-reload 2>/dev/null || true
    systemctl --user start \
        cognicode-rust \
        cognicode-python \
        cognicode-java \
        cognicode-go \
        cognicode-js \
        cognicode-ts 2>/dev/null || true
```

`sandbox-setup-js-ts` se deprecia (no se elimina, se marca como alias of sandbox-setup en comentario).

---

### Decision 4: Maven — Network=none vs mvnw download dependencies (CRÍTICA)

**Choice**: Estrategia híbrida — `./mvnw` en el manifest (zero installation), pero el container java requiere **network para la fase de build inicial**. Se resuelve con dos variantes de container java.

**Rationale**: La decisión必须在 spec (`Network=none` para aislamiento) y la realidad de Maven wrapper (necesita descargar deps la primera vez) reconciliar. Las opciones son:

| Opción | Ventaja | Desventaja |
|--------|---------|------------|
| (a) Imagen con deps pre-cacheadas | `Network=none` se mantiene | Imagen huge (~500MB+), cache se skew obsolete |
| (b) `Network=none` solo en runtime, build tiene red | Aislamiento real en prod | El orchestrator corre el build dentro del container — no hay fase de build separada |
| (c) Cache volume (`Volume=`) para `~/.m2/repository` | `Network=none` + deps cacheadas | Requiere pre-populate del volume antes del primer run |
| (d) `Network=slirp4netns` o `Network=pasta` | Red aislada pero funcional | Configuración adicional compleja, no todas las distros lo soportan |

**Decisión adoptada — Opción (c) cache volume**:

El `java.container` ya monta `%t/containers/cognicode-java-workspace:/workspace:z`. Se añade un cache volume persistente para maven local:

```ini
# En java.container — AÑADIR:
Volume=%t/containers/cognicode-java-m2-cache:/root/.m2/repository:z
```

Flujo:
1. **Primera vez** (sin cache): el container tiene `Network=none`. `./mvnw compile` falla si es la primera vez — se documenta como "cold start requires network". El usuario ejecuta `just sandbox-maven-warmup` (recipe nueva que hace un `./mvnw compile` con network temporal).
2. **Runs subsiguientes**: el volume `~/.m2/repository` tiene deps cacheadas. `./mvnw compile` funciona offline.
3. **El manifest `java_repos.yaml`** usa `./mvnw` (no `mvn` global).

**Recipe `sandbox-maven-warmup`** (nueva):

```just
# Pre-populate Maven cache for spring-petclinic (offline-capable after first run)
sandbox-maven-warmup:
    podman run --rm \
        --network=host \
        -v %t/containers/cognicode-java-m2-cache:/root/.m2/repository:z \
        docker.io/library/eclipse-temurin:17-jammy@sha256:<digest> \
        bash -c 'cd /tmp && git clone --depth1 https://github.com/spring-projects/spring-petclinic.git && cd spring-petclinic && ./mvnw compile -q -DskipTests'
    @echo "Maven cache warmed — subsequent runs work with Network=none"
```

**Manifest `java_repos.yaml`**: cambiar `./gradlew` → `./mvnw`:

```yaml
validation:
  stages:
    - name: build
      commands: ["./mvnw compile -q"]       # era ./gradlew compileJava -q
    - name: test
      commands: ["./mvnw test -q"]          # era ./gradlew test -q
```

**Decisión ADR**: NO se necesita ADR nuevo. El ADR-032 dice "Maven" y la proposal ya decide usar `mvnw`. Esta es una corrección de implementación que alinea el manifiesto con la arquitectura vigente.

---

### Decision 5: sandbox-nightly.yml Workflow

**Choice**: Workflow dedicado con 2 lanes (smoke + probe), `continue-on-error` para la smoke lane sobre ubuntu-latest (donde podman+systemd user puede no estar disponible), schedule `cron(0 3 * * *)`.

**Rationale**: El CI existente (`ci.yml`) corre en ubuntu-latest que tiene Docker pero no rootless podman con systemd user units. El workflow debe ser honesto sobre esta limitación y permitir ejecución manual via `workflow_dispatch`.

**Diseño**:

```yaml
name: Sandbox Nightly

on:
  schedule:
    - cron: '0 3 * * *'   # 03:00 UTC cada día
  workflow_dispatch:
    inputs:
      lane:
        description: 'Lane to run'
        required: true
        default: 'smoke'
        type: choice
        options: ['smoke', 'probe', 'full']

env:
  CARGO_TERM_COLOR: always
  JUST_ARGS: '--unstable'

jobs:
  sandbox-smoke:
    name: Sandbox Smoke Lane
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      - name: Setup Podman
        uses: rootful/setup-podman@v4
        with:
          podman-version: 5.2.0
      - name: Pull images
        run: just sandbox-pull || true
      - name: Setup containers
        run: just sandbox-setup || true
      - name: Run smoke lane
        run: just sandbox-ci-smoke
        continue-on-error: true
      - name: Upload artifacts
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: smoke-results
          path: sandbox/results/
          retention-days: 7

  sandbox-probe:
    name: Sandbox Probe Lane
    runs-on: ubuntu-latest
    needs: sandbox-smoke
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      - name: Setup Podman
        uses: rootful/setup-podman@v4
        with:
          podman-version: 5.2.0
      - name: Run probe lane
        run: just sandbox-ci-probe
        continue-on-error: true
      - name: Upload artifacts
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: probe-results
          path: sandbox/results/probe/
          retention-days: 7
```

**Notas técnicas**:
- `rootful/setup-podman@v4` usa Podman en modo rootful (no rootless). Para producción se necesita self-hosted runner con podman socket.
- `continue-on-error: true` en ambos jobs permite que el workflow complete aunque el sandbox no esté desplegable en el runner.

---

## Data Flow

```
just sandbox-pull → podman pull (6 imágenes) → local store
just sandbox-setup → clone_repos.sh + cp 6× .container → systemctl start 6 servicios
just sandbox-ci-smoke → build-orchestrator + orchestrator run → orchestrator report → artifacts
just sandbox-maven-warmup → podman run --network=host → pre-populate ~/.m2/repository
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `sandbox/containers/rust.container` | Modify | Digest real |
| `sandbox/containers/python.container` | Modify | Digest real |
| `sandbox/containers/java.container` | Modify | Digest real + m2-cache volume |
| `sandbox/containers/go.container` | Modify | Digest real + full hardening |
| `sandbox/containers/js.container` | Modify | Digest real |
| `sandbox/containers/ts.container` | Modify | Digest real |
| `sandbox/justfile` | Modify | sandbox-pull digests, sandbox-setup 6 containers, sandbox-maven-warmup |
| `sandbox/manifests/java_repos.yaml` | Modify | `./gradlew` → `./mvnw` |
| `sandbox/scripts/clone_repos.sh` | Modify | spring-petclinic SHA pin |
| `sandbox/SETUP_REQUIREMENTS.md` | Modify | Maven → disponible |
| `.github/workflows/sandbox-nightly.yml` | Create | smoke + probe lanes |

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | Digest format validation | `grep -E 'sha256:[a-f0-9]{64}' sandbox/containers/*.container` |
| Integration | 6 servicios activos | `systemctl --user is-active cognicode-{rust,python,go,java,js,ts}` |
| E2E | `just sandbox-ci-smoke` | Exit code 0 (pass) o 1 (product fail, acceptable si infra OK) |

## Standard Envelope

```yaml
status: success
artifacts:
  - "sddk/e30-sandbox-infra/design.md"
next_recommended: "sddk-tasks"
```
