# scripts/measure

Instrumentos de medición. Ninguno forma parte del core ni del gate.

## Qué enlaces sigue un agente real (TASK-94)

Cada respuesta de `cognicode-mcp` ofrece enlaces (`_links`, con `rel`, `tool` y
`required_args`). `link_follow.py` cuenta cuáles se siguieron después. Los
`_links` viajan solo en `structuredContent` y un transcript del cliente no los
conserva con su `rel`, así que la traza la escribe el servidor.

1. Arranca el servidor MCP del cliente con la variable apuntando a un fichero:

   ```sh
   COGNICODE_LINK_TRACE=$HOME/cognicode-links.jsonl cognicode-mcp --cwd <proyecto>
   ```

   En un cliente, la variable tiene que llegar al proceso de `cognicode-mcp`:
   expórtala en el entorno desde el que lo arrancas, o usa el campo de entorno
   que admita su configuración MCP. Una sesión sin la variable no escribe
   nada y no aporta datos. El fichero contiene argumentos de llamadas (rutas,
   símbolos): trátalo como sensible (ver `SECURITY.md`).

2. Usa el agente con normalidad. Cada llamada añade una línea
   `{tool, arguments, links, session}`; `session` es un proceso del servidor.

3. Cuenta:

   ```sh
   python3 scripts/measure/link_follow.py $HOME/cognicode-links.jsonl
   ```

   Imprime un JSON con `offered`, `followed`, `rate` y `verdict` por `rel`. Un
   `rel` nunca ofrecido es «sin datos», no un cero; «candidato a retirar» exige
   al menos 20 ofertas y tasa cero. El informe señala, no retira.

`link_trace.py` produce una traza sintética con una política determinista
(`--follow all|none|rel,rel`) para probar el contador; mide el contrato de
cable, no el comportamiento de un agente (con «seguir todo» la tasa es 1 por
construcción).

## Context pack frente a `smart_search` y `codebase_map`

`context_pack_ab.py` arranca sesiones MCP reales (`--profile full`) sobre un
árbol y, para cada caso con verdad terreno escrita a mano, mide `pack`,
`smart_search` (con la frase tal cual y con las palabras unidas por `_`) y
`codebase_map`: acierto del símbolo esperado entre los K primeros, precisión por
fichero, latencia (primera llamada de la sesión aparte), bytes de lo que lee el
agente y qué campos de evidencia trae la respuesta.

```sh
python3 -I scripts/measure/context_pack_ab.py --server <cognicode-mcp> \
  --workspace crates/cognicode-core/src/application --sessions 3 --rounds 5 \
  --out /tmp/context-pack-ab.json
```

Reglas del instrumento: el host es compartido, así que el informe registra la
carga y los núcleos y da p50 con su ruido y el número de muestras; una llamada
fallida se cuenta aparte y no entra en ningún número; los bytes no se convierten
en tokens; un campo que la respuesta no trae es `false`, no una estimación. La
comparación vale dentro de un mismo binario (uno debug es mucho más lento que
uno release).

## Pack de un cambio frente a `git diff` y leer ficheros

`change_pack_ab.py` crea en un directorio temporal un repositorio Rust escrito a
mano (nueve ficheros de código, cinco de tests, un README), aplica un cambio por
caso sobre el árbol de trabajo y compara `cognicode pack --change-scope
working_tree` con lo que haría hoy un agente: `diff` (solo `git diff`),
`grep_lines` (el diff y un `git grep -n -w` por nombre, sin leer ficheros) y
`grep_files` (el diff, `git grep -l -w` y leer entero cada fichero que casa).
Casos: firma pública (`api-signature`), cuerpo de un auxiliar privado
(`private-body`), un tipo que implementa un trait (`contract-implementor`), el
cuerpo de una hoja (`unrelated-body`) y solo documentación (`docs-only`). La
verdad terreno (`required`, `unwanted`) está escrita en el propio script.

```sh
cargo build -p cognicode-cli
python3 -I scripts/measure/change_pack_ab.py --cognicode <target>/debug/cognicode \
  --runs 7 --out /tmp/change-pack-ab.json
```

Qué mide, por caso y con varias pasadas: acierto (`recall`) y precisión sobre la
verdad terreno, bytes que lee el agente, llamadas, y latencia (p50 y rango
p10-p90; el pack se mide en frío, con una caché nueva por pasada, y en caliente).
`pack` cuenta por símbolo; las alternativas no nombran símbolos, así que cuentan
por fichero y se les concede que leer un fichero que declara un símbolo
requerido es acertarlo (lectura caritativa que favorece a la alternativa). Se
mide el `pack` con el presupuesto por defecto y con `--max-tokens 6000`
(`pack-wide`), para separar lo que deja fuera el presupuesto de lo que el grafo
no sabe. El informe guarda el estado y los tipos de hueco del pack: un caso con
una relación que el grafo no ve no debe volver completo.

Límites del instrumento: la compilación es de depuración (los tiempos solo se
comparan dentro de un mismo binario), el host es compartido (se guardan la carga
y los núcleos antes y después), el fixture es pequeño y una sola semilla de
cada tipo de cambio, y las alternativas no incluyen el razonamiento del agente
entre llamadas.

Límites del grafo que el instrumento deja a la vista: los tests que llaman a la
función dentro de una macro (`assert_eq!(f(), ..)`) no tienen arista de llamada,
y el lector de Rust no publica la relación `impl Trait for Tipo` como
`Inherits`, así que en `contract-implementor` el contrato y sus consumidores
indirectos no llegan.

## Navegar los niveles desde un `pack`

`pack_levels_walk.py` abre una sesión MCP real (`--profile full`) sobre un
fixture con `hub` y doce llamantes en cuatro ficheros, y para una tarea de
arquitectura («quién depende de `hub`») y una de fallo («lee el código de
`hub`») compara tres brazos: `pack_only` (una llamada), `pack_links` (el
`pack` y solo los enlaces `_links` del nivel que la tarea necesita, con los
argumentos que traen) y `files` (`git grep -l -w` y leer entero cada fichero
que casa). Por brazo: llamadas, bytes que lee el agente y éxito contra la
verdad terreno (cada hecho como palabra completa, solo en lo entregado), más
la autoridad de cada enlace seguido (`widened` vacío = ningún enlace ensancha
permisos) y el consejo de parada del `pack`.

```sh
cargo build -p cognicode-mcp
python3 -I scripts/measure/pack_levels_walk.py \
  --binary <target>/debug/cognicode-mcp --out /tmp/pack-levels-walk.json
```

Límites: la política de seguimiento es determinista (no es un agente), el
fixture es diminuto (un fichero entero cuesta menos bytes que la respuesta de
un `pack`, que lleva envoltorio, enlaces y evidencia), los bytes no se
convierten en tokens, una sola pasada, host compartido (se guarda la carga) y
no es el baseline de recuperación real del Context Pack.
