//! CU-1.2 — Implementación de producción del puerto `Analyze` sobre
//! `AnalysisService`. Este es el hueco que B2.W2 dejó abierto: el trait
//! existía desde B2.W1 pero ningún código de producción lo implementaba.
//!
//! Cobertura deliberadamente acotada (YAGNI por WU): las ops de grafo
//! que el consumidor Guard y la facade necesitan hoy (`GraphFull`,
//! `GraphComplexity`). Cada op adicional llega con su test. Las ops no
//! cubiertas devuelven `Unsupported` explícito — nunca panic, nunca
//! `Unknown` silencioso, nunca `Complete` vacío.
//!
//! Semántica de fallo honesto (SPEC-CU-001.5): `build_full_graph`
//! sobre un workspace sin material Rust devuelve error upstream; el
//! adaptador lo traduce a `Unsupported` con el motivo real, sin
//! convertirlo en éxito con grafo vacío. `Unsupported` es lo que la
//! especificación nombra para un proveedor que no puede crear un grafo,
//! y es el estado que el servidor MCP ya publica sobre el mismo
//! directorio: un error escondería el motivo de quien solo lee
//! respuestas con éxito.

use std::time::Instant;

use crate::application::analysis_basis::{
    AnalysisBasis, AnalysisError, AnalysisOp, AnalysisOutcome, AnalysisValue, Analyze, GraphSource,
    OutcomeState,
};
use crate::application::services::analysis_service::AnalysisService;

/// The reason a workspace has nothing to analyze.
///
/// A build that walked the tree and came back with no symbols is not a
/// complete analysis: it is the absence of material. The sentence lives
/// here, written once, because three surfaces answer this question about
/// the same project —the SDK and the CLI on one side, the MCP server on the
/// other— and a consumer that gets two different answers for the same
/// directory cannot tell whether the product or the caller is wrong.
pub fn no_material_reason(path: &std::path::Path) -> String {
    format!(
        "workspace {} no produjo símbolos (sin material Rust analizable)",
        path.display()
    )
}

/// Whether a workspace with this many symbols holds anything this
/// product can read.
///
/// The fact lives here once. The SDK adapter and the MCP server ask the
/// same question about the same directory, and two answers for one tree
/// is how the CLI and the MCP came to disagree about it: the CLI said
/// "unknown" where the MCP said "unsupported".
pub fn holds_material(symbol_count: usize) -> bool {
    symbol_count > 0
}

/// The answer for a workspace that holds nothing to read.
///
/// It is a state and not an error: the walk ran, the tree is there, and
/// there is simply nothing in it that this product can read.
/// SPEC-CU-001.5 names `Unsupported` for a provider that cannot build a
/// graph, and an error would hide the reason from a caller that only
/// reads successful payloads.
pub fn no_material_outcome(path: &std::path::Path) -> AnalysisOutcome<AnalysisValue> {
    AnalysisOutcome::unsupported(&no_material_reason(path))
}

/// The three holes a build can leave open.
///
/// They are three facts and they are told apart: a file the walk could not
/// process, a reference it could not tie to a declaration, and a relation
/// the repository refused. A build that left none of them open is complete.
#[derive(Debug, Clone, Default)]
pub struct CoverageFacts {
    /// The files the walk could not process, with the reason.
    pub skipped: Vec<crate::infrastructure::graph::per_file_graph::SkippedFile>,
    /// The references that could not be tied to a declaration.
    pub pending: Vec<crate::domain::value_objects::PendingReference>,
    /// The relations the repository refused.
    pub rejected: Vec<crate::domain::value_objects::RejectedRelation>,
}

impl CoverageFacts {
    /// Whether the build left nothing open.
    pub fn is_complete(&self) -> bool {
        self.skipped.is_empty() && self.pending.is_empty() && self.rejected.is_empty()
    }

