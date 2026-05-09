# Sandbox Isolation Architecture

## Problema

El sistema actual edita `catalog.rs` directamente en el proyecto principal:

```
❌ run_once.py → edita catalog.rs → cargo check → (peligro!)
   ├── Corrompe el workspace de desarrollo
   ├── Sin隔离 entre experimentos
   ├── Git history se contamina con intentos fallidos
   └── Imposible ejecutar múltiples agentes en paralelo
```

## Solución: Contenedores Efímeros

Cada experimento se ejecuta en un **contenedor Docker aislado** que:
- Clona el repo en un filesystem temporal
- Aplica el cambio experimental
- Ejecuta compilación + tests + sandbox
- Reporta resultados al host
- Se destruye al terminar (sin dejar rastro)

```
┌──────────────────────────────────────────────────────────────────┐
│                        HOST (tu máquina)                          │
│                                                                   │
│  autoresearch/                                                    │
│  ├── orchestrator/    ← LangGraph loop (Python)                   │
│  ├── evolution.tsv    ← Log persistente                           │
│  └── baseline/        ← Métricas baseline                         │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │              DOCKER SANDBOX (efímero)                     │     │
│  │                                                          │     │
│  │  /workspace/                                             │     │
│  │  ├── CogniCode/        ← clon fresco del repo            │     │
│  │  │   ├── catalog.rs    ← editado por el agente           │     │
│  │  │   └── ...                                            │     │
│  │  ├── corpus/           ← volumen mount (solo lectura)     │     │
│  │  └── results/          ← volumen mount (escritura)        │     │
│  │                                                          │     │
│  │  Ejecuta: cargo check → cargo test → sandbox eval        │     │
│  │  Reporta: JSON con métricas → host                       │     │
│  │                                                          │     │
│  │  DESTRUIDO al terminar ✅                                 │     │
│  └─────────────────────────────────────────────────────────┘     │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │              DOCKER SANDBOX (agente 2)                    │     │
│  │  ... (misma estructura, regla diferente)                  │     │
│  └─────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────┘
```

## Dockerfile

```dockerfile
# autoresearch/sandbox/Dockerfile
FROM rust:1.78-slim-bookworm

# System dependencies
RUN apt-get update && apt-get install -y \
    git curl build-essential pkg-config libssl-dev \
    python3 python3-pip nodejs npm \
    && rm -rf /var/lib/apt/lists/*

# Rust tools
RUN rustup component add clippy rustfmt
RUN cargo install cargo-audit

# Node tools (for ESLint)
RUN npm install -g eslint

# Python tools
RUN pip3 install ruff pylint

# Java tools (optional, heavy)
# RUN apt-get install -y openjdk-17-jdk

# SonarQube Scanner
RUN curl -sSLo /tmp/sonar-scanner.zip \
    https://binaries.sonarsource.com/Distribution/sonar-scanner-cli/sonar-scanner-cli-5.0.1.3006-linux.zip \
    && unzip /tmp/sonar-scanner.zip -d /opt \
    && ln -s /opt/sonar-scanner-*/bin/sonar-scanner /usr/local/bin/sonar-scanner \
    && rm /tmp/sonar-scanner.zip

WORKDIR /workspace

# Entry point: the experiment script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
```

## Entrypoint Script

```bash
#!/bin/bash
# autoresearch/sandbox/entrypoint.sh
# Runs inside the Docker container. Receives:
#   $1 = RULE_ID (e.g., "S134")
#   $2 = GIT_REF (commit/branch to clone)
#   $3 = CHANGE_SCRIPT (path to script that edits catalog.rs)

set -e

RULE_ID="$1"
GIT_REF="${2:-main}"
CHANGE_SCRIPT="$3"

echo "=== SANDBOX: Experiment for rule $RULE_ID ==="
echo "Git ref: $GIT_REF"

# 1. Clone fresh repo
echo "[1/6] Cloning repository..."
git clone --depth 1 --branch "$GIT_REF" \
    /host-repo /workspace/CogniCode 2>/dev/null || \
    git clone --depth 1 /host-repo /workspace/CogniCode

cd /workspace/CogniCode

# 2. Apply change
echo "[2/6] Applying experimental change..."
if [ -f "$CHANGE_SCRIPT" ]; then
    python3 "$CHANGE_SCRIPT" || {
        echo '{"status":"failed","reason":"change_script_failed"}'
        exit 1
    }
fi

# 3. Compilation check
echo "[3/6] Checking compilation..."
if ! cargo check -p cognicode-axiom 2>&1 | tee /tmp/check.log; then
    echo '{"status":"failed","reason":"compilation_error"}' 
    exit 1
fi

# 4. Run tests
echo "[4/6] Running tests..."
if ! cargo test --workspace 2>&1 | tee /tmp/test.log; then
    FAILED=$(grep -c "FAILED" /tmp/test.log || echo "?")
    echo "{\"status\":\"failed\",\"reason\":\"tests_failed\",\"failed\":$FAILED}"
    exit 1
fi

# 5. Build release
echo "[5/6] Building release..."
cargo build --release -p sandbox-orchestrator 2>&1

# 6. Run evaluation
echo "[6/6] Running sandbox evaluation..."
./target/release/sandbox-orchestrator run \
    sandbox/manifests/rust_fixture.yaml \
    --results-dir /results \
    --jsonl \
    --filter-rule "$RULE_ID" 2>&1

# 7. Report success
echo "{\"status\":\"success\",\"rule_id\":\"$RULE_ID\"}"
```

