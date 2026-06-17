# Plan: Sandbox Testing Strategy for CogniCode MCP

> **Status**: DRAFT — pending research delegation results
> **Date**: 2026-06-17
> **Goal**: Validate all 64 MCP tools with automated, measurable, reportable tests

---

## 1. Problem Statement

Tenemos 64 tools MCP con capabilities nuevas (type-refs en 11 lenguajes, IaC extraction, graph checkpointing, auth middleware, Prometheus metrics, file watcher, rate limiting, timeouts) pero **no las hemos probado end-to-end de forma automatizada y medible**.

El smoke test existente (`mcp_smoke_all.py`) clasifica OK/STUB/GATED pero no mide **calidad** — solo verifica que no crashea.

## 2. Sandbox Architecture

```
┌─────────────────────────────────────────────────────┐
│                  TEST HARNESS                         │
│  (orchestrates lifecycle, collects KPIs, reports)     │
└──────────┬──────────────────────┬────────────────────┘
           │                      │
     ┌─────▼─────┐         ┌─────▼─────┐
     │ MCP Server │         │ PostgreSQL │
     │ (HTTP/SSE) │◄───────►│ (sandbox)  │
     │ :9847      │         │ :5432      │
     └─────┬─────┘         └───────────┘
           │
     ┌─────▼─────┐
     │  FIXTURE   │
     │ WORKSPACE  │
     │ (multi-lang│
     │  known     │
     │  graph)    │
     └───────────┘
```

### Componentes del Sandbox

| Componente | Propósito | Estado actual |
|-----------|-----------|---------------|
| **PG Sandbox** | PostgreSQL aislado con schema nuevo (graph_nodes, graph_edges, scan_manifest) | ⚠️ Corriendo pero schema legacy |
| **MCP Server** | cognicode-mcp-server HTTP/SSE en :9847 | ✅ Compila y arranca |
| **Fixture Workspace** | Código controlado multi-lenguaje con grafo conocido | ⬜ No existe |
| **Test Harness** | Orquesta: build_graph → tools/list → tools/call → verify → KPI | ⬜ Parcial (mcp_smoke_all.py) |
| **KPI Collector** | Recopila métricas por tool desde /metrics + tracing logs | ⬜ No existe |
| **Report Generator** | Produce reporte HTML/JSON con resultados | ⬜ Parcial (smoke baseline JSON) |

## 3. Setup del Sandbox — Paso a Paso

### Paso 1: PG Fresh con Schema Nuevo

```bash
# 1. Parar el container actual
systemctl --user stop cognicode-postgres.container

# 2. Borrar volumen (datos legacy)
podman volume rm cognicode-pgdata

# 3. Arrancar fresh
systemctl --user start cognicode-postgres.container

# 4. Correr migraciones (cognicode-core las tiene)
cargo run -p cognicode-mcp -- --postgres "postgres://cognicode:cognicode@127.0.0.1:5432/cognicode" --cwd /tmp/dummy
# El binary corre migraciones automáticamente al arrancar en Mode B
```

**Verificación**: `SELECT table_name FROM information_schema.tables WHERE table_schema='public'`
Debe mostrar: `graph_nodes`, `graph_edges`, `scan_manifest`, `graph_reports`

### Paso 2: Fixture Workspace Controlado

Crear `tests/fixtures/sandbox-workspace/` con código multi-lenguaje que tiene un **grafo conocido y verificable**:

```
tests/fixtures/sandbox-workspace/
├── src/
│   ├── auth.rs          # UserService, AuthService, User struct
│   ├── handlers.rs      # HTTP handlers calling UserService
│   └── main.rs          # Entry point
├── python/
│   ├── models.py        # User class, Repository class
│   └── services.py      # SaveService(User, Repository)
├── terraform/
│   └── main.tf          # aws_instance.web → aws_security_group.sg
├── ansible/
│   └── site.yml         # playbook: hosts → tasks → apt module
└── README.md
```

**Grafo esperado** (known answer):
- ~15-20 symbols (functions, classes, structs)
- ~10-15 edges (calls, references, contains)
- 2 IaC resources (aws_instance.web, aws_security_group.sg)
- 1 Ansible play with 1 task

**Por qué fixture controlado**: cada tool tiene un "expected output" verificable, no solo "no crashea".

### Paso 3: Test Harness — Capas

#### Layer 1: Smoke Conformance (MCP Protocol)

Verifica que el servidor MCP cumple el protocolo:
```
initialize → tools/list → tools/call(build_graph) → tools/call(each tool)
```