    /// The facts a build report states.
    fn from_report(report: &crate::infrastructure::graph::per_file_graph::BuildReport) -> Self {
        match &report.status {
            crate::infrastructure::graph::per_file_graph::BuildStatus::Complete => Self::default(),
            crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                skipped,
                pending,
                rejected,
            } => Self {
                skipped: skipped.clone(),
                pending: pending.clone(),
                rejected: rejected.clone(),
            },
        }
    }
}

/// The verdict on a coverage: the word it is published with and the facts
/// that hold it up.
#[derive(Debug, Clone)]
pub enum Coverage {
    /// The tree holds nothing this product can read: the walk ran and there
    /// was nothing in it.
    NoMaterial(CoverageFacts),
    /// The answer covers the tree it was measured against.
    Complete,
    /// Nobody measured the coverage of this answer.
    Unmeasured,
    /// The answer covers the tree, except for the holes it names.
    Partial(CoverageFacts),
}

impl Coverage {
    /// The word this coverage is published with.
    ///
    /// One owner for the vocabulary: the CLI, the MCP and the SDK publish
    /// the same word for the same coverage, and a word that means two things
    /// is a lie the consumer cannot see.
    pub fn published_state(&self) -> OutcomeState {
        match self {
            Coverage::NoMaterial(_) => OutcomeState::Unsupported,
            Coverage::Complete => OutcomeState::Complete,
            Coverage::Unmeasured => OutcomeState::Unknown,
            Coverage::Partial(_) => OutcomeState::Partial,
        }
    }

    /// The holes this coverage has.
    ///
    /// A coverage that is complete has none: that is what makes it
    /// complete.
    pub fn facts(&self) -> CoverageFacts {
        match self {
            Coverage::NoMaterial(facts) | Coverage::Partial(facts) => facts.clone(),
            _ => CoverageFacts::default(),
        }
    }
}

/// The coverage of an answer, decided in one place.
///
/// The CLI and the MCP answer the same question about the same tree, so
/// they ask it the same function: two rules that look alike are two rules,
/// and the second one goes stale while the first one is kept.
///
/// The inputs are the facts both surfaces have. `source` says where the
/// answer came from; `report` is the report of the build of THIS call —
/// `None` when the answer was not built here; `holes` are the holes the
/// tree has NOW; and `holds_material` says whether the tree holds anything
/// this product can read.
///
/// The rules, in one place:
///
/// - an answer that was not built here is measured by the holes the tree
///   has now, and a snapshot only exists for a complete build, so its
///   references and relations are the empty ones;
/// - an answer built here is measured by its report, and a build that left
///   nothing open is `Complete`;
/// - an answer whose coverage nobody measured is `Unmeasured`: measuring
///   nothing and finding nothing are not the same thing;
/// - a tree with nothing to read is `NoMaterial`: the walk ran and there
///   was nothing in it.
pub fn coverage_verdict(
    source: GraphSource,
    report: Option<&crate::infrastructure::graph::per_file_graph::BuildReport>,
    holes: &[crate::infrastructure::graph::per_file_graph::SkippedFile],
    holds_material: bool,
) -> Coverage {
    let facts = if source != GraphSource::Built {
        // The answer was not built here, so its report is not ours: what
        // the tree has NOW is what it is measured against. A hole is not
        // one of the files the graph was built from, so the manifest never
        // knew about it — and a tree with a hole is partial, the same
        // verdict a fresh walk of it would give.
        CoverageFacts {
            skipped: holes.to_vec(),
            pending: Vec::new(),
            rejected: Vec::new(),
        }
    } else {
        match report {
            Some(report) => CoverageFacts::from_report(report),
            None => CoverageFacts::default(),
        }
    };

    if !holds_material {
        return Coverage::NoMaterial(facts);
    }

    // Nobody measured this coverage: the answer does not say "complete" —
    // it says "unknown", and the consumer knows it.
    if source == GraphSource::Built && report.is_none() {
        return Coverage::Unmeasured;
    }

    if facts.is_complete() {
        Coverage::Complete
    } else {
        Coverage::Partial(facts)
    }
}

