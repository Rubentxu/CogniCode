//! CU-1.2 — Facade contract: `ConsumerAnalysis` resolves capabilities
//! and executes through the existing `Analyze` port.
//!
//! Estado pre-CU-1.2 (caracterización 2026-10-16):
//!   - `AnalysisBasis`, `AnalysisOutcome<T>`, `AnalysisOp` y el trait
//!     `Analyze` existen desde B2.W1, pero el trait NO tiene ninguna
//!     implementación de producción: CLI (`CommandExecutor`) y MCP
//!     (handlers) invocan servicios directamente, cada canal arma su
//!     propio plan.
//!   - No existe ningún tipo `CapabilityRequest`/`CapabilityPlan`: el
//!     roadmap CU-1.2 exige resolver una petición de capacidades en un
//!     plan tipado con `Complete/Partial/Unsupported` ANTES de ejecutar.
//!   - El consumidor Guard (`guard-cognicode::RealCalculator`) consume
//!     `AnalysisService` crudo (build_full_graph + get_project_graph),
//!     sin ADT de resultado: `EmptyGraph` llega como error stringly.
//!
//! Lo que cierra este test (leyes de la facade):
//!   1. La resolución de capacidades es PURA: una petición sobre un
//!      workspace sin backend de grafo produce `CapabilityPlan::Partial`
//!      con gaps no vacíos; nunca `Complete` con grafo vacío (SPEC-CU-001.5).
//!   2. La ejecución pasa SIEMPRE por el puerto `Analyze`: la facade no
//!      llama `AnalysisService` directamente; recibe el `&dyn Analyze`
//!      inyectado (hexagonal, B2.W1 contract).
//!   3. `AnalysisOutcome` se reusa tal cual: la facade NO introduce un
//!      segundo ADT de resultado (SPEC-CU-001.2 "no duplicar ese ADT").
//!   4. La versión del proveedor se expone por metadatos originados en
//!      build real (`AnalysisProvenance`), no en `CARGO_PKG_VERSION` del
//!      consumidor (SPEC-CU-001.6).
//!
//! Política: igual que b2_w1_analysis_outcome_contract.rs, este
//! contrato vive como test de integración de `cognicode-core` porque
//! pinea la superficie pública `application::consumer_analysis`.

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisOp, AnalysisOutcome, AnalysisValue, Analyze,
};
use cognicode_core::application::consumer_analysis::{
    CapabilityPlan, CapabilityRequest, ConsumerAnalysisFacade,
};
use std::path::PathBuf;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Spy: registra las operaciones que cruzan el puerto. La facade debe
// delegar aquí; si llama AnalysisService directamente el spy no registra
// nada y las aserciones de delegación fallan.
// ---------------------------------------------------------------------------
struct SpyAnalyzer {
    calls: Mutex<Vec<&'static str>>,
    fail_graph_full: bool,
}

