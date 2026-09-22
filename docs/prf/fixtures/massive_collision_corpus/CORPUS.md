# Massive-collision corpus (PRF-ANA-07)

Corpus independiente (no derivado de `cognicode-core`) usado por la
unidad **PRF-ANA-07** para verificar que el resolver scope-aware de
`GlobalSymbolIndex` (introducido en F2.W5 / integrado en el camino real
del binario por F2.W7) sigue aplicando las reglas correctas cuando el
proyecto tiene **decenas de archivos con el mismo short-name**.

`cross_file_scope_aware/` (F2.W7) cubre el caso de 1-3 homónimos. Este
corpus ataca el siguiente escalón: **50 archivos `d{1..50}.rs`, cada
uno declarando `pub fn init()`**, más un candidato único cross-file.

## Estructura

| Path | Función declarada | Rol |
|---|---|---|
| `src/lib.rs` | local `init()`, `caller_in_lib()` | caller con visibilidad local |
| `src/d{1..50}.rs` | `pub fn init()` | **50 homónimos cross-file** |
| `src/sibling_unique_compute.rs` | `pub fn compute()` | candidato único cross-file |

Total: 51 archivos `src/*.rs`. Cada uno es un módulo top-level del
crate (declarado en `lib.rs` con `mod d{i};`). El resolver ve un mapa
`by_name["init"]` con **51 entradas** (1 local + 50 sibling), cada una
apuntando a un `SymbolId` distinto (uno por archivo).

## Las dos llamadas de `caller_in_lib` y las reglas que ejercen

| # | Caller invoca | Candidates globales | Regla esperada | Resultado esperado |
|---|---|---|---|---|
| 1 | `init()` | **51** (1 local + 50 sibling) | visibilidad: same-file picks local | `src/lib.rs:init` |
| 2 | `sibling_unique_compute::compute()` | **1** | single-candidate rule | `src/sibling_unique_compute.rs:compute` |

## Por qué este corpus

El requisito `PRF-ANA-07` exige que *"renames/moves/colisiones
preservan identidad o devuelven ambigüedad visible"*. Los corpus
anteriores sólo verificaban la rama de 1 candidato y de 2-3
homónimos. La pregunta que este corpus ataca es:

> ¿El resolver se mantiene correcto cuando el número de candidatos
> globales crece de 3 a 51?

El bug pre-F2.W7 era "elige el FQN lexicográficamente menor". En un
corpus con 51 homónimos, ese bug se manifiesta como **inestabilidad**:
el árbol FQN cambia con cada nuevo archivo añadido. La fix correcta
debe depender sólo de (a) presencia local y (b) identidad del caller.

El test del caso 1 (`mass_collision_same_name_picks_local`) pinea el
comportamiento de visibilidad cuando hay >50 candidatos. Si alguien
regresara al bug pre-F2.W7 o introdujera un nuevo "pick first inserted",
este test fallaría porque el destino elegido NO estaría en `lib.rs`.

## Lo que NO cubre este corpus (intencional)

El caso de "two-way cross-file ambiguity, no local anchor" (cuando
ambos homónimos están sólo en archivos siblings y NO hay local) ya
está pineado por `cross_file_scope_aware/`
(`w7_two_way_homonym_no_caller_file_honest_drop`). Repetirlo aquí
requeriría que Rust aceptara una llamada no calificada a una
función declarada en dos módulos — Rust directamente rechaza el
programa antes de llegar al parser, lo que hace ese test imposible
de escribir como corpus Rust válido.

## Uso

Los tests viven en `crates/cognicode-core/src/application/services/
analysis_service.rs` bajo el módulo
`prf_ana_07_massive_collision_tests`. Construyen el grafo con el
camino real (`AnalysisService::build_project_graph`) — el mismo que
ejecuta el binario.

## Generación

El corpus se regenera con:

```bash
python3 scripts/generate_massive_collision_corpus.py
```

El generador es determinista (loop simple, sin timestamps ni random).
El corpus está versionado también, así que la primera vez que se
clona el repo los archivos ya están presentes; re-ejecutar el script
es idempotente.
