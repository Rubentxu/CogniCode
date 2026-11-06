//! El coste de un reindex incremental, y la palabra que nombra su rebuild.
//!
//! Un reindex incremental hace cinco cosas: lee el estado del árbol
//! (scan), establece la identidad de cada fichero candidato (hash),
//! reinterpreta los que cambiaron (parse), escribe lo que saca de ellos
//! (index) y reconstruye el grafo (graph). Cada fase tiene un dueño, y
//! el dueño es quien la mide: el tiempo se toma donde el trabajo ocurre,
//! no se estima en el que lo cuenta.
//!
//! El coste se publica SIEMPRE. Es la respuesta del producto a la
//! pregunta "¿cuánto costó esto?", y una respuesta que sólo aparece
//! cuando hay un colector de métricas encendido es una respuesta que no
//! existe para la mayoría de los consumidores.
//!
//! Y ningún valor sin medir se publica como cero: un percentil sin
//! observaciones es `null`, y una memoria que la plataforma no publica
//! es `null`.
//!
//! Las palabras viven aquí, en la capa de aplicación, porque son del
//! producto y no del MCP: la CLI, el MCP y el SDK publican los mismos
//! hechos sobre el mismo trabajo.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::application::services::analysis_service::AnalysisService;

/// The phases an incremental reindex runs.
///
/// The order of the variants is the order the phases run in, and it is
/// the order they are published in: a consumer reads the same list
/// every time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReparsePhase {
    /// Reading the state the operation needs: the manifest, and the
    /// stamp of every candidate path.
    Scan,
    /// Establishing the identity of each candidate file: its content
    /// against the content the manifest recorded.
    Hash,
    /// Interpreting the files whose content changed.
    Parse,
    /// Writing the facts into the index and into the graph store.
    Index,
    /// Rebuilding the graph.
    Graph,
}

impl ReparsePhase {
    /// The word this phase is published with.
    ///
    /// The word is the owner's: the type names the phase and this
    /// method is the only place that spells it out, so a consumer and
    /// the product cannot drift apart.
    pub fn published_label(&self) -> &'static str {
        match self {
            ReparsePhase::Scan => "scan",
            ReparsePhase::Hash => "hash",
            ReparsePhase::Parse => "parse",
            ReparsePhase::Index => "index",
            ReparsePhase::Graph => "graph",
        }
    }
}

/// What kind of rebuild the operation performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebuildKind {
    /// Nothing was interpreted again: the graph already said it.
    CacheHit,
    /// Every candidate file was interpreted again.
    Full,
    /// Some files were interpreted again and the rest were reused.
    Partial,
    /// The rebuild failed. What was indexed before is still there.
    Error,
}

impl RebuildKind {
    /// The word this kind is published with.
    pub fn published_label(&self) -> &'static str {
        match self {
            RebuildKind::CacheHit => "cache_hit",
            RebuildKind::Full => "full",
            RebuildKind::Partial => "partial",
            RebuildKind::Error => "error",
        }
    }
}

/// How long one phase took.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseCost {
    /// The phase.
    pub phase: ReparsePhase,
    /// The time it took, in milliseconds.
    pub duration_ms: u64,
}

/// The cost of one incremental reindex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReparseCost {
    /// What kind of rebuild this was.
    pub rebuild_kind: RebuildKind,
    /// The phases that ran, with the time each one took.
    ///
    /// A phase that did not run is not in the list: the list says what
    /// was measured, and a phase that never started has no duration to
    /// publish. The phases are in the order they run.
    pub phases: Vec<PhaseCost>,
    /// The files the operation acted on: the ones it read again and the
    /// ones it removed.
    pub affected_files: usize,
    /// The files the operation considered.
    pub considered_files: usize,
    /// The whole operation, in milliseconds.
    pub total_ms: u64,
    /// The peak resident memory of the process, in bytes.
    ///
    /// `None` when the platform does not publish it: an unmeasured
    /// memory is not a zero.
    pub peak_memory_bytes: Option<u64>,
    /// The median cost of the recent operations of this session.
    ///
    /// `None` when there are not enough observations to have a median.
    pub duration_p50_ms: Option<u64>,
    /// The high cost of the recent operations of this session.
    ///
    /// `None` when there are not enough observations to have a
    /// percentile.
    pub duration_p95_ms: Option<u64>,
    /// How many operations the percentiles rest on.
    pub sample_size: usize,
}

/// What kind of rebuild these numbers describe.
///
/// The word is decided by the product's own numbers, never by a label
/// somebody wrote next to the call: the files the rebuild interpreted
/// again, and the ones it was supposed to cover.
///
/// * `failed` — the rebuild did not finish: the error.
/// * nothing interpreted again — the graph already said it: a cache hit.
/// * every file interpreted again — a full rebuild.
/// * some files interpreted again — a partial rebuild.
pub fn rebuild_kind_of(
    rebuild_ran: bool,
    failed: bool,
    parsed_files: usize,
    total_files: usize,
) -> RebuildKind {
    if failed {
        return RebuildKind::Error;
    }
    if !rebuild_ran || parsed_files == 0 {
        return RebuildKind::CacheHit;
    }
    if parsed_files >= total_files {
        return RebuildKind::Full;
    }
    RebuildKind::Partial
}

/// The percentiles of a window of observations.
///
/// The nearest-rank method: the p-th percentile of `n` sorted
/// observations is the observation at rank `ceil(p/100 * n)`.
///
/// A window with fewer than two observations has no percentiles at all.
/// A median of one observation is that observation, and publishing it
/// as a median would dress a single measurement up as a distribution.
pub fn percentiles(window: &[u64]) -> (Option<u64>, Option<u64>) {
    if window.len() < 2 {
        return (None, None);
    }
    let mut sorted: Vec<u64> = window.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    (Some(sorted[rank(50, n)]), Some(sorted[rank(95, n)]))
}