- `tools/list` devuelve exactamente 64 tools (no más, no menos)
- Cada tool tiene `cognicode_meta` con `stability`, `category`, `requires_graph`
- `build_graph` con el fixture workspace devuelve > 0 symbols
- `/ready` cambia de 503 a 200 después de build_graph

#### Layer 2: Functional Correctness (Per-Tool)

Cada tool se llama con input conocido y se verifica el output:

| Tool | Input conocido | Output esperado |
|------|---------------|-----------------|
| `get_call_hierarchy` | `symbol: "UserService"` | > 0 callees |
| `find_usages` | `symbol: "User"` | > 0 usages |
| `get_type_references` | `symbol: "User"` | > 0 references (en Rust) |
| `get_imports` | `file_path: "src/auth.rs"` | > 0 imports |
| `get_members` | `class_name: "UserService"` | methods + fields > 0 |
| `smart_search` | `query: "User"` | results from multiple sources |
| `iac_query` | `resource_id: "aws_instance.web"` | dependencies > 0 |
| `compare_graph` | (sin baseline PG) | GATED error esperado |
| `project_insights` | (default) | health_score > 0 |
| `project_overview` | `detail: "medium"` | architecture_score ≠ null |

#### Layer 3: Performance KPIs

Medidos vía `/metrics` (Prometheus) + timing del harness:

| KPI | Target | Medición |
|-----|--------|----------|
| `build_graph` latency | < 10s (fixture pequeño) | wall clock |
| Tool call latency p50 | < 500ms | `/metrics` histogram |
| Tool call latency p99 | < 5s (graph analytics) | `/metrics` histogram |
| Error rate (stable tools) | < 1% | `/metrics` counter `status=error` |
| `cognicode.tool.calls` after smoke | 64 tools × calls > 0 | `/metrics` counter |
| Graph build symbols | 15-20 (fixture known) | build_graph output |
| Token efficiency | < 2000 tokens per tool response | response size / 4 (approx) |

#### Layer 4: Observability Validation

- `/metrics` contiene `cognicode_tool_calls_total{tool="...",status="ok|error"}`
- `/metrics` contiene `cognicode_graph_symbols`, `cognicode_graph_edges`, `cognicode_graph_health_score`
- Structured log line por tool call (tracing::info con tool, duration_ms, status)
- File watcher emite log cuando un archivo del fixture cambia

#### Layer 5: Security & Operational

- Sin `COGNICODE_MCP_AUTH_TOKEN`: `/mcp` responde sin auth (dev mode)
- Con token: `/mcp` sin header → 401, con header correcto → 200
- `/metrics` siempre público (sin auth)
- `/health` siempre 200, `/ready` 503 antes de build_graph
- Rate limit: 101 calls rápidas a un tool → 1 error de rate limit
- Timeout: tool que excede categoría timeout → error "timeout"

### Paso 4: KPIs Reportables

#### Dashboard KPIs (JSON + HTML)

```json
{
  "timestamp": "2026-06-17T20:00:00Z",
  "workspace": "sandbox-workspace",
  "graph_stats": {
    "symbols": 18,
    "edges": 14,
    "iac_resources": 2,
    "ansible_plays": 1
  },
  "tools": {
    "total": 64,
    "tested": 64,
    "pass": 60,
    "gated_ok": 3,
    "fail": 1,
    "skip": 0
  },
  "performance": {
    "build_graph_ms": 3400,
    "tool_latency_p50_ms": 120,
    "tool_latency_p99_ms": 1800,
    "error_rate_pct": 0.5
  },
  "observability": {
    "metrics_endpoint": true,
    "metrics_instruments_visible": true,
    "structured_logs": true,
    "watcher_active": true
  },
  "by_category": {
    "graph": {"total": 25, "pass": 25, "fail": 0},
    "search": {"total": 5, "pass": 5, "fail": 0},
    "file": {"total": 5, "pass": 5, "fail": 0},
    "composite": {"total": 7, "pass": 7, "fail": 0},
    "quality": {"total": 5, "pass": 5, "fail": 0},
    "refactor": {"total": 1, "pass": 1, "fail": 0},
    "infra": {"total": 1, "pass": 1, "fail": 0},
    "navigation": {"total": 3, "pass": 3, "fail": 0}
  }
}
```

#### KPIs que importan para "tool quality"

