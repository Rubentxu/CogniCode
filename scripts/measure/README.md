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
