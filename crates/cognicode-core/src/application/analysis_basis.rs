//! B2.W1 — Common execution model for analysis operations.
//!
//! Estado pre-B2.W1:
//!   - CLI dispatch devolvia `Result<CommandOutput, _>` donde solo
//!     `Index` y `Graph` tenian variantes tipadas; el resto caia
//!     en `Other` (println! inline).
//!   - MCP handlers (handle_build_graph, handle_get_file_symbols,
//!     ...) no compartian modelo: cada uno tenia su firma propia y
//!     su manejo de error.
//!   - El envelope `assurance-evidence/v1` llevaba
//!     `CapabilityCompleteness` con el vocabulario
//!     (Complete|Partial|Unknown|Unsupported) pero solo a nivel
//!     del envelope, no como contrato de aplicacion.
//!
//! Lo que aporta este modulo:
//!   - `AnalysisBasis`: parametros de entrada normalizados
//!     (workspace root canonicalizado, perfil del consumidor,
//!     budget, cancelacion).
//!   - `AnalysisOp`: discriminante de operacion (Index, Graph,
//!     Navigate, ...) con sus parametros tipados por variante.
//!   - `AnalysisOutcome<T>`: resultado tipado con un
//!     `OutcomeState` que reusa el vocabulario de
//!     `CapabilityCompleteness` y le anade `Failed` (que el
//!     envelope assurance-evidence/v1 no tiene porque un envelope
//!     con Failed no se serializa).
//!   - `AnalysisProvenance`: procedencia minima
//!     (workspace + operacion + capability + elapsed_ms).
//!   - `AnalysisError`: un tipo error-shape con variantes para
//!     gap / unsupported / io_failure / cancellation / generic,
//!     serializable para que el bin/MCP emitan contexto al
//!     consumer sin perder semantica.
//!
//! Politica B2.W1 (de `odd/tasks/b2-shared-analysis-capability-contracts.md`):
//!   - El contrato es UNA trait con UN metodo, no un service
//!     locator global. Ver `Analyze` abajo.
//!   - Reutiliza `WorkspaceSession` y `AnalysisService`; no
//!     introduce un nuevo motor.
//!   - El dominio NO depende de este modulo (solo lo consume la
//!     capa de aplicacion y los adapters CLI/MCP).
//!   - El envelope `assurance-evidence/v1` se queda con su
//!     `CapabilityCompleteness` propio: la conversion entre ese
//!     y `AnalysisOutcome` ocurre en el adapter, no aqui.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Normalizacion sintactica: resuelve `.` y `..` por componente
/// sin tocar el disco. Esta es la unica normalizacion que el
/// pipeline `AnalysisBasis::for_workspace` puede aplicar a paths
/// que aun no existen (e.g. el directorio de un workspace que se
/// va a indexar por primera vez).
///
/// Mantener este normalizador en un sitio y pequeno evita
/// introducir una dependencia nueva (`path-clean` o similar)
/// solo para resolver `..` en paths nuevos.
fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    // Ya estamos en la raiz; mantenemos `..` para
                    // no perderlo (preserva el caso de paths
                    // absolutos que escapan del cwd).
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Parametros de entrada normalizados para cualquier operacion
/// de analisis. La idea es que CLI y MCP produzcan el mismo
/// `AnalysisBasis` antes de delegar, de modo que la cache y los
/// budgets se compartan sin que cada adapter tenga que inventar
/// su propia version del "workspace + opciones".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisBasis {
    workspace_root: PathBuf,
    profile: ConsumerProfile,
    budget: Budget,
}

