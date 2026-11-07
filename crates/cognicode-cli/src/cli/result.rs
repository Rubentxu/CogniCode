//! T1-5.3b — Structured result types for the CLI subcommands.
//!
//! Estado pre-T1-5.3b:
//!   - `execute_index` (y los otros 9 `execute_*`) devolvían
//!     `Result<(), Box<dyn Error>>` con la información del éxito
//!     comunicada exclusivamente vía `println!`/`eprintln!`.
//!   - La librería no tenía cómo decirle al bin qué había
//!     pasado: el bin solo podía re-imprimir lo que la librería
//!     había decidido poner en stdout.
//!   - El bin no podía formatear diferente (text vs JSON) sin
//!     cambiar el código de la librería.
//!
//! Lo que este módulo aporta:
//!   - `IndexResult`: el resultado exitoso de cada variante
//!     de `IndexCommand` como dato estructurado. El bin lo
//!     formatea (text o JSON) sin que la librería sepa nada
//!     del transporte.
//!   - `IndexError`: el error tipado con `#[source]` para
//!     preservar la chain. El bin decide el formato del
//!     error (text line, JSON envelope, etc.).
//!   - `Serialize` en ambos: el JSON es canónico; el text es
//!     decisión del presenter.
//!
//! Política T1-5.3:
//!   - La librería NO formatea. Solo emite datos.
//!   - El bin (cognicode-cli) tiene el presenter.
//!   - El cambio de format es un commit de "format change"
//!     separado, no un side-effect del refactor de T1-5.3c.
//!
//! Ver `odd/tasks/t1-5-3a-index-design.md` para el design
//! completo (antes/después, riesgos, y plan de réplica a
//! los otros 9 sub-comandos en T1-5.4..6).

use serde::Serialize;
use std::path::PathBuf;

use cognicode_core::domain::value_objects::{
    MermaidFormat, SymbolKind, published_column, published_line,
};

pub use cognicode_core::domain::traits::code_intelligence::ReferenceKind;
pub use cognicode_core::infrastructure::graph::SymbolLocation;
pub use cognicode_core::infrastructure::semantic::outline::OutlineNode;

/// Una posición como la publica el producto.
///
/// El índice y el outline cuentan desde cero —lo que entrega el
/// parser— y el producto cuenta desde uno: la línea y la columna que
/// lee una persona. El cruce vive aquí, y no en el presenter.
#[derive(Debug, Clone, Serialize)]
pub struct PublishedPosition {
    pub file: String,
    /// Line number (1-indexed): the line a person reads.
    pub line: u32,
    /// Column number (1-indexed): the column a person reads.
    pub column: u32,
}

/// Una localización como la publica el producto.
#[derive(Debug, Clone, Serialize)]
pub struct PublishedLocation {
    pub file: String,
    /// Line number (1-indexed): the line a person reads.
    pub line: u32,
    /// Column number (1-indexed): the column a person reads.
    pub column: u32,
    pub symbol_kind: SymbolKind,
}

/// Un nodo del outline como lo publica el producto.
#[derive(Debug, Clone, Serialize)]
pub struct PublishedOutlineNode {
    pub name: String,
    pub kind: SymbolKind,
    pub location: PublishedPosition,
    pub signature: Option<String>,
    pub children: Vec<PublishedOutlineNode>,
    pub is_private: bool,
}

impl From<&SymbolLocation> for PublishedLocation {
    fn from(location: &SymbolLocation) -> Self {
        Self {
            file: location.file.clone(),
            line: published_line(location.line),
            column: published_column(location.column),
            symbol_kind: location.symbol_kind,
        }
    }
}

impl From<&OutlineNode> for PublishedOutlineNode {
    fn from(node: &OutlineNode) -> Self {
        Self {
            name: node.name.clone(),
            kind: node.kind,
            location: PublishedPosition {
                file: node.location.file().to_string(),
                line: published_line(node.location.line()),
                column: published_column(node.location.column()),
            },
            signature: node.signature.clone(),
            children: node
                .children
                .iter()
                .map(PublishedOutlineNode::from)
                .collect(),
            is_private: node.is_private,
        }
    }
}