1. **Response Completeness**: ¿El tool devuelve datos reales, no vacíos? (% de tools con output > 0)
2. **Latency**: p50 < 500ms, p99 < 5s (excluyendo graph analytics)
3. **Error Rate**: < 1% para stable tools, < 5% para experimental
4. **Honesty**: 0 STUB tools en la surface (todas devuelven data real o GATED claro)
5. **Token Efficiency**: respuesta promedio < 2000 tokens (agentes LLM consumen menos contexto)
6. **Coverage**: 100% de tools probadas, 100% de categorías cubiertas
7. **Observability**: 100% de tools emiten métricas, 100% emiten structured log

## 4. Orquestación

### Script: `scripts/mcp/run_sandbox_tests.sh`

```bash
#!/bin/bash
set -euo pipefail

WORKSPACE="${1:-tests/fixtures/sandbox-workspace}"
PG_URL="postgres://cognicode:cognicode@127.0.0.1:5432/cognicode"
MCP_BIN="target/release/cognicode-mcp-server"
LISTEN="127.0.0.1:9847"

echo "=== 1. Reset PG sandbox ==="
# Fresh PG: drop + recreate tables via migration
$MCP_BIN --listen $LISTEN --postgres "$PG_URL" --cwd "$WORKSPACE" &
MCP_PID=$!
sleep 3

echo "=== 2. Health checks ==="
curl -sf http://$LISTEN/health || { echo "FAIL: /health"; exit 1; }
curl -sf http://$LISTEN/ready | grep -q "not_ready" || { echo "FAIL: /ready pre-build"; exit 1; }

echo "=== 3. Build graph ==="
python3 scripts/mcp/mcp_smoke_all.py --workspace "$WORKSPACE" --persist baseline.json

echo "=== 4. Functional tests ==="
python3 scripts/mcp/mcp_smoke_all.py --workspace "$WORKSPACE" --baseline baseline.json --fail-on-stable-stub

echo "=== 5. KPI collection ==="
curl -s http://$LISTEN/metrics > metrics_output.txt
python3 scripts/mcp/parse_kpis.py metrics_output.txt > kpi_report.json

echo "=== 6. Cleanup ==="
kill $MCP_PID

echo "=== Results ==="
cat kpi_report.json
```

### Script: `scripts/mcp/parse_kpis.py`

Lee `/metrics` (Prometheus text format), extrae KPIs, produce JSON report.

## 5. Ejecución Local

El sandbox se ejecuta localmente con un solo comando:

```bash
bash scripts/mcp/run_sandbox_tests.sh tests/fixtures/sandbox-workspace
```

No depende de ningún servicio externo. Todo es local: PG en container, MCP server en localhost, fixtures en el repo.

## 6. Fixture Workspace — Known Graph

El fixture debe tener un grafo **predecible y verificable**:

### Symbols esperados (Rust):
- `main` (function, entry point)
- `UserService::new` (method)
- `UserService::save` (method, calls Repository)
- `UserService::find` (method)
- `AuthService::authenticate` (method, takes User)
- `User` (struct, fields: id, name, email)
- `Repository` (trait)
- `PgRepository` (struct, implements Repository)

### Edges esperados:
- `main` → calls → `UserService::new`
- `UserService::save` → calls → `Repository::save`
- `AuthService::authenticate` → references → `User`
- `PgRepository` → inherits → `Repository`

### IaC esperado (Terraform):
- `tf:main.tf:aws_instance.web` (resource)
- `tf:main.tf:aws_security_group.sg` (resource)
- `aws_instance.web` → references → `aws_security_group.sg`

### Verificación:
Cada test compara el output del tool contra estos valores conocidos.
Si el tool devuelve algo diferente, es un bug — no un "unknown".

## 7. Pendiente de Research Delegations

Las delegaciones `yappy-aqua-rodent` (industry best practices) y `voiceless-orange-gorilla` (test inventory) siguen corriendo. Cuando completen, se añadirán:

- [ ] Patrones específicos de Sourcegraph Cody / Continue.dev para tool testing
- [ ] MCP conformance suite (si existe estándar)
- [ ] Inventario completo de tests existentes (qué cubren, qué falta)
- [ ] Property-based testing recommendations

## 8. Estimación de Esfuerzo

| Componente | Esfuerzo | Bloqueado por |
|-----------|----------|---------------|
| PG reset + migraciones | 30min | Nada |
| Fixture workspace (multi-lang) | 2-3h | Nada |
| mcp_smoke_all.py upgrade (functional layer) | 3-4h | Fixture |
| parse_kpis.py | 2h | Nada |
| run_sandbox_tests.sh | 1h | Todo lo anterior |
| CI workflow update | 1h | run_sandbox_tests.sh |
| **Total** | **~10h** | |
