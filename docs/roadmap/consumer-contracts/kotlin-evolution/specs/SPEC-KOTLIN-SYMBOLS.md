# SPEC-K-SYM — Kotlin AST y navegación por símbolo (K1)

## Comportamiento de producto

- **K-SYM-01 MUST:** registrar `.kt` y `.kts` en el mismo catálogo de lenguajes usado por parser, scanner e interfaces; reconocer scripts Kotlin diferenciando `SourceKind::KotlinFile | Script | BuildDsl`, sin afirmar que un script constituye un servicio runtime.
- **K-SYM-02 MUST:** elegir gramática Tree-sitter compatible *medida* mediante corpus pequeño de Kotlin (Kotlin/JVM 1.8–2.x relevante), mantener versión reproducible en `Cargo.lock`. Si un candidato falla node-types críticos, descartar con pruebas, no inventar nodos.
- **K-SYM-03 MUST:** reconocer declarations `package`, `import`, class/interface/object/companion/enum/data/sealed/value (si gramática permite), constructor, functions top-level y miembro, receiver de extensiones y `val/var`; conservar ubicación y padre/ámbito. Si la gramática no soporta un constructo, evidencia `Unresolved`.
- **K-SYM-04 MUST:** la extracción genérica utiliza los conjuntos `LanguageConfig` para nodos múltiples, evitando mantener una segunda tabla semántica en `Language::function_node_type` y su duplicado en parser. Introducir un seam de compatibilidad incremental, sin reescribir todos los lenguajes por anticipado.
- **K-SYM-05 MUST:** identidad determinista en package + owner + kind + signature cuando se conoce; permitir ambigüedad explícita y overloads. No unificar por simple name. Permitir coordenadas correctas en UTF-8/UTF-16 según interfaz sin desplazamiento silencioso.
- **K-SYM-06 MUST:** `get_file_symbols`, `get_symbol_code`, `query_symbol_index` deben funcionar con Kotlin del artefacto distribuido o comunicar `Unsupported/Partial` con precisión. No requerir graph completo para obtener símbolos.
- **K-SYM-07 MUST:** la matriz `product/languages.json` se actualiza por el generador/certificación ya existente y queda `experimental` hasta resolver semántica. No cambiar a `supported` solo por parsear nodos.

## Extracción FP

`SourceText + ParsedTree + LanguageDescriptor → Vec<DeclarationObservation> + Diagnostics`, determinista y libre de IO. Proveedor parser hace IO; normalizador funcional asigna IDs/tipos; repositorio grafo conserva puertos existentes. Las estructuras `DeclarationObservation`, `SymbolIdentity`, `SymbolScope` se modelan ADT, no `String`/`Option<String>` que escondan la naturaleza de la declaración.

## UAT K1

Corpus efímero construido durante el ciclo: un Kotlin JVM con clase, data class, object companion, función top-level, extensión `String.foo`, dos overloads, string Unicode, un `build.gradle.kts` y un script `.pipeline.kts`. Comprobar símbolo/ubicación/fragmento a través del **binario instalado**; si el parser no soporta un nodo, describirlo en la matriz y reducir el claim; no inventar evidencia.

## Cierre

Parser y API navegable pasando casos seleccionados; diferencia fría/caliente determinista; un solo test permanente discriminante para identity, otro para código de símbolo si no existe equivalente. Sin aceptar target desconocido por retorno vacío. Release efectiva.
