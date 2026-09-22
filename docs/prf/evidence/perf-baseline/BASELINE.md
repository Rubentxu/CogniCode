# PRF-CI-04 — Baseline de rendimiento (presupuesto) — 2026-09-22

Revisión: HEAD `69481cf2` (main local), binario release local
(cognicode-mcp 0.97.3). Máquina: Fedora linux-x86-64, 8MB rayon stack,
podman presente. Correlación con CI: estos números fijan el presupuesto
INICIAL; las comparaciones futuras se hacen contra este documento.

## Presupuesto observado (OBSERVED, una corrida c/u — no medias)

| Métrica | Valor | Cómo |
|---|---|---|
| Startup del servidor MCP (init hasta listo) | ~9.9–10.2 s | dominante en toda operación puntual (get_file_symbols total wall 10.15 s, incl. startup) |
| `build_graph` full del repo (53066 símbolos / 19435 aristas) | 9.98 s | reportado por el propio servidor (analyze_impact summary) |
| `analyze_impact` (sobre grafo recién auto-construido) | 10.01 s | idem |
| RSS máximo del servidor (build_graph full + idle) | 670064 KB (~654 MB) | `/usr/bin/time -v` |
| `get_complexity` sobre archivo grande (lifecycle.rs) | < 1 s tras startup | U05 session |

## Presupuesto de regresión (freeze inicial)

- `build_graph` full repo: ≤ 15 s (x1.5 del observado).
- `analyze_impact` tras grafo: ≤ 15 s.
- RSS en build full: ≤ 1 GB.
- Startup MCP: ≤ 15 s.
- `get_file_symbols` archivo grande: ≤ 2 s excluyendo startup.

Cualquier PRF que mueva estos caminos debe comparar contra este baseline
y justificar excedencias (o registrar deuda honesta).

## Limitaciones honestas
- Una sola corrida por métrica (máquina con carga del entorno de agentes);
  sin réplicas ni varianza. El presupuesto se corrige cuando exista
  runner de CI estable (U03/G6).
- `elapsed_ms` no expuesto por el servidor para todas las herramientas;
  mediciones de pared incluyen startup.
