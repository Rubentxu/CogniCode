//! CU-2.3 — Contrato de transferencia CogniCode → Guard.
//!
//! Este modulo es la **puerta** entre la proyeccion tecnica de
//! CogniCode y la admision de Guard. NO transfiere autoridad:
//!
//! - No firma nada.
//! - No fabrica `admitted_at` ni `AdmitterRole::HumanPromoter`.
//! - No convierte `Advisory` o `Inconclusive` en algo gateable.
//!
//! ## Contrato (SPEC-CU-003.3, .4, .5, .7)
//!
//! `to_guard_observation` es una funcion pura, determinista, sin
//! I/O, que mapea `Result<FindingProjection, ProjectionError>` a
//! `GuardObservation`:
//!
//! - `Verified` -> `Verifiable` (Guard debe verificar admission
//!   contra su trust root antes de promover a `Block`).
//! - `Advisory` -> `Advisory` (consultivo, no gateable).
//! - `Inconclusive` -> `Inconclusive` (con gaps visibles).
//! - `Unsupported(reason)` -> `Unsupported(reason)` (fail-closed).
//! - `Inconclusive(gaps)` (ProjectionError) -> `Inconclusive` con gaps.
//! - `Failed(message)` (ProjectionError) -> `Failed` (no promovido).
//!
//! ## No-goals
//!
//! - No es un comando. Es una funcion pura, sin estado.
//! - No es un servicio. No abre conexiones, no consulta stores.
//! - No es una traduccion a un wire format externo. `assurance-evidence/v1`
//!   sigue siendo el contrato con el consumer Kotlin; este modulo es el
//!   *seam* interno entre el kernel y Guard dentro de CogniCode.

use crate::domain::findings::DetectorExecutionRef;
use crate::domain::findings::projection::{
    FindingProjection, Gap, ProjectionError, TypedEvidenceHandle, UnknownReason,
};
use crate::domain::kernel_ids::SnapshotId;

/// Lo que Guard ve al consumir una proyeccion de CogniCode.
///
/// **No** incluye `signature`, `admitted_at`, ni `AdmitterRole`:
/// esos son autoridad de Guard, no de CogniCode. Si el caller los
/// anyade, los esta fabricando y el contrato de CU-2.3 queda
/// violado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardObservation {
    /// Evidencia verificada bajo un snapshot especifico, con cadena
    /// causal grounded. UNICA variante que Guard puede considerar
    /// como base de un `Block` — y solo despues de verificar
    /// admission contra su trust root.
    Verifiable {
        snapshot_id: SnapshotId,
        detector: DetectorExecutionRef,
        evidence: Vec<TypedEvidenceHandle>,
        causes: Vec<crate::domain::findings::projection::CauseProjection>,
    },
    /// Observacion informativa. Consultivo, NUNCA gateable. Guard
    /// puede surfacearlo pero no promoverlo a `Block`.
    Advisory {
        snapshot_id: SnapshotId,
        detector: DetectorExecutionRef,
        evidence: Vec<TypedEvidenceHandle>,
        message: String,
    },
    /// El finding no tiene suficiente informacion. Los gaps se
    /// exponen al consumer; no se inventan valores por defecto.
    Inconclusive {
        snapshot_id: SnapshotId,
        detector: DetectorExecutionRef,
        gaps: Vec<Gap>,
    },
    /// La proyeccion fallo (snapshot mismatch, evidence class
    /// desconocida, sin evidence). Fail-closed: el consumer no
    /// puede promover esto a un `Block`.
    Unsupported { reason: UnknownReason },
    /// La proyeccion llevo solo gaps, sin `SnapshotId` ni
    /// `DetectorExecutionRef` (caso de
    /// `ProjectionError::Inconclusive(Vec<Gap>)`). NO se
    /// promueve a `Block`: el discriminante solo es
    /// suficiente para que Guard lo rechace. La ausencia de
    /// provenance es estructural, no un magic number.
    /// (H0.4 — antes el seam fabricaba `SnapshotId(0)` y un
    /// detector sintetico, lo que ocultaba que la proyeccion
    /// no tenia contexto.)
    Ungrounded { gaps: Vec<Gap> },
    /// Error estructural en la proyeccion. Distinto de
    /// `Unsupported` para que el consumer pueda distinguir
    /// "datos invalidos" de "datos faltantes".
    Failed { message: String },
}

/// Transfiere una proyeccion a la observacion que Guard ve.
///
/// Esta es la unica funcion de decision del modulo. Es
/// determinista, sin efectos, y **no** enriquece la proyeccion
/// con datos de autoridad: ni firma, ni `admitted_at`, ni
/// `AdmitterRole`. Si Guard quiere firmar, debe hacerlo a partir
/// de esta observacion, con su propio trust root.
pub fn to_guard_observation(
    result: &Result<FindingProjection, ProjectionError>,
) -> GuardObservation {
    match result {
        Ok(FindingProjection::Verified {
            snapshot_id,
            detector,
            evidence,
            causes,
            ..
        }) => GuardObservation::Verifiable {
            snapshot_id: *snapshot_id,
            detector: detector.clone(),
            evidence: evidence.clone(),
            causes: causes.clone(),
        },
        Ok(FindingProjection::Advisory {
            snapshot_id,
            detector,
            evidence,
            message,
            ..
        }) => GuardObservation::Advisory {
            snapshot_id: *snapshot_id,
            detector: detector.clone(),
            evidence: evidence.clone(),
            message: message.clone(),
        },
        Ok(FindingProjection::Inconclusive {
            snapshot_id,
            detector,
            gaps,
        }) => GuardObservation::Inconclusive {
            snapshot_id: *snapshot_id,
            detector: detector.clone(),
            gaps: gaps.clone(),
        },
        Err(ProjectionError::Unsupported(reason)) => GuardObservation::Unsupported {
            reason: reason.clone(),
        },
        Err(ProjectionError::Inconclusive(gaps)) => {
            // El error lleva solo gaps (no snapshot/detector). H0.4:
            // emitimos un Ungrounded, no un Inconclusive con
            // sentinel. La ausencia de provenance es ahora parte del
            // tipo — el consumer no puede confundirla con un
            // Inconclusive con provenance real.
            GuardObservation::Ungrounded { gaps: gaps.clone() }
        }
        Err(ProjectionError::Failed(message)) => GuardObservation::Failed {
            message: message.clone(),
        },
    }
}