impl AnalysisBasis {
    /// Construye un basis canonicalizando el workspace root.
    ///
    /// Pipeline de normalizacion:
    ///  1. Normalizacion sintactica: resuelve `.` y `..` por
    ///     componente, sin tocar el disco. Esto garantiza que
    ///     `./a/../b` y `b` produzcan el mismo basis aunque el
    ///     path no exista aun.
    ///  2. Absolutize: si el path es relativo, se resuelve
    ///     contra el cwd.
    ///  3. canonicalize: si el path existe, sigue symlinks y
    ///     vuelve a resolver `.`/`..` desde el disco (cubre
    ///     el caso en que un componente intermedio es un symlink).
    ///     Si no existe, se devuelve el resultado de (1)+(2),
    ///     porque muchas operaciones corren sobre workspaces
    ///     que aun no tienen archivos (e.g. `index build` sobre
    ///     un directorio vacio).
    pub fn for_workspace(workspace_root: PathBuf) -> Self {
        let normalized = normalize_path(&workspace_root);
        // Step 2: absolutize against the cwd. The empty-after-normalize
        // case (".", "", "./..") is anchored at the cwd directly because
        // `std::path::absolute("")` returns the empty path unchanged on
        // stable Rust, which would otherwise produce a relative basis
        // and break the cache key. When the input is a non-empty
        // relative path, prefer `std::path::absolute` and fall back to
        // `cwd.join(normalized)` if the syscall layer is unavailable
        // (sandbox without a working dir, e.g. chroot scenarios).
        let absolute = if normalized.as_os_str().is_empty() {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
        } else if normalized.is_absolute() {
            normalized
        } else {
            std::path::absolute(&normalized).unwrap_or_else(|_| {
                std::env::current_dir()
                    .map(|cwd| cwd.join(&normalized))
                    .unwrap_or(normalized)
            })
        };
        let workspace_root = if absolute.exists() {
            absolute.canonicalize().unwrap_or(absolute)
        } else {
            absolute
        };
        Self {
            workspace_root,
            profile: ConsumerProfile::default(),
            budget: Budget::default(),
        }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn profile(&self) -> ConsumerProfile {
        self.profile
    }

    pub fn budget(&self) -> Budget {
        self.budget
    }

    pub fn with_profile(mut self, profile: ConsumerProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }
}

/// Perfil del consumidor. Hoy solo dos valores: `Human` (CLI
/// interactivo) y `Agent` (MCP server). Esto pinea que las
/// capabilities reportadas (B2.W2) sepan para quien formatean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsumerProfile {
    #[default]
    Human,
    Agent,
}

/// Budget de ejecucion. Por defecto sin limite, pero el caller
/// puede fijar `max_elapsed` o `max_entities` para evitar que
/// una operacion bloquee al consumer. Los servicios que ya
/// soportan cancelacion (AnalysisService::build_project_graph_async)
/// lo consultan; los que no lo hacen en este WU pueden
/// ignorarlo — el budget se pinea por el test, no por fuerza.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Budget {
    pub max_elapsed: Option<Duration>,
    pub max_entities: Option<usize>,
}

/// Estado del resultado. Replica el vocabulario de
/// `CapabilityCompleteness` y le anade `Failed` (que el envelope
/// no necesita porque un envelope con Failed no se serializa,
/// pero un AnalysisOutcome SI puede serializar el estado Failed
/// para que el consumer sepa que la operacion fallo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutcomeState {
    Complete,
    Partial,
    Unknown,
    Unsupported,
    Failed,
}

/// Resultado tipado de una operacion de analisis.
///
/// Pineamos constructores que assertan invariantes (gaps no
/// vacios para Partial, reason no vacio para Unsupported, error
/// no vacio para Failed). El panic es intencional: el caller
/// que construya un Partial sin gaps o un Unsupported sin reason
/// tiene un bug semantico que NO se puede expresar fielmente en
/// el tipo. La serializacion incluye `value` solo cuando hay
/// dato; los `gaps`/`reason`/`error` solo cuando aplican.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisOutcome<T> {
    state: OutcomeState,
    value: Option<T>,
    gaps: Vec<AnalysisError>,
    reason: Option<String>,
    error: Option<AnalysisError>,
    provenance: AnalysisProvenance,
}

impl<T> AnalysisOutcome<T> {
    pub fn complete(value: T) -> Self {
        Self {
            state: OutcomeState::Complete,
            value: Some(value),
            gaps: Vec::new(),
            reason: None,
            error: None,
            provenance: AnalysisProvenance::default(),
        }
    }

