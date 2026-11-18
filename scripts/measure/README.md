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
