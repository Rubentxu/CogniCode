"""Adaptadores: donde el harness toca el mundo.

El núcleo (`src/harness/`) es puro y decide; aquí viven git, cargo, PipelineK, el
ledger y la ejecución de procesos. Cada adaptador expone datos del dominio, no
salidas de herramienta, para que los tests puedan sustituirlo por un doble sin
levantar un repositorio de verdad.
"""
