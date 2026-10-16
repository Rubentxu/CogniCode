# ADR-TH-016 — La etapa de contratos corre en paralelo, con los mismos checks

**Estado:** ACCEPTED. **Fecha:** 2026-10-16. **Autoridad del gate:** ninguna cambia:
lo que se ejecuta es exactamente lo mismo; cambia cuándo se paga.

## Contexto

La aritmética publicada por `harness report` sobre la corrida `2f9d972d` dejó claro
dónde está el presupuesto del merge: **`contracts` era 371,6 s de 556,8 s, el 67%**, y
las tres mayores suites de producto juntas sumaban 99,5 s. El camino a los 420 s que
pedía el mandato pasaba por esta etapa, no por las pruebas de producto.

Y la etapa es serie: `scripts/ci/run-all-contracts.sh` invoca
`python3 scripts/run_contract_tests.py` con **37 ficheros** (6 de producto, 31 de CI) y
el runner los recorría uno detrás de otro, en un solo proceso. La cabecera del propio
fichero dice lo que son: «small, linear, dependency-free contract checks». Su trabajo
es lanzar procesos (cargo, git, python) y esperarlos.

## Decisión

1. **`run_contract_tests.py` acepta `--jobs N`** (por defecto 1: el comportamiento de
   siempre). Con N > 1 ejecuta **un contrato por proceso**, hasta N a la vez, usando el
   mismo runner para cada uno: el aislamiento de cada fichero es el que tendría
   invocado a mano, y no hay estado compartido entre ellos.
2. **La semántica de fallo no cambia.** Cada hijo devuelve su código; el padre devuelve
   1 si alguno falló, y nombra en stderr los ficheros que salieron distinto de cero. Un
   contrato que falla sigue tumbando la etapa.
3. **La salida se agrega en el orden de entrada**, no según terminan: un log de gate
   que cambia de orden entre corridas es un log que nadie puede diffear.
4. **`run-all-contracts.sh` pasa `--jobs`** con el número de CPUs (tope 8, porque por
   encima no acelera más y sí compite con el resto de la máquina). `CONTRACTS_JOBS=1`
   vuelve al modo serie, y un valor no numérico es error de uso (rc=2).
5. **El contrato del runner cubre el modo nuevo**: un contrato que falla hace fallar
   también la corrida paralela, y los contratos sanos se ejecutan y se reportan.

## Medición (2026-10-16, misma máquina, mismo conjunto de 37 ficheros)

| Modo | Pared | Checks | Ficheros |
|---|---|---|---|
| Serie (el de siempre) | **316 s** | 296 passed, 0 failed | 37 |
| `--jobs 8` | **114 s** | 298 passed, 0 failed | 37 |

**2,8× más rápido.** Y la equivalencia está comprobada fichero a fichero: **36 de los
37 ficheros reportan exactamente el mismo recuento** en los dos modos, y el único que
difiere es `test_ci_contracts.py` (7 → 9), porque entre las dos corridas **este mismo
bloque añadió dos tests a ese fichero**. No falta ni un check y no se añade ninguno.

Efecto en el gate: la etapa `contracts` debería bajar de ~320-370 s a ~110-160 s, y el
total del merge quedaría alrededor de los 360-400 s **sin dejar de ejecutar nada**,
que es la forma de cumplir el presupuesto que el mandato pedía (a diferencia de dejar
etapas fuera por selección, que es W6 y decisión del operador).

## Consecuencias

- El presupuesto de merge se alcanza por concurrencia, no por cobertura: ninguna
  obligación sale de la lane y ningún contrato deja de ejecutarse.
- El modo paralelo es el defecto de la etapa; el serie sigue disponible con
  `CONTRACTS_JOBS=1` para diagnosticar una interacción.
- La varianza de la máquina (otros agentes compilando) afecta a las dos cifras por
  igual; la comparación se hizo en corridas consecutivas y con el mismo conjunto.

## Lo que este ADR no decide

- El presupuesto de merge en general ni la selección por impacto (W6).
- Paralelizar las suites de producto del gate: cada una es un proceso `cargo` que ya
  compite por el mismo directorio de target, y su ganancia no está medida.
