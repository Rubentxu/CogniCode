# PRF-ANA-08 — UAT binario real: budgets y salida acotada

Fecha: 2026-09-22

`prf_ana_08_uat` (1/1) sobre el corpus de 51 homónimos:
- build_graph dentro del budget graph (60 s)
- find_usages ("compute", homónimo) dentro del budget search (500 ms +
slack de pipe), salida < 5 MiB
- símbolo inexistente → respuesta tipada inmediata, sin hang

Infraestructura existente verificada: timeout_for_category
(graph 60 s / navigation 45 s / search 500 ms / default 30 s) con tests
unitarios propios; validate_result_count (max_results 10000).

Sin defectos de producto.