/// Adaptador de producción: `Analyze` sobre el `AnalysisService` real.
///
/// El servicio interno no es `Clone` (posee caches `Arc`), así que el
/// adaptador lo comparte por `Arc` para que la facade (y cualquier
/// consumidor) pueda reutilizar la misma instancia entre ops.
pub struct ServiceAnalyzer {
    service: std::sync::Arc<AnalysisService>,
    /// Where the product keeps what it calculates.
    ///
    /// `None` when the caller names nowhere: the analyzer then calculates
    /// and keeps nothing, and says so in the log. A consumer that names
    /// the cache gets the snapshot the MCP also serves, and what one of
    /// them calculates the other one serves.
    cache_root: Option<std::path::PathBuf>,
}

impl ServiceAnalyzer {
    pub fn new() -> Self {
        Self {
            service: std::sync::Arc::new(AnalysisService::new()),
            cache_root: None,
        }
    }

    /// The analyzer that keeps what it calculates in `cache_root`.
    ///
    /// The cache root is a parameter and not a constant because the
    /// product's cache is the user's, and a test that wrote into it would
    /// leave entries nobody can clean. The CLI names the user's cache; a
    /// test names its own.
    pub fn with_cache_root(cache_root: impl Into<Option<std::path::PathBuf>>) -> Self {
        Self {
            service: std::sync::Arc::new(AnalysisService::new()),
            cache_root: cache_root.into(),
        }
    }

    /// Acceso al servicio interno para consumidores que necesitan ops
    /// aún no porteadas al ADT (transición; se retira cuando el puerto
    /// cubra el vocabulario completo).
    pub fn service(&self) -> &AnalysisService {
        &self.service
    }

    /// The analyzer that asks `oracle` whether the graph it holds is
    /// still fresh.
    ///
    /// The oracle is a port: the analyzer asks it a question and takes the
    /// answer. Without one the analyzer is exactly the one there was, and
    /// the product decides the freshness by itself.
    pub fn with_oracle(
        mut self,
        oracle: Option<std::sync::Arc<dyn crate::application::freshness::FreshnessOracle>>,
    ) -> Self {
        self.service = std::sync::Arc::new(AnalysisService::new().with_freshness_oracle(oracle));
        self
    }
}

impl Default for ServiceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyze for ServiceAnalyzer {
    fn execute(&self, basis: &AnalysisBasis, op: AnalysisOp) -> AnalysisOutcome<AnalysisValue> {
        let started = Instant::now();
        let workspace = basis.workspace_root().to_path_buf();

        // The source travels with the answer: a graph calculated now and a
        // graph served from the snapshot are the same answer only when the
        // tree did not change, and the consumer is the one who has to know
        // that the product checked.
        let (mut outcome, source) = match op.clone() {
            AnalysisOp::GraphFull { path, rebuild } => self.graph_full(&path, rebuild),
            AnalysisOp::GraphComplexity { path } => (self.graph_complexity(&path), None),
            other => (
                AnalysisOutcome::unsupported(&format!(
                    "op `{}` aún no está cableada al adaptador de producción (CU-1.2 cubre graph.full y graph.complexity)",
                    other.operation_id()
                )),
                None,
            ),
        };

        let mut provenance =
            AnalysisProvenance::new(workspace, op.operation_id(), capability_for_op(&op))
                .with_elapsed_ms(started.elapsed().as_millis());
        if let Some(source) = source {
            provenance = provenance.with_graph_source(source);
        }

        outcome = outcome.with_provenance(provenance);
        outcome
    }
}

use crate::application::analysis_basis::AnalysisProvenance;

