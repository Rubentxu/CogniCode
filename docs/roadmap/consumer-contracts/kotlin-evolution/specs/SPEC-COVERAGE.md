# SPEC-K-COV — Cobertura, inventario y equivalencia de estrategias (K0)

**Estado:** propuesta nueva complementaria a `docs/prf/specs/SPEC-ANALYSIS.md` (histórica) y `docs/roadmap/ROADMAP.md` (agenda).

## Requisitos normativos

- **K-COV-01 MUST:** el universo de escaneo se declara como `Discovered`, `PolicyIgnored`, `UnsupportedLanguage`, `Eligible`, `ReadFailed`, `ParseFailed`, `Indexed`, `Cancelled` y `Unknown`. `Unchanged` es un tipo de optimización incremental, no una razón de falta de soporte.
- **K-COV-02 MUST:** un `Complete(scope)` acredita todos los archivos **elegibles** conocidos del alcance declarado. Si Kotlin no está soportado, se registra como `UnsupportedLanguage` y no como «inexistente». Un «Complete de Rust» no significa «Complete del repo».
- **K-COV-03 MUST:** `full` y `per_file` utilizan el mismo inventario canónico y universo; con idéntica política/config/basis emiten conjuntos semánticamente equivalentes o diferencias tipadas, nunca errores silenciosos. Corregir selector de cuatro extensiones en `PerFileStrategy::build_full_graph_report`.
- **K-COV-04 MUST:** `SkippedFile {path,reason}` conserva la procedencia de walk/read/parse/unsupported/policy; si se trunca lista reportar contador y `truncated=true`, no omisiones invisibles. `source_manifest` e identidad del snapshot provienen de mecanismo existente, no de campos sin verificación.
- **K-COV-05 MUST:** `mtime/size` solo aceleran; cambios de bytes con mtime/tamaño iguales no deben emitir falsos `Unchanged` (se reevalúa la política de hashes y contrato incremental preexistentes).
- **K-COV-06 MUST:** una consulta CLI/MCP comunica `analysis_basis`, `coverage`, `languages`, conteos y motivo por lenguaje. `Unsupported` ≠ `Empty`; `Partial` no es éxito completo.
- **K-COV-07 MUST:** nombre/extensión de archivo en `skipped_files` no se toma como prueba de irrelevancia para C4; sin evidencia de que una entrada/relación queda cubierta, el consumidor puede declarar `Partial/Unknown`.
- **K-COV-08 SHOULD:** derivar lista de extensiones admitidas de `LanguageConfig`/`Language` con una única fuente de verdad; el runtime tool catalog no infiere soporte completo por aparición de una extensión.

## ADTs y función pura (diseño semántico, no interfaz obligatoria)

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
enum FileDecision { Eligible(Language), Ignored(IgnoreReason), Unsupported(Extension), Unknown(Reason) }
#[derive(Clone, Debug, PartialEq, Eq)]
enum FileOutcome { Indexed { symbols: usize }, Failed(FailureKind), Excluded(FileDecision) }
fn classify(path: &RelativePath, supported: &LanguageCatalog, policy: &ScanPolicy) -> FileDecision;
fn aggregate(scope: &ScanScope, observations: &[FileObservation]) -> CoverageReport;
```

Las decisiones de lectura/recorrido y la ejecución del parser pertenecen a adaptadores; `classify` y `aggregate` permanecen puras. En el runtime, reutilizar `BuildReport`, `SkipReason`, `AnalysisOutcome` existentes y no duplicar su semántica.

## Casos discriminantes mínimos

1. Corpus con un `.rs`, un `.java`, un `.kt`, `.kts`, un fichero ilegible y un fichero no relacionado: el universo inventariado no pierde los `.kt`; antes de K1 deben clasificarse `Unsupported`, no `Indexed`, y una vista de *toda* la arquitectura es parcial.
2. Falla de parseo de fuente elegible: `BuildReport` parcial con ruta y causa, no exit 0 sin cobertura.
3. Full/per_file con mismo scope: igual universo y diferencia real registrada si uno no pudo producir aristas.
4. Sin corrupción de cache en alteración de contenido: al actualizar bytes, revalidar base o marcar `Unknown`, jamás servir grafo viejo como actual.

## No objetivos

No instalar Kotlin, no abrir un segundo graph store, no declarar completitud del proyecto por un mero conteo. Aceptación sobre ABI/CLI anterior y tool MCP con datos nuevos opcionales y compatibles.
