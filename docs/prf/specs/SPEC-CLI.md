# PRF-CLI — CLI de análisis estable

**Owner funcional:** binario `cognicode`; `cogh` solo instala/versiona. En F0 inventariar comandos reales y no inventar `analyze`, `graph` u opciones que el binario todavía no publica.

- **PRF-CLI-01 MUST:** `--help`/`--version` y cada comando stable enumerado tienen código de salida, argv y diagnóstico documentados; sin `exit 0` si no se realizó la operación. Escenario: ruta inexistente y configuración inválida → no éxito.
- **PRF-CLI-02 MUST:** en operaciones con salida estructurada, stdout contiene solo datos con esquema/versión declarados; stderr contiene logs/avisos. No cambiar una salida histórica sin migración y test de cliente anterior.
- **PRF-CLI-03 MUST:** el usuario puede seleccionar workspace mediante ruta canónica, sin dependencia de arrancar Explorer, RPC, cloud o collector OTLP; fallos parciales no se transforman en resultados completos.
- **PRF-CLI-04 MUST:** para una capability ofrecida también por MCP, el resultado semántico (estado, símbolos, relaciones, cobertura, revisión y límites) procede del mismo caso de uso; las diferencias de transporte se documentan.
- **PRF-CLI-05 MUST:** los comandos de mutación/refactor exigen autorización separada y previsualización/rollback donde se anuncien; CLI estable core read-only por defecto, sin romper cliente histórico publicado.
- **PRF-CLI-06 MUST:** Unicode, espacios, cwd diferente y error de permisos se comportan de forma determinista y útil; sin exposición de secretos por verbose.
- **PRF-CLI-07 SHOULD:** salida JSON legible por máquinas y semver de esquema cuando se declare estable; ninguna promesa retroactiva hasta F0.

**Certificación:** C0/C1/C3/C5; U01,U02,U04,U06–U08,U10,U19. Oráculo: ejecutar **binario del bundle** sobre corpus versionado y comparar con cliente viejo.