    pub fn partial(value: T, gaps: Vec<AnalysisError>) -> Self {
        assert!(
            !gaps.is_empty(),
            "Partial expone al menos un gap; sin gaps es Complete"
        );
        Self {
            state: OutcomeState::Partial,
            value: Some(value),
            gaps,
            reason: None,
            error: None,
            provenance: AnalysisProvenance::default(),
        }
    }

    pub fn unknown() -> Self {
        Self {
            state: OutcomeState::Unknown,
            value: None,
            gaps: Vec::new(),
            reason: None,
            error: None,
            provenance: AnalysisProvenance::default(),
        }
    }

    /// An outcome that carries an answer but not a verdict.
    ///
    /// The value is there —the graph was built, and the consumer can read
    /// it— but the question "do you know everything?" has no answer, and
    /// answering "complete" would invent one. Not measuring and measuring
    /// nothing are two different things.
    pub fn unknown_with(value: T) -> Self {
        Self {
            state: OutcomeState::Unknown,
            value: Some(value),
            gaps: Vec::new(),
            reason: None,
            error: None,
            provenance: AnalysisProvenance::default(),
        }
    }

    pub fn unsupported(reason: &str) -> Self {
        assert!(
            !reason.trim().is_empty(),
            "Unsupported requiere reason no vacio"
        );
        Self {
            state: OutcomeState::Unsupported,
            value: None,
            gaps: Vec::new(),
            reason: Some(reason.to_string()),
            error: None,
            provenance: AnalysisProvenance::default(),
        }
    }

    pub fn failed(error: AnalysisError) -> Self {
        assert!(
            !error.message.trim().is_empty(),
            "Failed requiere error no vacio"
        );
        Self {
            state: OutcomeState::Failed,
            value: None,
            gaps: Vec::new(),
            reason: None,
            error: Some(error),
            provenance: AnalysisProvenance::default(),
        }
    }

    pub fn with_provenance(mut self, provenance: AnalysisProvenance) -> Self {
        self.provenance = provenance;
        self
    }

    pub fn state(&self) -> OutcomeState {
        self.state
    }

    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn gaps(&self) -> &[AnalysisError] {
        &self.gaps
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    pub fn error(&self) -> Option<&AnalysisError> {
        self.error.as_ref()
    }

    pub fn provenance(&self) -> &AnalysisProvenance {
        &self.provenance
    }

    /// `true` cuando el outcome lleva un valor util
    /// (Complete o Partial). False para Unknown / Unsupported /
    /// Failed. El bin CLI y el MCP server usan esto para
    /// decidir si emiten el campo `value` en el JSON shape.
    pub fn is_producing(&self) -> bool {
        matches!(self.state, OutcomeState::Complete | OutcomeState::Partial)
    }
}

/// Errores parciales / no fatales que un AnalysisOutcome puede
/// llevar dentro. El `kind` discrimina: `gap` (faltan datos
/// parciales), `unsupported` (la operacion no aplica), `io`
/// (fallo recuperable), `cancelled` (el budget detuvo), `generic`
/// (cualquier otro). El `message` se emite al consumer tal cual.
/// The capability of the gap that says a file could not be read.
pub const SKIPPED_FILE_CAPABILITY: &str = "graph";

/// The capability of the gap that says a reference could not be tied to a
/// declaration.
///
/// The two coverages are two gaps with two capabilities, because they are
/// two facts: a presenter that reads one of them as the other tells the
/// consumer half the truth.
pub const PENDING_REFERENCE_CAPABILITY: &str = "graph.pending";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisError {
    pub kind: AnalysisErrorKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub capability: Option<String>,
}

impl AnalysisError {
    pub fn gap(capability: &str, message: &str) -> Self {
        Self {
            kind: AnalysisErrorKind::Gap,
            message: message.to_string(),
            capability: Some(capability.to_string()),
        }
    }