## Sandbox Manager (Python)

```python
# autoresearch/sandbox/manager.py
"""Manages Docker sandboxes for isolated rule experiments."""

import subprocess
import json
import tempfile
from pathlib import Path
from typing import Optional, Dict
import logging

logger = logging.getLogger(__name__)

REPO_ROOT = Path(__file__).parent.parent.parent
SANDBOX_DIR = Path(__file__).parent
IMAGE_NAME = "cognicode-sandbox:latest"


class SandboxManager:
    """Creates and manages ephemeral Docker containers for experiments."""
    
    def __init__(self):
        self._ensure_image()
    
    def _ensure_image(self):
        """Build sandbox image if not exists."""
        result = subprocess.run(
            ["docker", "images", "-q", IMAGE_NAME],
            capture_output=True, text=True
        )
        if not result.stdout.strip():
            logger.info("Building sandbox Docker image...")
            subprocess.run(
                ["docker", "build", "-t", IMAGE_NAME, str(SANDBOX_DIR)],
                check=True
            )
            logger.info("Sandbox image built.")
    
    def run_rule_eval(self, rule_id: str, git_ref: str = "main") -> Dict:
        """Run a single rule evaluation in an isolated container.
        
        Args:
            rule_id: Rule to evaluate (e.g., "S134")
            git_ref: Git branch/commit to clone
            
        Returns:
            Dict with status, metrics, and any errors
        """
        container_name = f"cognicode-eval-{rule_id}-{_short_hash()}"
        
        logger.info(f"Starting sandbox container: {container_name}")
        
        try:
            result = subprocess.run([
                "docker", "run",
                "--rm",                          # Auto-remove on exit
                "--name", container_name,
                "--cpus", "4",                   # Limit CPU
                "--memory", "4g",                # Limit RAM
                "--network", "none",             # No network access
                "-v", f"{REPO_ROOT}:/host-repo:ro",  # Read-only repo
                "-v", f"{REPO_ROOT}/autoresearch/results:/results",  # Write results
                "-v", f"{REPO_ROOT}/autoresearch/corpus:/corpus:ro", # Read-only corpus
                IMAGE_NAME,
                rule_id,
                git_ref,
                "/tmp/change.py"                 # Placeholder for Phase 2
            ], capture_output=True, text=True, timeout=600)
            
            # Parse JSON result
            output = result.stdout.strip()
            for line in output.split("\n"):
                if line.startswith("{") and "status" in line:
                    return json.loads(line)
            
            return {"status": "failed", "reason": "no_json_output", 
                    "stdout": output[:500], "stderr": result.stderr[:500]}
                    
        except subprocess.TimeoutExpired:
            self._kill_container(container_name)
            return {"status": "timeout", "reason": "evaluation_exceeded_10_minutes"}
        except Exception as e:
            self._kill_container(container_name)
            return {"status": "failed", "reason": str(e)}
    
    def _kill_container(self, name: str):
        """Force-kill a container."""
        subprocess.run(["docker", "kill", name], 
                       capture_output=True)  # Ignore errors if already dead


def _short_hash() -> str:
    import hashlib, time
    return hashlib.md5(str(time.time()).encode()).hexdigest()[:8]
```

## Integración con el Loop

```python
# En run_once.py — el loop usa SandboxManager en lugar de editar directamente

from sandbox.manager import SandboxManager

def evaluate_isolated(rule_id: str, change: dict) -> dict:
    """Run evaluation in isolated sandbox."""
    sandbox = SandboxManager()
    
    # Write change script to temp file
    import tempfile
    with tempfile.NamedTemporaryFile(mode='w', suffix='.py', delete=False) as f:
        f.write(f'''
# Auto-generated change script for {rule_id}
import re
from pathlib import Path

catalog = Path("/workspace/CogniCode/crates/cognicode-axiom/src/rules/catalog.rs")
content = catalog.read_text()

# Apply change: {change['description']}
# ... (specific edit logic)
''')
        change_script = f.name
    
    try:
        result = sandbox.run_rule_eval(rule_id, git_ref="main")
        return result
    finally:
        Path(change_script).unlink()
```

## Multi-Agente Paralelo