impl SpyAnalyzer {
    fn recording() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            fail_graph_full: false,
        }
    }

    fn failing_graph_full() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            fail_graph_full: true,
        }
    }

    fn operation_ids(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl Analyze for SpyAnalyzer {
    fn execute(&self, _basis: &AnalysisBasis, op: AnalysisOp) -> AnalysisOutcome<AnalysisValue> {
        self.calls.lock().unwrap().push(op.operation_id());
        match op {
            AnalysisOp::GraphFull { .. } if self.fail_graph_full => {
                AnalysisOutcome::failed(AnalysisError::io_failure("workspace has no Rust sources"))
            }
            AnalysisOp::GraphFull { path, .. } => {
                AnalysisOutcome::complete(AnalysisValue::Graph(GraphValue::Full {
                    path,
                    symbols: 3,
                    dependencies: 2,
                    elapsed_ms: 12,
                    pending_references: vec![],
                }))
            }
            AnalysisOp::GraphComplexity { path } => {
                AnalysisOutcome::complete(AnalysisValue::Graph(GraphValue::Complexity {
                    path,
                    metrics: serde_json::json!({"total_functions": 3}),
                }))
            }
            _ => AnalysisOutcome::unsupported("spy only implements graph ops"),
        }
    }
}

use cognicode_core::application::analysis_basis::{AnalysisError, GraphValue};

// ---------------------------------------------------------------------------
// Ley 1: resolución pura — una capability de grafo sobre un workspace sin
// backend de grafo NUNCA resuelve Complete. Sin material para ninguna
// capability pedida el plan es Unsupported; con material parcial es
// Partial con gaps que nombran la capability (SPEC-CU-001.5).
// ---------------------------------------------------------------------------

#[test]
fn capability_plan_marks_graph_unavailable_as_partial_with_gaps() {
    let basis = AnalysisBasis::for_workspace(PathBuf::from("/definitely/not/a/rust/workspace"));
    let request = CapabilityRequest::for_graph(&basis);

    let plan = ConsumerAnalysisFacade::resolve(&request);

    assert!(
        !matches!(plan, CapabilityPlan::Complete { .. }),
        "un workspace sin fuentes Rust no puede resolver Complete; SPEC-CU-001.5"
    );
    match plan {
        CapabilityPlan::Unsupported { reason } => {
            assert!(!reason.trim().is_empty());
        }
        CapabilityPlan::Partial { gaps, .. } => {
            assert!(!gaps.is_empty(), "Partial exige gaps no vacíos");
            assert!(
                gaps.iter().any(|g| g.contains("graph")),
                "el gap debe nombrar la capability de grafo, got: {gaps:?}"
            );
        }
        other => panic!("esperaba Unsupported o Partial, obtuve {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Ley 2: la ejecución delega en el puerto Analyze — mismo op, mismo basis.
// ---------------------------------------------------------------------------

#[test]
fn execute_delegates_through_the_analyze_port() {
    let basis = AnalysisBasis::for_workspace(PathBuf::from("."));
    let request = CapabilityRequest::for_graph(&basis);
    let spy = SpyAnalyzer::recording();
    let facade = ConsumerAnalysisFacade::new(&spy);

    let outcome = facade.analyze(
        &request,
        AnalysisOp::GraphFull {
            path: PathBuf::from("."),
            rebuild: false,
        },
    );

    assert_eq!(outcome.state(), OutcomeState::Complete);
    assert_eq!(spy.operation_ids(), vec!["graph.full"]);
}

// ---------------------------------------------------------------------------
// Ley 3: el resultado es AnalysisOutcome reusado, no un segundo ADT.
// (Compilación: la signatura devuelve AnalysisOutcome<AnalysisValue>.)
// ---------------------------------------------------------------------------

#[test]
fn analyze_returns_the_upstream_analysis_outcome_verbatim() {
    let basis = AnalysisBasis::for_workspace(PathBuf::from("."));
    let request = CapabilityRequest::for_graph(&basis);
    let spy = SpyAnalyzer::recording();
    let facade = ConsumerAnalysisFacade::new(&spy);

    let outcome: AnalysisOutcome<AnalysisValue> = facade.analyze(
        &request,
        AnalysisOp::GraphFull {
            path: PathBuf::from("."),
            rebuild: false,
        },
    );

    // El valor llega intacto desde el puerto; la facade no lo re-envuelve.
    match outcome.value() {
        Some(AnalysisValue::Graph(GraphValue::Full { symbols, .. })) => {
            assert_eq!(*symbols, 3);
        }
        other => panic!("esperaba GraphValue::Full intacto, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Ley 4: failed del puerto se propaga como Failed — la facade no convierte
// fallo en Complete/Unknown silencioso (never upcast).
// ---------------------------------------------------------------------------

#[test]
fn facade_does_not_mask_port_failure_as_success() {
    let basis = AnalysisBasis::for_workspace(PathBuf::from("."));
    let request = CapabilityRequest::for_graph(&basis);
    let spy = SpyAnalyzer::failing_graph_full();
    let facade = ConsumerAnalysisFacade::new(&spy);

    let outcome = facade.analyze(
        &request,
        AnalysisOp::GraphFull {
            path: PathBuf::from("."),
            rebuild: false,
        },
    );

    assert_eq!(outcome.state(), OutcomeState::Failed);
    assert!(outcome.error().is_some());
}

use cognicode_core::application::analysis_basis::OutcomeState;

// ---------------------------------------------------------------------------
// Ley 5 (SPEC-CU-001.6): la provenance expone la versión del proveedor
// desde el build real del core, no desde el consumidor.
// ---------------------------------------------------------------------------

#[test]
fn provenance_carries_provider_version_not_consumer_version() {
    let provider_version = ConsumerAnalysisFacade::provider_version();
    assert!(
        !provider_version.is_empty(),
        "provider_version no puede ser vacío"
    );
    // El provider es cognicode-core: su versión es la del workspace, la
    // misma que publica el binario. Derivarla del propio Cargo.toml en
    // vez de pinear un literal: un literal caduca en cada bump de
    // release (rompió exactamente así en el bump 0.101.17→0.101.18 de
    // D1) y lo que la ley exige es IDENTIDAD con el workspace, no un
    // número concreto.
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
    let ws = std::fs::read_to_string(manifest).expect("leer Cargo.toml del workspace");
    let expected = ws
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("version = ")
                .map(|v| v.trim().trim_matches('"').to_string())
        })
        .expect("Cargo.toml del workspace declara versión");
    assert_eq!(
        provider_version, expected,
        "provider_version debe ser la versión del workspace ({expected})"
    );
}

// ---------------------------------------------------------------------------
// Ley 6: request inválido se rechaza en construcción (smart constructor).
// ---------------------------------------------------------------------------

#[test]
fn capability_request_rejects_empty_workspace_root() {
    let result = CapabilityRequest::new(PathBuf::from(""));
    assert!(result.is_err(), "workspace raíz vacío debe rechazarse");
}