impl ServiceAnalyzer {
    /// El grafo del workspace, servido del snapshot cuando se puede y
    /// calculado cuando no.
    ///
    /// La decisión es una sola y es la del producto: la función
    /// `load_or_build_graph` la toma, y el MCP toma la misma. Lo que uno de
    /// ellos calcula, el otro lo sirve.
    ///
    /// `rebuild` es la exigencia del consumidor de leer el arbol otra vez,
    /// y es lo que `--rebuild` significa en la CLI.
    fn graph_full(
        &self,
        path: &std::path::Path,
        rebuild: bool,
    ) -> (AnalysisOutcome<AnalysisValue>, Option<GraphSource>) {
        use crate::application::analysis_basis::GraphValue;

        let loaded = match crate::application::graph_snapshot::load_or_build_graph(
            &self.service,
            path,
            self.cache_root.as_deref(),
            None,
            rebuild,
        ) {
            Ok(loaded) => loaded,
            Err(e) => {
                return (
                    AnalysisOutcome::failed(AnalysisError::io_failure(&format!(
                        "build_full_graph falló sobre {}: {e:?}",
                        path.display()
                    ))),
                    None,
                );
            }
        };
        let source = Some(loaded.source);

        // CU-1.4 (paridad con MCP): el estado del build lo decide el
        // BuildReport compartido, no la mera existencia de símbolos.
        // Un walk con ficheros saltados es Partial con gaps = ficheros
        // saltados — exactamente lo que handle_build_graph reporta. Sin
        // esto, CLI/SDK decían "complete" mientras MCP decía "partial"
        // sobre el MISMO workspace (divergencia real detectada por el
        // UAT-CU-1 del 2026-10-16).
        //
        // La decisión es de un dueño: `coverage_verdict` la toma, y la
        // toma UNA vez —al cargar el grafo—, así que la CLI, el MCP y las
        // preguntas publishes la misma cobertura para el mismo árbol. Las
        // tres coberturas son tres hechos y los tres cuentan: un fichero
        // que no se pudo leer, una referencia que no se pudo atar y una
        // relación que el almacén rechazó son respuestas distintas a "lo
        // sabes todo?". La referencia y la relación rechazada viajan con el
        // valor —donde el consumidor lee lo que sí se resolvió— y el gap
        // dice qué cobertura no está completa.
        //
        // Un grafo servido del snapshot no tiene BuildReport: este proceso
        // no lo construyó. Sus huecos son los del arbol de AHORA —un
        // hueco no es uno de los ficheros del grafo, asi que el manifiesto
        // no lo ve— y sus referencias y relaciones son las del grafo
        // servido, que son vacias porque un snapshot solo existe para un
        // build completo.
        let coverage = loaded.coverage;

        let stats = self.service.get_graph_stats();
        let facts = coverage.facts();
        let value = AnalysisValue::Graph(GraphValue::Full {
            path: path.to_path_buf(),
            symbols: stats.symbol_count,
            dependencies: stats.edge_count,
            elapsed_ms: 0, // el elapsed real va en la provenance
            pending_references: facts.pending,
            rejected_relations: facts.rejected,
        });

        match coverage {
            // El build "exitoso" con cero símbolos es ausencia de
            // material, no éxito: SPEC-CU-001.5 prohíbe Complete vacío.
            Coverage::NoMaterial(_) => (no_material_outcome(path), None),
            Coverage::Complete => (AnalysisOutcome::complete(value), source),
            // No hay reporte: no se midio nada. Eso no es lo mismo que
            // medir y no encontrar nada, asi que la respuesta no dice
            // "complete": dice "unknown", y el consumidor lo sabe.
            Coverage::Unmeasured => (AnalysisOutcome::unknown_with(value), source),
            Coverage::Partial(facts) => {
                let gaps = coverage_gaps(&facts.skipped, &facts.pending, &facts.rejected);
                (AnalysisOutcome::partial(value, gaps), source)
            }
        }
    }

