# Oráculos mínimos y efímeros por hito — UAT/E2E/AAT

**Estado de ejecución en este paquete:** `NOT_RUN`. Son especificaciones para que el agente construya fixtures efímeras *en la sesión del hito* usando el runner real existente, no suites permanentes añadidas por defecto. Preparar oráculo en JSON/tabla antes de implementar el código. Si un requisito exige una operación no anunciada por CLI/MCP instalado, investigar catálogo real y marcar el caso `BLOCKED`, nunca inventar flags.

| ID | Tipo | Escenario consumidor y oráculo | Resultado no permitido | Hito |
|---|---|---|---|---|
| U-K0 | UAT CLI/MCP | Repositorio temporal mixto `.rs`, `.java`, `.kt`, `.kts`, falla lectura en un fichero; CLI/MCP reporta alcance, excluidos y por qué; `Complete` imposible si falta entrada esencial | Grafo vacío + `Complete`; omitidos ocultos | K0 |
| E-K0 | E2E efímero | Ejecutar Full y PerFile sobre mismo conjunto; comprobar igualdad de universo y causa de diferencias | `Complete` entre estrategias con archivos elegibles dispares | K0 |
| U-K1 | UAT de símbolo | Ejecutar binario release con Kotlin class/object/top-level/extension y Unicode; devolver símbolos, scopes y código de símbolo correcto | Resultado vacío marcado supported, span desfasado, código de archivo completo | K1 |
| U-K2 | UAT relaciones | Kotlin overload `save(Int)`/`save(String)`, class homónima por package, K→J y J→K; devolver arista concreta exacta y ambigüedad no inventada | Arista apuntando al primer homónimo | K2 |
| E-K2 | E2E MCP real | Cliente MCP JSON-RPC: iniciar, listar herramienta real, usar consulta de referencias, comparar campo canónico con CLI | Diferencia de destinos, logs en stdout, `Unsupported` disimulado | K2 |
| U-K3 | UAT provider | En proyecto Gradle/Maven JVM opcional, LSP devuelve definición/referencias S2 y source span Unicode bien calculado | S2 anunciado sin handshake, Unicode truncado | K3 |
| E-K3 | E2E degradación | Parar/quitar LSP, repetir símbolos: S0 sirve con diagnósticos y tiempo limitado; no bloquea core | Timeout infinito, sin advertencia, grafo inventado | K3 |
| U-K4 | UAT SCIP condicional | Índice SCIP externo coherente produce un target antes ambiguo; índice stale/root equivocado se rechaza | `S3` con data stale, fallback sin provenance | K4 |
| A-K4 | AAT agent | Agente consulta la mínima capacidad estructurada pertinente por CLI/MCP, aporta símbolos y aristas con evidence, evita raw/doc y se detiene sin bucle | Reconstrucción textual manual, C4 con relaciones no observadas | K4 |

**Política para K4 SCIP:** si el diferencial de K3 no justifica SCIP, U-K4 se marca `NOT_TRIGGERED` con evidencia del criterio y K4 entrega el AAT agente y una mejora pública efectiva. No fabricar una UAT SCIP que el consumidor no puede ejecutar.

## Negativos indispensables

- K0: modificar contenido de `.kt` sin cambiar mtime/size y demostrar que no se sirve como snapshot vigente si el inventario prometía control de integridad.
- K1: símbolo que no existe produce `not found/unsupported` tipado; no un símbolo inventado por regex.
- K2: firma irresoluble → `Unresolved/Ambiguous` (cero aristas falsas). Call graph no prueba red ni runtime.
- K3: LSP ausente y servidor que anuncia tool pero no la ejecuta. Diagnóstico por operación/lenguaje, sin declarar incompatible Kotlin en conjunto.
- K4: agente que recibe `Partial/skipped_files` verifica relevancia o marca `UNKNOWN`, nunca lee raw como salida rápida.

## Evidencia mínima a conservar antes de eliminar fixtures

`hito`, `SHA`, `tag/asset digest`, `interface`, `tools realmente invocadas`, `input/ficha`, `expected`, `observed`, `outcome`, `skips`, `cleanup`, `promoted_regression`, `risk`. El journal existente puede enlazar a logs temporales acotados, pero los logs no deben incluir credenciales ni volcar código de terceros. No conservar 30 UAT por un caso; conservar una regresión barata al seam del defecto.
