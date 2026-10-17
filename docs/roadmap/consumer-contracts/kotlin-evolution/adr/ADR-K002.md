# ADR-K002 — Integrar gramática Kotlin sin tablas semánticas duplicadas

**Estado:** PROPOSED. **Aplica a:** K1. **Relacionado:** `docs/adr/ADR-029-callgraph-projection-port-seam.md` (puertos), `docs/adr/ADR-016-renderer-neutral-semantic-projections.md`.

## Contexto

`Language` define enum, extensión y familias de nodos AST mediante métodos singulares; `LanguageConfig` mantiene otros conjuntos de tipos de nodos. `TreeSitterParser::find_all_symbols_with_path` usa tipos singulares `function_node_type` y `class_node_type`. Kotlin tiene múltiples tipos de declaración, scripts y propiedades; añadir un único `function_declaration` no equivale a cobertura útil.

## Decisión propuesta

- Ampliar **una** descripción canónica de gramática/extracción por lenguaje que consuma el extractor genérico. Reutilizar `LanguageConfig` y migrar el acceso de `TreeSitterParser` a ella por seam pequeño, conservando funciones públicas hasta el cutover.
- Comparar y fijar una gramática Tree-sitter Kotlin mediante corpus y la versión de Tree-sitter del proyecto; contrato `Language::Kotlin` + `.kt/.kts`, con tipo de fuente para script/build DSL.
- Extracción sin IO: `Source + Tree → [DeclarationObservation] + [ParseDiagnostic]`. IDs reproducibles con paquete/owner/signature; ambigüedad visible, no colisión silenciosa por nombre.
- Solo los adaptadores conocen `tree_sitter::Node`, `Path` o backend; dominio recibe declarativos tipados. Evitar macro-framework nuevo si los datos presentes bastan.

## Alternativas

- Gramática inventada: rechazada por mantenimiento/correctitud.
- LSP obligatorio desde la primera release: rechazado; haría imposible navegar proyectos sin JDK/config.
- Añadir `Kotlin` enum sin rutas de CLI/MCP reales: rechazado por falso anuncio de producto.

## Aceptación

Kotlin top-level/class/interface/object/companion, función de extensión, archivo `.gradle.kts` y Unicode navegables desde `cognicode` y `cognicode-mcp` **distribuidos**. La matriz soporta `experimental` separado de `supported`; no marcar 100% de semántica con AST. Producto del K1, no reescritura de todos los parsers.