/// The zero-based index of the nearest rank of a percentile.
fn rank(percentile: usize, n: usize) -> usize {
    let at = (percentile * n).div_ceil(100);
    at.saturating_sub(1).min(n - 1)
}

/// How many operations the window remembers.
pub const COST_WINDOW_CAPACITY: usize = 64;

/// The percentiles of a window, and how many observations they rest on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CostSummary {
    /// The median.
    pub p50_ms: Option<u64>,
    /// The high percentile.
    pub p95_ms: Option<u64>,
    /// How many operations were observed.
    pub sample_size: usize,
}

/// The recent costs of one session.
///
/// The window is bounded: it remembers the last [`COST_WINDOW_CAPACITY`]
/// operations and forgets the rest. A percentile over a window that
/// grows without bound is a percentile over the history of the session,
/// which is a different question from "what does this operation cost
/// now?".
#[derive(Debug, Clone, Default)]
pub struct CostWindow {
    samples: VecDeque<u64>,
}

impl CostWindow {
    /// Records the cost of one operation, forgetting the oldest when the
    /// window is full.
    pub fn record(&mut self, duration_ms: u64) {
        if self.samples.len() >= COST_WINDOW_CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(duration_ms);
    }

    /// The percentiles of the window, and how many observations they
    /// rest on.
    pub fn summary(&self) -> CostSummary {
        let samples: Vec<u64> = self.samples.iter().copied().collect();
        let (p50_ms, p95_ms) = percentiles(&samples);
        CostSummary {
            p50_ms,
            p95_ms,
            sample_size: self.samples.len(),
        }
    }

    /// The observations the window holds, oldest first.
    pub fn samples(&self) -> Vec<u64> {
        self.samples.iter().copied().collect()
    }
}

impl AnalysisService {
    /// Records what one operation of this session cost.
    ///
    /// The window belongs to the session and not to the process: a
    /// percentile over the operations of every workspace of a server is
    /// a percentile over a question nobody asked.
    pub fn record_reparse_cost(&self, duration_ms: u64) -> CostSummary {
        let mut window = self.reparse_costs.lock().unwrap();
        window.record(duration_ms);
        window.summary()
    }

    /// The percentiles of the recent operations of this session.
    pub fn reparse_cost_summary(&self) -> CostSummary {
        self.reparse_costs.lock().unwrap().summary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La palabra del rebuild la deciden las cifras, en los cuatro
    /// casos que un rebuild puede tener.
    #[test]
    fn the_rebuild_kind_is_decided_by_the_numbers() {
        assert_eq!(
            rebuild_kind_of(false, false, 0, 10),
            RebuildKind::CacheHit,
            "nada cambió: no hubo rebuild, y lo que el grafo ya decía sigue siendo lo que dice"
        );
        assert_eq!(
            rebuild_kind_of(true, false, 0, 10),
            RebuildKind::CacheHit,
            "el rebuild corrió pero no interpretó ningún fichero: el grafo ya lo tenía"
        );
        assert_eq!(
            rebuild_kind_of(true, false, 10, 10),
            RebuildKind::Full,
            "se interpretaron todos los ficheros: rebuild completo"
        );
        assert_eq!(
            rebuild_kind_of(true, false, 1, 10),
            RebuildKind::Partial,
            "se interpretó un fichero y se reutilizaron los otros nueve: rebuild parcial"
        );
        assert_eq!(
            rebuild_kind_of(true, true, 1, 10),
            RebuildKind::Error,
            "el rebuild falló: el error es lo que pasó, y no un rebuild parcial"
        );
    }

    /// Un rebuild sobre un árbol sin ficheros candidatos no es un
    /// rebuild completo: no se interpretó nada.
    #[test]
    fn a_rebuild_over_an_empty_tree_is_not_a_full_rebuild() {
        assert_eq!(
            rebuild_kind_of(true, false, 0, 0),
            RebuildKind::CacheHit,
            "un árbol sin ficheros candidatos no tiene nada que reinterpretar"
        );
    }

    /// El percentil de una ventana con menos de dos observaciones no
    /// existe, y no se publica como cero.
    #[test]
    fn a_window_without_observations_has_no_percentiles() {
        assert_eq!(
            percentiles(&[]),
            (None, None),
            "una ventana vacía no tiene percentiles"
        );
        assert_eq!(
            percentiles(&[42]),
            (None, None),
            "una sola observación es una medición, no una distribución"
        );
    }

    /// Con dos o más observaciones, los percentiles se leen de la
    /// ventana, y no dependen del orden en que llegaron.
    #[test]
    fn the_percentiles_come_from_the_window() {
        let (p50, p95) = percentiles(&[10, 20, 30, 40]);
        assert_eq!(
            p50,
            Some(20),
            "la mediana de cuatro observaciones es la segunda"
        );
        assert_eq!(
            p95,
            Some(40),
            "el percentil 95 de cuatro observaciones es la última"
        );
        assert_eq!(
            percentiles(&[40, 30, 20, 10]),
            (Some(20), Some(40)),
            "el orden de llegada no cambia el percentil"
        );
    }

    /// La ventana recuerda las últimas operaciones y olvida las viejas.
    #[test]
    fn the_window_forgets_the_oldest_observation_when_it_is_full() {
        let mut window = CostWindow::default();
        for _ in 0..COST_WINDOW_CAPACITY {
            window.record(1);
        }
        window.record(1000);
        let summary = window.summary();
        assert_eq!(
            summary.sample_size, COST_WINDOW_CAPACITY,
            "la ventana no crece"
        );
        assert_eq!(
            summary.p50_ms,
            Some(1),
            "la observación nueva no borra a las viejas"
        );
    }
}