    pub fn unsupported(message: &str) -> Self {
        Self {
            kind: AnalysisErrorKind::Unsupported,
            message: message.to_string(),
            capability: None,
        }
    }

    pub fn io_failure(message: &str) -> Self {
        Self {
            kind: AnalysisErrorKind::Io,
            message: message.to_string(),
            capability: None,
        }
    }

    pub fn cancelled(message: &str) -> Self {
        Self {
            kind: AnalysisErrorKind::Cancelled,
            message: message.to_string(),
            capability: None,
        }
    }

    pub fn generic(message: &str) -> Self {
        Self {
            kind: AnalysisErrorKind::Generic,
            message: message.to_string(),
            capability: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisErrorKind {
    Gap,
    Unsupported,
    Io,
    Cancelled,
    Generic,
}

/// Procedencia minima de un AnalysisOutcome. La idea es que el
/// consumer (CLI humano o MCP agent) pueda correlacionar el
/// resultado con la operacion que lo pidio, el workspace sobre
/// el que corrio, y la capability que se intento calcular.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisProvenance {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub workspace: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub capability: Option<String>,
    #[serde(default)]
    pub elapsed_ms: u128,
}

impl AnalysisProvenance {
    pub fn new(workspace: PathBuf, operation: &str, capability: &str) -> Self {
        Self {
            workspace: Some(workspace),
            operation: Some(operation.to_string()),
            capability: Some(capability.to_string()),
            elapsed_ms: 0,
        }
    }

    pub fn with_elapsed_ms(mut self, elapsed_ms: u128) -> Self {
        self.elapsed_ms = elapsed_ms;
        self
    }
}

/// Trait del contrato comun de ejecucion. UNA trait, UN metodo,
/// tal como pide `odd/tasks/b2-shared-analysis-capability-contracts.md`
/// §Riesgos ("El contrato debe ser UNA funcion/trait, no una
/// indireccion").
///
/// W1 aporta la firma; W2+ la conecta a las operaciones reales.
/// El impl concreto vive en `application::services::analyze` (B2.W2+).
pub trait Analyze: Send + Sync {
    fn execute(&self, basis: &AnalysisBasis, op: AnalysisOp) -> AnalysisOutcome<AnalysisValue>;
}

/// Discriminante de operacion. Replica el vocabulario de
/// `CliCommand` y `Tool` (MCP) sin acoplarse a clap/rmcp: el
/// adapter traduce su propia representacion a `AnalysisOp` y
/// delega en `Analyze::execute`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisOp {
    IndexBuild {
        path: PathBuf,
        strategy: String,
    },
    IndexQuery {
        symbol: String,
        path: PathBuf,
    },
    IndexOutline {
        file: PathBuf,
    },
    IndexSymbolCode {
        file: PathBuf,
        line: u32,
        column: u32,
    },
    GraphFull {
        path: PathBuf,
        rebuild: bool,
    },
    GraphOnDemand {
        symbol: String,
        depth: usize,
    },
    GraphPerFile {
        file: PathBuf,
    },
    GraphHotPaths {
        path: PathBuf,
        limit: usize,
        min_fan_in: usize,
    },
    GraphEntryPoints {
        path: PathBuf,
    },
    GraphLeafFunctions {
        path: PathBuf,
    },
    GraphTracePath {
        from: String,
        to: String,
        max_depth: usize,
    },
    GraphMermaid {
        path: PathBuf,
        format: String,
    },
    GraphHierarchy {
        symbol: String,
        depth: usize,
        direction: String,
    },
    GraphComplexity {
        path: PathBuf,
    },
    GraphImpact {
        symbol: String,
    },
    NavigateHover {
        file: PathBuf,
        line: u32,
        column: u32,
    },
    NavigateDefinition {
        file: PathBuf,
        line: u32,
        column: u32,
    },
    NavigateReferences {
        file: PathBuf,
        line: u32,
        column: u32,
    },
    FindUsages {
        symbol: String,
        scope: Option<PathBuf>,
    },
    /// Captura generica para operaciones que B2.W1 aun no
    /// especializa. La primera WU en cubrirla es B2.W3
    /// (respuestas estructuradas); mientras tanto el
    /// `Analyze` impl puede devolver `Unsupported`.
    Other(String),
}

impl AnalysisOp {
    pub fn operation_id(&self) -> &'static str {
        match self {
            Self::IndexBuild { .. } => "index.build",
            Self::IndexQuery { .. } => "index.query",
            Self::IndexOutline { .. } => "index.outline",
            Self::IndexSymbolCode { .. } => "index.symbol_code",
            Self::GraphFull { .. } => "graph.full",
            Self::GraphOnDemand { .. } => "graph.on_demand",
            Self::GraphPerFile { .. } => "graph.per_file",
            Self::GraphHotPaths { .. } => "graph.hot_paths",
            Self::GraphEntryPoints { .. } => "graph.entry_points",
            Self::GraphLeafFunctions { .. } => "graph.leaf_functions",
            Self::GraphTracePath { .. } => "graph.trace_path",
            Self::GraphMermaid { .. } => "graph.mermaid",
            Self::GraphHierarchy { .. } => "graph.hierarchy",
            Self::GraphComplexity { .. } => "graph.complexity",
            Self::GraphImpact { .. } => "graph.impact",
            Self::NavigateHover { .. } => "navigate.hover",
            Self::NavigateDefinition { .. } => "navigate.definition",
            Self::NavigateReferences { .. } => "navigate.references",
            Self::FindUsages { .. } => "find_usages",
            Self::Other(_) => "other",
        }
    }
}

/// Valor generico que cruza del analysis al adapter. Replica
/// el shape de `IndexResult` y `GraphResult` (ya serializables)
/// pero como enum de primer nivel para que el adapter decida
/// como envolverlo en su representacion nativa (CommandOutput
/// o JSON-RPC).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnalysisValue {
    Index(IndexValue),
    Graph(GraphValue),
    /// Valor textual estructurado (lo que actualmente sale por
    /// `println!` inline en execute_navigate, execute_doctor,
    /// etc.). B2.W3+ lo migrara a DTOs tipados.
    Text {
        summary: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IndexValue {
    Build {
        path: PathBuf,
        strategy: String,
        elapsed_ms: u128,
    },
    Query {
        symbol: String,
        path: PathBuf,
        locations: serde_json::Value,
    },
    QueryEmpty {
        symbol: String,
        path: PathBuf,
    },
    Outline {
        file: PathBuf,
        symbols: serde_json::Value,
    },
    SymbolCode {
        file: PathBuf,
        line: u32,
        column: u32,
        code: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphValue {
    Full {
        path: PathBuf,
        symbols: usize,
        dependencies: usize,
        elapsed_ms: u128,
        /// The references the build could not tie to a declaration.
        ///
        /// They travel with the value because the consumer asks one question
        /// — "do you know everything?" — and the answer has to say which
        /// references it does not know, in the same place where it says what
        /// it does know.
        pending_references: Vec<crate::domain::value_objects::PendingReference>,
    },
    OnDemand {
        symbol: String,
        depth: usize,
        entries_len: usize,
    },
    PerFile {
        file: PathBuf,
        symbols: usize,
        dependencies: usize,
    },
    HotPaths {
        path: PathBuf,
        entries: serde_json::Value,
    },
    EntryPoints {
        path: PathBuf,
        total: usize,
    },
    LeafFunctions {
        path: PathBuf,
        total: usize,
    },
    TracePathFound {
        from: String,
        to: String,
        hops: serde_json::Value,
    },
    TracePathNotFound {
        from: String,
        to: String,
    },
    Mermaid {
        path: PathBuf,
        diagram: String,
        truncated: bool,
    },
    Hierarchy {
        symbol: String,
        depth: usize,
        total_entries: usize,
    },
    Complexity {
        path: PathBuf,
        metrics: serde_json::Value,
    },
    Impact {
        symbol: String,
        risk_level: String,
        direct_dependents: usize,
        total_impacted_symbols: usize,
    },
}