    fn graph_complexity(&self, path: &std::path::Path) -> AnalysisOutcome<AnalysisValue> {
        use crate::application::analysis_basis::GraphValue;

        // Complejidad global del grafo: reusa el grafo cacheado; si no
        // hay, construye. Una métrica global no recalcula el análisis
        // por símbolo (SPEC-CU-001.4).
        if self.service.get_project_graph().symbol_count() == 0
            && let Err(e) = self.service.build_full_graph(path)
        {
            return AnalysisOutcome::failed(AnalysisError::io_failure(&format!(
                "build_full_graph falló sobre {}: {e:?}",
                path.display()
            )));
        }
        let graph = self.service.get_project_graph();
        if !holds_material(graph.symbol_count()) {
            return no_material_outcome(path);
        }

        // Complejidad por símbolo sobre el grafo real: el analyzer puro
        // (CallGraphAnalyzer) ya existe como proyección; lo usamos sin
        // recorrer el workspace por símbolo. El report de dominio no es
        // Serialize (y no debe serlo por un consumidor): proyectamos sus
        // campos a JSON explícitamente.
        let analyzer = crate::domain::services::CallGraphAnalyzer::new();
        let report = analyzer.calculate_complexity(&graph);
        let metrics = serde_json::json!({
            "total_symbols": report.total_symbols,
            "total_edges": report.total_edges,
            "cyclomatic_complexity": report.cyclomatic_complexity,
            "max_depth": report.max_depth,
            "entry_point_count": report.entry_point_count,
            "leaf_function_count": report.leaf_function_count,
        });

        // H0.2 (delivery-first-radical, package integration): el Partial
        // lo decide el BuildReport compartido, no la op. Si el walk fue
        // Partial, devolvemos Partial con los mismos gaps que
        // graph_full sobre el mismo workspace, llevando la métrica
        // calculada sobre el grafo parcial como valor. La traducción
        // BuildStatus→gaps vive SOLO en `partial_coverage` y
        // `coverage_gaps` (H0.2): añadir otra copia en una tercera op
        // reintroduciría la divergencia CLI/SDK/MCP que UAT-CU-1 detectó
        // el 2026-10-16.
        let (skipped, pending, rejected) = self.partial_coverage().unwrap_or_default();
        let gaps = coverage_gaps(&skipped, &pending, &rejected);
        if !gaps.is_empty() {
            return AnalysisOutcome::partial(
                AnalysisValue::Graph(GraphValue::Complexity {
                    path: path.to_path_buf(),
                    metrics,
                }),
                gaps,
            );
        }

        AnalysisOutcome::complete(AnalysisValue::Graph(GraphValue::Complexity {
            path: path.to_path_buf(),
            metrics,
        }))
    }

    /// Las tres coberturas que el último build no alcanzó.
    ///
    /// El servicio contesta con tres hechos que no se deducen uno del otro:
    /// los ficheros que el recorrido no pudo leer, las referencias que no
    /// se pudieron atar a una declaración, y las relaciones que el almacén
    /// rechazó. Un build que no dejó ninguno de los tres huecos está
    /// completo.
    fn partial_coverage(
        &self,
    ) -> Option<(
        Vec<crate::infrastructure::graph::per_file_graph::SkippedFile>,
        Vec<crate::domain::value_objects::PendingReference>,
        Vec<crate::domain::value_objects::RejectedRelation>,
    )> {
        let report = self.service.get_last_build_report()?;
        match report.status {
            crate::infrastructure::graph::per_file_graph::BuildStatus::Partial {
                skipped,
                pending,
                rejected,
            } => Some((skipped, pending, rejected)),
            crate::infrastructure::graph::per_file_graph::BuildStatus::Complete => {
                Some((Vec::new(), Vec::new(), Vec::new()))
            }
        }
    }
}

