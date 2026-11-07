#!/usr/bin/env bash
# El presupuesto de memoria de las corridas de mutación. Un solo dueño.
#
# QUÉ ES ESTE FICHERO
#
# Las corridas de mutación lanzan el binario de test del crate que se está
# mutando, y ese binario puede pedir la memoria que le dé la gana: un
# mutante que multiplica en vez de sumar crece hasta que la máquina se
# queda sin memoria. Medido el 2026-11-05 a las 19:40, el OOM-killer del
# kernel mató un binario de `cognicode-core` de 37 GB de RSS (67 GB
# virtuales) y, en la misma pasada, se llevó `coredns`, el `java` de un
# contenedor y dos procesos de otro — que no son de este repositorio.
#
# El techo que evita eso es `scripts/mem-capped-runner.sh`, y el
# presupuesto que lo acota vive en este fichero. El daño de pasarse no lo
# paga el mutante que se pasa: lo pagan sus vecinos, y un mutante que los
# tests matan en segundos se reporta como TIEMPO AGOTADO — un gate rojo
# que no dice nada del código—.
#
# POR QUÉ UN FICHERO Y NO DOS NÚMEROS EN DOS SCRIPTES
#
# Porque dos scripts que nombran el mismo presupuesto son dos fuentes de
# verdad, y la segunda se pudre: el paso que lee la vieja pide más
# memoria de la que la máquina tiene, y la certificación dice que el gate
# cabe. Los dos routes —el paso de mutantes del gate y la corrida focal
# de `scripts/mutate-file.sh`— cargan este fichero.
#
# LAS CIFRAS, Y DE DÓNDE SALEN
#
#   MUTANTS_BUDGET_KIB    16 GiB de espacio de direcciones a la vez. Es
#                         una cuarta parte de lo que una máquina de esta
#                         clase tiene libre, con sitio para el resto del
#                         workspace. Lo fija
#                         `scripts/ci/test_gate_contracts.py`.
#
#   MUTANTS_GATE_VMEM_KIB 2 GiB por binario de test. El valor que el
#                         runner ya usaba por defecto, y el que el gate
#                         lleva midiendo: la corrida certificada del
#                         2026-11-06 (627 mutantes, 618 atrapados, 9
#                         inviables, 0 supervivientes, 0 tiempos
#                         agotados) lo midió con este techo y con un
#                         trabajo a la vez.
#
#   MUTANTS_FOCAL_VMEM_KIB 4 GiB por binario de test. Una corrida focal
#                         muta el crate que se le pida, y el crate que
#                         se pide a mano es `cognicode-core` (217k
#                         líneas), que está FUERA del alcance del gate
#                         por coste. Su binario de test tiene un pico
#                         legítimo medido de 2 828 944 kB = 2,70 GiB de
#                         RSS (`cargo test -p cognicode-core --lib --
#                         --test-threads=4`, 2565 tests en 13,58 s). Un
#                         techo por debajo de ese pico no acota a un
#                         mutante runaway: mata al binario en CADA
#                         mutante y los reporta a todos como atrapados,
#                         que es un veredicto falso. 4 GiB deja el pico
#                         legítimo debajo y corta al mutante que se
#                         desborda antes de que se lleve la máquina.
#
# El contrato que vigila estos tres números y las dos rutas es
# `scripts/ci/test_gate_contracts.py`.

# El presupuesto: lo que las corridas de mutación pueden pedir a la vez.
MUTANTS_BUDGET_KIB=16777216

# El techo por binario de test del paso de mutantes del gate.
MUTANTS_GATE_VMEM_KIB=2097152

# El techo por binario de test de una corrida focal.
MUTANTS_FOCAL_VMEM_KIB=4194304