/// Resultado exitoso de un `cognicode index <subcommand>`.
///
/// `#[serde(tag = "kind")]` permite serializar a JSON con un
/// campo discriminador (`"build"`, `"query"`, etc.) sin
/// anidamiento extra. El bin formatea text o JSON desde esta
/// misma estructura. T1-5.5 JSON dispatch tests (en
/// `cognicode-cli/tests/t1_5_5_json_dispatch.rs`) parsean a
/// `serde_json::Value` para verificar shape (no round-trip
/// directo: u128 elapsed_ms no es soportado por serde_json).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IndexResult {
    /// `cognicode index build <path> <strategy>` — index built OK.
    Build {
        path: PathBuf,
        strategy: String,
        elapsed_ms: u128,
    },
    /// `cognicode index query <symbol> <path>` — symbol found at N
    /// locations. El presenter text emite
    /// "Found {n} location(s):\n  FILE:LINE:COL (KIND)\n  ...".
    Query {
        symbol: String,
        path: PathBuf,
        locations: Vec<PublishedLocation>,
    },
    /// `cognicode index query <symbol> <path>` — symbol not found.
    /// Variante separada de `Query { locations: vec![] }` para
    /// que el JSON distinga entre "found zero" (raro pero
    /// posible) y "didn't find" (lo normal cuando no hay match).
    QueryEmpty { symbol: String, path: PathBuf },
    /// `cognicode index outline <file>` — outline extracted.
    Outline {
        file: PathBuf,
        symbols: Vec<PublishedOutlineNode>,
    },
    /// `cognicode index symbol-code <file>:<line>:<col>` — source
    /// extracted for a symbol.
    SymbolCode {
        file: PathBuf,
        line: u32,
        column: u32,
        start_line: u32,
        end_line: u32,
        doc: Option<String>,
        code: String,
    },
}

impl IndexResult {
    /// Publishes the positions of an index query.
    ///
    /// The index counts from zero —what the parser hands out— and the
    /// product counts from one: the line and the column a person
    /// reads. The crossing happens here, once, and the CLI publishes
    /// the same base the rest of the product publishes.
    pub fn published_query(symbol: String, path: PathBuf, locations: &[SymbolLocation]) -> Self {
        Self::Query {
            symbol,
            path,
            locations: locations.iter().map(PublishedLocation::from).collect(),
        }
    }

    /// Publishes the nodes of an outline, with the same base.
    pub fn published_outline(file: PathBuf, nodes: &[OutlineNode]) -> Self {
        Self::Outline {
            file,
            symbols: nodes.iter().map(PublishedOutlineNode::from).collect(),
        }
    }
}