```python
# Cada agente en su propio contenedor → sin conflictos

from concurrent.futures import ThreadPoolExecutor, as_completed

def run_parallel_experiments(rules: list[str], max_workers: int = 3):
    """Run multiple rule evaluations in parallel sandboxes."""
    sandbox = SandboxManager()
    results = {}
    
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = {
            executor.submit(sandbox.run_rule_eval, rule_id): rule_id
            for rule_id in rules
        }
        
        for future in as_completed(futures):
            rule_id = futures[future]
            try:
                results[rule_id] = future.result()
                logger.info(f"  {rule_id}: {results[rule_id]['status']}")
            except Exception as e:
                results[rule_id] = {"status": "failed", "reason": str(e)}
    
    return results
```

## Volúmenes y Persistencia

```
Host                          Container
────                          ─────────
CogniCode/  ──── ro ─────▶   /host-repo (solo lectura)
                                   │
                                   ▼ clone fresco
                              /workspace/CogniCode (lectura/escritura)
                                   │
                                   ▼ edita catalog.rs
                              
autoresearch/results/ ◀── rw ── /results (resultados JSONL)
autoresearch/corpus/  ──── ro ──▶ /corpus (corpus OSS)
autoresearch/evolution.tsv ←── el host escribe después
```

## Ventajas del Aislamiento

| Sin Sandbox | Con Sandbox |
|-------------|-------------|
| ❌ Edita catalog.rs del proyecto real | ✅ Edita copia efímera en contenedor |
| ❌ Si falla, contamina git history | ✅ Contenedor se destruye, zero rastro |
| ❌ 1 agente a la vez (conflictos de archivo) | ✅ N agentes en paralelo |
| ❌ Depende del entorno local | ✅ Entorno Docker reproducible |
| ❌ Sin límites de recursos | ✅ CPU/RAM/network limits |
| ❌ Herramientas externas pueden faltar | ✅ Todas pre-instaladas en la imagen |

## Roadmap de Implementación

```
1. Dockerfile + entrypoint.sh        ← 30 min
2. SandboxManager (Python)           ← 1h
3. Integrar con run_once.py          ← 1h
4. Test: 1 regla en sandbox          ← 30 min
5. Test: 3 reglas en paralelo        ← 30 min
6. CI/CD: GitHub Actions con Docker  ← (Phase 4)
```

## Comparativa: Docker vs DevContainer vs Bastion

| Criterio | Docker Raw | DevContainer | Bastion |
|----------|-----------|--------------|---------|
| **Headless automation** | ✅ Nativo | ⚠️ Necesita VS Code | ✅ |
| **Multi-agente paralelo** | ✅ `docker run` N veces | ❌ 1 contenedor interactivo | ✅ |
| **Debug interactivo** | ❌ Solo logs | ✅ IDE completo | 🟡 SSH |
| **Reproducibilidad** | ✅ Dockerfile | ✅ Dockerfile + features | ✅ |
| **Arranque rápido** | ✅ < 5s | 🟡 15-30s (VS Code server) | 🟡 |
| **Limpieza automática** | ✅ `--rm` | ❌ Manual | ❌ |
| **Sin dependencia IDE** | ✅ Solo Docker | ❌ Necesita VS Code | ✅ |
| **Límites recursos** | ✅ CPU/RAM/network | 🟡 Config manual | 🟡 |
| **Coste** | Gratis | Gratis | 💰 Cloud |

### Veredicto

```
┌──────────────────────────────────────────────────────────────┐
│                 ESTRATEGIA HÍBRIDA                            │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  DOCKER RAW (modo principal)                                  │
│  ├── Headless automation: N contenedores en paralelo         │
│  ├── CI/CD: GitHub Actions con Docker                        │
│  └── Sin dependencias, rápido, limpio                         │
│                                                               │
│  DEVCONTAINER (modo desarrollo)                               │
│  ├── Debug interactivo del sandbox                           │
│  ├── Probar nuevas herramientas antes de añadir al Dockerfile │
│  └── Desarrollo del propio sistema de sandbox                │
│                                                               │
│  BASTION (futuro, si escala)                                  │
│  ├── Múltiples máquinas para paralelismo masivo              │
│  ├── GPUs para análisis pesado                               │
│  └── Entorno cloud production-grade                          │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

**Decisión**: Docker Raw para automatización + DevContainer para desarrollo. Bastion solo si escalamos a cloud.

### Flujo dual

```
┌──────────────────────────────────────────────────────────────┐
│  MODO HEADLESS (Docker Raw)                                  │
│                                                               │
│  $ python run_once.py --rule S134                            │
│    → SandboxManager.run_experiment()                         │
│    → docker run --rm cognicode-sandbox S134                  │
│    → JSON result                                             │
│    → evolution.tsv                                           │
│                                                               │
├──────────────────────────────────────────────────────────────┤
│  MODO DEV (DevContainer)                                     │
│                                                               │
│  $ code .                                                    │
│    → "Reopen in Container"                                   │
│    → VS Code + rust-analyzer + ESLint                        │
│    → Editar Dockerfile, entrypoint.sh, manager.py            │
│    → Probar cambios interactivamente                         │
│    → Commit → usar en modo headless                          │
└──────────────────────────────────────────────────────────────┘
```