/// Los gaps que dicen qué cobertura no está completa.
///
/// Cada cobertura dice la suya: un gap por fichero que no se pudo leer, un
/// gap por la cobertura de referencias y un gap por la de relaciones.
/// Un consumidor que lee los gaps sabe así cuál falta, y no sólo que falta
/// algo.
fn coverage_gaps(
    skipped: &[crate::infrastructure::graph::per_file_graph::SkippedFile],
    pending: &[crate::domain::value_objects::PendingReference],
    rejected: &[crate::domain::value_objects::RejectedRelation],
) -> Vec<AnalysisError> {
    let mut gaps: Vec<AnalysisError> = skipped
        .iter()
        .map(|s| {
            AnalysisError::gap(
                crate::application::analysis_basis::SKIPPED_FILE_CAPABILITY,
                &format!("{}: {}", s.path, skip_reason_text(&s.reason)),
            )
        })
        .collect();
    if !pending.is_empty() {
        gaps.push(AnalysisError::gap(
            crate::application::analysis_basis::PENDING_REFERENCE_CAPABILITY,
            &format!("{} reference(s) without a declaration", pending.len()),
        ));
    }
    if !rejected.is_empty() {
        gaps.push(AnalysisError::gap(
            crate::application::analysis_basis::REJECTED_RELATION_CAPABILITY,
            &format!("{} relation(s) refused by the repository", rejected.len()),
        ));
    }
    gaps
}

/// Texto estable para el motivo de skip, paralelo al `reason_kind` del
/// DTO MCP. Proyección explícita: el ADT de infraestructura no se
/// filtra al contrato de aplicación.
///
/// `pub(crate)` porque la proyección que publica la cobertura lo usa
/// también: el motivo que se publica tiene que ser el mismo en las dos
/// superficies, y dos textos para un motivo son dos verdades.
pub(crate) fn skip_reason_text(
    reason: &crate::infrastructure::graph::per_file_graph::SkipReason,
) -> String {
    match reason {
        crate::infrastructure::graph::per_file_graph::SkipReason::Read(d) => {
            format!("read error: {d}")
        }
        crate::infrastructure::graph::per_file_graph::SkipReason::Parse(d) => {
            format!("parse error: {d}")
        }
        crate::infrastructure::graph::per_file_graph::SkipReason::UnsupportedExtension(d) => {
            format!("unsupported extension: {d}")
        }
        crate::infrastructure::graph::per_file_graph::SkipReason::Symlink { target } => {
            format!("symlink to {target}, not followed")
        }
        crate::infrastructure::graph::per_file_graph::SkipReason::Walk(d) => {
            format!("walk error: {d}")
        }
        crate::infrastructure::graph::per_file_graph::SkipReason::Other(d) => {
            format!("other: {d}")
        }
    }
}

/// Capability de la op, para provenance.
fn capability_for_op(op: &AnalysisOp) -> &'static str {
    match op {
        AnalysisOp::GraphFull { .. }
        | AnalysisOp::GraphOnDemand { .. }
        | AnalysisOp::GraphPerFile { .. }
        | AnalysisOp::GraphHotPaths { .. }
        | AnalysisOp::GraphEntryPoints { .. }
        | AnalysisOp::GraphLeafFunctions { .. }
        | AnalysisOp::GraphTracePath { .. }
        | AnalysisOp::GraphMermaid { .. }
        | AnalysisOp::GraphHierarchy { .. }
        | AnalysisOp::GraphComplexity { .. }
        | AnalysisOp::GraphImpact { .. } => "graph",
        AnalysisOp::IndexBuild { .. }
        | AnalysisOp::IndexQuery { .. }
        | AnalysisOp::IndexOutline { .. }
        | AnalysisOp::IndexSymbolCode { .. } => "index",
        AnalysisOp::NavigateHover { .. }
        | AnalysisOp::NavigateDefinition { .. }
        | AnalysisOp::NavigateReferences { .. } => "navigate",
        AnalysisOp::FindUsages { .. } => "find_usages",
        AnalysisOp::Other(_) => "other",
    }
}