/// Error tipado de un `cognicode index <subcommand>`.
///
/// `#[source]` preserva la chain para que el presenter pueda
/// mostrar el mensaje del error original (no solo el wrapper).
/// `thiserror` deriva `Display` y `Error` por nosotros; el
/// `Box<dyn Error + Send + Sync>` permite al error cruzar
/// el async boundary y downcast-earse en el caller.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Error building index at {path} using {strategy} strategy: {source}")]
    Build {
        path: PathBuf,
        strategy: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Error querying symbol '{symbol}' in {path}: {source}")]
    Query {
        symbol: String,
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Error reading outline source {file}: {source}")]
    Outline {
        file: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Error reading symbol code at {file}:{line}:{column}: {source}")]
    SymbolCode {
        file: PathBuf,
        line: u32,
        column: u32,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl IndexResult {
    /// `true` if the result represents a successful operation
    /// that the presenter should emit. Used by the bin to
    /// decide between `present(&result)` (success) and
    /// `present_error(&err)` (failure).
    pub fn is_empty_query(&self) -> bool {
        matches!(self, IndexResult::QueryEmpty { .. })
    }
}

// ===================================================================
// T1-5.5 — `CommandOutput`
// ===================================================================
//
// Estado pre-T1-5.5:
//   - `CommandExecutor::execute` devolvía `Result<(), _>` con la
//     información del éxito comunicada por stdout/stderr en sitio.
//   - El bin no recibía datos del éxito; solo podía re-imprimir lo
//     que la librería había decidido emitir.
//
// Lo que T1-5.5 aporta:
//   - `CommandOutput` es el dato estructurado que cruza la frontera
//     library → bin. Hoy solo `Index` y `Graph` tienen variantes
//     tipadas (T1-5.3c + T1-5.4); el resto de sub-comandos sigue
//     emitiendo texto inline y se marca como `Other`.
//   - El bin (cognicode-cli) inspecciona este enum y decide el
//     formato (text o JSON) sin que la librería sepa nada del
//     transporte.
//
// Política T1-5.5:
//   - La librería NO formatea. Solo emite datos.
//   - El bin es dueño del presenter (text + JSON dispatch).
//   - Añadir un `*Result` para un sub-comando nuevo implica
//     añadir la variante correspondiente a `CommandOutput`.

/// Output estructurado de cualquier `cogh <subcommand>`.
///
/// El bin (cognicode-cli) consume este enum para decidir qué
/// presenter invocar y con qué formato. Las variantes con dato
/// tipado (`Index`, `Graph`, `Navigate`) permiten dispatch
/// `--format text|json`; `Other` cubre los sub-comandos no migrados
/// aún (Doctor, FindUsages, Capabilities, ExportAssurance, Refactor,
/// Evidence, DocsIngest, IssuesIngest, Analyze) que ya emitieron su
/// output internamente vía `println!` y no necesitan presenter.
#[derive(Debug)]
pub enum CommandOutput {
    /// `cogh index <subcommand>` — ver `IndexResult` para variantes.
    Index(IndexResult),
    /// `cogh graph <subcommand>` — ver `GraphResult` para 11 variantes.
    Graph(GraphResult),
    /// `cognicode navigate <subcommand>` — T1-5.6a. Ver `NavigateResult`.
    /// Antes de T1-5.6a el bin recibía `Other`; el presenter nuevo
    /// emite text y JSON desde esta variante.
    Navigate(NavigateResult),
    /// B4.W2 — `cogh check-architecture`. Read-only diagnostic surface:
    /// observations with evidence, never a gate. The catalog declares
    /// `gate_authority: false` + `produces_observations_only: true` so an
    /// MCP client can verify the contract without reading the prompt.
    Architecture(cognicode_core::architecture::ArchitectureDiagnostic),
    /// An answer the core rendered and the presenter writes.
    ///
    /// The core does not write to the standard output: it returns the
    /// answer and the presenter decides the channel. The rendering
    /// lives in the core for these commands because their format is
    /// chosen by the command's own `--format` flag, not by the
    /// top-level one — the presenter is not the one that knows which
    /// format the command was asked for.
    ///
    /// The commands that answer with a typed result (`Index`, `Graph`,
    /// `Navigate`) do not use this: there the presenter formats.
    Rendered {
        /// The answer, ready to write.
        text: String,
        /// The code the process ends with, when the command's outcome
        /// is an exit code and not only an answer.
        exit_code: Option<i32>,
    },
    /// Sub-comando no migrado a `*Result` aún. El texto ya fue
    /// emitido internamente por `execute_*`; el bin no hace nada.
    Other,
}

impl CommandOutput {
    /// An answer the core rendered, with no exit code of its own.
    pub fn rendered(text: impl Into<String>) -> Self {
        CommandOutput::Rendered {
            text: text.into(),
            exit_code: None,
        }
    }

    /// The code the process ends with, when the command's outcome is
    /// an exit code and not only an answer.
    ///
    /// The doctor is the case: its report is the answer, and a missing
    /// installation is also a failure, so the process ends with a code
    /// the consumer reads.
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            CommandOutput::Rendered { exit_code, .. } => *exit_code,
            _ => None,
        }
    }
}

// ===================================================================
// T1-5.4 — `GraphResult` + `GraphError`
// ===================================================================
//
// Estado pre-T1-5.4:
//   - `execute_graph` (11 sub-commands: OnDemand, PerFile, Full,
//     HotPaths, EntryPoints, LeafFunctions, TracePath, Mermaid,
//     Hierarchy, Complexity, Impact) emitted ~50 `println!` and
//     ~10 `eprintln!` calls scattered across the match arms.
//   - The data side was implicit: each arm re-computed strings
//     from in-scope variables, with no machine-readable form.
//   - The text output had no testable boundary — a regression in
//     a label, a number format, a column alignment could only be
//     caught by a human reading `cogh graph ...` output.
//
// Política T1-5.3 (sin cambios):
//   - La librería NO formatea. Solo emite datos.
//   - El bin (cognicode-cli) tiene el presenter.
//   - Los variants se diseñan pensando en JSON canónico
//     (`#[serde(tag = "kind")]` con `kind` como discriminador)
//     para que T1-5.5 pueda añadir dispatch text/JSON.
//
// Lo que T1-5.4 aporta:
//   - `GraphResult`: 11 variants, uno por sub-command, con los
//     campos mínimos para reproducir el text output pre-migration
//     byte-identical. El pilot de T1-5.4 es `PerFile` (migración
//     + snapshot); los otros 10 variants se migran en T1-5.4b..m
//     con el mismo flujo que T1-5.3c/d aplicó a `index`.
//   - `GraphError`: errores tipados con `#[source]`, paralelo a
//     `IndexError`. La primera variante (BuildIndex) cubre el
//     path de error que comparten `OnDemand` y `Hierarchy` (los
//     dos usan `OnDemandStrategy::build_index` y pueden fallar
//     en ese punto). El resto se añaden según se migren los
//     otros sub-commands.
//   - Tipos auxiliares (`HotPathEntry`, `PathHop`, `DepthBucket`,
//     `ComplexityMetrics`, `SkippedFile`) que dan forma a los
//     variants que los necesitan. Cada uno deriva `Serialize`
//     para que el JSON del bin (T1-5.5) los emita tal cual.
//
// Lo que T1-5.4 NO hace (y queda para T1-5.4b+):
//   - La partial-warning repetida en 6 sub-commands (HotPaths,
//     EntryPoints, LeafFunctions, TracePath, Mermaid, Complexity,
//     Impact) sigue siendo un `eprintln!` por ahora. Cuando se
//     migren esos variants, se introduce un
//     `GraphResult::PartialWarning { count, files: Vec<String> }`
//     o un campo opcional en cada variant. Decidirlo entonces,
//     no ahora — el pilot (PerFile) no emite esa warning.

use cognicode_core::domain::value_objects::Location;

/// A flat reference to a symbol in the graph (used by `EntryPoints`,
/// `LeafFunctions`, and as a step in path traces). The presenter
/// renders it as "name at file:line:column" or "name at file:line"
/// depending on context.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolRef {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// One row of the hot-paths table (Function, Fan-in, Fan-out,
/// Location). The presenter renders with `:<40` / `:>8` / `:>8`
/// formatting. `fan_in` / `fan_out` are usize to match the
/// underlying analyzer's `HotPath` struct (avoids a narrowing
/// conversion at the migration site).
#[derive(Debug, Clone, Serialize)]
pub struct HotPathEntry {
    pub name: String,
    pub fan_in: usize,
    pub fan_out: usize,
    pub file: String,
    pub line: u32,
}

/// A single step in a `TracePath` result.
#[derive(Debug, Clone, Serialize)]
pub struct PathHop {
    /// 1-indexed position in the path (matches the presenter's
    /// "{i}. {name} at {file}:{line}" output).
    pub position: u32,
    pub name: String,
    pub file: String,
    pub line: u32,
}

/// A bucket of entries at a given traversal depth, used by
/// `Hierarchy`. Every entry of the depth is published: the bucket used
/// to stop at five without saying so.
#[derive(Debug, Clone, Serialize)]
pub struct DepthBucket {
    pub depth: u32,
    pub entries: Vec<DepthEntry>,
}

/// One entry in a `DepthBucket`. `direction` is lowercase callers
/// or callees (matches `format!("{:?}", direction).to_lowercase()`
/// in the pre-migration code).
#[derive(Debug, Clone, Serialize)]
pub struct DepthEntry {
    pub name: String,
    pub direction: String,
    pub file: String,
    pub line: u32,
}

/// The 9 metrics emitted by `Complexity`. Each one maps 1:1 to a
/// line of the pre-migration output (e.g. "  Total symbols: {n}").
#[derive(Debug, Clone, Serialize)]
pub struct ComplexityMetrics {
    pub total_symbols: usize,
    pub total_edges: usize,
    pub max_depth: usize,
    pub cyclomatic_complexity: usize,
    pub high_fan_out_count: usize,
    pub medium_fan_out_count: usize,
    pub low_fan_out_count: usize,
    pub entry_point_count: usize,
    pub leaf_function_count: usize,
}

/// One file that the graph build skipped (e.g. parse error, I/O
/// error). Used by `Full` and (in a future T1-5.4b+) by the
/// shared `PartialWarning` channel.
#[derive(Debug, Clone, Serialize)]
pub struct SkippedFile {
    pub path: String,
    /// Debug-format of the underlying `SkipReason`; the pre-migration
    /// code uses `format!("{:?}", reason)`. The bin's JSON presenter
    /// (T1-5.5) can map this to a string for stable external contracts.
    pub reason: String,
}

/// Resultado exitoso de un `cognicode graph <subcommand>`.
///
/// 11 variants, uno por `GraphCommand`. El pilot de T1-5.4
/// (este commit) solo migra `PerFile`; los otros 10 se migran en
/// commits subsiguientes siguiendo el mismo flujo que T1-5.3c/d.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphResult {
    /// `cognicode graph on-demand <symbol> <depth> <direction> <path>`
    OnDemand {
        symbol: String,
        depth: u32,
        direction: String,
        elapsed_ms: u128,
        root_name: String,
        root_file: String,
        root_line: u32,
        root_column: u32,
        entries_len: usize,
    },
    /// `cognicode graph per-file <file>`
    /// Pilot de T1-5.4 (migrado en este commit).
    PerFile {
        file: String,
        symbols: usize,
        dependencies: usize,
        /// The references the file states and nobody declared.
        ///
        /// They travel beside the counts because they are part of the same
        /// answer: the counts say how much the graph holds, and these say
        /// what the file states that the graph does not hold. A surface
        /// that publishes the counts and stays silent about these says
        /// "this file states nothing" about a file that states plenty.
        pending_references: Vec<cognicode_core::interface::coverage::PendingReferenceInfo>,
    },
    /// `cognicode graph full [--rebuild] [--format text|json] <path>`
    /// El pre-migration code ya tenía un `FullGraphJson` ad-hoc; este
    /// variant lo absorbe y elimina la duplicación. El campo `status`
    /// es la palabra del estado: "complete", "partial", "unknown" o
    /// "unsupported". Un árbol sin nada que leer es "unsupported", y lo
    /// dice con la misma palabra que dice el servidor MCP.
    Full {
        path: String,
        rebuild: bool,
        elapsed_ms: u128,
        symbols: usize,
        dependencies: usize,
        status: String,
        skipped_files: Vec<SkippedFile>,
        /// The references the graph could not tie to a declaration.
        ///
        /// They travel beside `skipped_files` because they are the other
        /// half of the same question: a graph that read every file and
        /// tied every reference is complete, and a consumer has to be able
        /// to tell which of the two is missing.
        pending_references: Vec<cognicode_core::interface::coverage::PendingReferenceInfo>,
        /// The relations the repository refused to take.
        ///
        /// The `dependencies` count says how many relations the graph
        /// *holds*. A refused relation is not one of them, so it has to be
        /// named next to the count: otherwise the count reads as "every
        /// relation the build found".
        rejected_relations: Vec<cognicode_core::interface::coverage::RejectedRelationInfo>,
    },
    /// `cognicode graph hot-paths [--limit N] [--min-fan-in N] <path>`
    HotPaths {
        path: String,
        limit: usize,
        min_fan_in: usize,
        entries: Vec<HotPathEntry>,
        elapsed_ms: u128,
    },
    /// `cognicode graph entry-points <path>`
    /// `total` es el número completo de roots; `symbols` está
    /// truncado a 20 por el presenter (matches `iter().take(20)`).
    EntryPoints {
        path: String,
        symbols: Vec<SymbolRef>,
        total: usize,
        elapsed_ms: u128,
    },
    /// `cognicode graph leaf-functions <path>`
    /// Misma forma que `EntryPoints` (truncado a 20).
    LeafFunctions {
        path: String,
        symbols: Vec<SymbolRef>,
        total: usize,
        elapsed_ms: u128,
    },
    /// `cognicode graph trace-path <from> <to> <path>` — path found.
    TracePathFound {
        from: String,
        to: String,
        hops: Vec<PathHop>,
        elapsed_ms: u128,
    },
    /// `cognicode graph trace-path <from> <to> <path>` — no path.
    TracePathNotFound {
        from: String,
        to: String,
        elapsed_ms: u128,
    },
    /// `cognicode graph mermaid <path> [--format svg|png|txt]`
    /// `truncated` is `true` when the diagram exceeded 2000 chars.
    /// `diagram` is the full Mermaid source (the presenter slices
    /// it to 2000 chars when `truncated` is true; for svg/png
    /// it embeds the full source in a shell snippet regardless
    /// of length). The full content is in the data side because
    /// the text output must reproduce the diagram verbatim for
    /// downstream consumers (piping `cogh graph mermaid | head
    /// -30` to extract a preview, etc.).
    Mermaid {
        path: String,
        diagram: String,
        diagram_len: usize,
        /// The name the command published for the format that was asked
        /// for. It is the name the user wrote, and the published
        /// document keeps it.
        format: String,
        /// The format the graph publishes. It is not part of the
        /// published document: the name above is what the command
        /// published, and this is what the presenter branches on.
        #[serde(skip)]
        resolved_format: MermaidFormat,
        truncated: bool,
        elapsed_ms: u128,
    },
    /// `cognicode graph hierarchy <symbol> <depth> <direction> <path>`
    Hierarchy {
        symbol: String,
        depth: u32,
        direction: String,
        root_file: String,
        root_line: u32,
        root_column: u32,
        buckets: Vec<DepthBucket>,
        total_entries: usize,
        /// What the graph the answer was computed from covers.
        ///
        /// The entries are what the graph holds. Over a graph that does
        /// not cover the whole tree, that is not what the tree holds, and
        /// a reader that is not told so reads a zero as a finding.
        coverage: cognicode_core::interface::coverage::CoverageInfo,
        elapsed_ms: u128,
    },
    /// `cognicode graph complexity <path>`
    Complexity {
        path: String,
        metrics: ComplexityMetrics,
        elapsed_ms: u128,
    },
    /// `cognicode graph impact-composite <symbol> <path>`
    ///
    /// The composite impact query answers with a shape of its own —
    /// it is the query's own answer, not a summary of the graph — so
    /// the body travels as it is and the presenter writes it once.
    ///
    /// It used to travel twice: the core printed the answer and then
    /// returned an empty `PerFile` that the presenter printed after
    /// it, so the command published two documents where one was
    /// wanted.
    CompositeImpact {
        /// The answer, as the query produced it.
        body: String,
    },
    /// `cognicode graph impact <symbol> <path>`
    /// `impacted_symbols` / `impacted_files` están SIN truncar;
    /// el presenter los corta a 20 / 10 (matches el pre-migration).
    Impact {
        symbol: String,
        risk_level: String,
        direct_dependents: usize,
        total_impacted_symbols: usize,
        impacted_files_count: usize,
        impacted_symbols: Vec<String>,
        impacted_files: Vec<String>,
        /// The tests the change reaches, and the command that runs each one.
        ///
        /// Empty when no test reaches the change, and empty is the truth
        /// then: a change nothing tests has no test to name.
        affected_tests: Vec<cognicode_core::application::dto::AffectedTest>,
        /// What the graph the answer was computed from covers.
        ///
        /// The counts are what the graph holds. Over a graph that does
        /// not cover the whole tree, that is not what the tree holds, and
        /// a reader that is not told so reads a zero as a finding.
        coverage: cognicode_core::interface::coverage::CoverageInfo,
        elapsed_ms: u128,
    },
}

impl GraphResult {
    /// La respuesta del impacto, con las cifras que el grafo tiene.
    ///
    /// La respuesta se construye aquí y no en el comando: el comando sabe
    /// qué pregunta hacer, y esta forma es la que el consumidor lee. Las
    /// cifras son las del GRAFO —sobre un grafo que no cubre el árbol
    /// entero, no son las del árbol, y una respuesta que no lo dice lee un
    /// cero como un hallazgo—.
    pub fn impact_of(
        impact: cognicode_core::application::services::impact_of::ImpactOf,
        symbol: String,
        coverage: &cognicode_core::application::services::analyze::Coverage,
        elapsed_ms: u128,
    ) -> Self {
        GraphResult::Impact {
            symbol,
            risk_level: format!("{:?}", impact.risk).to_uppercase(),
            direct_dependents: impact.matched,
            total_impacted_symbols: impact.symbols.len(),
            impacted_files_count: impact.files.len(),
            impacted_symbols: impact.symbols,
            impacted_files: impact.files,
            affected_tests: impact.affected_tests,
            coverage: cognicode_core::interface::coverage::CoverageInfo::of(coverage),
            elapsed_ms,
        }
    }
}

/// Error tipado de un `cognicode graph <subcommand>`.
///
/// Solo la primera variante (`BuildIndex`) se usa en el pilot
/// (T1-5.4 cubre `OnDemand` y `Hierarchy`, los dos usan
/// `OnDemandStrategy::build_index`). Las otras variantes se
/// añaden cuando se migren los sub-commands correspondientes.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("Error building index at {path}: {source}")]
    BuildIndex {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Error building per-file graph for {file}: {source}")]
    PerFile {
        file: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Error building full graph at {path}: {source}")]
    Full {
        path: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

// ===================================================================
// T1-5.6a — `NavigateResult` + `NavigateError`
// ===================================================================
//
// Estado pre-T1-5.6a:
//   - `execute_navigate` (3 sub-commands: Definition, Hover, References)
//     emitted ~25 `println!` and ~3 `eprintln!` calls scattered across
//     its match arms.
//   - The text side and the data side were produced together inside
//     `execute_navigate`: each arm re-computed the same `file:line:column`
//     progress line from the parsed position, then branched on the LSP
//     outcome and printed either the "found" block or the "not found"
//     single-liner.
//   - The bin could not switch the navigate text to JSON; `format.json`
//     was silently ignored (the library printed its inline text and
//     returned).
//   - The `eprintln!("Error: {}", e)` in each arm sent the LSP error
//     message to stderr and bubbled it up via `Box<dyn Error>`. T1-5.6a
//     keeps the eprintln side-channel (T1-5.4 decision: partial-style
//     warnings stay on stderr) and bubbles the same error up via
//     `NavigateError::Backend` for the bin to format if it wants.
//
// Política T1-5.3 (sin cambios):
//   - La librería NO formatea. Solo emite datos.
//   - El bin (cognicode-cli) tiene el presenter (text + JSON).
//   - Los variants se diseñan pensando en JSON canónico
//     (`#[serde(tag = "kind")]`) para que T1-5.5 ya consolidó el
//     dispatch text/JSON. T1-5.6a solo añade una variante de
//     `CommandOutput` y dos funciones en el presenter.
//
// Lo que T1-5.6a aporta:
//   - `DefinitionLocation`, `HoverResult`, `ReferenceEntry`: tipos
//     auxiliares que dan forma a los variants. `ReferenceKind` se
//     re-exporta al inicio del módulo y se serializa vía su derive
//     (PascalCase Debug para `[KIND]` en el text path; mismo string
//     en el JSON path — `Read`, `Call`, `Import`, `Write`, `Type`).
//   - `NavigateResult`: 3 variants, uno por `NavigateCommand`. Cada
//     uno lleva `(file, line, column, workspace)` ya parseados para
//     reconstruir la progress line en el presenter; los datos del
//     resultado van en `location` / `info` / `references`.
//   - `NavigateError`: errores tipados con `#[source]`, paralelo a
//     `IndexError` / `GraphError`. `InvalidPosition` para el fallo
//     del parseo de `file:line:column`; `Backend` para los errores
//     que devuelve el LSP (CadeIntelligenceError envuelto en Box).
//
// Decisión de modelo (1-indexed line):
//   El pre-migration code almacena `Location { line: 0-indexed }` y
//   luego imprime `line + 1`. Para que el presenter no tenga que
//   hacer aritmética, el data side guarda `line: 1-indexed` (el
//   formato que ve el usuario en el CLI). El bin nunca ve el 0
//   interno — coincide con el contrato de `IndexResult::SymbolCode`
//   y de los snapshots de graph (que también llevan `line` 1-indexed).

/// Location devuelta por `cognicode navigate definition`.
///
/// `line` es 1-indexed (lo que el CLI imprime: el interno 0-indexed
/// de `Location::line()` se suma 1 en `execute_navigate` antes de
/// guardarse aquí). El presenter text emite
/// `  File:   {file}\n  Line:   {line}\n  Column: {column}\n` con la
/// indentación de 2 espacios y las anchuras pinneadas por el snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct DefinitionLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// Hover info devuelta por `cognicode navigate hover`.
///
/// `type_info` es el `content` del `HoverInfo` del LSP
/// (`fn() -> u32`, `i32`, etc.). `documentation` es opcional: cuando
/// es `None` el presenter text omite la línea `Docs:` (no la emite
/// vacía) y el JSON serializa `null`.
#[derive(Debug, Clone, Serialize)]
pub struct HoverResult {
    pub type_info: String,
    pub documentation: Option<String>,
}

/// Una referencia individual en `cognicode navigate references`.
///
/// `container` ya viene con la sustitución `(unknown)` aplicada
/// cuando el LSP devolvió `None` (el presenter text no necesita
/// conocer el `Option`); el caller lo decide en `execute_navigate`.
/// `kind` se serializa con su derive Debug (PascalCase).
#[derive(Debug, Clone, Serialize)]
pub struct ReferenceEntry {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub kind: ReferenceKind,
    pub container: String,
}

/// Resultado exitoso de un `cognicode navigate <subcommand>`.
///
/// `#[serde(tag = "kind", rename_all = "snake_case")]` da a cada
/// variant un discriminador (`"definition"`, `"hover"`, `"references"`)
/// que el bin switch-ea con un `match` en el presenter. Cada variant
/// lleva `file/line/column/workspace` ya parseados para reconstruir
/// la progress line en el text path (byte-identical con el
/// pre-migration), y el resultado del LSP va en `location` /
/// `info` / `references`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NavigateResult {
    /// `cognicode navigate definition <file>:<line>:<col>`.
    /// `location: Some` cuando el LSP devolvió una `Location`; `None`
    /// cuando devolvió `None` (no se encontró definición). El text
    /// path emite "Definition found:\n  File: ..." o
    /// "No definition found for {file}:{line}".
    Definition {
        file: String,
        line: u32,
        column: u32,
        workspace: String,
        location: Option<DefinitionLocation>,
    },
    /// `cognicode navigate hover <file>:<line>:<col>`.
    /// `info: Some` cuando el LSP devolvió `HoverInfo`; `None` cuando
    /// devolvió `None`. El text path emite "Hover information:\n  Type:
    /// ..." (+ "Docs:" si documentation está presente) o
    /// "No hover information found for {file}:{line}".
    Hover {
        file: String,
        line: u32,
        column: u32,
        workspace: String,
        info: Option<HoverResult>,
    },
    /// `cognicode navigate references <file>:<line>:<col>`.
    /// Vec vacía ⇒ "No references found for {file}:{line}"; no
    /// distinta variante (vs. `IndexResult::QueryEmpty`) porque la
    /// emptiness es esperada y no aporta semántica distinta.
    References {
        file: String,
        line: u32,
        column: u32,
        workspace: String,
        include_declaration: bool,
        references: Vec<ReferenceEntry>,
    },
}

/// Error tipado de un `cognicode navigate <subcommand>`.
///
/// `#[source]` preserva la chain para que el bin pueda formatear el
/// error (`"Navigate 'definition' failed for ...: {source}"`). El
/// error del LSP se bubble-a como `Box<dyn Error + Send + Sync>`
/// (mismo patrón que `GraphError::Full`).
#[derive(Debug, thiserror::Error)]
pub enum NavigateError {
    /// `parse_position` falló: el argumento `position` no tiene la
    /// forma `file:line:column`. El caller (CLI parser) no debería
    /// llegar aquí con errores tipo "empty string" porque clap lo
    /// rechaza antes; pero el `rsplitn(3, ':')` del parser puede
    /// recibir `foo:bar` (sin columna) o `foo:1:bar` (columna no
    /// numérica).
    #[error("Invalid position '{position}': expected file:line:column (e.g. src/main.rs:42:10)")]
    InvalidPosition { position: String },
    /// El LSP devolvió un error o el provider local no pudo resolver
    /// la posición. `operation` identifica el brazo ("definition" /
    /// "hover" / "references"). El `Box<dyn Error + Send + Sync>`
    /// preserva el `CodeIntelligenceError` original (downcasteable
    /// por el caller si quiere inspeccionar la variant).
    #[error("Navigate '{operation}' failed for {file}:{line}:{column} in {workspace}: {source}")]
    Backend {
        operation: &'static str,
        file: String,
        line: u32,
        column: u32,
        workspace: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

// Allow the data-side types from `infrastructure::graph` to flow
// into `GraphResult` without going through stringly-typed paths.
// (Currently unused — `Location` and `SymbolKind` are re-imported
// here so future WUs that build a `GraphResult::OnDemand` from a
// real `OnDemandStrategy::build_subgraph` can do so without
// re-importing. Cost: 2 lines; benefit: a single import site.)
#[allow(dead_code)]
fn _ensure_imports_used(_l: Location, _k: SymbolKind) {}
