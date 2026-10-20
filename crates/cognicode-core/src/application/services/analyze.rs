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
//! adaptador lo traduce a `Failed` con el mensaje real, sin convertirlo
//! en éxito con grafo vacío.

use std::time::Instant;

use crate::application::analysis_basis::{
    AnalysisBasis, AnalysisError, AnalysisOp, AnalysisOutcome, AnalysisValue, Analyze,
};
use crate::application::services::analysis_service::AnalysisService;

/// Adaptador de producción: `Analyze` sobre el `AnalysisService` real.
///
/// El servicio interno no es `Clone` (posee caches `Arc`), así que el
/// adaptador lo comparte por `Arc` para que la facade (y cualquier
/// consumidor) pueda reutilizar la misma instancia entre ops.
pub struct ServiceAnalyzer {
    service: std::sync::Arc<AnalysisService>,
}

impl ServiceAnalyzer {
    pub fn new() -> Self {
        Self {
            service: std::sync::Arc::new(AnalysisService::new()),
        }
    }

    /// Acceso al servicio interno para consumidores que necesitan ops
    /// aún no porteadas al ADT (transición; se retira cuando el puerto
    /// cubra el vocabulario completo).
    pub fn service(&self) -> &AnalysisService {
        &self.service
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

        let mut outcome = match op.clone() {
            AnalysisOp::GraphFull { path, rebuild } => self.graph_full(&path, rebuild),
            AnalysisOp::GraphComplexity { path } => self.graph_complexity(&path),
            other => AnalysisOutcome::unsupported(&format!(
                "op `{}` aún no está cableada al adaptador de producción (CU-1.2 cubre graph.full y graph.complexity)",
                other.operation_id()
            )),
        };

        let provenance =
            AnalysisProvenance::new(workspace, op.operation_id(), capability_for_op(&op))
                .with_elapsed_ms(started.elapsed().as_millis());

        outcome = outcome.with_provenance(provenance);
        outcome
    }
}

use crate::application::analysis_basis::AnalysisProvenance;

impl ServiceAnalyzer {
    fn graph_full(&self, path: &std::path::Path, rebuild: bool) -> AnalysisOutcome<AnalysisValue> {
        use crate::application::analysis_basis::GraphValue;

        if rebuild {
            if let Err(e) = self.service.build_full_graph(path) {
                return AnalysisOutcome::failed(AnalysisError::io_failure(&format!(
                    "build_full_graph falló sobre {}: {e:?}",
                    path.display()
                )));
            }
        } else if self.service.get_project_graph().symbol_count() == 0 {
            // Sin rebuild explícito y sin caché: construir una vez.
            if let Err(e) = self.service.build_full_graph(path) {
                return AnalysisOutcome::failed(AnalysisError::io_failure(&format!(
                    "build_full_graph falló sobre {}: {e:?}",
                    path.display()
                )));
            }
        }

        let graph = self.service.get_project_graph();
        if graph.symbol_count() == 0 {
            // El build "exitoso" con cero símbolos es ausencia de
            // material, no éxito: SPEC-CU-001.5 prohíbe Complete vacío.
            return AnalysisOutcome::failed(AnalysisError::io_failure(&format!(
                "workspace {} no produjo símbolos (sin material Rust analizable)",
                path.display()
            )));
        }

        // CU-1.4 (paridad con MCP): el estado del build lo decide el
        // BuildReport compartido, no la mera existencia de símbolos.
        // Un walk con ficheros saltados es Partial con gaps = ficheros
        // saltados — exactamente lo que handle_build_graph reporta. Sin
        // esto, CLI/SDK decían "complete" mientras MCP decía "partial"
        // sobre el MISMO workspace (divergencia real detectada por el
        // UAT-CU-1 del 2026-10-16).
        if let Some(gaps) = self.partial_gaps_from_last_build() {
            let stats = self.service.get_graph_stats();
            return AnalysisOutcome::partial(
                AnalysisValue::Graph(GraphValue::Full {
                    path: path.to_path_buf(),
                    symbols: stats.symbol_count,
                    dependencies: stats.edge_count,
                    elapsed_ms: 0,
                }),
                gaps,
            );
        }

        let stats = self.service.get_graph_stats();
        AnalysisOutcome::complete(AnalysisValue::Graph(GraphValue::Full {
            path: path.to_path_buf(),
            symbols: stats.symbol_count,
            dependencies: stats.edge_count,
            elapsed_ms: 0, // el elapsed real va en la provenance
        }))
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
        if graph.symbol_count() == 0 {
            return AnalysisOutcome::failed(AnalysisError::io_failure(&format!(
                "workspace {} no produjo símbolos (sin material Rust analizable)",
                path.display()
            )));
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
        // BuildStatus→gaps vive SOLO en `partial_gaps_from_last_build`
        // (H0.2): añadir otra copia en una tercera op reintroduciría la
        // divergencia CLI/SDK/MCP que UAT-CU-1 detectó el 2026-10-16.
        if let Some(gaps) = self.partial_gaps_from_last_build() {
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

    /// Traducción ÚNICA de `BuildReport` → `Vec<AnalysisError>` (gaps) en
    /// este adaptador. Devuelve `Some(gaps)` si y solo si el último
    /// build del service fue `BuildStatus::Partial` con al menos un
    /// skip. Cualquier op del adaptador que necesite reportar Partial
    /// CONSULTA este helper — no copia la traducción. Si el helper
    /// cambiara, todas las ops cambian a la vez, y la paridad
    /// CLI/SDK/MCP queda garantizada por construcción, no por
    /// disciplina.
    fn partial_gaps_from_last_build(&self) -> Option<Vec<AnalysisError>> {
        let report = self.service.get_last_build_report()?;
        let crate::infrastructure::graph::per_file_graph::BuildStatus::Partial { skipped, .. } =
            report.status
        else {
            return None;
        };
        if skipped.is_empty() {
            return None;
        }
        Some(
            skipped
                .iter()
                .map(|s| {
                    AnalysisError::gap(
                        "graph",
                        &format!("{}: {}", s.path, skip_reason_text(&s.reason)),
                    )
                })
                .collect(),
        )
    }
}

/// Texto estable para el motivo de skip, paralelo al `reason_kind` del
/// DTO MCP. Proyección explícita: el ADT de infraestructura no se
/// filtra al contrato de aplicación.
fn skip_reason_text(reason: &crate::infrastructure::graph::per_file_graph::SkipReason) -> String {
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
